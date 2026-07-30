from __future__ import annotations

import asyncio
import base64
import copy
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from typing import Any

import numpy as np
import pytest
from fastapi import FastAPI

from forgecad_agent.api.restricted_geometry_routes import (
    RESTRICTED_GEOMETRY_CAPABILITY_HEADER,
    RESTRICTED_GEOMETRY_INTERNAL_PREFIX,
)
from forgecad_agent.application.restricted_geometry_executor import (
    RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV,
    RestrictedGeometryBoundaryError,
    RestrictedGeometryCancellationRequest,
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
    _safe_geometry_error_details,
    _stable_geometry_error_code,
    sanitize_restricted_geometry_child_environment,
)
from forgecad_agent.application.shape_program import ShapeProgramValidationError
from forgecad_agent.application.shape_program_runtime import (
    UnsupportedRuntimeOperationError,
)
from forgecad_agent.application.profile_contracts import canonical_profile_payload
from forgecad_agent.application.surface_layer_pbr import (
    surface_layer_lowering_sha256,
    surface_layer_material_id,
)
from forgecad_agent.application.reference_uv_projection import _encode_png_rgb
from wushen_agent import main as main_module


CAPABILITY = "a" * 64
VALID_PROGRAM = {
    "schema_version": "ShapeProgram@1",
    "program_id": "shape_k003_restricted_geometry",
    "units": "millimeter",
    "seed": 17,
    "triangle_budget": 1000,
    "parameters": [],
    "operations": [
        {
            "operation_id": "op_body",
            "op": "box",
            "inputs": [],
            "args": {
                "size": [100, 40, 20],
                "part_role": "body_shell",
                "material_id": "mat_graphite",
                "zone_id": "zone_body_shell",
            },
        }
    ],
    "outputs": [
        {
            "output_id": "output_body",
            "operation_id": "op_body",
            "kind": "mesh",
            "part_role": "body_shell",
        }
    ],
    "non_functional_only": True,
}


def test_shape_program_worker_error_exposes_only_the_code_owned_prefix() -> None:
    error = ShapeProgramValidationError(
        "SHAPE_PROGRAM_EXTRUDE_REQUIRES_PROFILE: op_provider_private"
    )
    assert _stable_geometry_error_code(error) == "SHAPE_PROGRAM_EXTRUDE_REQUIRES_PROFILE"
    assert _stable_geometry_error_code(ShapeProgramValidationError("not safe")) == (
        "SHAPE_PROGRAM_INVALID"
    )


def test_unsupported_runtime_operation_detail_is_a_fixed_label_not_provider_text() -> None:
    assert _safe_geometry_error_details(
        UnsupportedRuntimeOperationError(
            operation_id="op_provider_private",
            op="fillet",
            reason="operation is not declared by ShapeProgramRuntimeManifest@1",
        )
    ) == {"unsupported_operation": "fillet"}
    assert _safe_geometry_error_details(
        UnsupportedRuntimeOperationError(
            operation_id="op_provider_private",
            op="customer_private_operation",
            reason="provider supplied text must not be projected",
        )
    ) == {}
PROFILE_SKETCH = {
    "schema_version": "ProfileSketch@1",
    "sketch_id": "sketch_k003_companion",
    "version": 1,
    "plane": "cross_section",
    "closed": True,
    "winding": "counter_clockwise",
    "start": [-0.5, -0.5],
    "segments": [
        {"kind": "line", "to": [0.5, -0.5]},
        {"kind": "line", "to": [0.5, 0.5]},
        {"kind": "line", "to": [-0.5, 0.5]},
        {"kind": "line", "to": [-0.5, -0.5]},
    ],
    "holes": [],
    "normalized_bounds": {"min": [-0.5, -0.5], "max": [0.5, 0.5]},
    "symmetry": "none",
    "continuity_hint": "linear",
    "resample_count": 8,
    "provenance": {"source": "agent"},
}
SECTION_SET = {
    "schema_version": "ProfileSectionSet@1",
    "section_set_id": "sectionset_k003_companion",
    "version": 1,
    "main_axis": "x",
    "profiles": [PROFILE_SKETCH],
    "sections": [
        {
            "section_id": "section_k003_start",
            "position": -1.0,
            "profile_sketch_id": "sketch_k003_companion",
            "scale": 1.0,
            "twist_degrees": 0.0,
            "cap_policy": "start",
        },
        {
            "section_id": "section_k003_end",
            "position": 1.0,
            "profile_sketch_id": "sketch_k003_companion",
            "scale": 1.0,
            "twist_degrees": 0.0,
            "cap_policy": "end",
        },
    ],
    "resample_policy": {"mode": "uniform_count", "count": 8},
    "symmetry": "none",
    "provenance": {"source": "agent"},
}


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def _surface_layer_lowering() -> dict[str, Any]:
    retained = {
        "vector_paths": [{
            "path_id": "path_k003_surface",
            "closed": False,
            "commands": [
                {"kind": "move", "points": [[0.1, 0.2]]},
                {"kind": "line", "points": [[0.8, 0.7]]},
            ],
        }],
        "decal_layers": [],
        "roughness_masks": [{
            "mask_id": "rough_k003_surface",
            "motif": "edge_wear",
            "coverage": "edge_band",
            "intensity_milli": 180,
            "seed": 18,
        }],
        "emissive_masks": [{
            "mask_id": "emissive_k003_surface",
            "motif": "double_flowline",
            "color_token": "accent_blue",
            "coverage": "center_band",
            "intensity_milli": 220,
            "seed": 19,
        }],
        "symmetry": {"mode": "none", "center_uv": [0.5, 0.5]},
        "uv_frame": {
            "frame_id": "uvframe_k003_surface",
            "u_min": 0.0,
            "u_max": 1.0,
            "v_min": 0.0,
            "v_max": 1.0,
            "rotation_degrees": 0.0,
        },
        "quality_profile": "interactive_preview",
    }
    source = {
        "program": "surface_layer_k003",
        "retained": retained,
        "base_material": "mat_graphite",
    }
    source_sha = hashlib.sha256(_canonical(source).encode("utf-8")).hexdigest()
    lowering = {
        "schema_version": "SurfaceLayerLowering@1",
        "source_program_sha256": source_sha,
        "adornments": [{
            "schema_version": "SurfaceAdornmentProgram@1",
            "program_id": f"adorn_{source_sha[:40]}_1",
            "target_part_id": "part_body_shell",
            "target_zone_id": "zone_body_shell",
            "kind": "normal_relief",
            "motif": "parallel_groove",
            "intensity": "subtle",
            "coverage": "center_band",
            "seed": 17,
            "base_material": "mat_graphite",
            "execution": "texture_bake",
            "skill_id": "skill_first_party_surface_adornment",
            "skill_version": 2,
            "skill_sha256": "a" * 64,
            "generator": "a005_v1",
            "non_functional_only": True,
        }],
        "retained_layers": retained,
        "retained_layers_sha256": hashlib.sha256(
            _canonical(retained).encode("utf-8")
        ).hexdigest(),
    }
    return lowering


def _sealed_surface_layer_input() -> dict[str, Any]:
    lowering = _surface_layer_lowering()
    return {
        "schema_version": "RestrictedSurfaceLayerInput@1",
        "lowering": lowering,
        "lowering_sha256": surface_layer_lowering_sha256(lowering),
    }


def _reference_uv_evidence_bake(*, zone_id: str = "zone_body_shell") -> dict[str, Any]:
    source = np.zeros((2, 2, 3), dtype=np.uint8)
    source[:, :, :] = (210, 214, 220)
    source_png = _encode_png_rgb(source)
    return {
        "schema_version": "ReferenceUvEvidenceBake@1",
        "projection_id": "projection_k003_front",
        "source_evidence_id": "evidence_k003_front",
        "source_image_sha256": hashlib.sha256(source_png).hexdigest(),
        "source_png_base64": base64.b64encode(source_png).decode("ascii"),
        "camera_hypothesis_id": "camera_k003_front",
        "camera_provenance_sha256": "b" * 64,
        "target_material_zone_id": zone_id,
        "texture_width": 128,
        "texture_height": 128,
        "observed_uv_rect_bps": [2500, 2500, 7500, 7500],
    }


def _reference_camera_uv_raster_bake(*, zone_id: str = "zone_body_shell") -> dict[str, Any]:
    source = np.zeros((8, 8, 3), dtype=np.uint8)
    source[:, :, :] = (210, 214, 220)
    source_png = _encode_png_rgb(source)
    return {
        "schema_version": "ReferenceCameraUvRasterBake@2",
        "projection_id": "projection_k003_camera_front",
        "source_evidence_id": "evidence_k003_camera_front",
        "source_image_sha256": hashlib.sha256(source_png).hexdigest(),
        "source_png_base64": base64.b64encode(source_png).decode("ascii"),
        "camera_hypothesis_id": "camera_k003_camera_front",
        "camera_provenance_sha256": "c" * 64,
        "target_material_zone_id": zone_id,
        "texture_width": 128,
        "texture_height": 128,
        # This fixture's 100×40×20 mm box is centred at the origin, so the
        # identity clip transform is a deterministic bounded test camera.
        "world_to_clip_row_major": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ],
    }


@dataclass(frozen=True)
class _Response:
    status_code: int
    body: bytes

    def json(self) -> Any:
        return json.loads(self.body.decode("utf-8"))


class _AsgiClient:
    def __init__(self, app: FastAPI, *, client_host: str = "testclient") -> None:
        self.app = app
        self.client_host = client_host

    def get(self, path: str, *, headers: dict[str, str] | None = None) -> _Response:
        return asyncio.run(self._request("GET", path, headers or {}, b""))

    def post(
        self,
        path: str,
        *,
        headers: dict[str, str] | None = None,
        json_payload: Any | None = None,
        content: bytes | None = None,
    ) -> _Response:
        if json_payload is not None and content is not None:
            raise ValueError("json_payload and content are mutually exclusive")
        body = (
            json.dumps(json_payload, ensure_ascii=False).encode("utf-8")
            if json_payload is not None
            else (content or b"")
        )
        request_headers = dict(headers or {})
        if json_payload is not None:
            request_headers["Content-Type"] = "application/json"
        return asyncio.run(self._request("POST", path, request_headers, body))

    async def _request(
        self,
        method: str,
        path: str,
        headers: dict[str, str],
        body: bytes,
    ) -> _Response:
        incoming = True
        outgoing: list[dict[str, Any]] = []

        async def receive() -> dict[str, Any]:
            nonlocal incoming
            if incoming:
                incoming = False
                return {"type": "http.request", "body": body, "more_body": False}
            return {"type": "http.disconnect"}

        async def send(message: dict[str, Any]) -> None:
            outgoing.append(message)

        scope = {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": method,
            "scheme": "http",
            "path": path,
            "raw_path": path.encode("ascii"),
            "query_string": b"",
            "root_path": "",
            "headers": [
                (name.lower().encode("ascii"), value.encode("ascii"))
                for name, value in headers.items()
            ],
            "client": (self.client_host, 50000),
            "server": ("testserver", 80),
            "state": {},
        }
        await self.app(scope, receive, send)
        start = next(item for item in outgoing if item["type"] == "http.response.start")
        response_body = b"".join(
            item.get("body", b"") for item in outgoing if item["type"] == "http.response.body"
        )
        return _Response(status_code=start["status"], body=response_body)


def _headers(token: str = CAPABILITY) -> dict[str, str]:
    return {RESTRICTED_GEOMETRY_CAPABILITY_HEADER: token}


def _execution_payload(
    *,
    execution_id: str = "exec_compile_1",
    idempotency_key: str = "idem_compile_1",
    cancellation_id: str = "cancel_compile_1",
    cancellation_token: str = "cancel_token_compile_1",
    timeout_ms: int = 20_000,
) -> dict[str, Any]:
    return {
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": execution_id,
        "idempotency_key": idempotency_key,
        "cancellation_id": cancellation_id,
        "cancellation_token": cancellation_token,
        "action": "compile_readback",
        "timeout_ms": timeout_ms,
        "artifact_profile_id": "interactive_preview",
        "shape_program": VALID_PROGRAM,
    }


def test_surface_layer_input_is_exactly_sealed_and_binds_retained_pbr_to_the_final_glb_zone() -> None:
    payload = _execution_payload(execution_id="exec_surface_layer")
    sealed = _sealed_surface_layer_input()
    payload["surface_layer_input"] = sealed
    payload["surface_adornment_programs"] = sealed["lowering"]["adornments"]
    request = RestrictedGeometryExecutionRequest.model_validate(payload)
    assert request.surface_layer_input == sealed

    executor = RestrictedGeometryExecutor(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    result = executor.execute(request)
    assert result.action == "compile_readback"
    assert result.readback is not None
    retained = next(
        item
        for item in result.readback["visual_texture_sets"]
        if item.get("surface_layer_lowering") is not None
    )
    lowering = sealed["lowering"]
    assert result.readback["material_count"] == 9
    assert retained["material_id"] == surface_layer_material_id(lowering)
    assert retained["texture_material_id"] == surface_layer_material_id(lowering)
    assert retained["material_zone_ids"] == ["zone_body_shell"]
    assert retained["surface_layer_lowering"] == lowering
    assert retained["surface_layer_lowering_sha256"] == sealed["lowering_sha256"]
    assert retained["surface_layer_retained_layers_sha256"] == lowering["retained_layers_sha256"]
    assert {item["texture_role"] for item in retained["maps"]} == {
        "base_color", "metallic_roughness", "normal", "occlusion", "emissive"
    }

    forged = copy.deepcopy(payload)
    forged["surface_layer_input"]["lowering_sha256"] = "0" * 64
    with pytest.raises(ValueError, match="surface layer input seal"):
        RestrictedGeometryExecutionRequest.model_validate(forged)

    mismatched = copy.deepcopy(payload)
    mismatched["surface_adornment_programs"] = []
    with pytest.raises(ValueError, match="exact Rust-lowered A005"):
        RestrictedGeometryExecutionRequest.model_validate(mismatched)


def test_reference_uv_evidence_bake_is_bound_to_one_retained_zone_and_read_back_from_the_final_glb() -> None:
    payload = _execution_payload(execution_id="exec_reference_uv")
    sealed = _sealed_surface_layer_input()
    payload["surface_layer_input"] = sealed
    payload["surface_adornment_programs"] = sealed["lowering"]["adornments"]
    payload["reference_uv_evidence_bakes"] = [_reference_uv_evidence_bake()]

    executor = RestrictedGeometryExecutor(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    result = executor.execute(RestrictedGeometryExecutionRequest.model_validate(payload))
    assert result.readback is not None
    retained = next(
        item
        for item in result.readback["visual_texture_sets"]
        if item.get("surface_layer_lowering") is not None
    )
    receipt = retained["reference_uv_evidence"]
    base_color = next(item for item in retained["maps"] if item["texture_role"] == "base_color")
    assert receipt["target_material_zone_id"] == "zone_body_shell"
    assert receipt["base_color_texture_id"] == base_color["texture_id"]
    assert receipt["base_color_sha256"] == base_color["sha256"]
    assert base_color["source"] == "imported_reference"
    assert receipt["observed_texel_count"] == 64 * 64
    assert receipt["unobserved_texel_count"] == 128 * 128 - 64 * 64

    wrong_zone = copy.deepcopy(payload)
    wrong_zone["reference_uv_evidence_bakes"][0]["target_material_zone_id"] = "zone_not_bound"
    with pytest.raises(ValueError, match="exact sealed retained surface layer"):
        RestrictedGeometryExecutionRequest.model_validate(wrong_zone)


def test_camera_uv_raster_bake_is_depth_filtered_and_read_back_from_the_final_glb() -> None:
    payload = _execution_payload(execution_id="exec_reference_camera_raster")
    sealed = _sealed_surface_layer_input()
    payload["surface_layer_input"] = sealed
    payload["surface_adornment_programs"] = sealed["lowering"]["adornments"]
    payload["reference_uv_evidence_bakes"] = [_reference_camera_uv_raster_bake()]

    executor = RestrictedGeometryExecutor(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    result = executor.execute(RestrictedGeometryExecutionRequest.model_validate(payload))
    assert result.readback is not None
    retained = next(
        item
        for item in result.readback["visual_texture_sets"]
        if item.get("surface_layer_lowering") is not None
    )
    receipt = retained["reference_uv_evidence"]
    assert receipt["schema_version"] == "ReferenceCameraUvRasterBakeReceipt@2"
    assert receipt["algorithm_id"] == "forgecad.reference_camera_uv_raster"
    assert receipt["world_to_clip_sha256"] == hashlib.sha256(
        json.dumps(_reference_camera_uv_raster_bake()["world_to_clip_row_major"], separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    assert receipt["raster_triangle_count"] > 0
    assert receipt["observed_texel_count"] > 0
    assert receipt["observed_texel_count"] + receipt["unobserved_texel_count"] == 128 * 128


def test_multi_zone_surface_layer_inputs_bake_distinct_retained_pbr_materials() -> None:
    payload = _execution_payload(execution_id="exec_surface_layers_multi")
    program = copy.deepcopy(VALID_PROGRAM)
    program["operations"].append({
        "operation_id": "op_trim",
        "op": "box",
        "inputs": [],
        "args": {
            "size": [40, 18, 8],
            "part_role": "accent_trim",
            "material_id": "mat_graphite",
            "zone_id": "zone_trim",
        },
    })
    program["outputs"].append({
        "output_id": "output_trim",
        "operation_id": "op_trim",
        "kind": "mesh",
        "part_role": "accent_trim",
    })
    first = _sealed_surface_layer_input()
    second = _sealed_surface_layer_input()
    second["lowering"]["adornments"][0]["program_id"] = "adorn_k003_surface_trim"
    second["lowering"]["adornments"][0]["target_part_id"] = "part_trim"
    second["lowering"]["adornments"][0]["target_zone_id"] = "zone_trim"
    second["lowering_sha256"] = surface_layer_lowering_sha256(second["lowering"])
    payload["shape_program"] = program
    payload["surface_layer_inputs"] = [first, second]
    payload["surface_adornment_programs"] = (
        first["lowering"]["adornments"] + second["lowering"]["adornments"]
    )
    request = RestrictedGeometryExecutionRequest.model_validate(payload)
    executor = RestrictedGeometryExecutor(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    result = executor.execute(request)
    assert result.readback is not None
    retained = [
        item for item in result.readback["visual_texture_sets"]
        if item.get("surface_layer_lowering") is not None
    ]
    assert {tuple(item["material_zone_ids"]) for item in retained} == {
        ("zone_body_shell",),
        ("zone_trim",),
    }

    duplicate = copy.deepcopy(payload)
    duplicate["surface_layer_inputs"] = [first, copy.deepcopy(first)]
    with pytest.raises(ValueError, match="duplicate material zones"):
        RestrictedGeometryExecutionRequest.model_validate(duplicate)


def test_surface_layer_rejects_a_missing_zone_and_a_forged_retained_seal() -> None:
    payload = _execution_payload(execution_id="exec_surface_layer_bad_zone")
    sealed = _sealed_surface_layer_input()
    sealed["lowering"]["adornments"][0]["target_zone_id"] = "zone_missing"
    sealed["lowering_sha256"] = surface_layer_lowering_sha256(sealed["lowering"])
    payload["surface_layer_input"] = sealed
    payload["surface_adornment_programs"] = sealed["lowering"]["adornments"]
    request = RestrictedGeometryExecutionRequest.model_validate(payload)
    executor = RestrictedGeometryExecutor(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    with pytest.raises(RestrictedGeometryBoundaryError, match="restricted geometry worker rejected"):
        executor.execute(request)

    forged = copy.deepcopy(payload)
    forged["surface_layer_input"]["lowering"]["retained_layers"]["roughness_masks"][0]["seed"] = 99
    # The exact retained hash is part of the Rust-owned seal. Reusing it after
    # changing even a bounded visual value must fail before any worker starts.
    with pytest.raises(ValueError, match="retained hash"):
        RestrictedGeometryExecutionRequest.model_validate(forged)


def _crash_worker(pipe: Any, _cancel: Any, _payload: dict[str, Any], _root: str | None) -> None:
    pipe.close()
    os._exit(17)


def _hang_worker(pipe: Any, _cancel: Any, _payload: dict[str, Any], _root: str | None) -> None:
    time.sleep(2)
    pipe.close()


def _late_worker(pipe: Any, _cancel: Any, _payload: dict[str, Any], _root: str | None) -> None:
    time.sleep(0.15)
    pipe.send({"ok": True, "result": {}})
    pipe.close()


def _environment_probe(pipe: Any, resource_root: str) -> None:
    os.environ["WUSHEN_LIBRARY_ROOT"] = "/private/forbidden-library"
    os.environ["FORGECAD_AGENT_API_KEY"] = "secret-like-value"
    os.environ["FORGECAD_ACTIVE_DESIGN_SNAPSHOT_WRITE_TOKEN"] = "write-token"
    sanitize_restricted_geometry_child_environment(resource_root)
    pipe.send(dict(os.environ))
    pipe.close()


def test_default_app_is_geometry_only_and_legacy_environment_cannot_reenable_product_core(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_legacy(_environment: dict[str, str]) -> FastAPI:
        raise AssertionError("legacy product core was constructed")

    monkeypatch.setattr(
        main_module,
        "_create_test_only_legacy_product_core",
        fail_legacy,
    )
    app = main_module.create_app(
        environment={
            "FORGECAD_K001_PACKAGED_PROBE": "1",
            "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE": "1",
            "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE": "1",
            RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY,
            "FORGECAD_SUPERVISOR_SESSION_ID": "a" * 32,
            "WUSHEN_LIBRARY_ROOT": "/must-not-reach-python-geometry",
            "FORGECAD_AGENT_API_KEY": "must-not-reach-python-geometry",
        }
    )
    assert app.state.product_state_owner == "rust_forgecad_core"
    assert app.state.persistent_state_writer is False
    client = _AsgiClient(app)
    health = client.get("/api/health")
    assert health.status_code == 200
    assert health.json()["database_access"] is False
    assert health.json()["provider_access"] is False
    assert health.json()["snapshot_write"] is False
    assert health.json()["supervisor_session_id"] == "a" * 32
    assert isinstance(health.json()["supervisor_process_group_id"], int)
    assert client.post("/api/v1/agent/threads", json_payload={}).status_code == 410
    assert (
        client.post("/api/v1/internal/k002/lifecycle/execute", json_payload={}).status_code == 410
    )


def test_default_import_graph_does_not_load_legacy_product_or_persistence_modules() -> None:
    agent_root = Path(__file__).resolve().parents[1]
    environment = dict(os.environ)
    environment["PYTHONPATH"] = os.pathsep.join(
        [
            str(agent_root),
            environment.get("PYTHONPATH", ""),
        ]
    ).rstrip(os.pathsep)
    environment["FORGECAD_K001_PACKAGED_PROBE"] = "1"
    environment["FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE"] = "1"
    environment["FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE"] = "1"
    environment[RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV] = CAPABILITY
    probe = subprocess.run(
        [
            sys.executable,
            "-c",
            (
                "import json, sys; import wushen_agent.main; "
                "print(json.dumps(sorted(name for name in sys.modules "
                "if name.startswith(('forgecad_agent', 'wushen_agent')))))"
            ),
        ],
        cwd=agent_root.parents[1],
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    loaded = set(json.loads(probe.stdout))
    assert loaded == {
        "forgecad_agent",
        "forgecad_agent.api",
        "forgecad_agent.api.factory",
        "forgecad_agent.api.restricted_geometry_routes",
        "forgecad_agent.application",
        "forgecad_agent.application.restricted_geometry_executor",
        "wushen_agent",
        "wushen_agent.main",
    }


def test_legacy_product_core_requires_explicit_direct_test_factory(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    sentinel = FastAPI()
    sentinel.state.test_only_legacy_product_core = True

    def fake_legacy(_environment: dict[str, str]) -> FastAPI:
        return sentinel

    monkeypatch.setattr(
        main_module,
        "_create_test_only_legacy_product_core",
        fake_legacy,
    )
    hostile_environment = {
        "FORGECAD_K001_PACKAGED_PROBE": "1",
        "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE": "1",
        "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE": "1",
    }
    assert main_module.create_app(environment=hostile_environment) is not sentinel
    assert (
        main_module.create_test_only_legacy_product_core_app(environment=hostile_environment)
        is sentinel
    )


def test_capability_is_loopback_only_and_reports_no_product_authority() -> None:
    app = main_module.create_restricted_geometry_app(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    path = f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/capability/ownership"
    client = _AsgiClient(app)
    assert client.get(path).status_code == 403
    assert client.get(path, headers=_headers("wrong-token")).status_code == 403
    response = client.get(path, headers=_headers())
    assert response.status_code == 200
    payload = response.json()
    assert payload["python_role"] == "restricted_geometry_executor"
    assert payload["database_access"] is False
    assert payload["object_store_access"] is False
    assert payload["provider_access"] is False
    assert payload["thread_session_access"] is False
    assert payload["snapshot_write"] is False
    assert payload["accepts_caller_glb"] is False
    assert payload["actions"] == ["compile_readback", "render"]
    remote = _AsgiClient(app, client_host="192.0.2.10")
    assert remote.get(path, headers=_headers()).status_code == 403

    execute_path = f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/execute"
    wrong_media = client.post(
        execute_path,
        headers=_headers(),
        content=b"{}",
    )
    assert wrong_media.status_code == 415
    assert wrong_media.json()["error"]["code"] == ("GEOMETRY_REQUEST_MEDIA_TYPE_INVALID")


@pytest.mark.parametrize(
    ("field", "value", "expected_code"),
    [
        ("database_path", "/private/library.db", "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("provider_key", "secret", "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("snapshot", {"revision": 4}, "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("glb_base64", "Z2xURg==", "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("plan", {"steps": []}, "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("style_token", {"profile": "industrial"}, "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("component_recipe", {"recipe_id": "forbidden"}, "GEOMETRY_CONTEXT_FORBIDDEN"),
        ("reference", "https://example.invalid/model.glb", "GEOMETRY_MACHINE_LOCATION_FORBIDDEN"),
    ],
)
def test_request_schema_fails_closed_before_geometry_execution(
    field: str,
    value: Any,
    expected_code: str,
) -> None:
    app = main_module.create_restricted_geometry_app(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    payload = _execution_payload()
    payload[field] = value
    response = _AsgiClient(app).post(
        f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/execute",
        headers=_headers(),
        json_payload=payload,
    )
    assert response.status_code == 400
    assert response.json()["error"]["code"] == expected_code


def test_profile_and_section_companions_must_be_strict_canonical_ir_witnesses() -> None:
    canonical_profile, _profile_json, profile_sha256 = canonical_profile_payload(PROFILE_SKETCH)
    canonical_sections, _section_json, section_sha256 = canonical_profile_payload(SECTION_SET)
    program = json.loads(json.dumps(VALID_PROGRAM))
    program["profile_inputs"] = [
        {
            "input_id": "profileinput_k003_profile",
            "input_kind": "profile_sketch",
            "contract_version": "ProfileSketch@1",
            "input_sha256": profile_sha256,
            "canonical_payload": canonical_profile,
        },
        {
            "input_id": "profileinput_k003_sections",
            "input_kind": "profile_section_set",
            "contract_version": "ProfileSectionSet@1",
            "input_sha256": section_sha256,
            "canonical_payload": canonical_sections,
        },
    ]
    request = _execution_payload(
        execution_id="exec_companions",
        idempotency_key="idem_companions",
        cancellation_id="cancel_companions",
        cancellation_token="cancel_token_companions",
    )
    request["shape_program"] = program
    request["profile_sketch"] = PROFILE_SKETCH
    request["section_set"] = SECTION_SET
    app = main_module.create_restricted_geometry_app(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    client = _AsgiClient(app)
    response = client.post(
        f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/execute",
        headers=_headers(),
        json_payload=request,
    )
    assert response.status_code == 200, response.json()

    unbound = _execution_payload(
        execution_id="exec_unbound_companion",
        idempotency_key="idem_unbound_companion",
        cancellation_id="cancel_unbound_companion",
        cancellation_token="cancel_token_unbound_companion",
    )
    unbound["profile_sketch"] = PROFILE_SKETCH
    rejected = client.post(
        f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/execute",
        headers=_headers(),
        json_payload=unbound,
    )
    assert rejected.status_code == 422
    assert rejected.json()["error"]["code"] == "GEOMETRY_PROFILE_COMPANION_UNBOUND"


def test_compile_readback_then_render_uses_only_opaque_ephemeral_handle() -> None:
    app = main_module.create_restricted_geometry_app(
        environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
    )
    client = _AsgiClient(app)
    execute_path = f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/execute"
    request = _execution_payload()
    compiled_response = client.post(
        execute_path,
        headers=_headers(),
        json_payload=request,
    )
    assert compiled_response.status_code == 200, compiled_response.json()
    compiled = compiled_response.json()
    glb = base64.b64decode(compiled["glb_base64"], validate=True)
    assert glb[:4] == b"glTF"
    assert hashlib.sha256(glb).hexdigest() == compiled["glb_sha256"]
    assert compiled["readback"]["glb_sha256"] == compiled["glb_sha256"]
    assert compiled["readback"]["shape_program_sha256"] == compiled["shape_program_sha256"]
    assert compiled["readback"]["triangle_count"] == compiled["triangle_count"]
    assert compiled["readback"]["bounds_mm"] == compiled["bounds_mm"]
    assert compiled["readback"]["mesh_count"] > 0
    assert compiled["readback"]["primitive_count"] > 0
    assert compiled["readback"]["material_count"] > 0
    assert compiled["readback"]["closed_manifold"] is True
    assert compiled["readback"]["surface_provenance_present"] is True
    assert compiled["artifact_handle"].startswith("geomart_")

    replay = client.post(
        execute_path,
        headers=_headers(),
        json_payload=request,
    )
    assert replay.status_code == 200
    assert replay.body == compiled_response.body

    render_request = {
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": "exec_render_1",
        "idempotency_key": "idem_render_1",
        "cancellation_id": "cancel_render_1",
        "cancellation_token": "cancel_token_render_1",
        "action": "render",
        "timeout_ms": 20_000,
        "artifact_handle": compiled["artifact_handle"],
        "shape_program_sha256": compiled["shape_program_sha256"],
        "render": {"width": 64, "height": 64, "exploded_parts": []},
    }
    rendered_response = client.post(
        execute_path,
        headers=_headers(),
        json_payload=render_request,
    )
    assert rendered_response.status_code == 200, rendered_response.json()
    rendered = rendered_response.json()
    assert rendered["action"] == "render"
    assert rendered["artifact_handle"] == compiled["artifact_handle"]
    assert rendered["glb_sha256"] == compiled["glb_sha256"]
    assert rendered.get("glb_base64") is None
    assert rendered["renderer_id"] == "forgecad-agent-software-raster@1"
    assert set(rendered["render_views"]) == {"iso", "front", "side", "top"}
    for view_id, encoded in rendered["render_views"].items():
        png = base64.b64decode(encoded, validate=True)
        assert png.startswith(b"\x89PNG\r\n\x1a\n")
        assert hashlib.sha256(png).hexdigest() == rendered["render_view_sha256"][view_id]

    convergence_request = copy.deepcopy(render_request)
    convergence_request.update(
        execution_id="exec_render_convergence",
        idempotency_key="idem_render_convergence",
        cancellation_id="cancel_render_convergence",
        cancellation_token="cancel_token_render_convergence",
    )
    convergence_request["render"]["view_profile"] = "convergence_eight"
    convergence_response = client.post(
        execute_path,
        headers=_headers(),
        json_payload=convergence_request,
    )
    assert convergence_response.status_code == 200, convergence_response.json()
    convergence = convergence_response.json()
    assert set(convergence["render_views"]) == {
        "iso",
        "front",
        "back",
        "left",
        "right",
        "top",
        "gripper_iso",
        "gripper_front",
    }
    assert convergence["render_view_sha256"]["gripper_iso"] != convergence["render_view_sha256"]["iso"]
    assert convergence["render_view_sha256"]["gripper_front"] != convergence["render_view_sha256"]["front"]

    turntable_request = copy.deepcopy(render_request)
    turntable_request.update(
        execution_id="exec_render_turntable",
        idempotency_key="idem_render_turntable",
        cancellation_id="cancel_render_turntable",
        cancellation_token="cancel_token_render_turntable",
    )
    turntable_request["render"]["view_profile"] = "turntable_eight"
    turntable_response = client.post(
        execute_path,
        headers=_headers(),
        json_payload=turntable_request,
    )
    assert turntable_response.status_code == 200, turntable_response.json()
    turntable = turntable_response.json()
    assert set(turntable["render_views"]) == {
        f"turntable_{angle:03d}" for angle in range(0, 360, 45)
    }
    # The deterministic box fixture is bilaterally symmetric, so opposite
    # angles may be byte-identical. The profile must still carry genuine
    # angular variation rather than eight renamed copies.
    assert len(set(turntable["render_view_sha256"].values())) >= 4
    assert (
        turntable["render_view_sha256"]["turntable_000"]
        != turntable["render_view_sha256"]["turntable_045"]
    )


def test_rust_shape_program_seal_controls_cross_language_float_identity() -> None:
    program = json.loads(json.dumps(VALID_PROGRAM))
    program["operations"][0]["args"]["position"] = [1e-7, -0.0, 0.0]
    program["operations"][0]["args"]["rotation"] = [
        0.9272952180016123,
        -0.6435011087932844,
        1.0,
    ]
    python_canonical = json.dumps(
        program,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    )
    rust_style_canonical = python_canonical.replace("1e-07", "1e-7")
    assert rust_style_canonical != python_canonical
    sealed_sha256 = hashlib.sha256(rust_style_canonical.encode("utf-8")).hexdigest()

    request = _execution_payload(
        execution_id="exec_rust_shape_seal",
        idempotency_key="idem_rust_shape_seal",
        cancellation_id="cancel_rust_shape_seal",
        cancellation_token="cancel_token_rust_shape_seal",
    )
    request["shape_program"] = program
    request["shape_program_canonical_json"] = rust_style_canonical
    request["shape_program_sha256"] = sealed_sha256
    response = _AsgiClient(
        main_module.create_restricted_geometry_app(
            environment={RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV: CAPABILITY}
        )
    ).post(
        f"{RESTRICTED_GEOMETRY_INTERNAL_PREFIX}/execute",
        headers=_headers(),
        json_payload=request,
    )
    assert response.status_code == 200, response.json()
    compiled = response.json()
    assert compiled["shape_program_sha256"] == sealed_sha256
    assert compiled["readback"]["shape_program_sha256"] == sealed_sha256

    tampered_hash = dict(request)
    tampered_hash["shape_program_sha256"] = "f" * 64
    with pytest.raises(ValueError):
        RestrictedGeometryExecutionRequest.model_validate(tampered_hash)

    tampered_value = json.loads(json.dumps(request))
    tampered_value["shape_program"]["operations"][0]["args"]["position"][0] = 2e-7
    with pytest.raises(ValueError):
        RestrictedGeometryExecutionRequest.model_validate(tampered_value)


def test_environment_and_cancellation_boundaries_are_fail_closed() -> None:
    with pytest.raises(RestrictedGeometryBoundaryError) as database_error:
        RestrictedGeometryExecutor(environment={"WUSHEN_LIBRARY_ROOT": "/private/db"})
    assert database_error.value.code == "GEOMETRY_EXECUTOR_ENVIRONMENT_FORBIDDEN"
    with pytest.raises(RestrictedGeometryBoundaryError) as provider_error:
        RestrictedGeometryExecutor(environment={"FORGECAD_AGENT_API_KEY": "secret"})
    assert provider_error.value.code == "GEOMETRY_EXECUTOR_ENVIRONMENT_FORBIDDEN"

    executor = RestrictedGeometryExecutor(environment={})
    executor.cancel(
        RestrictedGeometryCancellationRequest(
            cancellation_id="cancel_before_start",
            cancellation_token="cancel_token_before_start",
        )
    )
    request = RestrictedGeometryExecutionRequest.model_validate(
        _execution_payload(
            execution_id="exec_cancelled",
            idempotency_key="idem_cancelled",
            cancellation_id="cancel_before_start",
            cancellation_token="cancel_token_before_start",
        )
    )
    with pytest.raises(RestrictedGeometryBoundaryError) as cancelled:
        executor.execute(request)
    assert cancelled.value.code == "GEOMETRY_EXECUTION_CANCELLED"
    with pytest.raises(RestrictedGeometryBoundaryError) as replay:
        executor.execute(request)
    assert replay.value.code == "GEOMETRY_EXECUTION_CANCELLED"

    conflicting_request = RestrictedGeometryExecutionRequest.model_validate(
        _execution_payload(
            execution_id="exec_cancel_conflict",
            idempotency_key="idem_cancel_conflict",
            cancellation_id="cancel_before_start",
            cancellation_token="different_cancel_token",
        )
    )
    with pytest.raises(RestrictedGeometryBoundaryError) as conflict:
        executor.execute(conflicting_request)
    assert conflict.value.code == "GEOMETRY_CANCELLATION_ID_CONFLICT"


def test_disposable_worker_timeout_crash_and_late_result_tombstone() -> None:
    timeout_executor = RestrictedGeometryExecutor(
        environment={},
        worker_target=_hang_worker,
    )
    timeout_request = RestrictedGeometryExecutionRequest.model_validate(
        _execution_payload(
            execution_id="exec_timeout",
            idempotency_key="idem_timeout",
            cancellation_id="cancel_timeout",
            cancellation_token="cancel_token_timeout",
            timeout_ms=50,
        )
    )
    with pytest.raises(RestrictedGeometryBoundaryError) as timed_out:
        timeout_executor.execute(timeout_request)
    assert timed_out.value.code == "GEOMETRY_EXECUTION_TIMEOUT"
    with pytest.raises(RestrictedGeometryBoundaryError) as wrong_cancel:
        timeout_executor.cancel(
            RestrictedGeometryCancellationRequest(
                cancellation_id="cancel_timeout",
                cancellation_token="wrong_cancel_token",
            )
        )
    assert wrong_cancel.value.code == "GEOMETRY_CANCELLATION_TOKEN_MISMATCH"
    assert timeout_executor.cancel(
        RestrictedGeometryCancellationRequest(
            cancellation_id="cancel_timeout",
            cancellation_token="cancel_token_timeout",
        )
    ).accepted

    crash_executor = RestrictedGeometryExecutor(
        environment={},
        worker_target=_crash_worker,
    )
    crash_request = RestrictedGeometryExecutionRequest.model_validate(
        _execution_payload(
            execution_id="exec_crash",
            idempotency_key="idem_crash",
            cancellation_id="cancel_crash",
            cancellation_token="cancel_token_crash",
        )
    )
    with pytest.raises(RestrictedGeometryBoundaryError) as crashed:
        crash_executor.execute(crash_request)
    assert crashed.value.code == "GEOMETRY_EXECUTOR_CRASHED"

    late_executor = RestrictedGeometryExecutor(
        environment={},
        worker_target=_late_worker,
    )
    late_request = RestrictedGeometryExecutionRequest.model_validate(
        _execution_payload(
            execution_id="exec_late",
            idempotency_key="idem_late",
            cancellation_id="cancel_late",
            cancellation_token="cancel_token_late",
        )
    )
    observed: list[RestrictedGeometryBoundaryError] = []

    def execute_late() -> None:
        try:
            late_executor.execute(late_request)
        except RestrictedGeometryBoundaryError as exc:
            observed.append(exc)

    thread = threading.Thread(target=execute_late)
    thread.start()
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        with late_executor._lock:  # test-only observation of process registration
            record = late_executor._records.get("exec_late")
            if record is not None and record.process is not None:
                break
        time.sleep(0.005)
    late_executor.cancel(
        RestrictedGeometryCancellationRequest(
            cancellation_id="cancel_late",
            cancellation_token="cancel_token_late",
        )
    )
    thread.join(timeout=5)
    assert not thread.is_alive()
    assert [error.code for error in observed] == ["GEOMETRY_EXECUTION_CANCELLED"]
    with late_executor._lock:
        assert late_executor._records["exec_late"].result is None


def test_worker_child_environment_retains_only_audited_bundle_root() -> None:
    context = multiprocessing.get_context("spawn")
    parent, child = context.Pipe(duplex=False)
    process = context.Process(
        target=_environment_probe,
        args=(child, "/audited/forgecad-bundle"),
    )
    process.start()
    child.close()
    assert parent.poll(5)
    environment = parent.recv()
    process.join(timeout=5)
    parent.close()
    assert process.exitcode == 0
    assert environment == {"FORGECAD_RUNTIME_RESOURCE_ROOT": "/audited/forgecad-bundle"}


def test_sidecar_entry_overrides_resource_injection_and_strips_product_authority(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import uvicorn

    from wushen_agent import sidecar_entry

    monkeypatch.setattr(
        sys,
        "argv",
        ["wushen-agent", "agent", "serve", "--host", "127.0.0.1", "--port", "8123"],
    )
    monkeypatch.setattr(
        sidecar_entry,
        "_resource_root",
        lambda: Path("/audited/forgecad-bundle"),
    )
    monkeypatch.setenv("FORGECAD_RUNTIME_RESOURCE_ROOT", "/attacker/resource-root")
    monkeypatch.setenv("WUSHEN_LIBRARY_ROOT", "/attacker/library")
    monkeypatch.setenv("FORGECAD_AGENT_API_KEY", "must-not-survive")
    monkeypatch.setenv("FORGECAD_ACTIVE_DESIGN_SNAPSHOT_WRITE_TOKEN", "must-not-survive")
    monkeypatch.setenv("FORGECAD_K001_PACKAGED_PROBE", "1")
    monkeypatch.setenv("FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE", "1")
    monkeypatch.setenv("FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE", "1")
    observed: dict[str, Any] = {}

    def fake_run(app: FastAPI, *, host: str, port: int, log_level: str) -> None:
        observed.update(
            {
                "app": app,
                "host": host,
                "port": port,
                "log_level": log_level,
                "environment": dict(os.environ),
            }
        )

    monkeypatch.setattr(uvicorn, "run", fake_run)
    assert sidecar_entry.main() == 0
    environment = observed["environment"]
    assert observed["host"] == "127.0.0.1"
    assert observed["port"] == 8123
    assert environment["FORGECAD_RUNTIME_RESOURCE_ROOT"] == ("/audited/forgecad-bundle")
    assert "WUSHEN_LIBRARY_ROOT" not in environment
    assert "FORGECAD_AGENT_API_KEY" not in environment
    assert "FORGECAD_ACTIVE_DESIGN_SNAPSHOT_WRITE_TOKEN" not in environment
    assert "FORGECAD_K001_PACKAGED_PROBE" not in environment
    assert "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE" not in environment
    assert "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE" not in environment
    assert observed["app"].state.product_state_owner == "rust_forgecad_core"
    assert observed["app"].state.persistent_state_writer is False
