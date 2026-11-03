import sys

import numpy as np
from PIL import Image, ImageDraw

import score


def load_obj(path):
    v, f = [], []
    for line in open(path):
        t = line.split()
        if not t:
            continue
        if t[0] == "v":
            v.append([float(x) for x in t[1:4]])
        elif t[0] == "f":
            idx = [int(x.split("/")[0]) - 1 for x in t[1:]]
            f.extend([idx[0], idx[i], idx[i + 1]] for i in range(1, len(idx) - 1))
    return np.array(v), np.array(f)


def view(v, f, centre, span, az, el, size):
    old = score.project
    score.project = lambda vv, a, e, s, span_=span, centre_=centre: old(vv, a, e, s, span=span_, centre=centre_)
    im = score.render_image(v, f, az, el, size)
    score.project = old
    return im


def main():
    path, out = sys.argv[1], sys.argv[2]
    shots = [s.split(":") for s in sys.argv[3:]]
    v, f = load_obj(path)
    size = 420
    big = Image.new("RGB", (size * 4, size * len(shots)), "white")
    angles = [("front", 0, 0), ("side", 90, 0), ("iso", 35, 25), ("below", 30, -35)]
    for row, (name, x, y, z, span) in enumerate(shots):
        c = (float(x), float(y), float(z))
        keep = np.linalg.norm(v[f].mean(1) - c, axis=1) < float(span) * 0.9
        for col, (label, az, el) in enumerate(angles):
            im = view(v, f[keep], c, float(span), az, el, size)
            ImageDraw.Draw(im).text((6, 6), f"{name} {label}", fill=(200, 0, 0))
            big.paste(im, (col * size, row * size))
    big.save(out)


if __name__ == "__main__":
    main()
