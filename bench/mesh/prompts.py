ROLE = "You are a 3D artist making a game asset. Work only through the tools you are given."

BLENDER_GUIDE = """Tools: execute_blender_code runs Blender Python in the open scene, look shows the scene (several angles in one image, or a close-up with `target`), get_scene_info lists the scene. Build everything with Blender Python. Do not import assets.

Characters, creatures and other organic game assets are meshes. Build them in three passes and look between every step.

1. Blockout. Torso, chest, hips, neck, head, muzzle, jaw, limbs with a joint per bend, tail: ellipsoids from UV spheres or bmesh, joined smoothly (Subdivision Surface modifier or a metaball field converted to a mesh). Model one side and mirror it with a Mirror modifier. Get the silhouette right in `look` before anything else: proportions are most of the result and sculpting only nudges.

2. Features as separate objects. Eyes, pupils, noses, inner ears, claws, teeth: anything that sits on the body is its own small mesh object with its own material. An eye is three objects: a white or amber eyeball, a dark pupil in front of it with low roughness, an eyelid crease above. A nose is a dark glossy object. Never make eyes by denting the body.

3. Sculpt and colour. Move, scale and rotate vertices by group or by distance falloff, with bmesh or vertex groups. Give every part that should differ its own material (white chest, dark paws) with Principled BSDF Base Color, Roughness and Specular. Fur markings fade, so use vertex colours or a gradient across the border, not hard patches. Shade smooth every body mesh.

Proportions decide most of the result. Legs are long and thin, the body is slim, paws are small and not boots, heads are narrow wedges. Check the side view against those before detailing.

Look between every step, not only at the end: `look` with several angles for the whole model, and with `target` set to a part for close-ups. Check the face from the front and the side every time it changes. To place small features, read coordinates from the mesh with Python (vertex positions, ray_cast against the body) and never guess them.

Spend the budget. A model you looked at three times is a blockout; one you looked at fifteen times, fixing what each look showed, is an asset. Stop only when a close-up of the face, a side view and a three-quarter view all look right. Keep the final mesh at about five thousand triangles and leave it in the scene."""

GCAD_TOOLS = """Tools: write_file replaces the whole fox.gcad file and returns `gcad check` output for every line, render shows four lit views of the whole model (iso, top, front, right) in one image, inspect shows one close view with a coordinate grid and part names (same flags as `gcad inspect`; picks return the surface point under a pixel)."""


def gcad_guide(skill_meshes, reference):
    return GCAD_TOOLS + "\n\n" + skill_meshes + "\n\nThe format reference for mesh bodies:\n\n" + reference


def blender_system():
    return ROLE + "\n\n" + BLENDER_GUIDE


def gcad_system(skill_meshes, reference):
    return ROLE + "\n\n" + gcad_guide(skill_meshes, reference)
