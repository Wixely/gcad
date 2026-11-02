extends SceneTree

# Structural gate only. Source expectations come from GLB JSON, independently
# of GLTFState; rendered material/deformation parity needs separate probes.
var errors: Array[String] = []
var meshes: Array[MeshInstance3D] = []
var players: Array[AnimationPlayer] = []
var skeletons: Array[Skeleton3D] = []

func require(condition: bool, message: String) -> void:
	if not condition:
		errors.append(message)

func walk(node: Node) -> void:
	if node is MeshInstance3D:
		meshes.append(node)
	if node is AnimationPlayer:
		players.append(node)
	if node is Skeleton3D:
		skeletons.append(node)
	for child in node.get_children():
		walk(child)

func finish(report: Dictionary) -> void:
	report["errors"] = errors
	report["passed"] = errors.is_empty()
	var file := FileAccess.open("res://import-report.json", FileAccess.WRITE)
	if file == null:
		quit(1)
		return
	file.store_string(JSON.stringify(report, "  "))
	file.close()
	quit(0 if errors.is_empty() else 1)

func _initialize() -> void:
	var report := {"version": 1, "godot": Engine.get_version_info(), "scope": "GLB structural import", "consumer_roles_applied": false}
	var data := FileAccess.get_file_as_bytes("res://model.glb")
	if data.size() < 20 or data.decode_u32(0) != 0x46546c67 or data.decode_u32(4) != 2 or data.decode_u32(8) != data.size() or data.decode_u32(16) != 0x4e4f534a:
		errors.append("Invalid GLB header")
		finish(report)
		return
	var length := data.decode_u32(12)
	if length > data.size() - 20:
		errors.append("Invalid GLB JSON length")
		finish(report)
		return
	var parsed = JSON.parse_string(data.slice(20, 20 + length).get_string_from_utf8())
	if not parsed is Dictionary:
		errors.append("Invalid GLB JSON")
		finish(report)
		return
	var source: Dictionary = parsed
	var document := GLTFDocument.new()
	var state := GLTFState.new()
	var status := document.append_from_file("res://model.glb", state)
	if status != OK:
		errors.append("GLTFDocument import failed: %s" % status)
		finish(report)
		return
	var scene := document.generate_scene(state, 30.0, false, false)
	if scene == null:
		errors.append("Scene generation failed")
		finish(report)
		return
	root.add_child(scene)
	walk(scene)
	var active: Dictionary = {}
	var pending: Array = source.scenes[int(source.get("scene", 0))].nodes.duplicate()
	var expected_meshes := 0
	var expected_surfaces := 0
	var expected_triangles := 0
	var expected_skins := 0
	while not pending.is_empty():
		var index := int(pending.pop_back())
		if active.has(index):
			continue
		active[index] = true
		var node: Dictionary = source.nodes[index]
		pending.append_array(node.get("children", []))
		if node.has("mesh"):
			expected_meshes += 1
			if node.has("skin"):
				expected_skins += 1
			for primitive in source.meshes[int(node.mesh)].primitives:
				require(int(primitive.get("mode", 4)) == 4, "Only triangle primitives are supported by this probe")
				expected_surfaces += 1
				var accessor := int(primitive.get("indices", primitive.attributes.POSITION))
				expected_triangles += int(source.accessors[accessor].count) / 3
	var surfaces := 0
	var triangles := 0
	var skins := 0
	for instance in meshes:
		require(instance.mesh is ArrayMesh, "Imported mesh is not an ArrayMesh")
		if not instance.mesh is ArrayMesh:
			continue
		var mesh := instance.mesh as ArrayMesh
		if instance.skin != null:
			skins += 1
			require(instance.get_node_or_null(instance.skeleton) is Skeleton3D, "Skinned mesh has no skeleton")
			require(instance.skin.get_bind_count() > 0, "Skin has no bind matrices")
		surfaces += mesh.get_surface_count()
		for surface in range(mesh.get_surface_count()):
			require(mesh.surface_get_primitive_type(surface) == Mesh.PRIMITIVE_TRIANGLES, "Imported primitive is not triangles")
			var arrays := mesh.surface_get_arrays(surface)
			var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX] if arrays[Mesh.ARRAY_INDEX] != null else PackedInt32Array()
			var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
			triangles += (indices.size() if not indices.is_empty() else vertices.size()) / 3
			for vertex in vertices:
				require(vertex.is_finite(), "Nonfinite imported vertex")
	var animations: Array[String] = []
	for player in players:
		for name in player.get_animation_list():
			animations.append(String(name))
	for animation in source.get("animations", []):
		var name := String(animation.get("name", ""))
		require(not name.is_empty() and animations.has(name), "Missing named clip: " + name)
	require(meshes.size() == expected_meshes and expected_meshes > 0, "Mesh instance count differs")
	require(surfaces == expected_surfaces, "Surface count differs")
	require(triangles == expected_triangles and triangles > 0, "Triangle count differs")
	require(skins == expected_skins, "Skinned instance count differs")
	report.merge({"mesh_instances": meshes.size(), "surfaces": surfaces, "triangles": triangles, "skinned_instances": skins, "skeletons": skeletons.size(), "animations": animations, "expected_mesh_instances": expected_meshes, "expected_surfaces": expected_surfaces, "expected_triangles": expected_triangles, "expected_skinned_instances": expected_skins})
	scene.free()
	finish(report)
