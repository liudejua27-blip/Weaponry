extends SceneTree

const SOCKET_NAMES := [
    "forgecad-anchor-grip-primary",
    "forgecad-anchor-socket-energy-core-vfx",
    "forgecad-anchor-socket-magazine-well",
    "forgecad-anchor-socket-muzzle-vfx",
    "forgecad-anchor-socket-sight-primary",
    "forgecad-anchor-weapon-root",
]

const SCENES := [
    {"role": "lod0", "path": "res://assets/weapon-lod0.glb"},
    {"role": "lod1", "path": "res://assets/weapon-lod1.glb"},
    {"role": "lod2", "path": "res://assets/weapon-lod2.glb"},
    {"role": "animated", "path": "res://assets/weapon-animated.glb"},
]

var failures: Array[String] = []


func _initialize() -> void:
    call_deferred("_run")


func _run() -> void:
    var socket_expectations = JSON.parse_string(
        FileAccess.get_file_as_string("res://assets/socket-expectations.json")
    )
    var collision_sidecar = JSON.parse_string(
        FileAccess.get_file_as_string("res://assets/collision-proxy-set.json")
    )
    var animation_samples = JSON.parse_string(
        FileAccess.get_file_as_string("res://assets/socket-animation-samples.json")
    )
    if typeof(socket_expectations) != TYPE_ARRAY:
        _fail("socket expectations are not an array")
        socket_expectations = []
    if typeof(collision_sidecar) != TYPE_DICTIONARY:
        _fail("collision sidecar is not an object")
        collision_sidecar = {}
    if typeof(animation_samples) != TYPE_DICTIONARY:
        _fail("animation samples are not an object")
        animation_samples = {}

    var reports: Array[Dictionary] = []
    for scene_spec in SCENES:
        reports.append(_inspect_scene(scene_spec, socket_expectations, animation_samples))

    var triangle_counts: Array[int] = []
    var material_signatures: Array[String] = []
    for report in reports:
        if report["role"] != "animated":
            triangle_counts.append(report["triangle_count"])
            material_signatures.append("|".join(report["material_names"]))
    if triangle_counts.size() != 3 or not (
        triangle_counts[0] > triangle_counts[1] and triangle_counts[1] > triangle_counts[2]
    ):
        _fail("LOD triangle counts are not strictly decreasing")
    if material_signatures.size() != 3 or not (
        material_signatures[0] == material_signatures[1]
        and material_signatures[1] == material_signatures[2]
    ):
        _fail("LOD material signatures differ")

    var collision_report := _inspect_collision(collision_sidecar)
    var report := {
        "schema_version": "GodotGameWeaponImportProbeReport@1",
        "godot_version": Engine.get_version_info()["string"],
        "import_mode": "godot-headless-editor-gltf-glb-import@1",
        "scenes": reports,
        "lod_triangle_counts": triangle_counts,
        "lod_strictly_decreasing": triangle_counts.size() == 3 and (
            triangle_counts[0] > triangle_counts[1] and triangle_counts[1] > triangle_counts[2]
        ),
        "lod_material_signatures_exact": material_signatures.size() == 3 and (
            material_signatures[0] == material_signatures[1]
            and material_signatures[1] == material_signatures[2]
        ),
        "collision": collision_report,
        "actual_godot_headless_import": failures.is_empty(),
        "actual_engine_roundtrip": failures.is_empty(),
        "commercial_engine_roundtrip": false,
        "unity": "NOT_RUN",
        "unreal": "NOT_RUN",
        "candidate_confirmed": false,
        "export_performed": false,
        "human_review": "NOT_RUN",
        "visual_quality": "NOT_RUN",
        "quality_status": "structural_only",
        "failures": failures,
    }
    print(JSON.stringify(report))
    quit(0 if failures.is_empty() else 1)


func _inspect_scene(
    scene_spec: Dictionary, socket_expectations: Array, animation_samples: Dictionary
) -> Dictionary:
    var packed := ResourceLoader.load(
        scene_spec["path"], "PackedScene", ResourceLoader.CACHE_MODE_IGNORE
    ) as PackedScene
    if packed == null:
        _fail("Godot did not load " + scene_spec["path"])
        return {"role": scene_spec["role"], "load": "FAIL"}
    var instance := packed.instantiate()
    get_root().add_child(instance)
    var all_nodes: Array[Node] = []
    _collect_nodes(instance, all_nodes)
    var mesh_count := 0
    var triangle_count := 0
    var material_names: Array[String] = []
    var animation_players: Array[AnimationPlayer] = []
    for node in all_nodes:
        if node is MeshInstance3D:
            mesh_count += 1
            var mesh_instance := node as MeshInstance3D
            if mesh_instance.mesh != null:
                for surface in range(mesh_instance.mesh.get_surface_count()):
                    var arrays := mesh_instance.mesh.surface_get_arrays(surface)
                    var indices = arrays[Mesh.ARRAY_INDEX]
                    var vertices = arrays[Mesh.ARRAY_VERTEX]
                    triangle_count += int(indices.size() / 3) if indices.size() > 0 else int(vertices.size() / 3)
                    var material := mesh_instance.mesh.surface_get_material(surface)
                    if material != null:
                        material_names.append(material.resource_name)
        if node is AnimationPlayer:
            animation_players.append(node as AnimationPlayer)
    material_names.sort()
    if mesh_count != 5:
        _fail(scene_spec["role"] + " imported mesh count is not 5")

    var socket_rows: Array[Dictionary] = []
    for expected in socket_expectations:
        var socket := instance.find_child(expected["node_name"], true, false) as Node3D
        if socket == null:
            _fail(scene_spec["role"] + " missing socket " + expected["node_name"])
            continue
        var parent_name = null if expected["parent_node_name"] == null else str(socket.get_parent().name)
        if expected["parent_node_name"] != null and parent_name != expected["parent_node_name"]:
            _fail(scene_spec["role"] + " parent mismatch for " + expected["node_name"])
        if expected["parent_node_name"] == null and socket.get_parent() != instance:
            _fail(scene_spec["role"] + " scene-root parent mismatch for " + expected["node_name"])
        var expected_position := _vec3(expected["local_translation_m"])
        var expected_rotation := _quat(expected["local_rotation_quat_xyzw"])
        var expected_scale := _vec3(expected["local_scale_xyz"])
        if not socket.position.is_equal_approx(expected_position):
            _fail(scene_spec["role"] + " local position mismatch for " + expected["node_name"])
        if not socket.scale.is_equal_approx(expected_scale):
            _fail(scene_spec["role"] + " local scale mismatch for " + expected["node_name"])
        if not _quat_equal(socket.quaternion, expected_rotation):
            _fail(scene_spec["role"] + " local rotation mismatch for " + expected["node_name"])
        socket_rows.append({
            "name": str(socket.name),
            "parent": parent_name,
            "class": socket.get_class(),
            "position": _vector_json(socket.position),
            "quaternion_xyzw": [socket.quaternion.x, socket.quaternion.y, socket.quaternion.z, socket.quaternion.w],
            "scale": _vector_json(socket.scale),
            "non_rendering": not (socket is MeshInstance3D),
        })
    socket_rows.sort_custom(func(a, b): return a["name"] < b["name"])
    if socket_rows.size() != 6:
        _fail(scene_spec["role"] + " socket count is not 6")

    var animation_report := {
        "animation_player_count": animation_players.size(),
        "clip_name": null,
        "track_count": 0,
        "length_seconds": 0.0,
        "half_duration_follow_names": [],
    }
    if scene_spec["role"] == "animated":
        if animation_players.size() != 1:
            _fail("animated scene must import exactly one AnimationPlayer")
        elif not animation_players.is_empty():
            animation_report = _inspect_animation(instance, animation_players[0], animation_samples)
    elif not animation_players.is_empty():
        _fail(scene_spec["role"] + " unexpectedly imported animation")

    instance.queue_free()
    return {
        "role": scene_spec["role"],
        "resource_path": scene_spec["path"],
        "load": "PASS",
        "node_count": all_nodes.size(),
        "mesh_count": mesh_count,
        "triangle_count": triangle_count,
        "material_names": material_names,
        "socket_count": socket_rows.size(),
        "sockets": socket_rows,
        "animation": animation_report,
    }


func _inspect_animation(
    instance: Node, player: AnimationPlayer, animation_samples: Dictionary
) -> Dictionary:
    var clip_name := ""
    var animation: Animation = null
    for name in player.get_animation_list():
        if name == "RESET":
            continue
        var candidate := player.get_animation(name)
        if candidate != null and (animation == null or candidate.get_track_count() > animation.get_track_count()):
            clip_name = name
            animation = candidate
    if animation == null:
        _fail("Godot imported no non-RESET animation")
        return {"animation_player_count": 1, "clip_name": null, "track_count": 0, "length_seconds": 0.0, "half_duration_follow_names": []}
    if animation.get_track_count() != 2:
        _fail("Godot imported optimized animation track count is not 2")
    var expected_by_name := {}
    for sample in animation_samples.get("socket_animation_samples", []):
        expected_by_name[sample["name"]] = sample
    var before := {}
    player.play(clip_name)
    player.seek(0.0, true)
    player.advance(0.0)
    for socket_name in SOCKET_NAMES:
        var socket := instance.find_child(socket_name, true, false) as Node3D
        if socket != null:
            before[socket_name] = _world_projection(socket)
            _require_world_sample(socket_name, "start", before[socket_name], expected_by_name)
    player.seek(animation.length * 0.5, true)
    player.advance(0.0)
    var followed: Array[String] = []
    for socket_name in SOCKET_NAMES:
        var socket := instance.find_child(socket_name, true, false) as Node3D
        if socket != null and before.has(socket_name):
            var half := _world_projection(socket)
            _require_world_sample(socket_name, "half", half, expected_by_name)
            if (_vec3(half["position"]) - _vec3(before[socket_name]["position"])).length() > 0.000001:
                followed.append(socket_name)
    followed.sort()
    for required in ["forgecad-anchor-socket-energy-core-vfx", "forgecad-anchor-socket-magazine-well"]:
        if not followed.has(required):
            _fail("Godot animation did not move required socket " + required)
    return {
        "animation_player_count": 1,
        "clip_name": clip_name,
        "source_gltf_channel_count": 10,
        "godot_optimized_track_count": animation.get_track_count(),
        "semantic_sampling_exact": failures.is_empty(),
        "length_seconds": animation.length,
        "half_duration_follow_names": followed,
    }


func _require_world_sample(
    socket_name: String, phase: String, actual: Dictionary, expected_by_name: Dictionary
) -> void:
    if not expected_by_name.has(socket_name):
        _fail("missing cross-loader sample for " + socket_name)
        return
    var expected = expected_by_name[socket_name][phase]
    if not _vec3(actual["position"]).is_equal_approx(_vec3(expected["position"])):
        _fail("Godot/Three.js world position differs for " + socket_name + " at " + phase)
    if not _vec3(actual["scale"]).is_equal_approx(_vec3(expected["scale"])):
        _fail("Godot/Three.js world scale differs for " + socket_name + " at " + phase)
    if not _quat_equal(
        _quat(actual["quaternion_xyzw"]), _quat(expected["quaternion_xyzw"])
    ):
        _fail("Godot/Three.js world quaternion differs for " + socket_name + " at " + phase)


func _world_projection(node: Node3D) -> Dictionary:
    var transform := node.global_transform
    var quaternion := transform.basis.get_rotation_quaternion()
    return {
        "position": _vector_json(transform.origin),
        "quaternion_xyzw": [quaternion.x, quaternion.y, quaternion.z, quaternion.w],
        "scale": _vector_json(transform.basis.get_scale()),
    }


func _quat(values: Array) -> Quaternion:
    return Quaternion(float(values[0]), float(values[1]), float(values[2]), float(values[3]))


func _quat_equal(left: Quaternion, right: Quaternion) -> bool:
    return left.is_equal_approx(right) or left.is_equal_approx(-right)


func _inspect_collision(collision_sidecar: Dictionary) -> Dictionary:
    var proxies = collision_sidecar.get("proxies", [])
    var root := Node3D.new()
    root.name = "ForgeCADCollisionProbe"
    get_root().add_child(root)
    var imported_rows: Array[Dictionary] = []
    for proxy in proxies:
        if proxy.get("shape") != "box":
            _fail("unsupported collision proxy shape")
            continue
        var half := _vec3(proxy["half_extents_m"])
        if half.x <= 0.0 or half.y <= 0.0 or half.z <= 0.0:
            _fail("collision half extents are not positive")
        var body := StaticBody3D.new()
        body.name = proxy["part_id"]
        var collision := CollisionShape3D.new()
        collision.name = proxy["proxy_id"]
        var shape := BoxShape3D.new()
        shape.size = half * 2.0
        collision.shape = shape
        collision.position = _vec3(proxy["center_m"])
        body.add_child(collision)
        root.add_child(body)
        imported_rows.append({
            "part_id": proxy["part_id"],
            "proxy_id": proxy["proxy_id"],
            "shape": "BoxShape3D",
            "center_m": _vector_json(collision.position),
            "size_m": _vector_json(shape.size),
        })
    imported_rows.sort_custom(func(a, b): return a["part_id"] < b["part_id"])
    if imported_rows.size() != 5:
        _fail("Godot collision proxy count is not 5")
    root.queue_free()
    return {
        "source_policy": collision_sidecar.get("policy"),
        "source_proxy_count": proxies.size(),
        "godot_collision_shape_count": imported_rows.size(),
        "aabb_sidecar_readback": "PASS" if imported_rows.size() == proxies.size() else "FAIL",
        "physics_simulation": "NOT_RUN",
        "hitbox_semantics": false,
        "rows": imported_rows,
    }


func _collect_nodes(node: Node, output: Array[Node]) -> void:
    output.append(node)
    for child in node.get_children():
        _collect_nodes(child, output)


func _vec3(values: Array) -> Vector3:
    return Vector3(float(values[0]), float(values[1]), float(values[2]))


func _vector_json(value: Vector3) -> Array[float]:
    return [value.x, value.y, value.z]


func _fail(message: String) -> void:
    failures.append(message)
    push_error(message)
