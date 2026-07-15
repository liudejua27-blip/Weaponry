from __future__ import annotations

import sqlite3
from typing import Optional


class MaterialTextureRepository:
    """SQLite metadata for immutable, content-addressed visual textures."""

    _COLUMNS = """
        texture_asset_id, texture_role, display_name, mime_type, byte_size,
        sha256, object_path, width, height, source, license, license_ref,
        thumbnail_asset_id, visual_only, created_at, updated_at
    """

    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def get(self, texture_asset_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            f"SELECT {self._COLUMNS} FROM agent_material_texture_objects WHERE texture_asset_id = ?",
            (texture_asset_id,),
        ).fetchone()

    def get_by_sha_role(self, sha256: str, texture_role: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            f"""
            SELECT {self._COLUMNS}
            FROM agent_material_texture_objects
            WHERE sha256 = ? AND texture_role = ?
            """,
            (sha256, texture_role),
        ).fetchone()

    def insert(
        self,
        *,
        texture_asset_id: str,
        texture_role: str,
        display_name: str,
        mime_type: str,
        byte_size: int,
        sha256: str,
        object_path: str,
        width: int,
        height: int,
        source: str,
        license: str,
        license_ref: Optional[str],
        thumbnail_asset_id: Optional[str],
        created_at: str,
        updated_at: str,
    ) -> None:
        self.connection.execute(
            f"""
            INSERT INTO agent_material_texture_objects (
              {self._COLUMNS}
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
            """,
            (
                texture_asset_id,
                texture_role,
                display_name,
                mime_type,
                byte_size,
                sha256,
                object_path,
                width,
                height,
                source,
                license,
                license_ref,
                thumbnail_asset_id,
                created_at,
                updated_at,
            ),
        )

    def list(
        self,
        *,
        texture_role: Optional[str] = None,
        source: Optional[str] = None,
        query: Optional[str] = None,
        limit: int = 100,
    ) -> list[sqlite3.Row]:
        clauses = ["1 = 1"]
        params: list[object] = []
        if texture_role:
            clauses.append("texture_role = ?")
            params.append(texture_role)
        if source:
            clauses.append("source = ?")
            params.append(source)
        if query:
            clauses.append("(display_name LIKE ? OR texture_asset_id LIKE ?)")
            needle = f"%{query}%"
            params.extend([needle, needle])
        params.append(limit)
        return self.connection.execute(
            f"""
            SELECT {self._COLUMNS}
            FROM agent_material_texture_objects
            WHERE {' AND '.join(clauses)}
            ORDER BY updated_at DESC, texture_asset_id ASC
            LIMIT ?
            """,
            params,
        ).fetchall()
