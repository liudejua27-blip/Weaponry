use super::*;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const REQUEST_SCHEMA: &str = "ViewerProvenanceGraphRequest@1";
const RESPONSE_SCHEMA: &str = "ViewerProvenanceGraph@1";
const MAX_NODES: usize = 64;
const MAX_EDGES: usize = 128;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "VIEWER_PROVENANCE_GRAPH_INVALID: {}",
        message.into()
    ))
}

fn exact_request(value: &Value) -> Result<&Map<String, Value>, RuntimeError> {
    const FIELDS: [&str; 7] = [
        "schema_version",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "max_nodes",
        "max_edges",
    ];
    let object = value
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    if object.len() != FIELDS.len() + 1
        || FIELDS.iter().any(|field| !object.contains_key(*field))
        || !object.contains_key("canonical_sha256")
        || object
            .keys()
            .any(|key| key != "canonical_sha256" && !FIELDS.contains(&key.as_str()))
    {
        return Err(invalid("request field set is not closed"));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(REQUEST_SCHEMA)
        || object.get("max_nodes").and_then(Value::as_u64) != Some(MAX_NODES as u64)
        || object.get("max_edges").and_then(Value::as_u64) != Some(MAX_EDGES as u64)
    {
        return Err(invalid("request policy differs"));
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("request canonical hash is invalid"))?;
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .expect("validated object")
        .remove("canonical_sha256");
    if canonical_json_hash(&preimage) != canonical {
        return Err(invalid("request canonical hash differs"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| invalid(format!("{field} is invalid")))
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid(format!("{field} is invalid")))
}

#[derive(Default)]
struct GraphBuilder {
    nodes: Vec<Value>,
    edges: Vec<Value>,
    node_ids: BTreeSet<String>,
    edge_keys: BTreeSet<(String, String, String)>,
}

impl GraphBuilder {
    fn node(
        &mut self,
        node_id: impl Into<String>,
        kind: &str,
        label: impl Into<String>,
        contract_schema: Option<&str>,
        object_sha256: Option<&str>,
        canonical_sha256: Option<&str>,
        status: &str,
    ) -> Result<(), RuntimeError> {
        let node_id = node_id.into();
        if self.nodes.len() >= MAX_NODES {
            return Err(invalid("node budget exceeded; projection is not truncated"));
        }
        if node_id.is_empty() || node_id.len() > 128 || !self.node_ids.insert(node_id.clone()) {
            return Err(invalid("node identity is invalid or duplicated"));
        }
        for value in [object_sha256, canonical_sha256].into_iter().flatten() {
            if !is_sha256(value) {
                return Err(invalid("node hash is invalid"));
            }
        }
        self.nodes.push(json!({
            "node_id":node_id,
            "kind":kind,
            "label":label.into(),
            "contract_schema":contract_schema,
            "object_sha256":object_sha256,
            "canonical_sha256":canonical_sha256,
            "status":status,
        }));
        Ok(())
    }

    fn edge(&mut self, from: &str, to: &str, relation: &str) -> Result<(), RuntimeError> {
        if self.edges.len() >= MAX_EDGES {
            return Err(invalid("edge budget exceeded; projection is not truncated"));
        }
        if !self.node_ids.contains(from) || !self.node_ids.contains(to) {
            return Err(invalid("edge is dangling"));
        }
        let key = (from.to_owned(), to.to_owned(), relation.to_owned());
        if !self.edge_keys.insert(key) {
            return Err(invalid("edge is duplicated"));
        }
        let edge_id = format!("edge:{}", self.edges.len() + 1);
        self.edges.push(json!({
            "edge_id":edge_id,
            "from_node_id":from,
            "to_node_id":to,
            "relation":relation,
        }));
        Ok(())
    }
}

impl Runtime {
    /// Build one bounded, complete, read-only provenance projection for the
    /// exact candidate state selected by the Viewer. Optional branches are
    /// explicit; a present-but-invalid branch fails the whole request rather
    /// than yielding a plausible partial graph.
    pub fn viewer_provenance_graph(&self, request: &Value) -> Result<Value, RuntimeError> {
        let object = exact_request(request)?;
        let project_id = text(object, "project_id")?;
        let candidate_id = text(object, "candidate_id")?;
        let candidate_state_sha256 = sha(object, "candidate_state_sha256")?;
        let artifact_id = sha(object, "artifact_id")?;

        let candidate = self.ensure_candidate_artifact_binding(candidate_id, artifact_id)?;
        if candidate.project_id != project_id
            || candidate.canonical_sha256 != candidate_state_sha256
        {
            return Err(invalid(
                "project, candidate state or artifact binding differs",
            ));
        }
        // Reuse the confirmation-grade V2 geometry revalidation without
        // confirming or mutating anything. This verifies GLB bytes, strict
        // readback, program/catalog/config, geometry quality and reference.
        self.revalidate_candidate_for_confirmation(&candidate, artifact_id)?;
        let evidence = self
            .store
            .get_geometry_candidate_evidence(candidate_id)?
            .ok_or_else(|| invalid("durable geometry evidence is unavailable"))?;
        if evidence.project_id != project_id
            || evidence.candidate_id != candidate_id
            || evidence.artifact_object_sha256 != artifact_id
        {
            return Err(invalid("geometry evidence binding differs"));
        }
        let evidence_value = serde_json::to_value(&evidence)
            .map_err(|error| invalid(format!("geometry evidence serialization failed: {error}")))?;
        verify_output_canonical_hash(&evidence_value, "GeometryCandidateEvidence@1")?;

        let program: Value = serde_json::from_slice(&self.cas_read_bounded(
            &evidence.geometry_program_object_sha256,
            MAX_DERIVED_JSON_BYTES,
        )?)
        .map_err(|error| invalid(format!("GeometryProgram CAS is invalid: {error}")))?;
        let program_hash = hash_geometry_program_with_runtime_worker(&program)
            .map_err(|error| invalid(format!("GeometryProgram validation failed: {error}")))?;
        if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
            || program.get("project_id").and_then(Value::as_str) != Some(project_id)
            || program_hash.get("canonical_sha256").and_then(Value::as_str)
                != Some(evidence.geometry_program_sha256.as_str())
        {
            return Err(invalid("GeometryProgram provenance differs"));
        }
        let program_nodes = program
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("GeometryProgram nodes are unavailable"))?;

        let readback: Value = serde_json::from_slice(&self.cas_read_bounded(
            &evidence.artifact_readback_object_sha256,
            MAX_DERIVED_JSON_BYTES,
        )?)
        .map_err(|error| invalid(format!("ArtifactReadback CAS is invalid: {error}")))?;
        let readback_canonical = readback
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("ArtifactReadback canonical hash is invalid"))?;
        let geometry_quality: Value = serde_json::from_slice(&self.cas_read_bounded(
            &evidence.quality_report_object_sha256,
            MAX_DERIVED_JSON_BYTES,
        )?)
        .map_err(|error| invalid(format!("GeometryQuality CAS is invalid: {error}")))?;
        let geometry_quality_canonical = geometry_quality
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("GeometryQuality canonical hash is invalid"))?;

        let mut graph = GraphBuilder::default();
        graph.node(
            "candidate",
            "candidate",
            candidate_id,
            Some("Candidate@1"),
            None,
            Some(candidate_state_sha256),
            "verified",
        )?;
        graph.node(
            "geometry-evidence",
            "geometry-evidence",
            "Geometry candidate evidence",
            Some("GeometryCandidateEvidence@1"),
            None,
            Some(&evidence.canonical_sha256),
            "verified",
        )?;
        graph.edge("candidate", "geometry-evidence", "binds")?;
        graph.node(
            "geometry-program",
            "geometry-program",
            "GeometryProgram@2",
            Some("GeometryProgram@2"),
            Some(&evidence.geometry_program_object_sha256),
            Some(&evidence.geometry_program_sha256),
            "verified",
        )?;
        graph.edge("geometry-evidence", "geometry-program", "binds")?;

        let mut operator_ids = BTreeMap::<String, String>::new();
        for (index, node) in program_nodes.iter().enumerate() {
            let node_id = node
                .get("node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("GeometryProgram node id is invalid"))?;
            let operator_id = node
                .get("operator_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("GeometryProgram operator id is invalid"))?;
            let graph_id = format!("operator:{}", index + 1);
            if operator_ids
                .insert(node_id.to_owned(), graph_id.clone())
                .is_some()
            {
                return Err(invalid("GeometryProgram node id is duplicated"));
            }
            graph.node(
                &graph_id,
                "operator-node",
                format!("{node_id} · {operator_id}"),
                None,
                None,
                Some(&canonical_json_hash(node)),
                "structural_only",
            )?;
            graph.edge("geometry-program", &graph_id, "contains")?;
        }
        for node in program_nodes {
            let to = operator_ids
                .get(
                    node.get("node_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .ok_or_else(|| invalid("GeometryProgram node map changed"))?;
            let inputs = node
                .get("inputs")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("GeometryProgram inputs are invalid"))?;
            for input in inputs {
                let from = operator_ids
                    .get(input.as_str().unwrap_or_default())
                    .ok_or_else(|| invalid("GeometryProgram dependency is dangling"))?;
                graph.edge(from, to, "feeds")?;
            }
        }

        graph.node(
            "artifact",
            "artifact",
            "Candidate GLB",
            None,
            Some(artifact_id),
            None,
            "verified",
        )?;
        graph.edge("geometry-program", "artifact", "materializes")?;
        graph.node(
            "artifact-readback",
            "artifact-readback",
            "ArtifactReadback@2",
            Some("ArtifactReadback@2"),
            Some(&evidence.artifact_readback_object_sha256),
            Some(readback_canonical),
            "verified",
        )?;
        graph.edge("artifact", "artifact-readback", "readback")?;
        graph.node(
            "geometry-quality",
            "geometry-quality",
            "GeometryQualityReport@2",
            Some("GeometryQualityReport@2"),
            Some(&evidence.quality_report_object_sha256),
            Some(geometry_quality_canonical),
            "structural_only",
        )?;
        graph.edge("artifact-readback", "geometry-quality", "evaluates")?;

        if let (Some(reference_id), Some(reference_sha256)) = (
            evidence.reference_id.as_deref(),
            evidence.reference_sha256.as_deref(),
        ) {
            let reference = self
                .reference(reference_id)?
                .ok_or_else(|| invalid("candidate reference is unavailable"))?;
            graph.node(
                "reference",
                "reference",
                reference_id,
                Some("ReferenceEvidence@1"),
                Some(reference_sha256),
                Some(&reference.canonical_sha256),
                "verified",
            )?;
            graph.edge("reference", "geometry-evidence", "references")?;
        }

        let mut visual_branch = "unavailable";
        let mut unknowns = Vec::<String>::new();
        if self.store.get_visual_evidence(candidate_id)?.is_some() {
            let visual = self.visual_evidence(candidate_id)?;
            let render_set = visual
                .get("render_set")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("verified visual evidence omitted RenderSet"))?;
            let render_set_hash = visual
                .get("render_set_hash")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("RenderSet object hash is invalid"))?;
            let render_set_canonical = render_set
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("RenderSet canonical hash is invalid"))?;
            graph.node(
                "render-set",
                "render-set",
                "RenderSet@2",
                Some("RenderSet@2"),
                Some(render_set_hash),
                Some(render_set_canonical),
                "verified",
            )?;
            graph.edge("artifact", "render-set", "renders")?;
            let passes = render_set
                .get("passes")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("RenderSet pass list is invalid"))?;
            let pass_artifacts = render_set
                .get("pass_artifacts")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("RenderSet pass artifacts are invalid"))?;
            for (index, pass) in passes.iter().enumerate() {
                let pass = pass
                    .as_str()
                    .ok_or_else(|| invalid("RenderSet pass name is invalid"))?;
                let pass_hash = pass_artifacts
                    .get(pass)
                    .and_then(|value| value.get("sha256"))
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| invalid("RenderSet pass hash is invalid"))?;
                let graph_id = format!("render-pass:{}", index + 1);
                graph.node(
                    &graph_id,
                    "render-pass",
                    pass,
                    None,
                    Some(pass_hash),
                    None,
                    "verified",
                )?;
                graph.edge("render-set", &graph_id, "contains-pass")?;
            }
            let comparison = visual
                .get("comparison_report")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("verified visual evidence omitted comparison report"))?;
            let comparison_hash = visual
                .get("comparison_report_hash")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("comparison object hash is invalid"))?;
            let comparison_canonical = comparison
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("comparison canonical hash is invalid"))?;
            graph.node(
                "comparison",
                "comparison-report",
                "ReferenceComparisonReport@1",
                Some("ReferenceComparisonReport@1"),
                Some(comparison_hash),
                Some(comparison_canonical),
                "verified",
            )?;
            graph.edge("render-set", "comparison", "compares")?;
            if graph.node_ids.contains("reference") {
                graph.edge("reference", "comparison", "compares")?;
            }
            let quality = visual
                .get("quality_report")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("verified visual evidence omitted QualityReport"))?;
            let quality_object_hash = visual
                .get("quality_report_hash")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("visual quality object hash is invalid"))?;
            let quality_canonical = quality
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("visual quality canonical hash is invalid"))?;
            let visual_status = quality.get("visual_status").and_then(Value::as_str);
            // This projection carries one active ReferenceComparisonReport,
            // not a complete CrossViewEvidenceBundle. It therefore cannot
            // promote HQ_360 regardless of the visible-view score.
            unknowns.push("hq-360-blocked-reference-coverage".to_owned());
            let node_status =
                if quality.get("hard_gate_passed").and_then(Value::as_bool) == Some(true) {
                    "quality_passed"
                } else {
                    match visual_status {
                        Some("BLOCKED_REFERENCE_COVERAGE") => "blocked",
                        Some("not-run") | None => "not_run",
                        _ => "quality_target_not_met",
                    }
                };
            graph.node(
                "visual-quality",
                "visual-quality",
                visual_status.unwrap_or("not-run"),
                Some("QualityReport@2"),
                Some(quality_object_hash),
                Some(quality_canonical),
                node_status,
            )?;
            graph.edge("comparison", "visual-quality", "summarizes")?;
            visual_branch = "verified";
        } else {
            unknowns.push("visual-evidence-unavailable".to_owned());
        }

        let animation_request = {
            let mut value = json!({
                "schema_version":"MechanicalAnimationClipInventoryRequest@1",
                "project_id":project_id,
                "candidate_id":candidate_id,
                "artifact_id":artifact_id,
                "max_clips":16,
            });
            let hash = canonical_json_hash(&value);
            value["canonical_sha256"] = Value::String(hash);
            value
        };
        let animation_inventory = self.mechanical_animation_clip_inventory(&animation_request)?;
        let clips = animation_inventory
            .get("clips")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("mechanical animation inventory is invalid"))?;
        let animation_branch = if clips.is_empty() {
            unknowns.push("mechanical-animation-unavailable".to_owned());
            "unavailable"
        } else {
            for (index, clip) in clips.iter().enumerate() {
                let clip_id = clip
                    .get("clip_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("mechanical animation clip id is invalid"))?;
                let clip_object_sha256 = clip
                    .get("clip_object_sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| invalid("mechanical animation clip object hash is invalid"))?;
                let clip_sha256 = clip
                    .get("clip_sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| {
                        invalid("mechanical animation clip canonical hash is invalid")
                    })?;
                let graph_id = format!("animation:{}", index + 1);
                graph.node(
                    &graph_id,
                    "mechanical-animation-clip",
                    clip_id,
                    Some("MechanicalAnimationClip@1"),
                    Some(clip_object_sha256),
                    Some(clip_sha256),
                    "structural_only",
                )?;
                graph.edge("artifact", &graph_id, "animates")?;
            }
            "verified"
        };

        let node_count = graph.nodes.len();
        let edge_count = graph.edges.len();
        let mut response = json!({
            "schema_version":RESPONSE_SCHEMA,
            "status":"Ready",
            "read_only":true,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "artifact_id":artifact_id,
            "geometry_candidate_evidence_sha256":evidence.canonical_sha256,
            "max_nodes":MAX_NODES,
            "max_edges":MAX_EDGES,
            "complete":true,
            "truncated":false,
            "branch_status":{
                "geometry":"verified",
                "visual":visual_branch,
                "animation":animation_branch,
            },
            "omitted_kinds":[
                "modifier-apply-history",
                "boolean-preview-history",
                "subdivision-sidecar-history",
                "design-session-history"
            ],
            "unknowns":unknowns,
            "node_count":node_count,
            "edge_count":edge_count,
            "nodes":graph.nodes,
            "edges":graph.edges,
            "quality_status":"structural_only",
            "limitations":[
                "read-only-candidate-bound-provenance-projection",
                "missing-optional-branches-are-explicitly-unavailable",
                "complete-or-fail-no-silent-truncation",
                "structural-evidence-does-not-prove-visual-quality",
                "hq-360-remains-blocked-without-complete-cross-view-evidence",
                "not-blender-dependency-graph-or-python-runtime-parity"
            ],
        });
        let canonical = canonical_json_hash(&response);
        response["canonical_sha256"] = Value::String(canonical);
        if canonical_json_bytes(&response)
            .map_err(|error| invalid(error.to_string()))?
            .len()
            > MAX_RESPONSE_BYTES
        {
            return Err(invalid("response exceeds 1 MiB"));
        }
        Ok(response)
    }
}
