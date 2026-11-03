import asyncio
import base64
import json
import os
import re
import subprocess
import sys
import time
import urllib.request

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

BRIEF = (
    "Model a red fox game character from scratch: standing on four legs, about 40 units long, "
    "head with pointed ears, long narrow muzzle and a black nose, almond eyes, white cheeks, chest "
    "and tail tip, dark ears and paws, a long bushy tail. Make it look as good as you can. "
    "Use the tools to look at your work and fix it. Stop when you are happy with it."
)
TURNS = int(os.environ.get("TURNS", "24"))
CFG = json.load(open(os.path.expanduser("~/.pi/agent/models.json")))["providers"]["yalr"]
MODEL = os.environ.get("MODEL", "code-x-high")
OUT = os.environ.get("OUT", "/tmp/mb/bmcp/run")
GCAD = "/home/kieran/git/gcad/target/release/gcad"


def chat(messages, tools):
    body = {"model": MODEL, "messages": messages, "tools": tools, "max_tokens": 16000}
    req = urllib.request.Request(
        CFG["baseUrl"].rstrip("/") + "/chat/completions",
        json.dumps(body).encode(),
        {"Content-Type": "application/json", "Authorization": "Bearer " + CFG["apiKey"]},
    )
    for attempt in range(3):
        try:
            return json.load(urllib.request.urlopen(req, timeout=900))
        except Exception as e:
            err = e
            time.sleep(5)
    raise err


def image_part(b64, mime="image/png"):
    return {"type": "image_url", "image_url": {"url": f"data:{mime};base64,{b64}"}}


class Log:
    def __init__(self, name):
        os.makedirs(OUT, exist_ok=True)
        self.path = f"{OUT}/{name}.jsonl"
        self.f = open(self.path, "w")
        self.usage = {"prompt": 0, "completion": 0, "calls": 0, "images": 0, "seconds": 0.0}

    def add(self, **row):
        self.f.write(json.dumps(row) + "\n")
        self.f.flush()


async def agent(name, system, tools, run_tool):
    log = Log(name)
    messages = [{"role": "system", "content": system}, {"role": "user", "content": BRIEF}]
    start = time.time()
    for turn in range(TURNS):
        reply = chat(messages, tools)
        u = reply.get("usage", {})
        log.usage["prompt"] += u.get("prompt_tokens", 0)
        log.usage["completion"] += u.get("completion_tokens", 0)
        msg = reply["choices"][0]["message"]
        calls = msg.get("tool_calls") or []
        messages.append({k: v for k, v in msg.items() if k in ("role", "content", "tool_calls")})
        log.add(turn=turn, content=msg.get("content"), calls=[c["function"] for c in calls])
        if not calls:
            break
        pictures = []
        for c in calls:
            log.usage["calls"] += 1
            try:
                args = json.loads(c["function"]["arguments"] or "{}")
            except json.JSONDecodeError:
                args = {}
            text, images = await run_tool(c["function"]["name"], args)
            log.add(turn=turn, tool=c["function"]["name"], result=text[:4000], images=len(images))
            messages.append({"role": "tool", "tool_call_id": c["id"], "content": text[:12000] or "(no text)"})
            pictures.extend(images)
        if pictures:
            log.usage["images"] += len(pictures)
            messages.append({"role": "user", "content": [{"type": "text", "text": "Images from those tool calls:"}] + pictures})
    log.usage["seconds"] = round(time.time() - start)
    log.usage["turns"] = turn + 1
    log.add(usage=log.usage)
    return log.usage


async def blender():
    params = StdioServerParameters(command="uvx", args=["mcp-for-blender"], env={**os.environ, "DISABLE_TELEMETRY": "1"})
    async with stdio_client(params) as (r, w):
        async with ClientSession(r, w) as s:
            await s.initialize()
            await s.call_tool("execute_blender_code", {"code": "import bpy\nfor o in list(bpy.data.objects): bpy.data.objects.remove(o)\nfor m in list(bpy.data.meshes): bpy.data.meshes.remove(m)\nfor m in list(bpy.data.materials): bpy.data.materials.remove(m)"})
            keep = {"execute_blender_code", "get_scene_info", "look"}
            listed = [t for t in (await s.list_tools()).tools if t.name in keep]
            tools = [{"type": "function", "function": {"name": t.name, "description": t.description, "parameters": getattr(t, "inputSchema", None) or t.input_schema}} for t in listed]

            async def run(name, args):
                if name not in keep:
                    return f"unknown tool {name}", []
                res = await s.call_tool(name, args)
                text = "\n".join(c.text for c in res.content if getattr(c, "type", "") == "text")
                images = [image_part(c.data, getattr(c, "mime_type", None) or "image/png") for c in res.content if getattr(c, "type", "") == "image"]
                return text, images

            system = "You are a 3D artist working in Blender through these tools. Build everything with Blender Python. Do not import assets."
            usage = await agent("blender", system, tools, run)
            export = (
                "import bpy\nbpy.ops.object.select_all(action='SELECT')\n"
                f"bpy.ops.wm.obj_export(filepath='{OUT}/blender.obj', export_selected_objects=False, export_materials=True)\n"
                f"bpy.ops.wm.save_as_mainfile(filepath='{OUT}/blender.blend')\n"
                "print(sum(len(o.data.polygons) for o in bpy.data.objects if o.type=='MESH'))"
            )
            res = await s.call_tool("execute_blender_code", {"code": export})
            usage["export"] = "\n".join(c.text for c in res.content if getattr(c, "type", "") == "text")[-300:]
            await s.call_tool("execute_blender_code", {"code": "import bpy\nfor o in bpy.data.objects:\n    if o.type=='MESH' and max(o.dimensions)>120: o.hide_set(True)"})
            shot = await s.call_tool("look", {"mode": "angles", "views": ["three_quarter", "top", "front", "right"], "shading": "material", "max_size": 1200})
            for c in shot.content:
                if getattr(c, "type", "") == "image":
                    open(f"{OUT}/blender_final.png", "wb").write(base64.b64decode(c.data))
            return usage


async def gcad():
    docs = subprocess.run([GCAD, "docs"], capture_output=True, text=True).stdout
    meshes = docs[docs.index("## Meshes"): docs.index("## Assemblies")]
    path = f"{OUT}/fox.gcad"
    tools = [
        {"type": "function", "function": {"name": "write_file", "description": "Replace the whole fox.gcad file. Returns `gcad check` output for every line.", "parameters": {"type": "object", "properties": {"content": {"type": "string"}}, "required": ["content"]}}},
        {"type": "function", "function": {"name": "render", "description": "Four lit views of the whole model (iso, top, front, right) as one image.", "parameters": {"type": "object", "properties": {}}}},
        {"type": "function", "function": {"name": "inspect", "description": "One close view with a coordinate grid, part names, surface normals as colour; picks return the surface point under a pixel. Same flags as `gcad inspect`.", "parameters": {"type": "object", "properties": {"view": {"type": "string"}, "part": {"type": "string"}, "line": {"type": "integer"}, "pick": {"type": "array", "items": {"type": "string"}}, "mark": {"type": "array", "items": {"type": "string"}}, "at": {"type": "string"}, "span": {"type": "number"}}}}},
    ]

    async def run(name, args):
        if name == "write_file":
            open(path, "w").write(args.get("content", ""))
            r = subprocess.run([GCAD, "check", path], capture_output=True, text=True, timeout=600)
            return (r.stdout + r.stderr)[-6000:], []
        if name == "render":
            r = subprocess.run([GCAD, "render", path, f"{OUT}/gcad_render.png"], capture_output=True, text=True, timeout=600)
            if r.returncode:
                return r.stderr[-2000:], []
            return "rendered", [image_part(base64.b64encode(open(f"{OUT}/gcad_render.png", "rb").read()).decode())]
        if name == "inspect":
            cmd = [GCAD, "inspect", path, f"{OUT}/gcad_inspect.png", "--view", args.get("view", "front")]
            for key in ("part", "line", "at", "span"):
                if args.get(key) is not None:
                    cmd += [f"--{key}", str(args[key])]
            for p in args.get("pick") or []:
                cmd += ["--pick", p]
            for m in args.get("mark") or []:
                cmd += ["--mark", m]
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
            if r.returncode:
                return r.stderr[-2000:], []
            return r.stdout, [image_part(base64.b64encode(open(f"{OUT}/gcad_inspect.png", "rb").read()).decode())]
        return f"unknown tool {name}", []

    skill = open("/home/kieran/git/gcad/skills/gcad/SKILL.md").read()
    workflow = skill[skill.index("## Meshes"): skill.index("## Output")]
    system = (
        "You are a 3D artist modelling in gcad, a text CAD. The tools run gcad for you: write_file "
        "then render and inspect (inspect takes the same flags as `gcad inspect`).\n\n"
        + workflow
        + "\n\nThe format reference for mesh bodies:\n\n"
        + meshes
    )
    usage = await agent("gcad", system, tools, run)
    subprocess.run([GCAD, "render", path, f"{OUT}/gcad_final.png"], capture_output=True)
    subprocess.run([GCAD, "export", path, f"{OUT}/gcad.obj"], capture_output=True)
    return usage


async def main():
    which = sys.argv[1:] or ["blender", "gcad"]
    results = {}
    for w in which:
        results[w] = await (blender() if w == "blender" else gcad())
        print(w, json.dumps(results[w]), flush=True)
    json.dump(results, open(f"{OUT}/usage_{'_'.join(which)}.json", "w"), indent=1)


asyncio.run(main())
