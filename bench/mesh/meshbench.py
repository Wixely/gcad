import json
import random
import sys
from concurrent.futures import ThreadPoolExecutor

import numpy as np

import charbench as cb
import meshrep as M
import score

BUDGET = 400
VOX_Q = 2
Q = {"obj": 1, "soup": 1, "vox": VOX_Q}
EDITS = ["shift_top", "stretch_x", "flatten", "rotate", "delete"]

SPEC = {
    "obj": "Wavefront OBJ, triangles only, integer coordinates, `v x y z` and `f a b c` with global 1-based indices. World units: the model fits in a 48 x 48 x 48 box, z is up, the model faces -y, +x is its left.",
    "soup": "A triangle list, one triangle per line as `x1 y1 z1 x2 y2 z2 x3 y3 z3`, integer coordinates. World units: the model fits in a 48 x 48 x 48 box, z is up, the model faces -y, +x is its left.",
    "vox": "A voxel model. Each line is `x y zruns`: the solid unit cells in the column at integer (x, y), with z given as runs like `3-9,12` meaning cells z=3..9 and z=12. Cells are 2 world units wide, so the model fits in a 24 x 24 x 24 grid, z is up, the model faces -y, +x is its left.",
}
ENC = {"obj": lambda s: M.enc_obj(*s), "soup": lambda s: M.enc_soup(*s), "vox": lambda s: M.enc_vox(s)}
DEC = {"obj": M.dec_obj, "soup": M.dec_soup, "vox": M.dec_vox}


def build_state(name, rep, orig):
    v, f = orig
    if rep == "vox":
        return M.voxelise(v, f, VOX_Q)
    return M.decimate(v, f, BUDGET, 1)


def tri_set(mesh):
    if mesh is None:
        return set()
    v, f = mesh
    return {tuple(sorted(map(tuple, np.round(v[t], 2)))) for t in f}


def jaccard(a, b):
    a, b = tri_set(a), tri_set(b)
    u = len(a | b)
    return len(a & b) / u if u else 1.0


def edit_job(name, rep, kind, orig):
    r = random.Random(hash((name, kind)) & 0xFFFF)
    state = build_state(name, rep, orig)
    kd, text, spec = M.edit_spec(rep, state, r, kind)
    new = M.apply_edit(rep, state, spec)
    prompt = (f"{SPEC[rep]}\n\nHere is a model:\n\n{ENC[rep](state)}\n\nEdit: {text}\nNothing else changes. Reply with the complete edited model in the same format, nothing else.")
    return dict(task="edit", rep=rep, name=name, kind=kind, content=[{"type": "text", "text": prompt}],
                truth=M.to_world(rep, new, Q[rep]), start=M.to_world(rep, state, Q[rep]), orig=orig)


def create_job(name, rep, vnames, orig):
    state = build_state(name, rep, orig)
    ims = [cb.b64(i) for i in [score.render_image(*orig, *score.VIEWS[n], size=384) for n in vnames]]
    content = [{"type": "image_url", "image_url": {"url": u}} for u in ims]
    limit = {"obj": f"at most {BUDGET} triangles", "soup": f"at most {BUDGET} triangles", "vox": "any number of cells"}[rep]
    content.append({"type": "text", "text": f"{cb.IMG_NOTE}Images given, in order: {', '.join(vnames)}.\n\n{SPEC[rep]}\n\nModel the object in the images as closely as you can: {limit}. Reply with only the model."})
    return dict(task="create", rep=rep, name=name, kind="+".join(vnames), content=content, truth=orig,
                start=None, orig=orig, ceil=M.to_world(rep, state, Q[rep]))


def run(job):
    import time
    t = time.time()
    ans, ptok, ctok = cb.call(job["content"])
    text = cb.strip_fence(ans)
    pred = None
    if not ans.startswith("ERR"):
        try:
            pred = DEC[job["rep"]](text, Q[job["rep"]])
        except Exception:
            pred = None
    s = score.evaluate(pred, job["truth"])
    out = dict(task=job["task"], rep=job["rep"], name=job["name"], kind=job["kind"], ptok=ptok, ctok=ctok, secs=round(time.time() - t), ans=ans, **s)
    if job["task"] == "edit":
        out["jacc"] = jaccard(pred, job["truth"]) if pred is not None else 0.0
        out["exact"] = out["jacc"] == 1.0
        out["base_jacc"] = jaccard(job["start"], job["truth"])
        out["base_iou"] = score.evaluate(job["start"], job["truth"])["iou"]
    else:
        out["ceil_iou"] = score.evaluate(job["ceil"], job["truth"])["iou"]
    return out


def report(res):
    ed = [x for x in res if x["task"] == "edit"]
    if ed:
        print("\nEDIT  rep   n valid exact  jaccard  iou  (do-nothing jaccard / iou)  prompt  completion")
        for rep in ("obj", "soup", "vox"):
            g = [x for x in ed if x["rep"] == rep]
            if g:
                print(f"{rep:5} {len(g):3} {np.mean([x['valid'] for x in g]):5.2f} {np.mean([x['exact'] for x in g]):5.2f} {np.mean([x['jacc'] for x in g]):7.3f} "
                      f"{np.mean([x['iou'] for x in g]):5.3f}   ({np.mean([x['base_jacc'] for x in g]):.3f} / {np.mean([x['base_iou'] for x in g]):.3f})  "
                      f"{int(np.mean([x['ptok'] for x in g])):6} {int(np.mean([x['ctok'] for x in g])):6}")
        print("\nEDIT exact-match rate by kind")
        for rep in ("obj", "soup", "vox"):
            line = []
            for k in EDITS:
                g = [x for x in ed if x["rep"] == rep and x["kind"] == k]
                if g:
                    line.append(f"{k} {np.mean([x['exact'] for x in g]):.2f}")
            print(f"{rep:5} " + "  ".join(line))
    cr = [x for x in res if x["task"] == "create"]
    if cr:
        print("\nCREATE rep   views             n valid  iou   chamfer  (rep ceiling iou)  prompt completion")
        for rep in ("obj", "soup", "vox"):
            for kind in sorted({x["kind"] for x in cr if x["rep"] == rep}):
                g = [x for x in cr if x["rep"] == rep and x["kind"] == kind]
                v = [x for x in g if x["valid"]]
                print(f"{rep:5} {kind:22} {len(g):2} {len(v) / len(g):4.2f} {np.mean([x['iou'] for x in g]):5.3f} "
                      f"{np.mean([x['chamfer'] for x in v]) if v else float('nan'):6.2f}   ({np.mean([x['ceil_iou'] for x in g]):.3f})  "
                      f"{int(np.mean([x['ptok'] for x in g])):6} {int(np.mean([x['ctok'] for x in g])):6}")


def main():
    names = sys.argv[1].split(",") if len(sys.argv) > 1 and sys.argv[1] != "all" else M.ASSETS
    only = sys.argv[2] if len(sys.argv) > 2 else "edit,create"
    jobs = []
    for n in names:
        orig = M.load(n)
        for rep in ("obj", "soup", "vox"):
            if "edit" in only:
                for k in EDITS:
                    jobs.append(edit_job(n, rep, k, orig))
            if "create" in only:
                jobs.append(create_job(n, rep, ["front", "side", "top", "iso"], orig))
        if "create" in only:
            jobs.append(create_job(n, "obj", ["iso"], orig))
    import os
    res = [json.loads(l) for l in open(cb.OUT)] if os.path.exists(cb.OUT) else []
    done = {(x["task"], x["rep"], x["name"], x["kind"]) for x in res if not x["ans"].startswith("ERR")}
    res = [x for x in res if (x["task"], x["rep"], x["name"], x["kind"]) in done]
    jobs = [j for j in jobs if (j["task"], j["rep"], j["name"], j["kind"]) not in done]
    out = open(cb.OUT, "w")
    for x in res:
        out.write(json.dumps(x) + "\n")
    with ThreadPoolExecutor(8) as ex:
        for x in ex.map(run, jobs):
            res.append(x)
            out.write(json.dumps(x, default=float) + "\n")
            out.flush()
    report(res)


if __name__ == "__main__":
    if sys.argv[1] == "report":
        cb.OUT = sys.argv[2]
        report([json.loads(l) for l in open(sys.argv[2])])
    else:
        main()
