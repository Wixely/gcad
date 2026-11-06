# Collision

A `collision` line tells a game which mesh nodes physics should use and what shape to give
them. It does not name an engine, so any consumer can read it. The Unity adapter's `collider`
role in the [consumer profile](game-assets-consumer-profile.md) is separate and unchanged.

```text
collision NODE shape=box|sphere|capsule|hull|mesh [render=yes|no] [max_vertices=N]
          [surface=NAME] [friction=F] [restitution=R]
```

`NODE` is a mesh node. A node takes at most one declaration; give a node several shapes by
adding child nodes, each with its own. See `examples/game-assets/plate-collision.gasset`.

| shape | What the consumer gets |
| --- | --- |
| `box` | The node's mesh bounds, as a centre and half extents on the node's axes. |
| `sphere` | A sphere round the bounds centre that holds every vertex. |
| `capsule` | A capsule along the longest bounds axis that holds every vertex. |
| `hull` | The convex hull of the mesh, kept to `max_vertices` points (default 64). |
| `mesh` | The node's own triangles. |

gcad fits box, sphere, capsule and hull shapes itself, so every consumer gets the same shape.
A hull adds the farthest vertex outside it until it has `max_vertices` points, so a capped
hull can leave the mesh's tips slightly outside. The viewer and the export report that
distance as `error`. With 64 points a hull has at most 124 triangles, which fits engines that
allow 128 hull vertices and faces. Raise the cap only for an engine that allows more.

`render` defaults to `no`: the node is collision only and a game should not draw it. Use
`render=yes` to give a visible mesh a collision shape, such as a box round a crate. `surface`
is a free-form name for footstep sounds, decals or a physics material. `friction` is zero or
more and `restitution` is 0 to 1; gcad stores both and assumes no engine's defaults.

## Animation

A shape belongs to its node. A clip that moves or turns the node, or any of its parents, moves
the shape with it. In an engine this is a kinematic body set to the node's world transform
each step; an engine that turns a target transform into velocities keeps contacts with
pushed bodies stable.

Two things a physics shape cannot follow are refused when the asset loads:

- Scale. Engines bake scale into a shape when they build it, so a clip that scales a
  collision node or one of its parents is refused. Box, sphere and capsule shapes also need
  a transform without shear, and sphere and capsule shapes need even scale. A hull or mesh
  takes any fixed scale.
- Skinning. A skinned mesh's triangles move with its joints, so the mesh node of a skin
  cannot take a collision declaration. Add a child node under each joint that needs one,
  usually with a capsule or box, and it follows that bone rigidly.

Engines generally keep triangle-mesh shapes on static or kinematic bodies. A prop that falls
or is thrown needs a box, sphere, capsule or hull.

## Export

Collision-only nodes stay in the GLB as ordinary mesh nodes, so the shape shares its node's
transform and animation channels and nothing has to be kept in step with a second file. Each
declared node carries `extras.gcad_collision`:

```json
{"version":1,"shape":"hull","render":false,"length_unit":"metre","up_axis":"Y",
 "points":[[0.02,0.0,0.015],...],"error":0.0004,"surface":"stone","friction":0.8}
```

Coordinates are in the node's glTF space, the same metres and Y up as its mesh. A box has
`centre` and `half_extents`, a sphere `centre` and `radius`, a capsule `a`, `b` and `radius`,
a hull `points` and `error`, and a mesh `triangles`, the count of its node's mesh triangles.
A glTF viewer that ignores extras draws collision-only nodes; a game reads `render` to skip
them.

## Viewer

When an asset has collision declarations, the controls show Visual, Both and Collision:

- Visual draws what a game draws, so collision-only nodes are hidden. Snapshots and sprite
  sheets use this view.
- Both draws every node, with fitted shapes see-through on top.
- Collision draws only what physics sees, in one flat colour: fitted shapes, and the meshes
  of `mesh` declarations. Nodes with no declaration are hidden.

Below the mode, each declaration is listed with its shape, a hull's point count and error, or
a mesh's triangle count. Clicking a fitted shape selects its node, and **Copy reference**
adds a `collision` object with the shape, `render`, and whether the click hit the fitted
shape or the mesh. Face selection does not apply to fitted shapes because they have no source
faces.
