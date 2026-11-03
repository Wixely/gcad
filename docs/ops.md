# Operations and their tests

Every operation a parametric CAD program is expected to have, whether gcad has it,
the line that does it, and the tests that prove it. Each test builds a part and checks
it against a volume or bounding box worked out by hand.

A `missing` row's tests are already written with the syntax the operation should take
and are marked `#[ignore = "missing: ..."]`. Building the operation means making those
tests pass and flipping the row to `supported`. `tests/ops_doc.rs` fails if this table
and the tests disagree: a test not listed here, a listed test that does not exist, a
`supported` row whose test is ignored as missing, or a `missing` row whose test runs.

Run everything with `cargo test --release`. Tests that need a GPU (`flow`, `simulation::wind_*`)
are ignored unless you add `--features gpu-tests`. The OpenCascade check needs a Python with
OCP: `GCAD_OCP_PYTHON=~/git/cadkit/.venv/bin/python cargo test --release -- --ignored step_opens`.

## Sketch

| operation | status | syntax | tests |
|---|---|---|---|
| rectangle | supported | `rect 30 20 at=10,-5` | `sketch::rect`, `sketch::rect_offset_centre` |
| rounded rectangle | supported | `rect 30 20 r=4` | `sketch::rounded_rect` |
| circle | supported | `circle 10 at=x,y` | `sketch::circle` |
| polygon | supported | `poly 0,0 30,0 0,20` | `sketch::polygon`, `sketch::concave_polygon` |
| holes in a profile | supported | a profile drawn inside another | `sketch::profile_inside_profile_is_a_hole` |
| several profiles | supported | profiles side by side | `sketch::separate_profiles` |
| regular polygon | supported | `ngon 20 6` | `sketch::regular_polygon` |
| slot | supported | `slot 30 10` | `sketch::slot` |
| involute spur gear | supported | `gear 20 2 at=x,y` | `sketch::involute_gear` |
| internal gear | supported | `circle 80` then `gear 54 1 internal` | `sketch::internal_gear` |
| rack | supported | `rack 10 2 back=3` | `sketch::rack` |
| ellipse | supported | `ellipse 20 10` | `sketch::ellipse` |
| lines and arcs | supported | `pen`, `line`, `arc via=`, `close` | `sketch::lines_and_arcs` |
| spline | supported | `spline x,y ... closed` | `sketch::spline` |
| text | supported | `text GCAD size=10` | `sketch::text` |
| polygon with rounded corners | supported | `poly ... r=4` | `sketch::rounded_polygon` |
| polygon with chamfered corners | supported | `poly ... c=2` | `sketch::chamfered_polygon` |
| offset outline | supported | `offset 2` | `sketch::offset_outline` |
| arc by centre | supported | `arc x,y center=x,y` | `sketch::arc_by_centre` |
| parallel, perpendicular and equal | supported | `parallel a b c d` | `sketch::rectangle_from_constraints` |
| midpoint, point on a line, coincident | supported | `midpoint m a b`, `online p a b`, `coincident a b` | `sketch::square_with_midpoint_and_online` |
| constraints and dimensions | supported | `point`, `dist`, `horizontal` | `sketch::constraints` |
| trim and extend | supported | `close trim` | `sketch::trim` |
| construction geometry | supported | `circle 20 construct` | `sketch::construction_geometry` |
| mirror inside a sketch | supported | `reflect y` | `sketch::sketch_mirror` |
| pattern inside a sketch | supported | `array count=4 angle=360` | `sketch::sketch_pattern` |
| DXF import | supported | `dxf file.dxf` | `sketch::dxf_import` |
| SVG import | supported | `svg file.svg` | `sketch::svg_import` |


## Workplanes

| operation | status | syntax | tests |
|---|---|---|---|
| XY, XZ, YZ | supported | `plane XZ` | `workplane::xy`, `workplane::xz`, `workplane::yz` |
| offset plane | supported | `plane XY offset=10` | `workplane::offset` |
| plane on a face | supported | `plane base.end` | `workplane::on_a_face`, `workplane::side_face_keeps_world_coordinates` |
| rotated plane | supported | `plane XY rx=90` | `workplane::rotated` |
| plane through three points | supported | `plane x,y,z x,y,z x,y,z` | `workplane::through_three_points` |
| plane at an angle to an edge | supported | `plane edge=a&b angle=30` | `workplane::at_an_angle_to_an_edge` |
| plane normal to a path | supported | `plane path at=0.5` | `workplane::normal_to_a_path` |
| datum axis | supported | `axis name x,y,z x,y,z` | `workplane::datum_axis` |


## Features

| operation | status | syntax | tests |
|---|---|---|---|
| extrude | supported | `extrude 5`, `extrude -5` | `features::extrude`, `features::extrude_reversed` |
| extrude with draft | supported | `extrude 10 draft=5` | `features::extrude_draft` |
| symmetric extrude | supported | `extrude 10 both` | `features::extrude_symmetric` |
| extrude up to a face | supported | `extrude upto=base.end` | `features::extrude_up_to_face` |
| cut | supported | `cut 4`, `cut thru` | `features::cut_blind`, `features::cut_through` |
| cut with draft | supported | `cut 4 draft=3` | `features::cut_draft` |
| cut from a sketch inside the solid | supported | sketch plane below the top, or beside a taller block | `features::cut_from_inside_a_cylinder`, `features::cut_beside_a_taller_block` |
| revolve | supported | `revolve 360 axis=y`, `revolve 90` | `features::revolve_full`, `features::revolve_partial` |
| sweep along a path | supported | `path x,y,z ... r=8` then `sweep` | `features::sweep_bent_path`, `features::sweep_straight_path` |
| sweep along a helix | supported | `helix r=10 pitch=5 turns=3` then `sweep` | `features::sweep_helix` |
| loft | supported | `section` per profile, then `loft` | `features::loft`, `features::loft_many_matching_edges` |
| shell | supported | `shell 2 open=base.end` | `features::shell` |
| face draft | supported | `draft 5 base.side neutral=base.start` | `features::face_draft` |
| push or pull a face | supported | `push base.end 2` | `features::push_face` |
| revolve cut | supported | `revolve 360 mode=cut` | `features::revolve_cut` |
| sweep cut | supported | `sweep mode=cut` | `features::sweep_cut` |
| loft cut | supported | `loft mode=cut` | `features::loft_cut` |
| thin extrude | supported | `extrude 10 thin=2` | `features::thin_extrude` |
| extrude to an offset from a face | supported | `extrude upto=face offset=-2` | `features::extrude_up_to_offset_face` |
| extrude to the next face | supported | `extrude next` | `features::extrude_to_next_face` |
| rib | supported | open `pen` path, then `rib 2` | `features::rib` |
| smooth loft | supported | `loft smooth` | `features::smooth_loft` |
| loft between different profiles | supported | `section`s with different edge counts | `features::loft_mixed_profiles` |
| smooth loft with many edges | supported | `loft smooth` between sections of over 240 edges | `features::loft_many_matching_edges` |
| sweep along a smooth path | supported | `path ... smooth` | `features::sweep_smooth_path` |
| sweep with twist | supported | `sweep twist=90` | `features::sweep_twist` |
| sweep with scale | supported | `sweep scale=0.5` | `features::sweep_scale` |
| twisted or tapered extrude | supported | `extrude 10 twist=90`, `extrude 10 scale=0.5` | `features::extrude_twisted`, `features::extrude_scaled`, `features::twisted_extrude_with_a_hole` |
| helical gear | supported | `gear 20 2 helix=20` then `extrude 10` | `features::helical_gear` |
| bevel gear | supported | `gear 20 2 cone=45` then `extrude 5` | `features::bevel_gear` |
| modelled thread | supported | `thread M6 on=rod.side`, `thread M6 on=h.side` in a hole, `left`, `pitch=` | `features::modelled_thread`, `features::tapped_thread`, `features::thread_up_to_a_bolt_head`, `features::thread_on_an_undersized_rod_with_a_fine_pitch`, `features::thread_turns_right_handed`, `features::thread_needs_a_round_face_that_fits` |
| thread into a chamfer or countersink | supported | `chamfer 1 rod.end&rod.side` then `thread M6 on=rod.side` | `features::thread_runs_out_into_chamfers`, `features::left_hand_thread_runs_out_into_a_chamfer`, `features::tapped_thread_under_a_countersink` |
| shell with several openings | supported | `shell 2 open=a.end,a.start` | `features::shell_two_openings` |
| shell of any solid | supported | `shell 1` on a revolve | `features::shell_revolved` |
| shell after rounding | supported | `fillet 3 all` then `shell 2 open=base.end` | `features::shell_follows_rounded_corners`, `features::shell_as_thick_as_the_rounding`, `features::shell_follows_a_rounded_floor` |
| draft curved faces | supported | `draft 5 base.side` on a cylinder | `features::draft_curved` |
| push in with curved sides | supported | `push base.end -2` on a cylinder | `features::push_curved` |
| wrap a sketch onto a face | supported | `wrap base.side depth=0.5 [raise]` | `features::wrap_text`, `features::wrap_rectangle` |
| thicken a face | supported | `thicken base.side 1` | `features::thicken_face` |
| union with coplanar faces | supported | same-size box on a face | `features::stack_same_size` |
| cut with coplanar faces | supported | notch flush with the sides | `features::notch_flush_with_sides` |
| cut with a wall at the start of a corner fillet | supported | `cut` pocket whose wall sits on a rounded rectangle's fillet tangent | `features::cut_where_the_tool_wall_meets_a_corner_fillet_start` |
| cut with a circle on a round wall | supported | `circle D` on a ring of diameter D, or on its bore | `features::cut_on_an_outer_wall`, `features::cut_on_a_bore` |
| coplanar faces fused after a boolean | supported | a boss flush with a wall leaves one face | `inspect::flush_unions_leave_one_face_per_plane` |
| union flush with some sides | supported | step on top, flush with three sides | `features::step_flush_with_sides` |


## Holes

| operation | status | syntax | tests |
|---|---|---|---|
| through hole | supported | `hole 4 x,y ...` | `holes::through`, `holes::several` |
| blind hole | supported | `hole 4 0,0 depth=5` | `holes::blind` |
| counterbore | supported | `hole 3.2 0,0 cbore=6,3` | `holes::counterbore` |
| countersink | supported | `hole 3.2 0,0 csink=6.4,90` | `holes::countersink` |
| tapped hole | supported | `hole 3.3 0,0 thread=M4` | `holes::threaded` |
| angled hole | supported | `plane >Z rx=30` then `hole` | `holes::angled` |
| hole on a curved face | supported | `hole 4 0,10 on=rod.side` | `holes::on_a_curved_face` |
| exact circles on hole rims | supported | any `hole` or round `cut` | `holes::hole_rims_are_exact_circles` |
| clearance hole for a screw | supported | `hole M3 x,y`, `fit=close\|normal\|loose` | `holes::clearance_for_a_screw` |
| tap drill and heat-set insert holes | supported | `hole M4 x,y fit=tap`, `hole M3 x,y fit=insert` | `holes::tapped_and_insert_holes_for_a_screw` |
| counterbore or countersink for a screw | supported | `cbore=M3`, `csink=M4` | `holes::counterbore_and_countersink_for_a_screw` |
| hole fits | supported | `hole 6 x,y fit=H7` | `holes::hole_fit_limits`, `output::drawing_notes_fits_and_taps` |


## Enclosures

| operation | status | syntax | tests |
|---|---|---|---|
| lip on a rim | supported | `lip >Z 3 w=1` | `features::lip_on_a_rim` |
| groove on a rim | supported | `groove <Z 3 w=1 gap=0.2` | `features::groove_on_a_rim` |
| screw boss | supported | `boss 7 20 x,y ... hole=M3 fit=insert ribs=4` | `features::screw_boss`, `features::boss_for_an_insert` |
| vents through one wall | supported | `vent 20 3 x,y count=3 step=0,-5` | `features::vents_through_one_wall` |
| snap hook | supported | `snap 8 1.5 6 x,y hook=1.5 dir=90` | `features::snap_hook` |
| case and lid with screws | supported | `examples/assemblies/case.gasm` | `assembly::case_and_lid_with_screws` |

## Sheet metal

| operation | status | syntax | tests |
|---|---|---|---|
| sheet thickness, bend radius and K-factor | supported | `sheet 1.5 r=1.5 k=0.44` | `features::sheet_flange` |
| base tab | supported | `rect 40 30` then `base: tab` | `features::sheet_flange` |
| edge flange | supported | `flange base.end&>X 10`, `angle=` | `features::sheet_flange`, `features::sheet_flange_at_an_angle` |
| shorter flange and hem | supported | `flange e 10 inset=5`, `inset=2,8`, `angle=180` | `features::sheet_flange_inset_and_hem` |
| flange on a flange | supported | `flange >X&walls.end 5` | `features::sheet_flange_on_a_flange` |
| flat pattern | supported | `unfold`, holes and through cuts carried over | `features::sheet_unfolds` |
| bend a flat blank | supported | `bend a,b c,d angle=-90`, labels carried over, unfolds to the blank | `features::sheet_bends_a_flat_blank` |
| blank of any outline | supported | `slot` or arcs and holes, then `tab`, `bend`; DXF from the blank | `features::sheet_blank_with_any_outline` |
| pressed darts | supported | `dart fold -7 7 l=20 w=8` | `features::sheet_darts_stiffen_a_bend` |
| flat pattern DXF | supported | `gcad export part.gcad flat.dxf` | `output::flat_pattern_dxf` |

## Weldments

| operation | status | syntax | tests |
|---|---|---|---|
| structural members | supported | `member tube 40x40x3 x,y,z x,y,z ...`, `bar`, `pipe`, `rod`, `angle`, `channel` | `features::structural_members`, `features::member_materials`, `features::member_pattern_preserves_library_material` |
| member patterns | supported | `member bar 44x175 ... material=c24 count=19 step=400,0,-10`, `end_step=`, `when=`, `count=8 angle=360 axis=hub` | `features::linear_member_pattern`, `features::circular_member_pattern`, `features::member_row_tapers_and_skips` |
| mitred frame and cut list | supported | `member tube 40x40x3 ... closed`, `gcad bom` | `features::mitred_frame` |

## Fillets and chamfers

| operation | status | syntax | tests |
|---|---|---|---|
| fillet one edge | supported | `fillet 1 base.end&>Y` | `blends::fillet_edge` |
| fillet a chain with corners | supported | `fillet 1 base.end&base.side` | `blends::fillet_open_chain`, `blends::fillet_closed_chain_with_corners` |
| fillet a smooth chain | supported | rounded or circular edges | `blends::fillet_smooth_chain`, `blends::fillet_cylinder_rim` |
| fillet parallel edges | supported | `fillet 3 base.side&base.side` | `blends::fillet_vertical_edges` |
| fillet every edge of a flat-faced convex solid | supported | `fillet 2 all` | `blends::fillet_every_edge_of_a_box`, `blends::fillet_every_edge_of_a_prism` |
| fillet some of the edges where three meet | supported | `fillet 2 base.end&base.side\|base.side&base.side` | `blends::fillet_some_edges_at_a_corner`, `blends::fillet_three_edges_at_one_corner` |
| variable radius fillet | supported | `fillet 1 edges to=2` | `blends::fillet_variable_radius` |
| chamfer | supported | `chamfer 1 base.end&base.side` | `blends::chamfer_closed_chain`, `blends::chamfer_hole_rim` |
| chamfer with two distances | supported | `chamfer 1 edges d2=2` | `blends::chamfer_two_distances` |
| chamfer where three edges meet | supported | `chamfer 2 all` | `blends::chamfer_every_edge_of_a_box` |
| round corners on curved faces | supported | `fillet 1 all` on a rounded rect | `blends::fillet_every_edge_of_a_rounded_prism` |
| fillet every edge of a plate with holes | supported | `fillet 1 all` after `hole` | `blends::fillet_every_edge_of_a_plate_with_holes` |
| round inside corners where three edges meet | supported | `fillet 1 all` on an L shape, a pocket or a boss | `blends::fillet_every_edge_of_an_l_shape`, `blends::fillet_every_edge_of_a_pocket`, `blends::fillet_every_edge_of_a_boss`, `blends::fillet_inside_edge_ending_at_a_sharp_corner`, `blends::fillet_pocket_floor_with_sharp_walls`, `blends::chamfer_every_edge_of_an_l_shape` |
| variable radius along a chain | supported | `fillet 1 chain to=2` | `blends::fillet_variable_chain` |
| full round | supported | `fillet full base.end` | `blends::full_round` |


## Bodies

| operation | status | syntax | tests |
|---|---|---|---|
| union | supported | `extrude` onto existing material | `bodies::union`, `features::boss_glued_onto_a_face`, `features::boss_on_a_twisted_face`, `features::boss_over_an_edge_still_joins` |
| subtract | supported | `cut` | `bodies::subtract`, `bodies::thru_cut_across_thin_sheet_rows` |
| intersect | supported | `extrude 10 mode=intersect` | `bodies::intersect` |
| mirror | supported | `mirror YZ` | `bodies::mirror` |
| linear pattern | supported | `repeat h count=4 step=10,0` | `bodies::linear_pattern` |
| circular pattern | supported | `repeat h count=6 angle=360` | `bodies::circular_pattern` |
| move | supported | `move 10,0,0` | `bodies::translate` |
| rotate | supported | `rotate 90 axis=z` | `bodies::rotate`, `assembly::a_transform_in_a_later_body_keeps_earlier_bodies_groups` |
| scale | supported | `scale 2` | `bodies::scale` |
| split | supported | `split XY offset=5 keep=below` | `bodies::split`, `bodies::split_either_side` |
| separate bodies | supported | `body second` | `bodies::separate_bodies` |
| booleans between bodies | supported | `combine main b mode=cut` | `bodies::combine_bodies` |
| point separate bodies at an assembly | supported | `gcad check` on a `.gcad` left with two bodies | `bodies::check_sends_separate_parts_to_an_assembly` |
| mirror one feature | supported | `mirror YZ of=boss` | `bodies::mirror_feature` |
| pattern along a path | supported | `repeat h along=path count=4` | `bodies::pattern_along_path` |
| move or rotate a copy | supported | `move 20,0,0 copy` | `bodies::transform_copy` |
| scale unevenly | supported | `scale 2,1,1` | `bodies::scale_unevenly` |
| place one body on another | supported | `place lid on=>Z` | `bodies::assembly_mate` |
| interference check | supported | `measure overlap main b` | `bodies::interference` |

## Meshes

A mesh body is triangles, for organic shapes and game assets. It is blocked out from
blended primitives, turned into a mesh by `remesh`, and sculpted with `select` and the ops
after it. Each test checks a volume or bounding box worked out by hand.

| operation | status | syntax | tests |
|---|---|---|---|
| ellipsoid blob | supported | `head: blob 0,0,30 r=5,4,6 rz=20` | `mesh::blob_sphere`, `mesh::blob_ellipsoid_turned` |
| limb through joints | supported | `arm: limb 0,0,0 0,0,20 r=3,2` | `mesh::limb_capsule` |
| smooth blend between primitives | supported | `blob ... blend=2` | `mesh::blend_fills_the_join` |
| carve a blob out | supported | `blob ... mode=cut` | `mesh::blob_cut` |
| mirror a blockout part | supported | `mirror x of=ear_l` | `mesh::mirror_blockout_names_the_other_side` |
| remesh to triangles | supported | `remesh 0.3` | `mesh::blob_sphere` |
| select a named part | supported | `select head` | `mesh::select_part_moves_only_that_part`, `mesh::select_unknown_part_lists_the_parts` |
| select a region with falloff | supported | `select sphere=x,y,z r=4 falloff=6` | `mesh::select_falloff_keeps_far_vertices_still` |
| select both sides | supported | `select ... mirror=x` | `mesh::select_mirror_edits_both_sides` |
| move, scale, rotate a selection | supported | `move 0,0,3`, `scale 1.2`, `rotate 30 axis=z` | `mesh::select_part_moves_only_that_part`, `mesh::scale_selection_about_its_centre`, `mesh::rotate_selection` |
| inflate along normals | supported | `inflate 1` | `mesh::inflate_grows_a_sphere` |
| smooth | supported | `smooth 10` | `mesh::smooth_rounds_a_box_edge` |
| refine a region | supported | `refine 0.1` | `mesh::refine_hits_the_edge_length_and_stays_closed`, `mesh::refine_only_inside_the_selection`, `mesh::refine_follows_the_curve` |
| crease a groove | supported | `crease x,y,z x,y,z depth=0.3 width=0.4` | `mesh::crease_cuts_a_groove_of_the_given_depth` |
| pinch toward a point | supported | `pinch 0.5 toward=x,y,z` | `mesh::pinch_draws_vertices_together_along_the_surface` |
| flatten onto a plane | supported | `flatten at=0,0,0 normal=z` | `mesh::flatten_onto_a_plane` |
| surface noise | supported | `noise 0.2 size=1 seed=3` | `mesh::noise_is_repeatable_and_bounded` |
| relax triangles | supported | `relax 10` | `mesh::relax_evens_triangles_without_shrinking` |
| decimate to a budget | supported | `decimate 5000`, `decimate 25%` | `mesh::decimate_to_a_budget_keeps_the_shape`, `mesh::decimate_by_share_leaves_unselected_triangles`, `mesh::decimate_keeps_part_labels` |
| a game-ready fox | supported | `examples/mesh/fox.gcad` | `mesh::fox_example_fits_a_game_budget` |
| import a mesh | supported | `import file.obj mesh` | `mesh::mesh_import_obj` |
| export a mesh with its parts | supported | `gcad export file.gcad out.obj` | `mesh::mesh_export_obj_keeps_parts` |
| mesh and solid bodies in one file | supported | `body figure` then `blob` | `mesh::mesh_and_solid_bodies_share_a_file`, `mesh::solid_ops_refuse_a_mesh_body` |
| a creature, blocked out and sculpted | supported | `examples/mesh/creature.gcad` | `mesh::creature_example` |

## Assemblies

Assemblies are `.gasm` files. They bring in the bodies of `.gcad` part files and join them.

| operation | status | syntax | tests |
|---|---|---|---|
| bring in a part file | supported | `part box box.gcad body=lid` | `assembly::parts_from_files`, `assembly::kept_parts_follow_their_files`, `assembly::kept_parts_rebuild_only_what_changed`, `stored::stored_parts_follow_their_files`, `stored::stored_sub_assemblies_follow_their_parts`, `stored::stored_meshes_come_back_the_same`, `assembly::a_newer_build_stops_the_older_one` |
| set a part's variables | supported | `part plate plate.gcad w=size` | `assembly::part_variables` |
| geometry stays in part files | supported | `rect` in a `.gasm` fails | `assembly::geometry_stays_in_parts`, `assembly::part_errors_name_the_file` |
| concentric mate | supported | `concentric pin:pin.side plate:hole.side near=x,y,z` | `assembly::concentric_pin_in_a_hole`, `assembly::concentric_picks_the_hole_near_a_point`, `assembly::concentric_far_from_the_origin` |
| flush mate | supported | `flush pin:pin.start plate:base.end offset=1` | `assembly::concentric_pin_in_a_hole` |
| parallel mate | supported | `parallel b:rod.end a:rod.end` | `assembly::parallel_and_angle_turn_parts`, `assembly::a_parallel_mate_stops_a_joint` |
| angle mate | supported | `angle lid:plate.end base:plate.end 30` | `assembly::parallel_and_angle_turn_parts` |
| distance mate | supported | `distance b:rod.side a:rod.side 50` | `assembly::distance_between_axes` |
| tangent mate | supported | `tangent rod:rod.side plate:plate.end` | `assembly::tangent_lays_a_rod_on_a_plate` |
| mates hold parts together | supported | a screw mated to lid and box stops the hinge | `assembly::a_screw_locks_the_hinge` |
| sub-assemblies | supported | `part kit boxed.gasm` keeps its joints, mates and axes; `move kit 50,0,0` moves it whole | `assembly::a_sub_assembly_keeps_its_mates`, `assembly::a_sub_assembly_moves_as_one`, `assembly::sub_assemblies_build_a_shared_part_once`, `assembly::assemblies_that_hold_each_other_fail` |
| check holes line up | supported | `aligned lid:screws.side box:pilot.side tol=0.05` | `assembly::holes_line_up` |
| turning joint | supported | `joint open box.lid box.main turn about=hinge at=-90` | `assembly::hinge_opens_the_lid` |
| sliding joint | supported | `joint pull chest.drawer chest.main slide along=0,-1,0 max=30` | `assembly::slide_moves_a_drawer` |
| cylindrical joint | supported | a `turn` and a `slide` on one axis | `assembly::a_rod_turns_and_slides_on_one_axis` |
| closed loops of joints | supported | a four-bar linkage follows its crank | `assembly::a_four_bar_linkage_follows_its_crank` |
| gears and racks | supported | `couple jb ja ratio=-0.5` | `assembly::coupled_joints_move_together`, `assembly::couple_losses_are_checked`, `assembly::gears_in_mesh_turn_without_touching`, `assembly::planetary_gears_turn_without_touching` |
| motors driving joints through gears | supported | `examples/robot/arm.gasm` | `assembly::robot_arm_example` |
| posing a joint | supported | `pose open -90` | `assembly::pose_moves_children` |
| check every pair of parts | supported | `interference none` | `assembly::clear_assembly`, `assembly::strict_interference_fails` |
| meshing gears may touch | supported | `couple ... module=2 gear=a pinion=b`, overlap under `module`³ allowed | `assembly::meshing_gears_may_touch` |
| pattern parts | supported | `pattern s1 holes=case.lid:screws.side`, `count=` `step=`/`angle=` | `assembly::pattern_copies_a_part_and_its_mates` |
| exploded view | supported | `explode lid 0,0,30`, `render --explode` | `assembly::exploded_views_move_parts_apart` |
| assembly drawing with parts list | supported | `gcad export top.gasm top.svg` | `output::assembly_drawing_lists_parts` |
| bill of materials | supported | `gcad bom top.gasm [--csv]` | `assembly::bill_of_materials_counts_parts` |
| sweep a joint for clashes | supported | `interference joint=open steps=8` | `assembly::sweep_finds_a_clash`, `assembly::sweep_keeps_reporting_parts_that_do_not_move` |
| standard screws, nuts, washers and inserts | supported | `part s1 shcs:M3x10`, `nut:M3`, `washer:M3`, `insert:M3` | `assembly::standard_parts_from_the_library` |
| bearings, dowels and standoffs | supported | `part b bearing:608`, `dowel:4x16`, `standoff:M3x10` | `assembly::bearings_dowels_and_standoffs` |
| vendor STEP assembly as parts | supported | `part box vendor.step` | `assembly::step_assembly_as_a_part` |
| STEP length units and board exports | supported | `part pcb board.step` with inch parts, faceted breps and surface models | `assembly::step_part_in_its_own_units`, `assembly::kicad_board_in_inches_and_surface_models`, `assembly::step_voids_outside_the_solid_are_parts` |

## Output

| operation | status | syntax | tests |
|---|---|---|---|
| STEP export | supported | `gcad export part.gcad part.step` | `output::step`, `output::step_opens_in_opencascade`, `output::assembly_step_keeps_multi_shell_parts` |
| STL export | supported | `gcad export part.gcad part.stl` | `output::stl` |
| simulation parts export | supported | `gcad export wind.gsim parts.stl` writes the parts the simulation keeps after `leave` | `output::simulation_parts_export` |
| PNG views | supported | `gcad render part.gcad part.png` | `output::png` |
| STEP import | supported | `import part.step`, `import vendor.step solid=2` | `output::step_import`, `output::step_import_from_opencascade`, `output::step_import_asks_which_solid`, `output::step_import_boxes_what_it_cannot_read`, `output::step_import_reads_a_revolved_circle`, `output::step_import_leaves_unreadable_cavities_solid` |
| STEP instances placed | supported | `import rig.step solid=4` puts a repeated part where the file places it | `output::step_import_places_instances` |
| OBJ export | supported | `gcad export part.gcad part.obj` | `output::obj` |
| 3MF export | supported | `gcad export part.gcad part.3mf` | `output::three_mf`, `output::three_mf_names_the_bodies` |
| SVG drawing | supported | `gcad export part.gcad part.svg` | `output::drawing` |
| drawing dimensions and hole callouts | supported | overall sizes, `4× ⌀3.2` | `output::drawing_dimensions_and_section` |
| drawing labels clear of the view | supported | dimension and callout labels do not overlap the part | `output::drawing_labels_sit_clear_of_the_view` |
| section view | supported | `gcad export part.gcad part.svg --section y=0` | `output::drawing_dimensions_and_section` |
| STL import | supported | `import part.stl` | `output::stl_import` |
| drawing sheets | supported | `.gdwg` file: `sheet`, `view`, `section`, `dims`, `dim`, `level`, `callout`, `note`, `cutlist`, exported with `gcad export x.gdwg x.pdf` | `output::drawing_sheets`, `output::drawing_view_that_does_not_fit` |
| panel nesting | supported | `nest panels_* stock=2400x600 gap=4` in a `.gdwg`, or `gcad nest file.gasm --stock 2400x600 --gap 4`: packs flat parts onto stock with guillotine cuts, picks the least-waste mix of stock sizes, keeps showing edges on factory-edged sides, reports waste and lists the cuts in order | `output::drawing_nests_panels_on_stock` |
| STEP colours | supported | `color red` | `output::step_colours` |
| see-through bodies in the viewer | supported | `color #9cc7d9 opacity=0.3`, `color glass red opacity=0.5` | `assembly::see_through_parts` |
| STEP assembly structure | supported | `gcad export top.gasm top.step` | `output::step_assembly`, `output::step_assembly_opens_in_opencascade` |


## Parameters and inspection

| operation | status | syntax | tests |
|---|---|---|---|
| variables and expressions | supported | `let w=40 h=w*3/4` | `inspect::variables` |
| math functions | supported | `let a=atan2(3,4) r=sqrt(x)` | `inspect::functions` |
| face and edge selectors | supported | `label.group`, `>Z`, `+X`, `all`, `a,b`, `a&b`, `x\|y` | `inspect::groups_and_edges`, `inspect::directional_faces` |
| groups survive later edits | supported | | `inspect::groups_survive_later_cuts` |
| errors that say what to do | supported | | `inspect::errors` |
| measure distance | supported | `measure faces faces` | `inspect::measure_distance` |
| measure an angle | supported | `measure angle faces faces` | `inspect::measure_angle` |
| mass properties | supported | `measure mass` | `inspect::mass_properties` |
| materials and mass | supported | `material steel`, `material pla density=1.24` | `inspect::material_mass`, `assembly::materials_follow_parts` |
| wall thickness | supported | `measure thickness` | `inspect::wall_thickness` |
| draft analysis | supported | `measure draft pull=z` | `inspect::draft_analysis` |
| bounding boxes | supported | `measure box`, `measure box pcb base.end` | `inspect::measure_boxes` |
| clearance between bodies | supported | `measure clearance pcb lid` | `inspect::measure_clearance`, `assembly::clearance_between_parts` |
| free space inside a case | supported | `measure space at=0,0,10`, `gcad measure case.gasm space` | `inspect::measure_space`, `assembly::clearance_between_parts` |
| variables from outside the file | supported | `gcad check part.gcad --set w=20` | `inspect::outside_variables` |
| include another file | supported | `include part.gcad d=10`; its new `let` names carry over | `inspect::include_file`, `inspect::include_brings_in_let` |
| conditional lines | supported | `if w>30 chamfer 1 edges` | `inspect::conditional` |

## Simulation

The plan for the rest is in `docs/simulation.md`.

| operation | status | syntax | tests |
|---|---|---|---|
| joint torque against gravity | supported | `sim torque`, `part arm.gasm`, `sweep shoulder elbow steps=24` | `simulation::torque_of_a_pendulum`, `simulation::torque_needs_joints` |
| torque through gears and racks | supported | a `couple` carries the load to the joint it drives | `simulation::torque_through_gears`, `simulation::robot_arm_motor_torque` |
| gear and worm losses | supported | `couple ... efficiency=0.9 back=0.8`, `couple ... lead=3.6 friction=0.1` | `simulation::torque_through_lossy_gears`, `simulation::worm_that_locks` |
| torque on a joint | supported | `torque az 6.3` in `sim torque` | `simulation::torque_on_a_joint` |
| gear mesh forces, stresses, power and heat | supported | `couple ... module=1 face=10 gear=wheel pinion=worm`, `speed az 1` in `sim torque` | `simulation::worm_drive_forces_stresses_and_heat`, `simulation::spur_gear_mesh_forces`, `simulation::couple_geometry_needs_module_and_face` |
| joint friction | supported | `joint ... drag=0.05` | `simulation::drag_on_joints` |
| payloads | supported | `payload hand 300 at=x,y,z` | `simulation::torque_of_a_pendulum`, `simulation::payload_follows_the_pose` |
| expectations | supported | `expect shoulder<2 drive>0.5` | `simulation::torque_through_gears` |
| stale results and charts | supported | `gcad check x.gsim`, `gcad render x.gsim x.png` | `simulation::result_goes_stale` |
| readings and sweeps in the viewer | supported | the simulation card lists readings and checks, traces each sweep, and runs the file | `simulation::torque_readings_without_a_field` |
| progress while a simulation runs | supported | `gcad sim x.gsim` prints each stage to stderr (`-q` for none); the viewer's simulation card shows bars while it runs | `progress::static_run_reports_its_stages` |
| setup before a result | supported | the viewer draws the mesh with its loads and supports, or the flow parts, duct, wind and heated faces | `simulation::setup_shown_before_a_result`, `simulation::flow_setup_shown_before_a_result` |
| material libraries | supported | `materials.toml` beside or above a file, `~/.config/gcad/materials.toml`, `gcad materials` | `simulation::material_library_is_found` |
| how a material looks | supported | `color`, `roughness`, `metallic`, `clearcoat`, `transmission`, `ior`, `emission` in `materials.toml`, for the viewer and its ray tracer | `simulation::material_library_sets_how_a_body_looks` |
| standard material library | supported | `material stainless-316`, `material c24`, `material pir`; `gcad materials` lists them all | `simulation::standard_library_covers_common_materials`, `simulation::standard_library_covers_construction_and_printing` |
| complete standard materials | supported | every standard material has density, `E`, `nu`, `yield`, `k`, `alpha`, `cp`, colour, roughness, metallic and a texture | `simulation::every_standard_material_is_complete` |
| specific heat | supported | `cp = 900` in `materials.toml`, `material steel cp=500` | `simulation::specific_heat_is_a_material_property` |
| material textures | supported | `texture = "wood"`, `texture_scale`, `texture_rotation`, `color_map`, `normal_map`, `roughness_map`, `metallic_map` in `materials.toml` | `simulation::project_library_inherits_and_textures` |
| material files from other tools | supported | `materials workbench.xml`, `materials PLA.FCMat` | `simulation::material_files_import` |
| materials along three axes | supported | `E1= E2= E3= Xt= Zt=` in `materials.toml`, simulated with the weakest | `simulation::directional_material_takes_the_weakest` |
| stiffness, strength and conductivity | supported | `material pla`, `material delrin density=1.41 E=3.1 nu=0.35 yield=65` | `simulation::material_properties` |
| tetrahedral mesh | supported | `mesh 2`, `gcad sim x.gsim --dry-run`, `gcad render x.gsim x.png` | `simulation::tet_mesh_of_a_box`, `simulation::tet_mesh_of_curved_parts`, `simulation::tet_mesh_of_example_parts`, `simulation::tet_mesh_of_thin_walls`, `simulation::tet_mesh_of_a_ring_gear_part`, `simulation::mesh_dry_run`, `simulation::revolved_profile_on_its_axis_meshes`, `simulation::tet_mesh_of_stacked_cylinders`, `simulation::tet_mesh_of_a_bore_with_a_tiny_step`, `simulation::slightly_flat_elements_keep_the_mesh_size` |
| static stress | supported | `sim static`, `fix faces`, `load faces force=x,y,z`; `stress` leaves out the band around supports | `simulation::bar_in_tension`, `simulation::cantilever_stress`, `simulation::peak_stress_leaves_out_the_fixed_edge`, `simulation::small_part_keeps_its_own_stress_band`, `simulation::static_needs_supports_and_stiffness` |
| sliding supports and pressure | supported | `slide <X`, `pressure h.side 10` | `simulation::bar_in_tension`, `simulation::thick_cylinder_under_pressure` |
| pins and torques | supported | `hinge: pin h.side`, `torque rod.end 10` | `simulation::shaft_in_torsion`, `simulation::torque_about_a_named_axis`, `simulation::pinned_link` |
| bearings | supported | `left: bearing seat.side axial`, `right: bearing seat2.side` | `simulation::bearings_carry_force_and_no_moment`, `simulation::bearings_need_one_held_along_the_axis` |
| bearings between parts | supported | `bearing kfl:bore.side spindle axial C0=2900` | `simulation::bearing_between_parts_carries_no_moment`, `simulation::bearing_between_parts_takes_thrust`, `simulation::bearing_row_must_touch_the_shaft` |
| leaving parts out | supported | `leave ctl* *screw*` | `simulation::leave_parts_out` |
| mesh convergence check | supported | `converge`, `converge within=2` | `simulation::mesh_convergence_check` |
| probes | supported | `top: probe 100,0,10`; a point on a curved face just outside the mesh reads on its surface | `simulation::cantilever_stress`, `simulation::thick_cylinder_under_pressure`, `simulation::probe_on_a_curved_face_snaps_to_the_mesh` |
| example simulations | supported | `examples/parts/bracket.gsim`, `examples/parts/heatsink.gsim` | `simulation::example_simulations` |
| large problems | supported | conjugate gradients with a two-level preconditioner, above 100,000 unknowns for stress and 150,000 for heat; LOBPCG above 250,000 for frequencies and buckling | `simulation::large_problems_iterate`, `simulation::large_heat_problem_solves_iteratively`, `simulation::large_eigenproblems_iterate` |
| stress plots | supported | `gcad render x.gsim x.png`; the viewer reads the value, movement and point under the cursor, and draws and lists every load and support with its total | `simulation::stress_field_render`, `simulation::field_reads_at_a_point` |
| steady heat | supported | `sim thermal`, `cold: temperature faces 20`, `heat faces 5`, `source 5` | `simulation::conduction_through_a_wall`, `simulation::heat_needs_a_boundary` |
| convection | supported | `air: convect faces h=20 ambient=20` | `simulation::fin_with_convection` |
| thermal expansion | supported | `warm 50`, `warm -100 part=bolt`, `warm heat.gsim from=20` | `simulation::thermal_expansion`, `simulation::expansion_from_a_heat_result`, `simulation::preload_by_cooling_one_part` |
| buckling | supported | `sim buckling`, `load >Z force=0,0,-1000` | `simulation::column_buckling` |
| bonded assemblies | supported | `part stack.gasm`, `fix a:<Z`, `load b:>Z force=0,0,1000` | `simulation::bonded_parts_carry_load`, `simulation::bonded_joint_carries_moment`, `simulation::large_bonded_problem_converges`, `simulation::floating_part_is_refused`, `simulation::bonded_parts_conduct_heat`, `simulation::bonded_parts_vibrate_as_one` |
| keeping parts apart | supported | `apart cover* wheel*` | `simulation::parts_kept_apart_do_not_bond_across_a_clearance` |
| contact | supported | `contact a b` | `simulation::contact_presses_but_lets_go`, `simulation::contact_vibrates_as_bonded` |
| friction | supported | `contact a b friction=0.3` | `simulation::friction_holds_until_it_slides` |
| natural frequencies | supported | `sim modal`, `modes 3` | `simulation::cantilever_modes`, `simulation::free_beam_modes` |
| wind loads | supported | `sim flow`, `wind 30 toward=1,0,0 about=0,0,0`, `mesh 5`, `average 40`, `precision half`, `air density=1.2 viscosity=1.5e-5` | `simulation::wind_on_a_cube`, `simulation::wind_in_half_precision` |
| wind pressure in a static simulation | supported | `wind flow.gsim` | `simulation::wind_loads_a_static_part` |
| air sealed inside parts | supported | a closed box in a `sim flow` is airtight even with walls thinner than a cell | `simulation::wind_seals_a_closed_box` |
| flow slices, streamlines and playback | supported | `gcad render wind.gsim flow.png --flow`, or `--lines` for 3D streamlines; the viewer draws 3D streamlines, particles along them and the slices moving through time, from `wind.frames.bin` | `simulation::wind_draws_the_flow` |
| heat carried off by the air | supported | `heat <Z 10` in a `sim flow`, `air temperature=25`, `duct 40 25`; reads the heated faces' temperature, the hottest part and the resistance in K/W; a spinning fan cools parts that stand still | `simulation::fan_cools_a_ducted_heatsink`, `simulation::flow_heat_lines_parse`, `simulation::a_spinning_fan_cools_a_heated_post` |
| fans and propellers | supported | `spin spin rpm=3000` in a `sim flow` turns the joint's part through the lattice; reads `spin.torque`, `spin.power`, `spin.thrust`, `spin.flow` | `simulation::a_spinning_fan_pushes_air`, `simulation::spin_needs_a_turn_joint_in_a_flow` |
| motor torque, cogging and back-EMF | supported | `sim magnetic`, `turn spin sweep=6 steps=12`, `magnet`, `iron material=m19`, `coil turns= current=` on a 2D slice | `simulation::a_wire_turns_a_magnet_from_parts`, `simulation::magnetic_lines_need_their_simulation` |
| beam members from solids | supported | `sim frame`, each body a straight prismatic member, `fix`, `pin`, `slide` on end faces, `load` on end or side faces, `gravity` | `simulation::frame_cantilever_matches_beam_theory`, `simulation::frame_beam_on_pins_carries_a_uniform_load`, `simulation::frame_portal_sways_as_slope_deflection_says`, `simulation::frame_joins_a_girder_to_the_middle_of_a_column`, `simulation::frame_members_may_slope`, `simulation::frame_asks_for_what_it_needs` |
| member end releases | supported | `release beam:<X` | `simulation::frame_releases_turn_a_fixed_beam_into_a_simple_one` |
| load combinations | supported | `combine uls weight:1.35 live:1.5`, readings `uls.stress`, `uls.base` | `simulation::frame_combinations_add_loads_by_their_factors` |
| frame modes and buckling | supported | `modes 3`, `buckling 2` in a `sim frame` | `simulation::frame_cantilever_rings_at_its_beam_frequency`, `simulation::frame_columns_buckle_at_eulers_load` |
| frame stress and deflection drawn | supported | `gcad render portal.gsim portal.png`, the viewer's simulation card | `simulation::frame_view_shows_the_deflected_members` |
| thermal bridges and wall U-values | supported | `sim conduction`, `convect` with one label per side, readings `coupling`, `L.a.b`, `fRsi`, `coolest` | `simulation::conduction_through_a_composite_wall`, `simulation::a_steel_pin_bridges_the_insulation`, `simulation::conduction_through_a_round_part`, `simulation::conduction_asks_for_what_it_needs` |
| room acoustics | supported | `sim acoustics`, `absorb all plaster`, `speaker x,y,z`, `hall: probe x,y,z`, readings `hall.T30`, `hall.EDT`, `hall.C80`, `hall.D50`, `hall.STI` | `simulation::shoebox_rings_between_eyring_and_sabine`, `simulation::absorption_shortens_the_reverberation`, `simulation::obstacles_take_volume_out_of_the_room`, `simulation::acoustics_needs_absorption_and_a_source`, `simulation::acoustic_view_draws_the_room` |
| example building simulations | supported | `examples/frame/portal.gsim`, `examples/conduction/tie.gsim`, `examples/acoustics/room.gsim` | `simulation::building_example_simulations` |
| building simulations on remote runners | supported | `gcad sim x.gsim --remote` packs frame and acoustics jobs like any other | `simulation::building_kinds_run_remotely`, `simulation::new_kinds_have_no_mesh_to_converge` |
| wind on an assembly | supported | `part pair.gasm` in a `sim flow` reads `front.drag`, `back.drag` | `simulation::wind_shelters_a_part_behind` |
