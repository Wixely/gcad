import math

import numpy as np
from PIL import Image

VIEWS = {"front": (0, 0), "side": (90, 0), "top": (0, 90), "iso": (35, 30), "back_iso": (215, 30)}


def basis(az, el):
    a, e = math.radians(az), math.radians(el)
    right = np.array([math.cos(a), math.sin(a), 0.0])
    toward = np.array([math.sin(a) * math.cos(e), -math.cos(a) * math.cos(e), math.sin(e)])
    up = np.cross(toward, right)
    return right, up, toward


def project(v, az, el, size, span=56.0, centre=(24.0, 24.0, 20.0)):
    right, up, fwd = basis(az, el)
    p = np.asarray(v, float) - np.asarray(centre)
    s = size / span
    return np.stack([size / 2 + (p @ right) * s, size / 2 - (p @ up) * s, -(p @ fwd)], axis=1)


def raster(v, f, az, el, size, shade=False):
    pts = project(v, az, el, size)
    depth = np.full((size, size), np.inf)
    tone = np.zeros((size, size))
    right, up, fwd = basis(az, el)
    light = 0.7 * fwd + 0.5 * up - 0.35 * right
    light = light / np.linalg.norm(light)
    for tri in f:
        a, b, c = pts[tri[0]], pts[tri[1]], pts[tri[2]]
        x0, x1 = int(max(0, math.floor(min(a[0], b[0], c[0])))), int(min(size - 1, math.ceil(max(a[0], b[0], c[0]))))
        y0, y1 = int(max(0, math.floor(min(a[1], b[1], c[1])))), int(min(size - 1, math.ceil(max(a[1], b[1], c[1]))))
        if x1 < x0 or y1 < y0:
            continue
        den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(den) < 1e-9:
            continue
        xs, ys = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        w0 = ((b[1] - c[1]) * (xs - c[0]) + (c[0] - b[0]) * (ys - c[1])) / den
        w1 = ((c[1] - a[1]) * (xs - c[0]) + (a[0] - c[0]) * (ys - c[1])) / den
        w2 = 1 - w0 - w1
        m = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
        if not m.any():
            continue
        z = w0 * a[2] + w1 * b[2] + w2 * c[2]
        sub = depth[y0:y1 + 1, x0:x1 + 1]
        upd = m & (z < sub)
        if not upd.any():
            continue
        sub[upd] = z[upd]
        if shade:
            n = np.cross(v[tri[1]] - v[tri[0]], v[tri[2]] - v[tri[0]])
            ln = np.linalg.norm(n)
            n = n / ln if ln > 0 else n
            if n @ fwd < 0:
                n = -n
            tone[y0:y1 + 1, x0:x1 + 1][upd] = max(0.0, n @ light) * 0.75 + 0.25
    return depth, tone


def silhouette(v, f, az, el, size=96):
    d, _ = raster(v, f, az, el, size)
    return np.isfinite(d)


def render_image(v, f, az, el, size=384):
    d, t = raster(v, f, az, el, size, shade=True)
    g = np.where(np.isfinite(d), 40 + 200 * t, 255).astype(np.uint8)
    return Image.fromarray(g, "L").convert("RGB")


def sample_surface(v, f, n, rng):
    tri = v[f]
    area = np.linalg.norm(np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]), axis=1) / 2
    if area.sum() <= 0:
        return v[: min(len(v), n)]
    idx = rng.choice(len(f), n, p=area / area.sum())
    r1, r2 = rng.random(n), rng.random(n)
    s = np.sqrt(r1)
    a, b, c = tri[idx, 0], tri[idx, 1], tri[idx, 2]
    return (1 - s)[:, None] * a + (s * (1 - r2))[:, None] * b + (s * r2)[:, None] * c


def chamfer(p, q):
    d = np.linalg.norm(p[:, None, :] - q[None, :, :], axis=2)
    return float((d.min(axis=1).mean() + d.min(axis=0).mean()) / 2)


def evaluate(pred, truth, views=("front", "side", "top", "iso"), seed=0):
    if pred is None:
        return dict(valid=False, iou=0.0, chamfer=float("inf"))
    pv, pf = pred
    tv, tf = truth
    ious = []
    for name in views:
        az, el = VIEWS[name]
        a, b = silhouette(pv, pf, az, el), silhouette(tv, tf, az, el)
        u = (a | b).sum()
        ious.append((a & b).sum() / u if u else 1.0)
    rng = np.random.default_rng(seed)
    return dict(valid=True, iou=float(np.mean(ious)), chamfer=chamfer(sample_surface(pv, pf, 600, rng), sample_surface(tv, tf, 600, rng)))
