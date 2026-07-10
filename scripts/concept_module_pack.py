#!/usr/bin/env python3
"""Validate and explicitly import a ForgeCAD Concept Module Pack."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import struct
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from pydantic import ValidationError


ROOT = Path(__file__).resolve().parents[1]
AGENT_ROOT = ROOT / "apps" / "agent"
if str(AGENT_ROOT) not in sys.path:
    sys.path.insert(0, str(AGENT_ROOT))

from forgecad_agent.domain.concepts.models import (  # noqa: E402
    ModuleAssetManifest,
    ModulePackEntry,
    ModulePackManifest,
)


REQUIRED_RELEASE_CATEGORIES = {
    "core_shell",
    "front_shell",
    "rear_shell",
    "grip_shell",
    "top_accessory",
    "side_accessory",
    "lower_structure",
    "storage_visual",
    "armor_panel",
}
MODULE_SEQUENCE = r"(?:0[1-9]|[1-9][0-9])"
MODULE_ID_PATTERN = re.compile(
    rf"^module_({'|'.join(sorted(REQUIRED_RELEASE_CATEGORIES))})_{MODULE_SEQUENCE}$"
)
ASSET_ID_PATTERN = re.compile(
    rf"^asset_({'|'.join(sorted(REQUIRED_RELEASE_CATEGORIES))})_{MODULE_SEQUENCE}$"
)
PACK_ID_PATTERN = re.compile(r"^pack_[a-z][a-z0-9_]*_v[0-9]+$")
CONNECTOR_ID_PATTERN = re.compile(r"^connector_[a-z][a-z0-9]*(?:_[a-z0-9]+)+$")
MATERIAL_SLOT_PATTERN = re.compile(
    r"^MAT_(?:primary|secondary|accent|emissive|transparent)(?:_[a-z0-9]+)*$"
)


class ModulePackValidationError(RuntimeError):
    """Raised after collecting every deterministic pack validation error."""

    def __init__(self, errors: list[str]) -> None:
        self.errors = errors
        super().__init__("\n".join(errors))


@dataclass(frozen=True)
class ValidatedModule:
    entry: ModulePackEntry
    manifest: ModuleAssetManifest
    glb_path: Path
    payload: bytes


@dataclass(frozen=True)
class ValidatedModulePack:
    root: Path
    manifest: ModulePackManifest
    modules: tuple[ValidatedModule, ...]
    warnings: tuple[str, ...]

    def report(self, *, imported: list[dict[str, Any]] | None = None) -> dict[str, Any]:
        return {
            "ok": True,
            "mode": "import" if imported is not None else "dry-run",
            "pack_id": self.manifest.pack_id,
            "version": self.manifest.version,
            "module_count": len(self.modules),
            "categories": sorted({module.manifest.category for module in self.modules}),
            "warnings": list(self.warnings),
            "imported_module_ids": (
                [item["manifest"]["module_id"] for item in imported]
                if imported is not None
                else []
            ),
        }


def validate_module_pack(pack_root: Path, *, release: bool = False) -> ValidatedModulePack:
    root = pack_root.expanduser().resolve()
    errors: list[str] = []
    warnings: list[str] = []
    pack_path = root / "pack.json"
    if not pack_path.is_file():
        raise ModulePackValidationError(["pack.json: file is required at the pack root"])

    try:
        raw_pack = json.loads(pack_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ModulePackValidationError([f"pack.json: cannot be read as UTF-8 JSON: {exc}"]) from exc
    try:
        pack = ModulePackManifest.model_validate(raw_pack)
    except ValidationError as exc:
        raise ModulePackValidationError([f"pack.json: {item}" for item in _pydantic_errors(exc)]) from exc

    _validate_pack_naming(pack, errors)

    _require_text_file(root, pack.license.license_path, "pack license", errors)
    modules: list[ValidatedModule] = []
    asset_ids: set[str] = set()
    connector_ids: set[str] = set()
    for entry in pack.modules:
        validated = _validate_entry(root, pack, entry, errors)
        if validated is None:
            continue
        manifest = validated.manifest
        if manifest.asset_id in asset_ids:
            errors.append(f"{entry.module_id}: duplicate asset_id across pack: {manifest.asset_id}")
        asset_ids.add(manifest.asset_id)
        for connector in manifest.connectors:
            if connector.connector_id in connector_ids:
                errors.append(
                    f"{entry.module_id}: duplicate connector_id across pack: {connector.connector_id}"
                )
            connector_ids.add(connector.connector_id)
        modules.append(validated)

    categories = {module.manifest.category for module in modules}
    missing_categories = sorted(REQUIRED_RELEASE_CATEGORIES - categories)
    if missing_categories:
        message = f"pack: categories not represented: {', '.join(missing_categories)}"
        if release:
            errors.append(message)
        else:
            warnings.append(message)
    if release and not 8 <= len(modules) <= 12:
        errors.append(f"pack: release pack must contain 8-12 modules, found {len(modules)}")

    if errors:
        raise ModulePackValidationError(errors)
    return ValidatedModulePack(root, pack, tuple(modules), tuple(warnings))


def import_module_pack(pack: ValidatedModulePack, api_base_url: str) -> list[dict[str, Any]]:
    """Import an already validated pack through the immutable Module registry API."""

    responses: list[dict[str, Any]] = []
    base_url = api_base_url.rstrip("/")
    for module in pack.modules:
        request_body = {
            "client_request_id": f"module-pack-{pack.manifest.pack_id}-{module.entry.module_id}",
            "manifest": module.manifest.model_dump(mode="json"),
            "logical_path": f"packs/{pack.manifest.pack_id}/{module.entry.glb_path}",
            "glb_data_base64": base64.b64encode(module.payload).decode("ascii"),
        }
        canonical = json.dumps(
            request_body,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        idempotency_key = f"module-pack-{hashlib.sha256(canonical).hexdigest()[:32]}"
        responses.append(
            _json_request(
                f"{base_url}/api/v1/module-assets",
                request_body,
                idempotency_key=idempotency_key,
            )
        )
    return responses


def _validate_entry(
    root: Path,
    pack: ModulePackManifest,
    entry: ModulePackEntry,
    errors: list[str],
) -> ValidatedModule | None:
    expected_root = f"modules/{entry.module_id}"
    expected_paths = {
        "manifest_path": f"{expected_root}/module.json",
        "glb_path": f"{expected_root}/model.glb",
        "thumbnail_path": f"{expected_root}/thumbnail.png",
    }
    for field_name, expected in expected_paths.items():
        actual = getattr(entry, field_name)
        if actual != expected:
            errors.append(f"{entry.module_id}: {field_name} must be {expected}, found {actual}")

    manifest_path = _safe_pack_file(root, entry.manifest_path, "module manifest", errors)
    glb_path = _safe_pack_file(root, entry.glb_path, "module GLB", errors)
    thumbnail_path = _safe_pack_file(root, entry.thumbnail_path, "module thumbnail", errors)
    _require_text_file(root, entry.license_path, f"{entry.module_id} license", errors)
    if manifest_path is None or glb_path is None or thumbnail_path is None:
        return None

    try:
        manifest = ModuleAssetManifest.model_validate_json(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, ValidationError) as exc:
        if isinstance(exc, ValidationError):
            errors.extend(
                f"{entry.manifest_path}: {item}" for item in _pydantic_errors(exc)
            )
        else:
            errors.append(f"{entry.manifest_path}: cannot read module manifest: {exc}")
        return None

    if manifest.module_id != entry.module_id:
        errors.append(
            f"{entry.module_id}: manifest module_id is {manifest.module_id}, expected {entry.module_id}"
        )
    if manifest.pack_id != pack.pack_id:
        errors.append(
            f"{entry.module_id}: manifest pack_id is {manifest.pack_id}, expected {pack.pack_id}"
        )
    if entry.lod != "LOD0":
        errors.append(f"{entry.module_id}: P0 registry imports only LOD0, found {entry.lod}")
    _validate_manifest_naming(manifest, errors)

    try:
        payload = glb_path.read_bytes()
    except OSError as exc:
        errors.append(f"{entry.glb_path}: cannot read GLB: {exc}")
        return None
    digest = hashlib.sha256(payload).hexdigest()
    if digest != manifest.sha256:
        errors.append(
            f"{entry.module_id}: GLB sha256 {digest} does not match manifest {manifest.sha256}"
        )
    try:
        gltf = _read_glb_json(payload)
        _validate_gltf(gltf, manifest, errors)
    except ValueError as exc:
        errors.append(f"{entry.module_id}: invalid GLB: {exc}")
    _validate_thumbnail(thumbnail_path, entry.module_id, errors)
    return ValidatedModule(entry, manifest, glb_path, payload)


def _read_glb_json(payload: bytes) -> dict[str, Any]:
    if len(payload) < 20:
        raise ValueError("file is too small")
    magic, version, declared_length = struct.unpack_from("<4sII", payload, 0)
    if magic != b"glTF" or version != 2 or declared_length != len(payload):
        raise ValueError("expected a complete glTF 2.0 binary envelope")
    offset = 12
    json_payload: bytes | None = None
    while offset + 8 <= len(payload):
        chunk_length, chunk_type = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk_end = offset + chunk_length
        if chunk_end > len(payload):
            raise ValueError("chunk extends beyond declared file length")
        if chunk_type == 0x4E4F534A and json_payload is None:
            json_payload = payload[offset:chunk_end]
        offset = chunk_end
    if offset != len(payload) or json_payload is None:
        raise ValueError("JSON chunk is missing or GLB chunks are malformed")
    try:
        value = json.loads(json_payload.rstrip(b" \x00").decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError(f"JSON chunk is invalid: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError("JSON chunk root must be an object")
    return value


def _validate_gltf(
    gltf: dict[str, Any],
    manifest: ModuleAssetManifest,
    errors: list[str],
) -> None:
    prefix = manifest.module_id
    if str(gltf.get("asset", {}).get("version")) != "2.0":
        errors.append(f"{prefix}: GLB asset.version must be 2.0")
    meshes = gltf.get("meshes") or []
    accessors = gltf.get("accessors") or []
    materials = gltf.get("materials") or []
    if not meshes:
        errors.append(f"{prefix}: GLB must contain at least one mesh")
        return

    mesh_name_pattern = re.compile(
        rf"^MESH_{re.escape(manifest.module_id)}_LOD0(?:_[0-9]{{2}})?$"
    )
    for mesh_index, mesh in enumerate(meshes):
        mesh_name = mesh.get("name")
        if not isinstance(mesh_name, str) or not mesh_name_pattern.fullmatch(mesh_name):
            errors.append(
                f"{prefix}: mesh {mesh_index} name must follow "
                f"MESH_{manifest.module_id}_LOD0[_NN]"
            )

    node_name_pattern = re.compile(
        rf"^GEO_{re.escape(manifest.module_id)}_LOD0(?:_[0-9]{{2}})?$"
    )
    for node_index, node in enumerate(gltf.get("nodes") or []):
        if "mesh" not in node:
            continue
        node_name = node.get("name")
        if not isinstance(node_name, str) or not node_name_pattern.fullmatch(node_name):
            errors.append(
                f"{prefix}: mesh node {node_index} name must follow "
                f"GEO_{manifest.module_id}_LOD0[_NN]"
            )

    triangle_count = 0
    used_material_names: set[str] = set()
    bounds_min = [float("inf")] * 3
    bounds_max = [float("-inf")] * 3
    for mesh_index, mesh in enumerate(meshes):
        for primitive_index, primitive in enumerate(mesh.get("primitives") or []):
            label = f"{prefix}: mesh {mesh_index} primitive {primitive_index}"
            if primitive.get("mode", 4) != 4:
                errors.append(f"{label} must use TRIANGLES mode")
                continue
            attributes = primitive.get("attributes") or {}
            if "POSITION" not in attributes:
                errors.append(f"{label} is missing POSITION")
                continue
            if "TEXCOORD_0" not in attributes:
                errors.append(f"{label} is missing TEXCOORD_0 UVs")
            position_accessor = _accessor(accessors, attributes["POSITION"], label, errors)
            if position_accessor is not None:
                minimum = position_accessor.get("min")
                maximum = position_accessor.get("max")
                if not _vec3(minimum) or not _vec3(maximum):
                    errors.append(f"{label} POSITION accessor must declare min/max bounds")
                else:
                    bounds_min = [min(bounds_min[i], float(minimum[i])) for i in range(3)]
                    bounds_max = [max(bounds_max[i], float(maximum[i])) for i in range(3)]
            count_accessor_index = primitive.get("indices", attributes["POSITION"])
            count_accessor = _accessor(accessors, count_accessor_index, label, errors)
            if count_accessor is not None:
                count = count_accessor.get("count")
                if not isinstance(count, int) or count <= 0 or count % 3 != 0:
                    errors.append(f"{label} vertex/index count must be a positive multiple of 3")
                else:
                    triangle_count += count // 3
            material_index = primitive.get("material")
            if not isinstance(material_index, int) or not 0 <= material_index < len(materials):
                errors.append(f"{label} must reference a named material")
            else:
                material_name = materials[material_index].get("name")
                if not isinstance(material_name, str) or not material_name:
                    errors.append(f"{label} material must have a non-empty name")
                else:
                    used_material_names.add(material_name)

    if triangle_count != manifest.triangle_count:
        errors.append(
            f"{prefix}: GLB triangle count {triangle_count} does not match manifest "
            f"{manifest.triangle_count}"
        )
    if used_material_names != set(manifest.material_slots):
        errors.append(
            f"{prefix}: used GLB materials {sorted(used_material_names)} do not match manifest "
            f"{sorted(manifest.material_slots)}"
        )
    if all(value != float("inf") for value in bounds_min):
        measured_mm = [(bounds_max[i] - bounds_min[i]) * 1000 for i in range(3)]
        for axis, (measured, declared) in enumerate(zip(measured_mm, manifest.bounds_mm)):
            tolerance = max(0.5, declared * 0.01)
            if abs(measured - declared) > tolerance:
                errors.append(
                    f"{prefix}: axis {axis} GLB bound {measured:.3f} mm does not match "
                    f"manifest {declared:.3f} mm"
                )

    identity_matrix = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    for node_index, node in enumerate(gltf.get("nodes") or []):
        if "matrix" in node and node["matrix"] != identity_matrix:
            errors.append(f"{prefix}: node {node_index} has an unapplied matrix")
        if node.get("translation", [0, 0, 0]) != [0, 0, 0]:
            errors.append(f"{prefix}: node {node_index} has unapplied translation")
        if node.get("rotation", [0, 0, 0, 1]) != [0, 0, 0, 1]:
            errors.append(f"{prefix}: node {node_index} has unapplied rotation")
        if node.get("scale", [1, 1, 1]) != [1, 1, 1]:
            errors.append(f"{prefix}: node {node_index} has unapplied scale")


def _validate_pack_naming(pack: ModulePackManifest, errors: list[str]) -> None:
    if not PACK_ID_PATTERN.fullmatch(pack.pack_id):
        errors.append(
            f"pack: pack_id must follow pack_<name>_v<N>, found {pack.pack_id}"
        )
    for entry in pack.modules:
        if not MODULE_ID_PATTERN.fullmatch(entry.module_id):
            errors.append(
                f"{entry.module_id}: module_id must follow "
                "module_<category>_<NN> with a registered P0 category"
            )


def _validate_manifest_naming(
    manifest: ModuleAssetManifest,
    errors: list[str],
) -> None:
    expected_module_id = f"module_{manifest.category}_"
    if not MODULE_ID_PATTERN.fullmatch(manifest.module_id) or not manifest.module_id.startswith(
        expected_module_id
    ):
        errors.append(
            f"{manifest.module_id}: module_id category must match {manifest.category}"
        )
    match = MODULE_ID_PATTERN.fullmatch(manifest.module_id)
    expected_asset_id = (
        f"asset_{manifest.category}_{manifest.module_id.rsplit('_', 1)[-1]}"
        if match
        else None
    )
    if not ASSET_ID_PATTERN.fullmatch(manifest.asset_id) or (
        expected_asset_id is not None and manifest.asset_id != expected_asset_id
    ):
        errors.append(
            f"{manifest.module_id}: asset_id must be {expected_asset_id or 'asset_<category>_<NN>'}, "
            f"found {manifest.asset_id}"
        )
    for connector in manifest.connectors:
        if not CONNECTOR_ID_PATTERN.fullmatch(connector.connector_id):
            errors.append(
                f"{manifest.module_id}: connector_id must follow connector_<owner>_<interface>, "
                f"found {connector.connector_id}"
            )
    for material_slot in manifest.material_slots:
        if not MATERIAL_SLOT_PATTERN.fullmatch(material_slot):
            errors.append(
                f"{manifest.module_id}: material slot must use a ForgeCAD semantic MAT_ name, "
                f"found {material_slot}"
            )


def _accessor(
    accessors: list[Any],
    index: Any,
    label: str,
    errors: list[str],
) -> dict[str, Any] | None:
    if not isinstance(index, int) or not 0 <= index < len(accessors):
        errors.append(f"{label} references an invalid accessor")
        return None
    accessor = accessors[index]
    if not isinstance(accessor, dict):
        errors.append(f"{label} accessor must be an object")
        return None
    return accessor


def _vec3(value: Any) -> bool:
    return isinstance(value, list) and len(value) == 3 and all(
        isinstance(item, (int, float)) for item in value
    )


def _safe_pack_file(
    root: Path,
    relative_path: str,
    label: str,
    errors: list[str],
) -> Path | None:
    if "\\" in relative_path:
        errors.append(f"{label}: path must use POSIX separators: {relative_path}")
        return None
    pure = PurePosixPath(relative_path)
    if pure.is_absolute() or ".." in pure.parts or ":" in relative_path or not pure.parts:
        errors.append(f"{label}: unsafe relative path: {relative_path}")
        return None
    path = root.joinpath(*pure.parts)
    resolved = path.resolve()
    if not resolved.is_relative_to(root):
        errors.append(f"{label}: path escapes pack root: {relative_path}")
        return None
    if not path.is_file():
        errors.append(f"{label}: file does not exist: {relative_path}")
        return None
    return path


def _require_text_file(
    root: Path,
    relative_path: str,
    label: str,
    errors: list[str],
) -> None:
    path = _safe_pack_file(root, relative_path, label, errors)
    if path is None:
        return
    try:
        if not path.read_text(encoding="utf-8").strip():
            errors.append(f"{label}: file must contain license text: {relative_path}")
    except (OSError, UnicodeDecodeError) as exc:
        errors.append(f"{label}: license must be readable UTF-8 text: {exc}")


def _validate_thumbnail(path: Path, module_id: str, errors: list[str]) -> None:
    try:
        header = path.read_bytes()[:24]
    except OSError as exc:
        errors.append(f"{module_id}: cannot read thumbnail: {exc}")
        return
    if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        errors.append(f"{module_id}: thumbnail must be a PNG file")
        return
    width, height = struct.unpack(">II", header[16:24])
    if (width, height) != (512, 512):
        errors.append(
            f"{module_id}: thumbnail must be 512x512 pixels, found {width}x{height}"
        )


def _pydantic_errors(exc: ValidationError) -> list[str]:
    return [
        f"{'.'.join(str(part) for part in error['loc'])}: {error['msg']}"
        for error in exc.errors()
    ]


def _json_request(
    url: str,
    body: dict[str, Any],
    *,
    idempotency_key: str,
) -> dict[str, Any]:
    request = urllib.request.Request(
        url,
        data=json.dumps(body, ensure_ascii=False).encode("utf-8"),
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Idempotency-Key": idempotency_key,
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        payload = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"module import failed ({exc.code}): {payload}") from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"module import failed: {exc.reason}") from exc


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate a ModulePackManifest@1 and optionally import every LOD0 GLB."
    )
    parser.add_argument("pack_root", type=Path, help="directory containing pack.json")
    parser.add_argument(
        "--release",
        action="store_true",
        help="require 8-12 modules and all nine P0 categories",
    )
    parser.add_argument("--api-base-url", help="running local Agent, for example http://127.0.0.1:8000")
    parser.add_argument(
        "--import",
        dest="do_import",
        action="store_true",
        help="explicitly write validated modules through POST /api/v1/module-assets",
    )
    args = parser.parse_args()
    if args.do_import and not args.api_base_url:
        parser.error("--import requires --api-base-url")

    try:
        validated = validate_module_pack(args.pack_root, release=args.release)
        imported = (
            import_module_pack(validated, args.api_base_url)
            if args.do_import
            else None
        )
    except (ModulePackValidationError, RuntimeError) as exc:
        print(json.dumps({"ok": False, "errors": str(exc).splitlines()}, ensure_ascii=False, indent=2))
        return 1
    print(json.dumps(validated.report(imported=imported), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
