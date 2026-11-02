import base64, io, json, math, random, re, sys, time, urllib.request
from concurrent.futures import ThreadPoolExecutor
from PIL import Image, ImageDraw

URL = "http://localhost:8001/v1/chat/completions"
MODEL = "qwen3.8-27b"


def make_grid(n, seed):
    r = random.Random(seed)
    return [[r.choice([0, 0, 1, 2, 3, 4]) for _ in range(n)] for _ in range(n)]


class Mesh:
    def __init__(self, g):
        self.g = g
        self.n = len(g)
        self.quads = []
        n = self.n
        for y in range(n):
            for x in range(n):
                h = g[y][x]
                if h == 0:
                    continue
                self.quads.append([(x, y, 0), (x, y + 1, 0), (x + 1, y + 1, 0), (x + 1, y, 0)])
                self.quads.append([(x, y, h), (x + 1, y, h), (x + 1, y + 1, h), (x, y + 1, h)])
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = x + dx, y + dy
                    nh = g[ny][nx] if 0 <= nx < n and 0 <= ny < n else 0
                    if nh >= h:
                        continue
                    if dx == 1:
                        q = [(x + 1, y, nh), (x + 1, y + 1, nh), (x + 1, y + 1, h), (x + 1, y, h)]
                    elif dx == -1:
                        q = [(x, y + 1, nh), (x, y, nh), (x, y, h), (x, y + 1, h)]
                    elif dy == 1:
                        q = [(x + 1, y + 1, nh), (x, y + 1, nh), (x, y + 1, h), (x + 1, y + 1, h)]
                    else:
                        q = [(x, y, nh), (x + 1, y, nh), (x + 1, y, h), (x, y, h)]
                    self.quads.append(q)
        self.tris = [t for q in self.quads for t in ((q[0], q[1], q[2]), (q[0], q[2], q[3]))]

    def indexed(self, faces):
        idx, verts, out = {}, [], []
        for f in faces:
            row = []
            for p in f:
                if p not in idx:
                    idx[p] = len(verts) + 1
                    verts.append(p)
                row.append(idx[p])
            out.append(row)
        return verts, out


def rep_obj(m):
    v, f = m.indexed(m.tris)
    return "\n".join([f"v {x:.6f} {y:.6f} {z:.6f}" for x, y, z in v] + ["f " + " ".join(map(str, r)) for r in f])


def rep_obj_int(m):
    v, f = m.indexed(m.tris)
    return "\n".join([f"v {x} {y} {z}" for x, y, z in v] + ["f " + " ".join(map(str, r)) for r in f])


def rep_obj_quad(m):
    v, f = m.indexed(m.quads)
    return "\n".join([f"v {x} {y} {z}" for x, y, z in v] + ["f " + " ".join(map(str, r)) for r in f])


def rep_soup(m):
    return "\n".join(" ".join(f"{x} {y} {z}" for x, y, z in t) for t in m.tris)


def rep_boxes(m):
    return "\n".join(
        f"box {x} {y} 0 1 1 {h}" for y, row in enumerate(m.g) for x, h in enumerate(row) if h
    )


TEXT = {"obj_float": rep_obj, "obj_int": rep_obj_int, "obj_quad": rep_obj_quad, "soup": rep_soup, "boxes": rep_boxes}
TEXT_NOTE = {
    "obj_float": "Wavefront OBJ triangle mesh, 1-based indices.",
    "obj_int": "Wavefront OBJ triangle mesh with integer coordinates, 1-based indices.",
    "obj_quad": "Wavefront OBJ mesh of quad faces with integer coordinates, 1-based indices.",
    "soup": "A triangle list, one triangle per line as x y z x y z x y z.",
    "boxes": "A union of boxes, one per line as `box x y z width depth height` (min corner then size).",
}


def project(p, az, el, scale, cx, cy):
    x, y, z = p
    a, e = math.radians(az), math.radians(el)
    xr = x * math.cos(a) - y * math.sin(a)
    yr = x * math.sin(a) + y * math.cos(a)
    sx = xr
    sy = z * math.cos(e) - yr * math.sin(e) * -1
    d = yr * math.cos(e) + z * math.sin(e)
    return (cx + sx * scale, cy - (z * math.cos(e) + yr * math.sin(e)) * scale), d


def render_view(m, az, el, size):
    n = m.n
    ctr = (n / 2, n / 2, 2)
    im = Image.new("RGB", (size, size), "white")
    dr = ImageDraw.Draw(im)
    scale = size / (n * 1.7 + 2)
    items = []
    for q in m.quads:
        pts = [(p[0] - ctr[0], p[1] - ctr[1], p[2] - ctr[2]) for p in q]
        u = [pts[1][i] - pts[0][i] for i in range(3)]
        v = [pts[3][i] - pts[0][i] for i in range(3)]
        nrm = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
        L = math.sqrt(sum(c * c for c in nrm)) or 1
        nrm = tuple(c / L for c in nrm)
        proj = [project(p, az, el, scale, size / 2, size / 2) for p in pts]
        depth = sum(d for _, d in proj) / 4
        a, e = math.radians(az), math.radians(el)
        view = (math.sin(a) * math.cos(e) * -1, math.cos(a) * math.cos(e), math.sin(e))
        light = max(0.0, sum(nrm[i] * [0.3, -0.5, 0.8][i] for i in range(3)))
        items.append((depth, [p for p, _ in proj], 90 + int(130 * min(1, light))))
    for _, poly, g in sorted(items, key=lambda t: t[0]):
        dr.polygon(poly, fill=(g, g, g), outline=(30, 30, 30))
    return im


def img_iso(m):
    return [render_view(m, 35, 35, 768)]


def img_views4(m):
    views = [render_view(m, 35, 35, 400), render_view(m, 0, 90, 400), render_view(m, 0, 0, 400), render_view(m, 90, 0, 400)]
    big = Image.new("RGB", (800, 800))
    for i, v in enumerate(views):
        big.paste(v, ((i % 2) * 400, (i // 2) * 400))
    return [big]


def img_multi(m):
    return [render_view(m, a, 35, 400) for a in (35, 125, 215, 305)] + [render_view(m, 0, 90, 400)]


IMAGES = {"img_iso": img_iso, "img_views4": img_views4, "img_multi5": img_multi}


def tasks(m, r):
    g, n = m.g, m.n
    vol = sum(map(sum, g))
    mx = max(map(max, g))
    empty = sum(1 for row in g for h in row if h == 0)
    atmax = sum(1 for row in g for h in row if h == mx)
    x, y, z = r.randrange(n), r.randrange(n), r.randrange(4)
    inside = g[y][x] > z
    return [
        ("volume", "What is the volume of the solid in cubic units? Answer with one integer.", str(vol)),
        ("height", "What is the maximum z coordinate of the solid? Answer with one integer.", str(mx)),
        ("empty", f"The footprint is the square 0..{n} in x and y, divided into unit cells. How many unit cells have no material at all (nothing above the z=0 plane)? Answer with one integer.", str(empty)),
        ("tallest", "How many unit cells of the footprint reach the maximum height? Answer with one integer.", str(atmax)),
        ("point", f"Is the point ({x + 0.5}, {y + 0.5}, {z + 0.5}) inside the solid? Answer yes or no.", "yes" if inside else "no"),
    ]


def call(content):
    body = {"model": MODEL, "max_tokens": 16000, "temperature": 0, "messages": [{"role": "user", "content": content}],
            "chat_template_kwargs": {"enable_thinking": True}}
    q = urllib.request.Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json"})
    for _ in range(3):
        try:
            o = json.load(urllib.request.urlopen(q, timeout=900))
            return o["choices"][0]["message"].get("content") or "", o["usage"]["prompt_tokens"]
        except Exception as e:
            err = str(e)
            time.sleep(2)
    return "ERR " + err, 0


def b64(im):
    b = io.BytesIO()
    im.save(b, "PNG")
    return "data:image/png;base64," + base64.b64encode(b.getvalue()).decode()


def score(kind, ans, truth):
    a = ans.strip().lower()
    if kind == "point":
        m = re.findall(r"\b(yes|no)\b", a)
        return bool(m) and m[-1] == truth
    nums = re.findall(r"-?\d+(?:\.\d+)?", a)
    return bool(nums) and float(nums[-1]) == float(truth)


def run_one(job):
    rep, n, seed, tname, q, truth, m = job
    if rep in TEXT:
        pre = f"{TEXT_NOTE[rep]} It describes one solid.\n\n{TEXT[rep](m)}\n\n"
        content = [{"type": "text", "text": pre + q + "\nReply with only the answer."}]
    else:
        ims = IMAGES[rep](m)
        content = [{"type": "image_url", "image_url": {"url": b64(i)}} for i in ims]
        content.append({"type": "text", "text": f"Rendered views of a solid made of unit cells on a grid of {n}x{n}, x to the right, y into the page, z up. The last image may be the top view.\n" + q + "\nReply with only the answer."})
    ans, toks = call(content)
    return dict(rep=rep, n=n, seed=seed, task=tname, ok=score(tname, ans, truth), tokens=toks, ans=ans[:80], truth=truth)


def main():
    sizes = [int(s) for s in sys.argv[1].split(",")]
    seeds = int(sys.argv[2])
    reps = sys.argv[3].split(",") if len(sys.argv) > 3 else list(TEXT) + list(IMAGES)
    jobs = []
    for n in sizes:
        for seed in range(seeds):
            m = Mesh(make_grid(n, seed * 100 + n))
            r = random.Random(seed)
            for t in tasks(m, r):
                for rep in reps:
                    jobs.append((rep, n, seed, *t, m))
    with ThreadPoolExecutor(8) as ex:
        res = list(ex.map(run_one, jobs))
    json.dump(res, open("results.json", "w"))
    agg = {}
    for x in res:
        a = agg.setdefault((x["rep"], x["n"]), [0, 0, 0])
        a[0] += x["ok"]; a[1] += 1; a[2] += x["tokens"]
    print(f"{'rep':12} {'n':>3} {'acc':>6} {'avg prompt tokens':>18}")
    for (rep, n), (ok, tot, tk) in sorted(agg.items(), key=lambda k: (k[0][1], k[0][0])):
        print(f"{rep:12} {n:>3} {ok / tot:6.2f} {tk // tot:>18}")
    bytask = {}
    for x in res:
        a = bytask.setdefault((x["rep"], x["task"]), [0, 0]); a[0] += x["ok"]; a[1] += 1
    print()
    tn = ["volume", "height", "empty", "tallest", "point"]
    print(f"{'rep':12} " + " ".join(f"{t:>8}" for t in tn))
    for rep in reps:
        print(f"{rep:12} " + " ".join(f"{bytask[(rep, t)][0] / bytask[(rep, t)][1]:8.2f}" for t in tn))


if __name__ == "__main__":
    main()
