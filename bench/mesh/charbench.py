import base64
import io
import json
import random
import re
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor

import numpy as np

import chars
import score

import os

PROVIDER = os.environ.get("PROVIDER", "local")
MODEL = os.environ.get("MODEL", "qwen3.8-27b")
OUT = os.environ.get("OUT", "charbench.jsonl")
THINK = os.environ.get("THINK", "1") == "1"
_cfg = json.load(open(os.path.expanduser("~/.pi/agent/models.json")))["providers"][PROVIDER]
URL = _cfg["baseUrl"].rstrip("/") + "/chat/completions"
KEY = _cfg.get("apiKey", "")
OBJ_RES = {"obj6": 6, "obj10": 10}

PRIM_SPEC = """A character is a list of parts, one per line. World units: the character fits in a 48 x 48 x 48 box, z is up, the character faces -y, +x is its left. Part lines:
ellipsoid NAME cx cy cz rx ry rz
capsule NAME x1 y1 z1 x2 y2 z2 radius      (a cylinder with rounded ends between two points)
cylinder NAME x1 y1 z1 x2 y2 z2 r1 r2      (a flat-ended cone frustum, radius r1 at point 1 and r2 at point 2)
box NAME cx cy cz sx sy sz                 (axis aligned, centre and full size)
"""
OBJ_SPEC = "Wavefront OBJ, triangles only, integer coordinates. World units: the character fits in a 48 x 48 x 48 box, z is up, the character faces -y, +x is its left. Each part is an `o NAME` group followed by its `v` and `f` lines, indices are global and 1-based.\n"
IMG_NOTE = ("The images are orthographic renders of the same character, each 384 px square covering 56 world units, so 1 unit is about 6.9 px, and the image centre is the world point (24, 24, 20). "
            "Views: front (camera at -y, +x to the right, +z up), side (camera at +x, +y to the right, +z up), top (camera at +z, +x to the right, +y up), and a 3/4 iso view from front-right-above.\n")


def b64(im):
    b = io.BytesIO()
    im.save(b, "PNG")
    return "data:image/png;base64," + base64.b64encode(b.getvalue()).decode()


def call(content):
    return call_messages([{"role": "user", "content": content}])


def call_messages(messages):
    body = {"model": MODEL, "max_tokens": 30000, "temperature": 0, "messages": messages,
            **({"chat_template_kwargs": {"enable_thinking": THINK}} if PROVIDER == "local" else {})}
    q = urllib.request.Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json", "Authorization": "Bearer " + KEY})
    err = ""
    for _ in range(2):
        try:
            o = json.load(urllib.request.urlopen(q, timeout=900))
            return o["choices"][0]["message"].get("content") or "", o["usage"]["prompt_tokens"], o["usage"]["completion_tokens"]
        except Exception as e:
            err = str(e)
            time.sleep(2)
    return "ERR " + err, 0, 0


def strip_fence(text):
    m = re.findall(r"```[a-zA-Z]*\n(.*?)```", text, re.S)
    return max(m, key=len) if m else text


def encode(prims, rep):
    return "\n".join(p.dsl() for p in prims) if rep == "prim" else chars.to_obj(prims, OBJ_RES[rep])


def decode(text, rep):
    text = strip_fence(text)
    if rep == "prim":
        ps = chars.parse_prims(text)
        if not ps:
            return None
        v, f, _ = chars.mesh_of(ps, 8)
        return v, f
    return chars.parse_obj(text)


def truth_mesh(prims, rep):
    res = 8 if rep == "prim" else OBJ_RES[rep]
    v, f, _ = chars.mesh_of(prims, res)
    return (v, f) if rep == "prim" else (np.round(v), f)


def views(prims, names, size=384):
    v, f, _ = chars.mesh_of(prims, 12)
    return [score.render_image(v, f, *score.VIEWS[n], size=size) for n in names]


EDIT_KINDS = ["scale", "move", "swing", "delete"]


def edit_job(seed, rep, k):
    r = random.Random(seed * 10 + k)
    base = chars.make_char(seed)
    kind, text, out = chars.random_edit(base, r, EDIT_KINDS[k])
    spec = PRIM_SPEC if rep == "prim" else OBJ_SPEC
    prompt = f"{spec}\nHere is a character:\n\n{encode(base, rep)}\n\nEdit: {text}\nEverything else must stay exactly as it is. Reply with the complete edited character in the same format, nothing else."
    return dict(task="edit", rep=rep, seed=seed, kind=kind, content=[{"type": "text", "text": prompt}],
                truth=truth_mesh(out, rep), start=truth_mesh(base, rep))


def create_job(seed, rep, vnames):
    base = chars.make_char(seed)
    spec = PRIM_SPEC if rep == "prim" else OBJ_SPEC
    content = [{"type": "image_url", "image_url": {"url": b64(i)}} for i in views(base, vnames)]
    names = ", ".join(vnames)
    content.append({"type": "text", "text": f"{IMG_NOTE}Images given, in order: {names}.\n\n{spec}\nWrite a character that reproduces the one in the images as closely as you can, using about 8 to 12 parts"
                    f"{' of a few dozen vertices each' if rep != 'prim' else ''}. Reply with only the character."})
    return dict(task="create", rep=rep, seed=seed, kind="+".join(vnames), content=content, truth=truth_mesh(base, rep), start=None)


def run(job):
    t = time.time()
    ans, ptok, ctok = call(job["content"])
    pred = decode(ans, job["rep"]) if not ans.startswith("ERR") else None
    s = score.evaluate(pred, job["truth"])
    base = score.evaluate(job["start"], job["truth"]) if job["start"] is not None else None
    return dict(task=job["task"], rep=job["rep"], seed=job["seed"], kind=job["kind"], ptok=ptok, ctok=ctok, secs=round(time.time() - t),
                base_iou=base and base["iou"], **s, ans=ans)


def report(res):
    for task in ("edit", "create"):
        rows = [x for x in res if x["task"] == task]
        if not rows:
            continue
        print(f"\n{task}: rep/kind  n  valid  iou  chamfer  (do-nothing iou)  prompt_tok  completion_tok")
        keys = sorted({(x["rep"], x["kind"] if task == "create" else "") for x in rows})
        for rep, kind in keys:
            g = [x for x in rows if x["rep"] == rep and (task == "edit" or x["kind"] == kind)]
            v = [x for x in g if x["valid"]]
            bi = [x["base_iou"] for x in g if x["base_iou"] is not None]
            print(f"{rep:7} {kind:22} {len(g):2} {len(v) / len(g):5.2f} {np.mean([x['iou'] for x in g]):6.3f} "
                  f"{np.mean([x['chamfer'] for x in v]) if v else float('nan'):7.2f}  {np.mean(bi) if bi else float('nan'):6.3f}  "
                  f"{int(np.mean([x['ptok'] for x in g])):6} {int(np.mean([x['ctok'] for x in g])):6}")
    ed = [x for x in res if x["task"] == "edit"]
    if ed:
        print("\nedit by kind (iou, do-nothing iou):")
        for rep in sorted({x["rep"] for x in ed}):
            line = []
            for kd in ("scale", "move", "swing", "delete"):
                g = [x for x in ed if x["rep"] == rep and x["kind"] == kd]
                if g:
                    line.append(f"{kd} {np.mean([x['iou'] for x in g]):.2f}/{np.mean([x['base_iou'] for x in g]):.2f}")
            print(f"{rep:7} " + "  ".join(line))


def main():
    nseed = int(sys.argv[1])
    only = sys.argv[2] if len(sys.argv) > 2 else "edit,create"
    jobs = []
    for seed in range(nseed):
        if "edit" in only:
            for rep in ("prim", "obj6", "obj10"):
                for k in range(4):
                    jobs.append(edit_job(seed, rep, k))
        if "create" in only:
            for rep in ("prim", "obj6"):
                jobs.append(create_job(seed, rep, ["front", "side", "top", "iso"]))
            jobs.append(create_job(seed, "prim", ["iso"]))
            jobs.append(create_job(seed, "prim", ["front", "side"]))
    out = open(OUT, "w")
    res = []
    with ThreadPoolExecutor(8) as ex:
        for x in ex.map(run, jobs):
            res.append(x)
            out.write(json.dumps(x) + "\n")
            out.flush()
    report(res)


if __name__ == "__main__":
    if sys.argv[1] == "report":
        report([json.loads(l) for l in open(OUT)])
    else:
        main()
