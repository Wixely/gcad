import asyncio
import itertools
import base64
import inspect as pyinspect
import json
import os
import subprocess
import sys
import time

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from pydantic_ai import BinaryContent
from pydantic_ai.mcp import MCPToolset
from pydantic_ai.toolsets import FunctionToolset

import harness
import prompts
from briefs import BRIEFS

BRIEF_NAME = os.environ.get("BRIEF", "fox")
TASK = BRIEFS[BRIEF_NAME]["task"]
TURNS = int(os.environ.get("TURNS", "24"))
CFG = json.load(open(os.path.expanduser("~/.pi/agent/models.json")))["providers"]["yalr"]
MODEL = os.environ.get("MODEL", "code-x-high")
OUT = os.environ.get("OUT", "/tmp/mb/bmcp/run")
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
GCAD = os.environ.get("GCAD", f"{ROOT}/target/release/gcad")

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

CLEAR_SCENE = "import bpy\nfor o in list(bpy.data.objects): bpy.data.objects.remove(o)\nfor m in list(bpy.data.meshes): bpy.data.meshes.remove(m)\nfor m in list(bpy.data.materials): bpy.data.materials.remove(m)"
HIDE_GROUND = "import bpy\nfor o in bpy.data.objects:\n    if o.type=='MESH' and max(o.dimensions)>120: o.hide_set(True)"
SHOW_ALL = "import bpy\nfor o in bpy.data.objects: o.hide_set(False)"


def blender_params():
    return StdioServerParameters(command="uvx", args=["mcp-for-blender"], env={**os.environ, "DISABLE_TELEMETRY": "1"})


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


async def obj_in_blender(session, obj, png):
    await session.call_tool("execute_blender_code", {"code": IMPORT_OBJ % obj})
    return await look_png(session, png, hide=False)


def picture(path):
    data = open(path, "rb").read() if os.path.exists(path) else b""
    if not data:
        return "(the tool produced no image)"
    return BinaryContent(data, media_type="image/png")


def gcad_toolset(path):
    toolset = FunctionToolset()
    counter = itertools.count()

    @toolset.tool_plain
    def write_file(content: str):
        """Replace the whole fox.gcad file. Returns `gcad check` output for every line."""
        open(path, "w").write(content)
        r = subprocess.run([GCAD, "check", path], capture_output=True, text=True, timeout=600)
        return (r.stdout + r.stderr)[-6000:] or "(no output)"

    @toolset.tool_plain
    def render():
        """Four lit views of the whole model (iso, top, front, right) as one image."""
        image = f"{OUT}/gcad_render_{next(counter)}.png"
        r = subprocess.run([GCAD, "render", path, image], capture_output=True, text=True, timeout=600)
        if r.returncode:
            return r.stderr[-2000:]
        return [picture(image)]

    @toolset.tool_plain
    def inspect(view: str = "front", part: str | None = None, line: int | None = None, pick: list[str] | None = None, mark: list[str] | None = None, at: str | None = None, span: float | None = None):
        """One close view with a coordinate grid, part names, surface normals as colour; picks return the surface point under a pixel. Same flags as `gcad inspect`."""
        image = f"{OUT}/gcad_inspect_{next(counter)}.png"
        cmd = [GCAD, "inspect", path, image, "--view", view]
        for key, value in (("part", part), ("line", line), ("at", at), ("span", span)):
            if value is not None:
                cmd += [f"--{key}", str(value)]
        for p in pick or []:
            cmd += ["--pick", p]
        for m in mark or []:
            cmd += ["--mark", m]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        if r.returncode:
            return r.stderr[-2000:]
        return [r.stdout or "(no text)", picture(image)]

    return toolset


def gcad_system():
    docs = subprocess.run([GCAD, "docs"], capture_output=True, text=True).stdout
    reference = docs[docs.index("## Meshes"): docs.index("## Assemblies")]
    skill = open(f"{ROOT}/skills/gcad/SKILL.md").read()
    return prompts.gcad_system(skill[skill.index("## Meshes"): skill.index("## Output")], reference)


async def run_blender():
    async with stdio_client(blender_params()) as (r, w):
        async with ClientSession(r, w) as s:
            await s.initialize()
            await s.call_tool("execute_blender_code", {"code": CLEAR_SCENE})

            async def snapshot(n):
                return await look_png(s, f"{OUT}/blender_step{n}.png")

            keep = {"execute_blender_code", "get_scene_info", "look"}
            toolset = MCPToolset({"mcpServers": {"blender": {"command": "uvx", "args": ["mcp-for-blender"], "env": {"DISABLE_TELEMETRY": "1"}}}})
            record, history = await harness.run_side(
                model_name=MODEL, base_url=CFG["baseUrl"], api_key=CFG["apiKey"],
                system=prompts.blender_system(), task=TASK,
                toolset=toolset.filtered(lambda ctx, tool: tool.name in keep),
                mutating={"execute_blender_code"}, snapshot=snapshot, turns=TURNS,
            )
            export = (
                "import bpy\nbpy.ops.object.select_all(action='SELECT')\n"
                f"bpy.ops.wm.obj_export(filepath='{OUT}/blender.obj', export_selected_objects=False, export_materials=True)\n"
                f"bpy.ops.wm.save_as_mainfile(filepath='{OUT}/blender.blend')\n"
                "print(sum(len(o.data.polygons) for o in bpy.data.objects if o.type=='MESH'))"
            )
            await s.call_tool("execute_blender_code", {"code": export})
            await s.call_tool("execute_blender_code", {"code": HIDE_GROUND})
            await look_png(s, f"{OUT}/blender_final.png", hide=False)
            return record


async def run_gcad():
    path = f"{OUT}/fox.gcad"
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

            record, history = await harness.run_side(
                model_name=MODEL, base_url=CFG["baseUrl"], api_key=CFG["apiKey"],
                system=gcad_system(), task=TASK, toolset=gcad_toolset(path),
                mutating={"write_file"}, snapshot=snapshot, turns=TURNS,
            )
            subprocess.run([GCAD, "render", path, f"{OUT}/gcad_final.png"], capture_output=True)
            subprocess.run([GCAD, "export", path, f"{OUT}/gcad.obj"], capture_output=True)
            await obj_in_blender(s, f"{OUT}/gcad.obj", f"{OUT}/gcad_in_blender.png")
            return record


async def main():
    os.makedirs(OUT, exist_ok=True)
    which = sys.argv[1:] or ["gcad", "blender"]
    for side in which:
        record = await (run_blender() if side == "blender" else run_gcad())
        record["system_prompt_chars"] = len(prompts.blender_system() if side == "blender" else gcad_system())
        harness.save(f"{OUT}/usage_{side}.json", {side: record})
        print(side, json.dumps({k: v for k, v in record.items() if not k.endswith("_log") and k != "snapshots"}), flush=True)


if __name__ == "__main__":
    asyncio.run(main())
