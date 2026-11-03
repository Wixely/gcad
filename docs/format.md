# The gcad format

A `.gcad` file is a list of operations, one per line, run from top to bottom. Every
line is an operation; there are no comments and no blocks. Blank lines are skipped.

```
let w=40 h=30 t=3
rect w h r=4
base: extrude t
plane base.end
mounts: hole 3.2 15,10 -15,10 -15,-10 15,-10
fillet 0.8 base.end&base.side
chamfer 0.3 mounts.side&base.end
```

## Line shape

```
[label:] operation [argument ...] [name=value ...]
```

- `label:` names the faces the line creates, so later lines can select them. A line
  without a label is named `L<line number>`, which changes when lines move, so label
  anything you will refer to.
- Plain arguments fill the operation's parameters in order. Any parameter can also be
  given as `name=value`.
- Numbers are expressions: `+ - * /`, parentheses, `pi`, names set by `let`, and the
  functions `sqrt abs sin cos tan asin acos atan atan2 hypot min max floor ceil round`,
  like `atan2(y,x)`. Angles are in degrees.
  Expressions cannot contain spaces.
- A 2D point is `x,y`, a 3D point is `x,y,z`, each part an expression.
- Lengths are millimetres, angles are degrees.

## State

The file runs against these pieces of state, the way G-code runs against a machine:

| state | set by | used by |
|---|---|---|
| variables | `let` | every expression |
| workplane | `plane` (XY until set) | sketch operations, `extrude`, `cut`, `revolve`, `hole`, `helix`, `repeat` |
| sketch | `rect`, `circle`, `poly`, `ngon`, `gear`, `slot`, `ellipse`, `spline`, `text`, `pen` ... `close` | `extrude`, `cut`, `revolve`, `sweep`, `section`, which empty it |
| sections | `section` | `loft` |
| path | `path`, `helix` | `sweep`, `plane path`, `repeat along=path` |
| points | `point`, `dist`, `horizontal`, `vertical`, `angle` | any 2D point |
| axes | `axis` | `revolve`, `rotate` |
| solid | every operation that makes or removes material | `fillet`, `chamfer`, `shell`, `draft`, `push`, selectors |
| bodies | `body` | `combine`, and export, render and assemblies, which take every body |

## Operations

Variables, conditions and other files:

| operation | parameters | does |
|---|---|---|
| `let` | `name=expr ...` | sets variables, in order, so later pairs can use earlier ones; a variable given with `--set` keeps its outside value |
| `if` | `condition operation ...` | runs the rest of the line only when the condition is not zero; `< > <= >= == !=` give 1 or 0 |
| `include` | `file.gcad` `name=value ...` | runs another file's lines here, with those variables fixed; the file is relative to this one, and a `let` in it for a name not yet set here is set here too |

Sketch:

| operation | parameters | does |
|---|---|---|
| `plane` | `on` `offset=` `rx=` `ry=` `rz=` | sets the workplane to `XY`, `XZ`, `YZ` or a flat face selector, moved `offset` along its normal, then turned about its own x, y and normal axes by degrees |
| `plane` | `x,y,z x,y,z x,y,z` | the plane through three points, x along the first two |
| `plane` | `edge=a&b` `angle=` | the plane through a straight edge, lying on face `a` at 0 and turning toward face `b` |
| `plane` | `path` `at=` | the plane square to the path, `at` a fraction of its length |
| `axis` | `name x,y,z x,y,z` | names the line through two points, for `revolve axis=` and `rotate axis=` |
| `rect` | `w h` `at=x,y` `r=` | adds a rectangle centred on `at`, corners rounded by `r` |
| `circle` | `d` `at=x,y` | adds a circle |
| `ellipse` | `dx dy` `at=x,y` | adds an ellipse of those diameters |
| `poly` | `x,y x,y x,y ...` `r=` or `c=` | adds a closed polygon, corners rounded by `r` or cut by `c` |
| `ngon` | `d n` `at=x,y` `angle=` | adds a regular polygon of `n` sides with corners on a circle of diameter `d` |
| `gear` | `teeth module` `internal` `at=x,y` `angle=` `pressure=` `backlash=` `helix=` `cone=` | adds an involute spur gear with a tooth on `angle` (0 by default, along x), teeth of `module` (pitch diameter `teeth*module`), a `pressure` angle of 20 degrees unless given, and teeth thinned by `backlash` at the pitch circle. `internal` draws the toothed hole of a ring gear instead, to go inside a larger profile, with its tooth on `angle` pointing in. Two gears mesh at centres `module*(teeth_a+teeth_b)/2` apart, a gear inside a ring at `module*(ring-gear)/2`, with a tooth of one pointing at a gap of the other | `helix=` makes the next `extrude` a helical gear, its teeth leaning that many degrees at the pitch circle, right-hand when positive; mating helical gears take opposite signs. `cone=` makes the next `extrude` a bevel gear whose teeth shrink toward the apex of a pitch cone of that half-angle, `teeth*module/2/tan(cone)` along the extrude; a pair at right angles takes `cone=atan(teeth_a/teeth_b)` and the other `90` less that. The module is the one in the sketch plane, so helical gears mesh at the same centres as spur gears
| `rack` | `teeth module` `at=x,y` `angle=` `pressure=` `backlash=` `back=` | adds a straight rack of `teeth` teeth along x, its pitch line through `at` and its teeth pointing +y, `back` (2 modules unless given) of solid below the roots. It is `teeth*pi*module` long and centred on `at`, with a tooth on `at` when `teeth` is odd and a gap when even. A gear of pitch diameter `d` meshes with its centre `d/2` above the pitch line
| `slot` | `l w` `at=x,y` `angle=` | adds a slot `l` long overall and `w` wide, turned by `angle` degrees |
| `spline` | `x,y x,y ... closed` | adds a smooth closed curve through the points |
| `text` | `"words"` `size=` `at=x,y` | adds the outlines of the words, `size` tall, starting at `at` |
| `offset` | `d` | adds a copy of the last profile grown by `d` (shrunk when negative) |
| `reflect` | `x`, `y` or `x,y x,y` | adds the last profile mirrored across the sketch's x or y axis, or the line through two points |
| `array` | `count=` and `angle=` `at=x,y`, or `step=x,y` | adds copies of the last profile around `at` or in a row |
| `pen` | `x,y` | starts a path of lines and arcs at a point |
| `line` | `x,y` | draws a straight segment to the point |
| `arc` | `x,y` `via=x,y`, or `x,y center=x,y turn=cw\|ccw` | draws an arc to the point through `via`, or about a centre |
| `dxf` | `file.dxf` `at=x,y` `scale=` | adds the closed outlines in a DXF (lines, arcs, circles, polylines) |
| `svg` | `file.svg` `at=x,y` `scale=` | adds the closed shapes in an SVG, one user unit to a millimetre, with y turned up |
| `close` | `trim` | closes the path back to its start and adds it to the sketch; `trim` instead cuts it where its last line crosses an earlier one |

Any profile line can end with `construct`: the profile is kept for reference and
left out of the sketch.

Sketch points and constraints:

| operation | parameters | does |
|---|---|---|
| `point` | `name x,y`, or `name` `near=x,y` | adds a fixed point, or a free one the constraints place |
| `dist` | `a b d` | the points are `d` apart |
| `horizontal`, `vertical` | `a b` | the points are level, or one above the other |
| `angle` | `a b degrees` | the line from `a` to `b` points that way |
| `coincident` | `a b` | the points are the same point |
| `parallel`, `perpendicular` | `a b c d` | line `a b` is parallel or square to line `c d` |
| `equal` | `a b c d` | line `a b` is as long as line `c d` |
| `midpoint` | `m a b` | `m` is halfway between `a` and `b` |
| `online` | `p a b` | `p` is on the line through `a` and `b` |

After every point and constraint line the points are solved and `check` prints them
and the degrees of freedom left. A point name works anywhere a 2D point does, and
`a.x`, `a.y` work in expressions.

Features:

| operation | parameters | does |
|---|---|---|
| `extrude` | `d`, `next` or `upto=faces` `offset=`, `both`, `draft=`, `thin=`, `twist=`, `scale=`, `mode=add\|cut\|intersect` | adds the sketch along the plane normal (negative `d` goes the other way); `both` centres it on the plane; `upto` stops at a flat face, `offset` past it; `next` stops at the first face the sketch reaches; `draft` tapers it inward by degrees; `thin` makes walls that thick inside the profile; `twist` turns the sketch that many degrees by the end, counterclockwise looking back down the extrude, and `scale` sizes it by the end, both about the sketch origin and with a plain distance only |
| `cut` | `d` or `thru`, `draft=` | removes the sketch from the solid, going into it against the plane normal |
| `revolve` | `angle` `axis=x\|y\|name` `mode=` | spins the sketch about the workplane's x or y axis through its origin, or a named `axis` |
| `path` | `x,y,z x,y,z ...` `r=` or `smooth` | sets the sweep path in world coordinates; corners are bent with radius `r`, or `smooth` makes one curve through the points |
| `helix` | `r= pitch= turns=` `at=x,y` | sets a helical sweep path about the workplane normal |
| `sweep` | `mode=` `twist=` `scale=` | carries the sketch from the path start along the path, turning it `twist` degrees and sizing it to `scale` by the end |
| `section` | | stores the sketch as one cross-section for `loft` |
| `loft` | `smooth` `mode=` | joins the stored sections with ruled faces, or one smooth surface through them all with `smooth`; sections with different edge counts are matched up by splitting edges |
| `hole` | `d x,y ...` `depth=` `cbore=d,depth` `csink=d,angle` `thread=M4` `fit=` `on=label.side` | drills holes at each point, through unless `depth` is given, with an optional counterbore or countersink; `thread` checks the drill suits the tap and records it; with `on=` the points are angle,height on the side of an extruded circle and the holes go in square to it. `d` can be a screw size, `M2` to `M12`: `hole M3 x,y` is its clearance hole (ISO 273, `fit=normal` unless `fit=close` or `fit=loose`), `fit=tap` its tap drill, threaded like `thread=`, and `fit=insert` the hole for a heat-set insert, blind to the insert's length plus 1 unless `depth` says. `cbore=M3` is the counterbore for an ISO 4762 socket head (DIN 974 diameter, head height plus 0.4 deep) and `csink=M3` the 90 degree countersink for an ISO 10642 flat head, 0.2 wider than the head. For a plain diameter, `fit=H7` (H, G, F, E, D or JS, grades 5 to 11) records an ISO 286 fit: `check` prints its limits and drawings add it to the callout. Holes are always modelled at the nominal size |
| `thread` | `size` `on=faces` `pitch=` `left` | cuts an ISO metric thread (`M2` to `M12`) into a round face: a rod gets an outside thread with its crest on the rod, a hole an inside one with its crest on the drill; `pitch` overrides the coarse pitch and `left` makes it left-hand. Each end of the face must be a flat face square to it or a chamfer (or countersink) that goes past the thread's root, which the thread runs out into. Chamfer before threading; a rounded end, or a chamfer after, fails |
| `shell` | `t` `open=faces,...` | hollows an extrusion (open `label.end`, `label.start` or both) or a full revolve (open its flat `label.caps`) to walls `t` thick; where `fillet` rounded the extrusion's straight edges, the inside is rounded to that radius less `t`, so the walls stay `t` thick |
| `draft` | `angle faces` `neutral=face` | tilts flat side faces inward by `angle` degrees, hinged where they meet the neutral face; `label.side` of an extrusion that is the whole solid tapers curved sides too |
| `push` | `faces d` | moves flat faces `d` along their normal, out (positive) or in (negative); an extrusion's `label.end` or `label.start` moves in even with curved sides |
| `rib` | `t` | after an open `pen` path of one `line`, fills between that line and the solid with a web `t` thick, centred on the workplane |
| `wrap` | `faces depth=` `raise` | rolls the sketch onto a round face and cuts it `depth` in, or with `raise` stands it `depth` out. Draw the sketch on a plane along the cylinder's axis: its x runs around the cylinder and its y along the axis, read from the side the plane faces |
| `thicken` | `faces t` | grows every side of an extrusion (`label.side`) outward by `t`, or pushes flat faces out by `t` |
| `fillet` | `size edges` `to=` | rounds the edges with radius `size`, or from `size` to `to` along them (on a closed loop, up to `to` halfway round and back) |
| `fillet` | `full label.end` | rounds the end of an extruded rect into a half cylinder across its short side, while the extrusion is the whole solid |
| `chamfer` | `size edges` `d2=` | bevels the edges by `size`; with `d2`, the edges must be written `a&b` and `size` is cut along `a`, `d2` along `b` |

Enclosures:

| operation | parameters | does |
|---|---|---|
| `boss` | `d h x,y ...` `hole=` `fit=` `depth=` `ribs=` `rib=` `angle=` | stands posts `d` across and `h` tall on the workplane at each point, drilled from the top with `hole` (a diameter or a screw size, with `fit=` as for `hole`) down to the workplane, or `depth` deep (an insert hole is the insert's length plus 1). `ribs` adds that many triangular gussets `rib` thick (`d/5` unless given) around each post, reaching `d/2` out and three quarters of the way up, the first at `angle` degrees from x. Groups: `side`, `end`, `hole`, `ribs` |
| `lip` | `faces h` `w=` | on the flat rim of a hollow part (a face with one outline and one hole, such as the top of a shelled box), lowers the outer part of the rim by `h`, leaving a lip `w` wide (half the wall unless given) standing at the inner edge. Group: `step` |
| `groove` | `faces h` `w=` `gap=` | the matching rebate on the other part's rim: cuts the inner `w+gap` of the rim `h+gap` deep (`gap` is 0.2 unless given), so a lid shelled like the case and placed `h` lower than the case's rim drops onto its `lip` |
| `vent` | `l w x,y ...` `count=` `step=x,y` `angle=` `depth=` | cuts slots `l` long and `w` wide at each point, `count` of them `step` apart, through the wall under the workplane and no further, unless `depth` is given |
| `snap` | `l t w x,y ...` `hook=` `angle=` `dir=` | stands a cantilever snap hook on the workplane at each point: an arm `l` tall, `t` thick and `w` wide, with a catch `hook` deep (`t` unless given) at its tip, square underneath and with an `angle`-degree (30 unless given) lead-in above. The catch points `dir` degrees from the workplane's x. Group: `faces` |

`examples/assemblies/case.gasm` puts these together: a case with a lip, bossed for heat-set
inserts, a lid with the groove sitting on it, and four flat head screws from the library.
Make the `lip` before anything rises above the rim, since it takes the face the rim is on.

Sheet metal:

| operation | parameters | does |
|---|---|---|
| `sheet` | `t` `r=` `k=` | makes this part sheet metal `t` thick, bent to an inside radius `r` (`t` unless given) with K-factor `k` (0.44 unless given), so a bend of `a` degrees takes `a*pi/180*(r+k*t)` of flat sheet |
| `tab` | | turns the sketch into the flat base plate, `t` thick on the workplane. `flange` needs it drawn as one rect without rounded corners or one poly; a blank folded with `bend` can be any outline with holes in it, like a laser-cut part. Groups as for `extrude`: `start`, `end`, `side` |
| `flange` | `edges l` `angle=` `inset=` | bends a flange off each straight outside edge of a plate's top or bottom face: it rises on the side of the face the edge is on, turns `angle` degrees (90 unless given, up to 180 for a hem) about an inside radius `r`, and runs on flat for `l`. `inset=d` stops it `d` short of both ends of the edge, `inset=a,b` short of the start and end, going round the plate counterclockwise seen from its top; one edge can take several inset flanges that do not overlap. A 90 degree flange off the top reaches `t+r+l` above the plate's bottom. Groups: `face` (the side that carries on from the face the edge was on), `back`, `end` (the free end), `side`, `bend` |
| `bend` | `a,b c,d` `angle=` | folds the flat blank along the line through two points on the tab's workplane, by `angle` degrees (90 unless given, up to 180): positive folds toward the tab's `end` face, negative away. The side of the line with the sketch origin stays put, so the origin goes on the part that does not move. The blank must cross the line on straight edges square to it, with no holes in the strip that bends; bends must not cross. Labels made on the blank, holes and cuts included, carry over to the folded part. Groups: `bend` (the curved faces) |
| `dart` | `bend at ...` `l=` `w=` | presses a stiffening dart into the bend named `bend` at each position `at` along its line (0 unless given), measured on the tab's workplane like the bend's points: inside, a ridge runs across the corner from `l` along one side to `l` along the other (`3*(t+r)` unless given), `w` wide where it meets the bend (`2*t+4` unless given) and tapering to nothing at the ends of its legs; outside, a matching dent `t` deeper, so the walls stay `t` thick and the volume barely changes. Keep darts clear of holes. Bends up to 120 degrees |
| `unfold` | | replaces the part with its flat pattern, on the base plate's workplane, with the holes and through cuts made in the plates. A `bend` part unfolds to its blank |

A part is flanged or bent from a blank, not both. Draw a laser-cut part as its blank and fold
it: the flat pattern is the blank itself, holes and cuts made after the first `bend` are not in
it, and `unfold` counts them.

```
sheet 3 r=3
plane XY offset=95
slot 350 50 at=35,0
circle 12 at=-140,0
circle 12 at=210,0
blank: tab
plane blank.end
hole 6.5 -10,15.7 -10,-15.7 80,15.7 80,-15.7
left: bend -19.9,-1 -19.9,1 angle=-90
right: bend 90.9,-1 90.9,1 angle=-90
dart left -7 7 l=20 w=8
dart right -7 7 l=20 w=8
```

Flanges on edges that meet at a corner leave the corner open by `r+t`. Make holes and cuts in the plates, through the sheet, for them to
show in the flat pattern; `unfold` counts the ones it could not place. Its flat pattern is
the part's DXF: `gcad export part.gcad flat.dxf` writes the outline and holes on layer
`OUTLINE` and the middle of every bend on layer `BEND`.

```
sheet 1.5 r=1.5
rect 60 40
base: tab
plane base.end
hole M4 -20,0 20,0
walls: flange base.end&<Y 15
lip: flange walls.face&walls.end 6
```

Weldments:

| operation | parameters | does |
|---|---|---|
| `member` | `kind size x,y,z x,y,z ...` `closed` `up=x,y,z` `rotate=` `offset=u,v` `material=name` `count=` `step=x,y,z` `end_step=x,y,z` `when=expr` `angle=` `axis=name` | adds one straight structural member per segment of the path through the points, mitred where two meet; `closed` joins the last point back to the first. Kinds: `tube WxHxT` and `bar WxH` (centred), `pipe DxT` and `rod D` (centred), `angle WxHxT` (its outer corner on the path, legs along the profile's x and y) and `channel WxHxT` (the back of its web on the path, flanges up y). The profile's y points along `up`, or square to the path's plane when it is flat, or as near +Z as it can; `rotate` turns it about the path and `offset` moves it in its own x and y. Each member is its own body, named `label_1`, `label_2`, ... (just `label` for one segment). Groups: `side`, `end` |

Members are listed in the bill of materials as a cut list, by profile, length to the long
point and end cuts, like `tube 40x40x3  540.0 long, cut 45.0/45.0`, and `check` does not
count them as separate parts. `material=` assigns every generated member, with the same
optional property overrides as `material` (`density=`, `E=`, `nu=`, `yield=`, `k=`, `alpha=`).
Add `count=N step=x,y,z` to lay N members in a row, each moved `step` from the last, in
one line instead of N. Instance `k` (0 to N-1) is named `label_<k+1>`. `end_step=x,y,z` moves
the last point by its own step, so studs under a sloping roof keep their base level while
their tops follow the slope (two-point members only). `when=expr` skips the instances where
`expr` is 0, with `k` bound to the instance index, so a row can leave out an opening:
`member bar 44x150 x0,y,zp x0,y,ztop count=13 step=600,0,0 end_step=600,0,-15 when=(x0+600*k<ox0)+(x0+600*k>ox1)`.
For a circular array use `count=N angle=360 axis=hub`, where `hub` is a datum axis and
`angle` is the total sweep. Every instance keeps the material, finish and cut-list entry (a
row of equal lengths is one cut-list line). A multi-segment member repeats the whole path.
`pattern name ...` also works in part files for an existing body or member.

`mode=cut` and `mode=intersect` on `revolve`, `sweep` and `loft` remove the shape or
keep only what it shares with the solid.

Bodies:

| operation | parameters | does |
|---|---|---|
| `mirror` | `on` `offset=` `of=label` | adds the mirror image of the solid, or of one labelled feature, across a named plane or flat face |
| `repeat` | `label` `count=` and `step=x,y`, `angle=` or `along=path` | repeats what a labelled line added or removed, in a row along the workplane, around its normal, or spaced along the path |
| `move` | `x,y,z` `copy` | moves the solid, or adds a moved copy |
| `rotate` | `angle` `axis=x\|y\|z\|name` `about=x,y,z` `copy` | turns the solid, or adds a turned copy |
| `scale` | `factor` or `x,y,z` `about=x,y,z` | scales the solid, evenly or per axis |
| `split` | `on` `offset=` `keep=below\|above` | cuts the solid with a plane and keeps one side |
| `body` | `name` | sets the current solid aside and starts a new one: another part sharing this file's sizes, which an assembly brings in as `part.name`, or a piece to `combine` into one part; the first body is `main` |
| `combine` | `into from` `mode=add\|cut\|intersect` | joins body `from` into body `into`, or cuts it out, and drops `from` |
| `place` | `body` `on=faces` | moves the body so its lowest point sits on the faces |
| `import` | `file.step` or `file.stl`, `solid=` | adds the solid in a STEP file, or a watertight STL mesh with its flat regions merged into faces, relative to the `.gcad` file. A STEP file with several solids needs `solid=n` to pick one; the error lists them with their names and sizes. A STEP assembly's solids come where the file places them, a part used six times being six solids. Each solid is read in its own representation's length unit, so an inch vendor model inside a millimetre board comes in at its true size. Faceted breps and the surface models KiCad writes for small parts are built from their polygons, one solid per connected piece; a solid gcad still cannot read becomes a box of its extent, and the line says so and why. Void shells that lie outside their solid, which some exporters write for separate bodies, come in as solids of their own; a real cavity gcad cannot read is left solid, and the line counts them |
| `color` | name, `#rrggbb` or `r,g,b`, `opacity=` | colours the current body in STEP and 3MF files; `opacity` below 1 (0.3 for glass) makes the viewer draw it see-through |
| `material` | `name` `density=` `E=` `nu=` `yield=` `k=` `alpha=` `cp=` | sets what the current body is made of, for its mass and for simulations. Names come from the standard library of about 210 complete metals, plastics, printing materials, rubbers, timbers, boards, masonry, insulation, glass and composites (see Material libraries; `gcad materials` lists them). `density=` in g/cm³ gives any other; the rest set stiffness in GPa, Poisson's ratio, yield strength in MPa, conductivity in W/m·K, expansion in µm/m·K and specific heat in J/kg·K. A name from a material library (see Material libraries) is used before a built-in one of the same name |
| `materials` | `file` | loads a material library: a `.toml` in gcad's own layout, a MatML `.xml` (Ansys Engineering Data, Granta) or a FreeCAD `.FCMat`. Its materials take names in lower case with dashes (`Structural Steel` is `structural-steel`) and come before any found automatically |

Measuring, which changes nothing and prints the answer:

| operation | parameters | prints |
|---|---|---|
| `measure` | `faces faces` | the distance between two sets of faces |
| `measure` | `angle faces faces` | the angle between two face normals, two axes, or an axis and a face; `faces` is flat faces facing one way or one round face |
| `measure` | `mass` | volume, surface area and centroid of every body, and the mass in grams of those with a `material` |
| `measure` | `overlap a b` | the volume two bodies share |
| `measure` | `thickness` | the thinnest wall and where it is |
| `measure` | `draft` `pull=x\|y\|z` `min=` | how many faces have under `min` degrees (1 by default) of draft |
| `measure` | `box` `body\|faces ...` | the size and bounding box of each body named, or of the faces a selector matches; with nothing named, of every body and of them all |
| `measure` | `clearance a b` | the nearest gap between two bodies and the points it runs between, or the volume they share if they overlap, and the gap between their bounding boxes along x, y and z (negative where they overlap) |
| `measure` | `space` `at=x,y,z` | the largest empty box around a point that grows evenly until something stops it on every side, and what stops each side (`open` where nothing does), then how far the point is from the first body along each axis. `at` is the middle of all the bodies unless given, and must not be inside one |

`gcad measure file what ...` runs a file and then one `measure` line, without adding it: `gcad measure case.gasm space at=0,0,10`.

Profiles drawn inside another profile in the same sketch become holes in it.
A sketch for `sweep` is drawn around the workplane origin; `sweep` moves it to the first
path point (or the start of the helix) and turns it to face along the path.


## Material libraries

A file's materials come from, in order: `materials file` lines, a `materials.toml` beside the
file or in any folder above it up to the repository root (nearer files first), then
`~/.config/gcad/materials.toml`, then the standard library. `gcad materials [file or folder]` prints
what a file can use and where each came from. A library is checked into the project, so a
material gives the same result on any machine, and a changed library makes simulation results
stale.

`materials.toml` has one table per material, in the engineering-constants names every solver
shares. Axis 3 is the print direction for a printed part.

```toml
[petg-cf]
density = 1.30
E = 3.8
nu = 0.36
yield = 52
k = 0.25
alpha = 50
printed = true

[petg-print]
density = 1.27
E1 = 2.1
E2 = 2.1
E3 = 1.5
nu12 = 0.38
nu13 = 0.35
nu23 = 0.35
G12 = 0.76
G13 = 0.55
G23 = 0.55
Xt = 47
Yt = 47
Zt = 22
source = "own tensile bars, 0.2 mm layers, 100% infill"
```

| key | unit | meaning |
|---|---|---|
| `density` | g/cm³ | |
| `E`, `nu`, `yield`, `k`, `alpha`, `cp` | GPa, -, MPa, W/m·K, µm/m·K, J/kg·K | isotropic values; `cp` is the specific heat |
| `E1` `E2` `E3`, `G12` `G13` `G23` | GPa | Young's and shear moduli along and between the axes |
| `nu12` `nu13` `nu23` | - | Poisson's ratios |
| `Xt` `Yt` `Zt`, `Xc` `Yc` `Zc`, `S12` `S13` `S23` | MPa | tensile, compressive and shear strengths |
| `k1` `k2` `k3`, `alpha1` `alpha2` `alpha3` | W/m·K, µm/m·K | conductivity and expansion along the axes |
| `printed` | true or false | the readings say the values are for printed parts |
| `source` | text | where the numbers came from |
| `color` | `"#rrggbb"`, a colour name or `[r, g, b]` | the colour of a body made of it, unless the body has its own `color` line |
| `roughness` | 0 to 1 | 0 is a mirror, 1 is chalk |
| `metallic` | 0 to 1 | 1 is bare metal, 0 is paint, plastic, wood and stone |
| `specular` | 0 to 2 | reflection of a non-metal; 0.5 is the usual 4% |
| `clearcoat`, `clearcoat_roughness` | 0 to 1 | a lacquer layer over the colour, and how rough it is |
| `transmission`, `ior` | 0 to 1, 1 to 3 | how much light passes through (1 for glass) and how it bends (1.5 for glass) |
| `emission` | 0 and up | glows in its own colour, this bright |
| `texture` | a pattern name | a built-in tiling surface: `wood`, `osb`, `concrete`, `stone`, `brick`, `sand`, `fabric`, `foam`, `rubber`, `brushed`, `carbon-fibre`, `polished`, `cast`, `plaster`, `granulate`, `marble`, `asphalt` |
| `texture_scale` | mm | the size of one tile on the part, 100 unless given |
| `texture_rotation` | degrees | turns the texture on every face; 0 runs the grain along the part's longest side |
| `color_map`, `normal_map`, `roughness_map`, `metallic_map` | image file | PNG, JPEG or WebP maps beside the library, used instead of the pattern; the roughness and metallic maps are read from their red channel |

The look keys only change the viewer: it uses them when ray tracing, and the plain view shows
the roughness, metal, see-through glass and textures. Textures are projected from whichever
side a face mostly faces, in the part's own frame, so a pattern or member copy keeps its grain.

The standard library has about 210 materials (aliases included), and every one is complete:
density, Young's modulus, Poisson's ratio, strength, conductivity, expansion, specific heat
(`cp`), a colour, roughness, metallic and a texture. `strength` is the yield strength, or the
working compressive or tensile strength of a material that does not yield (concrete, brick,
timber in bending, foam, soil, and the tensile strength of rubber). A test fails if a standard material lacks any of them, so an
incomplete entry is a bug, not a gap. It covers steels by grade (`steel-s355`, `stainless-316`,
`rebar-b500b`), aluminium alloys, copper alloys, titanium and other metals; engineering plastics
and rubbers (`pom`, `peek`, `hdpe`, `epdm`, `pvc-flexible`); 3D printing filaments, resins and
printed metals, all flagged `printed` (`pla`, `pla-cf`, `petg-cf`, `pa-cf`, `pei`, `tpu`,
`resin-tough`, `ss316l-printed`, `ti64-printed`); timbers and boards (`oak`, `c24`, `glulam-gl24h`,
`lvl`, `clt`, `plywood`, `osb`, `mdf`); concrete, blocks, screed, render, brick, tile, stone and
ground (`concrete-c30-37`, `aac`, `screed`, `cement-render`, `clay-tile`, `granite`, `soil`);
board and insulation (`plasterboard`, `fibre-cement-board`, `pir`, `eps`, `xps`, `rockwool`,
`foamglass`, `sheep-wool`); glass and composites; and common names for them (`aluminum`,
`acetal`, `drywall`, `mineral-wool`, `float-glass`, `stucco`). Values are typical room
temperature figures, and a printed material's are along the print plane: use a supplier's
datasheet, or `E1 E2 E3` and `Xt Yt Zt` for the print direction, in a project library for
design work. Materials with no meaningful strength or stiffness, such as air and water, are
left out.

A project table named like a standard material keeps every key it does not give from the
standard one, so a project can give `steel` a paint finish and keep its weight and strength:

```toml
[steel]
color = "#3a3f44"
roughness = 0.45
clearcoat = 0.3

[oak]
texture_scale = 300

[decking]
density = 0.55
k = 0.13
color_map = "textures/decking.jpg"
normal_map = "textures/decking-normal.png"

[float-glass]
density = 2.5
color = "#e8f2ef"
transmission = 1
ior = 1.52
roughness = 0.02
```

The solvers are isotropic. A material given along three axes and no isotropic value is
simulated with its weakest: the smallest modulus, the smallest tensile or compressive
strength, `nu12`, the smallest conductivity and the largest expansion, and the simulation
notes say which part that applies to and the values used. Giving `E=` or `yield=` as well
overrides that choice.

MatML files are read the way Ansys Workbench writes them (`Young's Modulus X direction`,
`Poisson's Ratio XY`, `Stress Limits` with `Tensile X direction` and so on), converting from the
units each parameter declares; a property given against temperature takes the value nearest
22 °C. FreeCAD cards are read in the YAML layout of FreeCAD 1.0 (`LinearElastic`,
`OrthotropicLinearElastic`, `Thermal`) and the older INI layout, with quantities like
`1240 kg/m^3` and `0.000068 m/m/K`. Neither writes yield strength for every material; where it
is missing the tensile strength is used.

## Meshes

A body can be a mesh instead of a solid: triangles, for organic shapes such as
characters, creatures and game assets. A mesh is never written as triangles. You block
it out of primitives that blend into each other, turn that into a mesh with `remesh`,
then sculpt it by selecting a region and changing it, one line per change, the way a
solid is built feature by feature. A mesh of 100 000 triangles takes the same lines as
one of 1000.

```
torso: blob 0,0,20 r=7,5,8
head: blob 0,-2,33 r=5 blend=2
ear_l: blob 3.5,-1,38 r=1.2,0.8,3 ry=20 blend=0.8
mirror x of=ear_l
arm_l: limb 7,0,26 12,-1,19 14,-2,12 r=2,1.6,1.3 blend=1.5
mirror x of=arm_l
remesh 0.3
select head falloff=2
scale 1.1
eyes: select sphere=2,-6,35 r=1.2 mirror=x falloff=0.6
inflate -0.5
```

Blocking out, before `remesh`:

| operation | parameters | does |
|---|---|---|
| `blob` | `x,y,z` `r=` `blend=` `mode=add\|cut\|intersect` `rx=` `ry=` `rz=` | an ellipsoid centred on the point, of radius `r` or radii `rx,ry,rz` along x, y and z, turned by `rx`, `ry`, `rz` degrees about x, then y, then z. `blend` is how far its surface melts into what is already there, as a smooth fillet of about that size (0, a sharp join, unless given). `mode=cut` carves it out and `mode=intersect` keeps only what it shares |
| `limb` | `x,y,z x,y,z ...` `r=` `blend=` `mode=` | a tube through two or more joints with rounded ends, its radius `r` at every joint or one radius per joint (`r=2,1.6,1.3`), tapering smoothly between them. Arms, legs, tails, horns, necks |
| `mirror` | `x\|y\|z` `of=label` `offset=` | adds the mirror image of one labelled blob or limb (or of every one, without `of`) across the plane through `offset` (0) square to the axis. A part named `..._l` comes back `..._r` and the other way round |
| `remesh` | `cell` | turns the blockout into a closed mesh with triangles about `cell` across, a 120th of its size unless given. Each vertex belongs to the part (the label of the blob or limb) nearest it, so later lines can select `head` or `arm_l`. A sculpting line before any `remesh` remeshes at the default cell first |

Sculpting, after `remesh` or `import`:

| operation | parameters | does |
|---|---|---|
| `select` | part names, `all`, a label of an earlier `select`, `sphere=x,y,z r=`, `box=x0,y0,z0,x1,y1,z1`, `falloff=`, `mirror=x\|y\|z` | picks the vertices the next lines change. Several names and shapes add up (`select ear_l,ear_r`). `falloff` fades the change out smoothly to nothing over that distance beyond the selection, measured over the surface for parts and straight for shapes; without it the edge is sharp. `mirror=x` also picks the mirror image across x=0 and changes it the mirrored way, so one line edits both sides. Every vertex outside the selection and its falloff stays exactly where it is. Until the first `select` every line changes the whole mesh |
| `move` | `x,y,z` | moves the selection |
| `scale` | `factor` or `x,y,z` `about=x,y,z` | scales the selection about `about`, or about the selection's centre (the centre of a `sphere=`, else the middle of the vertices picked) |
| `rotate` | `angle` `axis=x\|y\|z\|name` `about=x,y,z` | turns the selection about the axis through `about`, the selection's centre unless given; right-handed, so `rotate 30 axis=z` turns +x toward +y |
| `inflate` | `by` | pushes the selection out along the surface by `by` (negative pulls it in): fatter limbs, a puffed chest, eye sockets |
| `smooth` | `passes` | relaxes the selection's surface, 1 pass unless given; 5 to 20 takes out lumps |
| `refine` | `edge` | splits the selection's triangles until no edge in it is longer than `edge`, placing new vertices on the curve of the surface rather than on the flat triangles. The rest of the mesh keeps its triangles and stays joined. Detail smaller than about two edges cannot be sculpted, so refine where the detail goes (a face, hands) before creasing or adding noise there |
| `crease` | `x,y,z x,y,z` `depth=` `width=` | cuts a groove along the line between the two points, `depth` deep (0.3, negative raises a ridge) and `width` wide (0.5) on each side, rounded at the bottom: mouths, eyelids, fingers, folds |
| `pinch` | `strength` `toward=x,y,z` | draws the selection along the surface toward a point (the selection's centre unless given), `strength` 1 all the way and negative spreading it out: sharper tips, tighter creases |
| `flatten` | `at=x,y,z normal=` | presses the selection onto a plane, the one through `at` square to `normal`, or one fitted to the selection when neither is given: soles, cut faces, flat backs |
| `noise` | `amplitude` `size=` `seed=` | pushes the selection in and out along the surface by up to about `amplitude`, in lumps about `size` across (1), the same for the same `seed`: fur, bark, rock, skin |
| `relax` | `passes` | evens out the triangles of the selection by sliding vertices along the surface, so the shape stays where `smooth` would shrink it |
| `decimate` | `triangles` or `share%` | merges triangles in the selection until the whole mesh has `triangles` of them (or keeps that share of the selection's), choosing the merges that change the shape least and never tearing the surface. Parts keep their names. The last step for a game asset: `select all` then `decimate 5000` |
| `import` | `file.obj mesh` or `file.stl mesh` | starts the body from a mesh file; an OBJ's `o` and `g` groups become its parts |
| `color` | as for solids | |

The mesh starts as fine as `remesh` made it, the same everywhere. Make detail where it goes
rather than everywhere: a coarse `remesh`, then `select` the face and `refine` it before
creasing it, and `decimate` at the end to the triangle budget of the game.

Each sculpting line reports how many vertices it moved, the most any one moved, and the
new volume, so a line that missed its target shows up as `moved 0 vertices`. A body is a
mesh or a solid: solid operations (`extrude`, `fillet`, `hole`, ...) refuse a mesh body,
and `body name` starts a new body of either kind in the same file. `gcad export` writes a
file with mesh bodies as `.obj` (each part its own `g` group), `.stl` or `.3mf`; solid
bodies in it are tessellated alongside. Assemblies take solids only.

Positions, sizes and the selection are in the same millimetres as everything else, so a
game asset is best modelled at a known scale, 1 unit to 1 cm for instance, and scaled on
import.

## Assemblies

A design with more than one part is a `.gasm` file, an assembly: it brings the parts
in from part files, places them and joins them, and makes no geometry of its own. A
`.gcad` part file holds one part, or several parts as bodies when they share sizes.
Its lines have the same shape and use `let`, `if` and `include` the same way.

| operation | parameters | does |
|---|---|---|
| `part` | `name file.gcad` `body=` `variable=value ...` | runs a part file with those variables fixed and brings in its bodies; one body is called `name`, several are `name.body`; `body=` takes just that one. A `.gasm` file brings in its parts, joints and axes too, as a sub-assembly: `az.spindle`, `pose az.spin`, `about=az.wa`. A `.step` file brings in each solid where the file places it, named after its product (`box`, `pin`, `pin_2`, ...), and the bill of materials counts repeats of one product together. Units and unreadable solids are handled as for `import`; a KiCad or agentee board export comes in whole, its components in place |
| `part` | `name kind:size` | brings in a standard part, see below |
| `move` | `part x,y,z` | moves a part and everything jointed to it; given a sub-assembly's name (`az` for `part az drive.gasm`) or a multi-body part's, moves all of its parts and its axes |
| `rotate` | `part angle` `axis=x\|y\|z\|name` `about=x,y,z` | turns a part and everything jointed to it, or a whole sub-assembly as for `move` |
| `axis` | `name x,y,z x,y,z` | names a line for joints and `rotate` |
| `joint` | `name child parent turn about=axis` or `slide along=x,y,z`, `min=` `max=` `at=` `drag=` | joins two parts; a `turn` joint spins the child about a datum `axis` by degrees, a `slide` joint moves it along a direction by millimetres; `at` poses it now; `drag` is the friction that resists it, in N·m (N on a slide): bearings and seals, which `sim torque` adds to the torque to drive it |
| `pose` | `joint value` | moves a joint to a new value |
| `couple` | `driven driver` `ratio=` `efficiency=` `back=`, or `lead=` `friction=`; `module=` `face=` `teeth=` `pressure=` `gear=` `pinion=` | makes one joint follow another: `driven` moves `ratio` of its units (degrees or millimetres) for each unit `driver` moves, from where both are now. Gears turn at `ratio=-teeth_a/teeth_b`; a rack slides `ratio=pi*d/360` mm per degree of its pinion. A driven joint cannot be posed itself. `efficiency` is the share of power that gets through when `driven` turns `driver` (a motor driving its load), `back` when `driver` turns `driven` (the load turning the motor), the same unless given, and 0 for a pair that locks. For a worm give its `lead` angle in degrees and the mesh `friction` instead (about 0.05 to 0.15 greased): efficiency is tan(lead)/tan(lead+atan(friction)), back tan(lead-atan(friction))/tan(lead), and a worm whose friction is over tan(lead) locks. `module` (mm) and `face` (the face width, mm) size the teeth so `sim torque` reads the mesh forces and stresses: `teeth` is the count on the `driven` side (a worm's starts, 1 unless given), the `driver` gear has `ratio` times as many, `pressure` is the pressure angle (20 unless given), and `gear` and `pinion` name the `driver` and `driven` gear bodies so their materials are used (a pinion or worm not named is taken as steel) |
| `concentric` | `moving:faces fixed:faces` `near=x,y,z` `flip` | turns and moves the moving part so its round faces share an axis with a hole in the fixed one; `near` picks the hole nearest a point, otherwise the one nearest where the part is; it can still slide along and spin about that axis |
| `flush` | `moving:faces fixed:faces` `offset=` | turns and moves the moving part so its flat faces lie on the fixed part's, facing each other, `offset` apart |
| `parallel` | `moving:faces fixed:faces` | turns the moving part so its face or axis is parallel to the fixed one; an axis and a face are parallel when the axis lies along the face |
| `angle` | `moving:faces fixed:faces degrees` | turns the moving part so its face or axis is at that angle to the fixed one; between two faces it is the angle between their outward normals, so 0 faces the same way and 180 faces each other |
| `distance` | `moving:faces fixed:faces length` | makes two faces, two axes, or an axis and a face parallel and `length` apart, keeping the moving part on the side it is on |
| `tangent` | `moving:faces fixed:faces` | lays a round face on a flat one, or against another round face, with its axis parallel |
| `aligned` | `a:faces b:faces` `tol=` | checks that every hole in `a` has a parallel hole in `b` on the same axis, within `tol` (0.05 by default), and fails the line if not |
| `pattern` | `part holes=part:faces`, or `count=` with `step=x,y,z` or `angle=` `axis=` | copies a part. `holes=` puts a copy in each other hole of a set the part is `concentric` with, and gives each copy the part's mates; `step=` and `angle=` space `count` copies out from the part. Each copy also gets the part's joints, named `joint_2` and so on, with their couples, so the planets of a planetary gear are one part and a pattern. Copies are called `part_2`, `part_3` and so on |
| `explode` | `part x,y,z` | moves a part and the parts held by it by that much in exploded views only: `render --explode` and the viewer's explode slider. It does not change where the part is for mates, joints, interference or export |
| `color` | `part colour` `opacity=` | colours a part, over the colour and opacity its file gave it |
| `material` | `part name` `density=` | sets a part's material, over the one its file gave it |
| `materials` | `file` | loads a material library, as in a part file |
| `interference` | `none`, `joint=` `steps=` | reports the volume each pair of parts shares; `none` fails the line if any do; `joint=` checks across the joint's range in `steps`, and with `none` fails at any pose with an overlap. Steps that are whole tooth pitches never catch a gear pair out of phase, so pick a count that is not. The gear and pinion of a `couple` with `module=` may share up to `module`³ where their teeth meet |
| `measure` | `mass`, `overlap a b`, `box`, `clearance a b`, `space at=` | as in part files, across every part where it is placed |

Standard parts are built in, M2 to M12, and come in steel (inserts and standoffs in brass). A screw's
axis is z, with the face its head bears on at z=0 and the shank going down, so
`concentric s:shank.side plate:holes.side` and `flush s:head.start plate:holes.cbore_floor`
seat it. A flat head's top is at z=0 instead and faces up, so
`distance s:head.start plate:top.end 0` sets it level with the surface.
Nuts and washers sit on z=0 and go up, inserts hang down from z=0.

| `kind:size` | part | faces |
|---|---|---|
| `shcs:M3x10` | ISO 4762 socket head cap screw, 10 long under the head | `head`, `socket`, `shank` |
| `fhcs:M3x10` | ISO 10642 countersunk socket screw, 10 long overall | `head`, `socket`, `shank` |
| `hex:M6x20` | ISO 4017 hex head bolt | `head`, `shank` |
| `nut:M3` | ISO 4032 hex nut | `nut`, `bore` |
| `washer:M3` | ISO 7089 plain washer | `washer`, `bore` |
| `insert:M3` | heat-set insert (M2 to M6, Ruthex sizes) | `insert`, `bore` |
| `standoff:M3x10` | hex standoff, tapped through, standing on z=0 (brass) | `body`, `bore` |
| `bearing:608` | deep groove ball bearing, solid with shallow seal grooves, standing on z=0: 623 to 626, 695, 688, 698, 608, 609, 6000 to 6005, 6200 to 6204, 6800 to 6804 | `ring` (outer race), `bore`, `seal` |
| `dowel:4x16` | ISO 8734 dowel pin, chamfered, standing on z=0 | `pin` |

Screws are drawn plain, without a modelled thread. The bill of materials lists them by
standard and size, like `ISO 4762 M3x10`.

`faces` in `parallel`, `angle`, `distance` and `tangent` is either flat faces on one
plane or the round faces of one cylinder, which stands for its axis.

`examples/robot/arm.gasm` puts these together. Each joint is a planetary reducer: the
motor is mated to the housing, its sun turns on it with a `turn` joint coupled to the
arm joint at `1+ring/sun`, and one planet turns on the output link coupled at
`-ring/planet`; `pattern planet count=3 angle=360 axis=...` adds the other two with
their joints and couples. The gripper's fingers are racks on one pinion, so one `slide`
drives both. The arm's own variables pose it:
`gcad check examples/robot/arm.gasm --set shoulder=30 --set jaw=6`.

A part can have several joints to the same parent, and their motions add up. They
must not depend on the order they are applied in, so they are slides in any
direction, or turns about one axis together with slides along it. A turn and a slide
on the same axis make a cylindrical joint:

```
axis shaft 0,0,0 0,0,1
joint spin rod frame turn about=shaft
joint push rod frame slide along=0,0,1 min=0 max=20
```

A `turn` joint without `min=` or `max=` turns all the way round: posing it to 270
leaves it at -90.

When a mate joins two parts that already hang off different joints, it closes a
loop, like the coupler and rocker of a four-bar linkage. Moving one joint in the loop
moves the others with it so the mate keeps holding, if they can; otherwise the move
fails as before. `pose` says which joints followed. See
`examples/assemblies/linkage.gasm`.

A mate holds the parts together, the way a joint does. The first mate of a part
places it and fixes it to the other part, so it moves when that part moves. Later
mates of the same part to other parts do not move it: they must already hold, and
from then on they keep holding. A joint that would pull a mate apart cannot move:
`pose` fails and the viewer locks its slider. A screw mated to a lid's hole and a
box's pilot hole keeps a hinged lid shut. `aligned` only checks holes; it holds
nothing.

`part:faces` names faces on one part with the selectors of its own file, so
`case.main:pilot.side` is the side of the `pilot` holes in the part `case.main`. They
follow the part as it moves.

```
let open=0 screwed=1 bx=32 by=17 h=30
part case ../parts/enclosure.gcad
axis hinge -40,25,30 40,25,30
joint lid case.lid case.main turn about=hinge min=-110 max=0 at=open
aligned case.lid:screws.side case.main:pilot.side
if screwed part s1 ../parts/screw.gcad
if screwed concentric s1:shank.side case.lid:screws.side near=bx,by,h
if screwed flush s1:head.start case.lid:top.end
if screwed concentric s1:shank.side case.main:pilot.side
if screwed explode s1 0,0,25
if screwed pattern s1 holes=case.lid:screws.side
pose lid open
explode case.lid 0,0,30
interference none
```

`gcad bom file` lists what a file is made of: one row per body of each part file
and set of variables, with how many there are, the material, and the volume and mass
of each. `--csv` writes the same as CSV, with the part names in the last column.

A `.step` export of an assembly keeps its structure: each part file and set of
variables becomes one product, and each part placed in the assembly is an instance of
it with its own name and position, so four screws are one screw used four times.

An `.svg` export of an assembly draws it exploded, if it has `explode` lines, with a
parts list (item, quantity, part, material, mass) and a numbered balloon on one of
each item in the iso view.

`check`, `render`, `export` and the viewer take either kind of file, and `--set`
reaches the assembly's own variables. The viewer rebuilds when any part file next to
the assembly is saved.

## Drawings

A `.gdwg` file lays out construction or workshop drawings of a part or assembly on sheets,
one operation per line, with `let` and `if` as usual. `gcad check x.gdwg` runs it and says
what each line drew; `gcad export x.gdwg x.pdf` writes every sheet as a page of one PDF, and
`x.svg` writes `x-1.svg`, `x-2.svg` and so on (or `x.svg` for a single sheet).

```
part ../shed.gasm
title "Office shed"
sheet "Back wall framing" size=A3 scale=1:25
view back_* profile="bar 44x150,bar 150x44" lengths=1 from=+y label="back wall from outside"
dims back_* profile="bar 44x150" along=h ends=1 style=ordinate
dim 9,3860,150 9,3860,2600 offset=8
cutlist back_* profile="bar*" summary=1 place=32,40
section !pv.* from=+x at=2500 window=-650,2150,350,3050 scale=1:10 label="eaves"
callout 2500,-300,2900 "EPDM on 11 OSB" to=-40,10
level 3000 "top"
note "first line|second line" place=230,40
```

| operation | parameters | does |
|---|---|---|
| `part` | `file` `variable=value ...` | the `.gcad` or `.gasm` to draw, run with those variables |
| `title` | `"text"` | the title in every sheet's title block |
| `sheet` | `"name"` `size=A0..A4` `scale=1:N` | starts a landscape sheet with a border and a title block (name, scale, sheet n of N, date) |
| `view` | `parts ...` `from=` `profile=` `also=` `scale=` `window=` `place=` `lengths=1` `label=` | draws the parts as seen from `-y`, `+y`, `-x`, `+x`, `+z` or `-z` (front, back, left, right, top, bottom), filled in their colours with their edges, nearer parts over farther ones |
| `section` | the `view` parameters and `at=` | cuts the parts with the plane square to `from` at that coordinate and fills each cut in the part's colour |
| `dims` | `parts ...` `profile=` `along=h\|v` `of=centres\|edges\|ends` `ends=1` `across=1` `row=` `style=chain\|running\|ordinate` `total=1` | dimensions the last view: a chain between the centres (or edges) of the parts along the sheet's horizontal or vertical, `ends=1` adds the outer ends of the group, `across=1` keeps only parts that run across the dimension (studs, not plates), `ordinate` writes setting-out distances from the first mark, `row` moves the line further out |
| `dim` | `x,y,z x,y,z` `offset=` `text=` | one dimension between two model points in the last view, horizontal or vertical on the sheet |
| `level` | `height "name"` | a level mark at that height beside the last elevation or section |
| `callout` | `x,y,z "text"` `to=dx,dy` | a leader from a model point in the last view to text `dx`, `dy` mm away on the sheet |
| `label` | `"text"` `target ...` `side=` | annotates the last view with no layout by hand. A target is `part:faces` (a hole or round face points at its rim and, with no text, is written `⌀12`, `4× ⌀3.4` or `M3 tapped (⌀2.5 drill)`), `part:a&b` (an edge) or `x,y,z`. Labels collect in a column beside the view, sorted to the order of their targets so leaders do not cross, clear of the geometry, dimensions, levels and the title block, and are laid out together when the next `view`, `section` or `sheet` starts. `side=left\|right\|top\|bottom` picks the column. `callout` with no `to=` is a `label` |
| `note` | `"text"` `size=` `place=` | text, `|` starting a new line |
| `cutlist` | `parts ...` `profile=` `summary=1` `place=` | a table of the weldment members among the parts, by section and length, or with `summary=1` by section with the piece count, total length and longest piece |
| `nest` | `parts ...` `stock=LxW,LxW` `gap=` `trim=` `thickness=` `grain=1` `edged=1` `edge=parts,...` `scale=` | packs the parts, as flat panels sized by their bounding box (the thinnest side is the thickness), onto stock with guillotine cuts. Given several stock sizes it buys the mix that wastes the least board. `edged=1` says the stock's two long edges are finished (factory edge banded), so `trim` only comes off the bare ends; parts named in `edge` have a long side that shows and are put with it on a finished edge, never turned. Each sheet and the whole plan report the board bought, the parts and the waste in m² and %. Cuts are guillotine cuts: every cut runs edge to edge across the board it is made in, as a panel or track saw cuts. `gap` (4 unless given) is the least space between pieces, the saw kerf plus any allowance; `trim` comes off every stock edge first; `grain=1` keeps each part's length along the stock's length. Draws one stock sheet per drawing sheet, the current sheet first and copies of it after, each with its pieces, the cuts numbered in the order to saw them as red dashed lines, and the list of cuts measured from the stock corner. Parts of different thicknesses are refused unless `thickness=` picks one board. `gcad nest file --stock 2440x1220,2440x600 --gap 4 --edged --edge 'shelf_*' parts...` prints the same layout as text |

Parts are named as in `gcad bom` (`back_1.osb`, `roof.L12`); `*` matches any run of
characters, a pattern starting with `!` leaves parts out, and no pattern means every part.
`profile=` keeps only members whose section matches (`bar 44x150`, `bar*`), and `also=`
adds parts back after that filter, such as a steel beam drawn with the timber. `window=u0,v0,u1,v1`
crops a view to a box in model millimetres, measured along the sheet's horizontal and vertical
for that view; with `scale=1:10` it makes a detail. Views and tables are placed left to right
in rows unless `place=x,y` puts their lower left corner that many mm from the sheet's lower
left; a view that would run off the sheet or into the title block is an error naming its size.
`lengths=1` writes each member's length along it, which is the cut list for raked studs.

## Simulations

A `.gsim` file describes one simulation of a part or assembly, one operation per line
like the other files, with `let`, `if` and `include` as usual. `gcad sim file.gsim` runs it,
prints the readings, and writes them to `file.result.bin` beside it. It fails if an
`expect` line does not hold. `--dry-run` reports the size of the problem without solving it.

```
sim torque
part arm.gasm grip=60
pose elbow -40
payload hand 300 at=420,0,200
sweep shoulder elbow steps=16
sweep wrist
expect shoulder<15 sun_s_turn<4
```

| operation | arguments | does |
|---|---|---|
| `sim` | `kind` | what to simulate. Comes first. `torque`, `static`, `thermal`, `modal`, `buckling`, `flow`, `magnetic`, `frame`, `conduction` and `acoustics`. `static`, `thermal`, `modal` and `buckling` mesh the part in tetrahedra, `magnetic` meshes a slice of it, `flow` and `conduction` put it on a grid, `frame` turns each body into a beam member and `acoustics` traces rays in the room; `--dry-run` shows the mesh, grid or members without solving. `flow`, `frame`, `conduction` and `acoustics` are described in `docs/simulation.md` |
| `part` | `file` `variable=value ...` | the `.gcad` or `.gasm` file simulated, with variables fixed as in an assembly's `part` line. Every body of it is simulated |
| `pose` | `joint value` | poses a joint before the run, as in an assembly |
| `material` | `part name` and the `material` arguments | overrides a part's material for this simulation |
| `materials` | `file` | loads a material library for the simulated parts |
| `gravity` | `x,y,z` | in m/s². A torque simulation uses `0,0,-9.81` unless given; a static or frame one leaves the part's weight out unless this line is there |
| `payload` | `part grams` `at=x,y,z` | a point mass carried by a part, at its centre of mass unless `at=` places it (in the assembly's coordinates as posed when the line runs). It moves with the part |
| `sweep` | `joint ... steps=` | runs every pose of the listed joints across their ranges in `steps` (24 unless given) equal steps; a joint that turns all the way round goes from -180 to 180. Several joints on one line make a grid of poses; separate lines make separate sweeps |
| `fix` | `faces` | holds the faces still (static). In a frame it holds the member ends on those end faces in every direction, and `pin` holds position and twist but leaves bending free, `slide` only along the end's normal. Its reaction force is a reading named after the line's label, or `fix1`, `fix2`, ... |
| `slide` | `faces` | holds the faces only along their normal, free to slide along them (static); a symmetry plane, or a part resting on a flat. Reading as for `fix` |
| `pin` | `faces` | holds every point of a round face on its axis, free to turn about it, like a link on a pin; the face cannot tilt, so a short band acts as a clamp (static). Reading as for `fix` |
| `bearing` | `faces` `axial` | holds a round band sideways at one ring of nodes, on the edge nearer the middle of the part, free to turn and tilt, like a shaft in a ball bearing: it carries force and no moment. With `axial` it also holds the band along its axis at one node; give that to one bearing of a shaft (static). Reading as for `fix`. `bearing part:faces shaft` `axial` `at=` `C0=` `stiffness=` instead joins two parts of an assembly: `shaft` turns in the housing's bore `faces` on a ball row at the middle of the bore, or across from `at=`, free to turn and tilt, so it carries force and no moment; with `axial` it also carries thrust, through the axis. `stiffness` is its radial stiffness in N/mm (1e5, a small ball bearing, unless given). It reads the radial load as the line's label (the housing's name unless labelled), `.axial` the thrust, and with the static rating `C0` in newtons `.safety`, C0 over the equivalent load max(Fr, 0.6 Fr + 0.5 Fa) |
| `torque` | `faces moment` `axis=x,y,z` `at=x,y,z` | twists the faces by `moment` N·m about their axis: the axis of a round face, or the normal through the middle of a flat one, unless `axis=` and `at=` say; `axis=` also takes `x`, `y`, `z` or `-z` (static) |
| `speed` | `joint value` | how fast a joint runs, in rpm (mm/s on a `slide`); joints it drives or is driven by follow through the couples, and each reads its power and each mesh its heat (torque) |
| `torque` | `joint value` | a steady torque in N·m (a force in N on a `slide`) on the joint, pushing it positive, like wind on an antenna (torque) |
| `load` | `faces force=x,y,z` | a force in newtons spread over the faces by area (static). In a frame a force on an end face is a force on that joint, and on a side face it is spread along the whole member; lines with the same label are one load case |
| `pressure` | `faces value` | a pressure in MPa pushing into the faces (static) |
| `warm` | `by` `part=`, or `file.gsim from=` | warms the whole part evenly by that many °C (negative cools it), or with `part=` one part of an assembly (lines add up, so a bolt can be cooled to preload a joint: its shrink over its length is the stretch it is tightened by), so it expands by its material's `alpha` (static, buckling). Given a `sim thermal` file with the same `part` and `mesh` lines, it takes the temperatures that simulation works out instead, expanding from `from` °C (20 unless given). Parts held from growing, or bonded to a material that grows differently, take stress from it |
| `probe` | `x,y,z`, with a label | reads the von Mises stress and displacement at a point inside the part or on its surface as `label.stress` and `label.displacement` (in a conduction simulation `label.temperature`, in a frame the nearest member, and in acoustics a listener whose readings are `label.T30`, `label.T20`, `label.EDT`, `label.C80`, `label.D50`, `label.STI`); a point just outside a coarse curved face is read on the mesh surface, with a note |
| `temperature` | `faces value` | holds the faces at that temperature in °C (thermal, conduction). The heat leaving the part through them, in watts, is a reading named after the line's label (negative where heat comes in) |
| `convect` | `faces h= ambient=` | the faces lose heat to air at `ambient` °C (20 unless given) with a film coefficient `h` in W/m²·K: about 5 to 10 for still air, 20 to 100 with a fan (thermal, conduction). In a conduction simulation lines with one label are one side of the construction, and the label names its heat flow. The heat lost is a reading named after the label |
| `heat` | `faces watts` | puts that much heat into the faces, spread by area (thermal, flow). In a flow simulation the air carries it away, and the line's label reads the mean temperature of those faces |
| `spin` | `joint rpm=` | turns a part on its `turn` joint through the air at that speed (flow), see `docs/simulation.md` |
| `turn` | `joint` `sweep=` `steps=` | the rotor's `turn` joint (magnetic). Its axis is the slice's normal and everything it carries turns. With `sweep` the rotor turns through that many degrees in `steps` (12 unless given), and the step must divide a whole turn |
| `slice` | `at=` | where the slice cuts, in mm along the joint axis from the joint's point; the middle of the turning parts unless given (magnetic) |
| `length` | `mm` | the stack length the slice's answer is multiplied by; the turning parts' length along the axis unless given (magnetic) |
| `magnet` | `parts ...` `Br=` `mu=` `toward=` | the parts are permanent magnets of remanence `Br` in tesla and recoil permeability `mu` (1.05 unless given), magnetized `toward=radial` (the default), `-radial` or along a direction like `0,1,0` that turns with the rotor (magnetic) |
| `iron` | `parts ...` `material=` or `mu=` | the parts are steel with the B-H curve of `material`: `m19` (the default, lamination) or `1010` (solid low carbon); or iron of constant relative permeability `mu` (magnetic) |
| `coil` | `parts ...` `turns=` `current=` `axis=` | the parts are windings of `turns` turns carrying `current` amps (0 unless given). A coil part the slice cuts in two pieces, one each side of the tooth it is wound round, carries the current one way in one and back in the other: positive current goes counterclockwise about `axis`, the direction from the motor's axis out through the coil unless given. Negative `turns` winds it the other way. Lines with the same label are one phase, and the label names its readings (magnetic) |
| `release` | `faces` | pins the member ends on those end faces: they carry force and no moment (frame) |
| `buckling` | `count` | how many buckling load factors `frame` finds, for all the loads together; they read `buckling1`, `buckling2`, ... |
| `combine` | `name load:factor ...` | adds the loads, each times its factor, into a combination read as `name.stress`, `name.displacement` and so on. A load is named by its label, and `weight` is the member's own weight (frame) |
| `absorb` | `faces material` or `faces a,b,c,d,e,f,g` `scattering=` | what the faces absorb, as a material (`concrete`, `brick`, `plaster`, `plasterboard`, `glass`, `wood`, `panelling`, `marble`, `carpet`, `curtain`, `ceiling-tile`, `audience`) or as one coefficient or seven, for the octave bands 125 Hz to 8 kHz. `scattering` is the share of reflections that are diffuse, 0.2 unless given. Later lines replace earlier ones, so start with `absorb all concrete` (acoustics) |
| `speaker` | `x,y,z` | where the sound comes from, in mm (acoustics) |
| `rays` | `count` | how many rays to trace, 20000 unless given (acoustics) |
| `source` | `watts` | makes that much heat evenly through the part's volume (thermal) |
| `modes` | `count` | how many natural frequencies `modal` or `frame` finds, 6 unless given |
| `contact` | `part part` `friction=` | the two parts press on each other where they touch instead of being bonded, and may part (static). Without `friction` they slide freely, so give them supports that stop that; with it (a coefficient, 0.3 for dry steel) they hold until the sideways force passes `friction` times the pressing force, and the run reports how many node pairs slip. A load that friction cannot hold is an error that says the parts slide. Frequencies and buckling treat the pair as bonded |
| `apart` | `part part` | the two parts do not bond where they come within a tenth of an element of each other, for a cover over a gear or anything else that sits close without carrying load across the gap. `*` matches any run of letters, as in `leave`, so `apart cover* wheel*` keeps every cover off every wheel |
| `leave` | `part ...` | leaves the named parts of an assembly out of the simulation; `*` matches any run of letters, so `leave ctl* *screw*` drops the control board parts and every screw. Use it for parts that carry no load (boards, cables, seals) and for fasteners in clearance holes, which only touch under their heads |
| `converge` | `within=` | solves a second time on a mesh 1.5 times coarser and adds a note naming every reading that moved more than `within` percent (5 unless given) between the two. A reading that moves has not settled: a peak stress at a sharp inside corner never does, so `probe` away from it or refine the mesh. Costs about a third more time |
| `mesh` | `size` | the target edge length of the tetrahedra in mm, for `static`, `thermal` and `modal`; a twentieth of the part's size unless given. In `conduction` it is the finest grid spacing, a quarter of the thinnest layer unless given, and in `frame` the longest element along a member, a quarter of each stretch between joints unless given. The mesher makes elements smaller where walls are thin, gaps are narrow or edges run close together |
| `expect` | `reading<limit ...` or `reading>limit` | a check on a reading, by name. `gcad sim` fails when one does not hold |

`sim torque` works out the load each joint of an assembly must hold against gravity and
payloads, from the mass and centre of mass of every part with a `material`, at the posed
position and at every pose of every `sweep`. Each moving joint gets a reading named after
it: the largest torque (N·m, for `turn`) or force (N, for `slide`) seen and the pose
where it was seen. Couples carry the load: a joint driven by another through
`couple ... ratio=r` holds the driver's load divided by `r`, so a motor's sun gear reads
the torque the motor needs. Through couples with an `efficiency` or a worm's `lead` and
`friction`, that reading is the torque to drive the load, divided by the efficiencies on the
way, and a second reading `joint.back` is the torque the load turns the joint back with,
0 where a pair locks; a note names the joints that lock. A joint's `drag` and the drag of
every joint it moves add to the torque to drive it, through the same ratios and losses, and
take away from the torque the load turns it back with. A joint behind lossy couples also reads
`joint.ideal` (the torque with no losses), `joint.friction` (lost in the meshes) and
`joint.drag`, which add up to its torque. Each couple reads `mesh.driver.efficiency` and
`.back` in %, and `.torque`, the torque through the mesh at the `driver` gear at the worst pose.
With `module` and `face` it also reads, on the `driver` gear, the `.tangential` force on the
pitch circle (a worm's thrust), the `.radial` force pushing the shafts apart and for a worm the
`.axial` force along the gear's axis (the worm's turning force), each in N; `.bending`, the
Lewis root stress with form factor 0.484 - 2.87/z; `.contact`, the Hertz flank stress (the
worm as a rack) when the gear's material has a stiffness; and `.safety`, its strength over the
bending stress. With a `speed`, each joint reads `.speed` and `.power` (W at its worst torque)
and each mesh its `.heat` (W lost to friction) and a worm its `.sliding` speed (m/s).
Joints that would read the same as another, like the copies a
`pattern` makes, are listed in a note instead, and their names still work in `expect`.
Parts with no material weigh nothing, and a note lists them.

`gcad check file.gsim` runs the lines and says whether `file.result.bin` is up to date, and
prints its readings if it is.
A result is stale when the spec, any file it reads, the `--set` variables or the gcad
version changed since it was written. `gcad render file.gsim file.png` plots the
result's sweeps: one panel per sweep and unit, each joint's load against the first swept
joint (the worst over the other joints when several are swept). The viewer shows the
simulated part, and once a `static`, `thermal` or `modal` result is up to date it shows the
part coloured by it instead (deformed as in `render`), with the range in the parts card.

`sim static` is linear elasticity on curved 10-node tetrahedra.
Units are millimetres, newtons and MPa. It needs a material with a stiffness and at least one
`fix` or `slide`. Its readings are `stress` (the peak von Mises stress and where, more than one
element size from any support or bonded face of the same part: the `mesh` size, or a sixth of
the part's diagonal when that is smaller), `support_stress` (the peak inside that band, when it is
higher), `displacement` (the largest movement and where), `safety` (yield strength over `stress`,
when the material has one), each support's reaction force (and `label.moment` in N·m when it also holds a twist) and each probe's values. A part its supports leave free to move or turn is refused before solving. The
stress at the edge of a fixed face or a bond is a singularity: it keeps rising as the mesh is
refined, which is why `stress` leaves that band out. A sharp inside corner elsewhere is one too, so
put a fillet there or `probe` the stress a little away from it.
`gcad render file.gsim file.png` draws the von Mises stress on the deformed part, with the
deformation exaggerated so it shows.

```
sim static
part bracket.gcad
material aluminium
mesh 2
base: fix <Z
load boss.end force=0,-200,0
top: probe 18,12,12
expect stress<80 displacement<0.1
```

`sim thermal` is steady heat conduction through the part, on the same mesh, using the
material's conductivity. It needs at least one `temperature` or `convect` line. Its readings
are `temperature` (the hottest point), `coolest`, the heat through each labelled boundary and
each probe's `.temperature`. `render` draws the temperatures.

```
sim thermal
part heatsink.gcad
material aluminium
heat pad.end 12
air: convect fins.side h=10
```

`sim modal` finds the lowest natural frequencies of the part, held by its `fix` and `slide`
lines, from its stiffness and density. The readings `mode1`, `mode2`, ... are in Hz, each with
the share of the part's mass that moves with it along x, y and z (the modes with a large share
are the ones a shaker or a motor along that axis excites). With no supports the six rigid-body
modes at 0 Hz are left out. `render` draws the first mode's shape.

`sim buckling` takes the same supports and loads as `static` and finds how many times those
loads the part can carry before it buckles: the reading `buckling` is that factor, and
`buckling2`, ... the next ones (`modes` sets how many, 3 unless given). A factor below 1 means
the loads given already buckle it. It is linear buckling, so real parts with imperfections
and yielding give way earlier; treat a factor under about 3 as too close. `render` draws the
first buckled shape. Loads that only stretch the part are refused.

`static`, `thermal`, `modal` and `buckling` take an assembly too. Each part is meshed on its own, in its
own material, and parts whose faces touch (closer than a tenth of the mesh size) are bonded
there, as if glued: no sliding, no gaps, heat passes freely. Pick faces as `part:faces`, like
`fix base:<Z` or `load arm:tip.end force=0,0,-50`. A note lists which parts were bonded and over
how many nodes. A part that touches nothing held (no support, or for heat no `temperature` or
`convect`, reaches it through touching parts) is refused by name before the solve, since the
solver cannot converge with it loose. `contact a b` makes a pair press on each other instead, free to slide and to
part, solved by repeating the solve until the touching nodes settle (static only; heat,
frequencies and buckling treat the pair as bonded). `friction=0.3` adds Coulomb friction. Stresses right at a bond between different materials are a corner singularity
like a sharp inside corner.

For any of the three, `gcad sim file.gsim --dry-run` meshes every body of the
part into tetrahedra and reports the node and element counts, how far the mesh volume is from
the solid's, and the worst element. `gcad render file.gsim file.png` draws the mesh with the
half nearer the front view cut away, the cut faces in orange.

The material names know their stiffness, strength and conductivity as well as their
density:

| material | E GPa | ν | yield MPa | k W/m·K | α µm/m·K |
|---|---|---|---|---|---|
| steel | 200 | 0.29 | 250 | 50 | 12 |
| stainless | 193 | 0.29 | 215 | 16 | 17 |
| aluminium | 69 | 0.33 | 240 | 167 | 23.6 |
| brass | 97 | 0.31 | 124 | 115 | 20.5 |
| copper | 117 | 0.34 | 70 | 390 | 17 |
| titanium | 114 | 0.34 | 880 | 6.7 | 8.6 |
| pla (printed) | 2.8 | 0.35 | 40 | 0.13 | 68 |
| petg (printed) | 2 | 0.38 | 35 | 0.2 | 60 |
| abs (printed) | 2 | 0.35 | 30 | 0.17 | 90 |
| asa (printed) | 1.9 | 0.35 | 32 | 0.17 | 95 |
| nylon (printed) | 1.5 | 0.39 | 40 | 0.25 | 100 |
| tpu (printed) | 0.026 | 0.48 | 8 | 0.2 | 150 |
| polycarbonate | 2.3 | 0.37 | 60 | 0.2 | 68 |
| acrylic | 3.2 | 0.37 | 65 | 0.19 | 75 |
| resin | 2.2 | 0.35 | 50 | 0.2 | 80 |
| wood | 10 | 0.3 | 40 | 0.12 | 5 |
| concrete | 30 | 0.2 | 30 | 2 | 10 |
| brick | 10 | 0.2 | 10 | 0.81 | 6 |
| glass | 70 | 0.22 | 40 | 1 | 9 |
| gypsum | 2 | 0.25 | 3 | 0.25 | 14 |
| eps | 0.005 | 0.1 | 0.1 | 0.036 | 70 |
| wool | 0.0005 | 0.1 | 0.02 | 0.04 | 3 |

Printed plastics carry printed-part values, lower than moulded ones, along the layers;
across the layers they are weaker again. Wood is along the grain. `material name E= nu= yield= k= alpha=` sets
any of them, for a listed material or a new one.

## Workplanes

`XY` has normal +Z, `XZ` has normal -Y with y up, `YZ` has normal +X with y up.
`plane <faces>` takes flat, coplanar faces and puts the origin where the world origin
projects onto them, so sketch coordinates on a face stay world coordinates. The
plane's x and y follow "Z is up, looking at the face from outside": on side faces y is
+Z, on top faces x is +X and y is +Y, on bottom faces y is -Y. `check` prints the
origin and axes of every `plane` line.

## Selectors

Faces:

| selector | faces |
|---|---|
| `label` | every face the labelled line made |
| `label.group` | one group of them, see below |
| `>Z`, `<X`, ... | flat faces facing +Z (or -X, ...) that are furthest that way |
| `+Z`, `-X`, ... | every flat face facing +Z (or -X, ...) |
| `all` | every face |
| `a,b` | faces in `a` or `b` |

Groups each operation records:

| operation | groups |
|---|---|
| `extrude` | `start`, `end`, `side` |
| `cut` | `end` (pocket floor), `side` |
| `hole` | `bottom` (blind holes), `side`, `cbore`, `cbore_floor`, `csink` |
| `revolve` | `caps`, `side` |
| `sweep`, `loft` | `start`, `end`, `side` |
| `push` | `start`, `end`, `side` |
| `shell` | `inside`, `floor` |
| `split` | `cut` |
| `fillet`, `chamfer`, `draft`, `thread`, `import` | `faces` |
| `mirror` | a copy of every group, under the mirror line's label |

A face belongs to a group while it still lies on the surface the operation made, so
groups survive later cuts and fillets that trim the face.

Edges, for `fillet` and `chamfer`:

| selector | edges |
|---|---|
| `faces` | every edge of those faces |
| `a&b` | edges where a face in `a` meets a face in `b` |
| `x\|y` | edges in `x` or `y` |

`base.end&base.side` is the top perimeter of an extrusion and leaves hole rims alone.
`fillet` and `chamfer` skip edges where the faces already meet smoothly, such as the
lines where a rounded corner of a `rect r=` joins its flat sides.
`base.side&base.side` is the vertical edges between its side walls.

## Checking a file

```
gcad check part.gcad                  # run every line, print what each did
gcad query part.gcad 'base.end&base.side' [--line N]
gcad render part.gcad part.png        # iso, top, front and right views
gcad export part.gcad part.step       # or .stl, .obj, .3mf, .svg (a four-view drawing), .dxf (sheet metal flat pattern)
gcad export part.gcad part.svg --section y=0   # adds a hatched section across y=0
gcad check part.gcad --set w=50       # override a `let` variable
gcad check part.gcad --time           # also show how long each line took
gcad sim arm.gsim                     # run a simulation, write arm.result.bin
gcad render arm.gsim arm.png          # plot a simulation's sweeps
gcad docs                             # print this reference
gcad part.gcad                        # open the viewer; plain `gcad` starts with a file picker
gcad view part.gcad [--line N] [--select 'base.end&base.side']
```

An `.svg` drawing shows the top, front, right and iso views with the overall width,
depth and height dimensioned, and calls out round holes in the views that look down
them, like `4× ⌀3.2`. `--section x=`, `y=` or `z=` adds a view of the part cut
across that plane, looking at the cut, with the cut faces hatched.

`check` stops at the first failing line and prints its number and why. After every
line that changes the solid it prints the face count, volume and bounding box.
Every part file an assembly brings in is kept built in `~/.cache/gcad/parts` (or
`$XDG_CACHE_HOME/gcad/parts`), up to 1 GB, and read back the next time the same file
is used with the same variables, as long as no file it read has changed (the part file,
its includes, imports and material libraries) and gcad is the same build. Sub-assemblies
are kept the same way, and simulation meshes are kept in `~/.cache/gcad/meshes` for the
same solid at the same size, so opening a simulation result does not mesh the part again.
Set `GCAD_NO_PART_CACHE=1` to build every part and mesh from scratch.

`view` opens a window that rebuilds on every save. The left column is the file,
one row per line, green when it ran, red where it failed. Clicking a row (or up and
down) shows the solid as it was after that line, with the faces that line made in
amber. The select box highlights whatever a selector matches at that line in cyan,
which is the quickest way to see what a `fillet` or `plane` selector will hit.

The viewer also has:

- a view cube in the corner: click a face of it, or the iso, top, front, right, back,
  left and bottom buttons under it, to look from that side;
- measure mode (the measure button): click two points on the model to get the
  distance between them and along each axis, corners snap, and each pick names the
  face it landed on with its area;
- section: cuts the model across x, y or z at a slider position, with flip to show
  the other half; the cut faces are orange, and measure picks ignore what is cut away;
- ortho: on by default, as in most mechanical CAD; switch it off for a perspective view;
- a parts card listing every body in its `color`, with a switch to hide each one, and
  an explode slider when the assembly has `explode` lines;
- a joints card with a slider for every `joint`, which moves the parts without
  rebuilding, follows couples and closed loops, locks joints that mates hold, and
  checks for parts that overlap at the slider positions;
- open (or ctrl-O) to switch to another `.gcad` file.

`render` lays the views out as iso (top left), top (top right), front from -Y
(bottom left) and right from +X (bottom right), each with an axis marker: X red,
Y green, Z blue.

## Limits

- Where three rounded edges meet, the corner is a sphere (or a flat triangle for a
  chamfer). Where an inside edge meets two outside ones, or an outside edge two inside
  ones, as at the rim of a pocket or the foot of a boss, it is a torus patch (or a flat
  quad). Rounding at such corners needs flat faces, straight edges, the face across the
  odd edge square to its sides, and square faces wherever a rounded edge meets one that
  is not rounded. An inside and an outside rounded edge cannot meet at a sharp one.
- `draft` and pushing a face inward work on flat-sided parts, where every moved corner
  is where three flat faces meet.
- `shell` hollows the extrusion or revolve that owns the open face, so do it before
  adding other features to it. A revolve that touches its axis can only be shelled
  while it is the whole solid.
- Edges a boolean makes are exact lines and circles where they are flat and round, as at
  the rim of a hole; other intersections, such as two crossing cylinders, are fine polylines.
- `draft` works on profiles without holes.
