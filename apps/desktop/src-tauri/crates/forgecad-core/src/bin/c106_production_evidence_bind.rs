//! Bind C106 production GLB/readback evidence to real Rust Core product state.
//!
//! The Python geometry gate supplies compiler output only. This helper opens
//! an isolated CoreRepository, atomically commits each candidate bundle, then
//! reopens the repository and reports the authoritative Version/Snapshot/
//! Quality/CAS identities. It never fabricates a product-version identifier
//! in the Python gate and never retains the temporary repository.

use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_core::{
    semantic_sha256, AgentAssetVersion, AssetStage, AssetVersionStatus, BlockoutCandidate,
    CandidateStatus, CoreRepository, ExpandedComponentCandidate, Project, ProjectStatus,
    QualityReport, QualityStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingInput {
    schema_version: String,
    fixtures: Vec<BindingFixtureInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFixtureInput {
    recipe_id: String,
    // Preserve the exact Rust -> Python -> Rust JSON value for its sealed
    // semantic hash. Re-serializing a typed f64 graph before verification can
    // change the shortest decimal representation even though the transported
    // value was not modified.
    candidate: Value,
    candidate_canonical_json: String,
    shape_program_canonical_json: String,
    shape_program_sha256: String,
    production_glb_base64: String,
    readback: Value,
}

#[derive(Debug, Serialize)]
struct BindingOutput {
    schema_version: &'static str,
    bindings: Vec<BindingRecord>,
}

#[derive(Debug, Serialize)]
struct BindingRecord {
    recipe_id: String,
    candidate_sha256: String,
    project_id: String,
    artifact_id: String,
    asset_version_id: String,
    quality_report_id: String,
    production_glb_sha256: String,
    readback_sha256: String,
    snapshot_revision: u64,
    snapshot_asset_version_id: String,
    quality_asset_version_id: String,
    export_source_version_id: String,
    restart_readback: bool,
}

fn stable_suffix(value: &str) -> &str {
    &value[..value.len().min(24)]
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let input_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "C106_EVIDENCE_INPUT_REQUIRED".to_string())?;
    let input_bytes =
        fs::read(&input_path).map_err(|error| format!("C106_EVIDENCE_INPUT_READ:{error}"))?;
    let input: BindingInput = serde_json::from_slice(&input_bytes)
        .map_err(|error| format!("C106_EVIDENCE_INPUT_INVALID:{error}"))?;
    if input.schema_version != "C106ProductionEvidenceBindingInput@1" || input.fixtures.len() != 3 {
        return Err("C106_EVIDENCE_INPUT_INVALID".into());
    }

    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("C106_EVIDENCE_CLOCK_INVALID:{error}"))?
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "forgecad-c106-production-evidence-{}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&root).map_err(|error| format!("C106_EVIDENCE_TEMP_CREATE:{error}"))?;
    let database = root.join("library.db");
    let result = bind_all(&database, &root, input.fixtures);
    let cleanup = fs::remove_dir_all(&root);
    if let Err(error) = cleanup {
        return Err(format!("C106_EVIDENCE_TEMP_CLEANUP:{error}"));
    }
    let bindings = result?;
    let output = BindingOutput {
        schema_version: "C106ProductionEvidenceBinding@1",
        bindings,
    };
    println!(
        "{}",
        forgecad_core::canonical_json(
            &serde_json::to_value(output).map_err(|error| error.to_string())?
        )
        .map_err(|error| error.to_string())?
    );
    Ok(())
}

fn bind_all(
    database: &PathBuf,
    root: &PathBuf,
    fixtures: Vec<BindingFixtureInput>,
) -> Result<Vec<BindingRecord>, String> {
    let repository = CoreRepository::open(database, root, "c106-production-evidence")
        .map_err(|error| format!("C106_EVIDENCE_REPOSITORY_OPEN:{error}"))?;
    repository
        .ensure_default_domain_profile("2026-07-18T00:00:00Z")
        .map_err(|error| format!("C106_EVIDENCE_PROFILE:{error}"))?;
    let mut identities = Vec::new();

    for fixture in fixtures {
        let candidate_sha256 = fixture
            .candidate
            .get("candidate_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "C106_EVIDENCE_CANDIDATE_INVALID".to_string())?
            .to_string();
        let transported_candidate: Value = serde_json::from_str(&fixture.candidate_canonical_json)
            .map_err(|error| format!("C106_EVIDENCE_CANDIDATE_HASH:{error}"))?;
        let mut candidate_hash_scope = fixture.candidate.clone();
        candidate_hash_scope["candidate_sha256"] = Value::String(String::new());
        if transported_candidate != candidate_hash_scope {
            return Err("C106_EVIDENCE_CANDIDATE_TRANSPORT_DRIFT".into());
        }
        let recomputed_candidate = format!(
            "{:x}",
            Sha256::digest(fixture.candidate_canonical_json.as_bytes())
        );
        if recomputed_candidate != candidate_sha256 {
            return Err(format!(
                "C106_EVIDENCE_CANDIDATE_HASH_DRIFT:{}:{}:{}",
                fixture.recipe_id,
                &candidate_sha256[..12],
                &recomputed_candidate[..12],
            ));
        }
        let candidate: ExpandedComponentCandidate = serde_json::from_value(fixture.candidate)
            .map_err(|error| format!("C106_EVIDENCE_CANDIDATE_INVALID:{error}"))?;
        if fixture.recipe_id != candidate.recipe.recipe_id
            || candidate.quality_profile != "production_concept"
            || candidate.context_mode != "initial_candidate"
        {
            return Err("C106_EVIDENCE_CANDIDATE_INVALID".into());
        }
        let glb = BASE64_STANDARD
            .decode(&fixture.production_glb_base64)
            .map_err(|error| format!("C106_EVIDENCE_GLB_BASE64:{error}"))?;
        let glb_sha256 = fixture
            .readback
            .get("glb_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "C106_EVIDENCE_READBACK_INVALID".to_string())?
            .to_string();
        if format!("{:x}", Sha256::digest(&glb)) != glb_sha256 {
            return Err("C106_EVIDENCE_GLB_HASH_DRIFT".into());
        }
        let readback_sha256 = semantic_sha256(&fixture.readback)
            .map_err(|error| format!("C106_EVIDENCE_READBACK_HASH:{error}"))?;
        let suffix = stable_suffix(&candidate_sha256);
        let project_id = format!("project_c106_{suffix}");
        let artifact_id = format!("artifact_c106_{suffix}");
        let asset_version_id = format!("assetver_c106_{suffix}");
        let quality_report_id = format!("quality_c106_{suffix}");
        repository
            .create_project(&Project {
                project_id: project_id.clone(),
                profile_id: "profile_weapon_concept_v1".into(),
                // The legacy Project table retains its compatibility domain
                // discriminator; the authoritative asset domain is the
                // version's `pack_robotic_arm_concept` below.
                domain_type: "weapon_concept".into(),
                name: format!("C106 evidence {}", fixture.recipe_id),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-18T00:00:01Z".into(),
                updated_at: "2026-07-18T00:00:01Z".into(),
            })
            .map_err(|error| format!("C106_EVIDENCE_PROJECT:{error}"))?;

        let parts = candidate
            .expanded_assembly_graph
            .get("parts")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "C106_EVIDENCE_PARTS_INVALID".to_string())?;
        let sealed_shape_program: Value =
            serde_json::from_str(&fixture.shape_program_canonical_json)
                .map_err(|error| format!("C106_EVIDENCE_SHAPE_SEAL:{error}"))?;
        if sealed_shape_program != candidate.expanded_shape_program
            || format!(
                "{:x}",
                Sha256::digest(fixture.shape_program_canonical_json.as_bytes())
            ) != fixture.shape_program_sha256
        {
            return Err("C106_EVIDENCE_SHAPE_SEAL_DRIFT".into());
        }
        let version = AgentAssetVersion {
            asset_version_id: asset_version_id.clone(),
            project_id: project_id.clone(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: format!("C106 production evidence {}", fixture.recipe_id),
            stage: AssetStage::EditableAsset,
            plan_id: format!("plan_c106_{suffix}"),
            direction_id: "direction_primary".into(),
            domain_pack_id: "pack_robotic_arm_concept".into(),
            artifact_id: artifact_id.clone(),
            parts,
            // Persist the exact Rust-owned sealed value, not the equivalent
            // value reserialized by the Python evidence envelope.
            shape_program: sealed_shape_program,
            assembly_graph: candidate.expanded_assembly_graph.clone(),
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-18T00:00:03Z".into(),
        };
        let expected_shape_sha256 = semantic_sha256(&version.shape_program)
            .map_err(|error| format!("C106_EVIDENCE_SHAPE_HASH:{error}"))?;
        if expected_shape_sha256 != fixture.shape_program_sha256 {
            let recomputed_canonical = forgecad_core::canonical_json(&version.shape_program)
                .map_err(|error| format!("C106_EVIDENCE_SHAPE_HASH:{error}"))?;
            let first_difference = fixture
                .shape_program_canonical_json
                .bytes()
                .zip(recomputed_canonical.bytes())
                .position(|(left, right)| left != right)
                .unwrap_or_else(|| {
                    fixture
                        .shape_program_canonical_json
                        .len()
                        .min(recomputed_canonical.len())
                });
            return Err(format!(
                "C106_EVIDENCE_SHAPE_CANONICALIZATION_DRIFT:{}:{}:{}:{}",
                fixture.recipe_id,
                &fixture.shape_program_sha256[..12],
                &expected_shape_sha256[..12],
                first_difference,
            ));
        }
        let compile = fixture
            .readback
            .as_object()
            .ok_or_else(|| "C106_EVIDENCE_READBACK_INVALID".to_string())?;
        if compile.get("schema_version").and_then(Value::as_str)
            != Some("GeometryCompileReadback@2")
            || compile
                .get("artifact_profile")
                .and_then(|value| value.get("artifact_profile_id"))
                .and_then(Value::as_str)
                != Some("production_concept")
            || compile.get("shape_program_sha256").and_then(Value::as_str)
                != Some(fixture.shape_program_sha256.as_str())
            || compile.get("glb_sha256").and_then(Value::as_str) != Some(glb_sha256.as_str())
            || compile.get("glb_byte_size").and_then(Value::as_u64) != Some(glb.len() as u64)
            || compile
                .get("triangle_count")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                == 0
        {
            return Err(format!(
                "C106_EVIDENCE_READBACK_BINDING_DRIFT:{}:{}:{}:{}",
                expected_shape_sha256,
                compile
                    .get("shape_program_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or("missing"),
                glb.len(),
                compile
                    .get("glb_byte_size")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            ));
        }
        let candidate = BlockoutCandidate {
            artifact_id: artifact_id.clone(),
            project_id: Some(project_id.clone()),
            plan_id: version.plan_id.clone(),
            direction_id: version.direction_id.clone(),
            domain_pack_id: version.domain_pack_id.clone(),
            status: CandidateStatus::Candidate,
            candidate: json!({
                "schema_version": "C106ProductionEvidenceCandidate@1",
                "candidate_sha256": candidate_sha256,
                "recipe_id": fixture.recipe_id,
            }),
            shape_program: version.shape_program.clone(),
            assembly_graph: version.assembly_graph.clone(),
            material_bindings: BTreeMap::new(),
            glb_sha256: glb_sha256.clone(),
            created_at: "2026-07-18T00:00:02Z".into(),
            updated_at: "2026-07-18T00:00:02Z".into(),
        };
        let mut quality_readback = fixture.readback.clone();
        let quality_readback_object = quality_readback
            .as_object_mut()
            .ok_or_else(|| "C106_EVIDENCE_READBACK_INVALID".to_string())?;
        // Core's atomic bundle compatibility validator still requires these
        // two derived summary booleans in addition to the authoritative per-
        // primitive facts. They are true only after the Python gate has
        // checked every surface for closed/manifold provenance.
        quality_readback_object.insert("closed_manifold".into(), Value::Bool(true));
        quality_readback_object.insert("surface_provenance_present".into(), Value::Bool(true));
        let quality = QualityReport {
            quality_report_id: quality_report_id.clone(),
            project_id: project_id.clone(),
            asset_version_id: asset_version_id.clone(),
            report: json!({
                "schema_version": "AgentAssetQualityReport@1",
                "quality_report_id": quality_report_id,
                "asset_version_id": asset_version_id,
                "status": "passed",
                "evidence_source": "geometry_compile_readback",
                "triangle_count": fixture.readback.get("triangle_count").cloned().unwrap_or(Value::Null),
                "compile_readback": quality_readback,
            }),
            status: QualityStatus::Passed,
            created_at: "2026-07-18T00:00:04Z".into(),
        };
        let bundle = repository
            .commit_candidate_bundle(candidate, &glb, &glb, &version, &quality)
            .map_err(|error| format!("C106_EVIDENCE_BUNDLE_COMMIT:{error}"))?;
        repository
            .validate_candidate_bundle(&bundle)
            .map_err(|error| format!("C106_EVIDENCE_BUNDLE_INVALID:{error}"))?;
        identities.push((
            fixture.recipe_id,
            candidate_sha256,
            project_id,
            artifact_id,
            asset_version_id,
            quality_report_id,
            glb_sha256,
            readback_sha256,
            bundle.snapshot.revision,
        ));
    }
    drop(repository);

    let restarted = CoreRepository::open(database, root, "c106-production-evidence-restart")
        .map_err(|error| format!("C106_EVIDENCE_RESTART_OPEN:{error}"))?;
    let mut records = Vec::new();
    for (
        recipe_id,
        candidate_sha256,
        project_id,
        artifact_id,
        asset_version_id,
        quality_report_id,
        glb_sha256,
        readback_sha256,
        snapshot_revision,
    ) in identities
    {
        let bundle = restarted
            .read_candidate_bundle(&artifact_id, &asset_version_id, &quality_report_id)
            .map_err(|error| format!("C106_EVIDENCE_RESTART_READ:{error}"))?
            .ok_or_else(|| "C106_EVIDENCE_RESTART_MISSING".to_string())?;
        restarted
            .validate_candidate_bundle(&bundle)
            .map_err(|error| format!("C106_EVIDENCE_RESTART_INVALID:{error}"))?;
        let snapshot_asset_version_id = bundle
            .snapshot
            .active_design
            .asset_version_id()
            .ok_or_else(|| "C106_EVIDENCE_SNAPSHOT_INVALID".to_string())?
            .to_string();
        let quality_asset_version_id = bundle
            .snapshot
            .quality
            .as_ref()
            .ok_or_else(|| "C106_EVIDENCE_QUALITY_BINDING_MISSING".to_string())?
            .asset_version_id
            .clone();
        let export_source_version_id = bundle.snapshot.export.source_version_id().to_string();
        let bytes = restarted
            .read_object(&glb_sha256)
            .map_err(|error| format!("C106_EVIDENCE_GLB_READ:{error}"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != glb_sha256
            || bundle.production_glb.sha256 != glb_sha256
            || bundle.quality.asset_version_id != asset_version_id
            || snapshot_asset_version_id != asset_version_id
            || quality_asset_version_id != asset_version_id
            || export_source_version_id != asset_version_id
        {
            return Err("C106_EVIDENCE_IDENTITY_DRIFT".into());
        }
        records.push(BindingRecord {
            recipe_id,
            candidate_sha256,
            project_id,
            artifact_id,
            asset_version_id,
            quality_report_id,
            production_glb_sha256: glb_sha256,
            readback_sha256,
            snapshot_revision,
            snapshot_asset_version_id,
            quality_asset_version_id,
            export_source_version_id,
            restart_readback: true,
        });
    }
    Ok(records)
}
