import asyncio
import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

import pytest
from pydantic_ai.toolsets import FunctionToolset

import harness


class FakeRouter:
    def __init__(self, script):
        self.script = script
        self.bodies = []
        outer = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                outer.bodies.append(body)
                reply = outer.script[min(len(outer.bodies), len(outer.script)) - 1]
                data = json.dumps(reply).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def log_message(self, *args):
                pass

        self.server = HTTPServer(("127.0.0.1", 0), Handler)
        self.url = f"http://127.0.0.1:{self.server.server_port}/v1"
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def close(self):
        self.server.shutdown()


def completion(message, prompt, cached, output, reasoning=0):
    return {
        "id": "x", "object": "chat.completion", "created": 0, "model": "fake",
        "choices": [{"index": 0, "finish_reason": "stop", "message": {"role": "assistant", **message}}],
        "usage": {
            "prompt_tokens": prompt, "completion_tokens": output, "total_tokens": prompt + output,
            "prompt_tokens_details": {"cached_tokens": cached},
            "completion_tokens_details": {"reasoning_tokens": reasoning},
        },
    }


def tool_call(name, args, call_id="c1"):
    return {"content": None, "tool_calls": [{"id": call_id, "type": "function", "function": {"name": name, "arguments": json.dumps(args)}}]}


def test_cost_prices_cached_input_at_the_cache_rate():
    prices = harness.Prices(input=4.0, cache_read=0.2, output=20.0)
    assert harness.cost(prices, uncached=1_000_000, cached=0, output=0) == 4.0
    assert harness.cost(prices, uncached=0, cached=1_000_000, output=0) == 0.2
    assert harness.cost(prices, uncached=500_000, cached=500_000, output=100_000) == pytest.approx(2.0 + 0.1 + 2.0)


def run(router, toolset, turns=10, mutating=(), snapshot=None):
    return asyncio.run(harness.run_side(
        model_name="fake", base_url=router.url, api_key="k", system="sys", task="go",
        toolset=toolset, mutating=set(mutating), snapshot=snapshot, turns=turns,
    ))


def test_usage_splits_cached_and_uncached_input_per_request():
    router = FakeRouter([
        completion(tool_call("add", {"a": 1, "b": 2}), prompt=1000, cached=0, output=50),
        completion({"content": "done"}, prompt=1200, cached=1000, output=10, reasoning=4),
    ])
    toolset = FunctionToolset()
    toolset.add_function(lambda a, b: a + b, name="add")
    record, _ = run(router, toolset)
    router.close()
    assert [r["cached"] for r in record["request_log"]] == [0, 1000]
    assert [r["uncached"] for r in record["request_log"]] == [1000, 200]
    assert record["input"] == 2200 and record["cached"] == 1000 and record["uncached"] == 1200
    assert record["output"] == 60 and record["tool_calls"] == 1 and record["requests"] == 2
    assert record["cost"] == pytest.approx(harness.cost(harness.Prices(), 1200, 1000, 60), abs=1e-4)
    assert record["http_attempts"] == 2 and record["http_errors"] == 0


def test_tool_calls_are_dispatched_and_timed():
    router = FakeRouter([
        completion(tool_call("shout", {"word": "fox"}), 100, 0, 5),
        completion({"content": "ok"}, 120, 100, 2),
    ])
    seen = []
    toolset = FunctionToolset()
    toolset.add_function(lambda word: seen.append(word) or word.upper(), name="shout")
    record, _ = run(router, toolset)
    result_message = router.bodies[1]["messages"][-1]
    router.close()
    assert seen == ["fox"]
    assert result_message["role"] == "tool" and "FOX" in result_message["content"]
    assert [t["name"] for t in record["tool_log"]] == ["shout"]


def test_mutating_tools_trigger_a_snapshot_that_is_excluded_from_wall_clock():
    router = FakeRouter([
        completion(tool_call("edit", {}), 100, 0, 5),
        completion({"content": "ok"}, 120, 100, 2),
    ])
    toolset = FunctionToolset()
    toolset.add_function(lambda: "edited", name="edit")

    async def snapshot(n):
        await asyncio.sleep(0.3)
        return f"shot{n}.png"

    record, _ = run(router, toolset, mutating={"edit"}, snapshot=snapshot)
    router.close()
    assert [s["image"] for s in record["snapshots"]] == ["shot0.png"]
    assert record["snapshot_seconds"] >= 0.3
    assert record["seconds"] < record["model_seconds"] + record["tool_seconds"] + 0.3


def test_the_turn_cap_stops_the_run_and_keeps_the_usage():
    router = FakeRouter([completion(tool_call("loop", {}), 100, 0, 5)])
    toolset = FunctionToolset()
    toolset.add_function(lambda: "again", name="loop")
    record, _ = run(router, toolset, turns=3)
    router.close()
    assert record["requests"] == 3 and record["stopped"] and "request_limit" in record["stopped"]


def test_each_request_extends_the_previous_one_so_the_router_can_cache_the_prefix():
    router = FakeRouter([
        completion(tool_call("t", {}), 100, 0, 5),
        completion({"content": "hi"}, 120, 100, 5),
    ])
    toolset = FunctionToolset()
    toolset.add_function(lambda: "x", name="t")
    run(router, toolset)
    first, second = router.bodies
    router.close()
    assert first["messages"][0] == {"role": "system", "content": "sys"}
    assert second["messages"][: len(first["messages"])] == first["messages"]
    assert second["tools"] == first["tools"]
