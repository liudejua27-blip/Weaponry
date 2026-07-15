import pytest

from forgecad_agent.application.agent_models import (
    ActiveDesignExportReference,
    ActiveDesignPreviewReference,
    ActiveDesignQualityReference,
    ActiveDesignSnapshot,
    AgentActiveDesignReference,
    LegacyActiveDesignReference,
)


def _agent_snapshot(**overrides):
    data = {
        "project_id": "prj_unit_test",
        "active_design": AgentActiveDesignReference(
            project_id="prj_unit_test", asset_version_id="assetver_v1", assembly_graph_id="mg_v1"
        ),
        "export": ActiveDesignExportReference(
            source="agent_asset", project_id="prj_unit_test", source_version_id="assetver_v1"
        ),
        "revision": 1,
        "updated_at": "2026-07-13T00:00:00Z",
    }
    data.update(overrides)
    return ActiveDesignSnapshot(**data)


def test_agent_snapshot_requires_export_to_follow_active_version():
    snapshot = _agent_snapshot(
        preview=ActiveDesignPreviewReference(
            project_id="prj_unit_test", change_set_id="assetcs_preview", base_asset_version_id="assetver_v1"
        ),
        quality=ActiveDesignQualityReference(
            project_id="prj_unit_test", quality_report_id="quality_v1", asset_version_id="assetver_v1"
        ),
    )
    assert snapshot.export.source_version_id == "assetver_v1"

    with pytest.raises(ValueError, match="export must reference"):
        _agent_snapshot(
            export=ActiveDesignExportReference(
                source="agent_asset", project_id="prj_unit_test", source_version_id="assetver_other"
            )
        )


def test_legacy_snapshot_cannot_attach_agent_state():
    with pytest.raises(ValueError, match="cannot select"):
        ActiveDesignSnapshot(
            project_id="prj_unit_test",
            active_design=LegacyActiveDesignReference(
                project_id="prj_unit_test", legacy_version_id="ver_v1", module_graph_id="mg_legacy"
            ),
            selected_part_id="part_body",
            export=ActiveDesignExportReference(
                source="legacy_concept_read_only", project_id="prj_unit_test", source_version_id="ver_v1"
            ),
            revision=1,
            updated_at="2026-07-13T00:00:00Z",
        )
