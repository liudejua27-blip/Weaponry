from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from forgecad_agent.infrastructure.db import AssetRepository, SQLiteConnectionFactory
from forgecad_agent.infrastructure.storage import ContentAddressedStore, ObjectStoreError

from ..models import (
    AssetFileResponse,
    AssetFileSummary,
    WeaponDetail,
    WeaponSummary,
    WeaponVersionSummary,
    utc_now,
)


class LibraryError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyLibraryService:
    """Frozen weapon-library/version use cases behind the AssetStore facade."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: ContentAddressedStore,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def activate_version(self, weapon_id: str, version_id: str) -> WeaponDetail:
        with self.connection_factory.connect() as connection:
            if not AssetRepository(connection).committed_version_exists(
                weapon_id,
                version_id,
            ):
                raise LibraryError(
                    "VERSION_NOT_FOUND",
                    "Version was not found for this weapon.",
                )
            now = utc_now()
            before_changes = connection.total_changes
            connection.execute(
                """
                UPDATE weapons
                SET current_version_id = ?, updated_at = ?
                WHERE weapon_id = ? AND status = 'active'
                """,
                (version_id, now, weapon_id),
            )
            if connection.total_changes == before_changes:
                raise LibraryError("WEAPON_NOT_FOUND", "Weapon was not found.")
        return self.get_weapon_detail(weapon_id)

    def list_weapons(self) -> list[WeaponSummary]:
        with self.connection_factory.connect() as connection:
            rows = connection.execute(
                """
                WITH latest_models AS (
                  SELECT *
                  FROM models_3d
                  WHERE model_id IN (
                    SELECT model_id
                    FROM models_3d lm
                    WHERE lm.weapon_id = models_3d.weapon_id
                    ORDER BY datetime(created_at) DESC, model_id DESC
                    LIMIT 1
                  )
                )
                SELECT
                  w.weapon_id,
                  w.name AS display_name,
                  w.weapon_family,
                  CASE
                    WHEN m.model_id IS NOT NULL THEN 'rough_3d'
                    WHEN w.current_version_id IS NOT NULL THEN 'concept'
                    ELSE 'draft'
                  END AS stage,
                  w.current_version_id,
                  m.model_id AS current_model_id,
                  (
                    SELECT file_id
                    FROM asset_files af
                    WHERE af.weapon_id = w.weapon_id
                      AND af.role IN ('rough_preview_png', 'concept_image')
                      AND af.soft_deleted_at IS NULL
                    ORDER BY af.created_at DESC
                    LIMIT 1
                  ) AS thumbnail_asset_id,
                  w.updated_at
                FROM weapons w
                LEFT JOIN latest_models m ON m.weapon_id = w.weapon_id
                WHERE w.status = 'active'
                ORDER BY w.updated_at DESC
                LIMIT 100
                """
            ).fetchall()
        return [
            WeaponSummary(
                weapon_id=row["weapon_id"],
                display_name=row["display_name"],
                weapon_family=row["weapon_family"],
                stage=row["stage"],
                current_version_id=row["current_version_id"],
                current_model_id=row["current_model_id"],
                thumbnail_asset_id=row["thumbnail_asset_id"],
                updated_at=row["updated_at"],
            )
            for row in rows
        ]

    def get_weapon_detail(self, weapon_id: str) -> WeaponDetail:
        with self.connection_factory.connect() as connection:
            weapon = connection.execute(
                """
                SELECT
                  w.weapon_id,
                  w.name AS display_name,
                  w.weapon_family,
                  w.fantasy_category,
                  w.style,
                  CASE
                    WHEN m.model_id IS NOT NULL THEN 'rough_3d'
                    WHEN w.current_version_id IS NOT NULL THEN 'concept'
                    ELSE 'draft'
                  END AS stage,
                  w.current_version_id,
                  m.model_id AS current_model_id,
                  (
                    SELECT file_id
                    FROM asset_files af
                    WHERE af.weapon_id = w.weapon_id
                      AND af.role IN (
                        'rough_preview_png',
                        'concept_patch',
                        'concept_image'
                      )
                      AND af.soft_deleted_at IS NULL
                    ORDER BY af.created_at DESC
                    LIMIT 1
                  ) AS thumbnail_asset_id,
                  w.updated_at
                FROM weapons w
                LEFT JOIN models_3d m ON m.weapon_id = w.weapon_id
                WHERE w.weapon_id = ? AND w.status = 'active'
                ORDER BY m.created_at DESC
                LIMIT 1
                """,
                (weapon_id,),
            ).fetchone()
            if weapon is None:
                raise KeyError(weapon_id)

            asset_rows = connection.execute(
                """
                SELECT file_id, role, version_id, job_id, logical_path, sha256,
                       byte_size, mime_type, width, height, created_at
                FROM asset_files
                WHERE weapon_id = ? AND soft_deleted_at IS NULL
                ORDER BY created_at ASC
                """,
                (weapon_id,),
            ).fetchall()
            assets_by_version: dict[str, list[AssetFileSummary]] = {}
            for asset in asset_rows:
                summary = AssetFileSummary(
                    asset_id=asset["file_id"],
                    role=asset["role"],
                    version_id=asset["version_id"],
                    job_id=asset["job_id"],
                    logical_path=asset["logical_path"],
                    sha256=asset["sha256"],
                    byte_size=asset["byte_size"],
                    mime_type=asset["mime_type"],
                    width=asset["width"],
                    height=asset["height"],
                    created_at=asset["created_at"],
                )
                if asset["version_id"]:
                    assets_by_version.setdefault(asset["version_id"], []).append(
                        summary
                    )

            version_rows = connection.execute(
                """
                SELECT version_id, parent_version_id, job_id, version_no,
                       version_type, status, summary, created_at
                FROM weapon_versions
                WHERE weapon_id = ?
                ORDER BY version_no ASC
                """,
                (weapon_id,),
            ).fetchall()
            versions = [
                WeaponVersionSummary(
                    version_id=row["version_id"],
                    parent_version_id=row["parent_version_id"],
                    job_id=row["job_id"],
                    version_no=row["version_no"],
                    version_type=row["version_type"],
                    status=row["status"],
                    summary=row["summary"],
                    created_at=row["created_at"],
                    assets=assets_by_version.get(row["version_id"], []),
                )
                for row in version_rows
            ]

            current_spec: dict[str, Any] = {}
            if weapon["current_version_id"]:
                spec = connection.execute(
                    "SELECT spec_json FROM weapon_specs WHERE version_id = ?",
                    (weapon["current_version_id"],),
                ).fetchone()
                if spec is not None:
                    current_spec = json.loads(spec["spec_json"])

            current_model: dict[str, Any] = {}
            if weapon["current_model_id"]:
                model = connection.execute(
                    """
                    SELECT model_id, provider, status, orientation_policy_json,
                           quality_report_json
                    FROM models_3d
                    WHERE model_id = ?
                    """,
                    (weapon["current_model_id"],),
                ).fetchone()
                if model is not None:
                    current_model = {
                        "model_id": model["model_id"],
                        "provider": model["provider"],
                        "status": model["status"],
                        "orientation_policy": json.loads(
                            model["orientation_policy_json"] or "{}"
                        ),
                        "quality_report": json.loads(
                            model["quality_report_json"] or "{}"
                        ),
                    }

            jobs = connection.execute(
                """
                SELECT job_id, job_type, status, current_step, created_at,
                       updated_at, finished_at
                FROM generation_jobs
                WHERE weapon_id = ?
                ORDER BY created_at DESC
                LIMIT 10
                """,
                (weapon_id,),
            ).fetchall()

        return WeaponDetail(
            weapon_id=weapon["weapon_id"],
            display_name=weapon["display_name"],
            weapon_family=weapon["weapon_family"],
            fantasy_category=weapon["fantasy_category"],
            style=weapon["style"],
            stage=weapon["stage"],
            current_version_id=weapon["current_version_id"],
            current_model_id=weapon["current_model_id"],
            thumbnail_asset_id=weapon["thumbnail_asset_id"],
            updated_at=weapon["updated_at"],
            versions=versions,
            current_spec=current_spec,
            current_model=current_model,
            latest_jobs=[dict(row) for row in jobs],
        )

    def get_asset_metadata(self, asset_id: str) -> AssetFileResponse:
        with self.connection_factory.connect() as connection:
            row = AssetRepository(connection).get_active(asset_id)
            if row is None:
                raise LibraryError(
                    "ASSET_FILE_MISSING",
                    f"Asset file was not found: {asset_id}",
                )
        return AssetFileResponse(
            asset_id=row["file_id"],
            weapon_id=row["weapon_id"],
            version_id=row["version_id"],
            job_id=row["job_id"],
            role=row["role"],
            logical_path=row["logical_path"],
            sha256=row["sha256"],
            byte_size=row["byte_size"],
            mime_type=row["mime_type"],
            width=row["width"],
            height=row["height"],
            created_at=row["created_at"],
        )

    def resolve_asset_file(self, asset_id: str) -> dict[str, Any]:
        with self.connection_factory.connect() as connection:
            row = AssetRepository(connection).get_active(asset_id)
            if row is None:
                raise LibraryError(
                    "ASSET_FILE_MISSING",
                    f"Asset file was not found: {asset_id}",
                )
        try:
            full_path = self.object_store.resolve(row["object_path"])
            self.object_store.read(
                row["object_path"],
                expected_sha256=row["sha256"],
            )
        except ObjectStoreError as exc:
            if exc.code == "OBJECT_PATH_DENIED":
                raise LibraryError("ASSET_PERMISSION_DENIED", str(exc)) from exc
            if exc.code == "OBJECT_MISSING":
                raise LibraryError(
                    "ASSET_FILE_MISSING",
                    f"Asset file is missing: {asset_id}",
                ) from exc
            raise LibraryError(
                "LOCAL_IO_ERROR",
                f"Asset sha256 mismatch: {asset_id}",
            ) from exc
        return {
            "path": full_path,
            "mime_type": row["mime_type"],
            "filename": Path(row["logical_path"]).name
            or f"{asset_id}.{row['ext']}",
            "sha256": row["sha256"],
            "role": row["role"],
            "logical_path": row["logical_path"],
        }

    def has_weapon(self, weapon_id: str) -> bool:
        with self.connection_factory.connect() as connection:
            return connection.execute(
                "SELECT 1 FROM weapons WHERE weapon_id = ? AND status = 'active'",
                (weapon_id,),
            ).fetchone() is not None
