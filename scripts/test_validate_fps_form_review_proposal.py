#!/usr/bin/env python3
"""Focused positive and fail-closed tests for the FPS form review validator."""

from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))
from validate_fps_form_review_proposal import VIEW_ORDER, validate_proposal  # noqa: E402


def polygon(x0: float, y0: float, x1: float, y1: float) -> list[list[float]]:
    return [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]


def proposal() -> dict:
    views = {}
    for view in VIEW_ORDER:
        negative_regions = []
        negative_status = "not-applicable-zero-rows-unconfirmed-proposal"
        if view in {"left", "right", "rear-three-quarter"}:
            negative_status = "CLOSED_POLYGON_PROPOSAL_NOT_AUTHORITY"
            negative_regions = [{
                "structure_id": f"{view}.trigger-void",
                "visual_role": "open-frame",
                "mask_operation": "subtract",
                "bbox": [0.4, 0.4, 0.6, 0.6],
                "source_crop_coordinate_space": "normalized_crop_pixels",
                "target_coordinate_space": "normalized_aspect_fit_512",
                "cross_space_containment_status": "not_evaluated_missing_transform",
                "boundary_relationship": "enclosed",
                "closed_contour_points": polygon(0.45, 0.45, 0.55, 0.55),
                "contour_status": "CLOSED_POLYGON_PROPOSAL_NOT_AUTHORITY",
                "runtime_visibility": "unknown",
                "requires_user_confirmation": True,
                "user_confirmed": False,
            }]
        views[view] = {
            "view_kind": view,
            "user_confirmed": False,
            "outer_contour_points": polygon(0.1, 0.1, 0.9, 0.9),
            "landmarks": [{
                "landmark_id": f"{view}.landmark",
                "point": [0.5, 0.5],
                "runtime_visibility_before_confirmation": "unknown",
                "user_confirmed": False,
            }],
            "part_regions_v3": [{
                "structure_id": f"{view}.part",
                "bbox": [0.2, 0.2, 0.8, 0.8],
                "source_crop_coordinate_space": "normalized_crop_pixels",
                "target_coordinate_space": "normalized_aspect_fit_512",
                "cross_space_containment_status": "not_evaluated_missing_transform",
                "closed_contour_points": polygon(0.3, 0.3, 0.7, 0.7),
                "closed": True,
                "normalized": True,
                "contour_status": "CLOSED_POLYGON_PROPOSAL_NOT_AUTHORITY",
                "contour_provenance": "proposal",
                "proposed_visibility": "inferred",
                "semantic_visibility": "inferred",
                "runtime_visibility": "unknown",
                "requires_user_confirmation": True,
                "requires_runtime_part_index_binding": True,
                "user_confirmed": False,
            }],
            "negative_space_v2": {
                "status": negative_status,
                "user_confirmed": False,
                "regions": negative_regions,
            },
            "line_flows_v2": [{
                "line_flow_id": f"{view}.flow",
                "runtime_kind_candidate": "seam",
                "continuity_group_id": f"lineflow.{view}.flow",
                "points": [[0.2, 0.5], [0.8, 0.5]],
                "runtime_visibility": "unknown",
                "requires_user_confirmation": True,
                "user_confirmed": False,
            }],
        }
    return {
        "schema_version": "ForgeCADWeaponFormReviewProposalV4@0",
        "status": "PROPOSAL_REVIEW_PENDING",
        "proposal_only": True,
        "user_confirmed": False,
        "runtime_write": False,
        "worker_started": False,
        "receipt_created": False,
        "source_asset_name": "fictional-energy-weapon-reference.png",
        "source_png_sha256": "1" * 64,
        "overlay_filename": "six-identity-view-review-overlay-v4.png",
        "overlay_sha256": "2" * 64,
        "view_order": list(VIEW_ORDER),
        "views": views,
    }


class FormReviewProposalValidatorTests(unittest.TestCase):
    def test_valid_unconfirmed_proposal_is_review_ready_not_runtime_truth(self) -> None:
        result = validate_proposal(proposal())
        self.assertEqual(result["status"], "READY_FOR_USER_REVIEW_NOT_RUNTIME_WRITE")
        self.assertEqual(result["counts"]["part_polygons"], 6)
        self.assertEqual(result["counts"]["negative_polygons"], 3)
        self.assertFalse(result["runtime_write"])

    def test_rejects_confirmed_observed_or_self_intersecting_proposal(self) -> None:
        confirmed = proposal()
        confirmed["user_confirmed"] = True
        with self.assertRaisesRegex(ValueError, "user_confirmed must be false"):
            validate_proposal(confirmed)

        observed = proposal()
        observed["views"]["front"]["part_regions_v3"][0]["proposed_visibility"] = "observed"
        with self.assertRaisesRegex(ValueError, "inferred/Runtime-unknown"):
            validate_proposal(observed)

        crossed = proposal()
        crossed["views"]["front"]["part_regions_v3"][0]["closed_contour_points"] = [
            [0.3, 0.3], [0.7, 0.7], [0.3, 0.7], [0.7, 0.3]
        ]
        with self.assertRaisesRegex(ValueError, "self-intersects"):
            validate_proposal(crossed)

    def test_rejects_negative_space_or_line_flow_semantic_drift(self) -> None:
        illegal_negative = proposal()
        illegal_negative["views"]["front"]["negative_space_v2"]["regions"] = copy.deepcopy(
            illegal_negative["views"]["left"]["negative_space_v2"]["regions"]
        )
        with self.assertRaisesRegex(ValueError, "negative-space zero-row"):
            validate_proposal(illegal_negative)

        illegal_flow = proposal()
        illegal_flow["views"]["left"]["line_flows_v2"][0]["runtime_kind_candidate"] = "upper-spine"
        with self.assertRaisesRegex(ValueError, "unsupported Runtime kind"):
            validate_proposal(illegal_flow)

        wrong_space = proposal()
        wrong_space["views"]["left"]["negative_space_v2"]["regions"][0]["target_coordinate_space"] = "normalized_crop_pixels"
        with self.assertRaisesRegex(ValueError, "target_coordinate_space must be normalized_aspect_fit_512"):
            validate_proposal(wrong_space)

        unsupported_containment = proposal()
        unsupported_containment["views"]["front"]["part_regions_v3"][0]["within_declared_bbox"] = True
        with self.assertRaisesRegex(ValueError, "unsupported cross-space truth claim"):
            validate_proposal(unsupported_containment)


if __name__ == "__main__":
    unittest.main()
