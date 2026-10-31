# Changelog

All notable changes to gcad are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.1] - 2026-10-31

### Added
- `gcad nest` packs flat parts onto stock sheets with guillotine cuts, with mixed stock sizes, edged stock and a waste report, and a `.gdwg` operation for it.

### Changed
- egui and eframe 0.37, which keeps the window running on Wayland when its workspace is hidden.

## [0.5.0] - 2026-10-19

### Added
- Drawings: `label` points at faces, holes, edges or points and gcad places the text in a column
  beside the view where it overlaps nothing; hole labels write their own size, count and tap.

### Changed
- Drawing PDFs are written with lopdf and their content streams are compressed.
- Drawing notes keep off the views.

## [0.4.0] - 2026-10-14

### Added
- The viewer shows the sims server: a `run on server` button, the balance and running jobs on
  a `server` toggle, and a `sims server` card with the account, runners on this build, the run
  limits, recent runs and cancel. `make account`, `buy credit` and `copy id` work from it. Buying credit opens
  https://apexstrata.ie/shop/gcad-sim-credit/ when the server names no shop.
- The skill offers the sims server on slow hardware and tells an agent to ask before spending.
- Remote runs pair a gcad with any runner of the same major and minor version, not the same
  commit, so a runner no longer goes stale with every code change. A test decodes stored jobs
  and fails when the job layout changes without a minor bump.
- A standard material library of about 210 metals, plastics, 3D printing materials, rubbers,
  timbers, boards, construction materials, stone, ground, insulation, glass and composites.
  Every entry states density, stiffness, Poisson's ratio, strength, conductivity, expansion,
  specific heat, colour, roughness, metallic and a texture, and a test fails if one does not. A
  project table named like one keeps every key it does not give.
- Specific heat: `cp` in `materials.toml` and `material ... cp=`, J/kg·K, also read from MatML
  and FreeCAD cards and shown by `gcad materials`.
- Material textures: `texture` (`wood`, `osb`, `concrete`, `stone`, `brick`, `sand`, `fabric`,
  `foam`, `rubber`, `brushed`, `carbon-fibre`, `polished`, `cast`, `plaster`, `granulate`,
  `marble`, `asphalt`, or `none`), `texture_scale`, `texture_rotation`,
  and `color_map`, `normal_map`, `roughness_map`, `metallic_map` image files, drawn in the
  viewer.
- `member ... material=c24` assigns a material to every generated member. `count=N
  step=x,y,z` lays a row of members in one line, `end_step=` moves only the last point (studs
  under a sloping roof) and `when=expr` skips instances, with `k` the index (openings);
  `count=N angle=360 axis=hub` makes a circular array. `pattern` also takes `count=1` and
  reports large patterns as a count.
- `materials.toml` sets how a material looks: `color`, `roughness`, `metallic`, `specular`,
  `clearcoat`, `transmission`, `ior` and `emission`, used by the viewer and its ray tracer. A
  table named like a built-in material with no `density` adds to the built-in one.
- `sim frame`: every body is a beam member, joined where ends meet, with `fix`, `pin`, `slide`,
  `release`, `load`, `gravity`, `combine`, `modes` and `buckling`; reads displacement, stress,
  safety, reactions and the deflected shape (`examples/frame/portal.gsim`).
- `sim conduction`: steady 3D conduction on a graded grid for thermal bridges, reading the
  coupling coefficient, heat flow per side, the coolest surface point and fRsi
  (`examples/conduction/tie.gsim`).
- `sim acoustics`: room acoustics by ray tracing with `absorb`, `speaker` and listener probes,
  reading T30, T20, EDT, C80, D50 and STI against Sabine and Eyring
  (`examples/acoustics/room.gsim`).
- Built-in materials `concrete`, `brick`, `glass`, `gypsum`, `eps` and `wool`.
- `.gdwg` drawing files: sheets with title blocks, views, sections and detail crops of named
  parts, dimension chains and setting-out, levels, callouts, notes and member cut lists,
  exported as a multi-page PDF or SVG sheets.
- Viewer: opening with no file shows a short help (what gcad is for, how to open a file,
  how the live view works), with a `?` toggle and `?` key to bring it back.
  The browser page shows it too instead of defaulting to an example.
- `color ... opacity=` on a body or an assembly part: the viewer draws it see-through, so
  glass can be glass.
- Sheet metal from a flat blank: `tab` takes any outline with holes, `bend a,b c,d angle=` folds
  it along a line, keeping every label made on the blank, and `unfold` and the DXF give back the
  blank. `dart bend at ... l= w=` presses stiffening darts into a bend.
- `measure box`, `measure clearance a b` and `measure space at=x,y,z` in parts and assemblies:
  bounding boxes, the nearest gap between two bodies, and the free box inside a case with what
  bounds each side. `gcad measure file ...` runs one without adding it to the file.
- Viewer: measure snaps to corners, edge midpoints, arc centres and edges, with a marker under
  the cursor; a unit picker (mm, cm, m, in) labels every length, area, volume and mass; the
  ortho toggle sits with the view presets.
- `spin joint rpm=` in `sim flow`: a fan or propeller turns through the lattice and reads its
  torque, shaft power, thrust and flow (`examples/flow/fan.gsim`).
- `sim magnetic`: a 2D slice through a motor with `magnet`, `iron` (M-19 and 1010 B-H curves) and
  `coil` lines, swept with `turn joint sweep= steps=` for torque, cogging, ripple and back-EMF
  (`examples/magnetic/cogging.gsim`).
- Sub-assemblies move as one: `move az 0,0,10` and `rotate az ...` take a sub-assembly's name
  (or a multi-body part's) and move all its parts, and its axes come in as `az.axis` for joints
  in the parent.
- Remote simulations: `gcad sim x.gsim --remote` builds the model locally, sends the built
  bodies, materials and selected faces to a paid GPU worker at sims.gcad.sh and writes the
  result beside the spec. `gcad remote signup|login|account|runners|jobs|cancel` manage the
  account; runners set their own price an hour and `--max-price` caps it.
- `bearing housing:bore shaft` in a static simulation joins two parts on a ball bearing: radial
  force at the ball row, free to turn and tilt, thrust with `axial`, stiffness from
  `stiffness=`, and the load against the static rating with `C0=`.
- Gear drives in `sim torque`: `couple ... module= face= teeth= pressure= gear= pinion=` reads
  each mesh's efficiency, torque, tangential, radial and axial forces, Lewis bending and Hertz
  contact stress and bending safety; `speed joint rpm` adds each joint's power, each mesh's
  friction heat and a worm's sliding speed; and a motor's torque splits into the ideal load,
  mesh friction and drag. The viewer shows readings with more than four values per part as a
  block of figures for each.
- Progress for every simulation: `gcad sim` reports each stage on stderr (model and part files,
  meshing, bonding, assembly, solver iterations and residual, eigen solves, flow steps with the
  running drag, torque poses, writing) with counts and time left, and `-q` turns it off. The
  viewer's simulation card shows one row per stage with its time, and a bar, the time left
  and one line of detail for the running stage; work inside a stage (part files, per-part
  meshing, interference) feeds that line instead of adding rows. The old result and its stale
  warning are hidden while it runs.
- The simulation card lays readings out as figures three to a row, x/y/z readings on one line,
  and per-part or per-probe readings as a table (the 8 largest, with a button for the rest); a
  reading named after a part fills its `value` column. Checks show as ok/fail lines, notes fold
  behind a button, and the stale warning is one line naming what changed.
- Opening or rebuilding a file in the viewer shows the same progress panel over the viewport.
- Viewer simulation card with readings, `expect` checks, notes, sweep traces and a run button.
- Viewer draws a simulation's setup before it has a result: the mesh with loads and supports,
  or for flow the parts in their duct or air box, the wind and the heated faces.
- `bearing faces axial`: a round band held sideways at one ring, free to turn and tilt, so it carries
  force and no moment. `pin` on a short band suggests it.
- `support_stress` reading: the peak von Mises stress within one element size of supports and bonds.
- `torque axis=` and `wind toward=` take `x`, `y`, `z` and `-z`.
- Gear and worm losses on couples: `efficiency=` and `back=`, or a worm's `lead=` and `friction=`,
  with `joint.back` readings and worms that lock.
- `tools/fluidx3d/`: the cube, sphere and tube flow benchmarks set up in FluidX3D, for speed and
  accuracy comparisons with `sim flow`.
- `torque joint value` in `sim torque`: a steady torque on a joint, like wind.
- `joint ... drag=`: bearing and seal friction on a joint, added to the torque to drive it.
- A boss glued onto a face, including twisted faces and over an edge; twisted extrudes with holes.

### Changed
- `interference none` lets a coupled gear and pinion share up to `module`³ where their teeth meet,
  so a sweep through a meshing pair fails only on a real clash.
- The viewer's run button simulates the model it has already built instead of building the file
  again, and after the run only redraws the scene to pick up the result instead of reloading.
- Simulation meshes are only refined again when an element is flatter than half a degree
  (was 2 degrees). Elements between the two solve fine, and parts that never reached 2 degrees
  were meshed far finer than asked: the rotator's electronics box at `mesh 3` made 15.7 million
  nodes instead of 78 thousand.
- Simulation meshes have far fewer flat elements: points on the surface slide along their face
  or edge to raise the worst element, and shedding flat elements no longer folds the surface.
  The worst element across the example parts went from 0.27 to 5.2 degrees.
- Booleans and part loading are faster: the truck example checks in under 4 s (was 23 s) on half
  the CPU, and its wheel part builds in 1 s (was 2.5 s). Faces of a boolean operand far from the
  other one are meshed coarsely. The viewer keeps loaded parts between rebuilds, so saving
  an assembly rebuilds only what changed.
- The viewer rebuilds only the parts whose files changed, and the sub-assemblies holding them; the
  rest of an assembly is reused. Saving one part of the truck example rebuilds it in 1.7 s (was 4.2 s).
  It also watches every file a build read, so parts and sub-assemblies in other folders rebuild on save.
- Built part files are kept in `~/.cache/gcad/parts` and read back while none of the files they
  read has changed, so opening or checking an assembly again skips building its parts: the truck
  example's parts load in 1 s instead of 4.4 s. `GCAD_NO_PART_CACHE=1` turns it off. Sub-assemblies
  are kept too, and `interference` meshes each part once per build instead of once per line: checking
  the rotator again takes 7.6 s instead of 24 s.
- Simulation results are binary: `x.result.bin` replaces `x.result.json` (the bracket example's
  result is 1.7 MB instead of 7.1 MB), and `gcad check x.gsim` prints the readings of an up to date
  result. Old `.result.json` files are not read; rerun `gcad sim`. Remote jobs and the part and mesh
  caches are written with bitcode instead of CBOR.
- Simulation meshes are kept in `~/.cache/gcad/meshes`, so opening a result in the viewer no longer
  meshes the part again to draw it: the rotator yoke at `mesh 1` opens in 4 s instead of 66 s.
- A save during a build stops it and starts again with the new files, after a quarter second without
  further saves, so an agent editing many parts no longer queues a build per file.
- An assembly builds every part file below it, through all its sub-assemblies, in one parallel
  batch, and a part used by several sub-assemblies with the same variables is built once.
- Simulations give the same answer every run; the solver's dot products no longer depend on how
  threads split the work.
- `stress` and `safety` leave out one element size of each part around its supports and bonds,
  where a rigid hold is singular. A part smaller than the `mesh` size uses a sixth of its own
  diagonal, so a key or pin in a large assembly still counts towards `safety`.
- A probe just outside a coarse curved face reads on the mesh surface with a note, instead of failing.
- A `let` in an included file carries over to the file that includes it.
- Viewer shows a stale simulation result with a warning instead of hiding it.
- Viewer reloads when a result is written.

### Fixed
- After opening another file the viewer stopped following the old file's parts; it used to rebuild
  the new file whenever one of them changed.
- `split` failed on many solids, a plate split on `YZ` among them; it now cuts the other side
  away, and its `cut` group names the new face.
- Assemblies that hold each other stop at 16 levels with an error instead of crashing.
- The wind setup's air box is drawn in the wind colour and labelled with its size; it was a
  black line on the dark background. Viewer labels stay inside the view.
- STEP imports read each solid in its own length unit; inch vendor models came in 25.4 times too
  small.
- STEP surfaces of revolution with a circle profile, like the groove on KiCad electrolytic
  capacitors, import as tori, and curves on surfaces the kernel cannot convert import as cubic
  fits, instead of the solid becoming a box.
- STEP solids whose void shells lie outside them, as some vendor models write separate bodies,
  come in as one solid per body instead of a box. A cavity that cannot be read is left solid
  and counted in the line's summary.
- STEP assemblies that monstertruck cannot walk, like KiCad and agentee board exports, come in with
  every part in place instead of all at the origin. Faceted breps and surface models are built
  from their polygons, and a solid that still cannot be read becomes a box and is named in the
  line's summary, instead of failing the import.
- Parts with a face narrower than the kernel's chord tolerance, like a 0.01 mm step where two
  bores meet, mesh for simulation. The surface came out open at most sizes; it is now rebuilt
  with finer boundary polylines until it closes.
- A shaft extruded in several pieces of one radius meshes at the size asked. Its seams left
  flat elements, so the mesher retried down to a fraction of a millimetre (1.2 million
  elements and two minutes for a 190 mm shaft).
- Revolved parts whose profile runs along the axis mesh for simulation; the zero-area faces
  the axis edge leaves are skipped.
- Smooth lofts between sections with more than 240 matching edges.

## [0.3.1] - 2026-10-12

### Added
- Heat transfer in flow simulations, and ducts.
- Early stop for flow runs that have settled.
- OpenFOAM heatsink comparison.
- Wind example, with CFD pictures in the README.

### Changed
- Flow results are read asynchronously and stored as f16.
- Finer meshes for clash checks.

### Fixed
- Release artifact path.

## [0.3.0] - 2026-10-12

### Added
- Simulations: static stress, steady heat, natural frequencies, linear buckling,
  thermal expansion, torque, frictionless and Coulomb friction contact.
- Tetrahedral mesher with a mesh convergence check and per-part mesh sizes.
- Bonded assemblies in simulations, with ties that carry moment, pin supports
  and torque loads. Parts can be left out of a simulation.
- GPU lattice Boltzmann flow solver (`sim flow`) with inlets, outlets, forces,
  curved walls, sponge layers and Smagorinsky turbulence.
- Wind loads on parts and assemblies, static loads from a flow result, and an
  OpenFOAM runner that exports a simulation's parts.
- Viewer: results coloured by field, value under the cursor, loads and supports,
  3D streamlines, flow slices with playback of unsteady results, animated
  particles, part opacity.
- Browser build of the viewer, deployed to Cloudflare Pages.
- Material libraries, with MatML and FreeCAD imports.
- Sheet metal tabs, flanges, hems and flat patterns.
- Structural members and cut lists.
- Bosses, lips, grooves, vents and snap hooks.
- Bearings, dowels, standoffs, screw holes, standard parts and gear extras.
- Angle measurement and two-distance chamfers.
- `solid=` to pick a solid from a STEP import.
- Parts with shared sizes may share a part file.

### Changed
- Viewer is built on the egui_bench 3D viewer, with part selection, ghosting,
  zoom and orbit about the cursor and screen-width edges.
- Coplanar faces are fused after booleans.
- Faster interference sweeps and part prefetch.
- Models build on web worker threads in the browser.

### Fixed
- Cuts only remove material below the sketch plane (#1).
- Overlap is estimated when a boolean fails (#2).
- Concentric circles are fitted about their mean (#3).
- Cuts grow off round walls (#5).
- STEP imports are healed (#6).

## [0.2.0] - 2026-10-08

### Added
- Modelled ISO metric threads that run out into chamfers and countersinks.

### Changed
- Fillets at corners and hole rims are built together, with exact end arcs and
  finer round fillets.
- Shells follow fillets.
- Exact lines and circles on boolean edges, and smooth STEP intersection curves.

### Fixed
- Inside corners where three edges meet are rounded.
- Fillets that leave the solid unchanged fail instead of passing silently.
- Edge gaps are checked in STEP output.

## [0.1.0] - 2026-10-08

First release.

### Added
- Line-per-operation part files (`.gcad`) with sketches, constraints, extrudes,
  revolves, sweeps, lofts, shells, drafts, ribs, holes, fillets, chamfers,
  patterns, mirrors, booleans, include, `if` and `--set`.
- Involute spur and internal gears.
- Assemblies (`.gasm`) with mates, joints, closed joint loops, couples,
  patterns, poses, exploded views and interference checks.
- Materials, mass and bills of materials.
- Export to STEP (with colours and assembly structure), STL, 3MF, OBJ and SVG
  drawings with section views, dimensions, parts lists and balloons.
- DXF and SVG import.
- egui viewer with a view cube, measuring, section plane, orthographic camera
  and joint sliders.
- Release builds, install script and agent skill.

[Unreleased]: https://github.com/v0l/gcad/compare/v0.4.0...HEAD
[0.5.0]: https://github.com/v0l/gcad/compare/v0.3.1...v0.5.0
[0.4.0]: https://github.com/v0l/gcad/compare/v0.3.1...2cb7d8f
[0.3.1]: https://github.com/v0l/gcad/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/v0l/gcad/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/v0l/gcad/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/v0l/gcad/releases/tag/v0.1.0
