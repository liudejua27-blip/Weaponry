//! Bounded reference-to-candidate view fitting.
//!
//! This module deliberately solves only a reviewed *discrete capture view*.
//! It compares the sealed reference object's observed 2D bounds with bounds
//! measured from the same candidate GLB's GPU silhouette/Part-ID capture. It
//! does not estimate unconstrained pose, reconstruct hidden surfaces, or make
//! a texture projection valid by itself. Callers must preserve the exact
//! sealed evidence and capture lineage before a resulting hypothesis can be
//! used in an [`AppearanceEvidenceBundle`].

use crate::{
    CameraParameterSource, CoreError, CoreResult, ReferenceCameraHypothesis,
    ReferenceProjectionType, REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION,
};

/// The lowest permitted intersection-over-union for a discrete view fit.
pub const MIN_SILHOUETTE_IOU_BPS: u16 = 7_000;
/// The greatest permitted Manhattan displacement between region centres.
pub const MAX_SILHOUETTE_CENTER_ERROR_PER_MILLE: u16 = 160;
/// A bounds-only fit never claims more than bounded, medium confidence.
pub const MAX_SILHOUETTE_FIT_CONFIDENCE_BPS: u16 = 8_500;
/// A profile is sixteen bounded foreground-occupancy samples. The profile is
/// a stronger tie-breaker than a box, but remains deliberately coarse and
/// cannot claim pixel-level visual similarity.
pub const SILHOUETTE_PROFILE_BUCKET_COUNT: usize = 16;
pub const MAX_SILHOUETTE_PROFILE_ERROR_PER_MILLE: u16 = 360;

/// A reference region produced from sealed evidence after the feature was
/// explicitly classified as observed. Coordinates are `[left, top, right,
/// bottom]` in image per-mille, so the implementation remains deterministic
/// and independent from image decoding libraries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceViewRegion {
    pub evidence_id: String,
    pub view_id: Option<String>,
    pub bounds_per_mille: [u16; 4],
    pub silhouette_profile_per_mille: Option<Vec<u16>>,
    pub observed_feature_ids: Vec<String>,
}

/// Bounds measured from one known GPU capture pose of the candidate GLB.
/// `view_id` must identify the exact capture manifest entry, not a free-form
/// name supplied by a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCameraSilhouette {
    pub view_id: String,
    pub projection_type: ReferenceProjectionType,
    pub vertical_fov_millidegrees: Option<u32>,
    pub bounds_per_mille: [u16; 4],
    pub silhouette_profile_per_mille: Option<Vec<u16>>,
    pub landmark_feature_ids: Vec<String>,
}

/// Selects one known candidate capture view when its silhouette region is a
/// bounded match for the sealed reference region. It rejects rather than
/// inventing an unconstrained camera solve.
pub fn fit_reference_camera_from_view_regions(
    reference: &ReferenceViewRegion,
    candidates: &[CandidateCameraSilhouette],
) -> CoreResult<ReferenceCameraHypothesis> {
    validate_reference(reference)?;
    if candidates.is_empty() {
        return Err(invalid(
            "REFERENCE_CAMERA_FIT_UNAVAILABLE",
            "A silhouette fit requires at least one reviewed candidate GPU capture view.",
        ));
    }

    let mut best: Option<(&CandidateCameraSilhouette, u16, u16, u16)> = None;
    for candidate in candidates {
        validate_candidate(candidate)?;
        let iou_bps = bounds_iou_bps(reference.bounds_per_mille, candidate.bounds_per_mille);
        let center_error =
            bounds_center_error(reference.bounds_per_mille, candidate.bounds_per_mille);
        let profile_error = silhouette_profile_error(
            reference.silhouette_profile_per_mille.as_deref(),
            candidate.silhouette_profile_per_mille.as_deref(),
        );
        if iou_bps < MIN_SILHOUETTE_IOU_BPS || center_error > MAX_SILHOUETTE_CENTER_ERROR_PER_MILLE
            || profile_error.is_some_and(|error| error > MAX_SILHOUETTE_PROFILE_ERROR_PER_MILLE)
        {
            continue;
        }

        let replace = match best {
            None => true,
            Some((current, current_iou, current_error, current_profile_error)) => {
                iou_bps > current_iou
                    || (iou_bps == current_iou
                        && (center_error < current_error
                            || (center_error == current_error
                                && (profile_error.unwrap_or(u16::MAX) < current_profile_error
                                    || (profile_error.unwrap_or(u16::MAX) == current_profile_error
                                        && candidate.view_id < current.view_id)))))
            }
        };
        if replace {
            best = Some((
                candidate,
                iou_bps,
                center_error,
                profile_error.unwrap_or(u16::MAX),
            ));
        }
    }

    let (candidate, iou_bps, center_error, _) = best.ok_or_else(|| {
        invalid(
            "REFERENCE_CAMERA_FIT_REJECTED",
            "No reviewed candidate GPU capture view met the silhouette overlap, centre-error and available occupancy-profile thresholds.",
        )
    })?;
    let alignment_bps = 10_000u16.saturating_sub(center_error.saturating_mul(10));
    let confidence_bps = ((u32::from(iou_bps) * u32::from(alignment_bps)) / 10_000)
        .min(u32::from(MAX_SILHOUETTE_FIT_CONFIDENCE_BPS)) as u16;
    let landmark_feature_ids = shared_feature_ids(reference, candidate);

    Ok(ReferenceCameraHypothesis {
        schema_version: REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION.to_owned(),
        hypothesis_id: format!(
            "silhouette-fit:{}:{}",
            reference.evidence_id, candidate.view_id
        ),
        evidence_id: reference.evidence_id.clone(),
        view_id: Some(candidate.view_id.clone()),
        projection_type: candidate.projection_type,
        parameter_source: CameraParameterSource::SilhouetteFit,
        vertical_fov_millidegrees: candidate.vertical_fov_millidegrees,
        reprojection_error_bps: Some(10_000u16.saturating_sub(iou_bps)),
        landmark_feature_ids,
        confidence_bps,
        unresolved_fields: Vec::new(),
    })
}

fn validate_reference(reference: &ReferenceViewRegion) -> CoreResult<()> {
    if reference.evidence_id.trim().is_empty()
        || reference.observed_feature_ids.is_empty()
        || reference
            .observed_feature_ids
            .iter()
            .any(|feature_id| feature_id.trim().is_empty())
        || !valid_bounds(reference.bounds_per_mille)
        || !valid_profile(reference.silhouette_profile_per_mille.as_deref())
    {
        return Err(invalid(
            "REFERENCE_CAMERA_FIT_REFERENCE_INVALID",
            "A silhouette fit requires a sealed evidence ID, observed feature IDs and bounded reference region.",
        ));
    }
    Ok(())
}

fn validate_candidate(candidate: &CandidateCameraSilhouette) -> CoreResult<()> {
    let perspective_fov_valid = candidate.projection_type != ReferenceProjectionType::Perspective
        || candidate
            .vertical_fov_millidegrees
            .is_some_and(|fov| (1..180_000).contains(&fov));
    if candidate.view_id.trim().is_empty()
        || candidate.projection_type == ReferenceProjectionType::Unknown
        || !perspective_fov_valid
        || !valid_bounds(candidate.bounds_per_mille)
        || !valid_profile(candidate.silhouette_profile_per_mille.as_deref())
    {
        return Err(invalid(
            "REFERENCE_CAMERA_FIT_CANDIDATE_INVALID",
            "A candidate fit view must be a known projection with bounded GPU-measured silhouette bounds.",
        ));
    }
    Ok(())
}

fn valid_profile(profile: Option<&[u16]>) -> bool {
    profile.map_or(true, |values| {
        values.len() == SILHOUETTE_PROFILE_BUCKET_COUNT
            && values.iter().all(|value| *value <= 1_000)
    })
}

fn silhouette_profile_error(reference: Option<&[u16]>, candidate: Option<&[u16]>) -> Option<u16> {
    let (Some(reference), Some(candidate)) = (reference, candidate) else {
        return None;
    };
    if reference.len() != SILHOUETTE_PROFILE_BUCKET_COUNT
        || candidate.len() != SILHOUETTE_PROFILE_BUCKET_COUNT
    {
        return None;
    }
    Some(
        (reference
            .iter()
            .zip(candidate)
            .map(|(left, right)| left.abs_diff(*right) as u32)
            .sum::<u32>()
            / SILHOUETTE_PROFILE_BUCKET_COUNT as u32) as u16,
    )
}

fn valid_bounds(bounds: [u16; 4]) -> bool {
    bounds[0] < bounds[2] && bounds[1] < bounds[3] && bounds[2] <= 1_000 && bounds[3] <= 1_000
}

fn bounds_iou_bps(left: [u16; 4], right: [u16; 4]) -> u16 {
    let intersection_width = left[2].min(right[2]).saturating_sub(left[0].max(right[0])) as u32;
    let intersection_height = left[3].min(right[3]).saturating_sub(left[1].max(right[1])) as u32;
    let intersection = u64::from(intersection_width) * u64::from(intersection_height);
    let left_area = u64::from(left[2] - left[0]) * u64::from(left[3] - left[1]);
    let right_area = u64::from(right[2] - right[0]) * u64::from(right[3] - right[1]);
    let union = left_area + right_area - intersection;
    ((intersection * 10_000) / union.max(1)) as u16
}

fn bounds_center_error(left: [u16; 4], right: [u16; 4]) -> u16 {
    let left_x2 = left[0] as i32 + left[2] as i32;
    let left_y2 = left[1] as i32 + left[3] as i32;
    let right_x2 = right[0] as i32 + right[2] as i32;
    let right_y2 = right[1] as i32 + right[3] as i32;
    (((left_x2 - right_x2).unsigned_abs() + (left_y2 - right_y2).unsigned_abs()) / 2) as u16
}

fn shared_feature_ids(
    reference: &ReferenceViewRegion,
    candidate: &CandidateCameraSilhouette,
) -> Vec<String> {
    let mut ids = reference
        .observed_feature_ids
        .iter()
        .filter(|feature_id| candidate.landmark_feature_ids.contains(*feature_id))
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        let mut observed = reference.observed_feature_ids.clone();
        observed.sort();
        observed.dedup();
        observed
    } else {
        ids
    }
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(bounds_per_mille: [u16; 4]) -> ReferenceViewRegion {
        ReferenceViewRegion {
            evidence_id: "evidence-robot".to_owned(),
            view_id: Some("reference-front".to_owned()),
            bounds_per_mille,
            silhouette_profile_per_mille: None,
            observed_feature_ids: vec!["feature-silhouette".to_owned(), "feature-visor".to_owned()],
        }
    }

    fn candidate(view_id: &str, bounds_per_mille: [u16; 4]) -> CandidateCameraSilhouette {
        CandidateCameraSilhouette {
            view_id: view_id.to_owned(),
            projection_type: ReferenceProjectionType::Perspective,
            vertical_fov_millidegrees: Some(42_000),
            bounds_per_mille,
            silhouette_profile_per_mille: None,
            landmark_feature_ids: vec!["feature-silhouette".to_owned()],
        }
    }

    #[test]
    fn accepts_close_gpu_capture_as_bounded_silhouette_fit() {
        let fit = fit_reference_camera_from_view_regions(
            &reference([180, 80, 800, 940]),
            &[candidate("front", [185, 82, 803, 942])],
        )
        .expect("close bounds should fit");

        assert_eq!(fit.parameter_source, CameraParameterSource::SilhouetteFit);
        assert_eq!(fit.view_id.as_deref(), Some("front"));
        assert_eq!(fit.projection_type, ReferenceProjectionType::Perspective);
        assert!(fit.confidence_bps > 7_000);
        assert!(fit.unresolved_fields.is_empty());
        assert_eq!(fit.landmark_feature_ids, vec!["feature-silhouette"]);
    }

    #[test]
    fn uses_view_id_as_stable_tiebreaker() {
        let fit = fit_reference_camera_from_view_regions(
            &reference([200, 100, 800, 900]),
            &[
                candidate("right", [205, 105, 805, 905]),
                candidate("left", [205, 105, 805, 905]),
            ],
        )
        .expect("both candidates are valid fits");

        assert_eq!(fit.view_id.as_deref(), Some("left"));
    }

    #[test]
    fn rejects_candidate_when_bounds_do_not_match() {
        let error = fit_reference_camera_from_view_regions(
            &reference([180, 80, 800, 940]),
            &[candidate("rear", [0, 0, 200, 200])],
        )
        .expect_err("distant silhouette must not become a camera solve");

        assert_eq!(error.code(), "REFERENCE_CAMERA_FIT_REJECTED");
    }

    #[test]
    fn rejects_unbounded_reference_and_unknown_candidate_projection() {
        let error = fit_reference_camera_from_view_regions(
            &reference([800, 80, 800, 940]),
            &[candidate("front", [180, 80, 800, 940])],
        )
        .expect_err("empty reference region is invalid");
        assert_eq!(error.code(), "REFERENCE_CAMERA_FIT_REFERENCE_INVALID");

        let mut unknown = candidate("front", [180, 80, 800, 940]);
        unknown.projection_type = ReferenceProjectionType::Unknown;
        let error =
            fit_reference_camera_from_view_regions(&reference([180, 80, 800, 940]), &[unknown])
                .expect_err("unknown capture projection is not a fit candidate");
        assert_eq!(error.code(), "REFERENCE_CAMERA_FIT_CANDIDATE_INVALID");
    }

    #[test]
    fn rejects_profile_mismatch_even_when_bounds_match() {
        let reference = ReferenceViewRegion {
            silhouette_profile_per_mille: Some(vec![100; SILHOUETTE_PROFILE_BUCKET_COUNT]),
            ..reference([180, 80, 800, 940])
        };
        let candidate = CandidateCameraSilhouette {
            silhouette_profile_per_mille: Some(vec![1_000; SILHOUETTE_PROFILE_BUCKET_COUNT]),
            ..candidate("front", [180, 80, 800, 940])
        };
        let error = fit_reference_camera_from_view_regions(&reference, &[candidate])
            .expect_err("a box match must not hide a silhouette profile mismatch");
        assert_eq!(error.code(), "REFERENCE_CAMERA_FIT_REJECTED");
    }

    #[test]
    fn profile_is_optional_for_legacy_fixtures_but_validated_when_present() {
        let mut candidate = candidate("front", [180, 80, 800, 940]);
        candidate.silhouette_profile_per_mille = Some(vec![100; 3]);
        let error = fit_reference_camera_from_view_regions(&reference([180, 80, 800, 940]), &[candidate])
            .expect_err("a malformed optional profile must fail closed");
        assert_eq!(error.code(), "REFERENCE_CAMERA_FIT_CANDIDATE_INVALID");
    }
}
