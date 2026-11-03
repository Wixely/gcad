import glob
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor

import judge

BASE = os.environ.get("BASE", "/tmp/mb/cost")
BRIEF = sys.argv[1]
TARGET = int(os.environ.get("TARGET", "28"))


def total(r):
    return sum(r.values())


def final_image(d, side):
    return f"{d}/gcad_in_blender.png" if side == "gcad" else f"{d}/blender_final.png"


def score_run(d, side):
    usage = json.load(open(f"{d}/usage_{side}.json"))[side]
    steps = usage["snapshots"]
    images = [s["image"] for s in steps if s.get("image") and os.path.exists(s["image"])]
    with ThreadPoolExecutor(4) as pool:
        scores = list(pool.map(lambda p: total(judge.rate(p)), images))
    reach = None
    for s, sc in zip([s for s in steps if s.get("image") and os.path.exists(s["image"])], scores):
        if sc >= TARGET:
            reach = s
            break
    final = judge.rate(final_image(d, side))
    slim = {k: v for k, v in usage.items() if k not in ("snapshots", "request_log", "tool_log", "http_log")}
    return {"usage": slim, "stalls": [r for r in usage["request_log"] if r["seconds"] > 120], "final": final, "final_total": total(final), "step_scores": scores, "reach": reach}


def main():
    out = {}
    for side in ("gcad", "blender"):
        out[side] = []
        for d in sorted(glob.glob(f"{BASE}/{BRIEF}/*")):
            if os.path.exists(f"{d}/usage_{side}.json"):
                out[side].append(score_run(d, side))
    json.dump(out, open(f"{BASE}/{BRIEF}_scores.json", "w"), indent=1)
    for side, runs in out.items():
        n = len(runs)
        mean = lambda f: sum(f(r) for r in runs) / max(n, 1)
        hit = [r for r in runs if r["reach"]]
        print(side, "n", n, "seconds", round(mean(lambda r: r["usage"]["seconds"])), "calls", round(mean(lambda r: r["usage"]["tool_calls"]), 1), "requests", round(mean(lambda r: r["usage"]["requests"]), 1),
              "uncached", round(mean(lambda r: r["usage"]["uncached"])), "cached", round(mean(lambda r: r["usage"]["cached"])), "output", round(mean(lambda r: r["usage"]["output"])),
              "cost", round(mean(lambda r: r["usage"]["cost"]), 3), "score", round(mean(lambda r: r["final_total"]), 1),
              f"reached{TARGET}", len(hit), "of", n)
        if hit:
            print("  to target: seconds", round(sum(r["reach"]["seconds"] for r in hit) / len(hit)), "calls", round(sum(r["reach"]["calls"] for r in hit) / len(hit), 1),
                  "cost", round(sum(r["reach"]["cost"] for r in hit) / len(hit), 3), "edits", round(sum(r["reach"]["n"] + 1 for r in hit) / len(hit), 1))


main()
