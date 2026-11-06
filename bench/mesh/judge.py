import base64
import json
import os
import sys
import urllib.request

from PIL import Image

CFG = json.load(open(os.path.expanduser("~/.pi/agent/models.json")))["providers"]["yalr"]
from briefs import BRIEFS

BRIEF = os.environ.get("BRIEF", "fox")
QUESTION = """Two 3D models were made from the same brief: "%s". Each image shows four views of one model. Ignore background colour and lighting style.
Score each model 1-10 on: anatomy (proportions, does it read as the subject), brief (each feature present and right), detail (finish of face, limbs, extremities), appeal (as a game asset). Then say which is better overall.
Reply as JSON: {"A": {"anatomy":n,"brief":n,"detail":n,"appeal":n}, "B": {...}, "better": "A" or "B", "why": "one sentence"}""" % BRIEFS[BRIEF]["describe"]

RATE = """A 3D model was made from this brief: "%s". The image shows four views of the model. Ignore background colour and lighting style.
Score it 1-10 on: anatomy (proportions, does it read as the subject), brief (each feature present and right), detail (finish of face, limbs, extremities), appeal (as a game asset). Be strict: 10 is a professional asset, 5 is a recognisable but crude blockout.
Reply as JSON: {"anatomy":n,"brief":n,"detail":n,"appeal":n}""" % BRIEFS[BRIEF]["describe"]


def b64(path):
    im = Image.open(path).convert("RGB").resize((900, 900))
    out = path + ".judge.png"
    im.save(out)
    return "data:image/png;base64," + base64.b64encode(open(out, "rb").read()).decode()


def post(content, max_tokens=3000):
    body = {"model": "code-x-high", "messages": [{"role": "user", "content": content}], "max_tokens": max_tokens}
    req = urllib.request.Request(
        CFG["baseUrl"].rstrip("/") + "/chat/completions",
        json.dumps(body).encode(),
        {"Content-Type": "application/json", "Authorization": "Bearer " + CFG["apiKey"]},
    )
    err = None
    for attempt in range(3):
        try:
            text = json.load(urllib.request.urlopen(req, timeout=600))["choices"][0]["message"]["content"]
            return json.loads(text[text.index("{"): text.rindex("}") + 1])
        except Exception as e:
            err = e
    raise err


def rate(path):
    j = post([{"type": "image_url", "image_url": {"url": b64(path)}}, {"type": "text", "text": RATE}])
    return {k: j[k] for k in ("anatomy", "brief", "detail", "appeal")}


def ask(a, b):
    return post([
        {"type": "text", "text": "Model A:"},
        {"type": "image_url", "image_url": {"url": b64(a)}},
        {"type": "text", "text": "Model B:"},
        {"type": "image_url", "image_url": {"url": b64(b)}},
        {"type": "text", "text": QUESTION},
    ])


def main():
    gcad, blender = sys.argv[1], sys.argv[2]
    rounds = []
    for first, second, names in ((gcad, blender, ("gcad", "blender")), (blender, gcad, ("blender", "gcad"))):
        j = ask(first, second)
        rounds.append({names[0]: j["A"], names[1]: j["B"], "better": names[0] if j["better"] == "A" else names[1], "why": j["why"]})
    print(json.dumps(rounds))


if __name__ == "__main__":
    main()
