# Fox benchmark: gcad vs Blender MCP

Brief: a red fox game character on four legs, pointed dark ears, long narrow muzzle, black nose, almond eyes, white cheeks, chest and tail tip, dark paws, long bushy tail. Same model on both sides, three rounds each, judged by `judge.py` (code-x-high, both orders per round, so 6 verdicts per comparison). Scores are the sum of anatomy, brief, detail and appeal (max 40).

## Results

| Comparison | gcad wins | gcad score | Blender score |
|---|---|---|---|
| Baseline: old skill (r1-r3) | 3 of 6 | 27.0 | 26.5 |
| New skill (s1-s3) | 6 of 6 | 30.8 | 26.0 |
| Same renderer: new skill gcad OBJ shown through Blender's look tool | 0 of 6 | 24.7 | 29.2 |

The same-renderer check was run on s1-s3 `gcad.obj` against r1-r3 `blender_final.png`. Images are `s*/gcad_in_blender_v2.png`. (The first attempt, `gcad_in_blender.png`, is empty: `obj_import` failed on its context, so the v2 script runs it under `temp_override` with the 3D viewport.)

## What changed between baseline and new skill

- `color ... soft=` fades a colour across a part border, so white chest and cheeks blend into the coat instead of reading as stuck-on patches.
- `blob mode=shell`: a separate closed, finer-meshed surface for eyes, pupils, nose and inner ears.
- `color ... gloss=` for wet-looking eyes and nose, written to the `.mtl`.
- Skill rewritten as three passes (blockout, features as shells, sculpt and colour) with fox proportions (long thin legs, slim body, small paws, wedge head), `inspect --pick` coordinates for small features, and a rule to look at the face from front and side every step and keep iterating.
- The fox example mouth is placed from picked surface points.

## Caveats

- Background: the Blender renders use the HDRI world in material preview, and re-whitening it did not change that, so the two sides in the first two comparisons do not share a background and lighting. The judge was told to ignore it, but it may still bias.
- The same-renderer test is harsher for gcad. The OBJ export loses the per-vertex soft colour borders (they come out as hard, stair-stepped patches) and the gloss, so it is not what gcad's own renderer shows. Judges also noted a washed-out coat, a long tube-like body, stubby sock-like legs and faint eyes in that view.
- Under that test Blender wins 6-0, so the 6-0 gcad win above is partly a renderer effect and not a pure model-quality result. A fair claim is that the new skill beats the old one clearly and is on par with or somewhat above Blender MCP in gcad's own renderer.
- Only 3 rounds, one judge model, one brief.

## Speed, calls, tokens and cost (issues 181, 186, 187)

The first version of this section (hand-rolled chat loop, full-price input, a one-line Blender prompt against a 12k-character gcad prompt) is withdrawn. Its verdict "gcad is more expensive" measured the unequal prefix at the wrong price. This is the rerun.

### Harness

`foxbench.py` now runs both sides through `harness.py`, built on pydantic-ai (chosen because it talks to the OpenAI-compatible router, reports `cache_read_tokens` per request, and loads the real `mcp-for-blender` server through `MCPToolset`). Same model (`code-x-high`, which the router maps to claude-opus-5-5), same loop, same 24-request cap, same system-prompt shape (`prompts.py`: one role line, then the tool guidance that side needs, three passes, look between every step), same picture handling (tool images go back in the tool result). Only the tools differ: gcad gets `write_file`, `render`, `inspect` as local functions; Blender gets `execute_blender_code`, `look`, `get_scene_info` from the MCP server. First request size (system prompt, tool schemas and task): gcad 5.8k tokens, Blender 3.3k tokens, medians. The gcad extra is the skill's Meshes section plus the format reference; the Blender prompt carries equivalent guidance but no format reference because Blender Python needs none.

Caching: the router caches the longest repeated prefix by itself (probed directly: with and without `cache_control` markers it reports the same `cached_tokens`), so every request extends the previous one and the cache is read back. `test_harness.py` checks that property. Every request logs input, cached, uncached, output and reasoning tokens (the router reports no reasoning split, so it is 0), model seconds from the harness, HTTP attempts and status, and every tool call with its time.

Prices: Anthropic list price for Claude Opus 5.5 from platform.claude.com/docs/en/about-claude/pricing: $4 input, $0.20 cache read, $20 output per million tokens. Cache writes are billed at the input rate here (the router reports no write count), which slightly understates both sides. The router's own price list for `code-x-high` is a flat 1000 sats per million and is not used.

Quality: the same Blender-renderer judge as before (single image, 1-10 on four axes, sum of 40), gcad exported with vertex colours and imported into Blender, so both sides are rendered by Blender's `look`. Raw numbers: `fox-cost-scores.json`, `penguin-cost-scores.json`.

### Results

Medians over 5 rounds per cell. Seconds exclude per-edit snapshot time.

| Brief | Side | Seconds | Tool calls | Requests | Uncached input | Cached input | Output | Cost | Judge /40 |
|---|---|---|---|---|---|---|---|---|---|
| Fox | gcad | 205 | 26 | 15 | 60k | 408k | 22k | $0.81 | 28 (all five rounds) |
| Fox | Blender | 239 | 37 | 24 | 33k | 380k | 23k | $0.67 | 25 (22-26) |
| Penguin | gcad | 116 | 18 | 10 | 46k | 222k | 13k | $0.50 | 26 (22-27) |
| Penguin | Blender | 1085 (227 and 319 in the two rounds without a stall) | 33 | 22 | 31k | 353k | 22k | $0.64 | 26 (24-26) |

- On the fox, 4 of 5 Blender rounds hit the 24-request cap before the model stopped; gcad finished in 17 requests or fewer in every round. A Blender round that ran longer would cost more.
- Rounds reaching a judge score of 28 on some intermediate snapshot: fox gcad 3 of 5 (mean 430 s, 7 edits, $0.53 to get there), fox Blender 2 of 5 (208 s, 17 edits, $0.56), penguin gcad 0 of 5, penguin Blender 1 of 5.
- Mean cost: fox gcad $0.83 vs Blender $0.66, penguin gcad $0.49 vs Blender $0.69.

### Stalls

Four rounds contained a single model request of 915-925 s (fox gcad round 2, penguin Blender rounds 1, 3 and 5). In all four the tools took under 2 s, there were no retries and no HTTP errors, the request had no cached prefix, and the HTTP response headers came back within 30 s: the router accepted the request and then held the body for about 15 minutes, which points at an upstream or router-side timeout and retry, not at gcad or Blender. The cause inside the router is unverified. These stalls hit one gcad round and three Blender rounds, so the penguin Blender time is dominated by them. Medians still carry them, which is why the penguin Blender row lists the unstalled rounds.

### Does the claim hold?

Partly, and not on every axis.

- Quality: yes on the fox. gcad scored 28 in all five rounds against 25 median for Blender (22-26), under the same renderer. On the penguin they tie at 26.
- Speed: yes on the penguin (116 s against 227-319 s in clean rounds), roughly equal on the fox (205 s against 239 s). gcad uses fewer requests and fewer tool calls on both briefs (15 vs 24 and 26 vs 37 on the fox, 10 vs 22 and 18 vs 33 on the penguin).
- Cost: mixed. gcad is cheaper on the penguin ($0.50 against $0.64) and about 20% dearer on the fox ($0.81 against $0.67), because its larger prompt and its bigger tool results leave twice the uncached input per round. Once caching is priced properly the cost gap is small either way, and nowhere near the 80% seen before.
- Tokens: gcad sends more uncached input per round (60k and 46k against 33k and 31k) and fewer output tokens on the penguin (13k against 22k), equal on the fox.

So "faster, fewer tool calls and better or equal quality" holds on both briefs. "Cheaper" holds on one of two. Limits: 5 rounds per cell, two briefs, one model, one judge scoring single images, cache write cost not modelled, and the stall rounds were not rerun.
