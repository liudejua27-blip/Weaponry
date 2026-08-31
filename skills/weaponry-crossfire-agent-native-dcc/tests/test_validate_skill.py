"""Behavioral checks for the static knife Skill validator."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


SKILL_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SKILL_ROOT / "scripts"))
import validate_skill  # noqa: E402


class KnifeSkillValidationTests(unittest.TestCase):
    def test_current_skill_is_valid(self) -> None:
        validate_skill.validate()

    def test_facade_order_is_closed(self) -> None:
        content = validate_skill.read(SKILL_ROOT / "SKILL.md")
        swapped = content.replace(
            "4. `authoring_transaction`",
            "99. `authoring_transaction`",
            1,
        )
        with self.assertRaises(AssertionError):
            validate_skill.check_facade_route(swapped)

    def test_contract_profile_drift_is_rejected(self) -> None:
        content = validate_skill.read(SKILL_ROOT / "SKILL.md")
        profile = {
            "profile_status": "development-only",
            "default_profile": {"facade_names": list(reversed(validate_skill.FACADES))},
        }
        with self.assertRaises(AssertionError):
            validate_skill.check_profile_binding(content, profile)

    def test_source_identity_patterns_are_rejected(self) -> None:
        samples = (
            "embedded_source: false\n" + "owner" + chr(64) + "example.invalid\n",
            "embedded_source: false\n+86 " + "1" * 10 + "\n",
            "embedded_source: false\ndata:" + "image/png;" + "base64," + "abc\n",
        )
        for sample in samples:
            with self.assertRaises(AssertionError):
                validate_skill.check_sanitized_benchmark(sample)

    def test_brief_runtime_route_and_immutable_successor_are_required(self) -> None:
        content = validate_skill.read(SKILL_ROOT / "SKILL.md")
        for marker in (
            "weaponry_knife_production_brief_prepare",
            "weaponry_knife_production_brief_get",
            "immutable-successor-preserve-source-claims@1",
            "KnifeReferenceIntentBundle@1",
            "KnifePassState@1",
            "KnifeCorrectionLedger@1",
        ):
            with self.subTest(marker=marker):
                with self.assertRaises(AssertionError):
                    validate_skill.check_static_markers(content.replace(marker, "removed", 1))


if __name__ == "__main__":
    unittest.main()
