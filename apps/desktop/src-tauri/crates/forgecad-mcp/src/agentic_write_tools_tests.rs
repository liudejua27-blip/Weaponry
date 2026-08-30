use super::*;
use crate::validate_declared_tool_input;

fn approval() -> Value {
    json!({
        "approved": true,
        "approval_receipt_id": "approval-1",
        "approval_summary": "user approved checkpoint",
        "idempotency_key": "idem-1"
    })
}

fn bound() -> Binding {
    Binding {
        session_id: Some("session-1".to_owned()),
        project_id: Some("project-1".to_owned()),
        candidate_id: Some("candidate-1".to_owned()),
    }
}

fn assembly_parameter_sink_response() -> Value {
    let hash = "a".repeat(64);
    let sink_specs = [
        (
            "receiver-envelope-width",
            "receiver-envelope",
            "forgecad.assembly.mutator.receiver-envelope@1",
            "ratio",
            "receiver-main",
            "receiver-width-node",
            "forgecad.geometry.longitudinal-section-loft@1",
            1.0_f64,
            0.8_f64,
            1.2_f64,
        ),
        (
            "receiver-envelope-height",
            "receiver-envelope",
            "forgecad.assembly.mutator.receiver-envelope@1",
            "ratio",
            "receiver-main",
            "receiver-height-node",
            "forgecad.geometry.longitudinal-section-loft@1",
            1.0_f64,
            0.8_f64,
            1.2_f64,
        ),
        (
            "receiver-envelope-shoulder",
            "receiver-envelope",
            "forgecad.assembly.mutator.receiver-envelope@1",
            "meter",
            "receiver-main",
            "receiver-shoulder-node",
            "forgecad.geometry.longitudinal-section-loft@1",
            0.0_f64,
            -0.12_f64,
            0.12_f64,
        ),
        (
            "muzzle-axis-shroud-envelope",
            "muzzle-axis",
            "forgecad.assembly.mutator.muzzle-axis@1",
            "ratio",
            "muzzle-shroud",
            "muzzle-shroud-node",
            "forgecad.geometry.longitudinal-section-loft@1",
            1.0_f64,
            0.8_f64,
            1.2_f64,
        ),
        (
            "muzzle-axis-emitter-envelope",
            "muzzle-axis",
            "forgecad.assembly.mutator.muzzle-axis@1",
            "ratio",
            "muzzle-emitter",
            "muzzle-emitter-node",
            "forgecad.geometry.longitudinal-section-loft@1",
            1.0_f64,
            0.8_f64,
            1.2_f64,
        ),
        (
            "muzzle-axis-core-aperture",
            "muzzle-axis",
            "forgecad.assembly.mutator.muzzle-axis@1",
            "ratio",
            "muzzle-core",
            "muzzle-core-node",
            "forgecad.geometry.primitive@2",
            1.0_f64,
            0.8_f64,
            1.2_f64,
        ),
    ];
    let sinks = sink_specs
        .into_iter()
        .map(
            |(
                parameter_id,
                group_id,
                mutator_id,
                unit,
                part_id,
                node_id,
                operator_id,
                current,
                min,
                max,
            )| {
                json!({
                    "parameter_id":parameter_id,
                    "group_id":group_id,
                    "mutator_id":mutator_id,
                    "current":current,
                    "min":min,
                    "max":max,
                    "step":0.01,
                    "unit":unit,
                    "application_status":"AVAILABLE",
                    "blocker_codes":[],
                    "target_part_ids":[part_id],
                    "source_node_ids":[node_id],
                    "operator_ids":[operator_id],
                    "evidence_requirements":[
                        "assembly-registry",
                        "geometry-program",
                        "operator-catalog",
                        "artifact-readback",
                        "candidate-state"
                    ]
                })
            },
        )
        .collect::<Vec<_>>();
    let mut registry = json!({
        "schema_version":"ProductionWeaponAssemblyParameterSinkRegistry@1",
        "sink_registry_id":"sink-registry-1",
        "profile_id":"fps-weapon-form-assembly@1",
        "sink_policy":"fps-weapon-product-owned-aggregate-parameter-sink-registry@1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash.clone(),
        "artifact_id":"artifact-1",
        "artifact_sha256":hash.clone(),
        "geometry_program_sha256":hash.clone(),
        "geometry_program_canonical_sha256":hash.clone(),
        "operator_catalog_sha256":hash.clone(),
        "assembly_registry_id":"assembly-registry-1",
        "assembly_registry_canonical_sha256":hash.clone(),
        "supported_group_ids":["receiver-envelope","muzzle-axis"],
        "sinks":sinks,
        "unavailable_parameter_ids":[
            "stock-open-frame-clearance",
            "stock-open-frame-angle",
            "trigger-void-clearance",
            "trigger-void-centroid",
            "rail-spine-continuity",
            "rail-spine-offset"
        ],
        "status":"READY",
        "read_only":true,
        "runtime_write_performed":false,
        "worker_invoked":false,
        "candidate_generated":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    let canonical_sha256 = forgecad_runtime::canonical_json_hash(&registry);
    registry["canonical_sha256"] = Value::String(canonical_sha256.clone());
    json!({
        "schema_version":"ProductionWeaponAssemblyParameterSinkGetResult@1",
        "registry":registry,
        "registry_canonical_sha256":canonical_sha256,
        "recomputed":true,
        "restart_hash_verified":true,
        "read_only":true,
        "structural_status":"structural_only",
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "runtime_write_performed":false,
        "worker_invoked":false,
        "candidate_generated":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    })
}

#[test]
fn production_weapon_assembly_parameter_sink_get_is_closed_read_only_and_scope_bound() {
    let name = "production_weapon_assembly_parameter_sink_get";
    assert!(is_tool(name));
    assert!(!is_write_tool(name));
    assert_eq!(runtime_method(name), Some(name));
    let reads = read_tools();
    assert_eq!(reads.len(), 34);
    assert!(!write_tools().iter().any(|tool| tool["name"] == name));
    let tool = reads
        .iter()
        .find(|tool| tool["name"] == name)
        .expect("assembly parameter sink get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], false);
    assert_eq!(tool["annotations"]["idempotentHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "sink_registry_id",
            "session_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "geometry_program_sha256",
            "geometry_program_canonical_sha256",
            "operator_catalog_sha256",
            "assembly_registry_id",
            "assembly_registry_canonical_sha256"
        ])
    );
    assert_eq!(
        tool["inputSchema"]["properties"].as_object().unwrap().len(),
        13
    );

    let hash = "a".repeat(64);
    let mut request = json!({
        "schema_version":"ProductionWeaponAssemblyParameterSinkGetRequest@1",
        "sink_registry_id":"sink-registry-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash.clone(),
        "artifact_id":"artifact-1",
        "artifact_sha256":hash.clone(),
        "geometry_program_sha256":hash.clone(),
        "geometry_program_canonical_sha256":hash.clone(),
        "operator_catalog_sha256":hash.clone(),
        "assembly_registry_id":"assembly-registry-1",
        "assembly_registry_canonical_sha256":hash.clone()
    });
    assert!(validate_declared_tool_input(name, &request, false).is_ok());
    assert!(validate_call(name, &request, &bound()).is_ok());
    assert!(validate_call(name, &request, &Binding::default()).is_ok());
    for field in [
        "raw_png_bytes",
        "raw_glb_bytes",
        "path",
        "url",
        "script",
        "secret",
        "unknown",
    ] {
        request[field] = json!("forbidden");
        assert!(
            validate_declared_tool_input(name, &request, false).is_err(),
            "{field}"
        );
        request.as_object_mut().unwrap().remove(field);
    }
    let mut nested_forbidden = request.clone();
    nested_forbidden["metadata"] = json!({"transport":{"secret":"forbidden"}});
    assert!(validate_call(name, &nested_forbidden, &bound()).is_err());
    let mut mismatch = request.clone();
    mismatch["candidate_id"] = json!("candidate-2");
    assert!(validate_call(name, &mismatch, &bound()).is_err());

    let response = assembly_parameter_sink_response();
    assert!(validate_response(name, &response, &bound()).is_ok());
    let mut tampered = response.clone();
    tampered["recomputed"] = json!(false);
    assert!(validate_response(name, &tampered, &bound()).is_err());
    let mut tampered_hash = response.clone();
    tampered_hash["registry"]["canonical_sha256"] = json!("b".repeat(64));
    assert!(validate_response(name, &tampered_hash, &bound()).is_err());
    let mut partial = assembly_parameter_sink_response();
    partial["registry"]["sinks"].as_array_mut().unwrap().pop();
    partial["registry"]["unavailable_parameter_ids"]
        .as_array_mut()
        .unwrap()
        .push(json!("muzzle-axis-core-aperture"));
    partial["registry"]["status"] = json!("PARTIAL_TYPED_SINKS");
    partial["registry"]["canonical_sha256"] = json!("");
    let partial_hash = forgecad_runtime::canonical_json_hash(&partial["registry"]);
    partial["registry"]["canonical_sha256"] = json!(partial_hash.clone());
    partial["registry_canonical_sha256"] = json!(partial_hash);
    assert!(validate_response(name, &partial, &bound()).is_ok());
    partial["registry"]["unavailable_parameter_ids"]
        .as_array_mut()
        .unwrap()
        .swap(0, 6);
    partial["registry"]["canonical_sha256"] = json!("");
    let reordered_hash = forgecad_runtime::canonical_json_hash(&partial["registry"]);
    partial["registry"]["canonical_sha256"] = json!(reordered_hash.clone());
    partial["registry_canonical_sha256"] = json!(reordered_hash);
    assert!(validate_response(name, &partial, &bound()).is_err());
    let mut raw = response;
    raw["path"] = json!("/tmp/forbidden");
    assert!(validate_response(name, &raw, &bound()).is_err());
    let mut nested_raw = assembly_parameter_sink_response();
    nested_raw["metadata"] = json!({"nested":{"url":"https://forbidden.example"}});
    assert!(validate_response(name, &nested_raw, &bound()).is_err());
}

fn candidate_animation_vfx_quality_v2_response(is_prepare: bool) -> Value {
    let hash = "a".repeat(64);
    let mut quality = Map::new();
    for field in CANDIDATE_ANIMATION_VFX_QUALITY_V2_RECORD_FIELDS {
        quality.insert(field.to_owned(), Value::String(hash.clone()));
    }
    quality.insert(
        "schema_version".to_owned(),
        Value::String("CandidateAnimationVfxQuality@2".to_owned()),
    );
    for field in [
        "animation_vfx_quality_id",
        "source_material_surface_transition_id",
        "source_material_surface_quality_id",
        "animation_clip_id",
    ] {
        quality.insert(field.to_owned(), Value::String(format!("{field}-1")));
    }
    quality.insert(
        "project_id".to_owned(),
        Value::String("project-1".to_owned()),
    );
    quality.insert(
        "candidate_id".to_owned(),
        Value::String("appearance-1".to_owned()),
    );
    quality.insert(
        "geometry_candidate_id".to_owned(),
        Value::String("candidate-1".to_owned()),
    );
    quality.insert(
        "appearance_candidate_id".to_owned(),
        Value::String("appearance-1".to_owned()),
    );
    quality.insert(
        "geometry_preservation_status".to_owned(),
        Value::String("source-output-renderable-geometry-byte-exact".to_owned()),
    );
    quality.insert(
        "anchor_binding_policy".to_owned(),
        Value::String("geometry-appearance-anchor-role-owner-trs-equivalent@1".to_owned()),
    );
    quality.insert("sample_count".to_owned(), Value::from(15_u64));
    quality.insert(
        "sample_time_ticks".to_owned(),
        Value::Array((0..15_u64).map(Value::from).collect()),
    );
    quality.insert("attachment_frame_count".to_owned(), Value::from(15_u64));
    quality.insert(
            "attachment_policy".to_owned(),
            Value::String(
                "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
                    .to_owned(),
            ),
        );
    quality.insert(
        "frame_scope".to_owned(),
        Value::String(
            "lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"
                .to_owned(),
        ),
    );
    quality.insert(
        "animation_vfx_scope".to_owned(),
        Value::String(
            "lod0-rigid-animation-full-vfx-stack-attachment-v3-all-15-frames@2".to_owned(),
        ),
    );
    quality.insert(
        "animation_vfx_policy".to_owned(),
        Value::String("candidate-animation-vfx-attachment-v3-structural-hard-gate@2".to_owned()),
    );
    quality.insert(
        "animation_vfx_policy_sha256".to_owned(),
        Value::String(forgecad_runtime::sha256_hex(
            b"candidate-animation-vfx-attachment-v3-structural-hard-gate@2",
        )),
    );
    quality.insert(
        "from_stage".to_owned(),
        Value::String("material-surface".to_owned()),
    );
    quality.insert(
        "to_stage".to_owned(),
        Value::String("animation-vfx".to_owned()),
    );
    quality.insert(
            "candidate_binding_status".to_owned(),
            Value::String(
                "same-material-surface-head-candidate-exact-attachment-v3-all-15-frames-no-geometry-mutation"
                    .to_owned(),
            ),
        );
    quality.insert("hard_gate".to_owned(), {
        let mut gate = Map::new();
        for field in CANDIDATE_ANIMATION_VFX_QUALITY_V2_HARD_GATE_FIELDS {
            gate.insert(field.to_owned(), Value::Bool(true));
        }
        Value::Object(gate)
    });
    quality.insert(
        "validator_status".to_owned(),
        Value::String("passed".to_owned()),
    );
    for (field, status) in [
        ("animation_status", "structural_only"),
        ("vfx_status", "structural_only"),
        ("visual_quality_status", "NOT_PROVEN"),
        ("artistic_quality_status", "NOT_PROVEN"),
        ("human_review_status", "NOT_RUN"),
        ("commercial_fps_quality_status", "NOT_PROVEN"),
        ("commercial_engine_status", "NOT_RUN"),
        (
            "materialization_status",
            "runtime-owned-durable-candidate-animation-vfx-quality-v2",
        ),
        ("quality_status", "structural_only"),
        ("created_at", "2026-08-22T00:00:00Z"),
    ] {
        quality.insert(field.to_owned(), Value::String(status.to_owned()));
    }
    for field in [
        "actual_engine_roundtrip",
        "functional_semantics",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "hard_gate_passed",
        "runtime_write_performed",
    ] {
        quality.insert(
            field.to_owned(),
            Value::Bool(matches!(
                field,
                "hard_gate_passed" | "runtime_write_performed"
            )),
        );
    }
    let mut input_preimage = Map::new();
    for field in CANDIDATE_ANIMATION_VFX_QUALITY_V2_PREPARE_FIELDS {
        if matches!(field, "input_sha256" | "idempotency_key") {
            continue;
        }
        input_preimage.insert(field.to_owned(), quality[field].clone());
    }
    let input_sha256 = forgecad_runtime::canonical_json_hash(&Value::Object(input_preimage));
    quality.insert(
        "input_sha256".to_owned(),
        Value::String(input_sha256.clone()),
    );
    quality.insert("request_sha256".to_owned(), Value::String(input_sha256));
    let mut canonical_preimage = quality.clone();
    canonical_preimage.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    quality.insert(
        "canonical_sha256".to_owned(),
        Value::String(forgecad_runtime::canonical_json_hash(&Value::Object(
            canonical_preimage,
        ))),
    );
    let result_schema = if is_prepare {
        "CandidateAnimationVfxQualityPrepareResult@2"
    } else {
        "CandidateAnimationVfxQualityGetResult@2"
    };
    json!({
        "schema_version":result_schema,
        "animation_vfx_quality":Value::Object(quality),
        "replayed":false,
        "runtime_write":is_prepare,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    })
}

fn animated_socket_v2_response_with_parent_counts(
    is_prepare: bool,
    parent_accessor_count_added: u64,
    parent_buffer_view_count_added: u64,
) -> Value {
    let hash = "a".repeat(64);
    let roles = [
        "weapon-root",
        "grip-primary",
        "muzzle-vfx",
        "magazine-well",
        "sight-primary",
        "energy-core-vfx",
    ];
    let socket_nodes = (0..6)
        .map(|index| {
            json!({
                "socket_node_id":format!("socket-{index}"),
                "anchor_id":format!("anchor-{index}"),
                "role":roles[index],
                "node_name":format!("socket-{index}"),
                "node_kind":"empty",
                "parent_kind":"synthetic-scene-root",
                "parent_node_name":null,
                "owner_part_id":null,
                "local_translation_m":[0.0,0.0,0.0],
                "local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                "local_scale_xyz":[1.0,1.0,1.0]
            })
        })
        .collect::<Vec<_>>();
    let mut receipt = json!({
        "schema_version":"GameWeaponAnimatedGlbSocketMaterializationReceipt@2",
        "animated_socket_materialization_key_sha256":hash,
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "appearance_candidate_state_sha256":hash,
        "appearance_delivery_manifest_object_sha256":hash,
        "appearance_artifact_sha256":hash,
        "appearance_artifact_readback_sha256":hash,
        "animation_glb_key_sha256":hash,
        "animated_artifact_sha256":hash,
        "animated_artifact_readback_sha256":hash,
        "animation_receipt_object_sha256":hash,
        "animation_receipt_canonical_sha256":hash,
        "clip_id":"clip-1",
        "clip_object_sha256":hash,
        "clip_sha256":hash,
        "anchor_set_object_sha256":hash,
        "anchor_set_canonical_sha256":hash,
        "request_sha256":hash,
        "socket_materialization_policy":"appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2",
        "lod_scope":"lod0-appearance-animated-source-only@2",
        "socket_node_id_encoding_sha256":hash,
        "derived_animated_socket_artifact_sha256":hash,
        "derived_animated_socket_artifact_readback_sha256":hash,
        "source_animation_projection_sha256":hash,
        "derived_animation_projection_sha256":hash,
        "source_animation_validation_sha256":hash,
        "derived_animation_validation_sha256":hash,
        "source_renderable_inventory_sha256":hash,
        "derived_renderable_inventory_sha256":hash,
        "source_bin_sha256":hash,
        "derived_bin_sha256":hash,
        "source_appearance_material_projection_sha256":hash,
        "derived_appearance_material_projection_sha256":hash
    });
    let details = json!({
        "sampling_policy_sha256":hash,
        "sample_time_ticks":[0,1000],
        "part_ids":["part-1"],
        "sampler_count":2,
        "channel_count":2,
        "node_count":1,
        "source_node_count":1,
        "derived_node_count":7,
        "accessor_count_added":parent_accessor_count_added,
        "buffer_view_count_added":parent_buffer_view_count_added,
        "socket_node_inventory_sha256":hash,
        "socket_node_count":6,
        "socket_nodes":socket_nodes
    });
    receipt
        .as_object_mut()
        .expect("receipt object")
        .extend(details.as_object().expect("details object").clone());
    let boundaries = json!({
        "owned_cas_kinds":["game-weapon-animated-glb-v2-socket-materialized-glb","game-weapon-animated-glb-v2-socket-materialization-receipt"],
        "animations_preserved":true,
        "channels_preserved":true,
        "samplers_preserved":true,
        "renderable_projection_exact":true,
        "bin_byte_exact":true,
        "source_static_projection_exact":true,
        "appearance_material_projection_exact":true,
        "material_pack_identity_exact":true,
        "no_skinning":true,
        "no_morph_targets":true,
        "socket_nodes_materialized":true,
        "runtime_write_performed":true,
        "restart_hash_verified":true,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "production_stage_advanced":false,
        "actual_engine_roundtrip":false,
        "semantic_scope":"fictional-nonfunctional-game-visual-authoring-only@1",
        "functional_semantics":false,
        "materialization_status":"runtime-owned-durable-game-weapon-animated-glb-v2-socket-materialization",
        "validator_status":"strict-appearance-aware-animated-glb-socket-materialization-readback-pass",
        "hard_gate_passed":true,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "limitations":["appearance-candidate-bound-rigid-Part-TRS-only","scheduled-integer-ticks-and-LINEAR-interpolation-only","no-skinning-morph-targets-armature-IK-constraints-NLA-or-drivers","source-BIN-and-appearance-material-projection-must-remain-exact","structural-readback-does-not-prove-visual-quality-or-engine-roundtrip"],
        "canonical_sha256":hash,
        "created_at":"2026-08-22T00:00:00Z"
    });
    receipt
        .as_object_mut()
        .expect("receipt object")
        .extend(boundaries.as_object().expect("boundaries object").clone());
    let mut durable_link = receipt.clone();
    let link_only_fields = [
        "sample_time_ticks",
        "part_ids",
        "sampler_count",
        "channel_count",
        "node_count",
        "source_node_count",
        "derived_node_count",
        "accessor_count_added",
        "buffer_view_count_added",
        "socket_node_inventory_sha256",
        "socket_node_count",
        "socket_nodes",
        "owned_cas_kinds",
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "appearance_material_projection_exact",
        "material_pack_identity_exact",
        "no_skinning",
        "no_morph_targets",
        "socket_nodes_materialized",
        "runtime_write_performed",
        "restart_hash_verified",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "production_stage_advanced",
        "actual_engine_roundtrip",
        "semantic_scope",
        "functional_semantics",
        "visual_quality_status",
        "commercial_fps_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "limitations",
    ];
    for field in link_only_fields {
        durable_link
            .as_object_mut()
            .expect("receipt object")
            .remove(field);
    }
    durable_link["schema_version"] = json!("GameWeaponAnimatedGlbSocketMaterializationLink@2");
    durable_link["receipt_object_sha256"] = Value::String(hash.clone());
    let schema = if is_prepare {
        "GameWeaponAnimatedGlbSocketMaterializationPrepareResult@2"
    } else {
        "GameWeaponAnimatedGlbSocketMaterializationGetResult@2"
    };
    json!({
        "schema_version":schema,
        "animated_socket_materialization_key_sha256":hash,
        "derived_animated_socket_artifact_sha256":hash,
        "receipt_object_sha256":hash,
        "receipt":receipt,
        "durable_link":durable_link,
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write_performed":is_prepare,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "production_stage_advanced":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    })
}

fn animated_socket_v2_response(is_prepare: bool) -> Value {
    animated_socket_v2_response_with_parent_counts(is_prepare, 3, 3)
}

#[test]
fn production_weapon_art_decision_proposal_get_is_closed_read_only_and_shape_checked() {
    let name = "production_weapon_art_decision_proposal_get";
    assert!(is_tool(name));
    assert!(!is_write_tool(name));
    assert_eq!(runtime_method(name), Some(name));
    let reads = read_tools();
    assert_eq!(reads.len(), 34);
    let tool = reads
        .iter()
        .find(|tool| tool["name"] == name)
        .expect("art-decision proposal get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "session_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "geometry_program_sha256",
            "geometry_program_canonical_sha256",
            "operator_catalog_sha256",
            "reference_canvas_canonical_sha256",
            "design_spec_canonical_sha256",
            "camera_lock_id",
            "camera_lock_canonical_sha256",
            "form_evidence_id",
            "form_evidence_object_sha256",
            "form_evidence_canonical_sha256",
            "form_art_evidence_id",
            "form_art_evidence_object_sha256",
            "form_art_evidence_canonical_sha256",
            "first_person_profile_id",
            "first_person_profile_sha256"
        ])
    );
    assert_eq!(
        tool["inputSchema"]["properties"].as_object().unwrap().len(),
        22
    );
    assert!(tool["inputSchema"]["properties"]
        .get("assembly_registry_id")
        .is_none());

    let hash = "a".repeat(64);
    let mut request = json!({
        "schema_version":"ProductionWeaponArtDecisionProposalGetRequest@1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash.clone(),
        "artifact_id":"artifact-1",
        "artifact_sha256":hash.clone(),
        "geometry_program_sha256":hash.clone(),
        "geometry_program_canonical_sha256":hash.clone(),
        "operator_catalog_sha256":hash.clone(),
        "reference_canvas_canonical_sha256":hash.clone(),
        "design_spec_canonical_sha256":hash.clone(),
        "camera_lock_id":"camera-lock-1",
        "camera_lock_canonical_sha256":hash.clone(),
        "form_evidence_id":"form-evidence-1",
        "form_evidence_object_sha256":hash.clone(),
        "form_evidence_canonical_sha256":hash.clone(),
        "form_art_evidence_id":"form-art-evidence-1",
        "form_art_evidence_object_sha256":hash.clone(),
        "form_art_evidence_canonical_sha256":hash.clone(),
        "first_person_profile_id":null,
        "first_person_profile_sha256":null
    });
    assert!(validate_call(name, &request, &bound()).is_ok());
    assert!(validate_call(name, &request, &Binding::default()).is_ok());
    for field in [
        "raw_png_bytes",
        "raw_glb_bytes",
        "path",
        "url",
        "script",
        "secret",
    ] {
        request[field] = json!("forbidden");
        assert!(validate_call(name, &request, &bound()).is_err(), "{field}");
        request.as_object_mut().unwrap().remove(field);
    }

    let views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ]
    .into_iter()
    .map(|kind| {
        json!({
            "view_kind":kind,
            "view_id":format!("view-{kind}"),
            "reference_id":format!("reference-{kind}"),
            "reference_sha256":hash.clone(),
            "camera_hash":hash.clone(),
            "camera_canonical_sha256":hash.clone(),
            "render_set_object_sha256":hash.clone(),
            "render_set_canonical_sha256":hash.clone(),
            "form_evidence_view_receipt_object_sha256":hash.clone(),
            "form_evidence_view_receipt_canonical_sha256":hash.clone(),
            "form_art_evidence_view_receipt_object_sha256":hash.clone(),
            "form_art_evidence_view_receipt_canonical_sha256":hash.clone(),
            "target_sha256":hash.clone(),
            "visual_structure_canonical_sha256":hash.clone(),
            "part_id_status":"observed",
            "negative_space_status":"unknown",
            "line_flow_status":"unknown",
            "view_observation_status":"observed"
        })
    })
    .collect::<Vec<_>>();
    let groups = [
        "receiver-envelope",
        "muzzle-axis",
        "stock-open-frame",
        "trigger-void",
        "rail-spine",
    ]
    .into_iter()
    .map(|group_id| {
        json!({
            "group_id":group_id,
            "status":"BLOCKED_PARAMETER_SINK",
            "part_ids":[format!("{group_id}-part")],
            "source_node_ids":[format!("{group_id}-node")],
            "parameter_ids":[format!("{group_id}-parameter")],
            "allowed_operator_ids":["forgecad.geometry.primitive@2"],
            "coupling_mode":"linked",
            "invariants":["shared-axis"],
            "affected_view_kinds":["front","back","left","right","top","rear-three-quarter"],
            "blocker_codes":["BLOCKED_PARAMETER_SINK"]
        })
    })
    .collect::<Vec<_>>();
    let gates = [
            "lineage",
            "reference-annotation",
            "camera",
            "assembly-registry",
            "parameter-sink",
            "negative-space",
            "line-flow",
            "first-person-readability",
            "candidate-search-critic",
            "surface-scope",
        ]
        .into_iter()
        .map(|gate_id| {
            json!({
                "gate_id":gate_id,
                "status":if gate_id == "lineage" {"PASS"} else {"BLOCKED"},
                "evidence_sha256":if gate_id == "lineage" {Value::String(hash.clone())} else {Value::Null},
                "blocker_codes":if gate_id == "lineage" {json!([])} else {json!(["BLOCKED_PARAMETER_SINK"])}
            })
        })
        .collect::<Vec<_>>();
    let mut response = json!({
        "schema_version":"ProductionWeaponArtDecisionProposalGetResult@1",
        "proposal_projection_id":"proposal-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash.clone(),
        "artifact_id":"artifact-1",
        "artifact_sha256":hash.clone(),
        "geometry_program_sha256":hash.clone(),
        "geometry_program_canonical_sha256":hash.clone(),
        "operator_catalog_sha256":hash.clone(),
        "assembly_registry_id":"assembly-registry-1",
        "assembly_registry_canonical_sha256":hash.clone(),
        "reference_canvas_canonical_sha256":hash.clone(),
        "design_spec_canonical_sha256":hash.clone(),
        "camera_lock_id":"camera-lock-1",
        "camera_lock_canonical_sha256":hash.clone(),
        "form_evidence_id":"form-evidence-1",
        "form_evidence_object_sha256":hash.clone(),
        "form_evidence_canonical_sha256":hash.clone(),
        "form_art_evidence_id":"form-art-evidence-1",
        "form_art_evidence_object_sha256":hash.clone(),
        "form_art_evidence_canonical_sha256":hash.clone(),
        "first_person_profile_id":null,
        "first_person_profile_sha256":null,
        "objective_policy":"assembly-form-search-negative-space-line-flow-first-person@1",
        "proposal_status":"BLOCKED_FIRST_PERSON_PROFILE",
        "read_only":true,
        "runtime_write_performed":false,
        "worker_invoked":false,
        "candidate_generated":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "replayed":true,
        "restart_hash_verified":true,
        "canonical_sha256":hash
    });
    response["view_bindings"] = Value::Array(views);
    response["assembly_group_decisions"] = Value::Array(groups);
    response["gate_results"] = Value::Array(gates);
    response["blockers"] = json!([{"blocker_code":"BLOCKED_FIRST_PERSON_PROFILE","scope":"global","group_id":null,"view_kind":null,"evidence_sha256":null}]);
    assert!(validate_response(name, &response, &bound()).is_ok());
    let mut unsafe_flags = response.clone();
    unsafe_flags["read_only"] = json!(false);
    assert!(validate_response(name, &unsafe_flags, &bound()).is_err());
    let mut short_views = response.clone();
    short_views["view_bindings"].as_array_mut().unwrap().pop();
    assert!(validate_response(name, &short_views, &bound()).is_err());
    let mut raw = response;
    raw["path"] = json!("/tmp/forbidden");
    assert!(validate_response(name, &raw, &bound()).is_err());
}

#[test]
fn annotations_keep_reads_and_prepares_distinct() {
    let reads = read_tools();
    assert_eq!(reads.len(), 34);
    assert!(reads.iter().all(|tool| {
        tool["annotations"]["readOnlyHint"] == true
            && tool["annotations"]["writeIntent"] == false
            && tool["annotations"]["approvalRequired"] == false
    }));
    for tool in write_tools() {
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["annotations"]["writeIntent"], true);
        let expected_approval = !matches!(
            tool["name"].as_str(),
            Some(
                "candidate_topology_quality_prepare"
                    | "candidate_material_surface_quality_prepare"
                    | "candidate_animation_vfx_quality_prepare"
                    | "candidate_animation_vfx_quality_v2_prepare"
                    | "mechanical_animation_clip_v2_prepare"
                    | "mechanical_animation_glb_v2_prepare"
                    | "game_weapon_animated_glb_socket_v2_prepare"
                    | "fictional_energy_vfx_animated_socket_attachment_prepare"
                    | "fictional_energy_vfx_animated_socket_attachment_v2_prepare"
                    | "fictional_energy_vfx_animated_socket_attachment_v3_prepare"
                    | "game_weapon_animated_glb_socket_transform_projection_prepare"
                    | "game_weapon_animated_glb_socket_transform_projection_v2_prepare"
                    | "fictional_energy_vfx_animated_socket_particles_sequence_prepare"
                    | "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"
                    | "fictional_energy_vfx_animated_socket_trails_sequence_prepare"
                    | "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare"
                    | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"
                    | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare"
                    | "production_weapon_form_quality_prepare"
                    | "production_weapon_form_evidence_prepare"
                    | "production_weapon_form_art_evidence_prepare"
                    | "production_weapon_form_quality_v2_prepare"
                    | "production_weapon_retopology_cage_source_prepare"
            )
        );
        assert_eq!(tool["annotations"]["approvalRequired"], expected_approval);
    }
}

#[test]
fn production_weapon_form_quality_surface_is_hidden_closed_and_read_only_get() {
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_weapon_form_quality_get")
        .expect("form-quality get tool");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_weapon_form_quality_prepare")
        .expect("form-quality prepare tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    let get_request = json!({
        "schema_version":"ProductionWeaponFormQualityGetRequest@1",
        "form_quality_id":"form-quality-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "form_stage":"blockout"
    });
    assert!(validate_declared_tool_input(
        "production_weapon_form_quality_get",
        &get_request,
        false
    )
    .is_ok());
    let mut unknown = get_request;
    unknown["raw_png_bytes"] = json!("forbidden");
    assert!(
        validate_declared_tool_input("production_weapon_form_quality_get", &unknown, false)
            .is_err()
    );
    assert!(validate_call(
        "production_weapon_form_quality_get",
        &json!({
            "schema_version":"ProductionWeaponFormQualityGetRequest@1",
            "form_quality_id":"form-quality-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "form_stage":"blockout"
        }),
        &Binding::default()
    )
    .is_ok());
}

#[test]
fn production_weapon_form_evidence_surface_is_hidden_closed_and_hash_only() {
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_weapon_form_evidence_get")
        .expect("form-evidence get tool");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_weapon_form_evidence_prepare")
        .expect("form-evidence prepare tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);

    let get_request = json!({
        "schema_version":"ProductionWeaponFormEvidenceGetRequest@1",
        "form_evidence_id":"form-evidence-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_declared_tool_input(
        "production_weapon_form_evidence_get",
        &get_request,
        false
    )
    .is_ok());
    let mut unknown = get_request.clone();
    unknown["raw_png_bytes"] = json!("forbidden");
    assert!(
        validate_declared_tool_input("production_weapon_form_evidence_get", &unknown, false)
            .is_err()
    );
    assert!(validate_call(
        "production_weapon_form_evidence_get",
        &get_request,
        &Binding::default()
    )
    .is_ok());
}

#[test]
fn production_weapon_form_art_evidence_surface_is_hidden_closed_and_scope_bound() {
    let prepare_name = "production_weapon_form_art_evidence_prepare";
    let get_name = "production_weapon_form_art_evidence_get";
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == prepare_name)
        .expect("form-art-evidence prepare tool");
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == get_name)
        .expect("form-art-evidence get tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["required"],
        json!([
            "schema_version",
            "art_evidence_id",
            "session_id",
            "project_id",
            "candidate_id",
            "form_evidence_object_sha256",
            "form_evidence_canonical_sha256",
            "art_evidence_policy",
            "art_evidence_policy_sha256",
            "input_sha256",
            "idempotency_key"
        ])
    );
    assert_eq!(
        get["inputSchema"]["required"],
        json!([
            "schema_version",
            "art_evidence_id",
            "session_id",
            "project_id",
            "candidate_id"
        ])
    );
    let hash = "a".repeat(64);
    let prepare_request = json!({
        "schema_version":"ProductionWeaponFormArtEvidencePrepareRequest@1",
        "art_evidence_id":"art-evidence-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "form_evidence_object_sha256":hash.clone(),
        "form_evidence_canonical_sha256":hash.clone(),
        "art_evidence_policy":"production-weapon-form-art-evidence-six-view-typed-observation@1",
        "art_evidence_policy_sha256":hash.clone(),
        "input_sha256":hash.clone(),
        "idempotency_key":"art-evidence-key-1"
    });
    let get_request = json!({
        "schema_version":"ProductionWeaponFormArtEvidenceGetRequest@1",
        "art_evidence_id":"art-evidence-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    let mut diagnostic = json!({
        "schema_version":"ProductionWeaponRasterSourceAttributionDiagnosticGetRequest@1",
        "diagnostic_id":"diagnostic-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash.clone(),
        "artifact_id":"artifact-1",
        "artifact_sha256":hash.clone(),
        "reference_id":"reference-left",
        "reference_sha256":hash.clone(),
        "form_art_evidence_object_sha256":hash.clone(),
        "form_art_evidence_canonical_sha256":hash.clone(),
        "view_kind":"left",
        "view_id":"view-left",
        "camera_hash":hash.clone(),
        "camera_canonical_sha256":hash.clone(),
        "input_sha256":hash.clone()
    });
    let mut get_with_diagnostic = get_request.clone();
    get_with_diagnostic["raster_source_attribution_diagnostic"] = diagnostic.clone();
    assert!(validate_declared_tool_input(get_name, &get_with_diagnostic, false).is_ok());
    diagnostic["camera"] = json!({"forbidden":"caller-provided"});
    get_with_diagnostic["raster_source_attribution_diagnostic"] = diagnostic;
    assert!(validate_declared_tool_input(get_name, &get_with_diagnostic, false).is_err());
    assert!(validate_call(
        prepare_name,
        &prepare_request,
        &Binding {
            session_id: Some("session-1".to_owned()),
            project_id: Some("project-1".to_owned()),
            candidate_id: Some("candidate-1".to_owned()),
        }
    )
    .is_ok());
    assert!(validate_call(get_name, &get_request, &Binding::default()).is_ok());
    assert!(validate_call(prepare_name, &prepare_request, &Binding::default()).is_err());
    let mut forbidden = prepare_request;
    forbidden["png_base64"] = json!("forbidden");
    assert!(contains_forbidden_transport_field(&forbidden));
    assert!(validate_call(
        prepare_name,
        &forbidden,
        &Binding {
            session_id: Some("session-1".to_owned()),
            project_id: Some("project-1".to_owned()),
            candidate_id: Some("candidate-1".to_owned()),
        }
    )
    .is_err());
}

#[test]
fn production_weapon_form_art_evidence_response_rejects_media_and_retarget() {
    let hash = "a".repeat(64);
    let policy = "production-weapon-form-art-evidence-six-view-typed-observation@1";
    let policy_sha256 = forgecad_runtime::sha256_hex(policy.as_bytes());
    let view = |kind: &str| {
        json!({
            "schema_version":"ProductionWeaponFormArtEvidenceView@1",
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "candidate_state_sha256":hash.clone(),
            "artifact_id":"artifact-1",
            "artifact_sha256":hash.clone(),
            "view_kind":kind,
            "view_id":format!("view-{kind}"),
            "reference_id":format!("reference-{kind}"),
            "reference_sha256":hash.clone(),
            "camera_hash":hash.clone(),
            "camera_canonical_sha256":hash.clone(),
            "form_evidence_view_receipt_object_sha256":hash.clone(),
            "form_evidence_view_receipt_canonical_sha256":hash.clone(),
            "target_object_sha256":hash.clone(),
            "target_canonical_sha256":hash.clone(),
            "visual_structure_canonical_sha256":hash.clone(),
            "visual_structure_review_status":"unknown",
            "silhouette_pass_object_sha256":hash.clone(),
            "part_id_pass_object_sha256":hash.clone(),
            "depth_pass_object_sha256":hash.clone(),
            "normal_pass_object_sha256":hash.clone(),
            "part_id_status":"observed",
            "part_id_expected_count":1,
            "part_id_observed_count":1,
            "part_id_missing_count":0,
            "part_id_unexpected_count":0,
            "part_id_coverage_milli":1000,
            "negative_space_status":"unknown",
            "negative_space_rows":[],
            "line_flow_status":"unknown",
            "line_flow_rows":[],
            "view_observation_status":"observed",
            "quality_status":"NOT_PROVEN",
            "receipt_object_sha256":hash.clone(),
            "canonical_sha256":hash.clone(),
            "created_at":"2026-08-23T00:00:00Z"
        })
    };
    let views = [
        view("front"),
        view("back"),
        view("left"),
        view("right"),
        view("top"),
        view("rear-three-quarter"),
    ];
    let record = json!({
        "schema_version":"ProductionWeaponFormArtEvidence@1",
        "art_evidence_id":"art-evidence-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash.clone(),
        "artifact_id":"artifact-1",
        "artifact_sha256":hash.clone(),
        "reference_canvas_object_sha256":hash.clone(),
        "reference_canvas_canonical_sha256":hash.clone(),
        "design_spec_object_sha256":hash.clone(),
        "design_spec_canonical_sha256":hash.clone(),
        "camera_lock_id":"camera-lock-1",
        "camera_lock_canonical_sha256":hash.clone(),
        "camera_rig_object_sha256":hash.clone(),
        "camera_rig_canonical_sha256":hash.clone(),
        "camera_lock_receipt_object_sha256":hash.clone(),
        "camera_lock_source_transition_id":"transition-1",
        "camera_lock_source_transition_sha256":hash.clone(),
        "camera_lock_source_head_canonical_sha256":hash.clone(),
        "form_evidence_object_sha256":hash.clone(),
        "form_evidence_canonical_sha256":hash.clone(),
        "view_kinds":["front","back","left","right","top","rear-three-quarter"],
        "views":views,
        "part_id_aggregate":{
            "status":"observed",
            "expected_count":1,
            "observed_count":1,
            "missing_count":0,
            "unexpected_count":0,
            "coverage_milli":1000
        },
        "art_evidence_policy":policy,
        "art_evidence_policy_sha256":policy_sha256,
        "quality_status":"NOT_PROVEN",
        "runtime_write_performed":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "request_sha256":hash.clone(),
        "input_sha256":hash.clone(),
        "receipt_object_sha256":hash.clone(),
        "canonical_sha256":hash.clone(),
        "created_at":"2026-08-23T00:00:00Z"
    });
    let response = json!({
        "schema_version":"ProductionWeaponFormArtEvidenceGetResult@1",
        "art_evidence":record,
        "replayed":true,
        "runtime_write":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "restart_hash_verified":true
    });
    assert!(validate_response(
        "production_weapon_form_art_evidence_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    let mut media = response.clone();
    media["art_evidence"]["views"][0]["png_base64"] = json!("forbidden");
    assert!(validate_response(
        "production_weapon_form_art_evidence_get",
        &media,
        &Binding::default()
    )
    .is_err());
    let mut retargeted = response;
    retargeted["art_evidence"]["views"][0]["candidate_id"] = json!("candidate-foreign");
    assert!(validate_response(
        "production_weapon_form_art_evidence_get",
        &retargeted,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn production_weapon_form_evidence_response_rejects_media_and_retarget() {
    let hash = "a".repeat(64);
    let view_kinds = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    let views = view_kinds
            .iter()
            .map(|kind| {
                json!({
                    "schema_version":"ProductionWeaponFormEvidenceView@1",
                    "project_id":"project-1",
                    "candidate_id":"candidate-1",
                    "candidate_state_sha256":hash.clone(),
                    "artifact_id":"artifact-1",
                    "artifact_sha256":hash.clone(),
                    "view_kind":kind,
                    "view_id":format!("view-{kind}"),
                    "reference_id":format!("reference-{kind}"),
                    "reference_sha256":hash.clone(),
                    "camera_hash":hash.clone(),
                    "camera_canonical_sha256":hash.clone(),
                    "render_set_object_sha256":hash.clone(),
                    "render_set_canonical_sha256":hash.clone(),
                    "render_set_view_id":format!("view-{kind}"),
                    "part_id_evidence":{
                        "observation":{"evidence_kind":"part-id","observation_status":"observed","quality_status":"NOT_PROVEN"},
                        "expected_part_ids":["receiver-main"],
                        "observed_part_ids":["receiver-main"],
                        "missing_part_ids":[],
                        "unexpected_part_ids":[],
                        "coverage_milli":1000
                    },
                    "negative_space_evidence":{
                        "observation":{"evidence_kind":"negative-space","observation_status":"unknown","quality_status":"NOT_PROVEN"},
                        "expected_count":0,
                        "observed_count":0,
                        "missing_count":0,
                        "sealed_count":0,
                        "coverage_milli":0
                    },
                    "line_flow_evidence":{
                        "observation":{"evidence_kind":"line-flow","observation_status":"unknown","quality_status":"NOT_PROVEN"},
                        "expected_count":0,
                        "observed_count":0,
                        "coverage_milli":0,
                        "continuity_milli":0,
                        "deviation_milli":0
                    },
                    "view_observation_status":"observed",
                    "quality_status":"NOT_PROVEN",
                    "receipt_object_sha256":hash.clone(),
                    "canonical_sha256":hash.clone(),
                    "created_at":"2026-08-23T00:00:00Z"
                })
            })
            .collect::<Vec<_>>();
    let response = json!({
        "schema_version":"ProductionWeaponFormEvidenceGetResult@1",
        "form_evidence":{
            "schema_version":"ProductionWeaponFormEvidence@1",
            "form_evidence_id":"form-evidence-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "candidate_state_sha256":hash.clone(),
            "artifact_id":"artifact-1",
            "artifact_sha256":hash.clone(),
            "reference_canvas_object_sha256":hash.clone(),
            "reference_canvas_canonical_sha256":hash.clone(),
            "design_spec_object_sha256":hash.clone(),
            "design_spec_canonical_sha256":hash.clone(),
            "camera_lock_id":"camera-lock-1",
            "camera_lock_canonical_sha256":hash.clone(),
            "camera_rig_object_sha256":hash.clone(),
            "camera_rig_canonical_sha256":hash.clone(),
            "camera_lock_receipt_object_sha256":hash.clone(),
            "camera_lock_source_transition_id":"transition-1",
            "camera_lock_source_transition_sha256":hash.clone(),
            "camera_lock_source_head_canonical_sha256":hash.clone(),
            "view_kinds":view_kinds,
            "views":views,
            "evidence_policy":"production-weapon-form-evidence-six-view-typed-observation@1",
            "evidence_policy_sha256":forgecad_runtime::sha256_hex(
                b"production-weapon-form-evidence-six-view-typed-observation@1"
            ),
            "quality_status":"NOT_PROVEN",
            "runtime_write_performed":true,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "request_sha256":hash.clone(),
            "input_sha256":hash.clone(),
            "receipt_object_sha256":hash.clone(),
            "canonical_sha256":hash.clone(),
            "created_at":"2026-08-23T00:00:00Z"
        },
        "replayed":true,
        "runtime_write":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "restart_hash_verified":true
    });
    assert!(validate_response(
        "production_weapon_form_evidence_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    let mut media = response.clone();
    media["form_evidence"]["views"][0]["png_base64"] = json!("forbidden");
    assert!(validate_response(
        "production_weapon_form_evidence_get",
        &media,
        &Binding::default()
    )
    .is_err());
    let mut retargeted = response;
    retargeted["form_evidence"]["views"][0]["candidate_id"] = json!("candidate-foreign");
    assert!(validate_response(
        "production_weapon_form_evidence_get",
        &retargeted,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn production_weapon_form_quality_response_rejects_nested_view_retarget() {
    let binding = json!({
        "source_kind":"not-proven",
        "source_object_sha256":null,
        "evidence_object_sha256":null,
        "status":"NOT_PROVEN"
    });
    let view = |kind: &str| {
        json!({
            "view_kind":kind,
            "view_id":format!("view-{kind}"),
            "part_id_evidence":{
                "source":binding.clone(),
                "expected_part_ids":["receiver-main"],
                "observed_part_ids":["receiver-main"],
                "missing_part_ids":[],
                "unexpected_part_ids":[],
                "coverage_milli":0
            },
            "negative_space_evidence":{
                "source":binding.clone(),
                "expected_count":0,
                "observed_count":0,
                "missing_count":0,
                "sealed_count":0,
                "coverage_milli":0
            },
            "line_flow_evidence":{
                "source":binding.clone(),
                "expected_count":0,
                "observed_count":0,
                "coverage_milli":0,
                "continuity_milli":0,
                "deviation_milli":0
            },
            "no_regression":{
                "status":"NOT_PROVEN",
                "metrics_not_regressed":false,
                "part_id_not_regressed":false,
                "negative_space_not_regressed":false,
                "line_flow_not_regressed":false
            }
        })
    };
    let mut views = json!([
        view("front"),
        view("back"),
        view("left"),
        view("right"),
        view("top"),
        view("rear-three-quarter")
    ]);
    validate_form_view_evaluations(&views).expect("six exact views accepted");
    views[2]["view_kind"] = json!("front");
    assert!(validate_form_view_evaluations(&views).is_err());
}

#[test]
fn mechanical_animation_clip_v2_surface_is_closed_project_appearance_bound_and_structural() {
    let reads = read_tools();
    let get = reads
        .iter()
        .find(|tool| tool["name"] == "mechanical_animation_clip_v2_get")
        .expect("appearance-aware clip get read tool");
    let preview = reads
        .iter()
        .find(|tool| tool["name"] == "mechanical_animation_clip_v2_preview")
        .expect("appearance-aware clip preview read tool");
    assert!(!reads
        .iter()
        .any(|tool| tool["name"] == "mechanical_animation_clip_v2_prepare"));
    for tool in [get, preview] {
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["writeIntent"], false);
        assert_eq!(tool["annotations"]["approvalRequired"], false);
        assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool["description"]
            .as_str()
            .is_some_and(|description| description.contains("raw GLB")));
    }
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "mechanical_animation_clip_v2_prepare")
        .expect("appearance-aware clip prepare write tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["properties"]["replay_policy"]["const"],
        "geometry-plus-appearance-double-worker-replay@1"
    );
    let required = prepare["inputSchema"]["required"]
        .as_array()
        .expect("appearance-aware clip required fields");
    for field in [
        "appearance_candidate_id",
        "appearance_artifact_sha256",
        "source_geometry_artifact_sha256",
        "material_surface_quality_id",
        "appearance_source_lineage_sidecar_object_sha256",
        "rest_frame",
        "pose_action",
        "sampling_policy",
        "idempotency_key",
    ] {
        assert!(
            required.iter().any(|value| value == field),
            "missing {field}"
        );
    }
    assert_eq!(
        AgenticTool::from_name("mechanical_animation_clip_v2_prepare")
            .expect("prepare enum")
            .runtime_method(),
        "mechanical_animation_clip_v2_prepare"
    );
    assert_eq!(
        AgenticTool::from_name("mechanical_animation_clip_v2_get")
            .expect("get enum")
            .runtime_method(),
        "mechanical_animation_clip_v2_get"
    );
    assert_eq!(
        AgenticTool::from_name("mechanical_animation_clip_v2_preview")
            .expect("preview enum")
            .runtime_method(),
        "mechanical_animation_clip_v2_preview"
    );

    let get_request = json!({
        "schema_version":"MechanicalAnimationClipGetRequest@2",
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "clip_id":"clip-1"
    });
    assert!(validate_call(
        "mechanical_animation_clip_v2_get",
        &get_request,
        &Binding::default()
    )
    .is_ok());
    let preview_request = json!({
        "schema_version":"MechanicalAnimationClipPreviewRequest@2",
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "clip_id":"clip-1",
        "sample_time_ticks":0,
        "preview_policy":"single-tick-transient-geometry-plus-appearance-double-worker-replay@1",
        "canonical_sha256":"a".repeat(64)
    });
    assert!(validate_call(
        "mechanical_animation_clip_v2_preview",
        &preview_request,
        &Binding::default()
    )
    .is_ok());
    let prepare_scope = json!({
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1"
    });
    assert!(validate_call(
        "mechanical_animation_clip_v2_prepare",
        &prepare_scope,
        &Binding::default()
    )
    .is_err());
    let appearance_binding = Binding {
        session_id: Some("session-1".to_owned()),
        project_id: Some("project-1".to_owned()),
        candidate_id: Some("appearance-1".to_owned()),
    };
    assert!(validate_call(
        "mechanical_animation_clip_v2_prepare",
        &prepare_scope,
        &appearance_binding
    )
    .is_ok());
    let mut mismatch = prepare_scope.clone();
    mismatch["appearance_candidate_id"] = json!("appearance-other");
    assert!(validate_call(
        "mechanical_animation_clip_v2_prepare",
        &mismatch,
        &appearance_binding
    )
    .is_err());
    let mut raw = prepare_scope;
    raw["raw_glb_bytes"] = json!("AA==");
    assert!(validate_call(
        "mechanical_animation_clip_v2_prepare",
        &raw,
        &appearance_binding
    )
    .is_err());

    let hash = "a".repeat(64);
    let preview_response = json!({
        "schema_version":"MechanicalAnimationClipPreview@2",
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "appearance_candidate_state_sha256":hash,
        "appearance_artifact_sha256":hash,
        "appearance_artifact_readback_sha256":hash,
        "appearance_artifact_readback_object_sha256":hash,
        "source_geometry_candidate_id":"geometry-1",
        "source_geometry_candidate_state_sha256":hash,
        "source_geometry_artifact_sha256":hash,
        "source_geometry_candidate_evidence_sha256":hash,
        "clip_id":"clip-1",
        "clip_object_sha256":hash,
        "clip_sha256":hash,
        "rest_frame_sha256":hash,
        "pose_action_sha256":hash,
        "sample_time_ticks":0,
        "frame_sha256":hash,
        "source_replay_worker_cohort_sha256":hash,
        "appearance_transient_artifact_sha256":hash,
        "appearance_transient_artifact_readback_sha256":hash,
        "appearance_replay_worker_cohort_sha256":hash,
        "appearance_program_sha256":hash,
        "appearance_transient_program_sha256":hash,
        "material_pack_manifest_sha256":hash,
        "geometry_preservation_projection_sha256":hash,
        "pose_geometry_preview":{
            "project_id":"project-1",
            "candidate_id":"geometry-1",
            "source_artifact_id":hash,
            "posed_program_sha256":hash,
            "runtime_write_performed":false,
            "validator_status":"passed",
            "quality_status":"structural_only"
        },
        "geometry_materialization":"transient-double-worker-glb-not-persisted",
        "appearance_materialization":"transient-double-worker-appearance-not-persisted",
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "limitations":[
            "rigid-parts-only-no-skinning-or-deformation",
            "single-scheduled-tick-per-preview-call",
            "transient-geometry-and-appearance-not-persisted",
            "no-ik-constraints-nla-fcurves-drivers-or-timeline",
            "not-blender-armature-animation-or-python-parity",
            "structural-replay-does-not-prove-visual-quality"
        ],
        "canonical_sha256":hash
    });
    assert!(validate_response(
        "mechanical_animation_clip_v2_preview",
        &preview_response,
        &appearance_binding
    )
    .is_ok());
    let mut tampered_preview = preview_response;
    tampered_preview["appearance_transient_program_sha256"] = Value::String("b".repeat(64));
    assert!(validate_response(
        "mechanical_animation_clip_v2_preview",
        &tampered_preview,
        &appearance_binding
    )
    .is_err());
}

#[test]
fn mechanical_animation_glb_v2_surface_is_closed_hidden_write_and_restart_read_only() {
    let read = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "mechanical_animation_glb_v2_get")
        .expect("appearance-aware animated GLB get tool");
    assert_eq!(read["annotations"]["readOnlyHint"], true);
    assert_eq!(read["annotations"]["writeIntent"], false);
    assert_eq!(read["annotations"]["approvalRequired"], false);
    assert_eq!(read["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(read["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        read["inputSchema"]["required"],
        json!([
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id"
        ])
    );
    assert!(read["description"]
        .as_str()
        .is_some_and(|description| description.contains("raw GLB")));

    let write = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "mechanical_animation_glb_v2_prepare")
        .expect("appearance-aware animated GLB prepare tool");
    assert_eq!(write["annotations"]["readOnlyHint"], false);
    assert_eq!(write["annotations"]["writeIntent"], true);
    assert_eq!(write["annotations"]["approvalRequired"], false);
    assert_eq!(write["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(write["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        write["inputSchema"]["properties"]["materialization_policy"]["const"],
        "appearance-aware-rigid-node-trs-gltf-linear-scheduled-samples@2"
    );
    let required = write["inputSchema"]["required"]
        .as_array()
        .expect("animated GLB required fields");
    assert_eq!(required.len(), 10);
    assert!(required
        .iter()
        .all(|field| !matches!(field.as_str(), Some("approved" | "approval_receipt_id"))));
    assert_eq!(
        runtime_method("mechanical_animation_glb_v2_prepare"),
        Some("mechanical_animation_glb_v2_prepare")
    );
    assert_eq!(
        runtime_method("mechanical_animation_glb_v2_get"),
        Some("mechanical_animation_glb_v2_get")
    );

    let hash = "a".repeat(64);
    let get_request = json!({
        "schema_version":"MechanicalAnimationGlbGetRequest@2",
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "clip_id":"clip-1"
    });
    assert!(validate_call(
        "mechanical_animation_glb_v2_get",
        &get_request,
        &Binding::default()
    )
    .is_ok());
    let prepare_request = json!({
        "schema_version":"MechanicalAnimationGlbPrepareRequest@2",
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "appearance_candidate_state_sha256":hash,
        "clip_id":"clip-1",
        "clip_object_sha256":"b".repeat(64),
        "clip_sha256":"c".repeat(64),
        "materialization_policy":"appearance-aware-rigid-node-trs-gltf-linear-scheduled-samples@2",
        "input_sha256":"d".repeat(64),
        "idempotency_key":"animation-glb-key-1"
    });
    assert!(validate_call(
        "mechanical_animation_glb_v2_prepare",
        &prepare_request,
        &Binding::default()
    )
    .is_err());
    let appearance_binding = Binding {
        session_id: Some("session-1".to_owned()),
        project_id: Some("project-1".to_owned()),
        candidate_id: Some("appearance-1".to_owned()),
    };
    assert!(validate_call(
        "mechanical_animation_glb_v2_prepare",
        &prepare_request,
        &appearance_binding
    )
    .is_ok());
    let mut mismatch = prepare_request.clone();
    mismatch["appearance_candidate_id"] = json!("appearance-other");
    assert!(validate_call(
        "mechanical_animation_glb_v2_prepare",
        &mismatch,
        &appearance_binding
    )
    .is_err());
    let mut raw_input = prepare_request.clone();
    raw_input["script"] = json!("bpy.ops.object.export_scene.gltf()");
    assert!(validate_call(
        "mechanical_animation_glb_v2_prepare",
        &raw_input,
        &appearance_binding
    )
    .is_err());

    let mut receipt = json!({
        "schema_version":"MechanicalAnimationGlbReceipt@2",
        "project_id":"project-1",
        "appearance_candidate_id":"appearance-1",
        "appearance_artifact_id":"appearance-artifact-1",
        "source_geometry_candidate_id":"geometry-1",
        "source_geometry_artifact_id":"geometry-artifact-1",
        "material_surface_quality_id":"quality-1",
        "material_pack_id":"pack-1",
        "material_pack_version":"1.0.0",
        "material_pack_license_spdx":"CC0-1.0",
        "clip_id":"clip-1",
        "sample_time_ticks":[0, 1000],
        "timebase_hz":1000,
        "interpolation":"LINEAR",
        "part_ids":["root"],
        "node_count":1,
        "sampler_count":2,
        "channel_count":2,
        "accessor_count_added":3,
        "buffer_view_count_added":3,
        "source_static_projection_exact":true,
        "binary_prefix_exact":true,
        "appearance_material_projection_exact":true,
        "material_pack_identity_exact":true,
        "no_skinning":true,
        "no_morph_targets":true,
        "validator_status":"strict-appearance-aware-rigid-gltf-animation-readback-pass",
        "hard_gate_passed":true,
        "materialization_status":"runtime-owned-cas-appearance-aware-animated-glb",
        "runtime_write_performed":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "limitations":["rigid-parts-only"],
        "created_at":"2026-08-22T00:00:00Z"
    });
    for field in [
        "animation_glb_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "material_pack_manifest_object_sha256",
        "material_pack_manifest_sha256",
        "material_pack_provenance_sha256",
        "texture_build_receipt_object_sha256",
        "texture_build_receipt_canonical_sha256",
        "candidate_surface_bake_receipt_object_sha256",
        "candidate_surface_bake_receipt_canonical_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "sampling_policy_sha256",
        "source_replay_worker_cohort_sha256",
        "frame_preview_hashes_sha256",
        "frame_preview_worker_cohort_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_validation_sha256",
        "source_static_projection_sha256",
        "appearance_material_projection_sha256",
        "canonical_sha256",
    ] {
        receipt[field] = Value::String("e".repeat(64));
    }
    receipt["animation_glb_key_sha256"] = Value::String("a".repeat(64));
    receipt["appearance_candidate_state_sha256"] = Value::String("b".repeat(64));
    receipt["animated_artifact_sha256"] = Value::String("c".repeat(64));

    let mut durable_link = receipt.clone();
    durable_link["schema_version"] = json!("MechanicalAnimationGlbLink@2");
    durable_link["receipt_object_sha256"] = Value::String("d".repeat(64));
    durable_link["receipt_canonical_sha256"] = receipt["canonical_sha256"].clone();
    durable_link["request_sha256"] = Value::String("f".repeat(64));
    let response = json!({
        "schema_version":"MechanicalAnimationGlbPrepareResult@2",
        "animation_glb_key_sha256":"a".repeat(64),
        "animated_artifact_sha256":"c".repeat(64),
        "animated_artifact_size_bytes":1024,
        "receipt_object_sha256":"d".repeat(64),
        "receipt":receipt,
        "durable_link":durable_link,
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write_performed":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only"
    });
    assert!(validate_response(
        "mechanical_animation_glb_v2_prepare",
        &response,
        &appearance_binding
    )
    .is_ok());
    let mut get_response = response.clone();
    get_response["schema_version"] = json!("MechanicalAnimationGlbGetResult@2");
    get_response["replayed"] = json!(false);
    get_response["runtime_write_performed"] = json!(false);
    assert!(validate_response(
        "mechanical_animation_glb_v2_get",
        &get_response,
        &Binding::default()
    )
    .is_ok());
    for forbidden in ["raw_glb_bytes", "png_base64", "path", "url", "script"] {
        let mut tampered = get_response.clone();
        tampered[forbidden] = json!("not-allowed");
        assert!(
            validate_response(
                "mechanical_animation_glb_v2_get",
                &tampered,
                &Binding::default()
            )
            .is_err(),
            "forbidden field {forbidden} must fail closed"
        );
    }
    let mut unsafe_flags = get_response;
    unsafe_flags["export_performed"] = json!(true);
    assert!(validate_response(
        "mechanical_animation_glb_v2_get",
        &unsafe_flags,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_socket_materialization_v2_response_is_structural_and_fails_closed() {
    let appearance_binding = Binding {
        session_id: Some("session-1".to_owned()),
        project_id: Some("project-1".to_owned()),
        candidate_id: Some("appearance-1".to_owned()),
    };
    let prepare = animated_socket_v2_response(true);
    assert!(validate_response(
        "game_weapon_animated_glb_socket_v2_prepare",
        &prepare,
        &appearance_binding
    )
    .is_ok());
    let get = animated_socket_v2_response(false);
    assert!(validate_response(
        "game_weapon_animated_glb_socket_v2_get",
        &get,
        &Binding::default()
    )
    .is_ok());

    for forbidden in ["raw_glb_bytes", "base64", "path", "url", "script"] {
        let mut tampered = get.clone();
        tampered[forbidden] = json!("not-allowed");
        assert!(
            validate_response(
                "game_weapon_animated_glb_socket_v2_get",
                &tampered,
                &Binding::default()
            )
            .is_err(),
            "forbidden field {forbidden} must fail closed"
        );
    }
    let mut missing_link = get.clone();
    missing_link
        .as_object_mut()
        .expect("response object")
        .remove("durable_link");
    assert!(validate_response(
        "game_weapon_animated_glb_socket_v2_get",
        &missing_link,
        &Binding::default()
    )
    .is_err());
    let mut unsafe_restart = get.clone();
    unsafe_restart["restart_hash_verified"] = json!(false);
    assert!(validate_response(
        "game_weapon_animated_glb_socket_v2_get",
        &unsafe_restart,
        &Binding::default()
    )
    .is_err());
    let mut unsafe_write = prepare;
    unsafe_write["candidate_confirmed"] = json!(true);
    assert!(validate_response(
        "game_weapon_animated_glb_socket_v2_prepare",
        &unsafe_write,
        &appearance_binding
    )
    .is_err());
    let mut cross_candidate = get;
    cross_candidate["receipt"]["appearance_candidate_id"] = json!("appearance-other");
    assert!(validate_response(
        "game_weapon_animated_glb_socket_v2_get",
        &cross_candidate,
        &appearance_binding
    )
    .is_err());
}

#[test]
fn animated_socket_v2_reuses_parent_glb_counts_and_rejects_zero_counts() {
    let appearance_binding = Binding {
        session_id: Some("session-1".to_owned()),
        project_id: Some("project-1".to_owned()),
        candidate_id: Some("appearance-1".to_owned()),
    };
    let parent_accessor_count_added = 7;
    let parent_buffer_view_count_added = 11;

    for (is_prepare, tool, binding, expected_runtime_write) in [
        (
            true,
            "game_weapon_animated_glb_socket_v2_prepare",
            &appearance_binding,
            true,
        ),
        (
            false,
            "game_weapon_animated_glb_socket_v2_get",
            &Binding::default(),
            false,
        ),
    ] {
        let response = animated_socket_v2_response_with_parent_counts(
            is_prepare,
            parent_accessor_count_added,
            parent_buffer_view_count_added,
        );
        assert_eq!(
            response["receipt"]["accessor_count_added"],
            json!(parent_accessor_count_added)
        );
        assert_eq!(
            response["receipt"]["buffer_view_count_added"],
            json!(parent_buffer_view_count_added)
        );
        assert_eq!(
            response["runtime_write_performed"],
            json!(expected_runtime_write)
        );
        assert!(
            response.get("receipt").is_some(),
            "{tool} must include receipt"
        );
        assert!(
            response.get("durable_link").is_some(),
            "{tool} must include durable_link"
        );
        assert!(
            response
                .get("animated_socket_materialization_key_sha256")
                .is_some(),
            "{tool} must include materialization key"
        );
        assert!(
            response
                .get("derived_animated_socket_artifact_sha256")
                .is_some(),
            "{tool} must include derived artifact hash"
        );
        assert!(
            response.get("receipt_object_sha256").is_some(),
            "{tool} must include receipt object hash"
        );
        assert!(validate_response(tool, &response, binding).is_ok());

        for field in ["accessor_count_added", "buffer_view_count_added"] {
            let mut zero_count = response.clone();
            zero_count["receipt"][field] = json!(0);
            assert!(
                validate_response(tool, &zero_count, binding).is_err(),
                "{tool} must reject zero parent {field}"
            );
        }
    }
}

#[test]
fn new_session_requires_null_resume_and_explicit_approval() {
    let mut request = json!({
        "session_id": null,
        "project_id": "project-1",
        "candidate_id": "candidate-1",
        "idempotency_key": "idem-1"
    });
    assert!(validate_call("session_create_or_resume", &request, &Binding::default()).is_err());
    request["approved"] = Value::Bool(true);
    request["approval_receipt_id"] = Value::String("approval-1".to_owned());
    request["approval_summary"] = Value::String("approved".to_owned());
    assert!(validate_call("session_create_or_resume", &request, &Binding::default()).is_ok());
}

#[test]
fn cross_project_candidate_and_unknown_visual_state_fail_closed() {
    let mut request = json!({
        "session_id": "session-1",
        "project_id": "project-other",
        "candidate_id": "candidate-other",
        "visual_state": "unknown",
        "evidence_sha256": "a".repeat(64),
        "idempotency_key": "idem-1"
    });
    request
        .as_object_mut()
        .unwrap()
        .extend(approval().as_object().unwrap().clone());
    let error = validate_call("checkpoint_prepare", &request, &bound()).unwrap_err();
    assert!(error.starts_with("AGENTIC_SCOPE_MISMATCH"));
    request["project_id"] = Value::String("project-1".to_owned());
    request["candidate_id"] = Value::String("candidate-1".to_owned());
    let error = validate_call("checkpoint_prepare", &request, &bound()).unwrap_err();
    assert!(error.starts_with("AGENTIC_VISUAL_STATE_UNKNOWN"));
}

#[test]
fn runtime_response_must_keep_scope() {
    let response = json!({
        "session_id":"session-1",
        "project_id":"project-2",
        "candidate_id":"candidate-1"
    });
    let error = validate_response("session_get", &response, &bound()).unwrap_err();
    assert!(error.starts_with("AGENTIC_SCOPE_MISMATCH"));
}

#[test]
fn readback_can_rebind_a_fresh_mcp_session() {
    let checkpoint_request = json!({
        "checkpoint_id": "checkpoint-1",
        "session_id": "session-1",
        "project_id": "project-1",
        "candidate_id": "candidate-1"
    });
    assert!(validate_call("checkpoint_get", &checkpoint_request, &Binding::default()).is_ok());
    let session_request = json!({
        "session_id": "session-1",
        "project_id": "project-1",
        "candidate_id": "candidate-1"
    });
    assert!(validate_call("session_get", &session_request, &Binding::default()).is_ok());
}

#[test]
fn unavailable_error_names_assumed_runtime_method() {
    assert_eq!(
            unavailable_error("checkpoint_prepare"),
            "AGENTIC_RUNTIME_METHOD_UNAVAILABLE: checkpoint_prepare requires Runtime method checkpoint_prepare"
        );
}

#[test]
fn production_stage_transition_is_approval_gated_and_scope_bound() {
    let mut request = json!({
        "schema_version":"ProductionStageTransitionPrepareRequest@1",
        "transition_id":"transition-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "from_stage":"draft",
        "to_stage":"gray-model",
        "candidate_state_sha256":"a".repeat(64),
        "artifact_sha256":"b".repeat(64),
        "output_kind":"gray-model-artifact",
        "output_object_sha256":"b".repeat(64),
        "quality_report_object_sha256":null,
        "comparison_report_object_sha256":null,
        "reference_id":"reference-1",
        "reference_sha256":"c".repeat(64),
        "camera_hash":"d".repeat(64),
        "evidence_sha256":"e".repeat(64),
        "parent_checkpoint_id":null,
        "parent_checkpoint_sha256":null,
        "input_sha256":"f".repeat(64),
        "approval_expires_at":"2026-08-21T23:59:59Z",
        "approval_session_id":"session-1",
        "idempotency_key":"production-stage-1"
    });
    assert!(validate_call("production_stage_transition_prepare", &request, &bound()).is_err());
    request
        .as_object_mut()
        .unwrap()
        .extend(approval().as_object().unwrap().clone());
    assert!(validate_call("production_stage_transition_prepare", &request, &bound()).is_ok());
    request["candidate_id"] = Value::String("candidate-other".to_owned());
    assert!(
        validate_call("production_stage_transition_prepare", &request, &bound())
            .unwrap_err()
            .starts_with("AGENTIC_SCOPE_MISMATCH")
    );
}

#[test]
fn production_stage_transition_get_can_restart_read_exact_scope() {
    let request = json!({
        "schema_version":"ProductionStageTransitionGetRequest@1",
        "transition_id":"transition-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "production_stage_transition_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let schema = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_stage_transition_get")
        .expect("production stage read tool");
    assert_eq!(schema["annotations"]["readOnlyHint"], true);
    assert_eq!(
        schema["inputSchema"]["required"],
        json!([
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "candidate_id"
        ])
    );
}

#[test]
fn production_stage_transition_v2_prepare_is_hidden_approval_gated_and_root_bound() {
    let mut request = json!({
        "schema_version":"ProductionStageTransitionPrepareRequest@2",
        "session_id":"session-1",
        "project_id":"project-1",
        "root_candidate_id":"candidate-1",
        "head_candidate_id":"candidate-material-1",
        "approved":true,
        "approval_receipt_id":"approval-1",
        "approval_summary":"promote passed topology to material surface",
        "idempotency_key":"transition-v2-1"
    });
    assert!(validate_call(
        "production_stage_transition_v2_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call("production_stage_transition_v2_prepare", &request, &bound()).is_ok());
    request["head_candidate_id"] = Value::String("candidate-1".to_owned());
    assert!(
        validate_call("production_stage_transition_v2_prepare", &request, &bound())
            .unwrap_err()
            .contains("must be distinct")
    );
    request["head_candidate_id"] = Value::String("candidate-material-1".to_owned());
    request["root_candidate_id"] = Value::String("candidate-other".to_owned());
    assert!(
        validate_call("production_stage_transition_v2_prepare", &request, &bound())
            .unwrap_err()
            .starts_with("AGENTIC_SCOPE_MISMATCH")
    );

    let reads = read_tools();
    assert!(!reads
        .iter()
        .any(|tool| tool["name"] == "production_stage_transition_v2_prepare"));
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_stage_transition_v2_prepare")
        .expect("V2 production-stage prepare tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], true);
    assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], true);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["properties"]["from_stage"],
        json!({"const":"topology"})
    );
}

#[test]
fn production_stage_transition_v2_get_is_read_only_and_restart_safe() {
    let request = json!({
        "schema_version":"ProductionStageTransitionGetRequest@2",
        "transition_id":"transition-v2-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "root_candidate_id":"candidate-1",
        "head_candidate_id":"candidate-material-1"
    });
    assert!(validate_call(
        "production_stage_transition_v2_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let reads = read_tools();
    let get = reads
        .iter()
        .find(|tool| tool["name"] == "production_stage_transition_v2_get")
        .expect("V2 production-stage get tool");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert!(!write_tools()
        .iter()
        .any(|tool| tool["name"] == "production_stage_transition_v2_get"));
    assert_eq!(
        get["inputSchema"]["required"],
        json!([
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "root_candidate_id",
            "head_candidate_id"
        ])
    );
}

#[test]
fn production_stage_transition_v3_prepare_is_hidden_approval_gated_and_same_bound() {
    let mut request = json!({
        "schema_version":"ProductionStageTransitionPrepareRequest@3",
        "session_id":"session-1",
        "project_id":"project-1",
        "root_candidate_id":"candidate-1",
        "head_candidate_id":"candidate-1",
        "approved":true,
        "approval_receipt_id":"approval-1",
        "approval_summary":"review complete reference coverage",
        "idempotency_key":"transition-v3-1"
    });
    assert!(validate_call(
        "production_stage_transition_v3_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call("production_stage_transition_v3_prepare", &request, &bound()).is_ok());
    request["head_candidate_id"] = Value::String("candidate-other".to_owned());
    assert!(
        validate_call("production_stage_transition_v3_prepare", &request, &bound())
            .unwrap_err()
            .contains("same candidate")
    );

    let reads = read_tools();
    assert!(!reads
        .iter()
        .any(|tool| tool["name"] == "production_stage_transition_v3_prepare"));
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_stage_transition_v3_prepare")
        .expect("V3 production-stage prepare tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], true);
    assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], true);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["properties"]["to_stage"],
        json!({"enum":["reference-coverage-reviewed","camera-calibrated"]})
    );
}

#[test]
fn production_stage_transition_v3_camera_output_requires_lock_binding_and_keeps_stage_flags() {
    let hash = "a".repeat(64);
    let mut transition = Map::new();
    for field in [
        "root_candidate_state_sha256",
        "root_artifact_sha256",
        "previous_head_candidate_state_sha256",
        "previous_head_artifact_sha256",
        "head_candidate_state_sha256",
        "head_artifact_sha256",
        "reference_sha256",
        "camera_hash",
        "evidence_sha256",
        "reference_canvas_object_sha256",
        "design_spec_object_sha256",
        "approval_summary_sha256",
        "request_key_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        transition.insert(field.to_owned(), Value::String(hash.clone()));
    }
    for field in [
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
    ] {
        transition.insert(field.to_owned(), Value::String(hash.clone()));
    }
    for (field, value) in [
        ("schema_version", "ProductionStageTransition@3"),
        ("transition_id", "transition-camera-1"),
        ("session_id", "session-1"),
        ("project_id", "project-1"),
        ("root_candidate_id", "candidate-1"),
        ("root_candidate_role", "reference-intake-candidate"),
        ("source_artifact_id", "artifact-1"),
        ("previous_head_candidate_id", "candidate-1"),
        ("previous_head_candidate_role", "reference-intake-candidate"),
        ("previous_head_artifact_id", "artifact-1"),
        ("previous_head_stage", "reference-coverage-reviewed"),
        ("head_candidate_id", "candidate-1"),
        ("head_candidate_role", "reference-intake-candidate"),
        ("output_artifact_id", "artifact-1"),
        ("from_stage", "reference-coverage-reviewed"),
        ("to_stage", "camera-calibrated"),
        ("candidate_binding_status", "same-candidate-evidence"),
        ("reference_id", "reference-1"),
        ("camera_lock_id", "camera-lock-1"),
        ("camera_lock_source_transition_id", "transition-coverage-1"),
        ("reference_canvas_object_sha256", &hash),
        ("design_spec_object_sha256", &hash),
        ("structural_status", "PASS_SOURCE_STRUCTURAL"),
        ("visual_status", "QUALITY_TARGET_NOT_MET"),
        ("human_status", "NOT_RUN"),
        ("engine_status", "NOT_RUN"),
        ("distribution_status", "NOT_RUN"),
        ("approval_receipt_id", "approval-1"),
        ("approval_session_id", "session-1"),
        ("approval_expires_at", "9999999999"),
        ("parent_transition_id", "transition-coverage-1"),
        ("parent_transition_sha256", &hash),
        (
            "parent_transition_schema_version",
            "ProductionStageTransition@3",
        ),
        ("gate_status", "pass"),
        ("status", "passed"),
        ("created_at", "2026-08-23T00:00:00Z"),
    ] {
        transition.insert(field.to_owned(), Value::String(value.to_owned()));
    }
    for field in [
        "quality_report_object_sha256",
        "comparison_report_object_sha256",
        "visual_receipt_object_sha256",
        "human_review_receipt_object_sha256",
        "engine_validation_receipt_object_sha256",
        "distribution_receipt_object_sha256",
    ] {
        transition.insert(field.to_owned(), Value::Null);
    }

    let mut head = transition.clone();
    head.remove("transition_id");
    head.remove("session_id");
    head.remove("project_id");
    head.remove("from_stage");
    head.remove("to_stage");
    head.remove("request_key_sha256");
    head.remove("input_sha256");
    head.remove("receipt_object_sha256");
    head.remove("parent_transition_id");
    head.remove("parent_transition_sha256");
    head.remove("parent_transition_schema_version");
    head.remove("gate_status");
    head.remove("status");
    head.remove("created_at");
    head.insert(
        "session_id".to_owned(),
        Value::String("session-1".to_owned()),
    );
    head.insert(
        "schema_version".to_owned(),
        Value::String("ProductionStageHead@3".to_owned()),
    );
    head.insert(
        "project_id".to_owned(),
        Value::String("project-1".to_owned()),
    );
    head.insert(
        "root_stage".to_owned(),
        Value::String("reference-intake".to_owned()),
    );
    head.insert(
        "head_stage".to_owned(),
        Value::String("camera-calibrated".to_owned()),
    );
    head.insert(
        "head_transition_id".to_owned(),
        Value::String("transition-camera-1".to_owned()),
    );
    head.insert(
        "head_transition_sha256".to_owned(),
        Value::String(hash.clone()),
    );
    head.insert(
            "compatibility_projection".to_owned(),
            json!({
                "schema_version":"ProductionStageCompatibilityProjection@3",
                "source_schema_version":"ProductionStageHead@3",
                "v3_stage":"camera-calibrated",
                "v3_stage_complete":true,
                "v1_projection_stage":null,
                "v1_projection_complete":false,
                "v2_projection_stage":null,
                "v2_projection_complete":false,
                "projection_status":"not-proven",
                "legacy_head_transition_id":null,
                "legacy_head_transition_sha256":null,
                "projection_policy_sha256":"3855241e8e3bba0b4966beda1f29ee7aea5e54eb6d66bc5aa961cec6d738d9f6"
            }),
        );
    for field in ["candidate_confirmed", "version_created", "export_performed"] {
        head.insert(field.to_owned(), Value::Bool(false));
    }
    head.insert(
        "materialization_status".to_owned(),
        Value::String("runtime-owned-durable-production-stage-head-v3".to_owned()),
    );
    head.insert("payload_json".to_owned(), Value::String("{}".to_owned()));
    head.insert(
        "updated_at".to_owned(),
        Value::String("2026-08-23T00:00:00Z".to_owned()),
    );

    let projection = head["compatibility_projection"].clone();
    let response = json!({
        "schema_version":"ProductionStageTransitionPrepareResult@3",
        "transition":Value::Object(transition.clone()),
        "production_stage_head":Value::Object(head.clone()),
        "compatibility_projection":projection,
        "replayed":false,
        "runtime_write":true,
        "production_stage_advanced":true,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(
        "production_stage_transition_v3_prepare",
        &response,
        &bound()
    )
    .is_ok());
    let mut tampered = response;
    tampered["transition"]["camera_lock_canonical_sha256"] = Value::Null;
    assert!(validate_response(
        "production_stage_transition_v3_prepare",
        &tampered,
        &bound()
    )
    .is_err());
}

#[test]
fn production_stage_transition_v3_get_is_read_only_and_fresh_process_safe() {
    let request = json!({
        "schema_version":"ProductionStageTransitionGetRequest@3",
        "transition_id":"transition-v3-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "root_candidate_id":"candidate-1",
        "head_candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "production_stage_transition_v3_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_stage_transition_v3_get")
        .expect("V3 production-stage get tool");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert!(!write_tools()
        .iter()
        .any(|tool| tool["name"] == "production_stage_transition_v3_get"));
    assert_eq!(
        get["inputSchema"]["required"],
        json!([
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "root_candidate_id",
            "head_candidate_id"
        ])
    );
}

#[test]
fn production_camera_lock_prepare_is_hidden_closed_and_requires_independent_approval() {
    let mut request = json!({
        "schema_version":"ProductionCameraLockPrepareRequest@1",
        "camera_lock_id":"camera-lock-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "approved":true,
        "approval_receipt_id":"approval-camera-1",
        "approval_session_id":"session-1",
        "approval_expires_at":"4102444800",
        "approval_summary":"six references and seven cameras reviewed",
        "idempotency_key":"camera-lock-key-1"
    });
    assert!(validate_call(
        "production_camera_lock_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call("production_camera_lock_prepare", &request, &bound()).is_ok());
    request["approval_session_id"] = Value::String("session-other".to_owned());
    assert!(
        validate_call("production_camera_lock_prepare", &request, &bound())
            .unwrap_err()
            .contains("APPROVAL_SESSION_MISMATCH")
    );

    let reads = read_tools();
    assert!(!reads
        .iter()
        .any(|tool| tool["name"] == "production_camera_lock_prepare"));
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_camera_lock_prepare")
        .expect("camera lock prepare tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], true);
    assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], true);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["properties"]["approved"],
        json!({"const":true})
    );
    assert_eq!(
        prepare["inputSchema"]["properties"]["required_reference_view_kinds"],
        json!({"const":["front","back","left","right","top","rear-three-quarter"]})
    );
}

#[test]
fn production_camera_lock_get_is_read_only_and_rejects_forbidden_transport() {
    let request = json!({
        "schema_version":"ProductionCameraLockGetRequest@1",
        "camera_lock_id":"camera-lock-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call("production_camera_lock_get", &request, &Binding::default()).is_ok());
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_camera_lock_get")
        .expect("camera lock get tool");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert!(!write_tools()
        .iter()
        .any(|tool| tool["name"] == "production_camera_lock_get"));
    let mut forbidden = request;
    forbidden["url"] = Value::String("https://invalid".to_owned());
    assert!(validate_call(
        "production_camera_lock_get",
        &forbidden,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn production_camera_lock_response_requires_exact_profiles_and_no_stage_advance() {
    let hash = "a".repeat(64);
    let mut lock = json!({
        "schema_version":"ProductionCameraLock@1",
        "camera_lock_id":"camera-lock-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "source_transition_id":"transition-v3-1",
        "source_transition_sha256":hash,
        "source_head_canonical_sha256":hash,
        "candidate_id":"candidate-1",
        "candidate_state_sha256":hash,
        "artifact_id":"artifact-1",
        "artifact_sha256":hash,
        "reference_id":"reference-1",
        "reference_sha256":hash,
        "reference_canvas_object_sha256":hash,
        "reference_canvas_canonical_sha256":hash,
        "design_spec_object_sha256":hash,
        "design_spec_canonical_sha256":hash,
        "camera_rig_object_sha256":hash,
        "camera_rig_canonical_sha256":hash,
        "required_reference_view_kinds":["front","back","left","right","top","rear-three-quarter"],
        "required_camera_view_kinds":["front","back","left","right","top","bottom","rear-three-quarter"],
        "primary_view_kind":"left",
        "calibration_policy":"fps-weapon-reviewed-six-reference-seven-camera-lock@1",
        "review_status":"user-approved-reference-coverage",
        "calibration_status":"passed",
        "structural_status":"PASS_SOURCE_STRUCTURAL",
        "visual_status":"QUALITY_TARGET_NOT_MET",
        "human_status":"NOT_RUN",
        "engine_status":"NOT_RUN",
        "distribution_status":"NOT_RUN",
        "approval_receipt_id":"approval-camera-1",
        "approval_session_id":"session-1",
        "approval_expires_at":"4102444800",
        "approval_summary_sha256":hash,
        "input_sha256":hash,
        "request_key_sha256":hash,
        "receipt_object_sha256":hash,
        "canonical_sha256":hash,
        "created_at":"2026-08-23T00:00:00Z"
    });
    let mut response = json!({
        "schema_version":"ProductionCameraLockGetResult@1",
        "camera_lock":lock,
        "replayed":false,
        "runtime_write":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "restart_hash_verified":true
    });
    assert!(
        validate_response("production_camera_lock_get", &response, &Binding::default()).is_ok()
    );
    response["production_stage_advanced"] = Value::Bool(true);
    assert!(
        validate_response("production_camera_lock_get", &response, &Binding::default()).is_err()
    );
    lock["required_camera_view_kinds"] = json!([
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter"
    ]);
    response["camera_lock"] = lock;
    assert!(
        validate_response("production_camera_lock_get", &response, &Binding::default()).is_err()
    );
}

#[test]
fn production_stage_transition_v2_schema_freezes_epoch_expiry_and_opaque_ids() {
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_stage_transition_v2_get")
        .expect("V2 production-stage get tool");
    let get_properties = &get["inputSchema"]["properties"];
    assert_eq!(
        get_properties["transition_id"]["pattern"],
        "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    );
    assert_eq!(
        get_properties["root_candidate_id"]["pattern"],
        "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    );

    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "production_stage_transition_v2_prepare")
        .expect("V2 production-stage prepare tool");
    let properties = &prepare["inputSchema"]["properties"];
    assert_eq!(
        properties["approval_expires_at"]["pattern"],
        "^[0-9]{1,10}$"
    );
    assert_eq!(
        properties["approval_receipt_id"]["pattern"],
        "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    );
    assert_eq!(
        properties["idempotency_key"]["pattern"],
        "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    );
}

#[test]
fn production_stage_transition_v2_response_requires_nested_binding_and_safe_flags() {
    let response = json!({
        "schema_version":"ProductionStageTransitionPrepareResult@2",
        "transition":{
            "schema_version":"ProductionStageTransition@2",
            "transition_id":"transition-v2-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "root_candidate_id":"candidate-1",
            "root_candidate_role":"topology-source",
            "head_candidate_id":"candidate-material-1",
            "head_candidate_role":"material-surface-output",
            "from_stage":"topology",
            "to_stage":"material-surface",
            "candidate_binding_status":"distinct-root-topology-to-material-surface-head",
            "topology_quality_status":"passed",
            "material_surface_quality_status":"passed",
            "gate_status":"pass",
            "status":"passed"
        },
        "production_stage_head":{
            "schema_version":"ProductionStageHead@2",
            "session_id":"session-1",
            "project_id":"project-1",
            "root_candidate_id":"candidate-1",
            "root_candidate_role":"topology-source",
            "root_stage":"topology",
            "head_candidate_id":"candidate-material-1",
            "head_candidate_role":"material-surface-output",
            "head_stage":"material-surface",
            "candidate_binding_status":"distinct-root-topology-to-material-surface-head",
            "topology_quality_status":"passed",
            "material_surface_quality_status":"passed",
            "head_transition_id":"transition-v2-1",
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false
        },
        "runtime_write":true,
        "production_stage_advanced":true,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(
        "production_stage_transition_v2_prepare",
        &response,
        &bound()
    )
    .is_ok());
    let mut mismatched = response.clone();
    mismatched["production_stage_head"]["root_candidate_id"] =
        Value::String("candidate-other".to_owned());
    assert!(validate_response(
        "production_stage_transition_v2_prepare",
        &mismatched,
        &bound()
    )
    .unwrap_err()
    .contains("dual-candidate binding"));
    let mut unsafe_flags = response;
    unsafe_flags["production_stage_advanced"] = Value::Bool(false);
    assert!(validate_response(
        "production_stage_transition_v2_prepare",
        &unsafe_flags,
        &bound()
    )
    .unwrap_err()
    .contains("side-effect flags"));
}

#[test]
fn candidate_topology_quality_prepare_is_hidden_write_and_scope_bound() {
    let request = json!({
        "schema_version":"CandidateTopologyQualityPrepareRequest@1",
        "topology_quality_id":"topology-quality-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "candidate_state_sha256":"a".repeat(64),
        "artifact_id":"artifact-1",
        "artifact_sha256":"b".repeat(64),
        "artifact_readback_sha256":"c".repeat(64),
        "artifact_readback_object_sha256":"d".repeat(64),
        "geometry_candidate_evidence_sha256":"e".repeat(64),
        "geometry_program_sha256":"f".repeat(64),
        "geometry_program_object_sha256":"0".repeat(64),
        "operator_catalog_sha256":"1".repeat(64),
        "readback_config_sha256":"2".repeat(64),
        "part_inventory_sha256":"3".repeat(64),
        "part_ids":["receiver","barrel"],
        "part_topology_snapshot_sha256s":["4".repeat(64),"5".repeat(64)],
        "authoring_topology_status":"not-available",
        "part_authoring_topology_sha256s":[null,null],
        "topology_quality_policy":"candidate-topology-hard-gate@1",
        "topology_quality_policy_sha256":"6".repeat(64),
        "from_stage":"gray-model",
        "to_stage":"topology",
        "input_sha256":"7".repeat(64),
        "idempotency_key":"topology-quality-idem-1"
    });
    assert!(validate_call(
        "candidate_topology_quality_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call("candidate_topology_quality_prepare", &request, &bound()).is_ok());
    let mut other_scope = request.clone();
    other_scope["candidate_id"] = Value::String("candidate-other".to_owned());
    assert!(
        validate_call("candidate_topology_quality_prepare", &other_scope, &bound())
            .unwrap_err()
            .starts_with("AGENTIC_SCOPE_MISMATCH")
    );
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_topology_quality_prepare")
        .expect("candidate topology prepare tool");
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], false);
}

#[test]
fn candidate_topology_quality_get_is_restart_read_only_and_closed() {
    let request = json!({
        "schema_version":"CandidateTopologyQualityGetRequest@1",
        "topology_quality_id":"topology-quality-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "candidate_topology_quality_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let reads = read_tools();
    let tool = reads
        .iter()
        .find(|tool| tool["name"] == "candidate_topology_quality_get")
        .expect("candidate topology read tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "topology_quality_id",
            "project_id",
            "candidate_id"
        ])
    );
    let mut unknown = request.clone();
    unknown["unexpected"] = Value::Bool(true);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert!(validate_response(
        "candidate_topology_quality_get",
        &json!({
            "schema_version":"CandidateTopologyQualityGetResult@1",
            "topology_quality":{"project_id":"project-1","candidate_id":"candidate-1"},
            "runtime_write":false,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false
        }),
        &bound()
    )
    .is_ok());
}

#[test]
fn candidate_material_surface_quality_prepare_is_hidden_and_source_scope_bound() {
    let request = json!({
        "project_id":"project-1",
        "source_candidate_id":"candidate-1",
        "output_candidate_id":"candidate-appearance-1"
    });
    assert!(validate_call(
        "candidate_material_surface_quality_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "candidate_material_surface_quality_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let mut same_candidate = request.clone();
    same_candidate["output_candidate_id"] = Value::String("candidate-1".to_owned());
    assert!(validate_call(
        "candidate_material_surface_quality_prepare",
        &same_candidate,
        &bound()
    )
    .unwrap_err()
    .contains("must be distinct"));
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_material_surface_quality_prepare")
        .expect("material-surface prepare tool");
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
}

#[test]
fn candidate_material_surface_quality_get_is_restart_read_only_and_dual_bound() {
    let request = json!({
        "schema_version":"CandidateMaterialSurfaceQualityGetRequest@1",
        "material_surface_quality_id":"material-surface-quality-1",
        "project_id":"project-1",
        "source_candidate_id":"candidate-1",
        "output_candidate_id":"candidate-appearance-1"
    });
    assert!(validate_call(
        "candidate_material_surface_quality_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_material_surface_quality_get")
        .expect("material-surface get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert!(validate_response(
        "candidate_material_surface_quality_get",
        &json!({
            "schema_version":"CandidateMaterialSurfaceQualityGetResult@1",
            "material_surface_quality":{
                "project_id":"project-1",
                "source_candidate_id":"candidate-1",
                "output_candidate_id":"candidate-appearance-1"
            },
            "replayed":false,
            "runtime_write":false,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false
        }),
        &bound()
    )
    .is_ok());
}

#[test]
fn candidate_animation_vfx_quality_prepare_is_hidden_and_project_bound() {
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-appearance-1"
    });
    assert!(validate_call(
        "candidate_animation_vfx_quality_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "candidate_animation_vfx_quality_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_animation_vfx_quality_prepare")
        .expect("animation-vfx prepare tool");
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
}

#[test]
fn candidate_animation_vfx_quality_get_is_restart_read_only_and_truthful() {
    let request = json!({
        "schema_version":"CandidateAnimationVfxQualityGetRequest@1",
        "animation_vfx_quality_id":"animation-vfx-quality-1",
        "project_id":"project-1",
        "candidate_id":"candidate-appearance-1"
    });
    assert!(validate_call(
        "candidate_animation_vfx_quality_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_animation_vfx_quality_get")
        .expect("animation-vfx get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let hard_gate = json!({
        "material_surface_head_binding":true,
        "material_surface_quality":true,
        "delivery_lod0_binding":true,
        "anchor_set_binding":true,
        "animation_clip_binding":true,
        "animation_glb_readback":true,
        "animated_socket_readback":true,
        "vfx_profile_binding":true,
        "base_frame_stack":true,
        "bloom_stack":true,
        "particle_stack":true,
        "trail_stack":true,
        "trail_bloom_stack":true,
        "cross_layer_parent_binding":true,
        "sample_camera_binding":true,
        "worker_cohort_binding":true,
        "render_pass_byte_exact":true,
        "bounded_resource_policy":true,
        "vfx_glb_socket_attachment":false,
        "nonfunctional_scope":true
    });
    assert!(validate_response(
            "candidate_animation_vfx_quality_get",
            &json!({
                "schema_version":"CandidateAnimationVfxQualityGetResult@1",
                "animation_vfx_quality":{
                    "schema_version":"CandidateAnimationVfxQuality@1",
                    "project_id":"project-1",
                    "candidate_id":"candidate-appearance-1",
                    "candidate_binding_status":"same-material-surface-head-candidate-no-geometry-mutation",
                    "from_stage":"material-surface",
                    "to_stage":"animation-vfx",
                    "hard_gate":hard_gate,
                    "validator_status":"failed",
                    "hard_gate_passed":false,
                    "quality_status":"structural_only",
                    "visual_quality_status":"NOT_PROVEN",
                    "artistic_quality_status":"NOT_PROVEN",
                    "human_review_status":"NOT_RUN",
                    "commercial_fps_quality_status":"NOT_PROVEN",
                    "commercial_engine_status":"NOT_RUN",
                    "actual_engine_roundtrip":false,
                    "functional_semantics":false,
                    "runtime_write_performed":true
                },
                "replayed":false,
                "runtime_write":false,
                "production_stage_advanced":false,
                "candidate_confirmed":false,
                "version_created":false,
                "export_performed":false
            }),
            &Binding::default()
        )
        .is_ok());
}

#[test]
fn candidate_animation_vfx_quality_v2_prepare_is_closed_hidden_and_dual_candidate_bound() {
    let reads = read_tools();
    assert!(!reads
        .iter()
        .any(|tool| tool["name"] == "candidate_animation_vfx_quality_v2_prepare"));
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_animation_vfx_quality_v2_prepare")
        .expect("CandidateAnimationVfxQuality@2 prepare tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["required"]
            .as_array()
            .expect("closed request fields")
            .len(),
        69
    );
    assert!(prepare["inputSchema"]["properties"]
        .get("vfx_sequence_key_sha256")
        .is_none());
    assert!(prepare["inputSchema"]["properties"]
        .get("particle_history_key_sha256s")
        .is_none());
    assert!(prepare["inputSchema"]["properties"]
        .get("attachment_frame_set_sha256")
        .is_some());

    let request = json!({
        "schema_version":"CandidateAnimationVfxQualityPrepareRequest@2",
        "project_id":"project-1",
        "candidate_id":"appearance-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"appearance-1"
    });
    assert!(validate_call(
        "candidate_animation_vfx_quality_v2_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "candidate_animation_vfx_quality_v2_prepare",
        &request,
        &bound()
    )
    .is_ok());

    let mut retargeted = request.clone();
    retargeted["candidate_id"] = json!("candidate-1");
    assert!(validate_call(
        "candidate_animation_vfx_quality_v2_prepare",
        &retargeted,
        &bound()
    )
    .is_err());
    let mut collapsed = request.clone();
    collapsed["geometry_candidate_id"] = json!("appearance-1");
    assert!(validate_call(
        "candidate_animation_vfx_quality_v2_prepare",
        &collapsed,
        &bound()
    )
    .is_err());
    let mut unknown = prepare["inputSchema"].clone();
    unknown["properties"]["legacy_sidecar_bool"] = json!({"type":"boolean"});
    assert_eq!(unknown["additionalProperties"], false);
    assert!(unknown["required"]
        .as_array()
        .is_some_and(|fields| !fields.iter().any(|field| field == "legacy_sidecar_bool")));
}

#[test]
fn candidate_animation_vfx_quality_v2_get_is_restart_read_only_and_exactly_validated() {
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "candidate_animation_vfx_quality_v2_get")
        .expect("CandidateAnimationVfxQuality@2 get tool");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        get["inputSchema"]["required"]
            .as_array()
            .expect("closed get fields")
            .len(),
        4
    );
    let request = json!({
        "schema_version":"CandidateAnimationVfxQualityGetRequest@2",
        "animation_vfx_quality_id":"quality-1",
        "project_id":"project-1",
        "candidate_id":"appearance-1"
    });
    assert!(validate_call(
        "candidate_animation_vfx_quality_v2_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let response = candidate_animation_vfx_quality_v2_response(false);
    assert!(validate_response(
        "candidate_animation_vfx_quality_v2_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    assert!(validate_call("candidate_animation_vfx_quality_v2_get", &request, &bound()).is_ok());
    assert!(validate_response(
        "candidate_animation_vfx_quality_v2_get",
        &response,
        &bound()
    )
    .is_ok());
    let mut unknown = response.clone();
    unknown["animation_vfx_quality"]["vfx_sequence_key_sha256"] = Value::String("b".repeat(64));
    assert!(validate_response(
        "candidate_animation_vfx_quality_v2_get",
        &unknown,
        &Binding::default()
    )
    .is_err());
    let mut raw_media = response;
    raw_media["animation_vfx_quality"]["raw_glb_bytes"] = json!("forbidden");
    assert!(validate_response(
        "candidate_animation_vfx_quality_v2_get",
        &raw_media,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn candidate_animation_vfx_quality_v2_prepare_output_is_full15_and_all_twenty_gates() {
    let response = candidate_animation_vfx_quality_v2_response(true);
    assert!(validate_response(
        "candidate_animation_vfx_quality_v2_prepare",
        &response,
        &bound()
    )
    .is_ok());
    assert_eq!(
        response["animation_vfx_quality"]["hard_gate"]["vfx_glb_socket_attachment"],
        true
    );
    assert_eq!(
        response["animation_vfx_quality"]["attachment_frame_count"],
        15
    );
}

#[test]
fn animated_socket_attachment_prepare_is_hidden_and_candidate_bound() {
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_attachment_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_attachment_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_attachment_prepare")
        .expect("animated socket attachment prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert!(tool["inputSchema"]["required"]
        .as_array()
        .expect("attachment required fields")
        .iter()
        .all(|field| field != "approved" && field != "approval_receipt_id"));
}

#[test]
fn animated_socket_attachment_get_is_restart_read_only_and_rejects_raw_media() {
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1",
        "attachment_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_attachment_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_attachment_get")
        .expect("animated socket attachment get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);

    let frame = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentFrame@1",
        "attachment_key_sha256":"a".repeat(64),
        "frame_index":0,
        "sample_time_ticks":0,
        "animation_pose_readback_sha256":"b".repeat(64),
        "socket_transform_inventory_sha256":"c".repeat(64),
        "socket_transform_readback_sha256":"d".repeat(64),
        "emitter_socket_bindings_sha256":"e".repeat(64),
        "trail_socket_bindings_sha256":"f".repeat(64),
        "base_frame_key_sha256":"1".repeat(64),
        "bloom_key_sha256":"2".repeat(64),
        "particle_key_sha256":"3".repeat(64),
        "trail_key_sha256":"4".repeat(64),
        "trail_bloom_key_sha256":"5".repeat(64),
        "canonical_sha256":"6".repeat(64),
        "created_at":"2026-08-21T00:00:00Z"
    });
    let mut response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetResult@1",
        "attachment_key_sha256":"a".repeat(64),
        "attachment":{
            "schema_version":"FictionalEnergyVfxAnimatedSocketAttachment@1",
            "attachment_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "frames":[frame]
        },
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_attachment_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    response["attachment"]["png_base64"] = json!("not-allowed");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_attachment_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_glb_socket_transform_projection_prepare_is_hidden_and_candidate_bound() {
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "game_weapon_animated_glb_socket_transform_projection_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "game_weapon_animated_glb_socket_transform_projection_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "game_weapon_animated_glb_socket_transform_projection_prepare")
        .expect("animated GLB socket transform projection prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap().len(),
        40
    );
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert!(tool["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field != "approved" && field != "approval_receipt_id"));
    assert_eq!(
        runtime_method("game_weapon_animated_glb_socket_transform_projection_prepare"),
        Some("game_weapon_animated_glb_socket_transform_projection_prepare")
    );
}

#[test]
fn animated_glb_socket_transform_projection_get_is_restart_read_only_and_rejects_raw_media() {
    let request = json!({
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1",
        "projection_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "game_weapon_animated_glb_socket_transform_projection_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "game_weapon_animated_glb_socket_transform_projection_get")
        .expect("animated GLB socket transform projection get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "projection_key_sha256",
            "project_id",
            "candidate_id"
        ])
    );

    let six_socket_frame = json!({"socket_transforms":[{}, {}, {}, {}, {}, {}]});
    let mut response = json!({
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetResult@1",
        "projection_key_sha256":"a".repeat(64),
        "projection_object_sha256":"b".repeat(64),
        "projection":{
            "schema_version":"GameWeaponAnimatedGlbSocketTransformProjection@1",
            "projection_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "frames":[six_socket_frame],
            "projection_status":"runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection",
            "quality_status":"structural_only",
            "visual_quality_status":"NOT_PROVEN",
            "commercial_fps_quality_status":"NOT_PROVEN",
            "human_review_status":"NOT_RUN",
            "commercial_engine_status":"NOT_RUN",
            "runtime_write_performed":true,
            "restart_hash_verified":true,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "production_stage_advanced":false
        },
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    let validation = validate_response(
        "game_weapon_animated_glb_socket_transform_projection_get",
        &response,
        &Binding::default(),
    );
    assert!(validation.is_ok(), "{validation:?}");
    response["projection"]["glb_bytes"] = json!("not-allowed");
    assert!(validate_response(
        "game_weapon_animated_glb_socket_transform_projection_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_glb_socket_transform_projection_v2_prepare_is_hidden_closed_and_bound() {
    let request = json!({
        "project_id":"project-1",
        "appearance_candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| {
            tool["name"] == "game_weapon_animated_glb_socket_transform_projection_v2_prepare"
        })
        .expect("animated GLB socket transform projection V2 prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert!(tool["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "animation_clip_canonical_sha256"));
    assert_eq!(
        tool["inputSchema"]["properties"]["coordinate_system"]["const"],
        "forgecad-rh-y-up-m@1"
    );
    assert!(tool["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field != "approved" && field != "approval_receipt_id"));
    assert_eq!(
        runtime_method("game_weapon_animated_glb_socket_transform_projection_v2_prepare"),
        Some("game_weapon_animated_glb_socket_transform_projection_v2_prepare")
    );
}

#[test]
fn animated_glb_socket_transform_projection_v2_get_is_read_only_and_rejects_raw_media() {
    let request = json!({
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2",
        "projection_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "appearance_candidate_id":"candidate-appearance-1",
        "animation_clip_id":"clip-1"
    });
    assert!(validate_call(
        "game_weapon_animated_glb_socket_transform_projection_v2_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "game_weapon_animated_glb_socket_transform_projection_v2_get")
        .expect("animated GLB socket transform projection V2 get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "projection_key_sha256",
            "project_id",
            "appearance_candidate_id",
            "animation_clip_id"
        ])
    );
    let hash = |byte: char| byte.to_string().repeat(64);
    let pose = json!({
        "translation_m":[0.0,0.0,0.0],
        "rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
        "scale_xyz":[1.0,1.0,1.0]
    });
    let socket = json!({
        "socket_node_id":"socket-1",
        "anchor_id":"anchor-1",
        "role":"weapon-root",
        "node_index":0,
        "parent_node_index":-1,
        "node_name":"socket-node",
        "parent_node_name":null,
        "node_kind":"socket",
        "parent_kind":"root",
        "owner_part_id":null,
        "local_transform":pose,
        "parent_world_transform":pose,
        "composed_world_transform":pose,
        "local_matrix_4x4":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0],
        "parent_world_matrix_4x4":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0],
        "composed_world_matrix_4x4":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0]
    });
    let frame = json!({
        "schema_version":"GameWeaponAnimatedGlbSocketTransformProjectionFrame@2",
        "projection_key_sha256":hash('a'),
        "frame_index":0,
        "sample_time_ticks":0,
        "source_animation_sample_sha256":hash('b'),
        "derived_socket_sample_sha256":hash('c'),
        "socket_transform_inventory_sha256":hash('d'),
        "socket_transform_readback_sha256":hash('e'),
        "projection_frame_canonical_sha256":hash('f'),
        "socket_transforms":[socket.clone(), socket.clone(), socket.clone(), socket.clone(), socket.clone(), socket],
        "canonical_sha256":hash('0'),
        "created_at":"2026-08-22T00:00:00Z"
    });
    let mut projection = Map::new();
    for field in [
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "derived_animated_socket_receipt_object_sha256",
        "derived_animated_socket_receipt_canonical_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_node_inventory_sha256",
        "socket_roles_sha256",
        "part_hierarchy_sha256",
        "sampling_policy_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        projection.insert(field.to_owned(), json!(hash('3')));
    }
    projection.insert(
        "schema_version".to_owned(),
        json!("GameWeaponAnimatedGlbSocketTransformProjection@2"),
    );
    projection.insert("projection_key_sha256".to_owned(), json!(hash('a')));
    projection.insert("project_id".to_owned(), json!("project-1"));
    projection.insert("appearance_candidate_id".to_owned(), json!("candidate-1"));
    projection.insert("animation_clip_id".to_owned(), json!("clip-1"));
    projection.insert("frames".to_owned(), json!([frame]));
    projection.insert(
        "socket_roles".to_owned(),
        json!([
            "weapon-root",
            "grip-primary",
            "muzzle-vfx",
            "magazine-well",
            "sight-primary",
            "energy-core-vfx"
        ]),
    );
    projection.insert("sample_count".to_owned(), json!(1));
    projection.insert("sample_time_ticks".to_owned(), json!([0]));
    projection.insert(
        "frame_scope".to_owned(),
        json!("lod0-animation-frame-range-1-16@2"),
    );
    projection.insert("timebase_hz".to_owned(), json!(60));
    projection.insert(
        "transform_projection_policy".to_owned(),
        json!("glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"),
    );
    projection.insert(
        "coordinate_system".to_owned(),
        json!("forgecad-rh-y-up-m@1"),
    );
    projection.insert(
        "transform_convention".to_owned(),
        json!("column-vector-parent-world-times-trs-quaternion-xyzw@1"),
    );
    projection.insert(
        "float_quantization_policy".to_owned(),
        json!("f32-round-nearest-canonical-json@1"),
    );
    projection.insert(
        "projection_status".to_owned(),
        json!("runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection-v2"),
    );
    projection.insert("quality_status".to_owned(), json!("structural_only"));
    projection.insert("visual_quality_status".to_owned(), json!("NOT_PROVEN"));
    projection.insert(
        "commercial_fps_quality_status".to_owned(),
        json!("NOT_PROVEN"),
    );
    projection.insert("human_review_status".to_owned(), json!("NOT_RUN"));
    projection.insert("commercial_engine_status".to_owned(), json!("NOT_RUN"));
    projection.insert("runtime_write_performed".to_owned(), json!(true));
    projection.insert("restart_hash_verified".to_owned(), json!(true));
    projection.insert("candidate_confirmed".to_owned(), json!(false));
    projection.insert("version_created".to_owned(), json!(false));
    projection.insert("export_performed".to_owned(), json!(false));
    projection.insert("actual_engine_roundtrip".to_owned(), json!(false));
    projection.insert("production_stage_advanced".to_owned(), json!(false));
    projection.insert("canonical_sha256".to_owned(), json!(hash('c')));
    projection.insert("created_at".to_owned(), json!("2026-08-22T00:00:00Z"));
    projection.insert("limitations".to_owned(), json!([]));
    let mut response = Map::new();
    response.insert(
        "schema_version".to_owned(),
        json!("GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2"),
    );
    response.insert("projection_key_sha256".to_owned(), json!(hash('a')));
    response.insert("projection_object_sha256".to_owned(), json!(hash('1')));
    response.insert("projection".to_owned(), Value::Object(projection));
    response.insert("replayed".to_owned(), json!(false));
    response.insert("restart_hash_verified".to_owned(), json!(true));
    response.insert("runtime_write_performed".to_owned(), json!(false));
    response.insert("quality_status".to_owned(), json!("structural_only"));
    response.insert("visual_quality_status".to_owned(), json!("NOT_PROVEN"));
    response.insert(
        "commercial_fps_quality_status".to_owned(),
        json!("NOT_PROVEN"),
    );
    response.insert("human_review_status".to_owned(), json!("NOT_RUN"));
    response.insert("commercial_engine_status".to_owned(), json!("NOT_RUN"));
    response.insert("actual_engine_roundtrip".to_owned(), json!(false));
    response.insert("production_stage_advanced".to_owned(), json!(false));
    response.insert("candidate_confirmed".to_owned(), json!(false));
    response.insert("version_created".to_owned(), json!(false));
    response.insert("export_performed".to_owned(), json!(false));
    let mut response = Value::Object(response);
    assert!(validate_response(
        "game_weapon_animated_glb_socket_transform_projection_v2_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    response["projection"]["glb_bytes"] = json!("not-allowed");
    assert!(validate_response(
        "game_weapon_animated_glb_socket_transform_projection_v2_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_socket_particles_sequence_prepare_is_hidden_and_candidate_bound() {
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| {
            tool["name"] == "fictional_energy_vfx_animated_socket_particles_sequence_prepare"
        })
        .expect("animated socket particles sequence prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap().len(),
        37
    );
    assert!(tool["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field != "approved" && field != "approval_receipt_id"));
    assert_eq!(
        runtime_method("fictional_energy_vfx_animated_socket_particles_sequence_prepare"),
        Some("fictional_energy_vfx_animated_socket_particles_sequence_prepare")
    );
}

#[test]
fn animated_socket_particles_sequence_get_is_restart_read_only_and_rejects_raw_media() {
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1",
        "sequence_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_particles_sequence_get")
        .expect("animated socket particles sequence get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id"
        ])
    );

    let mut response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@1",
        "sequence_key_sha256":"a".repeat(64),
        "sequence":{
            "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequence@1",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "frames":[{
                "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@1",
                "frame_index":0,
                "sample_time_ticks":0,
                "projection_frame_canonical_sha256":"0".repeat(64),
                "projection_socket_transform_inventory_sha256":"1".repeat(64),
                "projection_socket_transform_readback_sha256":"2".repeat(64),
                "base_frame_key_sha256":"3".repeat(64),
                "bloom_key_sha256":"4".repeat(64),
                "emitter_socket_bindings_sha256":"5".repeat(64),
                "input_sha256":"6".repeat(64),
                "particle_key_sha256":"7".repeat(64),
                "particle_seed_sha256":"8".repeat(64),
                "render_set_object_sha256":"9".repeat(64),
                "receipt_object_sha256":"a".repeat(64),
                "particle_color_object_sha256":"b".repeat(64),
                "particle_id_object_sha256":"c".repeat(64),
                "particle_depth_object_sha256":"d".repeat(64),
                "canonical_sha256":"e".repeat(64),
                "created_at":"2026-08-22T00:00:00Z"
            }],
            "geometry_preservation_projection_sha256":"f".repeat(64),
            "geometry_preservation_status":"source-output-renderable-geometry-byte-exact",
            "sequence_status":"runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence",
            "frame_scope":"lod0-animation-particles-frame-range-1-16@1",
            "particles_sequence_policy":"projection-driven-animated-socket-particles@1",
            "emitter_binding_policy":"projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1",
            "transform_projection_policy":"glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1",
            "quality_status":"structural_only",
            "visual_quality_status":"NOT_PROVEN",
            "commercial_fps_quality_status":"NOT_PROVEN",
            "human_review_status":"NOT_RUN",
            "commercial_engine_status":"NOT_RUN",
            "runtime_write_performed":true,
            "restart_hash_verified":true,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "production_stage_advanced":false
        },
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    let validation = validate_response(
        "fictional_energy_vfx_animated_socket_particles_sequence_get",
        &response,
        &Binding::default(),
    );
    assert!(validation.is_ok(), "{validation:?}");
    response["sequence"]["png_base64"] = json!("not-allowed");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_particles_sequence_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_socket_particles_sequence_v2_prepare_is_hidden_dual_bound_and_closed() {
    let request = json!({
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"candidate-appearance-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let mut same_candidate = request.clone();
    same_candidate["appearance_candidate_id"] = json!("candidate-1");
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare",
        &same_candidate,
        &bound()
    )
    .unwrap_err()
    .contains("must be distinct"));

    let tool = write_tools()
        .into_iter()
        .find(|tool| {
            tool["name"] == "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"
        })
        .expect("V2 animated socket particles sequence prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let required = tool["inputSchema"]["required"]
        .as_array()
        .expect("V2 required fields");
    let properties = tool["inputSchema"]["properties"]
        .as_object()
        .expect("V2 properties");
    assert_eq!(required.len(), 47);
    assert_eq!(required.len(), properties.len());
    let mut required_names = required
        .iter()
        .map(|field| field.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let mut property_names = properties.keys().cloned().collect::<Vec<_>>();
    required_names.sort();
    property_names.sort();
    assert_eq!(required_names, property_names);
    let mut unique_required = required_names.clone();
    unique_required.dedup();
    assert_eq!(unique_required.len(), required_names.len());
    assert_eq!(
        tool["inputSchema"]["properties"]["sample_count"]["maximum"],
        16
    );
    assert_eq!(tool["inputSchema"]["properties"]["frames"]["maxItems"], 16);
    assert_eq!(
        tool["inputSchema"]["properties"]["frames"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        runtime_method("fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"),
        Some("fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare")
    );
}

#[test]
fn animated_socket_particles_sequence_v2_get_is_read_only_and_rejects_raw_media() {
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2",
        "sequence_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-geometry-1",
        "appearance_candidate_id":"candidate-appearance-1",
        "geometry_delivery_manifest_object_sha256":"b".repeat(64),
        "appearance_delivery_manifest_object_sha256":"c".repeat(64)
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let mut forbidden = request.clone();
    forbidden["path"] = json!("/tmp/not-allowed");
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
        &forbidden,
        &Binding::default()
    )
    .is_err());
    let tool = read_tools()
        .into_iter()
        .find(|tool| {
            tool["name"] == "fictional_energy_vfx_animated_socket_particles_sequence_v2_get"
        })
        .expect("V2 animated socket particles sequence get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"],
        json!([
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "appearance_candidate_id",
            "geometry_delivery_manifest_object_sha256",
            "appearance_delivery_manifest_object_sha256"
        ])
    );
    assert_eq!(
        runtime_method("fictional_energy_vfx_animated_socket_particles_sequence_v2_get"),
        Some("fictional_energy_vfx_animated_socket_particles_sequence_v2_get")
    );
    let mut response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@2",
        "sequence_key_sha256":"a".repeat(64),
        "sequence":{
            "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequence@2",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "geometry_candidate_id":"candidate-geometry-1",
            "appearance_candidate_id":"candidate-appearance-1",
            "geometry_delivery_manifest_object_sha256":"b".repeat(64),
            "appearance_delivery_manifest_object_sha256":"c".repeat(64),
            "geometry_preservation_projection_sha256":"d".repeat(64),
            "geometry_preservation_status":"source-output-renderable-geometry-byte-exact",
            "anchor_binding_policy":"geometry-appearance-anchor-role-owner-trs-equivalent@1",
            "anchor_binding_sha256":"e".repeat(64),
            "frames":[{
                "schema_version":"FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@2",
                "frame_index":0,
                "sample_time_ticks":0,
                "projection_frame_canonical_sha256":"f".repeat(64),
                "projection_socket_transform_inventory_sha256":"0".repeat(64),
                "projection_socket_transform_readback_sha256":"1".repeat(64),
                "base_frame_key_sha256":"2".repeat(64),
                "bloom_key_sha256":"3".repeat(64),
                "emitter_socket_bindings_sha256":"4".repeat(64),
                "input_sha256":"5".repeat(64),
                "particle_key_sha256":"6".repeat(64),
                "particle_seed_sha256":"7".repeat(64),
                "render_set_object_sha256":"8".repeat(64),
                "receipt_object_sha256":"9".repeat(64),
                "particle_color_object_sha256":"a".repeat(64),
                "particle_id_object_sha256":"b".repeat(64),
                "particle_depth_object_sha256":"c".repeat(64),
                "canonical_sha256":"d".repeat(64),
                "created_at":"2026-08-22T00:00:00Z"
            }],
            "frame_scope":"lod0-animation-particles-frame-range-1-16@2",
            "particles_sequence_policy":"projection-v2-driven-animated-socket-particles-dual-candidate@2",
            "emitter_binding_policy":"projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1",
            "transform_projection_policy":"glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2",
            "sequence_status":"runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence-v2",
            "quality_status":"structural_only",
            "visual_quality_status":"NOT_PROVEN",
            "commercial_fps_quality_status":"NOT_PROVEN",
            "human_review_status":"NOT_RUN",
            "commercial_engine_status":"NOT_RUN",
            "runtime_write_performed":true,
            "restart_hash_verified":true,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "production_stage_advanced":false,
            "input_sha256":"e".repeat(64),
            "canonical_sha256":"f".repeat(64),
            "created_at":"2026-08-22T00:00:00Z"
        },
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    for field in [
        "geometry_candidate_state_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
    ] {
        response["sequence"][field] = json!("e".repeat(64));
    }
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    let mut downgraded_policy = response.clone();
    downgraded_policy["sequence"]["particles_sequence_policy"] =
        json!("projection-driven-animated-socket-particles-dual-candidate@1");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
        &downgraded_policy,
        &Binding::default()
    )
    .is_err());
    let mut projection_unbound = response.clone();
    projection_unbound["sequence"]
        .as_object_mut()
        .expect("V2 sequence object")
        .remove("projection_key_sha256");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
        &projection_unbound,
        &Binding::default()
    )
    .is_err());
    response["sequence"]["png_base64"] = json!("not-allowed");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_socket_attachment_v2_surface_is_hidden_closed_and_projection_bound() {
    let prepare_name = "fictional_energy_vfx_animated_socket_attachment_v2_prepare";
    let get_name = "fictional_energy_vfx_animated_socket_attachment_v2_get";
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(prepare_name, &request, &Binding::default()).is_err());
    assert!(validate_call(prepare_name, &request, &bound()).is_ok());
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == prepare_name)
        .expect("V2 animated socket attachment prepare tool");
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == get_name)
        .expect("V2 animated socket attachment get tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        runtime_method(get_name),
        Some("fictional_energy_vfx_animated_socket_attachment_v2_get")
    );

    let hash = |character: char| character.to_string().repeat(64);
    let mut frame = Map::new();
    frame.insert(
        "schema_version".to_owned(),
        json!("FictionalEnergyVfxAnimatedSocketAttachmentFrame@2"),
    );
    frame.insert("attachment_key_sha256".to_owned(), json!(hash('a')));
    frame.insert("frame_index".to_owned(), json!(0));
    frame.insert("projection_frame_index".to_owned(), json!(1));
    frame.insert("particle_sequence_frame_index".to_owned(), json!(1));
    frame.insert("sample_time_ticks".to_owned(), json!(0));
    for (index, field) in [
        "animation_pose_readback_sha256",
        "socket_transform_inventory_sha256",
        "socket_transform_readback_sha256",
        "emitter_socket_bindings_sha256",
        "trail_socket_bindings_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "particle_key_sha256",
        "trail_key_sha256",
        "trail_bloom_key_sha256",
        "projection_frame_canonical_sha256",
        "particle_sequence_frame_canonical_sha256",
        "trail_sequence_frame_canonical_sha256",
        "trail_bloom_sequence_frame_canonical_sha256",
        "canonical_sha256",
    ]
    .into_iter()
    .enumerate()
    {
        frame.insert(
            field.to_owned(),
            json!(hash("0123456789abcdef".chars().nth(index % 16).unwrap())),
        );
    }
    frame.insert("created_at".to_owned(), json!("2026-08-22T00:00:00Z"));

    let mut attachment = Map::new();
    attachment.insert(
        "schema_version".to_owned(),
        json!("FictionalEnergyVfxAnimatedSocketAttachment@2"),
    );
    attachment.insert("attachment_key_sha256".to_owned(), json!(hash('a')));
    attachment.insert("project_id".to_owned(), json!("project-1"));
    attachment.insert("candidate_id".to_owned(), json!("candidate-1"));
    attachment.insert("animation_clip_id".to_owned(), json!("clip-1"));
    for (index, field) in [
        "delivery_manifest_object_sha256",
        "candidate_state_sha256",
        "source_artifact_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animated_artifact_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "canonical_sha256",
    ]
    .into_iter()
    .enumerate()
    {
        attachment.insert(
            field.to_owned(),
            json!(hash("abcdef0123456789".chars().nth(index % 16).unwrap())),
        );
    }
    attachment.insert(
        "attachment_policy".to_owned(),
        json!("fictional-energy-vfx-animated-socket-attachment-projection-bound@2"),
    );
    attachment.insert(
        "frame_scope".to_owned(),
        json!("lod0-animation-vfx-trail-frame-range-1-15@2"),
    );
    attachment.insert(
        "frames".to_owned(),
        Value::Array(vec![Value::Object(frame)]),
    );
    attachment.insert(
        "attachment_status".to_owned(),
        json!("runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v2"),
    );
    attachment.insert("created_at".to_owned(), json!("2026-08-22T00:00:00Z"));
    let response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetResult@2",
        "attachment_key_sha256":hash('a'),
        "attachment":Value::Object(attachment),
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(get_name, &response, &bound()).is_ok());
    let mut tampered = response.clone();
    tampered["attachment"]["raw_glb_bytes"] = json!("not-allowed");
    assert!(validate_response(get_name, &tampered, &bound()).is_err());
}

#[test]
fn animated_socket_trails_sequence_prepare_is_hidden_and_bounded_to_fifteen_frames() {
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_sequence_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_sequence_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_trails_sequence_prepare")
        .expect("animated socket trails sequence prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap().len(),
        39
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["sample_count"]["maximum"],
        15
    );
    assert_eq!(tool["inputSchema"]["properties"]["frames"]["maxItems"], 15);
    assert_eq!(
        tool["inputSchema"]["properties"]["frames"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        runtime_method("fictional_energy_vfx_animated_socket_trails_sequence_prepare"),
        Some("fictional_energy_vfx_animated_socket_trails_sequence_prepare")
    );
}

#[test]
fn animated_socket_trails_sequence_get_is_read_only_and_rejects_transport_payloads() {
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1",
        "sequence_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_sequence_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let mut forbidden = request.clone();
    forbidden["path"] = json!("/tmp/not-allowed");
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_sequence_get",
        &forbidden,
        &Binding::default()
    )
    .is_err());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == "fictional_energy_vfx_animated_socket_trails_sequence_get")
        .expect("animated socket trails sequence get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);

    let hash = "b".repeat(64);
    let mut response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@1",
        "sequence_key_sha256":"a".repeat(64),
        "sequence":{
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequence@1",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "frames":[{
                "frame_index":0,
                "trail_key_sha256":hash,
                "trail_seed_sha256":"c".repeat(64),
                "trail_inventory_sha256":"d".repeat(64),
                "trail_id_encoding_sha256":"e".repeat(64),
                "emitter_binding_sha256":"f".repeat(64),
                "trail_color_object_sha256":"0".repeat(64),
                "trail_id_object_sha256":"1".repeat(64),
                "trail_depth_object_sha256":"2".repeat(64),
                "render_set_object_sha256":"3".repeat(64),
                "receipt_object_sha256":"4".repeat(64)
            }],
            "sequence_status":"runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-sequence",
            "frame_scope":"lod0-animation-trails-source-frames-1-15@1",
            "trails_sequence_policy":"projection-driven-animated-socket-trails@1",
            "history_policy":"one-to-eight-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@1",
            "history_pre_roll_policy":"same-parent-source-frame-zero-is-preroll-output-frames-one-to-fifteen@1",
            "trail_count":2,
            "trail_emitter_roles":["muzzle-vfx","energy-core-vfx"],
            "quality_status":"structural_only",
            "visual_quality_status":"NOT_PROVEN",
            "commercial_fps_quality_status":"NOT_PROVEN",
            "human_review_status":"NOT_RUN",
            "commercial_engine_status":"NOT_RUN",
            "runtime_write_performed":true,
            "restart_hash_verified":true,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "production_stage_advanced":false
        },
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_trails_sequence_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    response["sequence"]["url"] = json!("https://not-allowed");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_trails_sequence_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_socket_trails_sequence_v2_prepare_is_hidden_dual_bound_and_closed() {
    let prepare_name = "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare";
    let request = json!({
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"candidate-2"
    });
    assert!(validate_call(prepare_name, &request, &Binding::default()).is_err());
    assert!(validate_call(prepare_name, &request, &bound()).is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == prepare_name)
        .expect("Trails@2 prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let required = tool["inputSchema"]["required"]
        .as_array()
        .expect("Trails@2 required fields");
    let properties = tool["inputSchema"]["properties"]
        .as_object()
        .expect("Trails@2 properties");
    assert_eq!(required.len(), 51);
    assert_eq!(required.len(), properties.len());
    assert_eq!(
        tool["inputSchema"]["properties"]["frame_scope"]["const"],
        "lod0-animation-trails-v2-source-frames-1-15-with-particles-v2-frame-zero-preroll@2"
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["history_pre_roll_policy"]["const"],
        "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"
    );
    assert_eq!(runtime_method(prepare_name), Some(prepare_name));
    let mut same_candidate = request.clone();
    same_candidate["appearance_candidate_id"] = json!("candidate-1");
    assert!(validate_call(prepare_name, &same_candidate, &bound())
        .unwrap_err()
        .contains("must be distinct"));
}

#[test]
fn animated_socket_trails_sequence_v2_get_is_read_only_and_rejects_raw_media() {
    let get_name = "fictional_energy_vfx_animated_socket_trails_sequence_v2_get";
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2",
        "sequence_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"candidate-2",
        "geometry_delivery_manifest_object_sha256":"b".repeat(64),
        "appearance_delivery_manifest_object_sha256":"c".repeat(64)
    });
    assert!(validate_call(get_name, &request, &Binding::default()).is_ok());
    let mut forbidden = request.clone();
    forbidden["png_base64"] = json!("not-allowed");
    assert!(validate_call(get_name, &forbidden, &Binding::default()).is_err());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == get_name)
        .expect("Trails@2 get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(tool["inputSchema"]["required"].as_array().unwrap().len(), 7);
    assert_eq!(runtime_method(get_name), Some(get_name));
    let mut same_candidate = request.clone();
    same_candidate["appearance_candidate_id"] = json!("candidate-1");
    assert!(
        validate_call(get_name, &same_candidate, &Binding::default())
            .unwrap_err()
            .contains("must be distinct")
    );
}

#[test]
fn animated_socket_trails_bloom_sequence_prepare_is_hidden_and_bounded_to_fifteen_frames() {
    let request = json!({
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare",
        &request,
        &Binding::default()
    )
    .is_err());
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare",
        &request,
        &bound()
    )
    .is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| {
            tool["name"] == "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"
        })
        .expect("animated socket trails Bloom sequence prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["_meta"]["forgecad"]["requiresConfirmation"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let mut required = tool["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let mut properties = tool["inputSchema"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    required.sort();
    properties.sort();
    assert_eq!(required.len(), 42);
    let mut required_unique = required.clone();
    required_unique.dedup();
    assert_eq!(required_unique.len(), required.len());
    assert_eq!(required.len(), properties.len());
    assert_eq!(required, properties);
    assert_eq!(
        tool["inputSchema"]["properties"]["sample_count"]["maximum"],
        15
    );
    assert_eq!(tool["inputSchema"]["properties"]["frames"]["maxItems"], 15);
    assert_eq!(
        tool["inputSchema"]["properties"]["trail_bloom_profile"]["additionalProperties"],
        false
    );
    assert_eq!(
        runtime_method("fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"),
        Some("fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare")
    );
}

#[test]
fn animated_socket_trails_bloom_sequence_get_is_read_only_and_rejects_transport_payloads() {
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@1",
        "sequence_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "candidate_id":"candidate-1"
    });
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get",
        &request,
        &Binding::default()
    )
    .is_ok());
    let mut forbidden = request.clone();
    forbidden["uri"] = json!("file:///tmp/not-allowed");
    assert!(validate_call(
        "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get",
        &forbidden,
        &Binding::default()
    )
    .is_err());
    let tool = read_tools()
        .into_iter()
        .find(|tool| {
            tool["name"] == "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get"
        })
        .expect("animated socket trails Bloom sequence get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);

    let mut frame = json!({
        "frame_index":0,
        "trail_sequence_key_sha256":"b".repeat(64),
        "trail_sequence_canonical_sha256":"c".repeat(64),
        "trail_frame_canonical_sha256":"d".repeat(64),
        "trail_color_object_sha256":"e".repeat(64),
        "trail_id_object_sha256":"f".repeat(64),
        "trail_depth_object_sha256":"0".repeat(64),
        "particle_sequence_frame_canonical_sha256":"1".repeat(64),
        "base_frame_key_sha256":"2".repeat(64),
        "bloom_key_sha256":"3".repeat(64),
        "camera_object_sha256":"4".repeat(64),
        "camera_identity_sha256":"5".repeat(64),
        "render_profile_sha256":"6".repeat(64),
        "render_worker_build_cohort_sha256":"7".repeat(64),
        "trail_bloom_profile_sha256":"8".repeat(64),
        "base_opaque_depth_object_sha256":"9".repeat(64),
        "base_aov_byte_exact_verified":true,
        "base_opaque_depth_byte_exact_reused":true,
        "bloom_pass_byte_exact_reused":true,
        "particle_passes_byte_exact_reused":true,
        "trail_passes_byte_exact_reused":true,
        "base_bloom_mutated":false,
        "particle_passes_mutated":false,
        "trail_passes_mutated":false,
        "trail_bloom_input":true,
        "trail_emissive_source_rendered":true,
        "trail_bloom_contribution_rendered":true,
        "trail_bloom_rendered":true,
        "trail_bloom_key_sha256":"a".repeat(64),
        "trail_bloom_seed_sha256":"b".repeat(64),
        "trail_emissive_source_object_sha256":"c".repeat(64),
        "trail_bloom_contribution_object_sha256":"d".repeat(64),
        "render_set_object_sha256":"e".repeat(64),
        "receipt_object_sha256":"f".repeat(64)
    });
    frame["sample_time_ticks"] = json!(0);
    let mut response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@1",
        "sequence_key_sha256":"a".repeat(64),
        "sequence":{
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@1",
            "sequence_key_sha256":"a".repeat(64),
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "frames":[frame],
            "sequence_status":"runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom-sequence",
            "frame_scope":"lod0-animation-trails-bloom-source-frames-1-15@1",
            "trails_bloom_sequence_policy":"projection-driven-animated-socket-trails-bloom@1",
            "trail_key_scope":"animated-socket-trails-sequence-frame-binding@1",
            "trail_count":2,
            "trail_emitter_roles":["muzzle-vfx","energy-core-vfx"],
            "trail_bloom_profile_sha256":"8".repeat(64),
            "trail_bloom_profile":{
                "threshold":1,
                "source_gain":8,
                "radius_px":8,
                "intensity":4,
                "hdr_clamp":16,
                "blur_passes":2,
                "kernel":"separable-box-two-pass-fixed-radius@1"
            },
            "quality_status":"structural_only",
            "visual_quality_status":"NOT_PROVEN",
            "commercial_fps_quality_status":"NOT_PROVEN",
            "human_review_status":"NOT_RUN",
            "commercial_engine_status":"NOT_RUN",
            "runtime_write_performed":true,
            "restart_hash_verified":true,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "production_stage_advanced":false
        },
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get",
        &response,
        &Binding::default()
    )
    .is_ok());
    response["sequence"]["png_base64"] = json!("not-allowed");
    assert!(validate_response(
        "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get",
        &response,
        &Binding::default()
    )
    .is_err());
}

#[test]
fn animated_socket_trails_bloom_sequence_v2_prepare_is_hidden_dual_bound_and_closed() {
    let name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare";
    let request = json!({
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"candidate-2"
    });
    assert!(validate_call(name, &request, &Binding::default()).is_err());
    assert!(validate_call(name, &request, &bound()).is_ok());
    let tool = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == name)
        .expect("TrailsBloom@2 prepare tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["writeIntent"], true);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap().len(),
        56
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["frame_scope"]["const"],
        "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2"
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["trails_bloom_sequence_policy"]["const"],
        "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2"
    );
    assert_eq!(
        runtime_method(name),
        Some("fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare")
    );
    let mut same_candidate = request;
    same_candidate["appearance_candidate_id"] = json!("candidate-1");
    assert!(validate_call(name, &same_candidate, &bound())
        .unwrap_err()
        .contains("must be distinct"));
}

#[test]
fn animated_socket_trails_bloom_sequence_v2_get_is_read_only_and_rejects_transport_payloads() {
    let name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get";
    let request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2",
        "sequence_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"candidate-2",
        "geometry_delivery_manifest_object_sha256":"b".repeat(64),
        "appearance_delivery_manifest_object_sha256":"c".repeat(64)
    });
    assert!(validate_call(name, &request, &Binding::default()).is_ok());
    let mut forbidden = request.clone();
    forbidden["raw_glb_bytes"] = json!("not-allowed");
    assert!(validate_call(name, &forbidden, &Binding::default()).is_err());
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == name)
        .expect("TrailsBloom@2 get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(tool["inputSchema"]["required"].as_array().unwrap().len(), 7);
    let mut same_candidate = request;
    same_candidate["appearance_candidate_id"] = json!("candidate-1");
    assert!(validate_call(name, &same_candidate, &Binding::default())
        .unwrap_err()
        .contains("must be distinct"));
}

#[test]
fn animated_socket_trails_bloom_sequence_v2_response_is_structural_and_media_closed() {
    let name = "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get";
    let hash = "a".repeat(64);
    let mut sequence = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@2",
        "sequence_key_sha256":hash,
        "project_id":"project-1",
        "geometry_candidate_id":"candidate-1",
        "appearance_candidate_id":"candidate-2",
        "frame_scope":"lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2",
        "trails_bloom_sequence_policy":"projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2",
        "history_policy":"particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2",
        "history_pre_roll_policy":"same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2",
        "trail_key_scope":"animated-socket-trails-sequence-v2-frame-binding@2",
        "trail_count":2,
        "trail_emitter_roles":["muzzle-vfx","energy-core-vfx"],
        "trail_bloom_profile":{"threshold":1,"source_gain":8,"radius_px":8,"intensity":4,"hdr_clamp":16,"blur_passes":2,"kernel":"separable-box-two-pass-fixed-radius@1"},
        "sequence_status":"runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom-sequence-v2",
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "runtime_write_performed":true,
        "restart_hash_verified":true,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "frames":[]
    });
    for field in [
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_profile_sha256",
        "input_sha256",
        "canonical_sha256",
    ] {
        sequence[field] = json!("a".repeat(64));
    }
    for index in 0..15_u64 {
        let mut frame = json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@2",
            "frame_index":index,
            "trail_frame_index":index,
            "current_projection_frame_index":index+1,
            "current_particle_frame_index":index+1,
            "base_aov_byte_exact_verified":true,
            "base_opaque_depth_byte_exact_reused":true,
            "bloom_pass_byte_exact_reused":true,
            "particle_passes_byte_exact_reused":true,
            "trail_passes_byte_exact_reused":true,
            "base_bloom_mutated":false,
            "particle_passes_mutated":false,
            "trail_passes_mutated":false,
            "trail_bloom_input":true,
            "trail_emissive_source_rendered":true,
            "trail_bloom_contribution_rendered":true,
            "trail_bloom_rendered":true,
            "trail_bloom_contributions":[{},{}]
        });
        for field in [
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_frame_canonical_sha256",
            "trail_key_sha256",
            "trail_inventory_sha256",
            "trail_id_encoding_sha256",
            "emitter_binding_sha256",
            "trail_color_object_sha256",
            "trail_id_object_sha256",
            "trail_depth_object_sha256",
            "particle_sequence_key_sha256",
            "particle_sequence_frame_canonical_sha256",
            "current_projection_frame_canonical_sha256",
            "current_projection_socket_transform_inventory_sha256",
            "current_projection_socket_transform_readback_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "trail_bloom_profile_sha256",
            "base_opaque_depth_object_sha256",
            "trail_bloom_key_sha256",
            "trail_bloom_seed_sha256",
            "trail_emissive_source_object_sha256",
            "trail_bloom_contribution_object_sha256",
            "render_set_object_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
        ] {
            frame[field] = json!("a".repeat(64));
        }
        sequence["frames"].as_array_mut().unwrap().push(frame);
    }
    let response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@2",
        "sequence_key_sha256":"a".repeat(64),
        "sequence":sequence,
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(name, &response, &Binding::default()).is_ok());
    let mut tampered = response;
    tampered["sequence"]["frames"][0]["png_base64"] = json!("not-allowed");
    assert!(validate_response(name, &tampered, &Binding::default()).is_err());
}

#[test]
fn animated_socket_attachment_v3_surface_is_hidden_dual_bound_and_read_only() {
    let prepare_name = "fictional_energy_vfx_animated_socket_attachment_v3_prepare";
    let get_name = "fictional_energy_vfx_animated_socket_attachment_v3_get";
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == prepare_name)
        .expect("Attachment@3 prepare tool");
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == get_name)
        .expect("Attachment@3 get tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["properties"]["attachment_policy"]["const"],
        "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"
    );
    assert_eq!(
        prepare["inputSchema"]["properties"]["frame_scope"]["const"],
        "lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"
    );
    for field in [
        "geometry_candidate_id",
        "appearance_candidate_id",
        "geometry_delivery_manifest_object_sha256",
        "appearance_delivery_manifest_object_sha256",
        "sample_count",
        "sample_time_ticks",
        "idempotency_key",
    ] {
        assert!(
            prepare["inputSchema"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Attachment@3 prepare missing {field}"
        );
    }
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        get["inputSchema"]["properties"]["schema_version"]["const"],
        "FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3"
    );
    assert_eq!(
        runtime_method(prepare_name),
        Some("fictional_energy_vfx_animated_socket_attachment_v3_prepare")
    );
    assert_eq!(
        runtime_method(get_name),
        Some("fictional_energy_vfx_animated_socket_attachment_v3_get")
    );

    let get_request = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3",
        "attachment_key_sha256":"a".repeat(64),
        "project_id":"project-attachment-v3",
        "geometry_candidate_id":"geometry-v3",
        "appearance_candidate_id":"appearance-v3",
        "geometry_delivery_manifest_object_sha256":"b".repeat(64),
        "appearance_delivery_manifest_object_sha256":"c".repeat(64)
    });
    assert!(validate_call(get_name, &get_request, &Binding::default()).is_ok());
    let mut same_candidate = get_request.clone();
    same_candidate["appearance_candidate_id"] = json!("geometry-v3");
    assert!(
        validate_call(get_name, &same_candidate, &Binding::default())
            .unwrap_err()
            .contains("must be distinct")
    );
    let prepare_request = json!({
        "project_id":"project-attachment-v3",
        "geometry_candidate_id":"geometry-v3",
        "appearance_candidate_id":"appearance-v3"
    });
    assert!(validate_call(prepare_name, &prepare_request, &bound())
        .unwrap_err()
        .contains("must remain inside"));
}

#[test]
fn animated_socket_attachment_v3_response_requires_exact_fifteen_hash_only_frames() {
    let name = "fictional_energy_vfx_animated_socket_attachment_v3_get";
    let ticks: Vec<Value> = (1_u64..=15).map(Value::from).collect();
    let hash_value = || Value::String("a".repeat(64));
    let mut attachment = Map::new();
    for field in [
        "attachment_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "attachment_receipt_object_sha256",
        "attachment_receipt_canonical_sha256",
        "input_sha256",
        "canonical_sha256",
    ] {
        attachment.insert(field.to_owned(), hash_value());
    }
    for (field, value) in [
            (
                "schema_version",
                json!("FictionalEnergyVfxAnimatedSocketAttachment@3"),
            ),
            ("project_id", json!("project-attachment-v3")),
            ("geometry_candidate_id", json!("geometry-v3")),
            ("appearance_candidate_id", json!("appearance-v3")),
            ("material_surface_quality_id", json!("quality-v3")),
            (
                "geometry_preservation_status",
                json!("source-output-renderable-geometry-byte-exact"),
            ),
            (
                "anchor_binding_policy",
                json!("geometry-appearance-anchor-role-owner-trs-equivalent@1"),
            ),
            ("animation_clip_id", json!("clip-v3")),
            ("sample_count", json!(15)),
            ("sample_time_ticks", Value::Array(ticks)),
            (
                "attachment_policy",
                json!("projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"),
            ),
            (
                "frame_scope",
                json!("lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"),
            ),
            (
                "attachment_status",
                json!("runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v3"),
            ),
            ("quality_status", json!("structural_only")),
            ("visual_quality_status", json!("NOT_PROVEN")),
            ("commercial_fps_quality_status", json!("NOT_PROVEN")),
            ("human_review_status", json!("NOT_RUN")),
            ("commercial_engine_status", json!("NOT_RUN")),
            ("runtime_write_performed", json!(true)),
            ("restart_hash_verified", json!(true)),
            ("candidate_confirmed", json!(false)),
            ("version_created", json!(false)),
            ("export_performed", json!(false)),
            ("actual_engine_roundtrip", json!(false)),
            ("production_stage_advanced", json!(false)),
            ("created_at", json!("2026-08-22T00:00:00Z")),
            ("frames", json!([])),
        ] {
            attachment.insert(field.to_owned(), value);
        }
    let mut attachment = Value::Object(attachment);
    for index in 0_u64..15 {
        attachment["frames"].as_array_mut().unwrap().push(json!({
            "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentFrame@3",
            "attachment_key_sha256":"a".repeat(64),
            "frame_index":index,
            "sample_time_ticks":index+1,
            "projection_frame_index":index+1,
            "particle_sequence_frame_index":index+1,
            "trail_frame_index":index,
            "trail_bloom_frame_index":index,
            "projection_frame_canonical_sha256":"a".repeat(64),
            "projection_socket_transform_inventory_sha256":"a".repeat(64),
            "projection_socket_transform_readback_sha256":"a".repeat(64),
            "particle_sequence_key_sha256":"a".repeat(64),
            "particle_sequence_frame_canonical_sha256":"a".repeat(64),
            "trail_sequence_key_sha256":"a".repeat(64),
            "trail_sequence_frame_canonical_sha256":"a".repeat(64),
            "trail_key_sha256":"a".repeat(64),
            "trail_inventory_sha256":"a".repeat(64),
            "trail_id_encoding_sha256":"a".repeat(64),
            "emitter_binding_sha256":"a".repeat(64),
            "trail_bloom_sequence_key_sha256":"a".repeat(64),
            "trail_bloom_sequence_frame_canonical_sha256":"a".repeat(64),
            "trail_bloom_key_sha256":"a".repeat(64),
            "trail_bloom_seed_sha256":"a".repeat(64),
            "base_frame_key_sha256":"a".repeat(64),
            "bloom_key_sha256":"a".repeat(64),
            "camera_object_sha256":"a".repeat(64),
            "camera_identity_sha256":"a".repeat(64),
            "render_profile_sha256":"a".repeat(64),
            "render_worker_build_cohort_sha256":"a".repeat(64),
            "canonical_sha256":"a".repeat(64),
            "created_at":"2026-08-22T00:00:00Z"
        }));
    }
    let response = json!({
        "schema_version":"FictionalEnergyVfxAnimatedSocketAttachmentGetResult@3",
        "attachment_key_sha256":"a".repeat(64),
        "attachment":attachment,
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "commercial_fps_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "actual_engine_roundtrip":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    assert!(validate_response(name, &response, &Binding::default()).is_ok());
    let mut tampered = response.clone();
    tampered["attachment"]["frames"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(validate_response(name, &tampered, &Binding::default()).is_err());
    let mut media = response;
    media["attachment"]["frames"][0]["glb_bytes"] = json!("not-allowed");
    assert!(validate_response(name, &media, &Binding::default()).is_err());
}

fn retopo_prepare_request() -> Value {
    let mut request = json!({
        "schema_version":"ProductionWeaponRetopologyCageSourceBundlePrepareRequest@1",
        "bundle_key_sha256":null,
        "project_id":"project-1",
        "source_candidate_id":"candidate-1",
        "source_candidate_state_sha256":"a".repeat(64),
        "source_high_artifact_sha256":"b".repeat(64),
        "source_high_artifact_readback_object_sha256":"c".repeat(64),
        "target_triangle_count":100,
        "max_collapses":10,
        "locked_vertices":[{"primitive_ordinal":0,"vertex_index":0}],
        "offset_m":0.001,
        "max_offset_m":0.01,
        "max_coordinate_abs_m":10.0,
        "low_retopology_policy":"bounded-low-retopology-topology-correspondent-cage-source-only@1",
        "cage_policy":"bounded-low-retopology-topology-correspondent-cage-source-only@1",
        "input_sha256":"",
        "idempotency_key":"idem-1"
    });
    let mut preimage = request.clone();
    preimage.as_object_mut().unwrap().remove("input_sha256");
    preimage.as_object_mut().unwrap().remove("idempotency_key");
    request["input_sha256"] = json!(forgecad_runtime::canonical_json_hash(&preimage));
    request
}

fn retopo_response() -> Value {
    let hash = "a".repeat(64);
    let mut bundle = Map::new();
    for field in [
        "bundle_key_sha256",
        "source_candidate_state_sha256",
        "source_high_artifact_sha256",
        "source_high_artifact_readback_object_sha256",
        "low_artifact_sha256",
        "low_artifact_readback_object_sha256",
        "cage_artifact_sha256",
        "cage_artifact_readback_object_sha256",
        "low_mesh_object_sha256",
        "correspondence_object_sha256",
        "cage_offset_field_object_sha256",
        "receipt_object_sha256",
        "request_sha256",
        "canonical_sha256",
    ] {
        bundle.insert(field.to_owned(), Value::String(hash.clone()));
    }
    for (field, value) in [
        (
            "schema_version",
            json!("ProductionWeaponRetopologyCageSourceBundle@1"),
        ),
        ("project_id", json!("project-1")),
        ("source_candidate_id", json!("candidate-1")),
        (
            "low_retopology_policy",
            json!("bounded-low-retopology-topology-correspondent-cage-source-only@1"),
        ),
        (
            "cage_policy",
            json!("bounded-low-retopology-topology-correspondent-cage-source-only@1"),
        ),
        (
            "source_status",
            json!("runtime-owned-durable-production-weapon-retopology-cage-source-bundle"),
        ),
        ("quality_status", json!("structural_only")),
        ("visual_quality_status", json!("NOT_PROVEN")),
        ("human_review_status", json!("NOT_RUN")),
        ("commercial_engine_status", json!("NOT_RUN")),
        ("created_at", json!("2026-08-23T00:00:00Z")),
    ] {
        bundle.insert(field.to_owned(), value);
    }
    for (field, value) in [
        ("runtime_write_performed", json!(true)),
        ("production_stage_advanced", json!(false)),
        ("candidate_confirmed", json!(false)),
        ("version_created", json!(false)),
        ("export_performed", json!(false)),
    ] {
        bundle.insert(field.to_owned(), value);
    }
    let mut normalized = Value::Object(bundle.clone());
    let normalized_object = normalized.as_object_mut().unwrap();
    for field in [
        "bundle_key_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ] {
        normalized_object.insert(field.to_owned(), Value::String(String::new()));
    }
    let key = forgecad_runtime::canonical_json_hash(&normalized);
    bundle.insert("bundle_key_sha256".to_owned(), Value::String(key.clone()));
    bundle.insert("canonical_sha256".to_owned(), Value::String(key.clone()));
    json!({
        "schema_version":"ProductionWeaponRetopologyCageSourceBundleGetResult@1",
        "bundle_key_sha256":key,
        "bundle":Value::Object(bundle),
        "replayed":false,
        "restart_hash_verified":true,
        "runtime_write":false,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    })
}

#[test]
fn production_weapon_retopology_cage_source_surface_is_hidden_and_maps_bundle_runtime() {
    let prepare_name = "production_weapon_retopology_cage_source_prepare";
    let get_name = "production_weapon_retopology_cage_source_get";
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == prepare_name)
        .expect("retopology/Cage prepare tool");
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == get_name)
        .expect("retopology/Cage get tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        runtime_method(prepare_name),
        Some("production_weapon_retopology_cage_source_bundle_prepare")
    );
    assert_eq!(
        runtime_method(get_name),
        Some("production_weapon_retopology_cage_source_bundle_get")
    );
    let get_request = json!({
        "schema_version":"ProductionWeaponRetopologyCageSourceBundleGetRequest@1",
        "bundle_key_sha256":"a".repeat(64),
        "project_id":"project-1",
        "source_candidate_id":"candidate-1"
    });
    assert!(validate_declared_tool_input(get_name, &get_request, false).is_ok());
    assert!(validate_call(get_name, &get_request, &Binding::default()).is_ok());
}

#[test]
fn production_weapon_retopology_cage_source_prepare_is_closed_and_scope_bound() {
    let name = "production_weapon_retopology_cage_source_prepare";
    let request = retopo_prepare_request();
    assert!(validate_declared_tool_input(name, &request, true).is_ok());
    assert!(validate_call(name, &request, &Binding::default()).is_err());
    assert!(validate_call(name, &request, &bound()).is_ok());
    let mut mismatch = request.clone();
    mismatch["project_id"] = json!("project-2");
    assert!(validate_call(name, &mismatch, &bound()).is_err());
    let mut raw = request.clone();
    raw["glb_base64"] = json!("not-allowed");
    assert!(validate_declared_tool_input(name, &raw, true).is_err());
}

#[test]
fn production_weapon_retopology_cage_source_response_is_hash_only_and_structural() {
    let name = "production_weapon_retopology_cage_source_get";
    let response = retopo_response();
    assert!(validate_response(name, &response, &Binding::default()).is_ok());
    let mut raw = response.clone();
    raw["bundle"]["offset_field"] = json!([0.01, 0.02]);
    assert!(validate_response(name, &raw, &Binding::default()).is_err());
    let mut unsafe_flags = response.clone();
    unsafe_flags["production_stage_advanced"] = json!(true);
    assert!(validate_response(name, &unsafe_flags, &Binding::default()).is_err());
    let mut mismatch = response;
    mismatch["bundle"]["project_id"] = json!("project-2");
    assert!(validate_response(name, &mismatch, &bound()).is_err());
}

fn production_weapon_form_quality_v2_response(is_prepare: bool) -> Value {
    let hash = "a".repeat(64);
    let policy = "production-weapon-form-quality-six-view-art-evidence-gate@2";
    let threshold = "production-weapon-form-view-thresholds@1";
    let decision = |view_kind: &str| {
        json!({
            "view_kind":view_kind,
            "legacy_form_quality_view_id":format!("legacy-view-{view_kind}"),
            "legacy_form_quality_view_canonical_sha256":hash.clone(),
            "form_art_view_id":format!("art-view-{view_kind}"),
            "form_art_view_canonical_sha256":hash.clone(),
            "form_art_view_receipt_object_sha256":hash.clone(),
            "target_object_sha256":hash.clone(),
            "target_canonical_sha256":hash.clone(),
            "silhouette_pass_object_sha256":hash.clone(),
            "part_id_pass_object_sha256":hash.clone(),
            "depth_pass_object_sha256":hash.clone(),
            "normal_pass_object_sha256":hash.clone(),
            "cross_view_thresholds_passed":true,
            "no_regression_passed":true,
            "part_id_passed":true,
            "negative_space_passed":true,
            "line_flow_passed":true,
            "view_passed":true
        })
    };
    let views = [
        decision("front"),
        decision("back"),
        decision("left"),
        decision("right"),
        decision("top"),
        decision("rear-three-quarter"),
    ];
    let mut record = Map::new();
    record.extend([
        (
            "schema_version".to_owned(),
            json!("ProductionWeaponFormQuality@2"),
        ),
        ("form_quality_id".to_owned(), json!("form-quality-v2-1")),
        ("session_id".to_owned(), json!("session-1")),
        ("project_id".to_owned(), json!("project-1")),
        ("form_stage".to_owned(), json!("blockout")),
        ("source_stage".to_owned(), json!("camera-calibrated")),
        ("target_stage".to_owned(), json!("blockout-reviewed")),
        (
            "current_source_head_transition_id".to_owned(),
            json!("transition-1"),
        ),
        (
            "current_source_head_transition_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "current_source_head_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "current_source_head_stage".to_owned(),
            json!("camera-calibrated"),
        ),
        (
            "current_source_head_candidate_id".to_owned(),
            json!("candidate-1"),
        ),
        (
            "current_source_head_candidate_state_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "current_source_head_artifact_id".to_owned(),
            json!("artifact-1"),
        ),
        (
            "current_source_head_artifact_sha256".to_owned(),
            json!(hash.clone()),
        ),
        ("candidate_id".to_owned(), json!("candidate-1")),
        ("candidate_state_sha256".to_owned(), json!(hash.clone())),
        ("artifact_id".to_owned(), json!("artifact-1")),
        ("artifact_sha256".to_owned(), json!(hash.clone())),
        ("reference_id".to_owned(), json!("reference-1")),
        ("reference_sha256".to_owned(), json!(hash.clone())),
        (
            "reference_canvas_object_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "reference_canvas_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        ("design_spec_object_sha256".to_owned(), json!(hash.clone())),
        (
            "design_spec_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        ("camera_hash".to_owned(), json!(hash.clone())),
        ("camera_lock_id".to_owned(), json!("camera-lock-1")),
        (
            "camera_lock_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        ("camera_rig_object_sha256".to_owned(), json!(hash.clone())),
        (
            "camera_rig_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "camera_lock_receipt_object_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "camera_lock_source_transition_id".to_owned(),
            json!("transition-1"),
        ),
        (
            "camera_lock_source_transition_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "camera_lock_source_head_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
    ]);
    record.extend([
        (
            "reviewed_reference_view_kinds".to_owned(),
            json!([
                "front",
                "back",
                "left",
                "right",
                "top",
                "rear-three-quarter"
            ]),
        ),
        (
            "fixed_camera_view_kinds".to_owned(),
            json!([
                "front",
                "back",
                "left",
                "right",
                "top",
                "bottom",
                "rear-three-quarter"
            ]),
        ),
        (
            "legacy_form_quality_object_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "legacy_form_quality_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "form_art_evidence_object_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "form_art_evidence_canonical_sha256".to_owned(),
            json!(hash.clone()),
        ),
        (
            "view_decisions".to_owned(),
            Value::Array(views.into_iter().collect()),
        ),
        (
            "aggregate".to_owned(),
            json!({
                "view_count":6,
                "all_cross_view_thresholds_passed":true,
                "all_no_regression_passed":true,
                "all_part_id_passed":true,
                "all_negative_space_passed":true,
                "all_line_flow_passed":true,
                "all_view_passed":true
            }),
        ),
        ("previous_form_quality_id".to_owned(), Value::Null),
        (
            "previous_form_quality_report_object_sha256".to_owned(),
            Value::Null,
        ),
        (
            "previous_form_quality_canonical_sha256".to_owned(),
            Value::Null,
        ),
        ("form_quality_policy".to_owned(), json!(policy)),
        (
            "form_quality_policy_sha256".to_owned(),
            json!(forgecad_runtime::sha256_hex(policy.as_bytes())),
        ),
        ("threshold_policy".to_owned(), json!(threshold)),
        (
            "threshold_policy_sha256".to_owned(),
            json!(forgecad_runtime::sha256_hex(threshold.as_bytes())),
        ),
        ("hard_gate_passed".to_owned(), json!(true)),
        ("form_gate_passed".to_owned(), json!(true)),
        ("validator_status".to_owned(), json!("passed")),
        (
            "structural_status".to_owned(),
            json!("PASS_SOURCE_STRUCTURAL"),
        ),
        (
            "visual_status".to_owned(),
            json!("PASS_STAGE_VISUAL_STRUCTURE_ONLY"),
        ),
        ("human_status".to_owned(), json!("NOT_RUN")),
        ("engine_status".to_owned(), json!("NOT_RUN")),
        ("distribution_status".to_owned(), json!("NOT_RUN")),
        ("quality_status".to_owned(), json!("PASS_FORM_GATE")),
        ("runtime_write_performed".to_owned(), json!(true)),
        ("production_stage_advanced".to_owned(), json!(false)),
        ("candidate_confirmed".to_owned(), json!(false)),
        ("version_created".to_owned(), json!(false)),
        ("export_performed".to_owned(), json!(false)),
        ("request_sha256".to_owned(), json!(hash.clone())),
        ("input_sha256".to_owned(), json!(hash.clone())),
        ("receipt_object_sha256".to_owned(), json!(hash.clone())),
        ("canonical_sha256".to_owned(), json!(hash.clone())),
        ("created_at".to_owned(), json!("2026-08-23T00:00:00Z")),
    ]);
    let record = Value::Object(record);
    let mut response = json!({
        "schema_version":if is_prepare {
            "ProductionWeaponFormQualityPrepareResult@2"
        } else {
            "ProductionWeaponFormQualityGetResult@2"
        },
        "form_quality":record,
        "replayed":true,
        "runtime_write":is_prepare,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    if !is_prepare {
        response["restart_hash_verified"] = Value::Bool(true);
    }
    response
}

fn production_weapon_form_quality_v2_preflight_request() -> Value {
    let hash = "a".repeat(64);
    let mut request = json!({
        "schema_version":"ProductionWeaponFormQualityV2PreflightGetRequest@1",
        "preflight_id":"preflight-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "form_stage":"blockout",
        "evidence_source_kind":"legacy-source",
        "legacy_form_quality_object_sha256":hash.clone(),
        "legacy_form_quality_canonical_sha256":hash.clone(),
        "form_art_evidence_object_sha256":hash.clone(),
        "form_art_evidence_canonical_sha256":hash.clone(),
        "current_source_head_transition_id":"transition-1",
        "current_source_head_transition_sha256":hash.clone(),
        "current_source_head_canonical_sha256":hash,
        "input_sha256":""
    });
    let mut preimage = request.clone();
    preimage.as_object_mut().unwrap().remove("input_sha256");
    request["input_sha256"] = Value::String(forgecad_runtime::canonical_json_hash(&preimage));
    request
}

fn production_weapon_form_quality_v2_preflight_response() -> Value {
    let check = |reason_code: &str| {
        json!({
            "status":"blocked",
            "reason_code":reason_code,
            "object_sha256":null,
            "canonical_sha256":null
        })
    };
    let mut blockers = vec![
        "legacy_form_quality:LEGACY_FORM_QUALITY_REQUIRED".to_owned(),
        "form_art_evidence:FORM_ART_EVIDENCE_REQUIRED".to_owned(),
        "form_art_target_observation:FORM_ART_EVIDENCE_REQUIRED".to_owned(),
        "cross_view_evidence:LEGACY_FORM_QUALITY_REQUIRED".to_owned(),
        "camera_lock_stage:LEGACY_AND_FORM_ART_REQUIRED".to_owned(),
        "reference_authoring:LEGACY_AND_FORM_ART_REQUIRED".to_owned(),
        "candidate_artifact:CANDIDATE_ARTIFACT_REQUIRED".to_owned(),
    ];
    blockers.sort();
    let mut response = json!({
        "schema_version":"ProductionWeaponFormQualityV2PreflightGetResult@1",
        "preflight_id":"preflight-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "form_stage":"blockout",
        "checks":{
            "legacy_form_quality":check("LEGACY_FORM_QUALITY_REQUIRED"),
            "form_art_evidence":check("FORM_ART_EVIDENCE_REQUIRED"),
            "form_art_target_observation":check("FORM_ART_EVIDENCE_REQUIRED"),
            "cross_view_evidence":check("LEGACY_FORM_QUALITY_REQUIRED"),
            "camera_lock_stage":check("LEGACY_AND_FORM_ART_REQUIRED"),
            "reference_authoring":check("LEGACY_AND_FORM_ART_REQUIRED"),
            "candidate_artifact":check("CANDIDATE_ARTIFACT_REQUIRED")
        },
        "ready_for_v2_prepare":false,
        "blocking_reasons":blockers,
        "quality_status":"NOT_PROVEN",
        "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "runtime_write":false,
        "worker_started":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "restart_hash_verified":true,
        "readiness_sha256":""
    });
    let mut preimage = response.clone();
    preimage["readiness_sha256"] = Value::String(String::new());
    response["readiness_sha256"] = Value::String(forgecad_runtime::canonical_json_hash(&preimage));
    response
}

#[test]
fn production_weapon_form_quality_v2_preflight_surface_is_closed_read_only_and_unscoped() {
    let name = "production_weapon_form_quality_v2_preflight_get";
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == name)
        .expect("form-quality-v2 preflight get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap().len(),
        15
    );
    assert_eq!(
        tool["inputSchema"]["properties"].as_object().unwrap().len(),
        42
    );
    assert_eq!(runtime_method(name), Some(name));
    assert!(!is_write_tool(name));
    assert!(!write_tool_names().iter().any(|tool| tool == name));

    let request = production_weapon_form_quality_v2_preflight_request();
    assert!(validate_declared_tool_input(name, &request, false).is_ok());
    assert!(validate_call(name, &request, &Binding::default()).is_ok());
    assert!(validate_call(name, &request, &bound()).is_ok());
    let mut mismatch = request.clone();
    mismatch["candidate_id"] = json!("candidate-foreign");
    assert!(validate_call(name, &mismatch, &bound()).is_err());
    for field in ["raw_png_bytes", "path", "url", "script", "secret"] {
        let mut forbidden = request.clone();
        forbidden[field] = json!("not-allowed");
        assert!(validate_call(name, &forbidden, &Binding::default()).is_err());
        assert!(validate_declared_tool_input(name, &forbidden, false).is_err());
    }
}

#[test]
fn production_weapon_form_quality_v2_preflight_response_is_hash_bound_and_side_effect_free() {
    let name = "production_weapon_form_quality_v2_preflight_get";
    let response = production_weapon_form_quality_v2_preflight_response();
    assert!(validate_response(name, &response, &Binding::default()).is_ok());
    let mut write = response.clone();
    write["runtime_write"] = json!(true);
    assert!(validate_response(name, &write, &Binding::default()).is_err());
    let mut bad_hash = response.clone();
    bad_hash["readiness_sha256"] = json!("b".repeat(64));
    assert!(validate_response(name, &bad_hash, &Binding::default()).is_err());
    let mut raw = response.clone();
    raw["checks"]["legacy_form_quality"]["path"] = json!("/tmp/raw");
    assert!(validate_response(name, &raw, &Binding::default()).is_err());
    let mut mismatch = response;
    mismatch["candidate_id"] = json!("candidate-foreign");
    assert!(validate_response(name, &mismatch, &bound()).is_err());
}

fn production_weapon_high_low_bake_preflight_request() -> Value {
    let hash = "a".repeat(64);
    let mut request = json!({
        "schema_version":"ProductionWeaponHighLowBakePreflightGetRequest@1",
        "preflight_id":"high-low-preflight-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "expected_head_stage":"secondary-form-approved",
        "expected_head_transition_id":"transition-1",
        "expected_head_transition_sha256":hash.clone(),
        "expected_head_canonical_sha256":hash,
        "input_sha256":""
    });
    let mut preimage = request.clone();
    preimage.as_object_mut().unwrap().remove("input_sha256");
    request["input_sha256"] = Value::String(forgecad_runtime::canonical_json_hash(&preimage));
    request
}

fn production_weapon_high_low_bake_preflight_response() -> Value {
    let check = |reason_code: &str| {
        json!({
            "status":"missing",
            "reason_code":reason_code,
            "object_sha256":null,
            "canonical_sha256":null
        })
    };
    let blockers = vec![
        "AUTHORING_LOW_TOPOLOGY_MISSING",
        "FORMAL_CAGE_ARTIFACT_MISSING",
        "FORMAL_HIGH_ARTIFACT_MISSING",
        "HERO_UV_LAYOUT_MISSING",
        "HIGH_LOW_CORRESPONDENCE_MISSING",
        "RAY_DIAGNOSTIC_NOT_RUN",
        "SECONDARY_FORM_HEAD_MISSING",
    ];
    let mut response = json!({
        "schema_version":"ProductionWeaponHighLowBakePreflightGetResult@1",
        "preflight_id":"high-low-preflight-1",
        "session_id":"session-1",
        "project_id":"project-1",
        "candidate_id":"candidate-1",
        "expected_head_stage":"secondary-form-approved",
        "observed_head_stage":null,
        "observed_head_transition_id":null,
        "observed_head_transition_sha256":null,
        "observed_head_canonical_sha256":null,
        "checks":{
            "authoring_low_topology":check("AUTHORING_LOW_TOPOLOGY_MISSING"),
            "formal_bake":check("FORMAL_BAKE_NOT_REACHED"),
            "formal_cage_artifact":check("FORMAL_CAGE_ARTIFACT_MISSING"),
            "formal_high_artifact":check("FORMAL_HIGH_ARTIFACT_MISSING"),
            "hero_uv_layout":check("HERO_UV_LAYOUT_MISSING"),
            "high_low_correspondence":check("HIGH_LOW_CORRESPONDENCE_MISSING"),
            "ray_diagnostic":check("RAY_DIAGNOSTIC_NOT_RUN"),
            "secondary_form_head":check("SECONDARY_FORM_HEAD_MISSING")
        },
        "ready_for_formal_bake":false,
        "blocking_reasons":blockers,
        "quality_status":"structural_only",
        "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN",
        "commercial_engine_status":"NOT_RUN",
        "distribution_status":"NOT_RUN",
        "runtime_write":false,
        "worker_started":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "restart_hash_verified":true,
        "readiness_sha256":""
    });
    let mut preimage = response.clone();
    preimage["readiness_sha256"] = Value::String(String::new());
    response["readiness_sha256"] = Value::String(forgecad_runtime::canonical_json_hash(&preimage));
    response
}

#[test]
fn production_weapon_high_low_bake_preflight_is_closed_read_only_and_scope_bound() {
    let name = "production_weapon_high_low_bake_preflight_get";
    let tool = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == name)
        .expect("HighLowBake preflight get tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["writeIntent"], false);
    assert_eq!(tool["annotations"]["approvalRequired"], false);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        tool["inputSchema"]["required"].as_array().unwrap().len(),
        10
    );
    assert_eq!(
        tool["inputSchema"]["properties"].as_object().unwrap().len(),
        10
    );
    assert_eq!(runtime_method(name), Some(name));
    assert!(!is_write_tool(name));

    let request = production_weapon_high_low_bake_preflight_request();
    assert!(validate_declared_tool_input(name, &request, false).is_ok());
    assert!(validate_call(name, &request, &Binding::default()).is_ok());
    assert!(validate_call(name, &request, &bound()).is_ok());
    let mut mismatch = request.clone();
    mismatch["candidate_id"] = json!("candidate-foreign");
    assert!(validate_call(name, &mismatch, &bound()).is_err());
    let mut forbidden = request;
    forbidden["mesh_base64"] = json!("not-allowed");
    assert!(validate_declared_tool_input(name, &forbidden, false).is_err());
}

#[test]
fn production_weapon_high_low_bake_preflight_response_is_hash_bound_and_non_writing() {
    let name = "production_weapon_high_low_bake_preflight_get";
    let response = production_weapon_high_low_bake_preflight_response();
    assert!(validate_response(name, &response, &Binding::default()).is_ok());
    assert!(validate_response(name, &response, &bound()).is_ok());
    let mut write = response.clone();
    write["worker_started"] = json!(true);
    assert!(validate_response(name, &write, &Binding::default()).is_err());
    let mut reordered = response.clone();
    reordered["blocking_reasons"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert!(validate_response(name, &reordered, &Binding::default()).is_err());
    let mut partial_head = response.clone();
    partial_head["observed_head_stage"] = json!("camera-calibrated");
    assert!(validate_response(name, &partial_head, &Binding::default()).is_err());
    let mut forged_formal_pass = response.clone();
    forged_formal_pass["checks"]["formal_bake"]["status"] = json!("passed");
    forged_formal_pass["checks"]["formal_bake"]["reason_code"] = json!("FORMAL_BAKE_VERIFIED");
    let mut forged_preimage = forged_formal_pass.clone();
    forged_preimage["readiness_sha256"] = Value::String(String::new());
    forged_formal_pass["readiness_sha256"] =
        Value::String(forgecad_runtime::canonical_json_hash(&forged_preimage));
    assert!(validate_response(name, &forged_formal_pass, &Binding::default()).is_err());
    let mut raw = response;
    raw["checks"]["formal_bake"]["glb_bytes"] = json!("forbidden");
    assert!(validate_response(name, &raw, &Binding::default()).is_err());
}

#[test]
fn production_weapon_form_quality_v2_surface_is_hidden_closed_and_read_only_get() {
    let prepare_name = "production_weapon_form_quality_v2_prepare";
    let get_name = "production_weapon_form_quality_v2_get";
    let prepare = write_tools()
        .into_iter()
        .find(|tool| tool["name"] == prepare_name)
        .expect("form-quality-v2 prepare tool");
    let get = read_tools()
        .into_iter()
        .find(|tool| tool["name"] == get_name)
        .expect("form-quality-v2 get tool");
    assert_eq!(prepare["annotations"]["readOnlyHint"], false);
    assert_eq!(prepare["annotations"]["writeIntent"], true);
    assert_eq!(prepare["annotations"]["approvalRequired"], false);
    assert_eq!(prepare["inputSchema"]["additionalProperties"], false);
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["writeIntent"], false);
    assert_eq!(get["annotations"]["approvalRequired"], false);
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        prepare["inputSchema"]["required"].as_array().unwrap().len(),
        24
    );
    assert_eq!(get["inputSchema"]["required"].as_array().unwrap().len(), 7);
    assert_eq!(
        runtime_method(prepare_name),
        Some("production_weapon_form_quality_v2_prepare")
    );
    assert_eq!(
        runtime_method(get_name),
        Some("production_weapon_form_quality_v2_get")
    );
}

#[test]
fn production_weapon_form_quality_v2_response_rejects_aov_and_retarget() {
    let get_name = "production_weapon_form_quality_v2_get";
    let response = production_weapon_form_quality_v2_response(false);
    assert!(validate_response(get_name, &response, &Binding::default()).is_ok());
    let prepare_name = "production_weapon_form_quality_v2_prepare";
    let prepare = production_weapon_form_quality_v2_response(true);
    assert!(validate_response(prepare_name, &prepare, &Binding::default()).is_ok());
    let mut raw_aov = response.clone();
    raw_aov["form_quality"]["view_decisions"][0]["raw_aov_bytes"] = json!("forbidden");
    assert!(validate_response(get_name, &raw_aov, &Binding::default()).is_err());
    let mut retargeted = response;
    retargeted["form_quality"]["candidate_id"] = json!("candidate-foreign");
    assert!(validate_response(get_name, &retargeted, &bound()).is_err());
}
