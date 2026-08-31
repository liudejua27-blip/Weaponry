"""Focused checks for the closed blade constraint fixture.

These tests stay at the Skill/JSON boundary.  They do not call MCP, Runtime,
CAS, a renderer, or any external DCC.
"""

from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


SKILL_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SKILL_ROOT / "scripts"))
import generate_blade_multiview_constraint as generator  # noqa: E402


FIXTURE = SKILL_ROOT / "references" / "blade-multiview-constraint-set-v1.json"


class BladeMultiViewConstraintTests(unittest.TestCase):
    def setUp(self) -> None:
        self.document = generator.build_constraint_set()

    def test_fixture_is_deterministic_and_canonical(self) -> None:
        fixture = generator.load_and_validate(FIXTURE)
        self.assertEqual(fixture, self.document)
        self.assertEqual(FIXTURE.read_bytes(), generator.canonical_bytes(fixture) + b"\n")

    def test_view_set_is_closed_and_ordered(self) -> None:
        self.assertEqual([item["view_id"] for item in self.document["views"]], list(generator.VIEW_IDS))
        invalid = copy.deepcopy(self.document)
        invalid["views"].append(copy.deepcopy(invalid["views"][0]))
        invalid["views"][-1]["view_id"] = "BACK"
        invalid["canonical_sha256"] = generator.canonical_sha256(invalid)
        with self.assertRaises(ValueError):
            generator.validate_constraint_set(invalid)

    def test_unknown_view_measurement_cannot_be_invented(self) -> None:
        top = next(item for item in self.document["views"] if item["view_id"] == "TOP")
        point = top["curve_landmarks"]["spine"][0]
        self.assertEqual(point["status"], "unknown")
        self.assertIsNone(point["value"])
        invalid = copy.deepcopy(self.document)
        invalid_top = next(item for item in invalid["views"] if item["view_id"] == "TOP")
        invalid_top["curve_landmarks"]["spine"][0]["value"] = [0.5, 0.5]
        invalid_top["curve_landmarks"]["spine"][0]["status"] = "observed"
        invalid_top["curve_landmarks"]["spine"][0]["confidence"] = 0.5
        invalid_top["curve_landmarks"]["spine"][0]["basis"] = "inferred"
        invalid_top["curve_landmarks"]["spine"][0]["source_panel"] = "top"
        invalid["canonical_sha256"] = generator.canonical_sha256(invalid)
        with self.assertRaises(ValueError):
            generator.validate_constraint_set(invalid)

    def test_section_dimensions_remain_unknown_but_stations_are_frozen(self) -> None:
        self.assertEqual(self.document["section_loft"]["station_order"], list(generator.LANDMARK_IDS))
        self.assertEqual(
            [item["u"] for item in self.document["section_loft"]["stations"]],
            [0.0, 0.36, 0.72, 1.0],
        )
        invalid = copy.deepcopy(self.document)
        invalid["section_loft"]["stations"][0]["cross_section"]["thickness_m"] = 0.004
        invalid["canonical_sha256"] = generator.canonical_sha256(invalid)
        with self.assertRaises(ValueError):
            generator.validate_constraint_set(invalid)

    def test_first_round_scope_freezes_ornament_assembly_and_materials(self) -> None:
        scope = self.document["correction_scope"]
        self.assertEqual(scope["allowed_parts"], ["blade-body", "cutting-edge"])
        self.assertIn("dragon-relief", scope["locked_parts"])
        self.assertIn("guard-dragon-head", scope["locked_parts"])
        self.assertIn("grip", scope["locked_parts"])
        self.assertIn("pommel", scope["locked_parts"])
        self.assertEqual(len(scope["locked_material_zones"]), 5)
        invalid = copy.deepcopy(self.document)
        invalid["correction_scope"]["allowed_parts"].append("dragon-relief")
        invalid["canonical_sha256"] = generator.canonical_sha256(invalid)
        with self.assertRaises(ValueError):
            generator.validate_constraint_set(invalid)

    def test_fixture_does_not_embed_source_payload_or_path(self) -> None:
        serialized = FIXTURE.read_text(encoding="utf-8").lower()
        self.assertNotIn("data:", serialized)
        self.assertNotIn("file://", serialized)
        self.assertIsNone(self.document["reference"]["source_sha256"])


if __name__ == "__main__":
    unittest.main()
