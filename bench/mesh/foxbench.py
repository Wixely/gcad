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

from briefs import BRIEFS

BRIEF_NAME = os.environ.get("BRIEF", "fox")
BRIEF = BRIEFS[BRIEF_NAME]["task"]
PRICE_IN = float(os.environ.get("PRICE_IN", "5"))
PRICE_OUT = float(os.environ.get("PRICE_OUT", "25"))
TURNS = int(os.environ.get("TURNS", "24"))
CFG = json.load(open(os.path.expanduser("~/.pi/agent/models.json")))["providers"]["yalr"]
MODEL = os.environ.get("MODEL", "code-x-high")
OUT = os.environ.get("OUT", "/tmp/mb/bmcp/run")
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
GCAD = os.environ.get("GCAD", f"{ROOT}/target/release/gcad")


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


LOOK = {"mode": "angles", "views": ["three_quarter", "top", "front", "right"], "shading": "material", "max_size": 1200}

IMPORT_OBJ = """import bpy
for o in list(bpy.data.objects): bpy.data.objects.remove(o, do_unlink=True)
win = bpy.context.window_manager.windows[0]
area = [a for a in win.screen.areas if a.type == "VIEW_3D"][0]
with bpy.context.temp_override(window=win, area=area, screen=win.screen):
    bpy.ops.wm.obj_import(filepath=%r, forward_axis="Y", up_axis="Z")
for o in bpy.context.scene.objects:
    o.select_set(True)
    if o.type != "MESH" or not o.data.color_attributes:
        continue
    attribute = o.data.color_attributes[0].name
    for slot in o.material_slots:
        m = slot.material
        if not m or not m.use_nodes:
            continue
        bsdf = next(n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
        node = m.node_tree.nodes.new("ShaderNodeVertexColor")
        node.layer_name = attribute
        m.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Base Color"])
"""

HIDE_GROUND = "import bpy\nfor o in bpy.data.objects:\n    if o.type=='MESH' and max(o.dimensions)>120: o.hide_set(True)"
SHOW_ALL = "import bpy\nfor o in bpy.data.objects: o.hide_set(False)"


async def look_png(session, path, hide=True):
    if hide:
        await session.call_tool("execute_blender_code", {"code": HIDE_GROUND})
    shot = await session.call_tool("look", LOOK)
    if hide:
        await session.call_tool("execute_blender_code", {"code": SHOW_ALL})
    for c in shot.content:
        if getattr(c, "type", "") == "image":
            open(path, "wb").write(base64.b64decode(c.data))
            return path
    return None


def blender_params():
    return StdioServerParameters(command="uvx", args=["mcp-for-blender"], env={**os.environ, "DISABLE_TELEMETRY": "1"})


async def obj_in_blender(session, obj, png):
    await session.call_tool("execute_blender_code", {"code": IMPORT_OBJ % obj})
    return await look_png(session, png, hide=False)


def dollars(prompt, completion):
    return round((prompt * PRICE_IN + completion * PRICE_OUT) / 1e6, 4)


async def agent(name, system, tools, run_tool, mutating, snapshot):
    log = Log(name)
    messages = [{"role": "system", "content": system}, {"role": "user", "content": BRIEF}]
    start = time.time()
    paused = 0.0
    tool_seconds = 0.0
    snapshots = []
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
        edited = False
        for c in calls:
            log.usage["calls"] += 1
            try:
                args = json.loads(c["function"]["arguments"] or "{}")
            except json.JSONDecodeError:
                args = {}
            began = time.time()
            text, images = await run_tool(c["function"]["name"], args)
            tool_seconds += time.time() - began
            edited = edited or c["function"]["name"] in mutating
            log.add(turn=turn, tool=c["function"]["name"], result=text[:4000], images=len(images))
            messages.append({"role": "tool", "tool_call_id": c["id"], "content": text[:12000] or "(no text)"})
            pictures.extend(images)
        if pictures:
            log.usage["images"] += len(pictures)
            messages.append({"role": "user", "content": [{"type": "text", "text": "Images from those tool calls:"}] + pictures})
        if edited:
            began = time.time()
            image = await snapshot(len(snapshots))
            paused += time.time() - began
            snapshots.append({
                "n": len(snapshots),
                "image": image,
                "seconds": round(time.time() - start - paused, 1),
                "calls": log.usage["calls"],
                "prompt": log.usage["prompt"],
                "completion": log.usage["completion"],
                "cost": dollars(log.usage["prompt"], log.usage["completion"]),
            })
    log.usage["seconds"] = round(time.time() - start - paused, 1)
    log.usage["tool_seconds"] = round(tool_seconds, 1)
    log.usage["llm_seconds"] = round(log.usage["seconds"] - tool_seconds, 1)
    log.usage["turns"] = turn + 1
    log.usage["cost"] = dollars(log.usage["prompt"], log.usage["completion"])
    log.usage["snapshots"] = snapshots
    log.add(usage=log.usage)
    return log.usage


async def blender():
    async with stdio_client(blender_params()) as (r, w):
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
            async def snapshot(n):
                return await look_png(s, f"{OUT}/blender_step{n}.png")

            usage = await agent("blender", system, tools, run, {"execute_blender_code"}, snapshot)
            export = (
                "import bpy\nbpy.ops.object.select_all(action='SELECT')\n"
                f"bpy.ops.wm.obj_export(filepath='{OUT}/blender.obj', export_selected_objects=False, export_materials=True)\n"
                f"bpy.ops.wm.save_as_mainfile(filepath='{OUT}/blender.blend')\n"
                "print(sum(len(o.data.polygons) for o in bpy.data.objects if o.type=='MESH'))"
            )
            res = await s.call_tool("execute_blender_code", {"code": export})
            usage["export"] = "\n".join(c.text for c in res.content if getattr(c, "type", "") == "text")[-300:]
            await s.call_tool("execute_blender_code", {"code": HIDE_GROUND})
            await look_png(s, f"{OUT}/blender_final.png", hide=False)
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

    skill = open(f"{ROOT}/skills/gcad/SKILL.md").read()
    workflow = skill[skill.index("## Meshes"): skill.index("## Output")]
    system = (
        "You are a 3D artist modelling in gcad, a text CAD. The tools run gcad for you: write_file "
        "then render and inspect (inspect takes the same flags as `gcad inspect`).\n\n"
        + workflow
        + "\n\nThe format reference for mesh bodies:\n\n"
        + meshes
    )
    async with stdio_client(blender_params()) as (r, w):
        async with ClientSession(r, w) as s:
            await s.initialize()

            async def snapshot(n):
                step = f"{OUT}/gcad_step{n}"
                ok = subprocess.run([GCAD, "export", path, step + ".obj"], capture_output=True).returncode == 0
                subprocess.run([GCAD, "render", path, step + ".png"], capture_output=True)
                if not ok:
                    return None
                return await obj_in_blender(s, step + ".obj", step + "_blender.png")

            usage = await agent("gcad", system, tools, run, {"write_file"}, snapshot)
            subprocess.run([GCAD, "render", path, f"{OUT}/gcad_final.png"], capture_output=True)
            subprocess.run([GCAD, "export", path, f"{OUT}/gcad.obj"], capture_output=True)
            await obj_in_blender(s, f"{OUT}/gcad.obj", f"{OUT}/gcad_in_blender.png")
    return usage


async def main():
    which = sys.argv[1:] or ["blender", "gcad"]
    results = {}
    for w in which:
        results[w] = await (blender() if w == "blender" else gcad())
        print(w, json.dumps(results[w]), flush=True)
    json.dump(results, open(f"{OUT}/usage_{'_'.join(which)}.json", "w"), indent=1)


asyncio.run(main())
