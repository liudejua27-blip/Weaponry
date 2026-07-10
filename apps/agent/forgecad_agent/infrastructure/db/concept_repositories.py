from __future__ import annotations

import sqlite3
from typing import Optional


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
