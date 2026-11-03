---
name: gcad
description: Model mechanical parts and assemblies with gcad, a parametric CAD where every line of a `.gcad` part file or `.gasm` assembly file is one operation, like G-code, checked by the `gcad` CLI. Any design with more than one part is a `.gasm` assembly. Use when writing or editing `*.gcad`, `*.gasm` or `*.gsim` files, sketching and extruding parts, adding holes, fillets, shells, gears or text, picking faces with selectors, placing parts with mates, joints, couples and patterns, checking interference, posing an assembly, sizing joint motors with a `.gsim` torque simulation, exporting STEP, STL, 3MF, SVG drawings and a bill of materials, or modelling organic mesh shapes such as characters, creatures and game assets from blended blobs and limbs sculpted with selections.
---

# gcad

A part is a text file of operations, one per line, run top to bottom against modal state the
way G-code runs against a machine: a workplane, a sketch, the solid, named faces. There are no
comments and no blocks. Every line is an operation, so you write the design by appending lines
and check it by running them.

```
let w=40 h=30 t=3
rect w h r=4
base: extrude t
plane base.end
mounts: hole 3.2 15,10 -15,10 -15,-10 15,-10
fillet 0.8 base.end&base.side
```

Anything made of more than one part is an assembly: a `.gasm` file brings the parts in from
part files, places them with mates, joins them with joints and checks that nothing overlaps.
That holds for two parts as much as for thirty: a box and its lid, a bracket and its bolts, a
shaft and its gear. Plan a multi-part design as a `.gasm` from the start, and check, render,
export and `bom` the `.gasm`, not a part file. A part file holds one part, or several parts as
bodies when they share sizes, like a box and the lid that fits it; the `.gasm` brings each in as
`part.body`. Sketch and solid operations are errors in a `.gasm`, joints are errors in a
`.gcad`.
Split a big design into sub-assemblies: `part az drive.gasm` brings one in with its parts,
joints and axes as `az.spindle`, `az.spin`, `az.wa`; `move az x,y,z` and `rotate az ...` place it
whole, and a `.gsim` can simulate the sub-assembly's file on its own.

## Installing

Run `gcad --version` first. If it is missing:

```sh
curl -fsSL https://raw.githubusercontent.com/v0l/gcad/master/install.sh | sh   # Linux, macOS
```

On Windows take the zip from https://github.com/v0l/gcad/releases. Building from source is a
plain build: `Cargo.toml` pins a commit of the kernel fork's `cad-regressions` branch through
`[patch.crates-io]`, so cargo fetches it.

```sh
git clone https://github.com/v0l/gcad
cd gcad && cargo build --release
```

## Reference

`gcad docs` prints the whole format: every operation with its parameters, the selectors, the
groups each operation records, and the assembly lines. Read the table for the operation you are
about to write before writing it. This skill covers how to work, not every parameter.

## The loop

For an assembly, run the loop on each part file alone, then on the `.gasm`.

1. Add or change a few lines.
2. `gcad check part.gcad`. It runs every line and prints what each did: the profile count after
   a sketch line, the face count, volume and bounding box after a solid line, the plane origin
   and axes after a `plane`. It stops at the first failing line, prints why, and exits 1.
   `--set w=50` overrides a `let`, `--time` adds how long each line took.
3. `gcad render part.gcad /tmp/p.png` and look at the PNG (iso, top, front from -Y, right from
   +X). A clean check does not mean the part is right: a hole on the wrong face or a fillet on
   the wrong edges only shows in the picture.
4. `gcad query part.gcad 'base.end&base.side' --line 5` before any `fillet`, `chamfer` or
   `plane` whose selector you are not sure of. It lists the faces or edges it matches.

Read the volume and bounding box after every solid line and compare them with what you
expected. That catches most mistakes one line after you make them: a cut that removed nothing,
an extrude that went the wrong way, a hole that missed.

`gcad part.gcad` opens the viewer, which rebuilds on every save. Start it for a human who is
watching; you work from `check` and `render`.

## Writing a part

- `label: operation args name=value`. Label every line you will select from later. An unlabelled
  line is `L<number>`, which changes when lines move.
- Expressions have no spaces: `w/2-r`, `atan2(y,x)`, `sqrt(a*a+b*b)`. Angles are degrees.
- The sketch builds up from profile lines (`rect`, `circle`, `poly`, `ngon`, `slot`, `ellipse`,
  `spline`, `text`, `gear`, `pen` ... `close`) and is used up by the next `extrude`, `cut`,
  `revolve` or `sweep`. A profile inside another is a hole in it.
- `plane` sets where the next sketch goes. `plane base.end` puts it on a face; coordinates on a
  face stay world coordinates projected onto it.
- `extrude` adds, `cut` removes going into the solid, `hole` drills at a list of points.
  `mode=cut` and `mode=intersect` work on `revolve`, `sweep` and `loft`.
- `body name` starts another solid in the same file: a second part that shares this file's
  sizes, which an assembly brings in as `part.name`, or a piece you `combine` into this part.
  When a part file ends with several bodies, `check` reminds you to bring it into a `.gasm`.
- `color` and `material` on a body feed the STEP colours, the viewer and the mass in `bom`.
- The standard library has about 210 complete materials (density, `E`, `nu`, `yield`, `k`,
  `alpha`, `cp`, colour, roughness, metallic, texture): steel grades, `rebar-b500b`, alloys,
  plastics, printing materials (`pla-cf`, `petg-cf`, `pa-cf`, `resin-tough`, `ti64-printed`),
  timbers (`c24`, `glulam-gl24h`, `clt`, `osb`), `concrete-c30-37`, `screed`, `cement-render`,
  `brick`, `pir`, `rockwool`, `foamglass`, `glass`. Run `gcad materials` before inventing one.
- Materials not in the standard library go in a `materials.toml` beside the files or above them
  (`[petg-cf]` then `density = 1.3`, `E = 3.8`, `nu = 0.36`, `yield = 52`; `E1 E2 E3 Xt Zt`
  for printed parts, axis 3 the print direction). `materials file.xml` or `file.FCMat` loads
  Ansys MatML or FreeCAD cards. `gcad materials` lists what a file can use.
- The same tables set the look: `color`, `roughness`, `metallic` (1 only for bare metal;
  painted or powder-coated metal is 0), `clearcoat`, `transmission = 1` with `ior = 1.5` for
  glass. A `[steel]` table keeps every key of the standard steel it does not give.
- Textures: `texture = "wood"` (also `osb`, `concrete`, `stone`, `brick`, `sand`, `fabric`,
  `foam`, `rubber`, `brushed`, `carbon-fibre`) with `texture_scale` in mm and
  `texture_rotation`, or image maps `color_map`, `normal_map`, `roughness_map`, `metallic_map`
  beside the library. Grain runs along a part's longest side.

Selectors pick faces: `label`, `label.end`, `label.side`, `label.start`, `>Z` (flat faces
furthest along +Z), `+Z` (every flat face facing +Z), `all`, and `a,b` for either. Edges are
`a&b` (where a face of `a` meets a face of `b`) and `x|y`. `base.end&base.side` is the top
rim of an extrusion and leaves hole rims alone. `gcad docs` lists which groups each operation
records.

## Assemblies

Lay a design out as part files and one assembly:

```
box/
  parts/enclosure.gcad   case and lid as two bodies, sized from one set of lets
  parts/screw.gcad
  box.gasm
```

Parts that share sizes can share a part file, one body each, so a change to `w` moves them
together. Parts that stand alone (fasteners, motors, a gear used in several places) get their
own file. The `.gasm` sets sizes on its `part` lines (`part case parts/enclosure.gcad w=80`).
A part can be drawn where it sits in the assembly, as the enclosure lid is, or at its own origin
and placed with `move`, `rotate` and mates. Reuse one part file for every copy, with different
variables or `pattern`.

```
part case ../parts/enclosure.gcad w=80
part s1 ../parts/screw.gcad
concentric s1:shank.side case.lid:screws.side near=32,17,30
flush s1:head.start case.lid:top.end
pattern s1 holes=case.lid:screws.side
axis hinge -40,25,30 40,25,30
joint lid case.lid case.main turn about=hinge min=-110 max=0
interference none
interference joint=lid steps=12
explode case.lid 0,0,30
```

- `part:faces` selects faces of a part with that part file's own labels.
- Place a part with `move` and `rotate` first, then mate it. The first mate of a part fixes it
  to the other part; later mates must already hold and are then kept. `flush` wants the faces
  to face each other.
- `joint NAME child parent turn about=AXIS` or `slide along=x,y,z`, with `min=` and `max=`.
  Define every joint with the assembly in its home position, then `pose` it at the end.
- `couple driven driver ratio=` makes one joint follow another: `-teeth_a/teeth_b` for a gear
  pair, `pi*d/360` mm per degree for a rack. A planetary sun turns `1+ring/sun` per turn of the
  carrier, a planet `-ring/planet` relative to it. Add losses with `efficiency=` (driven
  turning driver) and `back=` (the load turning it back), or for a worm `lead=` in degrees and
  `friction=` (0.05 to 0.15 greased), which works out both and whether it locks.
- `pattern part count=3 angle=360 axis=A` copies a part with its mates, joints and couples.
- End the file with `interference none` and sweep each joint once with
  `interference joint=NAME steps=12` to find the range where it clashes. Set `min`/`max` from
  that, then keep the sweep as `interference none joint=NAME steps=N` so the check fails if a
  later change clashes; without `none` a sweep only reports. Pick `N` so a step is not a whole
  tooth pitch of any geared joint, or a gear pair out of phase never shows. A couple's gear and
  pinion may share up to `module`³ where their teeth meet.
- Size what goes inside before you draw it. `gcad measure case.gasm space at=x,y,z` grows the
  largest empty box around a point and names what stops it on each side, then gives the free
  distance from the point along each axis, which is the inside width. `gcad measure asm.gasm box`
  lists every part's bounding box, and `clearance a b` the nearest gap between two parts and
  where it is. None of these change the file.
- `gcad bom asm.gasm` counts the parts; `gcad render asm.gasm x.png --explode` draws it apart.
- Fasteners come from the built-in library rather than a part file: `part s1 shcs:M3x10`
  (also `fhcs:`, `hex:`, `nut:M3`, `washer:M3`, `insert:M3`, `standoff:M3x10`, `bearing:608`,
  `dowel:4x16`). Screw heads bear on z=0 with the
  shank down; mate `s1:shank.side` and `s1:head.start`, nuts and washers by `bore.side`. Size the
  holes they go in with the same name: `hole M3 x,y` (clearance), `cbore=M3`, `csink=M3`,
  `fit=tap`, `fit=insert`. Level a flat head with `distance s:head.start lid:top.end 0`.
- Enclosures: `lip >Z 3` on the case rim and `groove <Z 3` on a lid shelled the same way and
  placed 3 lower; `boss 7 20 x,y ... hole=M3 fit=insert ribs=4` for screw posts; `vent 20 3 x,y
  count=3 step=0,-5` cuts through the one wall under the workplane; `snap l t w x,y dir=` for
  clips. See `examples/assemblies/case.gasm`.
- A vendor STEP assembly comes in whole with `part box vendor.step`: each solid where the file
  puts it, named after its product.

## Gears

`gear teeth module` draws an involute spur gear with a tooth on `angle=` (default +x);
`gear teeth module internal` inside a larger `circle` makes a ring gear. Centres are
`module*(a+b)/2` apart outside, `module*(ring-gear)/2` inside. Two gears mesh when a tooth of
one points at a gap of the other: with a tooth on the line of centres on one, turn the other
by `180/teeth`. Use 17 or more teeth at 20 degrees; smaller pinions overlap their mate at the
root because undercut is not modelled. `interference none` catches a gear out of phase.
`gear ... helix=20` then `extrude` makes a helical gear (mate it with `helix=-20`), and
`gear ... cone=atan(a/b)` then `extrude` a bevel gear. `rack teeth module` draws a rack with its
pitch line on `at`; a gear meshes with its centre `pitch_diameter/2` above it.

## Sheet metal

`sheet t r= k=`, then a rect or poly and `base: tab` for the flat base, then
`walls: flange base.end&>X 10` for each bent edge (`angle=`, off a top edge it rises up, off a
`base.start` edge it goes down). Flange a flange from its `end`; pick the one edge on the side
the new flange should rise from (`>X&walls.end`, `walls.face&walls.end`), since selecting both
sides of an edge fails. Put holes and through cuts in the plates so they reach the flat
pattern, then `gcad export part.gcad flat.dxf`, or end the file with `unfold` to see it.

A laser-cut and bent part is drawn flat first: sketch the whole blank (any outline, `slot` for
round ends, holes as circles), `blank: tab`, cut the holes and keyways while it is flat, then
`left: bend a,b c,d angle=-90` along each bend line in the tab's workplane. The side with the
sketch origin stays put; labels made on the blank carry over, and the blank is the flat
pattern. The blank must cross each bend on straight edges with no holes in the bending strip.
`dart left -7 7 l=20 w=8` presses stiffening darts into a bend; keep them clear of holes.

## Weldments

`member tube 40x40x3 0,0,0 500,0,0 500,300,0 0,300,0 closed` lays a mitred frame of square
tube, one body per straight run; also `bar`, `pipe DxT`, `rod D`, `angle`, `channel`.
`gcad bom` turns them into a cut list. `up=`, `rotate=` and `offset=u,v` set how the profile
sits on the path. `material=wood` assigns every member. Add `count=19 step=400,0,-10`
for equally spaced copies in one line, or `count=8 angle=360 axis=hub` for a circular array.
The count includes the seed; copies retain the material, finish and cut-list entry. In a row,
`end_step=x,y,z` moves only the last point (studs under a sloping roof) and `when=expr` skips
instances where it is 0, with `k` the index (openings). Never write one `member` line per stud.

## Simulations

A `.gsim` file is a simulation of a part or assembly, run with `gcad sim x.gsim`. It prints
readings, writes `x.result.bin` (binary; `gcad check x.gsim` prints its readings again), and fails
when an `expect` line does not hold. Kinds:
`torque`, `static`, `thermal`, `modal`, `buckling` (load factor before buckling under the
`static` lines; keep it above about 3), `flow` (wind on the GPU), `magnetic`, `frame`, `conduction`
and `acoustics` (the last three are described below); the plan and limits are in `docs/simulation.md`.
Progress goes to stderr, one line per finished stage and one every 10 s while a stage runs, with
the iterations, residual and time left; `-q` turns it off. On a slow
machine run it on the sims server instead (`gcad sim x.gsim --remote`, see below).

```
sim torque
part arm.gasm
payload hand 300
sweep shoulder elbow steps=12
expect shoulder<15 sun_s_turn<4
```

A payload is in grams, at the part's centre of mass unless `at=x,y,z`. Several joints on one
`sweep` make a grid of poses. Every moving joint gets a reading: the worst holding torque (N·m) or force (N) against
gravity and where it happened. Couples pass the load on, so a sun gear driven by a joint
at `ratio=4` reads a quarter of it: that is the motor's torque. With lossy couples that
reading is the torque to drive the load and `joint.back` the torque the load pushes back with
(0 when a worm locks). `torque az 6.3` adds a steady joint torque, like wind, and `joint ...
drag=0.05` gives a joint bearing and seal friction in N·m (N on a slide). Add `module=1 face=10
gear=wheel pinion=worm` to a `couple` for its mesh forces (tangential, radial, axial), Lewis
bending and Hertz contact stress and safety, and `speed az 1` (rpm) for power, mesh heat and
worm sliding speed; motor torque also splits into `.ideal`, `.friction` and `.drag`. Parts need a `material` to
weigh anything. `gcad check x.gsim` says when the result is stale and `gcad render x.gsim
x.png` plots the sweeps. The viewer shows the readings, draws the setup before a result
exists, and has a run button. Materials also carry `E= nu= yield= k= alpha=`.

`sim static` solves linear stress on a part file (mm, N, MPa):

```
sim static
part bracket.gcad
material aluminium
mesh 2
base: fix <Z
load boss.end force=0,-200,0
top: probe 18,12,12
expect stress<80 safety>2
```

`slide faces` holds only the normal (symmetry planes), `bearing seat.side` holds a shaft's bearing band sideways but free to tilt (add `axial` on one of them), `pin h.side` holds a round face free to
turn, `bearing kfl:bore.side spindle axial C0=2900` lets one part turn in another's bore on a ball row (radial load, `.axial` and `.safety` against C0, no moment), `pressure faces 10` pushes in MPa, `torque shaft.end 5` twists in N·m, `warm 40` heats the whole part (thermal expansion stress); `warm -100 part=bolt` cools one part, which preloads a bolted joint.
Readings: `stress`, `displacement`, `safety`, each support's reaction, `probe.stress`. `stress`
and `safety` leave out one element size of each part around its supports and bonds, whose singular peak reads as
`support_stress`. A sharp inside corner is a singularity too: fillet it or probe away from it. `gcad render x.gsim x.png` draws the
stress on the deformed part.

`sim thermal`: `temperature faces 60`, `convect faces h=10 ambient=25` (W/m²·K; still air 5-10),
`heat faces 3` (W in), `source 3` (W through the volume). Readings: `temperature` (hottest),
`coolest`, heat through each labelled boundary, `probe.temperature`.

Static, thermal and modal take a `.gasm` too: touching parts are bonded, each in its own
material; pick faces as `part:faces` (`fix base:<Z`). `contact a b` makes a pair press and let go
instead of bonding (static); `friction=0.3` makes it hold until it slips.

`leave ctl* *screw*` drops parts that carry no load (boards, cables, screws in clearance
holes) from an assembly sim.

Add `converge` to any meshed simulation to solve again on a coarser mesh and get a note naming
readings that moved more than 5%; trust the ones it does not name.

`sim modal` with `fix` lines and `modes 6`: readings `mode1`... in Hz, each with its effective
mass share along x, y, z. No supports means free-free (rigid modes dropped).

`sim flow` blows wind past a part or assembly on the GPU (lattice Boltzmann):

```
sim flow
part head.gasm
leave *screw* *nut*
wind 33.3 toward=0,1,0 about=0,0,0
mesh 5
```

Speed in m/s, `toward` is where the air goes, moments are about `about`. `mesh` is the cell in
mm (default: 32 cells across the square root of the frontal area); run `--dry-run` first, it
prints cells, GPU memory and steps. Readings: `drag`, `side`, `lift` (N), `cda` (m²),
`moment.x/y/z` (N·m), `part.drag` per part, `pressure`/`suction` (Pa). Round members under
about 10 cells across read like square bars (drag up to 1.8x too high); sharp-edged boxes read
well. Then `wind flow.gsim` in a `sim static` loads the parts with that surface pressure; it
needs the flow's result to be fresh. `gcad render wind.gsim flow.png --flow` draws the air speed on
slices through the parts, and `--lines` the streamlines round them in 3D. Only `leave` parts inside a closed box: a gland or screw left out is
a hole the wind blows into.

`sim frame` is for buildings: each body is a straight prismatic member (a column drawn floor to the
girder's centreline, a girder between column faces), joined where ends meet. `fix`, `pin` and `slide`
take member end faces (`left:<Z`); `load left:>Z force=...` on an end face is a joint force and on a side
face (`girder:+Z`) a line load; `gravity 0,0,-9.81` adds the self weight; `combine uls weight:1.35 live:1.5`
makes `uls.stress` and `uls.displacement`; `release beam:<X` pins an end; `modes 3` and `buckling 2` add
`mode1` (Hz) and `buckling1` (a load factor). A body under 3 times its depth is refused: `leave` it.

`sim conduction` is for thermal bridges: parts with a conductivity (`concrete`, `brick`, `glass`, `eps`,
`wool`, or `k=`), `inside: convect concrete:<Z h=7.69 ambient=20` and `outside: convect board:>Z h=25
ambient=-5` (h is 1/R; lines with one label are one side). Read `coupling` in W/K, `fRsi` (keep it above
0.7) and `coolest`. Boundaries need flat faces square to X, Y or Z; round parts are staircased.

`sim acoustics` takes the air inside a room as the part: `absorb all plaster`, then `absorb -Z carpet`
(later lines win; every face needs one), `speaker x,y,z` in mm, and `hall: probe x,y,z` for each listener.
It reads `hall.T30`, `hall.EDT`, `hall.C80`, `hall.D50` and `hall.STI`, against `sabine` and `eyring`.
Bodies inside the room are furniture. Raise `rays` if a reading is missing.

## Slow hardware: run on the sims server

Offer the sims server whenever a simulation would be slow or cannot run on the user's machine.
Signs: no GPU or a weak one (`gcad gpu-bench` prints the FP32 TFLOPS and memory bandwidth as
JSON and fails with no GPU), `gcad sim x.gsim --dry-run` sizing a flow or field problem past the
card's memory, a laptop, or a local run that has gone over a few minutes. Say what it costs and
let the user decide: runs are prepaid in euro and charged per second of the runner's time.

```sh
gcad remote account                  # id, balance, what is held; no account yet: gcad remote signup
gcad remote runners                  # GPU, TFLOPS, price an hour, and which gcad build each runs
gcad sim x.gsim --remote --max-price 0          # free runners only, no balance needed
gcad sim x.gsim --remote --max-minutes 20 --max-price 2.50
gcad remote jobs                     # recent runs, their time and cost
gcad remote cancel ID                # the time it used is still charged
gcad remote login TOKEN              # use the account on another machine
```

- Never make an account or start a paid run without the user's yes. Start with `--max-price 0`
  and a short `--max-minutes`, since a run holds `minutes x price` from the balance until it ends.
- A runner takes only a job from a gcad with the same major and minor version (`gcad --version`,
  so 0.4.x with 0.4.x); with none online for yours the command says which versions they run.
- Credit is the user's to add, at https://apexstrata.ie/shop/gcad-sim-credit/ with the account id from
  `gcad remote account`; an "out of credit" error prints the link. Give the user the link, never pay.
- The model is built here first, so fix a model that fails locally before sending it. `torque`
  runs stay local. The result is written beside the `.gsim` as if it had run here, and rerunning
  the same command after an interruption picks the job up instead of paying twice.
- The viewer shows it too: the simulation card has `run on server` and a `server` toggle that
  reads the balance and running jobs, and opens a card with the account, runners, the run
  limits and recent runs, each with a cancel button.

## Drawings

A `.gdwg` file is a drawing of a part or assembly: `part file` once, `title`, then for each page a
`sheet "name" size=A3 scale=2:1`, its `view`s and `section`s, and the dimensions and notes on
them. Check with `gcad check x.gdwg`, write with `gcad export x.gdwg x.pdf` (`.svg` writes one
file per sheet), then `pdftoppm -png -r 70 x.pdf /tmp/x` and look at it. Read every page: a
clean check does not mean it reads well. A second `part` line swaps the model for the next sheet.

```
part ../parts/shaft.gcad d=12 L=131
title "El shaft, 1 off, Ø12 silver steel"
sheet "El shaft" size=A3 scale=3:2
view from=-y place=40,40 label="front"
dim -6,0,0 -6,0,131 offset=14
section from=-y at=0 place=130,40 label="section A-A"
label main:keyway.side side=right
label main:tap0.side
note "Ø12 silver steel, 131 long|Break all edges 0.5" place=20,20
```

- **Sheet and scale.** `size=A4..A0` is landscape, with a title block in the bottom right that
  nothing may overlap. `scale=2:1` enlarges, `1:25` reduces. Pick the largest scale at which
  each view fits: a 166 mm long shaft at 2:1 needs an A3 sheet, not A4 (the error says what did
  not fit). Give a view its own scale with `scale=` for a detail.
- **Views.** `view from=-y|+y|-x|+x|+z|-z` (front, back, left, right, top, bottom), with part
  patterns as in `gcad bom` (`*`, `!pv.*`, `profile=`). `section from=-y at=0` cuts square to
  the direction it looks. `place=x,y` puts the view's lower left corner x, y mm from the
  sheet's lower left; leave it off to flow views left to right. `window=u0,v0,u1,v1` crops.
- **Labels, the default way to annotate.** `label "text" target ...` or `label target` points
  at something in the last view and gcad places the text, so you never write coordinates for
  text. A target is `body:faces` (a hole or round face gets its leader on the rim and, with no
  text, writes its own `⌀12`, `4× ⌀3.4` or `M3 tapped (⌀2.5 drill)`), `body:a&b` (an edge) or
  `x,y,z`. The body is a body name in the part file (`main` for a plain part, `side_b` for a
  `body side_b` in `driveplates.gcad`) or a part in the assembly. Group names are the labels in
  the part file (`bore.side`, `fix.side`). Several targets on one line share one text and
  leaders to each; one target gets one leader.
- **How labels are placed.** Collected per view and laid out when the sheet ends, so every view,
  dimension, note and the title block on the sheet is an obstacle. They go in a column beside
  the view, ordered so leaders do not cross, clear of the part. `side=left|right|top|bottom`
  picks the column; use it to keep a long note off the neighbouring view. If it errors with
  "no room beside the view", shrink the scale, `place` the views further apart or use a bigger
  sheet. `callout x,y,z "text"` without `to=` is a `label`; with `to=dx,dy` it is the old manual leader.
- **Dimensions.** `dim x,y,z x,y,z offset=N` between two model points in the last view, drawn
  `N` mm outside the part: positive offset puts a horizontal dimension below and a vertical one
  to the left. Dimension from edge points on the near side (`-4,0,0` to `-4,0,83` on a shaft along
  z viewed from -y) and stack by raising the offset (`offset=12`, then `24`). It prints one decimal
  (102.2) and `text="..."` overrides. `dims parts along=h|v of=ends|edges|centres` chains between
  parts of an assembly, `style=ordinate` sets out from the first mark, `level` marks heights on an
  elevation, `cutlist` tables structural members, `nest` lays panels out on stock sheets with the saw cuts in order.
- **Notes.** `note "line one|line two" place=x,y` (an error if it would sit on a view) for the material, finish, general tolerances
  and anything a view cannot show. Keep a made part's drawing to one sheet: views, the
  dimensions a machinist needs, one label per feature, a note for the rest. Say the quantity in
  the title (`"Worm shaft, 2 off"`).
- **Long parts.** Turn a long part lying along z so it draws horizontally: make a one-line
  `.gasm` (`part p ../parts/x.gcad` then `rotate p 90 axis=y`) and draw that, then label with
  `p:feature.side`.
- PDF text is Latin-1: `Ø`, `⌀`, `×`, `°` and `±` print, other symbols become `?`.

## Meshes

Characters, creatures and other organic game assets are mesh bodies, not solids. Never write
triangles. Block the shape out with labelled `blob` and `limb` lines that `blend` into each
other, `mirror x of=` the left-side parts, `remesh`, then sculpt: `select` a part by its label
(or `sphere=`/`box=` with `falloff=`), then `move`, `scale`, `rotate`, `inflate`, `smooth`.

- Label every blob and limb (`head:`, `arm_l:`). The labels become the mesh's parts, and
  selecting a part by name is far more reliable than guessing coordinates from a picture.
- Model side parts on +x as `_l` and `mirror x of=` them; `select ... mirror=x` then edits
  both sides with one line.
- Read what each sculpting line reports: `moved 0 vertices` means the selection missed.
- Detail costs triangles only where you `refine`: keep `remesh` coarse, `select` the face and
  `refine 0.1` it before `crease`, `pinch` or `noise` there. A line that needs a finer mesh
  says so.
- End a game asset with `select all` and `decimate 5000` (or the budget it has), behind a
  variable (`if budget>0 decimate budget`) so the full detail stays available for renders.
- Small features are their own labelled blobs (eyes, toes, nose), not dents. A blob gets its own
  colour with `color ... part=` and stays sharp through `decimate`.
- Never guess where fine detail goes. `gcad inspect file.gcad v.png --part head --view right`
  draws the part with a coordinate grid and part names; `--pick x,y` on that picture prints the
  surface point, part and normal under a pixel. Write `select sphere=`, `crease` and `pinch` with
  the picked numbers, then `gcad inspect ... --line N` to see the selection outlined and check it.
- `gcad render` after the blockout and again after sculpting, and compare the four views with
  what you meant. Fix the blockout first; sculpting only nudges.
- Export with `gcad export file.gcad out.obj`: each part is its own `g` group.

## Output

```sh
gcad export part.gcad part.step          # also .stl .obj .3mf
gcad export part.gcad part.svg --section y=0   # four-view drawing with a hatched section
gcad export part.gcad flat.dxf           # the sheet metal flat pattern
gcad export sheet.gdwg sheet.pdf         # the drawing sheets, also .svg
gcad export asm.gasm asm.step            # one product per part file, instances placed
gcad bom asm.gasm --csv
```

## Traps

- `XZ` has normal -Y, so `plane XZ offset=12` sits at y=-12, and `extrude -36` from it goes
  toward +Y. Read the plane line `check` prints before extruding from it.
- On a face whose outward normal is +Y the plane's x runs along -X (it is "looking at the face
  from outside, Z up"). Coordinates on it are mirrored; prefer the opposite face or a world
  plane for holes on such faces.
- `hole` and `cut` go into the solid against the plane normal. A hole whose plane faces away
  from the solid removes nothing and the line fails or the volume does not change.
- Booleans are the slow and fragile part of the kernel. Unions of faces lying in one plane work,
  but when one fails ("not oriented and closed"), draw the shape as one profile instead of
  adding overlapping pieces one at a time.
- Fillets where three rounded edges meet need flat faces and straight edges. Inside and
  outside edges can meet there (pocket rims, boss feet, L shapes), but not at a corner
  whose third edge stays sharp. `shell` hollows the extrusion that owns the open face,
  so shell before adding other features; fillets on it can come first, and the inside
  follows them at their radius less the wall.
- Boolean edges that are lines or circles (hole rims) are exact; other intersections, such as
  two crossing cylinders, are fine polylines.
- `thread M6 on=rod.side` needs each end of the round face to be flat and square to it, or a
  chamfer deeper than the thread (`chamfer 1 rod.end&rod.side` on an M6). Chamfer first and
  thread last. Holes take `thread M6 on=h.side` after `h: hole 5 0,0 thread=M6`, with or
  without `csink=`.
- Assembly parts are built in parallel and cached per file and variables, so many copies of
  one part cost one build.

## Worked examples

- `examples/parts`: single parts with analytic-volume tests (`plate`, `bracket`, `pipe`,
  `ring`, `enclosure`, whose case and lid are two bodies sharing one set of sizes).
- `examples/assemblies/enclosure.gasm`: the enclosure's case and lid hinged together, with screws
  mated into the lid and patterned into every hole; the screws stop the hinge.
- `examples/assemblies/linkage.gasm`: a four-bar linkage, a closed loop of joints.
- `examples/mesh/creature.gcad`: a creature blocked out of blobs and limbs, mirrored, remeshed
  and sculpted by part name.
- `examples/mesh/fox.gcad`: a fox with real anatomy (thighs, hocks, toes, cupped ears, almond
  eyes, cheek ruffs), part colours, and `--set budget=8000` for a game-ready mesh.
- `examples/robot/arm.gasm`: a 34-part robot arm. Every joint is a motor on its axis driving a
  planetary reducer whose ring is cut into the housing; suns and planets are coupled at their
  ratios and the planets are patterned. The gripper is two racks on one pinion. Copy its
  patterns before inventing new ones. `examples/robot/arm.gsim` sizes its motors.
- `examples/truck/truck.gasm`: a 90-part 1:10 crawler. Four-link solid axles kept level by
  couples, coil-over shocks, telescoping driveshafts and the drag link and tie rod are closed
  loops that settle when `fsus`, `rsus` or `steer` is posed. Shared heights (`hi`, `rlo`) go
  from the `.gasm` into every part file. Fasteners are library screws and nuts placed with
  `concentric` and `distance` and patterned into hole sets.
- `examples/parts/bracket.gsim` (static stress) and `examples/parts/heatsink.gsim` (heat with
  convection).
