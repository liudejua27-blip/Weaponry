use base64::Engine;
use forgecad_runtime::{canonical_json_hash, sha256_hex, Runtime};
use serde_json::{json, Value};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn program(project_id: &str, catalog_sha256: &str, segments: u64) -> Value {
    let mut value = json!({
        "schema_version":"GeometryProgram@2",
        "project_id":project_id,
        "representation_plan_sha256":"5".repeat(64),
        "operator_catalog_sha256":catalog_sha256,
        "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
        "budgets":{"max_nodes":2,"max_triangles":1000,"max_glb_bytes":1048576,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
        "nodes":[
            {"node_id":"root-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[0.4,0.4,0.4],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
            {"node_id":"arm-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"cylinder","radius_m":0.2,"height_m":0.8,"radial_segments":segments,"position_m":[1.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}
        ],
        "part_outputs":[
            {"part_id":"root-part","input_node_ids":["root-node"],"material_zone_id":"zone-root","solid":true},
            {"part_id":"arm-part","input_node_ids":["arm-node"],"material_zone_id":"zone-arm","solid":true}
        ]
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    value
}

fn clip_prepare_request(project_id: &str, prepared: &Value) -> Value {
    let artifact = &prepared["artifact"];
    let mut sequence = json!({
        "schema_version":"MechanicalPoseSequencePreviewRequest@1",
        "project_id":project_id,
        "artifact_id":artifact["artifact_id"],
        "candidate_id":prepared["candidate"]["candidate_id"],
        "artifact_readback_sha256":artifact["canonical_sha256"],
        "program_sha256":artifact["program_sha256"],
        "operator_catalog_sha256":artifact["operator_catalog_sha256"],
        "readback_config_sha256":artifact["readback_config_sha256"],
        "rest_frame_draft":{
            "schema_version":"MechanicalRestFrameDraft@1",
            "rest_frame_id":"engine-probe-rest",
            "coordinate_system":"forgecad-rh-y-up-m@1",
            "transform_convention":"column-vector-trs-quaternion@1",
            "root_link_id":"root-link",
            "links":[
                {"link_id":"root-link","part_id":"root-part","source_node_ids":["root-node"],"joint_type":"fixed","rest_translation_m":[0.0,0.0,0.0],"rest_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"axis_local":null,"limit_min":null,"limit_max":null,"value_unit":"none"},
                {"link_id":"arm-link","part_id":"arm-part","source_node_ids":["arm-node"],"joint_type":"revolute","rest_translation_m":[1.0,0.0,0.0],"rest_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"axis_local":[0.0,1.0,0.0],"limit_min":-1.0,"limit_max":1.0,"value_unit":"radian"}
            ],
            "parent_map":[{"child_link_id":"arm-link","parent_link_id":"root-link"}]
        },
        "pose_action_draft":{
            "schema_version":"MechanicalPoseActionDraft@1",
            "action_id":"engine-probe-action",
            "timebase_hz":1000,
            "duration_ticks":1000,
            "interpolation":"linear@1",
            "extrapolation":"clamp@1",
            "unkeyed_policy":"rest@1",
            "channels":[{"link_id":"arm-link","value_unit":"radian","keys":[{"time_ticks":0,"value":0.0},{"time_ticks":1000,"value":0.5}]}]
        },
        "sample_time_ticks":[0,500,1000],
        "input_sha256":""
    });
    let mut sequence_preimage = sequence.clone();
    sequence_preimage
        .as_object_mut()
        .unwrap()
        .remove("input_sha256");
    sequence["input_sha256"] = Value::String(canonical_json_hash(&sequence_preimage));
    let mut request = json!({
        "schema_version":"MechanicalAnimationClipPrepareRequest@1",
        "clip_id":"engine-probe-clip",
        "pose_sequence_request":sequence,
        "clip_policy":"runtime-owned-immutable-cas-rigid-mechanical-action@1",
        "input_sha256":""
    });
    let mut preimage = request.clone();
    preimage.as_object_mut().unwrap().remove("input_sha256");
    request["input_sha256"] = Value::String(canonical_json_hash(&preimage));
    request
}

fn request_hash(mut value: Value) -> Value {
    let mut preimage = value.clone();
    preimage.as_object_mut().unwrap().remove("canonical_sha256");
    value["canonical_sha256"] = Value::String(canonical_json_hash(&preimage));
    value
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forgecad-threejs-game-delivery-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    let database = root.join("runtime.sqlite");
    let cas = root.join("cas");
    let runtime = Runtime::open_with_cas(&database, &cas)?;
    let project = runtime.create_project("Three.js delivery probe", json!({"profile":"mvp"}))?;
    let catalog_sha256 = runtime.active_operator_catalog()["canonical_sha256"]
        .as_str()
        .ok_or("operator catalog hash missing")?
        .to_owned();
    let prepared = [64, 32, 16]
        .into_iter()
        .map(|segments| {
            runtime.prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({
                    "typed":"geometry",
                    "geometry_program":program(&project.project_id, &catalog_sha256, segments)
                }),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    runtime.mechanical_animation_clip_prepare(&clip_prepare_request(
        &project.project_id,
        &prepared[0],
    ))?;
    let animation = runtime.mechanical_animation_glb_prepare(&request_hash(json!({
        "schema_version":"MechanicalAnimationGlbPrepareRequest@1",
        "project_id":project.project_id,
        "candidate_id":prepared[0]["candidate"]["candidate_id"],
        "candidate_state_sha256":prepared[0]["candidate"]["canonical_sha256"],
        "clip_id":"engine-probe-clip",
        "materialization_policy":"rigid-node-trs-gltf-linear-scheduled-samples@1",
        "canonical_sha256":""
    })))?;
    let lods = prepared
        .iter()
        .enumerate()
        .map(|(level, value)| {
            json!({
                "level":level,
                "candidate_id":value["candidate"]["candidate_id"],
                "candidate_state_sha256":value["candidate"]["canonical_sha256"],
                "artifact_sha256":value["artifact"]["artifact_id"],
                "artifact_readback_sha256":value["artifact"]["canonical_sha256"]
            })
        })
        .collect::<Vec<_>>();
    let delivery = runtime.game_asset_delivery_prepare(&request_hash(json!({
        "schema_version":"GameAssetDeliveryPrepareRequest@1",
        "project_id":project.project_id,
        "lods":lods,
        "animation":{
            "clip_id":"engine-probe-clip",
            "animated_artifact_sha256":animation["animated_artifact_sha256"],
            "receipt_object_sha256":animation["receipt_object_sha256"]
        },
        "lod_policy":"authored-three-level-part-stable-progressive-triangles@1",
        "collision_policy":"per-part-aabb-box-from-lod2-visual-geometry@1",
        "readiness_policy":"engine-neutral-gltf2-embedded-assets-stable-names@1",
        "canonical_sha256":""
    })))?;

    let lod_shas = prepared
        .iter()
        .map(|value| {
            value["artifact"]["artifact_id"]
                .as_str()
                .ok_or("LOD hash missing")
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let animated_sha = animation["animated_artifact_sha256"]
        .as_str()
        .ok_or("animated hash missing")?
        .to_owned();
    let delivery_hashes = [
        delivery["lod_receipt_object_sha256"].as_str().unwrap(),
        delivery["collision_proxy_object_sha256"].as_str().unwrap(),
        delivery["readiness_object_sha256"].as_str().unwrap(),
        delivery["delivery_manifest_object_sha256"]
            .as_str()
            .unwrap(),
    ]
    .map(str::to_owned);
    let lod_bytes = lod_shas
        .iter()
        .map(|sha256| runtime.cas_read(sha256))
        .collect::<Result<Vec<_>, _>>()?;
    let animated_bytes = runtime.cas_read(&animated_sha)?;
    drop(runtime);

    let reopened = Runtime::open_with_cas(&database, &cas)?;
    let mut restart_hashes = lod_shas.clone();
    restart_hashes.push(animated_sha.clone());
    restart_hashes.extend(delivery_hashes.iter().cloned());
    let restart_hash_passed = restart_hashes.iter().all(|sha| {
        reopened
            .cas_read(sha)
            .map(|bytes| sha256_hex(&bytes) == *sha)
            .unwrap_or(false)
    });
    let durable_get = reopened.game_asset_delivery_get(&json!({
        "schema_version":"GameAssetDeliveryGetRequest@1",
        "project_id":project.project_id,
        "delivery_manifest_object_sha256":delivery["delivery_manifest_object_sha256"]
    }))?;
    drop(reopened);
    let output = json!({
        "schema_version":"ForgeCadThreeJsGameDeliveryProbe@1",
        "project_id":project.project_id,
        "part_ids":["arm-part","root-part"],
        "lod_triangle_counts":delivery["lod_receipt"]["levels"].as_array().unwrap().iter().map(|level| level["triangle_count"].clone()).collect::<Vec<_>>(),
        "collision_proxies":delivery["collision_proxy_set"]["proxies"],
        "collision_proxy_count":delivery["collision_proxy_set"]["proxies"].as_array().unwrap().len(),
        "lod_sha256s":lod_shas,
        "animated_sha256":animated_sha,
        "animation_channel_count":animation["receipt"]["channel_count"],
        "restart_hash_passed":restart_hash_passed,
        "durable_get_passed":durable_get["restart_hash_verified"],
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_commercial_engine_roundtrip":false,
        "lod_glb_base64s":lod_bytes.iter().map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)).collect::<Vec<_>>(),
        "animated_glb_base64":base64::engine::general_purpose::STANDARD.encode(animated_bytes)
    });
    println!("{}", serde_json::to_string(&output)?);
    let _ = fs::remove_dir_all(root);
    Ok(())
}
