import math
import random
import re

import numpy as np

SIZE = 48.0
KINDS = {"ellipsoid": (3, 3), "capsule": (6, 1), "cylinder": (6, 2), "box": (3, 3)}


def fmt(x):
    s = f"{x:.1f}"
    return s[:-2] if s.endswith(".0") else s


def rot_to_z(d):
    d = np.asarray(d, float)
    n = np.linalg.norm(d)
    if n < 1e-9:
        return np.eye(3)
    d = d / n
    z = np.array([0, 0, 1.0])
    v = np.cross(z, d)
    c = float(z @ d)
    if np.linalg.norm(v) < 1e-9:
        return np.eye(3) if c > 0 else np.diag([1, -1, -1.0])
    k = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + k + k @ k * (1 / (1 + c))


def revolve(profile, u):
    verts = [(0.0, 0.0, profile[0][1])]
    for rho, z in profile[1:-1]:
        for j in range(u):
            a = 2 * math.pi * j / u
            verts.append((rho * math.cos(a), rho * math.sin(a), z))
    verts.append((0.0, 0.0, profile[-1][1]))
    rings = len(profile) - 2
    faces = []
    for j in range(u):
        faces.append((0, 1 + j, 1 + (j + 1) % u))
    for i in range(rings - 1):
        for j in range(u):
            a, b = 1 + i * u + j, 1 + i * u + (j + 1) % u
            c, d = a + u, b + u
            faces.append((a, c, d))
            faces.append((a, d, b))
    last = 1 + (rings - 1) * u
    top = len(verts) - 1
    for j in range(u):
        faces.append((last + j, top, last + (j + 1) % u))
    return np.array(verts), np.array(faces)


def sphere_profile(rings):
    return [(math.sin(math.pi * i / (rings + 1)), -math.cos(math.pi * i / (rings + 1))) for i in range(rings + 2)]


class Prim:
    def __init__(self, kind, name, pts, rad):
        self.kind = kind
        self.name = name
        self.pts = np.array(pts, float).reshape(-1, 3)
        self.rad = np.array(rad, float).reshape(-1)

    def copy(self):
        return Prim(self.kind, self.name, self.pts.copy(), self.rad.copy())

    def centre(self):
        return self.pts.mean(axis=0)

    def dsl(self):
        nums = list(self.pts.reshape(-1)) + list(self.rad)
        return f"{self.kind} {self.name} " + " ".join(fmt(x) for x in nums)

    @staticmethod
    def parse(line):
        t = line.split()
        if len(t) < 2 or t[0] not in KINDS:
            return None
        npts, nrad = KINDS[t[0]]
        try:
            nums = [float(x) for x in t[2:]]
        except ValueError:
            return None
        if len(nums) != npts + nrad:
            return None
        return Prim(t[0], t[1], nums[:npts], nums[npts:])

    def scaled(self, f):
        c = self.centre()
        return Prim(self.kind, self.name, c + (self.pts - c) * f, self.rad * f)

    def scaled_about(self, origin, f):
        o = np.asarray(origin, float)
        return Prim(self.kind, self.name, o + (self.pts - o) * f, self.rad * f)

    def moved(self, d):
        return Prim(self.kind, self.name, self.pts + np.asarray(d, float), self.rad.copy())

    def swung(self, deg, pivot):
        t = math.radians(deg)
        c, s = math.cos(t), math.sin(t)
        p = np.asarray(pivot, float)
        out = []
        for q in self.pts:
            d = q - p
            out.append(p + np.array([d[0] * c + d[2] * s, d[1], -d[0] * s + d[2] * c]))
        return Prim(self.kind, self.name, out, self.rad.copy())

    def mesh(self, res):
        rings = max(2, res // 2)
        if self.kind == "ellipsoid":
            v, f = revolve(sphere_profile(rings), res)
            return v * self.rad + self.pts[0], f
        if self.kind == "box":
            s = self.rad / 2
            v = np.array([[x, y, z] for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)], float) * s + self.pts[0]
            quads = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
            f = np.array([t for q in quads for t in ((q[0], q[1], q[2]), (q[0], q[2], q[3]))])
            return v, f
        a, b = self.pts
        d = b - a
        length = float(np.linalg.norm(d))
        r = self.rad
        if self.kind == "capsule":
            rr = r[0]
            half = max(1, rings // 2)
            prof = [(rr * math.sin(math.pi / 2 * i / half), -rr * math.cos(math.pi / 2 * i / half)) for i in range(half + 1)]
            top = [(rho, length - z) for rho, z in reversed(prof)]
            profile = [(0.0, -rr)] + prof[1:] + top[:-1] + [(0.0, length + rr)]
            profile = [(0.0, profile[0][1])] + [p for p in profile[1:-1]] + [(0.0, profile[-1][1])]
            v, f = revolve(profile, res)
        else:
            profile = [(0.0, 0.0), (r[0], 0.0), (r[1], length), (0.0, length)]
            v, f = revolve(profile, res)
        return v @ rot_to_z(d).T + a, f


def mesh_of(prims, res):
    vs, fs, groups, off = [], [], [], 0
    for p in prims:
        v, f = p.mesh(res)
        vs.append(v)
        fs.append(f + off)
        groups.append((p.name, off, off + len(v), len(f)))
        off += len(v)
    return np.vstack(vs), np.vstack(fs), groups


def to_obj(prims, res, decimals=0):
    lines, off = [], 0
    for p in prims:
        v, f = p.mesh(res)
        lines.append(f"o {p.name}")
        for x, y, z in v:
            lines.append("v " + " ".join(f"{c:.{decimals}f}" if decimals else str(int(round(c))) for c in (x, y, z)))
        for a, b, c in f:
            if len({a, b, c}) == 3:
                lines.append(f"f {a + 1 + off} {b + 1 + off} {c + 1 + off}")
        off += len(v)
    return "\n".join(lines)


def parse_obj(text):
    verts, faces = [], []
    for line in text.splitlines():
        t = line.split()
        if not t:
            continue
        if t[0] == "v" and len(t) >= 4:
            try:
                verts.append([float(x) for x in t[1:4]])
            except ValueError:
                pass
        elif t[0] == "f" and len(t) >= 4:
            try:
                idx = [int(x.split("/")[0]) - 1 for x in t[1:]]
            except ValueError:
                continue
            for i in range(1, len(idx) - 1):
                faces.append((idx[0], idx[i], idx[i + 1]))
    if not verts or not faces:
        return None
    v = np.array(verts)
    f = np.array(faces)
    ok = (f >= 0).all(axis=1) & (f < len(v)).all(axis=1)
    f = f[ok]
    return (v, f) if len(f) else None


def parse_prims(text):
    prims = [Prim.parse(l.strip()) for l in text.splitlines()]
    prims = [p for p in prims if p]
    return prims or None


def make_char(seed):
    r = random.Random(seed)
    u = r.uniform
    cx = 24.0
    foot_h = 3.0
    leg_len = u(10, 14)
    leg_r = u(2.0, 3.0)
    hip_z = foot_h + leg_len
    t_rx, t_ry, t_rz = u(5.5, 8), u(4, 6), u(6, 8.5)
    t_c = hip_z + t_rz * 0.8
    top_t = t_c + t_rz
    h_r = u(4.5, 6.5)
    h_c = top_t + h_r * 0.75
    parts = [
        Prim("ellipsoid", "torso", [cx, cx, t_c], [t_rx, t_ry, t_rz]),
        Prim("ellipsoid", "head", [cx, cx, h_c], [h_r, h_r * u(0.85, 1.05), h_r * u(0.9, 1.1)]),
    ]
    arm_len = u(11, 15)
    arm_r = u(1.6, 2.4)
    sh_z = t_c + t_rz * 0.6
    swing = u(0, 0.5)
    for side, sgn in (("l", 1), ("r", -1)):
        sx = cx + sgn * (t_rx + 0.5)
        parts.append(Prim("capsule", f"arm_{side}", [sx, cx, sh_z], [0, 0, 0], ) if False else Prim("capsule", f"arm_{side}", [sx, cx, sh_z, sx + sgn * arm_len * swing, cx, sh_z - arm_len * math.sqrt(1 - swing**2)], [arm_r]))
    for side, sgn in (("l", 1), ("r", -1)):
        lx = cx + sgn * u(3, 4.5)
        parts.append(Prim("capsule", f"leg_{side}", [lx, cx, hip_z, lx, cx, foot_h + leg_r], [leg_r]))
        parts.append(Prim("box", f"foot_{side}", [lx, cx + 1.5, foot_h / 2, ], [leg_r * 2, u(6, 9), foot_h]))
    extras = r.sample(["hat", "tail", "ears", "horns", "pack", "nose"], 2)
    top_h = h_c + h_r
    for e in extras:
        if e == "hat":
            parts.append(Prim("cylinder", "hat", [cx, cx, top_h - 1, cx, cx, top_h + u(5, 8)], [h_r * 1.1, 0.5]))
        elif e == "tail":
            parts.append(Prim("capsule", "tail", [cx, cx + t_ry, hip_z + 2, cx, cx + t_ry + u(8, 12), hip_z + u(0, 8)], [u(1.2, 2.0)]))
        elif e == "ears":
            for side, sgn in (("l", 1), ("r", -1)):
                parts.append(Prim("ellipsoid", f"ear_{side}", [cx + sgn * h_r * 0.8, cx, h_c + h_r * 0.7], [1.5, 1.2, u(3, 4.5)]))
        elif e == "horns":
            for side, sgn in (("l", 1), ("r", -1)):
                parts.append(Prim("cylinder", f"horn_{side}", [cx + sgn * h_r * 0.6, cx, top_h - 1.5, cx + sgn * h_r * 0.9, cx, top_h + u(3, 5)], [1.3, 0.2]))
        elif e == "pack":
            parts.append(Prim("box", "pack", [cx, cx + t_ry + 1.5, t_c], [u(7, 10), 4, u(8, 11)]))
        elif e == "nose":
            parts.append(Prim("ellipsoid", "nose", [cx, cx - h_r * 1.0, h_c - 0.5], [1.2, u(2, 3.5), 1.2]))
    allp = np.vstack([np.vstack([p.pts for p in parts])])
    mz = max(Prim.mesh(p, 8)[0][:, 2].max() for p in parts)
    ext = max(max(abs(Prim.mesh(p, 8)[0][:, :2] - cx).max() for p in parts), 1)
    f = min(1.0, 45.0 / mz, 22.0 / ext)
    parts = [p.scaled_about([cx, cx, 0], f) for p in parts]
    return [Prim.parse(p.dsl()) for p in parts]


def random_edit(prims, r, kind=None):
    kind = kind or r.choice(["scale", "move", "swing", "delete"])
    if kind == "swing":
        cands = [p for p in prims if p.kind in ("capsule", "cylinder") and p.name.startswith(("arm", "leg"))]
    elif kind == "delete":
        cands = [p for p in prims if p.name not in ("torso",)]
    else:
        cands = [p for p in prims if p.name != "torso"]
    p = r.choice(cands)
    out = [q.copy() for q in prims]
    i = prims.index(p)
    if kind == "scale":
        f = r.choice([0.5, 1.5, 2.0])
        out[i] = p.scaled(f)
        c = p.centre()
        text = f"Scale the part `{p.name}` uniformly by {fmt(f)} about its own centre (for a capsule or cylinder the centre is the midpoint of its two end points). Every radius, size and distance from the centre is multiplied by {fmt(f)}."
    elif kind == "move":
        d = [r.choice([-6, -4, -2, 0, 2, 4, 6]) for _ in range(3)]
        if not any(d):
            d[2] = 4
        out[i] = p.moved(d)
        text = f"Move the part `{p.name}` by dx={d[0]}, dy={d[1]}, dz={d[2]} without changing its shape."
    elif kind == "swing":
        deg = r.choice([-60, -45, -30, 30, 45, 60])
        pv = p.pts[0]
        out[i] = p.swung(deg, pv)
        text = (f"Rotate the part `{p.name}` by {deg} degrees about the line parallel to the y axis through the pivot point ({fmt(pv[0])}, {fmt(pv[1])}, {fmt(pv[2])}). "
                "It is a right-handed rotation about +y: a point at offset (dx, dy, dz) from the pivot goes to (dx cos t + dz sin t, dy, -dx sin t + dz cos t).")
    else:
        del out[i]
        text = f"Delete the part `{p.name}` completely."
    return kind, text, out
