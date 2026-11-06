import json
import os
import sys
import time
from concurrent.futures import ThreadPoolExecutor

import numpy as np
from PIL import ImageDraw
from scipy.spatial import cKDTree

import charbench as cb
import layered as L
import score

ASSETS = {
    "spot": "a cow standing on four legs, facing +y (head at large y), z up, its left side is -x",
    "Duck": "a rubber duck facing +x (beak at +x), z up",
    "Fox": "a fox standing on four legs, facing -y (nose at small y), tail towards +y, z up, its left side is +x",
    "CesiumMan": "a humanoid figure in a T pose facing -y, z up, its left side is +x",
    "Avocado": "half an avocado lying cut face towards -y, stone showing in the middle, z up",
}

EDITS = {
    "spot": [
        ("Make the head 25% bigger.", [("select", [24, 40, 36], 10, 6), ("scale", 1.25)]),
        ("Push a hump out of the middle of the back, about 2 units high.", [("select", [24, 16, 26], 7, 3.5), ("inflate", 2)]),
        ("Pull the snout 3 units further forward.", [("select", [24, 44, 30], 6, 3), ("move", [0, 3, 0])]),
        ("Turn the head 30 degrees to the cow's left, about a vertical axis through the neck.", [("select", [24, 40, 36], 10, 7), ("rotate", "z", 30, [24, 34, 34])]),
        ("Make the right horn 3 units taller.", [("select", [27.4, 35.9, 43.5], 3, 1.5), ("move", [0, 0, 3])]),
        ("Lengthen the tail by pulling its tip 3 units further back.", [("select", [24, 3, 19], 4, 2), ("move", [0, -3, 0])]),
    ],
    "Duck": [
        ("Push the tip of the beak 3 units further out.", [("select", [44, 26, 33], 5, 2.5), ("move", [3, 0, 0])]),
        ("Make the head 20% bigger.", [("select", [28, 26, 33], 11, 7), ("scale", 1.2)]),
        ("Lift the tail up by 4 units.", [("select", [4, 25, 14], 7, 3.5), ("move", [0, 0, 4])]),
        ("Puff the chest out by 2 units.", [("select", [38, 24, 14], 8, 4), ("inflate", 2)]),
        ("Flatten the top of the head by pressing it down 3 units.", [("select", [27, 24, 43], 6, 3), ("move", [0, 0, -3])]),
        ("Tilt the head 20 degrees forward, about the y axis through the neck.", [("select", [28, 26, 33], 11, 7), ("rotate", "y", 20, [28, 26, 24])]),
    ],
    "Fox": [
        ("Raise the end of the tail by 5 units.", [("select", [24, 42, 9], 6, 3), ("move", [0, 0, 5])]),
        ("Make the ears 60% bigger.", [("select", [24, 7, 22], 3, 1.5), ("scale", 1.6)]),
        ("Make the snout 2 units longer.", [("select", [24, 3, 17], 4, 2), ("move", [0, -2, 0])]),
        ("Make the body fatter by about 1.5 units.", [("select", [24, 22, 16], 8, 4), ("inflate", 1.5)]),
        ("Lift the head 3 units.", [("select", [24, 6, 19], 6, 3), ("move", [0, 0, 3])]),
        ("Swing the tail 30 degrees towards the fox's left side, about a vertical axis at the base of the tail.", [("select", [24, 40, 11], 9, 6), ("rotate", "z", -30, [24, 33, 15])]),
    ],
    "CesiumMan": [
        ("Lower the left arm by rotating it 40 degrees down at the shoulder.", [("select", [35, 21, 27], 8, 6), ("rotate", "y", 40, [28.5, 22, 31])]),
        ("Make the head 20% bigger.", [("select", [24, 24, 41], 7, 5), ("scale", 1.2)]),
        ("Puff the chest out by 1.5 units.", [("select", [24, 21, 30], 5, 2.5), ("inflate", 1.5)]),
        ("Move the right foot 3 units forward.", [("select", [21, 22, 3], 4, 2), ("move", [0, -3, 0])]),
        ("Raise the right arm by rotating it 30 degrees up at the shoulder.", [("select", [13, 21, 27], 8, 6), ("rotate", "y", 30, [19.5, 22, 31])]),
        ("Make the belly stick out by 1.5 units.", [("select", [24, 20, 21], 4, 2), ("inflate", 1.5)]),
    ],
    "Avocado": [
        ("Push the stone 2 units further out of the cut face.", [("select", [24, 15, 18], 6, 4), ("move", [0, -2, 0])]),
        ("Make the narrow top end 2 units longer.", [("select", [24, 24, 43], 5, 2.5), ("move", [0, 0, 2])]),
        ("Make the stone 30% bigger.", [("select", [24, 15, 18], 6, 4), ("scale", 1.3)]),
        ("Pinch the bottom of the avocado in by 2 units.", [("select", [24, 24, 6], 8, 4), ("inflate", -2)]),
    ],
}

VIEW_NOTE = ("The images are orthographic renders with a world grid: red vertical lines and red labels are the horizontal axis, blue horizontal lines and blue labels the vertical axis, every 4 units, labelled every 8. "
             "Views in order: front (camera at -y looking +y: x to the right, z up), side (camera at +x: y to the right, z up), top (camera above: x to the right, y up), iso (from -y +x above).")
AXES = {"front": ("x", "z"), "side": ("y", "z"), "top": ("x", "y")}


def grid_view(v, f, name):
    im = score.render_image(v, f, *score.VIEWS[name], size=448)
    if name not in AXES:
        return im
    dr = ImageDraw.Draw(im)
    s = 448 / 56
    cv = {"x": 24, "y": 24, "z": 20}
    h, vv = AXES[name]
    for k in range(0, 49, 4):
        p = 224 + (k - cv[h]) * s
        q = 224 - (k - cv[vv]) * s
        dr.line([(p, 0), (p, 447)], fill=(255, 170, 170) if k % 8 else (230, 0, 0))
        dr.line([(0, q), (447, q)], fill=(170, 170, 255) if k % 8 else (0, 0, 230))
        if k % 8 == 0:
            dr.text((p + 2, 2), str(k), fill=(230, 0, 0))
            dr.text((2, q - 11), str(k), fill=(0, 0, 230))
    return im


def asset(name, cache={}):
    if name not in cache:
        hv, hf = L.detailed(name)
        cv, cf = L.lod(hv, hf, 300)
        cv = np.round(cv, 1)
        cage = L.Cage(hv, cv, cf)
        view = L.lod(hv, hf, 20000)
        imgs = [cb.b64(grid_view(*view, n)) for n in ("front", "side", "top", "iso")]
        cache[name] = dict(hv=hv, hf=hf, cv=cv, cf=cf, cage=cage, imgs=imgs, rest=cage.deform(cv))
    return cache[name]


def region_of(ops):
    sel = [o for o in ops if o[0] == "select"][0]
    return np.asarray(sel[1], float), float(sel[2])


def task_text(name, i):
    text, ops = EDITS[name][i]
    c, r = region_of(ops)
    return text, c, r


def prompt(cond, name, i, A, patch, hint=True):
    text, c, r = task_text(name, i)
    where = f"The edit should only affect the region within {L.fmt1(r)} units of the point ({', '.join(L.fmt1(x) for x in c)}).\n\n" if hint else "Find the part to edit from the images. Change nothing else.\n\n"
    head = (f"You are editing a detailed {len(A['hf'])}-triangle 3D mesh: {ASSETS[name]}. World units, the model fits in a 48 unit box.\n{VIEW_NOTE}\n\n"
            f"Edit request: {text}\n{where}")
    if cond == "patch":
        return head + (f"You get only that region, as a {len(patch['f'])}-triangle patch in OBJ form (1-based indices). The first {patch['nb']} vertices are its boundary: they join the rest of the mesh "
                       "and must stay exactly where they are, in the same order. You may move, add or remove any other vertices and faces. Write the complete edited patch as OBJ `v`/`f` lines, boundary vertices first, nothing else.\n\n"
                       + L.patch_text(patch))
    if cond == "cage":
        return head + ("The mesh is bound to a coarse control cage (below, OBJ form, 1-based vertex numbers). Moving cage vertices deforms the detailed mesh smoothly and keeps its surface detail. "
                       "Reply with one line per cage vertex you move, as `N x y z` (the vertex number and its new position), nothing else. Leave the others out.\n\n"
                       + L.cage_text(A["cv"], A["cf"]))
    return head + ("Edit it with operations, one per line, applied in order by the modelling kernel to the detailed mesh:\n" + L.OP_HELP + "\nReply with only the operation lines.")


def apply(cond, A, ans, patch):
    text = cb.strip_fence(ans)
    if cond == "ops":
        ops = L.parse_ops(text)
        return (L.apply_ops(A["hv"], A["hf"], ops), A["hf"]) if ops else None, {}
    if cond == "cage":
        new = L.parse_cage_diff(text, A["cv"])
        if new is None:
            return None, {}
        return (A["hv"] + A["cage"].deform(new) - A["rest"], A["hf"]), dict(moved=int((np.abs(new - A["cv"]).max(1) > 1e-6).sum()))
    p, moved = L.parse_patch(text, patch)
    if p is None:
        return None, {}
    return L.stitch(A["hv"], A["hf"], patch, p), dict(boundary_moved=moved)


def oracle(cond, A, ops, patch):
    if cond == "ops":
        return L.apply_ops(A["hv"], A["hf"], ops), A["hf"]
    if cond == "cage":
        return A["hv"] + A["cage"].deform(L.apply_ops(A["cv"], A["cf"], ops)) - A["rest"], A["hf"]
    pv = L.apply_ops(patch["v"], patch["f"], ops)
    pv[: patch["nb"]] = patch["v"][: patch["nb"]]
    return L.stitch(A["hv"], A["hf"], patch, (pv, patch["f"]))


def area_sample(v, f, n, rng, ball):
    c, r = ball
    cent = v[f].mean(1)
    keep = np.linalg.norm(cent - c, axis=1) < r
    if keep.sum() == 0:
        return np.zeros((0, 3)), np.zeros(0)
    return score.sample_surface(v, f[keep], n, rng), keep


def roughness(v, f, c, r):
    cent = v[f].mean(1)
    keep = np.linalg.norm(cent - c, axis=1) < r
    if keep.sum() < 10:
        return 0.0
    fn = np.cross(v[f[:, 1]] - v[f[:, 0]], v[f[:, 2]] - v[f[:, 0]])
    area = np.linalg.norm(fn, axis=1)
    nn = fn / np.maximum(area, 1e-12)[:, None]
    tree = cKDTree(cent)
    idx = np.where(keep)[0]
    out = []
    for i, nb in zip(idx, tree.query_ball_point(cent[idx], 1.0)):
        m = (nn[nb] * area[nb, None]).sum(0)
        m /= max(np.linalg.norm(m), 1e-12)
        out.append(np.degrees(np.arccos(np.clip(nn[i] @ m, -1, 1))))
    return float(np.mean(out))


def measure(pred, truth, start, ops, rng_seed=0):
    c, r = region_of(ops)
    shift = max([np.linalg.norm(o[1]) for o in ops if o[0] == "move"] + [0])
    ball = (c, r * 1.6 + shift)
    rng = np.random.default_rng(rng_seed)

    def pts(m):
        v, f = m
        used = np.unique(f)
        vv = v[used]
        vv = vv[np.linalg.norm(vv - ball[0], axis=1) < ball[1]]
        s, _ = area_sample(v, f, 3000, rng, ball)
        return vv, np.vstack([vv, s]) if len(s) else vv

    def ch(a, b):
        av, aa = pts(a)
        bv, bb = pts(b)
        if len(av) == 0 or len(bv) == 0:
            return 99.0
        return float((cKDTree(bb).query(av)[0].mean() + cKDTree(aa).query(bv)[0].mean()) / 2)

    e_pred, e_start = ch(pred, truth), ch(start, truth)
    gain = 1 - e_pred / e_start if e_start > 1e-9 else 0.0
    rt = roughness(*truth, c, r)
    rp = roughness(*pred, c, r)
    far = np.linalg.norm(truth[0][: len(start[0])] - c, axis=1) > r * 1.6 + shift
    outside = float(np.abs(pred[0][: len(start[0])][far] - start[0][far]).max()) if len(pred[0]) >= len(start[0]) else 0.0
    return dict(err=e_pred, start_err=e_start, gain=gain, detail=rp / rt if rt else 0.0, outside=outside)


VARIANTS = {
    "patch": dict(cond="patch", hint=True, check=False),
    "cage": dict(cond="cage", hint=True, check=False),
    "ops": dict(cond="ops", hint=True, check=False),
    "ops+check": dict(cond="ops", hint=True, check=True),
    "ops-nohint": dict(cond="ops", hint=False, check=False),
    "ops-nohint+check": dict(cond="ops", hint=False, check=True),
}


def jobs_for(names, variants):
    out = []
    for n in names:
        A = asset(n)
        for i, (text, ops) in enumerate(EDITS[n]):
            c, r = region_of(ops)
            patch = L.patch_for(A["hv"], A["hf"], c, r) if any(VARIANTS[v]["cond"] == "patch" for v in variants) else None
            for var in variants:
                out.append(dict(name=n, i=i, var=var, patch=patch))
    return out


def check_round(A, content, ans, pred):
    view = L.lod(pred[0], pred[1], 20000)
    imgs = [cb.b64(grid_view(*view, n)) for n in ("front", "side", "top", "iso")]
    follow = [{"type": "image_url", "image_url": {"url": u}} for u in imgs]
    follow.append({"type": "text", "text": "These are the same four views after the kernel applied your operations. Compare them with the originals and the request. "
                   "If the edit is right, reply with your operations unchanged. If not, reply with the full corrected list of operations, which will be applied to the original mesh instead. Reply with only the operation lines."})
    return [{"role": "user", "content": content}, {"role": "assistant", "content": ans}, {"role": "user", "content": follow}]


def run(job):
    A = asset(job["name"])
    v = VARIANTS[job["var"]]
    cond = v["cond"]
    text, ops = EDITS[job["name"]][job["i"]]
    content = [{"type": "image_url", "image_url": {"url": u}} for u in A["imgs"]]
    content.append({"type": "text", "text": prompt(cond, job["name"], job["i"], A, job["patch"], v["hint"])})
    t = time.time()
    ans, ptok, ctok = cb.call(content)
    pred, extra = (None, {}) if ans.startswith("ERR") else apply(cond, A, ans, job["patch"])
    start = (A["hv"], A["hf"])
    truth = (L.apply_ops(A["hv"], A["hf"], ops), A["hf"])
    first = measure(pred, truth, start, ops)["gain"] if pred is not None else 0.0
    if v["check"] and pred is not None:
        ans2, p2, c2 = cb.call_messages(check_round(A, content, ans, pred))
        ptok += p2
        ctok += c2
        if not ans2.startswith("ERR"):
            pred2, _ = apply(cond, A, ans2, job["patch"])
            if pred2 is not None:
                pred, ans = pred2, ans + "\n----\n" + ans2
    res = dict(name=job["name"], i=job["i"], var=job["var"], cond=cond, ptok=ptok, ctok=ctok, secs=round(time.time() - t), ans=ans, first_gain=first)
    res.update(extra)
    res["valid"] = pred is not None
    res.update(measure(pred, truth, start, ops) if pred is not None else dict(gain=0.0, detail=0.0, outside=0.0, err=None))
    res["oracle"] = measure(oracle(cond, A, ops, job["patch"]), truth, start, ops)
    return res


def report(res):
    for x in res:
        og = x["oracle"]["gain"]
        x["norm"] = max(-1.0, min(1.5, x["gain"] / og)) if og > 0.05 else 0.0
    print("norm = gain / that condition's own perfect-answer gain, the share of the reachable improvement")
    print("variant            n valid  gain  (oracle gain)  norm  detail  (oracle)  outside_max  prompt  completion")
    for cond in VARIANTS:
        g = [x for x in res if x.get("var", x["cond"]) == cond]
        if not g:
            continue
        print(f"{cond:17} {len(g):2} {np.mean([x['valid'] for x in g]):5.2f} {np.mean([x['gain'] for x in g]):5.2f}  ({np.mean([x['oracle']['gain'] for x in g]):5.2f})  {np.mean([x['norm'] for x in g]):5.2f}  "
              f"{np.mean([x['detail'] for x in g if x['valid']] or [0]):5.2f}   ({np.mean([x['oracle']['detail'] for x in g]):4.2f})    "
              f"{np.mean([x['outside'] for x in g]):7.3f}   {int(np.mean([x['ptok'] for x in g])):6} {int(np.mean([x['ctok'] for x in g])):6}")
    vs = [v for v in VARIANTS if any(x.get("var", x["cond"]) == v for x in res)]
    print("\nper edit gain: " + " / ".join(vs))
    for n in EDITS:
        for i in range(len(EDITS[n])):
            row = [next((x for x in res if x["name"] == n and x["i"] == i and x.get("var", x["cond"]) == v), None) for v in vs]
            if any(row):
                print(f"{n:10} {EDITS[n][i][0][:55]:55} " + "  ".join(f"{x['gain']:5.2f}" if x else "  -  " for x in row))


def main():
    names = sys.argv[1].split(",") if len(sys.argv) > 1 and sys.argv[1] != "all" else list(EDITS)
    variants = sys.argv[2].split(",") if len(sys.argv) > 2 else list(VARIANTS)
    jobs = jobs_for(names, variants)
    res = [json.loads(l) for l in open(cb.OUT)] if os.path.exists(cb.OUT) else []
    done = {(x["name"], x["i"], x["var"]) for x in res if not x["ans"].startswith("ERR")}
    res = [x for x in res if (x["name"], x["i"], x["var"]) in done]
    jobs = [j for j in jobs if (j["name"], j["i"], j["var"]) not in done]
    out = open(cb.OUT, "w")
    for x in res:
        out.write(json.dumps(x) + "\n")
    with ThreadPoolExecutor(6) as ex:
        for x in ex.map(run, jobs):
            res.append(x)
            out.write(json.dumps(x, default=float) + "\n")
            out.flush()
    report(res)


if __name__ == "__main__":
    if sys.argv[1] == "report":
        report([json.loads(l) for l in open(sys.argv[2])])
    else:
        main()
