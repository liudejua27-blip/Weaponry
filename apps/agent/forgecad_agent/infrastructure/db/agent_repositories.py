from __future__ import annotations

import json
import sqlite3
from typing import TYPE_CHECKING, Any, Mapping, Optional

if TYPE_CHECKING:
    from forgecad_agent.application.agent_models import ActiveDesignSnapshot


class ActiveDesignSnapshotError(RuntimeError):
    pass


class ActiveDesignSnapshotConflict(ActiveDesignSnapshotError):
    pass


class ActiveDesignSnapshotRepository:
    """Persistence and compare-and-swap for the single workbench design state."""

    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def get_snapshot(self, project_id: str) -> Optional["ActiveDesignSnapshot"]:
        render_column = "render_preset_json" if self._has_render_preset_column() else "NULL AS render_preset_json"
        zone_column = "selected_material_zone_id" if self._has_material_zone_column() else "NULL AS selected_material_zone_id"
        display_column = "part_display_json" if self._has_part_display_column() else "NULL AS part_display_json"
        row = self.connection.execute(
            f"""
            SELECT project_id, source, active_asset_version_id, active_assembly_graph_id,
                   legacy_version_id, legacy_module_graph_id, selected_part_id,
                   {zone_column},
                   preview_change_set_id, preview_base_asset_version_id,
                   quality_report_id, quality_asset_version_id,
                   export_source, export_source_version_id, revision, updated_at,
                   {render_column}, {display_column}
            FROM active_design_snapshots
            WHERE project_id = ?
            """,
            (project_id,),
        ).fetchone()
        return _snapshot_from_row(row) if row is not None else None

    def create_agent_snapshot(
        self,
        *,
        project_id: str,
        asset_version_id: str,
        assembly_graph_id: str,
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        self._require_agent_asset(project_id, asset_version_id, assembly_graph_id)
        try:
            if self._has_render_preset_column() and self._has_part_display_column():
                self.connection.execute(
                    """
                    INSERT INTO active_design_snapshots (
                      project_id, source, active_asset_version_id, active_assembly_graph_id,
                      legacy_version_id, legacy_module_graph_id, selected_part_id,
                      preview_change_set_id, preview_base_asset_version_id,
                      quality_report_id, quality_asset_version_id,
                      export_source, export_source_version_id, render_preset_json, part_display_json, revision, updated_at
                    ) VALUES (?, 'agent_asset', ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'agent_asset', ?, ?, ?, 1, ?)
                    """,
                    (
                        project_id,
                        asset_version_id,
                        assembly_graph_id,
                        asset_version_id,
                        _default_render_preset_json(project_id, asset_version_id, updated_at),
                        _default_part_display_json(project_id, asset_version_id),
                        updated_at,
                    ),
                )
            elif self._has_render_preset_column():
                self.connection.execute(
                    """
                    INSERT INTO active_design_snapshots (
                      project_id, source, active_asset_version_id, active_assembly_graph_id,
                      legacy_version_id, legacy_module_graph_id, selected_part_id,
                      preview_change_set_id, preview_base_asset_version_id,
                      quality_report_id, quality_asset_version_id,
                      export_source, export_source_version_id, render_preset_json, revision, updated_at
                    ) VALUES (?, 'agent_asset', ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'agent_asset', ?, ?, 1, ?)
                    """,
                    (project_id, asset_version_id, assembly_graph_id, asset_version_id, _default_render_preset_json(project_id, asset_version_id, updated_at), updated_at),
                )
            else:
                self.connection.execute(
                    """
                    INSERT INTO active_design_snapshots (
                      project_id, source, active_asset_version_id, active_assembly_graph_id,
                      legacy_version_id, legacy_module_graph_id, selected_part_id,
                      preview_change_set_id, preview_base_asset_version_id,
                      quality_report_id, quality_asset_version_id,
                      export_source, export_source_version_id, revision, updated_at
                    ) VALUES (?, 'agent_asset', ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'agent_asset', ?, 1, ?)
                    """,
                    (project_id, asset_version_id, assembly_graph_id, asset_version_id, updated_at),
                )
        except sqlite3.IntegrityError as exc:
            raise ActiveDesignSnapshotConflict("active design snapshot already exists") from exc
        snapshot = self.get_snapshot(project_id)
        if snapshot is None:
            raise ActiveDesignSnapshotError("created active design snapshot could not be read")
        return snapshot

    def create_legacy_snapshot(
        self,
        *,
        project_id: str,
        legacy_version_id: str,
        module_graph_id: str,
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        self._require_legacy_version(project_id, legacy_version_id, module_graph_id)
        try:
            self.connection.execute(
                """
                INSERT INTO active_design_snapshots (
                  project_id, source, active_asset_version_id, active_assembly_graph_id,
                  legacy_version_id, legacy_module_graph_id, selected_part_id,
                  preview_change_set_id, preview_base_asset_version_id,
                  quality_report_id, quality_asset_version_id,
                  export_source, export_source_version_id, revision, updated_at
                ) VALUES (?, 'legacy_concept_read_only', NULL, NULL, ?, ?, NULL, NULL, NULL, NULL, NULL,
                          'legacy_concept_read_only', ?, 1, ?)
                """,
                (project_id, legacy_version_id, module_graph_id, legacy_version_id, updated_at),
            )
        except sqlite3.IntegrityError as exc:
            raise ActiveDesignSnapshotConflict("active design snapshot already exists") from exc
        snapshot = self.get_snapshot(project_id)
        if snapshot is None:
            raise ActiveDesignSnapshotError("created active design snapshot could not be read")
        return snapshot

    def record_legacy_conversion_intent(
        self,
        *,
        project_id: str,
        expected_revision: int,
        legacy_version_id: str,
        module_graph_id: str,
        requested_at: str,
    ) -> None:
        snapshot = self.get_snapshot(project_id)
        if snapshot is None or snapshot.revision != expected_revision:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        if getattr(snapshot.active_design, "source", None) != "legacy_concept_read_only":
            raise ActiveDesignSnapshotError("active design is already an Agent asset")
        if (
            snapshot.active_design.legacy_version_id != legacy_version_id
            or snapshot.active_design.module_graph_id != module_graph_id
        ):
            raise ActiveDesignSnapshotError("legacy conversion source does not match the active design")
        self.connection.execute(
            """
            INSERT INTO legacy_agent_conversion_intents (
              project_id, legacy_version_id, legacy_module_graph_id,
              snapshot_revision, requested_at
            ) VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(project_id) DO UPDATE SET
              legacy_version_id = excluded.legacy_version_id,
              legacy_module_graph_id = excluded.legacy_module_graph_id,
              snapshot_revision = excluded.snapshot_revision,
              requested_at = excluded.requested_at
            """,
            (project_id, legacy_version_id, module_graph_id, expected_revision, requested_at),
        )

    def get_legacy_conversion_intent(self, project_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT project_id, legacy_version_id, legacy_module_graph_id,
                   snapshot_revision, requested_at
            FROM legacy_agent_conversion_intents
            WHERE project_id = ?
            """,
            (project_id,),
        ).fetchone()

    def promote_legacy_to_agent_snapshot(
        self,
        *,
        project_id: str,
        expected_revision: int,
        asset_version_id: str,
        assembly_graph_id: str,
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        self._require_agent_asset(project_id, asset_version_id, assembly_graph_id)
        intent = self.get_legacy_conversion_intent(project_id)
        if intent is None or int(intent["snapshot_revision"]) != expected_revision:
            raise ActiveDesignSnapshotError("legacy design conversion was not explicitly requested")
        render_update = ", render_preset_json = ?" if self._has_render_preset_column() else ""
        zone_update = ", selected_material_zone_id = NULL" if self._has_material_zone_column() else ""
        display_update = ", part_display_json = ?" if self._has_part_display_column() else ""
        parameters = (asset_version_id, assembly_graph_id, asset_version_id)
        if self._has_render_preset_column():
            parameters += (_default_render_preset_json(project_id, asset_version_id, updated_at),)
        if self._has_part_display_column():
            parameters += (_default_part_display_json(project_id, asset_version_id),)
        parameters += (updated_at, project_id, expected_revision)
        changed = self.connection.execute(
            f"""
            UPDATE active_design_snapshots
            SET source = 'agent_asset',
                active_asset_version_id = ?, active_assembly_graph_id = ?,
                legacy_version_id = NULL, legacy_module_graph_id = NULL,
                selected_part_id = NULL{zone_update},
                preview_change_set_id = NULL, preview_base_asset_version_id = NULL,
                quality_report_id = NULL, quality_asset_version_id = NULL,
                export_source = 'agent_asset', export_source_version_id = ?{render_update}{display_update},
                revision = revision + 1, updated_at = ?
            WHERE project_id = ? AND source = 'legacy_concept_read_only' AND revision = ?
            """,
            parameters,
        ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        self.connection.execute(
            "DELETE FROM legacy_agent_conversion_intents WHERE project_id = ?",
            (project_id,),
        )
        result = self.get_snapshot(project_id)
        if result is None:
            raise ActiveDesignSnapshotError("promoted active design snapshot could not be read")
        return result

    def advance_agent_snapshot(
        self,
        *,
        project_id: str,
        expected_revision: int,
        asset_version_id: str,
        assembly_graph_id: str,
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        self._require_agent_asset(project_id, asset_version_id, assembly_graph_id)
        current = self._require_agent_snapshot(project_id, expected_revision)
        selected_part_id, selected_zone_id = self._selection_for_asset(
            asset_version_id,
            assembly_graph_id,
            current.selected_part_id,
            current.selected_material_zone_id,
        )
        if not self._has_material_zone_column():
            selected_part_id, selected_zone_id = None, None
        part_display = self._part_display_for_asset(
            asset_version_id=asset_version_id,
            assembly_graph_id=assembly_graph_id,
            current=current.part_display,
            project_id=project_id,
        ) if self._has_part_display_column() else None
        render_update = ", render_preset_json = ?" if self._has_render_preset_column() else ""
        zone_update = ", selected_material_zone_id = ?" if self._has_material_zone_column() else ""
        display_update = ", part_display_json = ?" if self._has_part_display_column() else ""
        parameters = (asset_version_id, assembly_graph_id)
        if self._has_material_zone_column():
            parameters += (selected_part_id, selected_zone_id)
        else:
            parameters += (selected_part_id,)
        parameters += (asset_version_id,)
        if self._has_render_preset_column():
            parameters += (_default_render_preset_json(project_id, asset_version_id, updated_at),)
        if self._has_part_display_column():
            parameters += (_canonical_json(part_display.model_dump(mode="json")),)
        parameters += (updated_at, project_id, expected_revision)
        changed = self.connection.execute(
            f"""
            UPDATE active_design_snapshots
            SET active_asset_version_id = ?, active_assembly_graph_id = ?,
                selected_part_id = ?{zone_update},
                preview_change_set_id = NULL, preview_base_asset_version_id = NULL,
                quality_report_id = NULL, quality_asset_version_id = NULL,
                export_source = 'agent_asset', export_source_version_id = ?{render_update}{display_update},
                revision = revision + 1, updated_at = ?
            WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
            """,
            parameters,
        ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        snapshot = self.get_snapshot(project_id)
        if snapshot is None:
            raise ActiveDesignSnapshotError("updated active design snapshot could not be read")
        return snapshot

    def set_render_preset(
        self,
        *,
        project_id: str,
        expected_revision: int,
        camera_view: str,
        light_preset: str,
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        if not self._has_render_preset_column():
            raise ActiveDesignSnapshotError("active design render presets require migration 0028")
        snapshot = self._require_agent_snapshot(project_id, expected_revision)
        asset_version_id = snapshot.active_design.asset_version_id
        render_json = _render_preset_json(
            project_id=project_id,
            asset_version_id=asset_version_id,
            camera_view=camera_view,
            light_preset=light_preset,
            updated_at=updated_at,
        )
        changed = self.connection.execute(
            """
            UPDATE active_design_snapshots
            SET render_preset_json = ?, revision = revision + 1, updated_at = ?
            WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
            """,
            (render_json, updated_at, project_id, expected_revision),
        ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        result = self.get_snapshot(project_id)
        if result is None:
            raise ActiveDesignSnapshotError("updated render preset snapshot could not be read")
        return result

    def _has_render_preset_column(self) -> bool:
        columns = self.connection.execute("PRAGMA table_info(active_design_snapshots)").fetchall()
        return any(str(row[1]) == "render_preset_json" for row in columns)

    def _has_material_zone_column(self) -> bool:
        columns = self.connection.execute("PRAGMA table_info(active_design_snapshots)").fetchall()
        return any(str(row[1]) == "selected_material_zone_id" for row in columns)

    def _has_part_display_column(self) -> bool:
        columns = self.connection.execute("PRAGMA table_info(active_design_snapshots)").fetchall()
        return any(str(row[1]) == "part_display_json" for row in columns)

    def select_agent_part(
        self,
        *,
        project_id: str,
        expected_revision: int,
        part_id: Optional[str],
        updated_at: str,
        material_zone_id: Optional[str] = None,
    ) -> "ActiveDesignSnapshot":
        snapshot = self.get_snapshot(project_id)
        if snapshot is None or snapshot.revision != expected_revision:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        if getattr(snapshot.active_design, "source", None) != "agent_asset":
            raise ActiveDesignSnapshotError("legacy_concept_read_only snapshot cannot select an Agent part")
        selected_zone_id: Optional[str] = None
        if part_id is not None:
            zones = self._require_part(snapshot.active_design.asset_version_id, snapshot.active_design.assembly_graph_id, part_id)
            display = snapshot.part_display
            if display is not None and (
                part_id in display.hidden_part_ids
                or (display.isolated_part_id is not None and part_id != display.isolated_part_id)
            ):
                raise ActiveDesignSnapshotError("selected part is not currently visible")
            if material_zone_id is not None and material_zone_id not in zones:
                raise ActiveDesignSnapshotError("selected material zone does not belong to the selected part")
            selected_zone_id = material_zone_id or (zones[0] if zones else None)
        elif material_zone_id is not None:
            raise ActiveDesignSnapshotError("selected material zone requires a selected part")
        if self._has_material_zone_column():
            changed = self.connection.execute(
                """
                UPDATE active_design_snapshots
                SET selected_part_id = ?, selected_material_zone_id = ?, revision = revision + 1, updated_at = ?
                WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
                """,
                (part_id, selected_zone_id, updated_at, project_id, expected_revision),
            ).rowcount
        else:
            changed = self.connection.execute(
                """
                UPDATE active_design_snapshots
                SET selected_part_id = ?, revision = revision + 1, updated_at = ?
                WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
                """,
                (part_id, updated_at, project_id, expected_revision),
            ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        result = self.get_snapshot(project_id)
        if result is None:
            raise ActiveDesignSnapshotError("updated active design snapshot could not be read")
        return result

    def set_part_display(
        self,
        *,
        project_id: str,
        expected_revision: int,
        action: str,
        part_id: Optional[str],
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        """Persist one bounded display/protection action without changing geometry."""
        if not self._has_part_display_column():
            raise ActiveDesignSnapshotError("active design part display requires migration 0031")
        snapshot = self._require_agent_snapshot(project_id, expected_revision)
        if snapshot.preview is not None:
            raise ActiveDesignSnapshotError("confirm or cancel the pending preview before changing part display")
        asset_version_id = snapshot.active_design.asset_version_id
        display = self._part_display_for_asset(
            asset_version_id=asset_version_id,
            assembly_graph_id=snapshot.active_design.assembly_graph_id,
            current=snapshot.part_display,
            project_id=project_id,
        )
        part_ids = self._part_ids_for_asset(asset_version_id, snapshot.active_design.assembly_graph_id)
        if part_id is not None and part_id not in part_ids:
            raise ActiveDesignSnapshotError("part display action targets a part outside the active assembly")

        locked = set(display.locked_part_ids)
        hidden = set(display.hidden_part_ids)
        isolated = display.isolated_part_id
        if action == "lock":
            assert part_id is not None
            locked.add(part_id)
        elif action == "unlock":
            assert part_id is not None
            locked.discard(part_id)
        elif action == "hide":
            assert part_id is not None
            hidden.add(part_id)
            if isolated == part_id:
                isolated = None
        elif action == "show":
            assert part_id is not None
            hidden.discard(part_id)
        elif action == "isolate":
            assert part_id is not None
            hidden.discard(part_id)
            isolated = part_id
        elif action == "clear_isolation":
            isolated = None
        elif action == "show_all":
            hidden.clear()
            isolated = None
        else:
            raise ActiveDesignSnapshotError("unsupported active design part display action")

        from forgecad_agent.application.agent_models import ActiveDesignPartDisplay

        next_display = ActiveDesignPartDisplay(
            project_id=project_id,
            asset_version_id=asset_version_id,
            locked_part_ids=sorted(locked),
            hidden_part_ids=sorted(hidden),
            isolated_part_id=isolated,
        )
        selected_part_id = snapshot.selected_part_id
        selected_zone_id = snapshot.selected_material_zone_id
        if selected_part_id is not None and (
            selected_part_id in next_display.hidden_part_ids
            or (next_display.isolated_part_id is not None and selected_part_id != next_display.isolated_part_id)
        ):
            selected_part_id, selected_zone_id = None, None
        changed = self.connection.execute(
            """
            UPDATE active_design_snapshots
            SET part_display_json = ?, selected_part_id = ?, selected_material_zone_id = ?,
                revision = revision + 1, updated_at = ?
            WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
            """,
            (
                _canonical_json(next_display.model_dump(mode="json")),
                selected_part_id,
                selected_zone_id,
                updated_at,
                project_id,
                expected_revision,
            ),
        ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        result = self.get_snapshot(project_id)
        if result is None:
            raise ActiveDesignSnapshotError("updated active design snapshot could not be read")
        return result

    def set_preview(
        self,
        *,
        project_id: str,
        expected_revision: int,
        change_set_id: Optional[str],
        base_asset_version_id: Optional[str],
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        snapshot = self._require_agent_snapshot(project_id, expected_revision)
        if (change_set_id is None) != (base_asset_version_id is None):
            raise ActiveDesignSnapshotError("preview reference must provide both change_set_id and base_asset_version_id")
        if change_set_id is not None:
            self._require_change_set(project_id, change_set_id, base_asset_version_id)
            if base_asset_version_id != snapshot.active_design.asset_version_id:
                raise ActiveDesignSnapshotError("preview base asset must match active Agent asset version")
        changed = self.connection.execute(
            """
            UPDATE active_design_snapshots
            SET preview_change_set_id = ?, preview_base_asset_version_id = ?,
                revision = revision + 1, updated_at = ?
            WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
            """,
            (change_set_id, base_asset_version_id, updated_at, project_id, expected_revision),
        ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        result = self.get_snapshot(project_id)
        if result is None:
            raise ActiveDesignSnapshotError("updated active design snapshot could not be read")
        return result

    def set_quality(
        self,
        *,
        project_id: str,
        expected_revision: int,
        quality_report_id: Optional[str],
        asset_version_id: Optional[str],
        updated_at: str,
    ) -> "ActiveDesignSnapshot":
        snapshot = self._require_agent_snapshot(project_id, expected_revision)
        if (quality_report_id is None) != (asset_version_id is None):
            raise ActiveDesignSnapshotError("quality reference must provide both quality_report_id and asset_version_id")
        if asset_version_id is not None and asset_version_id != snapshot.active_design.asset_version_id:
            raise ActiveDesignSnapshotError("quality asset must match active Agent asset version")
        changed = self.connection.execute(
            """
            UPDATE active_design_snapshots
            SET quality_report_id = ?, quality_asset_version_id = ?,
                revision = revision + 1, updated_at = ?
            WHERE project_id = ? AND source = 'agent_asset' AND revision = ?
            """,
            (quality_report_id, asset_version_id, updated_at, project_id, expected_revision),
        ).rowcount
        if changed != 1:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        result = self.get_snapshot(project_id)
        if result is None:
            raise ActiveDesignSnapshotError("updated active design snapshot could not be read")
        return result

    def _require_agent_snapshot(self, project_id: str, expected_revision: int) -> "ActiveDesignSnapshot":
        snapshot = self.get_snapshot(project_id)
        if snapshot is None or snapshot.revision != expected_revision:
            raise ActiveDesignSnapshotConflict("active design snapshot revision is stale")
        if getattr(snapshot.active_design, "source", None) != "agent_asset":
            raise ActiveDesignSnapshotError("legacy_concept_read_only snapshot cannot use Agent workflow metadata")
        return snapshot

    def _require_agent_asset(self, project_id: str, asset_version_id: str, assembly_graph_id: str) -> None:
        self._require_active_project(project_id)
        row = self.connection.execute(
            """
            SELECT project_id, status, assembly_graph_json
            FROM agent_asset_versions
            WHERE asset_version_id = ?
            """,
            (asset_version_id,),
        ).fetchone()
        if row is None or str(row["project_id"]) != project_id or str(row["status"]) != "committed":
            raise ActiveDesignSnapshotError("Agent asset version does not belong to the active project")
        try:
            graph = json.loads(str(row["assembly_graph_json"]))
        except json.JSONDecodeError as exc:
            raise ActiveDesignSnapshotError("Agent asset assembly graph is invalid") from exc
        if not isinstance(graph, dict) or graph.get("graph_id") != assembly_graph_id:
            raise ActiveDesignSnapshotError("Agent asset assembly graph ID does not match the active design reference")

    def _require_part(self, asset_version_id: str, assembly_graph_id: str, part_id: str) -> list[str]:
        row = self.connection.execute(
            "SELECT assembly_graph_json FROM agent_asset_versions WHERE asset_version_id = ?",
            (asset_version_id,),
        ).fetchone()
        if row is None:
            raise ActiveDesignSnapshotError("Agent asset version is unavailable")
        try:
            graph = json.loads(str(row["assembly_graph_json"]))
        except json.JSONDecodeError as exc:
            raise ActiveDesignSnapshotError("Agent asset assembly graph is invalid") from exc
        if not isinstance(graph, dict) or graph.get("graph_id") != assembly_graph_id:
            raise ActiveDesignSnapshotError("active assembly graph no longer matches the active asset")
        parts = graph.get("parts")
        if not isinstance(parts, list):
            raise ActiveDesignSnapshotError("active assembly has no parts")
        selected = next((item for item in parts if isinstance(item, dict) and str(item.get("part_id")) == part_id), None)
        if selected is None:
            raise ActiveDesignSnapshotError("selected part does not belong to the active assembly")
        zones = selected.get("material_zones")
        if not isinstance(zones, list):
            zones = selected.get("material_zone_ids")
        if not isinstance(zones, list):
            zones = []
        return [str(zone) for zone in zones if isinstance(zone, str) and zone.startswith("zone_")]

    def _selection_for_asset(
        self,
        asset_version_id: str,
        assembly_graph_id: str,
        part_id: Optional[str],
        material_zone_id: Optional[str],
    ) -> tuple[Optional[str], Optional[str]]:
        """Retain a selection across an immutable version switch when it still exists."""
        if part_id is None:
            return None, None
        try:
            zones = self._require_part(asset_version_id, assembly_graph_id, part_id)
        except ActiveDesignSnapshotError:
            return None, None
        if material_zone_id in zones:
            return part_id, material_zone_id
        return part_id, (zones[0] if zones else None)

    def _part_ids_for_asset(self, asset_version_id: str, assembly_graph_id: str) -> set[str]:
        row = self.connection.execute(
            "SELECT assembly_graph_json FROM agent_asset_versions WHERE asset_version_id = ?",
            (asset_version_id,),
        ).fetchone()
        if row is None:
            raise ActiveDesignSnapshotError("Agent asset version is unavailable")
        try:
            graph = json.loads(str(row["assembly_graph_json"]))
        except json.JSONDecodeError as exc:
            raise ActiveDesignSnapshotError("Agent asset assembly graph is invalid") from exc
        if not isinstance(graph, dict) or graph.get("graph_id") != assembly_graph_id:
            raise ActiveDesignSnapshotError("active assembly graph no longer matches the active asset")
        parts = graph.get("parts")
        if not isinstance(parts, list):
            raise ActiveDesignSnapshotError("active assembly has no parts")
        return {
            str(item.get("part_id"))
            for item in parts
            if isinstance(item, dict) and isinstance(item.get("part_id"), str) and str(item.get("part_id")).startswith("part_")
        }

    def _part_display_for_asset(
        self,
        *,
        project_id: str,
        asset_version_id: str,
        assembly_graph_id: str,
        current: Any,
    ) -> Any:
        from forgecad_agent.application.agent_models import ActiveDesignPartDisplay

        part_ids = self._part_ids_for_asset(asset_version_id, assembly_graph_id)
        locked = set(current.locked_part_ids) if current is not None else set()
        hidden = set(current.hidden_part_ids) if current is not None else set()
        isolated = current.isolated_part_id if current is not None else None
        return ActiveDesignPartDisplay(
            project_id=project_id,
            asset_version_id=asset_version_id,
            locked_part_ids=sorted(locked & part_ids),
            hidden_part_ids=sorted(hidden & part_ids),
            isolated_part_id=isolated if isolated in part_ids and isolated not in hidden else None,
        )

    def _require_change_set(self, project_id: str, change_set_id: str, base_asset_version_id: Optional[str]) -> None:
        row = self.connection.execute(
            "SELECT project_id, base_asset_version_id FROM agent_asset_change_sets WHERE change_set_id = ?",
            (change_set_id,),
        ).fetchone()
        if row is None or str(row["project_id"]) != project_id or str(row["base_asset_version_id"]) != base_asset_version_id:
            raise ActiveDesignSnapshotError("preview change set does not belong to the active project/version")

    def _require_legacy_version(self, project_id: str, legacy_version_id: str, module_graph_id: str) -> None:
        self._require_active_project(project_id)
        version = self.connection.execute(
            "SELECT project_id, module_graph_id, status FROM project_versions WHERE version_id = ?",
            (legacy_version_id,),
        ).fetchone()
        graph = self.connection.execute(
            "SELECT project_id FROM module_graphs WHERE graph_id = ?",
            (module_graph_id,),
        ).fetchone()
        if (
            version is None
            or graph is None
            or str(version["project_id"]) != project_id
            or str(graph["project_id"]) != project_id
            or str(version["module_graph_id"] or "") != module_graph_id
            or str(version["status"]) == "soft_deleted"
        ):
            raise ActiveDesignSnapshotError("legacy version/graph does not belong to the active project")

    def _require_active_project(self, project_id: str) -> None:
        row = self.connection.execute(
            "SELECT project_id FROM projects WHERE project_id = ? AND status = 'active'",
            (project_id,),
        ).fetchone()
        if row is None:
            raise ActiveDesignSnapshotError("active design snapshot requires an active project")


def _snapshot_from_row(row: sqlite3.Row) -> "ActiveDesignSnapshot":
    from forgecad_agent.application.agent_models import (
        ActiveDesignExportReference,
        ActiveDesignPreviewReference,
        ActiveDesignQualityReference,
        ActiveDesignPartDisplay,
        ActiveDesignRenderPreset,
        ActiveDesignSnapshot,
        AgentActiveDesignReference,
        LegacyActiveDesignReference,
    )
    project_id = str(row["project_id"])
    source = str(row["source"])
    if source == "agent_asset":
        active_design = AgentActiveDesignReference(
            project_id=project_id,
            asset_version_id=str(row["active_asset_version_id"]),
            assembly_graph_id=str(row["active_assembly_graph_id"]),
        )
        preview = (
            ActiveDesignPreviewReference(
                project_id=project_id,
                change_set_id=str(row["preview_change_set_id"]),
                base_asset_version_id=str(row["preview_base_asset_version_id"]),
            )
            if row["preview_change_set_id"] is not None
            else None
        )
        quality = (
            ActiveDesignQualityReference(
                project_id=project_id,
                quality_report_id=str(row["quality_report_id"]),
                asset_version_id=str(row["quality_asset_version_id"]),
            )
            if row["quality_report_id"] is not None
            else None
        )
    else:
        active_design = LegacyActiveDesignReference(
            project_id=project_id,
            legacy_version_id=str(row["legacy_version_id"]),
            module_graph_id=str(row["legacy_module_graph_id"]),
        )
        preview = None
        quality = None
    render_preset = None
    raw_render_preset = row["render_preset_json"] if "render_preset_json" in row.keys() else None
    if raw_render_preset:
        try:
            render_preset = ActiveDesignRenderPreset.model_validate_json(str(raw_render_preset))
        except (TypeError, ValueError) as exc:
            raise ActiveDesignSnapshotError("stored active design render preset is invalid") from exc
    part_display = None
    raw_part_display = row["part_display_json"] if "part_display_json" in row.keys() else None
    if raw_part_display:
        try:
            part_display = ActiveDesignPartDisplay.model_validate_json(str(raw_part_display))
        except (TypeError, ValueError) as exc:
            raise ActiveDesignSnapshotError("stored active design part display is invalid") from exc
    return ActiveDesignSnapshot(
        project_id=project_id,
        active_design=active_design,
        selected_part_id=row["selected_part_id"],
        selected_material_zone_id=row["selected_material_zone_id"],
        preview=preview,
        quality=quality,
        render_preset=render_preset,
        part_display=part_display,
        export=ActiveDesignExportReference(
            source=str(row["export_source"]),
            project_id=project_id,
            source_version_id=str(row["export_source_version_id"]),
        ),
        revision=int(row["revision"]),
        updated_at=str(row["updated_at"]),
    )


def _default_render_preset_json(project_id: str, asset_version_id: str, updated_at: str) -> str:
    return _render_preset_json(
        project_id=project_id,
        asset_version_id=asset_version_id,
        camera_view="iso",
        light_preset="cad_neutral",
        updated_at=updated_at,
    )


def _default_part_display_json(project_id: str, asset_version_id: str) -> str:
    return _canonical_json(
        {
            "schema_version": "ActiveDesignPartDisplay@1",
            "project_id": project_id,
            "asset_version_id": asset_version_id,
            "locked_part_ids": [],
            "hidden_part_ids": [],
            "isolated_part_id": None,
        }
    )


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _render_preset_json(
    *,
    project_id: str,
    asset_version_id: str,
    camera_view: str,
    light_preset: str,
    updated_at: str,
) -> str:
    return json.dumps(
        {
            "schema_version": "ActiveDesignRenderPreset@1",
            "preset_id": f"render_{asset_version_id}_{camera_view}_{light_preset}",
            "project_id": project_id,
            "asset_version_id": asset_version_id,
            "camera_view": camera_view,
            "light_preset": light_preset,
            "updated_at": updated_at,
        },
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    )


class AgentKernelRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add_thread(
        self,
        *,
        thread_id: str,
        project_id: Optional[str],
        title: str,
        provider_id: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_threads (
              thread_id, project_id, title, status, summary, provider_id,
              created_at, updated_at, last_turn_id
            ) VALUES (?, ?, ?, 'idle', '', ?, ?, ?, NULL)
            """,
            (thread_id, project_id, title, provider_id, created_at, created_at),
        )

    def get_thread(self, thread_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT thread_id, project_id, title, status, summary, provider_id,
                   created_at, updated_at, last_turn_id
            FROM agent_threads WHERE thread_id = ?
            """,
            (thread_id,),
        ).fetchone()

    def list_threads(self, limit: int = 100) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT thread_id, project_id, title, status, summary, provider_id,
                   created_at, updated_at, last_turn_id
            FROM agent_threads
            WHERE status != 'archived'
            ORDER BY updated_at DESC, thread_id DESC
            LIMIT ?
            """,
            (limit,),
        ).fetchall()

    def update_thread(
        self,
        *,
        thread_id: str,
        status: str,
        summary: Optional[str],
        last_turn_id: Optional[str],
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE agent_threads
            SET status = ?, summary = COALESCE(?, summary),
                last_turn_id = COALESCE(?, last_turn_id), updated_at = ?
            WHERE thread_id = ?
            """,
            (status, summary, last_turn_id, updated_at, thread_id),
        )

    def add_turn(
        self,
        *,
        turn_id: str,
        thread_id: str,
        request_text: str,
        status: str,
        created_at: str,
        context_hash: Optional[str] = None,
        prompt_contract_version: Optional[str] = None,
        provider_request_fingerprint: Optional[str] = None,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_turns (
              turn_id, thread_id, request_text, status, usage_json,
              context_hash, prompt_contract_version, provider_request_fingerprint,
              created_at, updated_at
            ) VALUES (?, ?, ?, ?, '{}', ?, ?, ?, ?, ?)
            """,
            (
                turn_id, thread_id, request_text, status, context_hash,
                prompt_contract_version, provider_request_fingerprint,
                created_at, created_at,
            ),
        )

    def get_turn(self, turn_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT turn_id, thread_id, request_text, status, error_code,
                   error_message, usage_json, context_hash, prompt_contract_version,
                   provider_request_fingerprint, created_at, updated_at
            FROM agent_turns WHERE turn_id = ?
            """,
            (turn_id,),
        ).fetchone()

    def update_turn(
        self,
        *,
        turn_id: str,
        status: str,
        updated_at: str,
        error_code: Optional[str] = None,
        error_message: Optional[str] = None,
        usage: Optional[Mapping[str, Any]] = None,
    ) -> None:
        self.connection.execute(
            """
            UPDATE agent_turns
            SET status = ?, error_code = ?, error_message = ?,
                usage_json = COALESCE(?, usage_json), updated_at = ?
            WHERE turn_id = ?
            """,
            (
                status,
                error_code,
                error_message,
                json.dumps(usage, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
                if usage is not None
                else None,
                updated_at,
                turn_id,
            ),
        )

    def next_sequence(self, thread_id: str) -> int:
        row = self.connection.execute(
            "SELECT COALESCE(MAX(sequence), 0) + 1 AS next_sequence FROM agent_items WHERE thread_id = ?",
            (thread_id,),
        ).fetchone()
        return int(row["next_sequence"])

    def add_item(
        self,
        *,
        item_id: str,
        thread_id: str,
        turn_id: str,
        sequence: int,
        item_type: str,
        status: str,
        payload: Mapping[str, Any],
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_items (
              item_id, thread_id, turn_id, sequence, item_type, status,
              payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                item_id,
                thread_id,
                turn_id,
                sequence,
                item_type,
                status,
                json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                created_at,
            ),
        )

    def list_items(self, thread_id: str, *, after: int = 0) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT item_id, thread_id, turn_id, sequence, item_type, status,
                   payload_json, created_at
            FROM agent_items
            WHERE thread_id = ? AND sequence > ?
            ORDER BY sequence ASC
            """,
            (thread_id, after),
        ).fetchall()

    def update_item_status(self, item_id: str, *, status: str) -> None:
        self.connection.execute(
            "UPDATE agent_items SET status = ? WHERE item_id = ?",
            (status, item_id),
        )

    def list_turns(self, thread_id: str) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT turn_id, thread_id, request_text, status, error_code,
                   error_message, usage_json, context_hash, prompt_contract_version,
                   provider_request_fingerprint, created_at, updated_at
            FROM agent_turns WHERE thread_id = ?
            ORDER BY created_at ASC, turn_id ASC
            """,
            (thread_id,),
        ).fetchall()

    def has_in_flight_turn(self, thread_id: str) -> bool:
        row = self.connection.execute(
            """
            SELECT 1 FROM agent_turns
            WHERE thread_id = ? AND status IN ('queued', 'running')
            LIMIT 1
            """,
            (thread_id,),
        ).fetchone()
        return row is not None

    def latest_memory_summary(self, thread_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT summary_id, thread_id, up_to_sequence, summary_text,
                   domain_pack_id, snapshot_fingerprint, prompt_contract_version, created_at
            FROM agent_thread_memory_summaries
            WHERE thread_id = ?
            ORDER BY up_to_sequence DESC, summary_id DESC
            LIMIT 1
            """,
            (thread_id,),
        ).fetchone()

    def add_memory_summary(
        self,
        *,
        summary_id: str,
        thread_id: str,
        up_to_sequence: int,
        summary_text: str,
        domain_pack_id: Optional[str],
        snapshot_fingerprint: Optional[str],
        prompt_contract_version: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_thread_memory_summaries (
              summary_id, thread_id, up_to_sequence, summary_text, domain_pack_id,
              snapshot_fingerprint, prompt_contract_version, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                summary_id, thread_id, up_to_sequence, summary_text, domain_pack_id,
                snapshot_fingerprint, prompt_contract_version, created_at,
            ),
        )

    def reserve_daily_budget(
        self,
        *,
        day_utc: str,
        provider_id: str,
        budget_micros: int,
        reservation_micros: int,
        updated_at: str,
    ) -> bool:
        self.connection.execute(
            """
            INSERT INTO agent_provider_daily_budgets (
              day_utc, provider_id, budget_micros, updated_at
            ) VALUES (?, ?, ?, ?)
            ON CONFLICT(day_utc, provider_id) DO NOTHING
            """,
            (day_utc, provider_id, budget_micros, updated_at),
        )
        row = self.connection.execute(
            """
            SELECT budget_micros, spent_micros, reserved_micros, unmetered_turns
            FROM agent_provider_daily_budgets
            WHERE day_utc = ? AND provider_id = ?
            """,
            (day_utc, provider_id),
        ).fetchone()
        if row is None or int(row["unmetered_turns"]) > 0:
            return False
        if int(row["spent_micros"]) + int(row["reserved_micros"]) + reservation_micros > int(row["budget_micros"]):
            return False
        self.connection.execute(
            """
            UPDATE agent_provider_daily_budgets
            SET reserved_micros = reserved_micros + ?, updated_at = ?
            WHERE day_utc = ? AND provider_id = ?
            """,
            (reservation_micros, updated_at, day_utc, provider_id),
        )
        return True

    def settle_daily_budget(
        self,
        *,
        day_utc: str,
        provider_id: str,
        reservation_micros: int,
        actual_micros: Optional[int],
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE agent_provider_daily_budgets
            SET reserved_micros = MAX(0, reserved_micros - ?),
                spent_micros = spent_micros + COALESCE(?, 0),
                unmetered_turns = unmetered_turns + CASE WHEN ? IS NULL THEN 1 ELSE 0 END,
                updated_at = ?
            WHERE day_utc = ? AND provider_id = ?
            """,
            (reservation_micros, actual_micros, actual_micros, updated_at, day_utc, provider_id),
        )

    def add_approval(
        self,
        *,
        approval_id: str,
        thread_id: str,
        turn_id: str,
        item_id: str,
        action: str,
        payload: Mapping[str, Any],
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_approvals (
              approval_id, thread_id, turn_id, item_id, action, status,
              payload_json, created_at, resolved_at
            ) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, NULL)
            """,
            (
                approval_id,
                thread_id,
                turn_id,
                item_id,
                action,
                json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                created_at,
            ),
        )

    def get_approval(self, approval_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT approval_id, thread_id, turn_id, item_id, action, status,
                   payload_json, created_at, resolved_at
            FROM agent_approvals WHERE approval_id = ?
            """,
            (approval_id,),
        ).fetchone()

    def resolve_approval(self, approval_id: str, *, status: str, resolved_at: str) -> None:
        self.connection.execute(
            """
            UPDATE agent_approvals SET status = ?, resolved_at = ?
            WHERE approval_id = ? AND status = 'pending'
            """,
            (status, resolved_at, approval_id),
        )

    def list_approvals(self, thread_id: str) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT approval_id, thread_id, turn_id, item_id, action, status,
                   payload_json, created_at, resolved_at
            FROM agent_approvals WHERE thread_id = ?
            ORDER BY created_at ASC, approval_id ASC
            """,
            (thread_id,),
        ).fetchall()


class AgentAssetRepository:
    """Persistence for the general mechanical Agent asset/editing slice.

    This repository intentionally has no dependency on the legacy
    WeaponConcept ModuleGraph tables. Candidate previews and confirmed Agent
    asset versions are immutable JSON snapshots; only the head pointer and
    change-set state are mutable workflow metadata.
    """

    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add_candidate(
        self,
        *,
        artifact_id: str,
        project_id: Optional[str],
        plan_id: str,
        direction_id: str,
        domain_pack_id: str,
        candidate_json: str,
        shape_program_json: str,
        assembly_graph_json: str,
        material_bindings_json: str,
        glb_base64: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_blockout_candidates (
              artifact_id, project_id, plan_id, direction_id, domain_pack_id,
              status, candidate_json, shape_program_json, assembly_graph_json,
              material_bindings_json, glb_base64, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, 'candidate', ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(artifact_id) DO UPDATE SET
              project_id = excluded.project_id,
              candidate_json = excluded.candidate_json,
              shape_program_json = excluded.shape_program_json,
              assembly_graph_json = excluded.assembly_graph_json,
              material_bindings_json = excluded.material_bindings_json,
              glb_base64 = excluded.glb_base64,
              updated_at = excluded.updated_at
            """,
            (
                artifact_id,
                project_id,
                plan_id,
                direction_id,
                domain_pack_id,
                candidate_json,
                shape_program_json,
                assembly_graph_json,
                material_bindings_json,
                glb_base64,
                created_at,
                created_at,
            ),
        )

    def get_candidate(self, artifact_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT artifact_id, project_id, plan_id, direction_id, domain_pack_id,
                   status, candidate_json, shape_program_json, assembly_graph_json,
                   material_bindings_json, glb_base64, created_at, updated_at
            FROM agent_blockout_candidates
            WHERE artifact_id = ?
            """,
            (artifact_id,),
        ).fetchone()

    def mark_candidate(self, artifact_id: str, *, status: str, updated_at: str) -> None:
        self.connection.execute(
            "UPDATE agent_blockout_candidates SET status = ?, updated_at = ? WHERE artifact_id = ?",
            (status, updated_at, artifact_id),
        )

    def next_version_number(self, project_id: str) -> int:
        row = self.connection.execute(
            "SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no FROM agent_asset_versions WHERE project_id = ?",
            (project_id,),
        ).fetchone()
        return int(row["next_version_no"])

    def add_version(
        self,
        *,
        asset_version_id: str,
        project_id: str,
        parent_asset_version_id: Optional[str],
        version_no: int,
        status: str,
        summary: str,
        stage: str,
        plan_id: str,
        direction_id: str,
        domain_pack_id: str,
        artifact_id: str,
        parts_json: str,
        shape_program_json: str,
        assembly_graph_json: str,
        material_bindings_json: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_asset_versions (
              asset_version_id, project_id, parent_asset_version_id, version_no,
              status, summary, stage, plan_id, direction_id, domain_pack_id,
              artifact_id, parts_json, shape_program_json, assembly_graph_json,
              material_bindings_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                asset_version_id,
                project_id,
                parent_asset_version_id,
                version_no,
                status,
                summary,
                stage,
                plan_id,
                direction_id,
                domain_pack_id,
                artifact_id,
                parts_json,
                shape_program_json,
                assembly_graph_json,
                material_bindings_json,
                created_at,
            ),
        )

    def get_version(self, asset_version_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT asset_version_id, project_id, parent_asset_version_id, version_no,
                   status, summary, stage, plan_id, direction_id, domain_pack_id,
                   artifact_id, parts_json, shape_program_json, assembly_graph_json,
                   material_bindings_json, created_at
            FROM agent_asset_versions
            WHERE asset_version_id = ? AND status != 'soft_deleted'
            """,
            (asset_version_id,),
        ).fetchone()

    def add_quality_report(
        self,
        *,
        quality_report_id: str,
        project_id: str,
        asset_version_id: str,
        report_json: str,
        status: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_asset_quality_reports (
              quality_report_id, project_id, asset_version_id, report_json, status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            """,
            (quality_report_id, project_id, asset_version_id, report_json, status, created_at),
        )

    def get_quality_report(self, quality_report_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT quality_report_id, project_id, asset_version_id, report_json, status, created_at
            FROM agent_asset_quality_reports
            WHERE quality_report_id = ?
            """,
            (quality_report_id,),
        ).fetchone()

    def list_versions(self, project_id: str, *, limit: int = 50) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT asset_version_id, project_id, parent_asset_version_id, version_no,
                   status, summary, stage, plan_id, direction_id, domain_pack_id,
                   artifact_id, parts_json, shape_program_json, assembly_graph_json,
                   material_bindings_json, created_at
            FROM agent_asset_versions
            WHERE project_id = ? AND status != 'soft_deleted'
            ORDER BY version_no DESC, asset_version_id DESC
            LIMIT ?
            """,
            (project_id, limit),
        ).fetchall()

    def set_head(self, *, project_id: str, asset_version_id: str, updated_at: str) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_asset_heads (project_id, asset_version_id, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(project_id) DO UPDATE SET
              asset_version_id = excluded.asset_version_id,
              updated_at = excluded.updated_at
            """,
            (project_id, asset_version_id, updated_at),
        )

    def get_head(self, project_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT project_id, asset_version_id, updated_at
            FROM agent_asset_heads
            WHERE project_id = ?
            """,
            (project_id,),
        ).fetchone()

    def supersede(self, asset_version_id: str) -> None:
        self.connection.execute(
            "UPDATE agent_asset_versions SET status = 'superseded' WHERE asset_version_id = ? AND status = 'committed'",
            (asset_version_id,),
        )

    def add_navigation_frame(
        self,
        *,
        resulting_asset_version_id: str,
        project_id: str,
        undo_target_asset_version_id: Optional[str],
        redo_target_asset_version_id: Optional[str],
        action: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_asset_navigation_frames (
              resulting_asset_version_id, project_id, undo_target_asset_version_id,
              redo_target_asset_version_id, action, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            """,
            (
                resulting_asset_version_id,
                project_id,
                undo_target_asset_version_id,
                redo_target_asset_version_id,
                action,
                created_at,
            ),
        )

    def get_navigation_frame(self, resulting_asset_version_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT resulting_asset_version_id, project_id, undo_target_asset_version_id,
                   redo_target_asset_version_id, action, created_at
            FROM agent_asset_navigation_frames
            WHERE resulting_asset_version_id = ?
            """,
            (resulting_asset_version_id,),
        ).fetchone()

    def add_change_set(
        self,
        *,
        change_set_id: str,
        project_id: str,
        base_asset_version_id: str,
        summary: str,
        operations_json: str,
        protected_part_ids_json: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_asset_change_sets (
              change_set_id, project_id, base_asset_version_id, summary,
              operations_json, protected_part_ids_json, preview_json, status, resulting_asset_version_id,
              created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, NULL, 'proposed', NULL, ?, ?)
            """,
            (
                change_set_id,
                project_id,
                base_asset_version_id,
                summary,
                operations_json,
                protected_part_ids_json,
                created_at,
                created_at,
            ),
        )

    def get_change_set(self, change_set_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT change_set_id, project_id, base_asset_version_id, summary,
                   operations_json, protected_part_ids_json, preview_json, status, resulting_asset_version_id,
                   created_at, updated_at
            FROM agent_asset_change_sets
            WHERE change_set_id = ?
            """,
            (change_set_id,),
        ).fetchone()

    def update_change_set(
        self,
        change_set_id: str,
        *,
        status: str,
        preview_json: Optional[str],
        resulting_asset_version_id: Optional[str],
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE agent_asset_change_sets
            SET status = ?, preview_json = COALESCE(?, preview_json),
                resulting_asset_version_id = COALESCE(?, resulting_asset_version_id),
                updated_at = ?
            WHERE change_set_id = ?
            """,
            (status, preview_json, resulting_asset_version_id, updated_at, change_set_id),
        )

    def add_component(
        self,
        *,
        component_id: str,
        project_id: str,
        domain_pack_id: str,
        role: str,
        display_name: str,
        description: str,
        source_asset_version_id: str,
        source_part_id: str,
        part_template_json: str,
        shape_operation_json: str,
        material_bindings_json: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_components (
              component_id, project_id, domain_pack_id, role, display_name,
              description, source_asset_version_id, source_part_id,
              part_template_json, shape_operation_json, material_bindings_json,
              status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
            """,
            (component_id, project_id, domain_pack_id, role, display_name, description,
             source_asset_version_id, source_part_id, part_template_json,
             shape_operation_json, material_bindings_json, created_at, created_at),
        )

    def get_component(self, component_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT component_id, project_id, domain_pack_id, role, display_name,
                   description, source_asset_version_id, source_part_id,
                   part_template_json, shape_operation_json, material_bindings_json,
                   status,
                   COALESCE((
                     SELECT report.status
                     FROM agent_asset_quality_reports AS report
                     WHERE report.asset_version_id = agent_components.source_asset_version_id
                       AND json_extract(report.report_json, '$.evidence_source') IN (
                         'geometry_compile_readback', 'external_glb_inspection'
                       )
                     ORDER BY report.created_at DESC, report.quality_report_id DESC
                     LIMIT 1
                   ), 'unavailable') AS source_quality_status,
                   created_at, updated_at
            FROM agent_components WHERE component_id = ?
            """,
            (component_id,),
        ).fetchone()

    def list_components(
        self,
        project_id: str,
        *,
        domain_pack_id: Optional[str] = None,
        role: Optional[str] = None,
        query: Optional[str] = None,
        include_disabled: bool = False,
        limit: int = 100,
    ) -> list[sqlite3.Row]:
        clauses = ["project_id = ?"]
        if not include_disabled:
            clauses.append("status = 'active'")
        args: list[Any] = [project_id]
        if domain_pack_id:
            clauses.append("domain_pack_id = ?")
            args.append(domain_pack_id)
        if role:
            clauses.append("role = ?")
            args.append(role)
        if query:
            clauses.append("(display_name LIKE ? OR description LIKE ? OR role LIKE ?)")
            needle = f"%{query}%"
            args.extend([needle, needle, needle])
        args.append(limit)
        return self.connection.execute(
            f"""
            SELECT component_id, project_id, domain_pack_id, role, display_name,
                   description, source_asset_version_id, source_part_id,
                   part_template_json, shape_operation_json, material_bindings_json,
                   status,
                   COALESCE((
                     SELECT report.status
                     FROM agent_asset_quality_reports AS report
                     WHERE report.asset_version_id = agent_components.source_asset_version_id
                       AND json_extract(report.report_json, '$.evidence_source') IN (
                         'geometry_compile_readback', 'external_glb_inspection'
                       )
                     ORDER BY report.created_at DESC, report.quality_report_id DESC
                     LIMIT 1
                   ), 'unavailable') AS source_quality_status,
                   created_at, updated_at
            FROM agent_components
            WHERE {' AND '.join(clauses)}
            ORDER BY updated_at DESC, component_id DESC
            LIMIT ?
            """,
            tuple(args),
        ).fetchall()

    def add_imported_glb(
        self,
        *,
        import_id: str,
        project_id: str,
        asset_version_id: str,
        domain_pack_id: str,
        file_name: str,
        object_path: str,
        sha256: str,
        byte_size: int,
        triangle_count: int,
        bounds_mm_json: str,
        mesh_count: int,
        primitive_count: int,
        material_count: int,
        node_count: int,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO agent_imported_glbs (
              import_id, project_id, asset_version_id, domain_pack_id, file_name,
              object_path, sha256, byte_size, triangle_count, bounds_mm_json,
              mesh_count, primitive_count, material_count, node_count, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                import_id, project_id, asset_version_id, domain_pack_id, file_name,
                object_path, sha256, byte_size, triangle_count, bounds_mm_json,
                mesh_count, primitive_count, material_count, node_count, created_at,
            ),
        )

    def get_imported_glb(self, asset_version_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT import_id, project_id, asset_version_id, domain_pack_id, file_name,
                   object_path, sha256, byte_size, triangle_count, bounds_mm_json,
                   mesh_count, primitive_count, material_count, node_count, created_at
            FROM agent_imported_glbs
            WHERE asset_version_id = ?
            """,
            (asset_version_id,),
        ).fetchone()
