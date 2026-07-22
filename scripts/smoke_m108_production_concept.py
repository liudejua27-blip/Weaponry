#!/usr/bin/env python3
"""FGC-M108 production-concept profile, identity and CAS regression gate."""

from __future__ import annotations

import copy
import base64
import asyncio
import hashlib
import json
import math
import tempfile
from pathlib import Path

from forgecad_agent.api import (
    LocalApiSettings,
    build_agent_asset_router,
    create_local_api,
)
from forgecad_agent.application.agent_asset_editing import (
    AgentAssetEditingService,
    AgentAssetError,
    PRODUCTION_CONCEPT_COMPILER_CONTRACT,
)
from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import (
    GeometryCompileReadbackError,
    build_blockout,
    compile_preview_shape_program,
    compile_production_concept_shape_program,
    list_blockout_variants,
    read_shape_program_glb_facts,
)
from forgecad_agent.application.mechanical_planner import (
    DeterministicMechanicalPlanner,
)
from forgecad_agent.application.shape_program_runtime import (
    MANIFEST_SCHEMA_VERSION,
)
from forgecad_agent.application.visual_texture_sets import (
    builtin_visual_material_count,
    builtin_visual_texture_cache_facts,
    geometry_artifact_profile_manifest,
)
from forgecad_agent.infrastructure.storage.content_addressed_store import (
    ContentAddressedStore,
)
from forgecad_agent.infrastructure.db import (
    SQLiteConnectionFactory,
    SQLiteMigrationRunner,
)
from smoke_m108_visual_pbr import (
    BRIEFS,
    _embedded_png,
    _glb_parts,
    _glb_payload,
    _png_rgb_pixels,
)
from smoke_q003_compile_readback_quality import (
    _make_asset,
    _revision,
    _seed,
)


ROOT = Path(__file__).resolve().parents[1]


def _canonical_json(value: object) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )


def _surface_identity(readback: object) -> list[tuple[object, ...]]:
    return [
        (
            item.primitive_id,
            item.part_instance_id,
            item.part_role,
            item.profile_input_id,
            item.material_zone_id,
            tuple(item.source_operation_ids),
        )
        for item in readback.surface_provenance
    ]


def _zone_identity(readback: object) -> list[tuple[object, ...]]:
    return [
        (
            item.primitive_id,
            item.part_instance_id,
            item.material_zone_id,
            item.material_id,
            tuple(item.source_operation_ids),
        )
        for item in readback.material_zone_faces
    ]


def _feature_identity(readback: object) -> list[tuple[object, ...]]:
    return [
        (
            item.node_id,
            item.operation,
            tuple(item.input_node_ids),
            tuple(item.material_ids),
            tuple(item.material_zone_ids),
            tuple(item.surface_roles),
        )
        for item in readback.feature_history
    ]


def _percentile(values: list[float], quantile: float) -> float:
    assert values
    return sorted(values)[round((len(values) - 1) * quantile)]


def _box_downsample(
    width: int,
    height: int,
    pixels: tuple[tuple[int, int, int], ...],
    factor: int,
) -> tuple[int, int, tuple[tuple[int, int, int], ...]]:
    assert width % factor == 0 and height % factor == 0
    sampled = []
    for y in range(0, height, factor):
        for x in range(0, width, factor):
            block = [
                pixels[(y + row) * width + x + column]
                for row in range(factor)
                for column in range(factor)
            ]
            sampled.append(
                tuple(
                    round(
                        sum(pixel[channel] for pixel in block)
                        / len(block)
                    )
                    for channel in range(3)
                )
            )
    return width // factor, height // factor, tuple(sampled)


def _normal_p95_degrees(
    pixels: tuple[tuple[int, int, int], ...],
) -> float:
    angles = []
    for red, green, blue in pixels:
        x = red / 255 * 2 - 1
        y = green / 255 * 2 - 1
        z = blue / 255 * 2 - 1
        length = max(math.sqrt(x * x + y * y + z * z), 1e-9)
        angles.append(
            math.degrees(
                math.acos(max(-1.0, min(1.0, z / length)))
            )
        )
    return _percentile(angles, 0.95)


def _channel_span(
    pixels: tuple[tuple[int, int, int], ...],
    channel: int,
) -> float:
    values = [float(pixel[channel]) for pixel in pixels]
    return _percentile(values, 0.95) - _percentile(values, 0.05)


def _assert_mip_aware_texture_visibility(
    glb: bytes,
    checked_material_ids: set[str],
) -> None:
    """Reject production maps whose detail disappears at 128/64 mip scales."""

    document, binary = _glb_parts(glb)
    used_material_indices = sorted(
        {
            int(primitive["material"])
            for mesh in document["meshes"]
            for primitive in mesh["primitives"]
        }
    )
    detailed_materials = {
        "mat_primary",
        "mat_aluminum",
        "mat_signal_red",
        "mat_composite",
        "mat_rubber",
        "mat_automotive_paint",
    }
    for material_index in used_material_indices:
        material = document["materials"][material_index]
        material_id = material["extras"]["forgecad_texture_material_id"]
        if material_id in checked_material_ids:
            continue
        base_width, base_height, base_pixels = _png_rgb_pixels(
            _embedded_png(
                document,
                binary,
                material_index,
                "base_color",
            )
        )
        normal_width, normal_height, normal_pixels = _png_rgb_pixels(
            _embedded_png(
                document,
                binary,
                material_index,
                "normal",
            )
        )
        rough_width, rough_height, rough_pixels = _png_rgb_pixels(
            _embedded_png(
                document,
                binary,
                material_index,
                "metallic_roughness",
            )
        )
        assert {
            (base_width, base_height),
            (normal_width, normal_height),
            (rough_width, rough_height),
        } == {(1024, 1024)}
        for factor, normal_range, roughness_range in (
            (4, (3.5, 8.0), (6.0, 18.0)),
            (8, (2.0, 6.0), (5.0, 16.0)),
        ):
            _, _, mip_base = _box_downsample(
                base_width,
                base_height,
                base_pixels,
                factor,
            )
            assert max(
                _channel_span(mip_base, channel)
                for channel in range(3)
            ) <= 8.0, (material_id, factor, "base_color")
            if material_id not in detailed_materials:
                continue
            _, _, mip_normal = _box_downsample(
                normal_width,
                normal_height,
                normal_pixels,
                factor,
            )
            _, _, mip_roughness = _box_downsample(
                rough_width,
                rough_height,
                rough_pixels,
                factor,
            )
            normal_p95 = _normal_p95_degrees(mip_normal)
            roughness_span = _channel_span(mip_roughness, 1)
            assert normal_range[0] <= normal_p95 <= normal_range[1], (
                material_id,
                factor,
                "normal",
                normal_p95,
            )
            assert roughness_range[0] <= roughness_span <= roughness_range[1], (
                material_id,
                factor,
                "roughness",
                roughness_span,
            )
        checked_material_ids.add(material_id)


def _assert_profile_tamper_rejected(glb: bytes) -> None:
    document, binary = _glb_parts(glb)
    tampered = copy.deepcopy(document)
    tampered["extras"]["forgecad_geometry_artifact_profile"][
        "texture_width"
    ] = 128
    try:
        read_shape_program_glb_facts(_glb_payload(tampered, binary))
    except ValueError as exc:
        assert "artifact profile" in str(exc)
    else:
        raise AssertionError("tampered production profile must be rejected")


def _assert_cache_roundtrip(
    *,
    asset_version_id: str,
    compiled: object,
    shape_program: dict[str, object],
) -> None:
    profile = geometry_artifact_profile_manifest("production_concept")
    shape_program_sha256 = hashlib.sha256(
        _canonical_json(shape_program).encode("utf-8")
    ).hexdigest()
    cache_identity = {
        "schema_version": "ProductionConceptArtifactCacheKey@1",
        "asset_version_id": asset_version_id,
        "shape_program_sha256": shape_program_sha256,
        "artifact_profile_sha256": profile["profile_sha256"],
        "runtime_manifest_version": MANIFEST_SCHEMA_VERSION,
        "compiler_contract": PRODUCTION_CONCEPT_COMPILER_CONTRACT,
    }
    cache_key = hashlib.sha256(
        _canonical_json(cache_identity).encode("utf-8")
    ).hexdigest()
    with tempfile.TemporaryDirectory(
        prefix="forgecad_m108_production_cache_"
    ) as directory:
        store = ContentAddressedStore(Path(directory))
        service = AgentAssetEditingService(None, store)  # type: ignore[arg-type]
        service._write_production_artifact_cache(  # pylint: disable=protected-access
            cache_key=cache_key,
            cache_identity=cache_identity,
            compiled=compiled,
        )
        # A fresh service instance models an Agent process restart.  The
        # production artifact must be read from the same CAS/index without an
        # in-memory cache or compiler result surviving from the writer.
        restarted = AgentAssetEditingService(None, store)  # type: ignore[arg-type]
        cached = restarted._read_production_artifact_cache(  # pylint: disable=protected-access
            cache_key=cache_key,
            cache_identity=cache_identity,
        )
        assert cached is not None
        assert cached.glb_bytes == compiled.glb_bytes
        assert cached.readback == compiled.readback

        index_path = store.resolve(
            f"derived/production-concept/{cache_key}.json"
        )
        index_entry = json.loads(index_path.read_text(encoding="utf-8"))
        assert "glb_base64" not in _canonical_json(index_entry)
        object_path = store.resolve(index_entry["object_path"])
        object_path.write_bytes(object_path.read_bytes() + b"\x00")
        try:
            restarted._read_production_artifact_cache(  # pylint: disable=protected-access
                cache_key=cache_key,
                cache_identity=cache_identity,
            )
        except AgentAssetError as exc:
            assert exc.code == "PRODUCTION_ARTIFACT_CACHE_INVALID"
        else:
            raise AssertionError("corrupted production CAS object must be rejected")


def _asset_api_get(
    service: AgentAssetEditingService,
    path: str,
) -> tuple[int, dict[str, str], bytes]:
    app = create_local_api(LocalApiSettings(title="M108A API Gate"))
    app.include_router(build_agent_asset_router(service))

    async def request() -> tuple[int, dict[str, str], bytes]:
        messages: list[dict[str, object]] = []
        request_delivered = False

        async def receive() -> dict[str, object]:
            nonlocal request_delivered
            if not request_delivered:
                request_delivered = True
                return {
                    "type": "http.request",
                    "body": b"",
                    "more_body": False,
                }
            return {"type": "http.disconnect"}

        async def send(message: dict[str, object]) -> None:
            messages.append(message)

        await app(
            {
                "type": "http",
                "asgi": {"version": "3.0", "spec_version": "2.3"},
                "http_version": "1.1",
                "method": "GET",
                "scheme": "http",
                "path": path,
                "raw_path": path.encode("ascii"),
                "query_string": b"",
                "root_path": "",
                "headers": [],
                "client": ("127.0.0.1", 41000),
                "server": ("127.0.0.1", 8000),
            },
            receive,
            send,
        )
        start = next(
            message
            for message in messages
            if message.get("type") == "http.response.start"
        )
        body = b"".join(
            bytes(message.get("body", b""))
            for message in messages
            if message.get("type") == "http.response.body"
        )
        headers = {
            bytes(name).decode("latin-1").lower(): bytes(value).decode(
                "latin-1"
            )
            for name, value in start["headers"]  # type: ignore[index]
        }
        return int(start["status"]), headers, body  # type: ignore[arg-type]

    return asyncio.run(request())


def _assert_binary_api_and_restart_cache() -> None:
    """Bind HTTP bytes, headers, quality/export and restart CAS to one truth."""

    with tempfile.TemporaryDirectory(
        prefix="forgecad_m108_production_api_"
    ) as directory:
        root = Path(directory)
        database_path = root / "library.db"
        factory = SQLiteConnectionFactory(database_path)
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed(factory)
        _, version = _make_asset(
            factory,
            "future",
            "设计一个虚构非功能的未来概念道具",
            "pack_future_weapon_prop",
        )
        object_store = ContentAddressedStore(root / "object-store")
        service = AgentAssetEditingService(factory, object_store)
        preview_status, preview_headers, preview_body = _asset_api_get(
            service,
            f"/api/v1/agent/asset-versions/{version.asset_version_id}:preview.glb"
        )
        assert preview_status == 200
        assert preview_headers["content-type"].startswith(
            "model/gltf-binary"
        )
        preview_sha256 = hashlib.sha256(preview_body).hexdigest()
        assert preview_headers["etag"] == f'"{preview_sha256}"'
        assert preview_headers["x-forgecad-glb-sha256"] == preview_sha256
        assert preview_headers["x-forgecad-artifact-profile"] == (
            "interactive_preview"
        )
        assert preview_headers[
            "x-forgecad-artifact-profile-sha256"
        ] == geometry_artifact_profile_manifest("interactive_preview")[
            "profile_sha256"
        ]
        assert int(preview_headers["x-forgecad-glb-byte-size"]) == len(
            preview_body
        )
        assert int(preview_headers["x-forgecad-triangle-count"]) > 0
        preview_shape_sha256 = preview_headers[
            "x-forgecad-shape-program-sha256"
        ]

        model_status, model_headers, model_body = _asset_api_get(
            service,
            f"/api/v1/agent/asset-versions/{version.asset_version_id}:model.glb"
        )
        assert model_status == 200
        assert model_headers["content-type"].startswith("model/gltf-binary")
        model_sha256 = hashlib.sha256(model_body).hexdigest()
        assert model_sha256 != preview_sha256
        assert model_headers["etag"] == f'"{model_sha256}"'
        assert model_headers["x-forgecad-glb-sha256"] == model_sha256
        assert model_headers["x-forgecad-artifact-profile"] == (
            "production_concept"
        )
        assert model_headers[
            "x-forgecad-artifact-profile-sha256"
        ] == geometry_artifact_profile_manifest("production_concept")[
            "profile_sha256"
        ]
        assert model_headers[
            "x-forgecad-shape-program-sha256"
        ] == preview_shape_sha256
        assert int(model_headers["x-forgecad-glb-byte-size"]) == len(
            model_body
        )
        assert int(model_headers["x-forgecad-triangle-count"]) > int(
            preview_headers["x-forgecad-triangle-count"]
        )
        assert model_headers["content-disposition"] == (
            f'attachment; filename="{version.asset_version_id}.glb"'
        )

        report = service.quality(
            version.asset_version_id,
            expected_revision=_revision(factory, version.project_id),
            idempotency_key="m108-production-api-quality",
        )
        assert report.compile_readback is not None
        assert report.compile_readback.glb_sha256 == model_sha256
        assert report.compile_readback.glb_byte_size == len(model_body)
        exported = service.export_glb(version.asset_version_id)
        assert base64.b64decode(
            exported.glb_base64,
            validate=True,
        ) == model_body

        # The second HTTP service has no in-memory result.  Disabling the
        # compiler proves both first repeat-hit and restart replay use CAS.
        from forgecad_agent.application import agent_asset_editing as editing

        original_compile = editing.compile_production_concept_shape_program
        editing.compile_production_concept_shape_program = (
            lambda _program: (_ for _ in ()).throw(
                AssertionError("restart/repeat cache hit recompiled production GLB")
            )
        )
        try:
            restarted = AgentAssetEditingService(factory, object_store)
            for _ in range(2):
                replay_status, replay_headers, replay_body = _asset_api_get(
                    restarted,
                    f"/api/v1/agent/asset-versions/"
                    f"{version.asset_version_id}:model.glb"
                )
                assert replay_status == 200
                assert replay_body == model_body
                assert replay_headers["x-forgecad-glb-sha256"] == model_sha256
        finally:
            editing.compile_production_concept_shape_program = original_compile

        # Authoritative SQLite/event state may contain hashes, never binary or
        # base64 production-model payloads.  A legacy blockout table still has
        # a glb_base64 column for interactive artifacts, so test the exact
        # production bytes instead of pretending the historical schema is gone.
        # CAS indexes likewise reference object paths.
        database_bytes = database_path.read_bytes()
        assert model_body[:128] not in database_bytes
        assert base64.b64encode(model_body) not in database_bytes
        index_paths = tuple(
            (root / "object-store" / "derived" / "production-concept").glob(
                "*.json"
            )
        )
        assert index_paths
        for index_path in index_paths:
            index_text = index_path.read_text(encoding="utf-8")
            assert "glb_base64" not in index_text
            assert base64.b64encode(model_body[:256]).decode(
                "ascii"
            ) not in index_text


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    production_profile = geometry_artifact_profile_manifest(
        "production_concept"
    )
    preview_profile = geometry_artifact_profile_manifest(
        "interactive_preview"
    )
    assert production_profile["texture_width"] == 1024
    assert preview_profile["texture_width"] == 128
    assert (
        builtin_visual_texture_cache_facts()["production_entry_count"] == 0
    )

    production_results = []
    mip_checked_material_ids: set[str] = set()
    for brief_index, brief in enumerate(BRIEFS):
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(
            brief=brief,
            pack=pack,
            project_id="prj_m108_production",
            action_loop_enabled=False,
        )
        candidate = next(
            item
            for item in list_blockout_variants(pack.pack_id)
            if item.endswith("_a")
        )
        result = build_blockout(
            plan,
            plan.directions[0].direction_id,
            variant_id=candidate,
            presentation_profile="showcase",
        )
        preview = compile_preview_shape_program(result.shape_program)
        production = compile_production_concept_shape_program(
            result.shape_program
        )
        repeated = compile_production_concept_shape_program(
            result.shape_program
        )
        production_results.append(production)

        assert production.glb_bytes == repeated.glb_bytes
        assert production.readback == repeated.readback
        assert (
            preview.readback.shape_program_sha256
            == production.readback.shape_program_sha256
        )
        assert preview.readback.bounds_mm == production.readback.bounds_mm
        assert preview.readback.operation_ids == production.readback.operation_ids
        assert preview.readback.output_roles == production.readback.output_roles
        assert preview.readback.material_ids == production.readback.material_ids
        assert _surface_identity(preview.readback) == _surface_identity(
            production.readback
        )
        assert _zone_identity(preview.readback) == _zone_identity(
            production.readback
        )
        assert _feature_identity(preview.readback) == _feature_identity(
            production.readback
        )
        assert (
            production.readback.triangle_count
            > preview.readback.triangle_count
        )
        assert production.readback.glb_sha256 != preview.readback.glb_sha256
        assert (
            production.readback.artifact_profile is not None
            and production.readback.artifact_profile.artifact_profile_id
            == "production_concept"
        )
        assert (
            preview.readback.artifact_profile is not None
            and preview.readback.artifact_profile.artifact_profile_id
            == "interactive_preview"
        )
        production_facts = read_shape_program_glb_facts(
            production.glb_bytes
        )
        assert production_facts.artifact_profile == production_profile
        assert {
            (
                int(texture_map["width"]),
                int(texture_map["height"]),
            )
            for texture_set in production_facts.visual_texture_sets
            for texture_map in texture_set["maps"]
        } == {(1024, 1024)}
        assert all(
            str(texture_set["visual_texture_set_id"]).endswith(
                "_builtin_v4"
            )
            for texture_set in production_facts.visual_texture_sets
        )
        if brief_index == 0:
            cache_facts = builtin_visual_texture_cache_facts()
            assert (
                0
                < cache_facts["production_entry_count"]
                < builtin_visual_material_count()
            ), cache_facts
            _assert_profile_tamper_rejected(production.glb_bytes)
            _assert_cache_roundtrip(
                asset_version_id="assetver_m108_production_cache",
                compiled=production,
                shape_program=result.shape_program,
            )
        _assert_mip_aware_texture_visibility(
            production.glb_bytes,
            mip_checked_material_ids,
        )

    assert len(production_results) == 4
    assert mip_checked_material_ids == {
        "mat_primary",
        "mat_aluminum",
        "mat_signal_red",
        "mat_composite",
        "mat_rubber",
        "mat_dark_glass",
        "mat_emissive_blue",
        "mat_automotive_paint",
    }
    _assert_binary_api_and_restart_cache()
    print(
        "M108 production concept smoke passed: one ShapeProgram yields "
        "deterministic preview/production GLBs, 1K PBR readback and verified CAS"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
