import math
import random

import fast_simplification
import numpy as np
import trimesh
from scipy import ndimage

FIT = 44.0
CENTRE = 24.0
ASSETS = ["spot", "Duck", "Fox", "CesiumMan", "Lantern", "Avocado"]


def load(name):
    path = f"assets/{name}.obj" if name == "spot" else f"assets/{name}.glb"
    m = trimesh.load(path, force="mesh")
    v = np.asarray(m.vertices, float)
    f = np.asarray(m.faces, int)
    v = np.stack([v[:, 0], -v[:, 2], v[:, 1]], axis=1)
    lo, hi = v.min(0), v.max(0)
    s = FIT / (hi - lo).max()
    v = (v - lo) * s
    v[:, 0] += CENTRE - v[:, 0].max() / 2
    v[:, 1] += CENTRE - v[:, 1].max() / 2
    v[:, 2] += 2.0
    return v, f


def weld(v, f):
    u, inv = np.unique(v, axis=0, return_inverse=True)
    inv = inv.reshape(-1)
    g = inv[f]
    ok = (g[:, 0] != g[:, 1]) & (g[:, 1] != g[:, 2]) & (g[:, 0] != g[:, 2])
    g = np.unique(np.sort(g[ok], axis=1), axis=0) if False else g[ok]
    used = np.unique(g)
    remap = -np.ones(len(u), int)
    remap[used] = np.arange(len(used))
    return u[used], remap[g]


def decimate(v, f, budget, q):
    if len(f) > budget:
        pts, fc = fast_simplification.simplify(v.astype(np.float32), f.astype(np.int32), target_count=budget)
        v, f = pts.astype(float), fc.astype(int)
    return weld(np.round(v / q), f)


def voxelise(v, f, q):
    m = trimesh.Trimesh(v / q, f, process=False)
    vg = m.voxelized(1.0)
    pts = np.round(np.asarray(vg.points)).astype(int)
    lo = pts.min(0) - 1
    grid = np.zeros(tuple(pts.max(0) - lo + 2), bool)
    grid[tuple((pts - lo).T)] = True
    grid = ndimage.binary_fill_holes(grid)
    cells = np.argwhere(grid) + lo
    return cells[np.lexsort((cells[:, 2], cells[:, 1], cells[:, 0]))]


def voxel_mesh(cells, q):
    s = set(map(tuple, cells))
    verts, faces, idx = [], [], {}

    def vid(p):
        if p not in idx:
            idx[p] = len(verts)
            verts.append(p)
        return idx[p]

    dirs = [((1, 0, 0), [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)]), ((-1, 0, 0), [(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)]),
            ((0, 1, 0), [(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)]), ((0, -1, 0), [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)]),
            ((0, 0, 1), [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)]), ((0, 0, -1), [(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)])]
    for c in s:
        for d, quad in dirs:
            if (c[0] + d[0], c[1] + d[1], c[2] + d[2]) in s:
                continue
            ids = [vid((c[0] + o[0], c[1] + o[1], c[2] + o[2])) for o in quad]
            faces.append((ids[0], ids[1], ids[2]))
            faces.append((ids[0], ids[2], ids[3]))
    if not faces:
        return None
    return np.array(verts, float) * q, np.array(faces)


def enc_obj(v, f):
    return "\n".join([f"v {int(a)} {int(b)} {int(c)}" for a, b, c in v] + [f"f {a + 1} {b + 1} {c + 1}" for a, b, c in f])


def enc_soup(v, f):
    return "\n".join(" ".join(str(int(x)) for x in v[t].reshape(-1)) for t in f)


def enc_vox(cells):
    cols = {}
    for x, y, z in cells:
        cols.setdefault((x, y), []).append(z)
    lines = []
    for (x, y) in sorted(cols):
        zs = sorted(cols[(x, y)])
        runs, s = [], zs[0]
        for a, b in zip(zs, zs[1:] + [None]):
            if b is None or b != a + 1:
                runs.append(f"{s}-{a}" if a != s else f"{s}")
                s = b
        lines.append(f"{x} {y} " + ",".join(runs))
    return "\n".join(lines)


def dec_obj(text, q):
    from chars import parse_obj
    r = parse_obj(text)
    return None if r is None else (r[0] * q, r[1])


def dec_soup(text, q):
    tris = []
    for line in text.splitlines():
        try:
            nums = [float(x) for x in line.split()]
        except ValueError:
            continue
        if len(nums) == 9:
            tris.append(nums)
    if not tris:
        return None
    a = np.array(tris).reshape(-1, 3) * q
    return a, np.arange(len(a)).reshape(-1, 3)


def dec_vox(text, q):
    cells = []
    for line in text.splitlines():
        t = line.split()
        if len(t) != 3:
            continue
        try:
            x, y = int(t[0]), int(t[1])
            for run in t[2].split(","):
                a, _, b = run.partition("-")
                for z in range(int(a), int(b or a) + 1):
                    cells.append((x, y, z))
        except ValueError:
            continue
    return voxel_mesh(np.array(cells), q) if cells else None


def points_of(rep, state):
    return state[0] if rep != "vox" else state


def edit_spec(rep, state, r, kind):
    pts = state if rep == "vox" else state[0]
    lo, hi = pts.min(0), pts.max(0)
    ext = hi - lo
    cx, cy = int(round((lo[0] + hi[0]) / 2)), int(round((lo[1] + hi[1]) / 2))
    step = max(2, int(round(ext.max() * 0.12)))
    if kind == "shift_top":
        z0 = int(np.percentile(pts[:, 2], 60))
        d = [r.choice([-step, 0, step]) for _ in range(2)] + [r.choice([step, 2 * step])]
        text = f"Move every {'vertex' if rep != 'vox' else 'cell'} with z > {z0} by dx={d[0]}, dy={d[1]}, dz={d[2]}."
        return kind, text, ("shift", z0, d)
    if kind == "stretch_x":
        s = r.choice([2, 0.5])
        text = f"Scale the x coordinate of every {'vertex' if rep != 'vox' else 'cell'} about x={cx}: new x = {cx} + (x - {cx}) * {s}, rounded to the nearest integer (halves round up)."
        return kind, text, ("stretch", cx, s)
    if kind == "flatten":
        y0 = int(np.percentile(pts[:, 1], 25))
        text = f"Every {'vertex' if rep != 'vox' else 'cell'} with y < {y0} gets y = {y0}."
        return kind, text, ("flatten", y0)
    if kind == "rotate":
        text = (f"Rotate the whole thing 90 degrees about the vertical line x={cx}, y={cy}: (x, y, z) becomes ({cx} - (y - {cy}), {cy} + (x - {cx}), z).")
        return kind, text, ("rotate", cx, cy)
    x0 = int(np.percentile(pts[:, 0], 70))
    if rep == "vox":
        text = f"Delete every cell with x > {x0}."
    else:
        text = f"Delete every triangle that has any vertex with x > {x0} (leave the other triangles and their vertices unchanged)."
    return "delete", text, ("delete", x0)


def apply_edit(rep, state, spec):
    pts = (state if rep == "vox" else state[0]).astype(float)
    kind = spec[0]
    keep = None
    if kind == "shift":
        m = pts[:, 2] > spec[1]
        pts[m] += np.array(spec[2])
    elif kind == "stretch":
        pts[:, 0] = np.floor(spec[1] + (pts[:, 0] - spec[1]) * spec[2] + 0.5)
    elif kind == "flatten":
        pts[:, 1] = np.where(pts[:, 1] < spec[1], spec[1], pts[:, 1])
    elif kind == "rotate":
        x, y = pts[:, 0].copy(), pts[:, 1].copy()
        pts[:, 0] = spec[1] - (y - spec[2])
        pts[:, 1] = spec[2] + (x - spec[1])
    elif kind == "delete":
        if rep == "vox":
            return np.unique(pts[pts[:, 0] <= spec[1]].astype(int), axis=0)
        bad = (pts[:, 0] > spec[1])[state[1]].any(axis=1)
        return pts, state[1][~bad]
    if rep == "vox":
        return np.unique(pts.astype(int), axis=0)
    return pts, state[1]


def to_world(rep, state, q):
    if rep == "vox":
        return voxel_mesh(state, q)
    return state[0] * q, state[1]
