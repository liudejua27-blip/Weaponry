//! Rust-owned contract for transient candidate PBR capture.
//!
//! A capture session is deliberately not a preview, a quality report, or an
//! asset version. It binds the exact compiled GLB/readback to a bounded
//! render plan so a desktop PBR runner can submit visual evidence before a
//! `SingleResultDecision@1` is allowed to exist.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{semantic_sha256, CoreError, CoreResult};

pub const CANDIDATE_PBR_CAPTURE_SESSION_SCHEMA_VERSION: &str = "CandidatePbrCaptureSession@1";
pub const CANDIDATE_PBR_CAPTURE_EVIDENCE_SCHEMA_VERSION: &str = "CandidatePbrCaptureEvidence@1";
pub const WORKBENCH_PBR_RENDERER_ID: &str = "forgecad-workbench-pbr@1";
pub const CANDIDATE_PBR_RENDERER_ID: &str = "forgecad-candidate-pbr-runner@1";
pub const TURN_TABLE_EIGHT_VIEW_IDS: [&str; 8] = [
    "turntable_000",
    "turntable_045",
    "turntable_090",
    "turntable_135",
    "turntable_180",
    "turntable_225",
    "turntable_270",
    "turntable_315",
];
pub const MAX_CAPTURE_VIEW_BYTES: u64 = 12 * 1024 * 1024;
/// Five deterministic GPU diagnostic passes are packed into one contact-sheet
/// PNG per turntable view. They are not sent to the vision Provider, but they
/// prove that the same workbench renderer emitted silhouette/normal/depth and
/// stable part/material-ID evidence alongside the user-visible PBR beauty.
pub const MAX_CAPTURE_AUXILIARY_VIEW_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_CAPTURE_TOTAL_BYTES: u64 = (MAX_CAPTURE_VIEW_BYTES
    + MAX_CAPTURE_AUXILIARY_VIEW_BYTES)
    * TURN_TABLE_EIGHT_VIEW_IDS.len() as u64;
pub const MAX_CAPTURE_TTL_MS: u64 = 180_000;
/// Every visual-comparison image is rendered at this exact physical canvas
/// size. A resizeable workbench remains free for the user; the candidate
/// capture briefly uses the same renderer at this fixed evidence resolution.
pub const WORKBENCH_PBR_CAPTURE_WIDTH_PX: u32 = 640;
pub const WORKBENCH_PBR_CAPTURE_HEIGHT_PX: u32 = 640;
pub const WORKBENCH_PBR_AUXILIARY_PASS_WIDTH_PX: u32 = 320;
pub const WORKBENCH_PBR_AUXILIARY_PASS_HEIGHT_PX: u32 = 320;
pub const WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX: u32 = WORKBENCH_PBR_AUXILIARY_PASS_WIDTH_PX * 3;
pub const WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX: u32 =
    WORKBENCH_PBR_AUXILIARY_PASS_HEIGHT_PX * 2;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidatePbrCaptureSession {
    pub schema_version: String,
    pub session_id: String,
    pub project_id: String,
    pub turn_id: String,
    pub candidate_glb_sha256: String,
    pub shape_program_sha256: String,
    pub compile_readback_sha256: String,
    pub artifact_profile_id: String,
    pub render_manifest_sha256: String,
    pub expected_renderer_id: String,
    pub capture_nonce: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub required_view_ids: Vec<String>,
    /// Rust derives an immutable camera binding for every required view from
    /// the exact compiled GLB bounds. Browser camera matrices remain useful
    /// audit evidence, but cannot authorize reference-to-UV projection.
    pub projection_camera_binding_sha256_by_view_id: BTreeMap<String, String>,
    pub max_view_bytes: u64,
    pub max_total_bytes: u64,
    pub capture_width_px: u32,
    pub capture_height_px: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidatePbrCapturedView {
    pub view_id: String,
    pub glb_sha256: String,
    pub renderer_id: String,
    pub render_manifest_sha256: String,
    pub camera_pose_sha256: String,
    pub projection_camera_binding_sha256: String,
    pub image_sha256: String,
    pub byte_size: u64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub auxiliary_capture_sha256: String,
    pub auxiliary_byte_size: u64,
    pub auxiliary_pixel_width: u32,
    pub auxiliary_pixel_height: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidatePbrCaptureSubmission {
    pub schema_version: String,
    pub session_id: String,
    pub capture_nonce: String,
    pub candidate_glb_sha256: String,
    pub renderer_id: String,
    pub render_manifest_sha256: String,
    pub views: Vec<CandidatePbrCapturedView>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidatePbrCaptureEvidence {
    pub schema_version: String,
    pub session_id: String,
    pub candidate_glb_sha256: String,
    pub renderer_id: String,
    pub render_manifest_sha256: String,
    pub capture_sha256: String,
    pub views: Vec<CandidatePbrCapturedView>,
}

impl CandidatePbrCaptureSession {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != CANDIDATE_PBR_CAPTURE_SESSION_SCHEMA_VERSION
            || !stable_id(&self.session_id)
            || !self.session_id.starts_with("pbrcapture_")
            || !stable_id(&self.project_id)
            || !stable_id(&self.turn_id)
            || !stable_id(&self.capture_nonce)
            || self.capture_nonce.len() < 16
            // Current high-detail candidates compile with the code-owned
            // `production_concept` profile. `interactive_preview` remains
            // valid for lightweight paths, but a session must name one of
            // these exact reviewed profiles rather than an arbitrary label.
            || !matches!(self.artifact_profile_id.as_str(), "interactive_preview" | "production_concept")
            || !matches!(self.expected_renderer_id.as_str(), WORKBENCH_PBR_RENDERER_ID | CANDIDATE_PBR_RENDERER_ID)
            || self.issued_at_unix_ms >= self.expires_at_unix_ms
            || self.expires_at_unix_ms - self.issued_at_unix_ms > MAX_CAPTURE_TTL_MS
            || self.max_view_bytes == 0
            || self.max_view_bytes > MAX_CAPTURE_VIEW_BYTES
            || self.max_total_bytes == 0
            || self.max_total_bytes > MAX_CAPTURE_TOTAL_BYTES
            || self.capture_width_px != WORKBENCH_PBR_CAPTURE_WIDTH_PX
            || self.capture_height_px != WORKBENCH_PBR_CAPTURE_HEIGHT_PX
        {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SESSION_INVALID",
                "Capture session identity, renderer, expiry, or limits are invalid.",
            ));
        }
        for (field, value) in [
            ("candidate_glb_sha256", &self.candidate_glb_sha256),
            ("shape_program_sha256", &self.shape_program_sha256),
            ("compile_readback_sha256", &self.compile_readback_sha256),
            ("render_manifest_sha256", &self.render_manifest_sha256),
        ] {
            require_sha256(field, value)?;
        }
        if self.required_view_ids.len() != TURN_TABLE_EIGHT_VIEW_IDS.len()
            || self
                .required_view_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != TURN_TABLE_EIGHT_VIEW_IDS
            || self.required_view_ids.iter().collect::<BTreeSet<_>>().len()
                != self.required_view_ids.len()
        {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SESSION_INVALID",
                "Capture session must require the generic turntable eight views exactly once.",
            ));
        }
        let expected_binding_views = self
            .required_view_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let binding_views = self
            .projection_camera_binding_sha256_by_view_id
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if binding_views != expected_binding_views
            || self
                .projection_camera_binding_sha256_by_view_id
                .values()
                .any(|hash| require_sha256("projection_camera_binding_sha256", hash).is_err())
        {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SESSION_INVALID",
                "Capture session must bind exactly one Rust-owned projection camera hash per fixed view.",
            ));
        }
        Ok(())
    }

    pub fn accept(
        &self,
        submission: CandidatePbrCaptureSubmission,
        now_unix_ms: u64,
    ) -> CoreResult<CandidatePbrCaptureEvidence> {
        self.validate()?;
        if now_unix_ms > self.expires_at_unix_ms {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_EXPIRED",
                "Candidate PBR capture session has expired.",
            ));
        }
        if submission.schema_version != CANDIDATE_PBR_CAPTURE_EVIDENCE_SCHEMA_VERSION
            || submission.session_id != self.session_id
            || submission.capture_nonce != self.capture_nonce
            || submission.candidate_glb_sha256 != self.candidate_glb_sha256
            || submission.renderer_id != self.expected_renderer_id
            || submission.render_manifest_sha256 != self.render_manifest_sha256
        {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                "Capture submission does not match the Rust-issued session.",
            ));
        }
        let mut total_bytes = 0_u64;
        let mut views = submission.views;
        views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
        if views.len() != self.required_view_ids.len() {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                "Capture submission has an incomplete fixed-view set.",
            ));
        }
        let expected = self
            .required_view_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let actual = views
            .iter()
            .map(|view| view.view_id.clone())
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                "Capture submission has duplicate, missing, or unexpected view identities.",
            ));
        }
        for view in &views {
            if view.glb_sha256 != self.candidate_glb_sha256
                || view.renderer_id != self.expected_renderer_id
                || view.render_manifest_sha256 != self.render_manifest_sha256
                || view.byte_size == 0
                || view.byte_size > self.max_view_bytes
                || view.auxiliary_byte_size == 0
                || view.auxiliary_byte_size > MAX_CAPTURE_AUXILIARY_VIEW_BYTES
                || view.pixel_width != self.capture_width_px
                || view.pixel_height != self.capture_height_px
                || view.auxiliary_pixel_width != WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX
                || view.auxiliary_pixel_height != WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX
            {
                return Err(invalid("CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID", "A captured view does not match the candidate, renderer, manifest, or byte limit."));
            }
            require_sha256("camera_pose_sha256", &view.camera_pose_sha256)?;
            let expected_binding_sha256 = self
                .projection_camera_binding_sha256_by_view_id
                .get(&view.view_id)
                .ok_or_else(|| {
                    invalid(
                        "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                        "Captured view has no Rust-issued projection camera binding.",
                    )
                })?;
            if view.projection_camera_binding_sha256 != *expected_binding_sha256 {
                return Err(invalid(
                    "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                    "Captured view does not use the Rust-issued projection camera binding.",
                ));
            }
            require_sha256(
                "projection_camera_binding_sha256",
                &view.projection_camera_binding_sha256,
            )?;
            require_sha256("image_sha256", &view.image_sha256)?;
            require_sha256("auxiliary_capture_sha256", &view.auxiliary_capture_sha256)?;
            total_bytes = total_bytes
                .checked_add(view.byte_size)
                .and_then(|value| value.checked_add(view.auxiliary_byte_size))
                .ok_or_else(|| {
                    invalid(
                        "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                        "Capture byte accounting overflowed.",
                    )
                })?;
        }
        if total_bytes > self.max_total_bytes {
            return Err(invalid(
                "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID",
                "Capture evidence exceeds the session total byte limit.",
            ));
        }
        let mut evidence = CandidatePbrCaptureEvidence {
            schema_version: CANDIDATE_PBR_CAPTURE_EVIDENCE_SCHEMA_VERSION.into(),
            session_id: self.session_id.clone(),
            candidate_glb_sha256: self.candidate_glb_sha256.clone(),
            renderer_id: self.expected_renderer_id.clone(),
            render_manifest_sha256: self.render_manifest_sha256.clone(),
            capture_sha256: String::new(),
            views,
        };
        evidence.capture_sha256 = semantic_sha256(&evidence)?;
        Ok(evidence)
    }
}

fn stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn require_sha256(field: &str, value: &str) -> CoreResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(invalid(
        "CANDIDATE_PBR_CAPTURE_HASH_INVALID",
        format!("{field} must be a lowercase SHA-256."),
    ))
}

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
    }
    fn indexed_hash(index: usize) -> String {
        format!("{index:064x}")
    }

    fn session() -> CandidatePbrCaptureSession {
        CandidatePbrCaptureSession {
            schema_version: CANDIDATE_PBR_CAPTURE_SESSION_SCHEMA_VERSION.into(),
            session_id: "pbrcapture_turn_001".into(),
            project_id: "prj_001".into(),
            turn_id: "turn_001".into(),
            candidate_glb_sha256: hash('a'),
            shape_program_sha256: hash('b'),
            compile_readback_sha256: hash('c'),
            artifact_profile_id: "interactive_preview".into(),
            render_manifest_sha256: hash('d'),
            expected_renderer_id: WORKBENCH_PBR_RENDERER_ID.into(),
            capture_nonce: "nonce_0123456789abcdef".into(),
            issued_at_unix_ms: 1000,
            expires_at_unix_ms: 120_000,
            required_view_ids: TURN_TABLE_EIGHT_VIEW_IDS
                .iter()
                .map(|value| value.to_string())
                .collect(),
            projection_camera_binding_sha256_by_view_id: TURN_TABLE_EIGHT_VIEW_IDS
                .iter()
                .enumerate()
                .map(|(index, view_id)| ((*view_id).into(), indexed_hash(index + 96)))
                .collect(),
            max_view_bytes: 1024,
            max_total_bytes: 8192,
            capture_width_px: WORKBENCH_PBR_CAPTURE_WIDTH_PX,
            capture_height_px: WORKBENCH_PBR_CAPTURE_HEIGHT_PX,
        }
    }

    fn submission(session: &CandidatePbrCaptureSession) -> CandidatePbrCaptureSubmission {
        CandidatePbrCaptureSubmission {
            schema_version: CANDIDATE_PBR_CAPTURE_EVIDENCE_SCHEMA_VERSION.into(),
            session_id: session.session_id.clone(),
            capture_nonce: session.capture_nonce.clone(),
            candidate_glb_sha256: session.candidate_glb_sha256.clone(),
            renderer_id: session.expected_renderer_id.clone(),
            render_manifest_sha256: session.render_manifest_sha256.clone(),
            views: TURN_TABLE_EIGHT_VIEW_IDS
                .iter()
                .enumerate()
                .map(|(index, view_id)| CandidatePbrCapturedView {
                    view_id: (*view_id).into(),
                    glb_sha256: session.candidate_glb_sha256.clone(),
                    renderer_id: session.expected_renderer_id.clone(),
                    render_manifest_sha256: session.render_manifest_sha256.clone(),
                    camera_pose_sha256: indexed_hash(index + 1),
                    projection_camera_binding_sha256: session
                        .projection_camera_binding_sha256_by_view_id
                        .get(*view_id)
                        .unwrap()
                        .clone(),
                    image_sha256: indexed_hash(index + 32),
                    byte_size: 512,
                    pixel_width: WORKBENCH_PBR_CAPTURE_WIDTH_PX,
                    pixel_height: WORKBENCH_PBR_CAPTURE_HEIGHT_PX,
                    auxiliary_capture_sha256: indexed_hash(index + 64),
                    auxiliary_byte_size: 256,
                    auxiliary_pixel_width: WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX,
                    auxiliary_pixel_height: WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX,
                })
                .collect(),
        }
    }

    #[test]
    fn candidate_pbr_capture_accepts_only_bound_turntable_evidence() {
        let session = session();
        let evidence = session.accept(submission(&session), 10_000).unwrap();
        assert_eq!(evidence.views.len(), 8);
        assert_eq!(evidence.renderer_id, WORKBENCH_PBR_RENDERER_ID);
        assert_eq!(evidence.capture_sha256.len(), 64);
    }

    #[test]
    fn candidate_pbr_capture_fails_closed_for_replay_drift_and_expiry() {
        let session = session();
        let mut wrong_hash = submission(&session);
        wrong_hash.candidate_glb_sha256 = hash('f');
        assert_eq!(
            session.accept(wrong_hash, 10_000).unwrap_err().code(),
            "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID"
        );
        let mut duplicate = submission(&session);
        duplicate.views[1].view_id = duplicate.views[0].view_id.clone();
        assert_eq!(
            session.accept(duplicate, 10_000).unwrap_err().code(),
            "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID"
        );
        let mut wrong_dimensions = submission(&session);
        wrong_dimensions.views[0].pixel_width = WORKBENCH_PBR_CAPTURE_WIDTH_PX - 1;
        assert_eq!(
            session.accept(wrong_dimensions, 10_000).unwrap_err().code(),
            "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID"
        );
        assert_eq!(
            session
                .accept(submission(&session), 120_001)
                .unwrap_err()
                .code(),
            "CANDIDATE_PBR_CAPTURE_EXPIRED"
        );
        let mut wrong_binding = submission(&session);
        wrong_binding.views[0].projection_camera_binding_sha256 = hash('e');
        assert_eq!(
            session.accept(wrong_binding, 10_000).unwrap_err().code(),
            "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID"
        );
    }

    #[test]
    fn candidate_pbr_capture_requires_every_gpu_auxiliary_contact_sheet() {
        let session = session();
        let mut missing_auxiliary = submission(&session);
        missing_auxiliary.views[0].auxiliary_byte_size = 0;
        assert_eq!(
            session
                .accept(missing_auxiliary, 10_000)
                .unwrap_err()
                .code(),
            "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID"
        );
        let mut wrong_auxiliary_dimensions = submission(&session);
        wrong_auxiliary_dimensions.views[0].auxiliary_pixel_width = 640;
        assert_eq!(
            session
                .accept(wrong_auxiliary_dimensions, 10_000)
                .unwrap_err()
                .code(),
            "CANDIDATE_PBR_CAPTURE_SUBMISSION_INVALID"
        );
    }
}
