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
            VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                graph_id,
                project_id,
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
