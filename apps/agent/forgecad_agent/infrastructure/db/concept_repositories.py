from __future__ import annotations

import sqlite3
from typing import Any, Mapping, Optional


class DomainProfileRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def upsert(
        self,
        *,
        profile_id: str,
        domain_type: str,
        schema_version: str,
        pack_id: str,
        display_name: str,
        profile_json: str,
        profile_sha256: str,
        status: str,
        created_at: str,
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO domain_profiles (
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(profile_id) DO UPDATE SET
              domain_type = excluded.domain_type,
              schema_version = excluded.schema_version,
              pack_id = excluded.pack_id,
              display_name = excluded.display_name,
              profile_json = excluded.profile_json,
              profile_sha256 = excluded.profile_sha256,
              status = excluded.status,
              updated_at = excluded.updated_at
            """,
            (
                profile_id,
                domain_type,
                schema_version,
                pack_id,
                display_name,
                profile_json,
                profile_sha256,
                status,
                created_at,
                updated_at,
            ),
        )

    def get_active(self, profile_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT profile_id, domain_type, schema_version, pack_id, display_name,
                   profile_json, profile_sha256, status, created_at, updated_at
            FROM domain_profiles
            WHERE profile_id = ? AND status = 'active'
            """,
            (profile_id,),
        ).fetchone()


class ConceptProjectRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add(
        self,
        *,
        project_id: str,
        profile_id: str,
        domain_type: str,
        name: str,
        status: str,
        created_at: str,
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO projects (
              project_id, profile_id, domain_type, name, status,
              current_version_id, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, NULL, ?, ?)
            """,
            (
                project_id,
                profile_id,
                domain_type,
                name,
                status,
                created_at,
                updated_at,
            ),
        )

    def get_active(self, project_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT project_id, profile_id, domain_type, name, status,
                   current_version_id, created_at, updated_at
            FROM projects
            WHERE project_id = ? AND status = 'active'
            """,
            (project_id,),
        ).fetchone()

    def list_active(self, *, limit: int = 100) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT project_id, profile_id, domain_type, name, status,
                   current_version_id, created_at, updated_at
            FROM projects
            WHERE status = 'active'
            ORDER BY updated_at DESC, project_id DESC
            LIMIT ?
            """,
            (limit,),
        ).fetchall()

    def add_version(
        self,
        *,
        version_id: str,
        project_id: str,
        parent_version_id: Optional[str],
        version_no: int,
        status: str,
        summary: str,
        spec_schema_version: str,
        spec_json: str,
        spec_sha256: str,
        module_graph_id: Optional[str],
        change_set_id: Optional[str],
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO project_versions (
              version_id, project_id, parent_version_id, version_no, status,
              summary, spec_schema_version, spec_json, spec_sha256,
              module_graph_id, change_set_id, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                version_id,
                project_id,
                parent_version_id,
                version_no,
                status,
                summary,
                spec_schema_version,
                spec_json,
                spec_sha256,
                module_graph_id,
                change_set_id,
                created_at,
            ),
        )

    def get_version(self, project_id: str, version_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT version_id, parent_version_id, version_no, status,
                   summary, spec_schema_version, spec_json, spec_sha256,
                   module_graph_id, change_set_id, created_at
            FROM project_versions
            WHERE project_id = ? AND version_id = ? AND status != 'soft_deleted'
            """,
            (project_id, version_id),
        ).fetchone()

    def find_version(self, version_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT version_id, project_id, parent_version_id, version_no, status,
                   summary, spec_schema_version, spec_json, spec_sha256,
                   module_graph_id, change_set_id, created_at
            FROM project_versions
            WHERE version_id = ? AND status != 'soft_deleted'
            """,
            (version_id,),
        ).fetchone()

    def list_versions(self, project_id: str) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT version_id, parent_version_id, version_no, status,
                   summary, spec_schema_version, spec_sha256,
                   module_graph_id, change_set_id, created_at
            FROM project_versions
            WHERE project_id = ? AND status != 'soft_deleted'
            ORDER BY version_no ASC
            """,
            (project_id,),
        ).fetchall()

    def next_version_number(self, project_id: str) -> int:
        row = self.connection.execute(
            """
            SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no
            FROM project_versions
            WHERE project_id = ?
            """,
            (project_id,),
        ).fetchone()
        return int(row["next_version_no"])

    def set_current_version(
        self,
        *,
        project_id: str,
        version_id: str,
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE projects
            SET current_version_id = ?, updated_at = ?
            WHERE project_id = ? AND status = 'active'
            """,
            (version_id, updated_at, project_id),
        )


class ConceptAssetRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add(
        self,
        *,
        asset_id: str,
        project_id: Optional[str],
        version_id: Optional[str],
        role: str,
        logical_path: str,
        object_path: str,
        sha256: str,
        byte_size: int,
        mime_type: str,
        metadata_json: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO concept_assets (
              asset_id, project_id, version_id, role, logical_path, object_path,
              sha256, byte_size, mime_type, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                asset_id,
                project_id,
                version_id,
                role,
                logical_path,
                object_path,
                sha256,
                byte_size,
                mime_type,
                metadata_json,
                created_at,
            ),
        )

    def get_active(self, asset_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT asset_id, project_id, version_id, role, logical_path, object_path,
                   sha256, byte_size, mime_type, metadata_json, created_at
            FROM concept_assets
            WHERE asset_id = ? AND soft_deleted_at IS NULL
            """,
            (asset_id,),
        ).fetchone()


class ModuleRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add_manifest(
        self,
        *,
        module_id: str,
        pack_id: str,
        category: str,
        asset_id: str,
        schema_version: str,
        manifest_json: str,
        manifest_sha256: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO module_assets (
              module_id, pack_id, category, asset_id, schema_version,
              manifest_json, manifest_sha256, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
            """,
            (
                module_id,
                pack_id,
                category,
                asset_id,
                schema_version,
                manifest_json,
                manifest_sha256,
                created_at,
                created_at,
            ),
        )

    def add_connector(
        self,
        *,
        connector_id: str,
        module_id: str,
        slot: str,
        connector_type: str,
        transform_json: str,
        scale_min: float,
        scale_max: float,
        exclusive: bool,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO module_connectors (
              connector_id, module_id, slot, connector_type, transform_json,
              scale_min, scale_max, exclusive, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                connector_id,
                module_id,
                slot,
                connector_type,
                transform_json,
                scale_min,
                scale_max,
                1 if exclusive else 0,
                created_at,
            ),
        )

    def get_manifest(self, module_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT ma.module_id, ma.pack_id, ma.category, ma.asset_id, ma.schema_version,
                   ma.manifest_json, ma.manifest_sha256, ma.status,
                   ca.logical_path, ca.object_path, ca.byte_size, ca.mime_type,
                   ma.created_at, ma.updated_at
            FROM module_assets ma
            JOIN concept_assets ca ON ca.asset_id = ma.asset_id
            WHERE ma.module_id = ? AND ma.status = 'active' AND ca.soft_deleted_at IS NULL
            """,
            (module_id,),
        ).fetchone()

    def list_manifests(
        self,
        *,
        pack_id: Optional[str] = None,
        category: Optional[str] = None,
        limit: int = 100,
    ) -> list[sqlite3.Row]:
        clauses = ["ma.status = 'active'", "ca.soft_deleted_at IS NULL"]
        parameters: list[Any] = []
        if pack_id:
            clauses.append("ma.pack_id = ?")
            parameters.append(pack_id)
        if category:
            clauses.append("ma.category = ?")
            parameters.append(category)
        parameters.append(limit)
        return self.connection.execute(
            f"""
            SELECT ma.module_id, ma.pack_id, ma.category, ma.asset_id, ma.schema_version,
                   ma.manifest_json, ma.manifest_sha256, ma.status,
                   ca.logical_path, ca.object_path, ca.byte_size, ca.mime_type,
                   ma.created_at, ma.updated_at
            FROM module_assets ma
            JOIN concept_assets ca ON ca.asset_id = ma.asset_id
            WHERE {' AND '.join(clauses)}
            ORDER BY ma.category ASC, ma.module_id ASC
            LIMIT ?
            """,
            parameters,
        ).fetchall()

    def connector_map(self, module_ids: list[str]) -> dict[str, sqlite3.Row]:
        if not module_ids:
            return {}
        placeholders = ",".join("?" for _ in module_ids)
        rows = self.connection.execute(
            f"""
            SELECT connector_id, module_id, slot, connector_type, transform_json,
                   scale_min, scale_max, exclusive
            FROM module_connectors
            WHERE module_id IN ({placeholders})
            """,
            module_ids,
        ).fetchall()
        return {str(row["connector_id"]): row for row in rows}

    def add_graph(
        self,
        *,
        graph_id: str,
        project_id: str,
        version_id: Optional[str],
        root_node_id: str,
        schema_version: str,
        graph_json: str,
        graph_sha256: str,
        validation_status: str,
        nodes: list[Mapping[str, Any]],
        edges: list[Mapping[str, Any]],
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO module_graphs (
              graph_id, project_id, version_id, root_node_id, schema_version,
              graph_json, graph_sha256, validation_status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                graph_id,
                project_id,
                version_id,
                root_node_id,
                schema_version,
                graph_json,
                graph_sha256,
                validation_status,
                created_at,
                created_at,
            ),
        )
        for node in nodes:
            self.connection.execute(
                """
                INSERT INTO module_graph_nodes (
                  graph_id, node_id, module_id, transform_json, locked, visible
                )
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                (
                    graph_id,
                    node["node_id"],
                    node["module_id"],
                    node["transform_json"],
                    1 if node["locked"] else 0,
                    1 if node["visible"] else 0,
                ),
            )
        for edge in edges:
            self.connection.execute(
                """
                INSERT INTO module_graph_edges (
                  graph_id, edge_id, from_node_id, from_connector_id,
                  to_node_id, to_connector_id, status
                )
                VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    graph_id,
                    edge["edge_id"],
                    edge["from_node_id"],
                    edge["from_connector_id"],
                    edge["to_node_id"],
                    edge["to_connector_id"],
                    edge["status"],
                ),
            )

    def get_graph(self, graph_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT graph_id, project_id, version_id, root_node_id, schema_version,
                   graph_json, graph_sha256, validation_status, created_at, updated_at
            FROM module_graphs
            WHERE graph_id = ?
            """,
            (graph_id,),
        ).fetchone()

    def bind_graph_to_version(self, graph_id: str, version_id: str, *, updated_at: str) -> None:
        self.connection.execute(
            """
            UPDATE module_graphs
            SET version_id = ?, updated_at = ?
            WHERE graph_id = ? AND validation_status = 'valid'
            """,
            (version_id, updated_at, graph_id),
        )


class ChangeSetRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add(
        self,
        *,
        change_set_id: str,
        project_id: str,
        base_version_id: str,
        schema_version: str,
        change_set_json: str,
        change_set_sha256: str,
        status: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO design_change_sets (
              change_set_id, project_id, base_version_id, result_version_id,
              schema_version, change_set_json, change_set_sha256, status,
              created_at, updated_at
            )
            VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?)
            """,
            (
                change_set_id,
                project_id,
                base_version_id,
                schema_version,
                change_set_json,
                change_set_sha256,
                status,
                created_at,
                created_at,
            ),
        )

    def get(self, change_set_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT change_set_id, project_id, base_version_id, result_version_id,
                   schema_version, change_set_json, change_set_sha256, status,
                   preview_spec_json, preview_graph_json, preview_sha256,
                   created_at, updated_at, confirmed_at
            FROM design_change_sets
            WHERE change_set_id = ?
            """,
            (change_set_id,),
        ).fetchone()

    def save_preview(
        self,
        *,
        change_set_id: str,
        change_set_json: str,
        preview_spec_json: str,
        preview_graph_json: str,
        preview_sha256: str,
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE design_change_sets
            SET change_set_json = ?, status = 'previewed',
                preview_spec_json = ?, preview_graph_json = ?,
                preview_sha256 = ?, updated_at = ?
            WHERE change_set_id = ?
            """,
            (
                change_set_json,
                preview_spec_json,
                preview_graph_json,
                preview_sha256,
                updated_at,
                change_set_id,
            ),
        )

    def confirm(
        self,
        *,
        change_set_id: str,
        change_set_json: str,
        result_version_id: str,
        confirmed_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE design_change_sets
            SET change_set_json = ?, status = 'confirmed',
                result_version_id = ?, confirmed_at = ?, updated_at = ?
            WHERE change_set_id = ?
            """,
            (
                change_set_json,
                result_version_id,
                confirmed_at,
                confirmed_at,
                change_set_id,
            ),
        )

    def mark_stale(
        self,
        change_set_id: str,
        *,
        change_set_json: str,
        updated_at: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE design_change_sets
            SET change_set_json = ?, status = 'stale', updated_at = ?
            WHERE change_set_id = ?
            """,
            (change_set_json, updated_at, change_set_id),
        )


class QualityRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add_report(
        self,
        *,
        quality_run_id: str,
        project_id: str,
        version_id: str,
        ruleset_version: str,
        status: str,
        report_json: str,
        findings: list[Mapping[str, Any]],
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO quality_runs (
              quality_run_id, project_id, version_id, report_asset_id,
              ruleset_version, status, report_json, created_at
            )
            VALUES (?, ?, ?, NULL, ?, ?, ?, ?)
            """,
            (
                quality_run_id,
                project_id,
                version_id,
                ruleset_version,
                status,
                report_json,
                created_at,
            ),
        )
        for finding in findings:
            self.connection.execute(
                """
                INSERT INTO quality_findings (
                  finding_id, quality_run_id, check_id, category, severity,
                  status, node_ids_json, measured_value_json, threshold_json,
                  message, suggestion
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    finding["finding_id"],
                    quality_run_id,
                    finding["check_id"],
                    finding["category"],
                    finding["severity"],
                    finding["status"],
                    finding["node_ids_json"],
                    finding["measured_value_json"],
                    finding["threshold_json"],
                    finding["message"],
                    finding["suggestion"],
                ),
            )

    def get_report(self, quality_run_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT quality_run_id, project_id, version_id, report_asset_id,
                   ruleset_version, status, report_json, created_at
            FROM quality_runs
            WHERE quality_run_id = ?
            """,
            (quality_run_id,),
        ).fetchone()

    def latest_report(self, version_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT quality_run_id, project_id, version_id, report_asset_id,
                   ruleset_version, status, report_json, created_at
            FROM quality_runs
            WHERE version_id = ?
            ORDER BY created_at DESC, quality_run_id DESC
            LIMIT 1
            """,
            (version_id,),
        ).fetchone()


class ExportRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add(
        self,
        *,
        export_id: str,
        project_id: str,
        version_id: str,
        profile: str,
        package_asset_id: str,
        manifest_json: str,
        status: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO export_packages_v2 (
              export_id, project_id, version_id, profile, package_asset_id,
              manifest_json, status, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                export_id,
                project_id,
                version_id,
                profile,
                package_asset_id,
                manifest_json,
                status,
                created_at,
            ),
        )

    def add_artifact_link(
        self,
        *,
        project_id: str,
        version_id: str,
        asset_id: str,
        relation: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO artifact_links (
              project_id, version_id, asset_id, relation, created_at
            )
            VALUES (?, ?, ?, ?, ?)
            """,
            (project_id, version_id, asset_id, relation, created_at),
        )

    def get(self, export_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT ep.export_id, ep.project_id, ep.version_id, ep.profile,
                   ep.package_asset_id, ep.manifest_json, ep.status, ep.created_at,
                   ca.logical_path, ca.object_path, ca.sha256 AS package_sha256,
                   ca.byte_size AS package_byte_size, ca.mime_type,
                   (
                     SELECT cj.job_id
                     FROM concept_jobs cj
                     WHERE json_extract(cj.output_json, '$.export_id') = ep.export_id
                     ORDER BY cj.created_at DESC
                     LIMIT 1
                   ) AS job_id
            FROM export_packages_v2 ep
            JOIN concept_assets ca ON ca.asset_id = ep.package_asset_id
            WHERE ep.export_id = ? AND ep.status != 'soft_deleted'
              AND ca.soft_deleted_at IS NULL
            """,
            (export_id,),
        ).fetchone()


class BriefVariantRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add_brief(
        self,
        *,
        brief_id: str,
        project_id: str,
        source_text: str,
        reference_asset_ids_json: str,
        interpreted_spec_json: str,
        status: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO design_briefs (
              brief_id, project_id, source_text, reference_asset_ids_json,
              interpreted_spec_json, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                brief_id,
                project_id,
                source_text,
                reference_asset_ids_json,
                interpreted_spec_json,
                status,
                created_at,
                created_at,
            ),
        )

    def get_brief(self, project_id: str, brief_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT brief_id, project_id, source_text, reference_asset_ids_json,
                   interpreted_spec_json, status, created_at, updated_at
            FROM design_briefs
            WHERE project_id = ? AND brief_id = ?
            """,
            (project_id, brief_id),
        ).fetchone()

    def add_variant(
        self,
        *,
        variant_id: str,
        project_id: str,
        brief_id: str,
        rank: int,
        name: str,
        summary: str,
        module_graph_json: str,
        status: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO design_variants (
              variant_id, project_id, brief_id, rank, name, summary,
              module_graph_json, status, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                variant_id,
                project_id,
                brief_id,
                rank,
                name,
                summary,
                module_graph_json,
                status,
                created_at,
            ),
        )

    def list_variants(
        self,
        project_id: str,
        *,
        brief_id: Optional[str] = None,
    ) -> list[sqlite3.Row]:
        if brief_id:
            return self.connection.execute(
                """
                SELECT variant_id, project_id, brief_id, rank, name, summary,
                       module_graph_json, status, created_at
                FROM design_variants
                WHERE project_id = ? AND brief_id = ?
                ORDER BY rank ASC
                """,
                (project_id, brief_id),
            ).fetchall()
        return self.connection.execute(
            """
            SELECT variant_id, project_id, brief_id, rank, name, summary,
                   module_graph_json, status, created_at
            FROM design_variants
            WHERE project_id = ?
            ORDER BY created_at DESC, rank ASC
            """,
            (project_id,),
        ).fetchall()

    def get_variant(self, project_id: str, variant_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT variant_id, project_id, brief_id, rank, name, summary,
                   module_graph_json, status, created_at
            FROM design_variants
            WHERE project_id = ? AND variant_id = ?
            """,
            (project_id, variant_id),
        ).fetchone()

    def select_variant(
        self,
        *,
        project_id: str,
        brief_id: str,
        variant_id: str,
    ) -> None:
        self.connection.execute(
            """
            UPDATE design_variants
            SET status = CASE WHEN variant_id = ? THEN 'selected' ELSE 'rejected' END
            WHERE project_id = ? AND brief_id = ?
            """,
            (variant_id, project_id, brief_id),
        )
        self.connection.execute(
            """
            UPDATE design_briefs
            SET status = 'confirmed', updated_at = datetime('now')
            WHERE project_id = ? AND brief_id = ?
            """,
            (project_id, brief_id),
        )


class ConceptJobRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def add_job(
        self,
        *,
        job_id: str,
        project_id: str,
        version_id: Optional[str],
        job_type: str,
        status: str,
        current_step: str,
        input_hash: str,
        input_json: str,
        output_json: str,
        created_at: str,
        finished_at: Optional[str],
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO concept_jobs (
              job_id, project_id, version_id, job_type, status, current_step,
              input_hash, input_json, output_json, created_at, updated_at, finished_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                job_id,
                project_id,
                version_id,
                job_type,
                status,
                current_step,
                input_hash,
                input_json,
                output_json,
                created_at,
                created_at,
                finished_at,
            ),
        )

    def add_event(
        self,
        *,
        event_id: str,
        job_id: str,
        seq: int,
        project_id: str,
        version_id: Optional[str],
        step: str,
        level: str,
        status: str,
        message: str,
        progress: float,
        artifact_asset_id: Optional[str],
        metadata_json: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO concept_job_events (
              event_id, job_id, seq, project_id, version_id, step, level,
              status, message, progress, artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                event_id,
                job_id,
                seq,
                project_id,
                version_id,
                step,
                level,
                status,
                message,
                progress,
                artifact_asset_id,
                metadata_json,
                created_at,
            ),
        )

    def get_job(self, job_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT job_id, project_id, version_id, job_type, status, current_step,
                   input_hash, input_json, output_json, error_code, error_message,
                   created_at, updated_at, finished_at
            FROM concept_jobs
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()

    def event_seq(self, job_id: str, event_id: str) -> Optional[int]:
        row = self.connection.execute(
            """
            SELECT seq FROM concept_job_events
            WHERE job_id = ? AND event_id = ?
            """,
            (job_id, event_id),
        ).fetchone()
        return int(row["seq"]) if row is not None else None

    def events(
        self,
        job_id: str,
        *,
        after_seq: Optional[int] = None,
    ) -> list[sqlite3.Row]:
        if after_seq is None:
            return self.connection.execute(
                """
                SELECT event_id, job_id, seq, project_id, version_id, step,
                       level, status, message, progress, artifact_asset_id,
                       metadata_json, created_at
                FROM concept_job_events
                WHERE job_id = ?
                ORDER BY seq ASC
                """,
                (job_id,),
            ).fetchall()
        return self.connection.execute(
            """
            SELECT event_id, job_id, seq, project_id, version_id, step,
                   level, status, message, progress, artifact_asset_id,
                   metadata_json, created_at
            FROM concept_job_events
            WHERE job_id = ? AND seq > ?
            ORDER BY seq ASC
            """,
            (job_id, after_seq),
        ).fetchall()
