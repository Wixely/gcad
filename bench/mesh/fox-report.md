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
