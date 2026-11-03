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

## Speed, calls, tokens and cost (issue 181)

Setup: `costseries.sh` runs 5 rounds per side on two briefs (fox, penguin, see `briefs.py`), same model both sides (`code-x-high`, routed to claude-opus-5-5), 24 turn cap. `foxbench.py` records wall-clock seconds (with the per-edit screenshot time excluded), tool calls, prompt and completion tokens and cost per round, plus a snapshot after every edit. Cost uses assumed list prices of $5 per million input tokens and $25 per million output (`PRICE_IN`, `PRICE_OUT`); the router did not report a price. `costreport.py` scores each final image and every intermediate snapshot alone (1-10 on four axes, sum out of 40, strict rubric) and finds the first edit reaching a score of 28. Raw numbers: `fox-cost-scores.json`, `penguin-cost-scores.json`.

Renderer fix: the OBJ export now writes per-vertex colours (`v x y z r g b`), so soft borders survive. `foxbench.py` imports every gcad OBJ into Blender, wires the vertex colour into the material, and judges gcad through Blender's own `look` tool, exactly like the Blender side. All scores below use that same renderer on both sides, same background. Regression test: `obj_export_writes_soft_colours_per_vertex` in `tests/mesh.rs`.

Medians over 5 rounds (means in brackets where an outlier matters):

| Brief | Side | Seconds | Tool calls | Input tokens | Output tokens | Cost | Judge score /40 | Rounds reaching 28 |
|---|---|---|---|---|---|---|---|---|
| Fox | gcad | 202 (376 mean) | 29 | 414k | 22k | $2.64 | 28 (26.6 mean) | 4 of 5 |
| Fox | Blender | 192 (193) | 26 | 215k | 17k | $1.47 | 24 (23.2) | 0 of 5 |
| Penguin | gcad | 121 (496) | 18 | 249k | 13k | $1.55 | 24 (24.4) | 0 of 5 |
| Penguin | Blender | 211 (207) | 27 | 264k | 20k | $1.85 | 24 (25.2) | 2 of 5 |

Where it was reached, the time to a score of 28: fox gcad 331 s mean over 4 rounds (about 6 edits, $0.97); penguin Blender 150 s (9 edits, $0.92). The other two cells never reached it.

The gcad means are inflated by two stalls per brief of 1000+ seconds that were all model latency (`llm_seconds` is within 5 s of `seconds`; gcad's own tools take under 10 s a round). Medians are the fairer number.

### Does the claim hold?

No, not as stated. gcad mesh is not faster or cheaper than Blender MCP across these runs.

- Fox: gcad scored clearly higher (28 vs 24 median, and 4 of 5 rounds reached 28 against none), but it used about twice the input tokens and 80% more cost, with similar wall-clock time and about 3 more tool calls.
- Penguin: gcad was faster (121 s vs 211 s median), used fewer calls (18 vs 27) and cost less ($1.55 vs $1.85), with equal median quality (24 each). Blender had the higher mean score and reached 28 twice, gcad never.
- Combining the two briefs, there is no consistent win on any efficiency axis. Cost per round is dominated by re-sending the long gcad skill prompt plus images each turn, which is why the fox gcad input tokens are so high. Quality per round was ahead for gcad on fox only.
- The earlier 0-6 loss under the same renderer was a renderer effect: with vertex colours exported, gcad's median fox score under Blender's renderer is above Blender's own.

Limits: 5 rounds per cell, one judge model scoring single images (not pairwise), one model family, penguin never had a tuned skill example. A win on efficiency would need a shorter system prompt or fewer turns; that work is not done.
