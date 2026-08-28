#!/usr/bin/env python3
"""Focused positive and fail-closed tests for form-review confirmation."""

from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from test_validate_fps_form_review_proposal import proposal  # noqa: E402
from validate_fps_form_review_confirmation import canonical_sha256, line_flow_mapping, validate_confirmation  # noqa: E402


def confirmation(source: dict) -> dict:
    mapping = line_flow_mapping(source)
    ids = [row["line_flow_id"] for row in mapping]
    points = [[0.25, 0.64], [0.27, 0.56], [0.29, 0.52], [0.31, 0.53], [0.33, 0.58], [0.31, 0.72], [0.29, 0.78], [0.27, 0.77], [0.25, 0.70]]
    return {
        "schema_version": "ForgeCADWeaponFormReviewConfirmation@1",
        "status": "USER_CONFIRMED_2D_REVIEW_INPUT",
        "source_reference_sha256": source["source_png_sha256"],
        "source_proposal_file_sha256": "3" * 64,
        "source_proposal_canonical_sha256": canonical_sha256(source),
        "source_proposal_overlay_sha256": source["overlay_sha256"],
        "confirmation_scope": "visible-2d-reference-annotations-only",
        "user_confirmed": True,
        "line_flow_confirmation": {
            "accepted_line_flow_ids": ids,
            "accepted_mapping_count": len(mapping),
            "accepted_mapping_canonical_sha256": canonical_sha256(mapping),
            "mapping_semantics": "visual-only-nonfunctional-depth-unknown",
            "user_confirmed": True,
        },
        "outer_contour_correction": {
            "view_kind": "rear-three-quarter",
            "coordinate_space": "normalized_expanded_reference_crop",
            "source_board_size": [1491, 1055],
            "source_crop_box_xyxy": [883, 676, 1460, 903],
            "source_crop_size": [577, 227],
            "source_crop_sha256": "4" * 64,
            "runtime_crop_png_sha256": "7" * 64,
            "contour_points": [[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]],
            "contour_bbox": [0.1, 0.1, 0.9, 0.9],
            "contour_source": "codex-designed-deterministic-largest-visible-foreground-boundary-user-delegated",
            "depth_status": "UNKNOWN",
            "user_confirmed": True,
        },
        "negative_space_correction": {
            "structure_id": "rear3q.open-stock-void",
            "view_kind": "rear-three-quarter",
            "visual_role": "open-frame",
            "mask_operation": "subtract",
            "boundary_relationship": "enclosed",
            "visibility": "observed",
            "depth_policy": "unknown",
            "profile_policy": "material-only",
            "coordinate_space": "normalized_expanded_reference_crop",
            "source_board_size": [1491, 1055],
            "source_crop_box_xyxy": [883, 676, 1460, 903],
            "source_crop_size": [577, 227],
            "source_crop_sha256": "4" * 64,
            "runtime_crop_png_sha256": "7" * 64,
            "source_overlay_sha256": "5" * 64,
            "source_mask_sha256": "6" * 64,
            "contour_points": points,
            "contour_bbox": [0.25, 0.52, 0.33, 0.78],
            "contour_source": "codex-designed-conservative-inset-user-delegated",
            "containment_status": "PENDING_RUNTIME_TARGET_CONTAINMENT_VALIDATION",
            "user_confirmed": True,
        },
        "runtime_write": False,
        "worker_started": False,
        "candidate_match_status": "NOT_RUN",
        "depth_status": "UNKNOWN",
        "visual_quality_status": "NOT_PROVEN",
        "human_visual_review_status": "NOT_RUN",
    }


class FormReviewConfirmationValidatorTests(unittest.TestCase):
    def test_accepts_exact_mapping_and_bounded_2d_correction(self) -> None:
        source = proposal()
        result = validate_confirmation(source, confirmation(source), "3" * 64)
        self.assertEqual(result["status"], "READY_FOR_RUNTIME_TARGET_PREPARE")
        self.assertEqual(result["accepted_line_flow_count"], 6)
        self.assertFalse(result["runtime_write"])

    def test_rejects_mapping_drift_or_partial_confirmation(self) -> None:
        source = proposal()
        drifted = confirmation(source)
        drifted["line_flow_confirmation"]["accepted_line_flow_ids"].pop()
        with self.assertRaisesRegex(ValueError, "exactly match proposal order"):
            validate_confirmation(source, drifted, "3" * 64)

        wrong_hash = confirmation(source)
        wrong_hash["line_flow_confirmation"]["accepted_mapping_canonical_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "mapping canonical"):
            validate_confirmation(source, wrong_hash, "3" * 64)

    def test_rejects_bad_polygon_depth_or_crop_binding(self) -> None:
        source = proposal()
        crossed = confirmation(source)
        crossed["negative_space_correction"]["contour_points"] = [[0.2, 0.2], [0.8, 0.8], [0.2, 0.8], [0.8, 0.2]]
        crossed["negative_space_correction"]["contour_bbox"] = [0.2, 0.2, 0.8, 0.8]
        with self.assertRaisesRegex(ValueError, "self-intersects"):
            validate_confirmation(source, crossed, "3" * 64)

        depth = confirmation(source)
        depth["negative_space_correction"]["depth_policy"] = "bounded-inference"
        with self.assertRaisesRegex(ValueError, "depth_policy must be unknown"):
            validate_confirmation(source, depth, "3" * 64)

        crop = confirmation(source)
        crop["negative_space_correction"]["source_crop_size"] = [577, 160]
        with self.assertRaisesRegex(ValueError, "expanded crop binding differs"):
            validate_confirmation(source, crop, "3" * 64)

        outside = confirmation(source)
        outside["negative_space_correction"]["contour_points"] = [[0.05, 0.05], [0.2, 0.05], [0.2, 0.2], [0.05, 0.2]]
        outside["negative_space_correction"]["contour_bbox"] = [0.05, 0.05, 0.2, 0.2]
        with self.assertRaisesRegex(ValueError, "not strictly inside"):
            validate_confirmation(source, outside, "3" * 64)

    def test_rejects_quality_promotion_or_runtime_write(self) -> None:
        source = proposal()
        promoted = confirmation(source)
        promoted["visual_quality_status"] = "PASS"
        with self.assertRaisesRegex(ValueError, "cannot claim visual"):
            validate_confirmation(source, promoted, "3" * 64)

        write = confirmation(source)
        write["runtime_write"] = True
        with self.assertRaisesRegex(ValueError, "runtime_write must be false"):
            validate_confirmation(source, write, "3" * 64)


if __name__ == "__main__":
    unittest.main()
