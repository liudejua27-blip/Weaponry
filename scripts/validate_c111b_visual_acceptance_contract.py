#!/usr/bin/env python3
"""Standalone offline validation for the frozen C111B acceptance contract."""

from __future__ import annotations

import json
from pathlib import Path

from c111b_visual_acceptance_contract import (
    load_c111b_visual_acceptance_contract,
    summarize_contract,
)


ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = ROOT / "packages/concept-spec/fixtures/c111-golden-surface-robotic-arm-visual-detail-inventory.json"


def main() -> int:
    inventory = json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
    contract, contract_sha256 = load_c111b_visual_acceptance_contract(ROOT, inventory)
    print(json.dumps({"status": "pass", "visual_acceptance_contract": summarize_contract(contract, contract_sha256)}, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
