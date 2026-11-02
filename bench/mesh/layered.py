import math
import re

import fast_simplification
import numpy as np
import trimesh
from scipy.spatial import cKDTree

import meshrep as M


def detailed(name, target=100_000):
    v, f = M.load(name)
    m = trimesh.Trimesh(v, f, process=True)
    m.merge_vertices(merge_tex=True, merge_norm=True)
    while len(m.faces) * 4 <= target * 1.5:
        m = m.subdivide_loop(1)
    v = np.asarray(m.vertices, float)
    n = np.asarray(m.vertex_normals, float)
    x, y, z = v.T
    h = 0.18 * (np.sin(1.9 * x + 0.3 * z) * np.sin(2.3 * y) + 0.6 * np.sin(3.7 * z + 1.1 * x) * np.cos(2.9 * y + 0.7 * x))
    return v + n * h[:, None], np.asarray(m.faces, int)


def lod(v, f, faces):
    if len(f) <= faces:
        return v.copy(), f.copy()
    pts, fc = fast_simplification.simplify(v.astype(np.float32), f.astype(np.int32), target_count=faces)
    return np.asarray(pts, float), np.asarray(fc, int)


def vertex_normals(v, f):
    fn = np.cross(v[f[:, 1]] - v[f[:, 0]], v[f[:, 2]] - v[f[:, 0]])
    vn = np.zeros_like(v)
    for i in range(3):
        np.add.at(vn, f[:, i], fn)
    return vn / np.maximum(np.linalg.norm(vn, axis=1, keepdims=True), 1e-12)


def falloff(v, c, inner, outer):
    d = np.linalg.norm(v - np.asarray(c, float), axis=1)
    t = np.clip((outer - d) / max(outer - inner, 1e-9), 0, 1)
    return t * t * (3 - 2 * t)


def rotation(axis, deg):
    t = math.radians(deg)
    c, s = math.cos(t), math.sin(t)
    i = "xyz".index(axis)
    R = np.eye(3)
    a, b = (i + 1) % 3, (i + 2) % 3
    R[a, a], R[a, b], R[b, a], R[b, b] = c, -s, s, c
    return R


def apply_ops(v, f, ops):
    v = v.copy()
    w = np.zeros(len(v))
    centre = v.mean(0)
    for op in ops:
        k = op[0]
        if k == "select":
            _, c, outer, inner = op
            w = falloff(v, c, inner, outer)
            centre = np.asarray(c, float)
        elif k == "move":
            v += w[:, None] * np.asarray(op[1], float)
        elif k == "scale":
            v += w[:, None] * (op[1] - 1) * (v - centre)
        elif k == "rotate":
            _, axis, deg, p = op
            p = np.asarray(p, float)
            v += w[:, None] * (((v - p) @ rotation(axis, deg).T + p) - v)
        elif k == "inflate":
            v += w[:, None] * op[1] * vertex_normals(v, f)
        elif k == "smooth":
            for _ in range(int(op[1])):
                nb = np.zeros_like(v)
                cnt = np.zeros(len(v))
                for i, j in ((0, 1), (1, 2), (2, 0), (1, 0), (2, 1), (0, 2)):
                    np.add.at(nb, f[:, i], v[f[:, j]])
                    np.add.at(cnt, f[:, i], 1)
                avg = nb / np.maximum(cnt, 1)[:, None]
                v += 0.5 * w[:, None] * (avg - v)
    return v


OP_HELP = """select sphere X Y Z R [INNER]   pick the region within R of the point, full strength inside INNER (default R/2), fading smoothly to 0 at R
move DX DY DZ                   move the selection
scale S                         scale the selection about the selection centre
rotate AXIS DEG PX PY PZ        rotate the selection about the line parallel to AXIS (x, y or z) through the pivot; right-handed
inflate H                       push the selection out along its normals by H (negative pulls in)
smooth N                        relax the selection N times
Several select lines may each be followed by their own ops. Ops apply in order."""


def parse_ops(text):
    ops = []
    for line in text.splitlines():
        t = line.strip().split()
        if not t:
            continue
        try:
            if t[0] == "select" and len(t) >= 6 and t[1] == "sphere":
                r = float(t[5])
                ops.append(("select", [float(x) for x in t[2:5]], r, float(t[6]) if len(t) > 6 else r / 2))
            elif t[0] == "move" and len(t) == 4:
                ops.append(("move", [float(x) for x in t[1:4]]))
            elif t[0] == "scale" and len(t) == 2:
                ops.append(("scale", float(t[1])))
            elif t[0] == "rotate" and len(t) == 6 and t[1] in ("x", "y", "z"):
                ops.append(("rotate", t[1], float(t[2]), [float(x) for x in t[3:6]]))
            elif t[0] == "inflate" and len(t) == 2:
                ops.append(("inflate", float(t[1])))
            elif t[0] == "smooth" and len(t) == 2:
                ops.append(("smooth", min(20, int(float(t[1])))))
        except ValueError:
            continue
    return ops or None


class Cage:
    def __init__(self, hv, cv, cf, k=4):
        self.cv = cv
        self.cf = cf
        cent = cv[cf].mean(1)
        _, near = cKDTree(cent).query(hv, k)
        self.near = near
        bary, off = [], []
        for j in range(k):
            tri = cv[cf[near[:, j]]]
            b, o = self._bind(hv, tri)
            bary.append(b)
            off.append(o)
        self.bary = np.stack(bary, 1)
        self.off = np.stack(off, 1)
        d = np.linalg.norm(hv[:, None, :] - cent[near], axis=2)
        w = 1 / np.maximum(d, 1e-3) ** 2
        self.w = w / w.sum(1, keepdims=True)

    @staticmethod
    def frame(tri):
        e1 = tri[:, 1] - tri[:, 0]
        e2 = tri[:, 2] - tri[:, 0]
        n = np.cross(e1, e2)
        n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)
        t = e1 / np.maximum(np.linalg.norm(e1, axis=1, keepdims=True), 1e-12)
        return t, np.cross(n, t), n

    def _bind(self, p, tri):
        t, b, n = self.frame(tri)
        d = p - tri[:, 0]
        e1 = tri[:, 1] - tri[:, 0]
        e2 = tri[:, 2] - tri[:, 0]
        uu = np.stack([np.einsum("ij,ij->i", e1, t), np.einsum("ij,ij->i", e1, b)], 1)
        vv = np.stack([np.einsum("ij,ij->i", e2, t), np.einsum("ij,ij->i", e2, b)], 1)
        dd = np.stack([np.einsum("ij,ij->i", d, t), np.einsum("ij,ij->i", d, b)], 1)
        det = uu[:, 0] * vv[:, 1] - uu[:, 1] * vv[:, 0]
        det = np.where(np.abs(det) < 1e-12, 1e-12, det)
        l1 = (dd[:, 0] * vv[:, 1] - dd[:, 1] * vv[:, 0]) / det
        l2 = (uu[:, 0] * dd[:, 1] - uu[:, 1] * dd[:, 0]) / det
        h = np.einsum("ij,ij->i", d, n)
        return np.stack([1 - l1 - l2, l1, l2], 1), h

    def deform(self, new_cv):
        out = np.zeros((len(self.near), 3))
        for j in range(self.near.shape[1]):
            tri = new_cv[self.cf[self.near[:, j]]]
            _, _, n = self.frame(tri)
            p = np.einsum("ij,ijk->ik", self.bary[:, j], tri) + n * self.off[:, j][:, None]
            out += self.w[:, j][:, None] * p
        return out


def fmt1(x):
    s = f"{x:.1f}"
    return "0" if s in ("0.0", "-0.0") else (s[:-2] if s.endswith(".0") else s)


def cage_text(cv, cf):
    return "\n".join([f"v {fmt1(a)} {fmt1(b)} {fmt1(c)}" for a, b, c in cv] + [f"f {a + 1} {b + 1} {c + 1}" for a, b, c in cf])


def parse_cage_diff(text, cv):
    new = cv.copy()
    n = 0
    for line in text.splitlines():
        t = line.strip().split()
        if len(t) == 4:
            try:
                i = int(t[0]) - 1
                xyz = [float(x) for x in t[1:]]
            except ValueError:
                continue
            if 0 <= i < len(cv):
                new[i] = xyz
                n += 1
    return new if n else None


def region_faces(v, f, c, r):
    return np.linalg.norm(v[f].mean(1) - np.asarray(c, float), axis=1) < r


def patch_for(hv, hf, c, r, budget=500):
    inside = region_faces(hv, hf, c, r).sum()
    target = int(len(hf) * budget / max(inside, 1))
    lv, lf = lod(hv, hf, target)
    lv = np.round(lv, 1)
    sel = region_faces(lv, lf, c, r)
    pf = lf[sel]
    used = np.unique(pf)
    outside_used = np.unique(lf[~sel])
    boundary = np.intersect1d(used, outside_used)
    interior = np.setdiff1d(used, boundary)
    order = np.concatenate([boundary, interior])
    remap = -np.ones(len(lv), int)
    remap[order] = np.arange(len(order))
    return dict(v=lv[order], f=remap[pf], nb=len(boundary), hole=region_faces(hv, hf, c, r), lod_faces=len(lf))


def patch_text(p):
    lines = [f"v {fmt1(a)} {fmt1(b)} {fmt1(c)}" for a, b, c in p["v"]]
    lines += [f"f {a + 1} {b + 1} {c + 1}" for a, b, c in p["f"]]
    return "\n".join(lines)


def parse_patch(text, p):
    from chars import parse_obj
    r = parse_obj(text)
    if r is None:
        return None, 0
    v, f = r
    nb = p["nb"]
    if len(v) < nb:
        return None, 0
    moved = int((np.abs(v[:nb] - p["v"][:nb]).max(1) > 0.051).sum())
    v[:nb] = p["v"][:nb]
    return (v, f), moved


def stitch(hv, hf, p, patch):
    keep = hf[~p["hole"]]
    pv, pf = patch
    return np.vstack([hv, pv]), np.vstack([keep, pf + len(hv)])
