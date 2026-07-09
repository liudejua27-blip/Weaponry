#!/usr/bin/env python3
"""M4 smoke checks for mock patch versioning and mask validation."""

from __future__ import annotations

import json
import sqlite3
import struct
import sys
import tempfile
import zlib
from pathlib import Path
from typing import Any

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import AssetStoreError, SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, PatchWeaponRequest  # noqa: E402
from wushen_agent.models import utc_now  # noqa: E402


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m4_patch_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        store = SQLiteAssetStore(library_root=library_root, migrations_dir=ROOT / "migrations")
        create_request = CreateWeaponRequest(
            client_request_id="m4-source",
            text="青玉雷纹长枪，3渲2国风神兵，逼真外观，仅作为虚构 Unity 游戏资产",
        )
        source_job = store.create_weapon(create_request, idempotency_key="m4-source-key")
        source_version_id = source_job.outputs["current_version_id"]
        source_image_id = _asset_id_by_role(source_job, "concept_image")
        source_image = _asset_row(store.db_path, source_image_id)

        mask_id = _write_patch_asset(
            store,
            weapon_id=source_job.weapon_id,
            version_id=source_version_id,
            role="patch_mask",
            logical_name="patch_mask.png",
            payload=png_mask(int(source_image["width"]), int(source_image["height"]), ink=True),
            ext=".png",
            mime_type="image/png",
            metadata={"purpose": "m4_smoke_mask"},
            width=int(source_image["width"]),
            height=int(source_image["height"]),
        )
        manifest_id = _write_manifest(store, source_job.weapon_id, source_image, mask_id)
        request = PatchWeaponRequest(
            client_request_id="m4-patch",
            source_version_id=source_version_id,
            source_image_asset_id=source_image_id,
            mask_asset_id=mask_id,
            patch_manifest_asset_id=manifest_id,
            target_area="core",
            instruction="把枪身核心改成青蓝雷纹能量核，保持国风神兵外观",
            preserve=["overall_silhouette", "chinese_motifs", "toon_outline"],
            strength="medium",
        )
        patch_job = store.patch_weapon(source_job.weapon_id, request, idempotency_key="m4-patch-key")
        replay = store.patch_weapon(source_job.weapon_id, request, idempotency_key="m4-patch-key")
        assert patch_job.job_id == replay.job_id
        _assert_patch_success(store.db_path, source_job.job_id, patch_job.job_id, source_version_id)
        patch_version_id = patch_job.outputs["current_version_id"]
        parent_detail = store.activate_version(source_job.weapon_id, source_version_id)
        assert parent_detail.current_version_id == source_version_id
        patch_detail = store.activate_version(source_job.weapon_id, patch_version_id)
        assert patch_detail.current_version_id == patch_version_id

        conflict_request = request.model_copy(update={"instruction": "同 key 不同内容"})
        try:
            store.patch_weapon(source_job.weapon_id, conflict_request, idempotency_key="m4-patch-key")
        except Exception as exc:  # noqa: BLE001 - smoke checks idempotency mapping.
            assert "Idempotency-Key" in str(exc)
        else:
            raise AssertionError("patch idempotency conflict did not fail")

        empty_mask_id = _write_patch_asset(
            store,
            weapon_id=source_job.weapon_id,
            version_id=source_version_id,
            role="patch_mask",
            logical_name="empty_mask.png",
            payload=png_mask(int(source_image["width"]), int(source_image["height"]), ink=False),
            ext=".png",
            mime_type="image/png",
            metadata={"purpose": "m4_empty_mask"},
            width=int(source_image["width"]),
            height=int(source_image["height"]),
        )
        empty_manifest_id = _write_manifest(store, source_job.weapon_id, source_image, empty_mask_id)
        empty_request = request.model_copy(update={"client_request_id": "m4-empty", "mask_asset_id": empty_mask_id, "patch_manifest_asset_id": empty_manifest_id})
        _assert_patch_error(store, source_job.weapon_id, empty_request, "m4-empty-key", "MASK_EMPTY")

        mismatch_mask_id = _write_patch_asset(
            store,
            weapon_id=source_job.weapon_id,
            version_id=source_version_id,
            role="patch_mask",
            logical_name="mismatch_mask.png",
            payload=png_mask(16, 16, ink=True),
            ext=".png",
            mime_type="image/png",
            metadata={"purpose": "m4_mismatch_mask"},
            width=16,
            height=16,
        )
        mismatch_manifest_id = _write_manifest(store, source_job.weapon_id, source_image, mismatch_mask_id)
        mismatch_request = request.model_copy(
            update={"client_request_id": "m4-mismatch", "mask_asset_id": mismatch_mask_id, "patch_manifest_asset_id": mismatch_manifest_id}
        )
        _assert_patch_error(store, source_job.weapon_id, mismatch_request, "m4-mismatch-key", "MASK_SIZE_MISMATCH")

        findings, stats = validate(library_root, store.db_path)
        assert stats["blockers"] == 0, findings
        print(
            json.dumps(
                {
                    "ok": True,
                    "source_job_id": source_job.job_id,
                    "patch_job_id": patch_job.job_id,
                    "source_version_id": source_version_id,
                    "patch_version_id": patch_version_id,
                    "asset_count": stats["asset_files"],
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _asset_id_by_role(job: Any, role: str) -> str:
    for file_id, item_role in job.outputs["asset_roles"].items():
        if item_role == role:
            return file_id
    raise AssertionError(f"asset role not found: {role}")


def _asset_row(db_path: Path, file_id: str) -> sqlite3.Row:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    row = conn.execute("SELECT * FROM asset_files WHERE file_id = ?", (file_id,)).fetchone()
    if row is None:
        raise AssertionError(f"asset not found: {file_id}")
    return row


def _write_patch_asset(
    store: SQLiteAssetStore,
    *,
    weapon_id: str,
    version_id: str,
    role: str,
    logical_name: str,
    payload: bytes,
    ext: str,
    mime_type: str,
    metadata: dict[str, Any],
    width: int | None = None,
    height: int | None = None,
) -> str:
    with store._connect() as conn:  # noqa: SLF001 - smoke seeds pre-existing uploaded assets.
        file_id = store._write_asset(  # noqa: SLF001
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=None,
            role=role,
            logical_path=f"weapons/{weapon_id}/versions/{version_id}/{logical_name}",
            payload=payload,
            ext=ext,
            mime_type=mime_type,
            metadata=metadata,
            width=width,
            height=height,
        )
        conn.commit()
        return file_id


def _write_manifest(store: SQLiteAssetStore, weapon_id: str, source_image: sqlite3.Row, mask_id: str) -> str:
    mask = _asset_row(store.db_path, mask_id)
    manifest = {
        "schema_version": "PatchManifest@1",
        "weapon_id": weapon_id,
        "source_asset_id": source_image["file_id"],
        "source_image": source_image["logical_path"],
        "mask_asset_id": mask_id,
        "mask_image": mask["logical_path"],
        "selection": {
            "tool": "rectangle",
            "polygon": [{"x": 120, "y": 120}, {"x": 360, "y": 120}, {"x": 360, "y": 320}, {"x": 120, "y": 320}],
        },
        "instruction": {"target": "core", "text": "把核心区域改成青蓝雷纹能量核"},
        "preserve": ["overall_silhouette", "chinese_motifs", "toon_outline"],
        "strength": "medium",
        "regenerate_3d": False,
        "created_at": utc_now(),
    }
    return _write_patch_asset(
        store,
        weapon_id=weapon_id,
        version_id=source_image["version_id"],
        role="patch_manifest",
        logical_name=f"patch_manifest_{mask_id}.json",
        payload=json.dumps(manifest, ensure_ascii=False).encode("utf-8"),
        ext=".json",
        mime_type="application/json",
        metadata={"schema_version": "PatchManifest@1"},
    )


def _assert_patch_success(db_path: Path, source_job_id: str, patch_job_id: str, source_version_id: str) -> None:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    versions = conn.execute("SELECT version_id, parent_version_id, version_no, version_type FROM weapon_versions ORDER BY version_no").fetchall()
    assert len(versions) == 2
    assert versions[0]["version_id"] == source_version_id
    assert versions[1]["parent_version_id"] == source_version_id
    assert versions[1]["version_type"] == "patch"
    source_assets = conn.execute("SELECT COUNT(*) AS count FROM asset_files WHERE job_id = ?", (source_job_id,)).fetchone()["count"]
    patch_roles = {
        row["role"]
        for row in conn.execute("SELECT role FROM asset_files WHERE job_id = ?", (patch_job_id,)).fetchall()
    }
    assert source_assets >= 8
    assert {"patch_prompt", "concept_patch", "quality_report"} <= patch_roles
    steps = [
        row["step"]
        for row in conn.execute("SELECT step FROM agent_events WHERE job_id = ? ORDER BY seq", (patch_job_id,)).fetchall()
    ]
    assert steps == ["patch_interpreter", "image_inpaint", "image_quality_check", "finalize_job"]


def _assert_patch_error(store: SQLiteAssetStore, weapon_id: str, request: PatchWeaponRequest, key: str, expected_code: str) -> None:
    try:
        store.patch_weapon(weapon_id, request, idempotency_key=key)
    except AssetStoreError as exc:
        assert exc.code == expected_code, exc.code
        return
    raise AssertionError(f"patch did not fail with {expected_code}")


def png_mask(width: int, height: int, *, ink: bool) -> bytes:
    rows = []
    for y in range(height):
        row = bytearray([0])
        for x in range(width):
            value = 255 if ink and width // 4 <= x < width // 2 and height // 4 <= y < height // 2 else 0
            row.extend([value, value, value, value])
        rows.append(bytes(row))
    raw = b"".join(rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + png_chunk(b"IDAT", zlib.compress(raw))
        + png_chunk(b"IEND", b"")
    )


def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


if __name__ == "__main__":
    sys.exit(main())
