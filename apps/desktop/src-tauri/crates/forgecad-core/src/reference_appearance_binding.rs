//! Exact feature-to-zone admission for reference-pixel appearance baking.
//!
//! A photo bake is never selected by a provider, by a UI index, or by the
//! first image/zone that happens to exist.  This contract derives a bounded
//! mapping from an *observed* `VisualFeatureContract` requirement through a
//! same-project sealed image and into one real UAS@2 material zone.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    semantic_sha256, AppearanceChannel, CoreError, CoreResult, EvidenceStatus, ReferenceEvidence,
    ReferenceEvidenceKind, UniversalAssetSourceV2,
};

pub const REFERENCE_APPEARANCE_BINDING_SCHEMA_VERSION: &str = "ReferenceAppearanceBinding@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceAppearanceBinding {
    pub schema_version: String,
    pub source_sha256: String,
    pub evidence_id: String,
    pub evidence_sha256: String,
    /// The code-owned turntable slot used to derive the geometry camera.
    pub source_view_id: String,
    /// More than one observed feature may share an image/zone, but all are
    /// explicitly named; this is not an implicit "hero material" fallback.
    pub feature_ids: Vec<String>,
    pub target_subject_part_id: String,
    pub target_material_zone_id: String,
    pub binding_sha256: String,
}

/// Derive every unambiguous observed image-to-zone mapping.  No candidate is
/// returned for inferred/hidden/conflicting features, missing regions, unknown
/// view aliases, non-PNG evidence, or zones that have competing image views.
pub fn derive_reference_appearance_bindings(
    source: &UniversalAssetSourceV2,
    evidence: &[ReferenceEvidence],
) -> CoreResult<Vec<ReferenceAppearanceBinding>> {
    source.validate()?;
    let source_sha256 = semantic_sha256(source)?;
    let request_evidence = source
        .request
        .reference_inputs
        .iter()
        .map(|reference| {
            (
                reference.evidence_id.as_str(),
                reference.evidence_sha256.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let evidence_by_id = evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    if evidence_by_id.len() != evidence.len() {
        return Err(invalid(
            "REFERENCE_APPEARANCE_BINDING_EVIDENCE_DUPLICATE",
            "sealed reference evidence IDs must be unique before photo-bake binding",
        ));
    }

    // (part, zone, evidence, camera view) -> explicitly covered feature IDs.
    let mut grouped = BTreeMap::<(String, String, String, String), BTreeSet<String>>::new();
    for requirement in &source.visual_feature_contract.requirements {
        if requirement.evidence_status != EvidenceStatus::Observed
            || !requirement
                .channels
                .iter()
                .any(is_projectable_appearance_channel)
        {
            continue;
        }
        for region in &requirement.evidence_regions {
            let Some(view_id) = region.view_id.as_deref().and_then(turntable_view_for_hint) else {
                continue;
            };
            let Some(expected_sha) = request_evidence.get(region.evidence_id.as_str()) else {
                return Err(invalid(
                    "REFERENCE_APPEARANCE_BINDING_EVIDENCE_UNSEALED",
                    "observed appearance feature references an image outside the sealed request",
                ));
            };
            let Some(sealed) = evidence_by_id.get(region.evidence_id.as_str()) else {
                return Err(invalid(
                    "REFERENCE_APPEARANCE_BINDING_EVIDENCE_MISSING",
                    "observed appearance feature has no sealed evidence record",
                ));
            };
            sealed.validate()?;
            if sealed.project_id != source.request.project_id
                || sealed.kind != ReferenceEvidenceKind::Image
                || sealed.source_media_type != "image/png"
                || semantic_sha256(*sealed)? != *expected_sha
            {
                return Err(invalid(
                    "REFERENCE_APPEARANCE_BINDING_EVIDENCE_INVALID",
                    "appearance baking requires exact same-project sealed PNG evidence",
                ));
            }
            for target_part_id in &requirement.affected_part_ids {
                for zone in source
                    .appearance_compilation
                    .zones
                    .iter()
                    .filter(|zone| zone.target_subject_part_id == *target_part_id)
                {
                    grouped
                        .entry((
                            target_part_id.clone(),
                            zone.target_material_zone_id.clone(),
                            region.evidence_id.clone(),
                            view_id.to_string(),
                        ))
                        .or_default()
                        .insert(requirement.feature_id.clone());
                }
            }
        }
    }

    // A side/painted zone may not silently choose between different photos or
    // camera slots.  Multi-view fusion is a later explicit representation.
    let mut source_by_zone = BTreeMap::<(String, String), BTreeSet<(String, String)>>::new();
    for (part, zone, evidence_id, view_id) in grouped.keys() {
        source_by_zone
            .entry((part.clone(), zone.clone()))
            .or_default()
            .insert((evidence_id.clone(), view_id.clone()));
    }
    if source_by_zone.values().any(|sources| sources.len() > 1) {
        return Err(invalid(
            "REFERENCE_APPEARANCE_BINDING_MULTIVIEW_CONFLICT",
            "one U004 raster bake zone cannot silently merge competing reference views",
        ));
    }

    grouped
        .into_iter()
        .map(|((part, zone, evidence_id, view_id), feature_ids)| {
            let evidence_sha256 = request_evidence
                .get(evidence_id.as_str())
                .expect("grouped evidence was checked above")
                .to_string();
            let mut binding = ReferenceAppearanceBinding {
                schema_version: REFERENCE_APPEARANCE_BINDING_SCHEMA_VERSION.into(),
                source_sha256: source_sha256.clone(),
                evidence_id,
                evidence_sha256,
                source_view_id: view_id,
                feature_ids: feature_ids.into_iter().collect(),
                target_subject_part_id: part,
                target_material_zone_id: zone,
                binding_sha256: String::new(),
            };
            binding.binding_sha256 = semantic_sha256(&BindingWithoutSha::from(&binding))?;
            Ok(binding)
        })
        .collect()
}

impl ReferenceAppearanceBinding {
    pub fn validate_against(
        &self,
        source: &UniversalAssetSourceV2,
        evidence: &[ReferenceEvidence],
    ) -> CoreResult<()> {
        if self.schema_version != REFERENCE_APPEARANCE_BINDING_SCHEMA_VERSION
            || self.source_sha256 != semantic_sha256(source)?
            || self.feature_ids.is_empty()
            || self.feature_ids.len() != self.feature_ids.iter().collect::<BTreeSet<_>>().len()
            || turntable_view_for_hint(&self.source_view_id) != Some(self.source_view_id.as_str())
        {
            return Err(invalid(
                "REFERENCE_APPEARANCE_BINDING_INVALID",
                "reference appearance binding identity or stable fields are invalid",
            ));
        }
        let expected = derive_reference_appearance_bindings(source, evidence)?;
        if !expected.iter().any(|candidate| candidate == self) {
            return Err(CoreError::conflict(
                "REFERENCE_APPEARANCE_BINDING_DRIFT",
                "reference appearance binding does not match observed feature and UAS@2 lineage",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct BindingWithoutSha<'a> {
    schema_version: &'a str,
    source_sha256: &'a str,
    evidence_id: &'a str,
    evidence_sha256: &'a str,
    source_view_id: &'a str,
    feature_ids: &'a [String],
    target_subject_part_id: &'a str,
    target_material_zone_id: &'a str,
}

impl<'a> From<&'a ReferenceAppearanceBinding> for BindingWithoutSha<'a> {
    fn from(value: &'a ReferenceAppearanceBinding) -> Self {
        Self {
            schema_version: &value.schema_version,
            source_sha256: &value.source_sha256,
            evidence_id: &value.evidence_id,
            evidence_sha256: &value.evidence_sha256,
            source_view_id: &value.source_view_id,
            feature_ids: &value.feature_ids,
            target_subject_part_id: &value.target_subject_part_id,
            target_material_zone_id: &value.target_material_zone_id,
        }
    }
}

fn is_projectable_appearance_channel(channel: &AppearanceChannel) -> bool {
    matches!(
        channel,
        AppearanceChannel::BaseColor
            | AppearanceChannel::Normal
            | AppearanceChannel::Roughness
            | AppearanceChannel::Metallic
            | AppearanceChannel::Emissive
    )
}

fn turntable_view_for_hint(value: &str) -> Option<&'static str> {
    match value {
        "turntable_000" | "front" | "front_view" => Some("turntable_000"),
        "turntable_045" | "front_right" | "front_three_quarter" => Some("turntable_045"),
        "turntable_090" | "right" | "right_view" => Some("turntable_090"),
        "turntable_135" | "rear_right" => Some("turntable_135"),
        "turntable_180" | "back" | "rear" => Some("turntable_180"),
        "turntable_225" | "rear_left" => Some("turntable_225"),
        "turntable_270" | "left" | "left_view" => Some("turntable_270"),
        "turntable_315" | "front_left" => Some("turntable_315"),
        _ => None,
    }
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}
