import json
import os
import time
from dataclasses import dataclass, field

import httpx
from openai import AsyncOpenAI
from pydantic_ai import Agent
from pydantic_ai.models import ModelRequestParameters
from pydantic_ai.models.openai import OpenAIChatModel
from pydantic_ai.models.wrapper import WrapperModel
from pydantic_ai.providers.openai import OpenAIProvider
from pydantic_ai.toolsets import WrapperToolset
from pydantic_ai.usage import UsageLimits


@dataclass
class Prices:
    input: float = 4.0
    cache_read: float = 0.20
    output: float = 20.0
    source: str = "Anthropic list price for Claude Opus 5.5 (platform.claude.com pricing), $ per million tokens"


def cost(prices, uncached, cached, output):
    return round((uncached * prices.input + cached * prices.cache_read + output * prices.output) / 1e6, 5)


@dataclass
class Recorder:
    prices: Prices
    requests: list = field(default_factory=list)
    tools: list = field(default_factory=list)
    http: list = field(default_factory=list)
    pending: dict = field(default_factory=dict)
    paused: float = 0.0

    def request_row(self, seconds, usage, calls):
        cached = usage.cache_read_tokens
        row = {
            "n": len(self.requests),
            "seconds": round(seconds, 2),
            "input": usage.input_tokens,
            "cached": cached,
            "uncached": usage.input_tokens - cached,
            "output": usage.output_tokens,
            "reasoning": usage.details.get("reasoning_tokens", 0),
            "tool_calls": calls,
            "cost": cost(self.prices, usage.input_tokens - cached, cached, usage.output_tokens),
        }
        self.requests.append(row)
        return row

    def totals(self):
        r = self.requests
        return {
            "input": sum(x["input"] for x in r),
            "cached": sum(x["cached"] for x in r),
            "uncached": sum(x["uncached"] for x in r),
            "output": sum(x["output"] for x in r),
            "reasoning": sum(x["reasoning"] for x in r),
            "model_seconds": round(sum(x["seconds"] for x in r), 1),
            "tool_seconds": round(sum(t["seconds"] for t in self.tools), 1),
            "snapshot_seconds": round(self.paused, 1),
            "cost": round(sum(x["cost"] for x in r), 4),
            "requests": len(r),
            "tool_calls": len(self.tools),
            "http_attempts": len(self.http),
            "http_errors": sum(1 for h in self.http if h["status"] != 200),
        }


class TimedModel(WrapperModel):
    def __init__(self, wrapped, recorder):
        super().__init__(wrapped)
        self.recorder = recorder

    async def request(self, messages, model_settings, model_request_parameters: ModelRequestParameters):
        began = time.time()
        response = await self.wrapped.request(messages, model_settings, model_request_parameters)
        calls = sum(1 for p in response.parts if p.part_kind == "tool-call")
        self.recorder.request_row(time.time() - began, response.usage, calls)
        return response


class TimedToolset(WrapperToolset):
    def __init__(self, wrapped, recorder, mutating, snapshot):
        super().__init__(wrapped)
        self.recorder = recorder
        self.mutating = mutating
        self.snapshot = snapshot
        self.snapshots = []
        self.clock = time.time()

    async def call_tool(self, name, tool_args, ctx, tool):
        began = time.time()
        try:
            return await self.wrapped.call_tool(name, tool_args, ctx, tool)
        finally:
            self.recorder.tools.append({"name": name, "seconds": round(time.time() - began, 2), "after_request": len(self.recorder.requests)})
            if name in self.mutating and self.snapshot:
                shot = time.time()
                image = await self.snapshot(len(self.snapshots))
                self.recorder.paused += time.time() - shot
                totals = self.recorder.totals()
                self.snapshots.append({
                    "n": len(self.snapshots),
                    "image": image,
                    "seconds": round(time.time() - self.clock - self.recorder.paused, 1),
                    "calls": totals["tool_calls"],
                    "cost": totals["cost"],
                })


def build_model(name, base_url, api_key, recorder):
    async def on_request(request):
        recorder.pending[id(request)] = time.time()

    async def on_response(response):
        began = recorder.pending.pop(id(response.request), time.time())
        recorder.http.append({"status": response.status_code, "seconds": round(time.time() - began, 2)})

    client = AsyncOpenAI(
        base_url=base_url,
        api_key=api_key,
        max_retries=2,
        timeout=900,
        http_client=httpx.AsyncClient(event_hooks={"request": [on_request], "response": [on_response]}, timeout=900),
    )
    return TimedModel(OpenAIChatModel(name, provider=OpenAIProvider(openai_client=client)), recorder)


async def run_side(*, model_name, base_url, api_key, system, task, toolset, mutating, snapshot, turns, prices=None):
    recorder = Recorder(prices or Prices())
    timed = TimedToolset(toolset, recorder, mutating, snapshot)
    agent = Agent(
        build_model(model_name, base_url, api_key, recorder),
        instructions=system,
        toolsets=[timed],
        model_settings={"cache": True, "max_tokens": 16000},
    )
    start = time.time()
    stopped = None
    try:
        async with agent:
            result = await agent.run(task, usage_limits=UsageLimits(request_limit=turns))
            history = result.all_messages()
    except Exception as e:
        stopped = repr(e)[:300]
        history = []
    wall = time.time() - start - recorder.paused
    record = {
        **recorder.totals(),
        "seconds": round(wall, 1),
        "stopped": stopped,
        "prices": {"input": recorder.prices.input, "cache_read": recorder.prices.cache_read, "output": recorder.prices.output, "source": recorder.prices.source},
        "request_log": recorder.requests,
        "tool_log": recorder.tools,
        "http_log": recorder.http,
        "snapshots": timed.snapshots,
    }
    record["slowest_request"] = max(recorder.requests, key=lambda r: r["seconds"], default=None)
    return record, history


def save(path, record):
    json.dump(record, open(path, "w"), indent=1)
