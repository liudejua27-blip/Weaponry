#!/usr/bin/env python3
"""Verify D002 inference plus D003 clarification persistence write barriers."""

from __future__ import annotations

import tempfile
from pathlib import Path

from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import CreateAgentThreadRequest, StartAgentTurnRequest
from forgecad_agent.application.domain_inference import infer_domain
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[1]


def _table_count(factory: SQLiteConnectionFactory, table: str) -> int:
    connection = factory.connect()
    try:
        return int(connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])
    finally:
        connection.close()


def main() -> int:
    expected = {
        "设计一辆冰原探索车": "pack_vehicle_concept",
        "Design a compact VTOL aircraft": "pack_aircraft_concept",
        "设计一台三关节机械臂": "pack_robotic_arm_concept",
        "制作一个科幻武器游戏道具": "pack_future_weapon_prop",
    }
    for brief, pack_id in expected.items():
        result = infer_domain(brief)
        assert result.status == "recognized" and result.domain_pack_id == pack_id, result
    ambiguous = infer_domain("设计一台能飞的无人机载具")
    assert ambiguous.status == "ambiguous"
    assert set(ambiguous.candidate_domain_pack_ids) == {"pack_vehicle_concept", "pack_aircraft_concept"}
    unsupported = infer_domain("做一个海洋生物雕塑")
    assert unsupported.status == "unsupported"

    with tempfile.TemporaryDirectory(prefix="forgecad-domain-inference-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        service = AgentKernelService(factory)
        thread = service.create_thread(CreateAgentThreadRequest(client_request_id="domain-thread"), "domain-thread")
        baseline_assets = {
            table: _table_count(factory, table)
            for table in ("agent_blockout_candidates", "agent_asset_versions")
        }
        ambiguous_turn = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="domain-turn-1", message="设计一台能飞的无人机载具"),
            "domain-turn-1",
        )
        assert ambiguous_turn.status == "waiting_for_clarification"
        ambiguous_item = next(item for item in ambiguous_turn.items if item.item_type == "clarification")
        assert ambiguous_item.payload["status"] == "ambiguous"
        assert len(ambiguous_item.payload["options"]) == 4
        assert set(ambiguous_item.payload["domain_inference"]["candidate_domain_pack_ids"]) == {
            "pack_vehicle_concept",
            "pack_aircraft_concept",
        }
        assert service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="domain-turn-1", message="设计一台能飞的无人机载具"),
            "domain-turn-1",
        ).turn_id == ambiguous_turn.turn_id

        unsupported_turn = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="domain-turn-2", message="做一个海洋生物雕塑"),
            "domain-turn-2",
        )
        assert unsupported_turn.status == "waiting_for_clarification"
        unsupported_item = next(item for item in unsupported_turn.items if item.item_type == "clarification")
        assert unsupported_item.payload["status"] == "unsupported"
        assert len(unsupported_item.payload["options"]) == 4
        for table, count in baseline_assets.items():
            assert _table_count(factory, table) == count, f"{table} changed after clarification"

        known = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="domain-turn-known", message="设计一辆双座冰原探索车"),
            "domain-turn-known",
        )
        assert known.status == "completed"

    print("D002/D003 domain smoke passed: four packs, persisted single-question clarification, asset write barrier")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
