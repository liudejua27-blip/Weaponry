from __future__ import annotations

import os
import struct
import base64
import json
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from typing import Any, Dict, Literal, Optional, Protocol

from ..models import ProviderSettings, utc_now


class ThreeDProviderError(Exception):
    def __init__(self, code: str, message: str, recoverable: bool = True) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class ThreeDProvider(Protocol):
    provider_id: str

    def submit_rough_model(
        self,
        *,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> "RoughModelSubmission":
        ...

    def poll_rough_model(self, *, provider_task_id: str) -> "RoughModelPollResult":
        ...

    def fetch_rough_model(
        self,
        *,
        provider_task_id: str,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> "RoughModelResult":
        ...

    def cancel_rough_model(self, *, provider_task_id: str) -> "RoughModelCancelResult":
        ...

    def generate_rough_model(
        self,
        *,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> "RoughModelResult":
        ...


@dataclass(frozen=True)
class RoughModelResult:
    raw_glb_bytes: bytes
    normalized_glb_bytes: bytes
    optimized_glb_bytes: bytes
    unity_material_json: Dict[str, Any]
    provider_task_id: Optional[str]
    metadata: Dict[str, Any]
    metrics: Dict[str, Any]


@dataclass(frozen=True)
class RoughModelSubmission:
    provider_task_id: str
    status: Literal["submitted", "polling"] = "submitted"
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class RoughModelPollResult:
    provider_task_id: str
    status: Literal["submitted", "polling", "succeeded", "failed", "cancelled", "unknown"]
    progress: float
    metadata: Dict[str, Any] = field(default_factory=dict)
    error_code: Optional[str] = None
    error_message: Optional[str] = None


@dataclass(frozen=True)
class RoughModelCancelResult:
    provider_task_id: str
    status: Literal["cancelled", "cancel_requested", "unsupported", "unknown"]
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class LocalHTTPThreeDConfig:
    base_url: str
    provider_id: str = "local_http_3d"
    timeout_seconds: float = 30
    poll_interval_seconds: float = 1
    max_wait_seconds: float = 300
    retry_attempts: int = 2
    retry_backoff_seconds: float = 0.5
    api_key: Optional[str] = None


class MockThreeDProvider:
    provider_id = "mock_3d"

    def __init__(self) -> None:
        raw_sequence = os.environ.get("WUSHEN_MOCK_3D_POLL_SEQUENCE", "succeeded")
        self.poll_sequence = [item.strip() for item in raw_sequence.split(",") if item.strip()] or ["succeeded"]
        self.poll_counts: Dict[str, int] = {}

    def submit_rough_model(
        self,
        *,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> RoughModelSubmission:
        if target_format != "glb":
            raise ThreeDProviderError("INVALID_REQUEST", "Mock 3D provider only supports GLB output.", recoverable=False)
        provider_task_id = f"mock_3d_{model_id}"
        self.poll_counts.setdefault(provider_task_id, 0)
        return RoughModelSubmission(
            provider_task_id=provider_task_id,
            status="submitted",
            metadata={
                "provider": self.provider_id,
                "mock": True,
                "model_id": model_id,
                "style": style,
                "target_format": target_format,
                "source_image_asset_id": source_image_asset_id,
                "source_image": source_image_logical_path,
                "source_image_bytes": len(source_image_bytes),
                "source_image_mime_type": source_image_mime_type,
                "orientation_policy": orientation_policy,
                "scale_policy": scale_policy,
                "non_manufacturing_asset": True,
            },
        )

    def poll_rough_model(self, *, provider_task_id: str) -> RoughModelPollResult:
        count = self.poll_counts.get(provider_task_id, 0)
        index = min(count, len(self.poll_sequence) - 1)
        status = self.poll_sequence[index]
        self.poll_counts[provider_task_id] = count + 1
        if status not in {"submitted", "polling", "succeeded", "failed", "cancelled", "unknown"}:
            status = "unknown"
        progress = 1.0 if status == "succeeded" else 0.55 if status in {"submitted", "polling"} else 0.0
        return RoughModelPollResult(
            provider_task_id=provider_task_id,
            status=status,  # type: ignore[arg-type]
            progress=progress,
            metadata={"provider": self.provider_id, "mock": True, "poll_count": count + 1},
            error_code="PROVIDER_BAD_OUTPUT" if status == "failed" else None,
            error_message="Mock 3D provider failed during polling." if status == "failed" else None,
        )

    def fetch_rough_model(
        self,
        *,
        provider_task_id: str,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> RoughModelResult:
        if target_format != "glb":
            raise ThreeDProviderError("INVALID_REQUEST", "Mock 3D provider only supports GLB output.", recoverable=False)
        raw = mock_glb_payload(weapon_id, model_id=model_id, stage="raw")
        normalized = mock_glb_payload(weapon_id, model_id=model_id, stage="normalized")
        optimized = mock_glb_payload(weapon_id, model_id=model_id, stage="optimized")
        metrics = {
            "triangle_count": 36,
            "mesh_count": 1,
            "material_count": 1,
            "bounds": {
                "min": [-0.44, -0.91, -0.08],
                "max": [0.44, 1.875, 0.08],
            },
            "source_image_bytes": len(source_image_bytes),
            "source_image_mime_type": source_image_mime_type,
        }
        return RoughModelResult(
            raw_glb_bytes=raw,
            normalized_glb_bytes=normalized,
            optimized_glb_bytes=optimized,
            unity_material_json=mock_unity_material(weapon_id),
            provider_task_id=provider_task_id,
            metadata={
                "provider": self.provider_id,
                "provider_task_id": provider_task_id,
                "mock": True,
                "style": style,
                "target_format": target_format,
                "source_image_asset_id": source_image_asset_id,
                "source_image": source_image_logical_path,
                "orientation_policy": orientation_policy,
                "scale_policy": scale_policy,
                "runtime": "provider_fetch",
                "non_manufacturing_asset": True,
            },
            metrics=metrics,
        )

    def cancel_rough_model(self, *, provider_task_id: str) -> RoughModelCancelResult:
        self.poll_counts.pop(provider_task_id, None)
        return RoughModelCancelResult(
            provider_task_id=provider_task_id,
            status="cancelled",
            metadata={"provider": self.provider_id, "mock": True},
        )

    def generate_rough_model(
        self,
        *,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> RoughModelResult:
        submission = self.submit_rough_model(
            weapon_id=weapon_id,
            model_id=model_id,
            source_image_asset_id=source_image_asset_id,
            source_image_bytes=source_image_bytes,
            source_image_mime_type=source_image_mime_type,
            source_image_logical_path=source_image_logical_path,
            target_format=target_format,
            style=style,
            orientation_policy=orientation_policy,
            scale_policy=scale_policy,
        )
        poll = self.poll_rough_model(provider_task_id=submission.provider_task_id)
        if poll.status == "failed":
            raise ThreeDProviderError(poll.error_code or "PROVIDER_BAD_OUTPUT", poll.error_message or "3D provider failed.")
        if poll.status == "cancelled":
            raise ThreeDProviderError("PROVIDER_CANCELLED", "3D provider task was cancelled.")
        return self.fetch_rough_model(
            provider_task_id=submission.provider_task_id,
            weapon_id=weapon_id,
            model_id=model_id,
            source_image_asset_id=source_image_asset_id,
            source_image_bytes=source_image_bytes,
            source_image_mime_type=source_image_mime_type,
            source_image_logical_path=source_image_logical_path,
            target_format=target_format,
            style=style,
            orientation_policy=orientation_policy,
            scale_policy=scale_policy,
        )


class LocalHTTPThreeDProvider:
    """Adapter for a local SF3D/TripoSR/Hunyuan-style image-to-3D runtime service."""

    def __init__(self, config: LocalHTTPThreeDConfig) -> None:
        self.config = config
        self.provider_id = config.provider_id
        self.base_url = config.base_url.rstrip("/")

    def submit_rough_model(
        self,
        *,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> RoughModelSubmission:
        if target_format != "glb":
            raise ThreeDProviderError("INVALID_REQUEST", "Local HTTP 3D provider only supports GLB output.", recoverable=False)
        payload = {
            "schema_version": "WushenThreeDProviderRequest@1",
            "weapon_id": weapon_id,
            "model_id": model_id,
            "source_image_asset_id": source_image_asset_id,
            "source_image": {
                "logical_path": source_image_logical_path,
                "mime_type": source_image_mime_type,
                "data_base64": base64.b64encode(source_image_bytes).decode("ascii"),
            },
            "target_format": target_format,
            "style": style,
            "orientation_policy": orientation_policy,
            "scale_policy": scale_policy,
            "output_contract": {
                "asset_type": "fictional_game_art",
                "non_manufacturing_asset": True,
                "preferred_format": "glb",
            },
        }
        data = self._json_request("POST", "/v1/rough-models", payload)
        provider_task_id = _string_field(data, "provider_task_id") or _string_field(data, "task_id") or _string_field(data, "id")
        if not provider_task_id:
            raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", "3D provider submit response did not include provider_task_id.")
        status = _normalize_submission_status(str(data.get("status") or "submitted"))
        return RoughModelSubmission(
            provider_task_id=provider_task_id,
            status=status,
            metadata={
                "provider": self.provider_id,
                "runtime": "local_http",
                "base_url": self.base_url,
                "model_id": model_id,
                "source_image_asset_id": source_image_asset_id,
                **_dict_field(data, "metadata"),
            },
        )

    def poll_rough_model(self, *, provider_task_id: str) -> RoughModelPollResult:
        data = self._json_request("GET", f"/v1/rough-models/{_quote_path(provider_task_id)}", None)
        status = _normalize_poll_status(str(data.get("status") or data.get("state") or "unknown"))
        error = _dict_field(data, "error")
        return RoughModelPollResult(
            provider_task_id=provider_task_id,
            status=status,
            progress=_progress_value(data.get("progress")),
            metadata={
                "provider": self.provider_id,
                "runtime": "local_http",
                **_dict_field(data, "metadata"),
            },
            error_code=str(error.get("code") or data.get("error_code") or "PROVIDER_BAD_OUTPUT") if status == "failed" else None,
            error_message=str(error.get("message") or data.get("error_message") or "3D provider task failed.") if status == "failed" else None,
        )

    def fetch_rough_model(
        self,
        *,
        provider_task_id: str,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> RoughModelResult:
        if target_format != "glb":
            raise ThreeDProviderError("INVALID_REQUEST", "Local HTTP 3D provider only supports GLB output.", recoverable=False)
        data = self._json_request("GET", f"/v1/rough-models/{_quote_path(provider_task_id)}/result", None)
        raw = _decode_required_base64(data, "raw_glb_base64")
        normalized = _decode_optional_base64(data, "normalized_glb_base64") or raw
        optimized = _decode_optional_base64(data, "optimized_glb_base64") or normalized
        _ensure_glb(raw, "raw_glb_base64")
        _ensure_glb(normalized, "normalized_glb_base64")
        _ensure_glb(optimized, "optimized_glb_base64")
        unity_material = _dict_field(data, "unity_material_json") or mock_unity_material(weapon_id)
        metrics = _dict_field(data, "metrics")
        if not metrics:
            metrics = {
                "source_image_bytes": len(source_image_bytes),
                "source_image_mime_type": source_image_mime_type,
                "provider_reported_metrics": False,
            }
        return RoughModelResult(
            raw_glb_bytes=raw,
            normalized_glb_bytes=normalized,
            optimized_glb_bytes=optimized,
            unity_material_json=unity_material,
            provider_task_id=provider_task_id,
            metadata={
                "provider": self.provider_id,
                "provider_task_id": provider_task_id,
                "runtime": "local_http",
                "style": style,
                "target_format": target_format,
                "source_image_asset_id": source_image_asset_id,
                "source_image": source_image_logical_path,
                "orientation_policy": orientation_policy,
                "scale_policy": scale_policy,
                "non_manufacturing_asset": True,
                **_dict_field(data, "metadata"),
            },
            metrics=metrics,
        )

    def cancel_rough_model(self, *, provider_task_id: str) -> RoughModelCancelResult:
        try:
            data = self._json_request("POST", f"/v1/rough-models/{_quote_path(provider_task_id)}/cancel", {})
        except ThreeDProviderError as exc:
            if exc.code in {"PROVIDER_HTTP_404", "PROVIDER_HTTP_405", "PROVIDER_HTTP_501"}:
                return RoughModelCancelResult(
                    provider_task_id=provider_task_id,
                    status="unsupported",
                    metadata={"provider": self.provider_id, "runtime": "local_http", "provider_cancel_error": exc.code},
                )
            raise
        status = _normalize_cancel_status(str(data.get("status") or "unknown"))
        return RoughModelCancelResult(
            provider_task_id=provider_task_id,
            status=status,
            metadata={
                "provider": self.provider_id,
                "runtime": "local_http",
                **_dict_field(data, "metadata"),
            },
        )

    def generate_rough_model(
        self,
        *,
        weapon_id: str,
        model_id: str,
        source_image_asset_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
    ) -> RoughModelResult:
        submission = self.submit_rough_model(
            weapon_id=weapon_id,
            model_id=model_id,
            source_image_asset_id=source_image_asset_id,
            source_image_bytes=source_image_bytes,
            source_image_mime_type=source_image_mime_type,
            source_image_logical_path=source_image_logical_path,
            target_format=target_format,
            style=style,
            orientation_policy=orientation_policy,
            scale_policy=scale_policy,
        )
        deadline = time.time() + self.config.max_wait_seconds
        last_status = "submitted"
        while time.time() < deadline:
            poll = self.poll_rough_model(provider_task_id=submission.provider_task_id)
            last_status = poll.status
            if poll.status == "succeeded":
                return self.fetch_rough_model(
                    provider_task_id=submission.provider_task_id,
                    weapon_id=weapon_id,
                    model_id=model_id,
                    source_image_asset_id=source_image_asset_id,
                    source_image_bytes=source_image_bytes,
                    source_image_mime_type=source_image_mime_type,
                    source_image_logical_path=source_image_logical_path,
                    target_format=target_format,
                    style=style,
                    orientation_policy=orientation_policy,
                    scale_policy=scale_policy,
                )
            if poll.status == "failed":
                raise ThreeDProviderError(poll.error_code or "PROVIDER_BAD_OUTPUT", poll.error_message or "3D provider task failed.")
            if poll.status == "cancelled":
                raise ThreeDProviderError("PROVIDER_CANCELLED", "3D provider task was cancelled.")
            time.sleep(self.config.poll_interval_seconds)
        raise ThreeDProviderError("PROVIDER_TIMEOUT", f"3D provider did not finish before timeout. Last status: {last_status}.")

    def _json_request(self, method: str, path: str, payload: Optional[Dict[str, Any]]) -> Dict[str, Any]:
        url = self.base_url + path
        body = _canonical_json(payload).encode("utf-8") if payload is not None else None
        headers = {"Accept": "application/json"}
        if body is not None:
            headers["Content-Type"] = "application/json"
        if self.config.api_key:
            headers["Authorization"] = f"Bearer {self.config.api_key}"
        last_error: Optional[Exception] = None
        attempts = max(1, self.config.retry_attempts)
        for attempt in range(1, attempts + 1):
            request = urllib.request.Request(url, data=body, headers=headers, method=method)
            try:
                with urllib.request.urlopen(request, timeout=self.config.timeout_seconds) as response:
                    response_body = response.read()
                return _json_response(response_body, url)
            except urllib.error.HTTPError as exc:
                last_error = exc
                message = _safe_http_error_message(exc)
                code = f"PROVIDER_HTTP_{exc.code}"
                if exc.code < 500 or attempt == attempts:
                    raise ThreeDProviderError(code, message, recoverable=exc.code >= 500) from exc
            except (urllib.error.URLError, TimeoutError) as exc:
                last_error = exc
                if attempt == attempts:
                    raise ThreeDProviderError("PROVIDER_CONNECTION_FAILED", f"3D provider request failed: {exc}") from exc
            time.sleep(self.config.retry_backoff_seconds * attempt)
        raise ThreeDProviderError("PROVIDER_CONNECTION_FAILED", f"3D provider request failed: {last_error}")


def three_d_provider_from_env() -> ThreeDProvider:
    provider = os.environ.get("WUSHEN_3D_PROVIDER", "mock").strip().lower()
    if provider in {"mock", "mock_3d"}:
        return MockThreeDProvider()
    if provider in {"local_http", "http", "sf3d_http", "triposr_http", "hunyuan_http"}:
        base_url = os.environ.get("WUSHEN_3D_HTTP_BASE_URL") or os.environ.get("WUSHEN_3D_BASE_URL")
        if not base_url:
            raise ThreeDProviderError("PROVIDER_UNCONFIGURED", "WUSHEN_3D_HTTP_BASE_URL is required for local_http 3D provider.", recoverable=False)
        return LocalHTTPThreeDProvider(
            LocalHTTPThreeDConfig(
                base_url=base_url,
                provider_id=os.environ.get("WUSHEN_3D_HTTP_PROVIDER_ID", "local_http_3d"),
                timeout_seconds=float(os.environ.get("WUSHEN_3D_HTTP_TIMEOUT_SECONDS", "30")),
                poll_interval_seconds=float(os.environ.get("WUSHEN_3D_HTTP_POLL_INTERVAL_SECONDS", "1")),
                max_wait_seconds=float(os.environ.get("WUSHEN_3D_HTTP_MAX_WAIT_SECONDS", "300")),
                retry_attempts=int(os.environ.get("WUSHEN_3D_HTTP_RETRY_ATTEMPTS", "2")),
                retry_backoff_seconds=float(os.environ.get("WUSHEN_3D_HTTP_RETRY_BACKOFF_SECONDS", "0.5")),
                api_key=os.environ.get("WUSHEN_3D_HTTP_API_KEY"),
            )
        )
    else:
        raise ThreeDProviderError("PROVIDER_UNCONFIGURED", f"3D provider is not configured: {provider}", recoverable=False)


def three_d_provider_settings_from_env() -> list[ProviderSettings]:
    selected = os.environ.get("WUSHEN_3D_PROVIDER", "mock").strip().lower()
    local_base_url = os.environ.get("WUSHEN_3D_HTTP_BASE_URL") or os.environ.get("WUSHEN_3D_BASE_URL")
    return [
        ProviderSettings(
            provider_id="mock_3d",
            kind="three_d",
            type="mock",
            display_name="Mock 3D Provider",
            enabled=selected in {"mock", "mock_3d"},
            status="configured" if selected in {"mock", "mock_3d"} else "available",
            base_url="mock://3d",
            has_secret=False,
            updated_at=utc_now(),
        ),
        ProviderSettings(
            provider_id=os.environ.get("WUSHEN_3D_HTTP_PROVIDER_ID", "local_http_3d"),
            kind="three_d",
            type="local_http",
            display_name="Local HTTP 3D Provider",
            enabled=selected in {"local_http", "http", "sf3d_http", "triposr_http", "hunyuan_http"},
            status="configured" if local_base_url else "missing_config",
            base_url=local_base_url,
            has_secret=bool(os.environ.get("WUSHEN_3D_HTTP_API_KEY")),
            updated_at=utc_now(),
        ),
    ]


def _quote_path(value: str) -> str:
    return urllib.parse.quote(value, safe="")


def _string_field(data: Dict[str, Any], key: str) -> Optional[str]:
    value = data.get(key)
    return str(value) if value is not None and str(value) else None


def _dict_field(data: Dict[str, Any], key: str) -> Dict[str, Any]:
    value = data.get(key)
    return value if isinstance(value, dict) else {}


def _normalize_submission_status(value: str) -> Literal["submitted", "polling"]:
    normalized = value.strip().lower()
    return "polling" if normalized in {"polling", "running", "processing", "queued"} else "submitted"


def _normalize_poll_status(value: str) -> Literal["submitted", "polling", "succeeded", "failed", "cancelled", "unknown"]:
    normalized = value.strip().lower()
    if normalized in {"submitted", "queued", "pending"}:
        return "submitted"
    if normalized in {"polling", "running", "processing", "in_progress"}:
        return "polling"
    if normalized in {"succeeded", "success", "completed", "done", "finished"}:
        return "succeeded"
    if normalized in {"failed", "error"}:
        return "failed"
    if normalized in {"cancelled", "canceled"}:
        return "cancelled"
    return "unknown"


def _normalize_cancel_status(value: str) -> Literal["cancelled", "cancel_requested", "unsupported", "unknown"]:
    normalized = value.strip().lower()
    if normalized in {"cancelled", "canceled", "already_cancelled", "already_canceled"}:
        return "cancelled"
    if normalized in {"cancel_requested", "requested", "accepted", "pending"}:
        return "cancel_requested"
    if normalized in {"unsupported", "not_supported", "not_implemented"}:
        return "unsupported"
    return "unknown"


def _progress_value(value: Any) -> float:
    try:
        progress = float(value)
    except (TypeError, ValueError):
        return 0.0
    return max(0.0, min(progress, 1.0))


def _decode_required_base64(data: Dict[str, Any], key: str) -> bytes:
    encoded = data.get(key)
    if not isinstance(encoded, str) or not encoded:
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider result missing {key}.")
    try:
        return base64.b64decode(encoded, validate=True)
    except ValueError as exc:
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider result has invalid base64 field {key}.") from exc


def _decode_optional_base64(data: Dict[str, Any], key: str) -> Optional[bytes]:
    encoded = data.get(key)
    if encoded in {None, ""}:
        return None
    if not isinstance(encoded, str):
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider result field {key} must be base64 text.")
    try:
        return base64.b64decode(encoded, validate=True)
    except ValueError as exc:
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider result has invalid base64 field {key}.") from exc


def _ensure_glb(payload: bytes, field_name: str) -> None:
    if len(payload) < 12 or payload[:4] != b"glTF":
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider {field_name} is not a GLB payload.")
    version, declared_length = struct.unpack("<II", payload[4:12])
    if version != 2 or declared_length != len(payload):
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider {field_name} has an invalid GLB header.")


def _json_response(response_body: bytes, url: str) -> Dict[str, Any]:
    try:
        data = json.loads(response_body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider returned non-JSON response from {url}.") from exc
    if not isinstance(data, dict):
        raise ThreeDProviderError("PROVIDER_BAD_OUTPUT", f"3D provider JSON response from {url} must be an object.")
    return data


def _safe_http_error_message(exc: urllib.error.HTTPError) -> str:
    try:
        body = exc.read().decode("utf-8", errors="replace")
    except Exception:
        body = ""
    return f"3D provider HTTP {exc.code}: {body[:500] or exc.reason}"


def mock_glb_payload(weapon_id: str, *, model_id: str, stage: str) -> bytes:
    vertices: list[float] = []
    normals: list[float] = []
    indices: list[int] = []
    primitives = [
        _box_mesh(center=(0.0, 0.95, 0.0), size=(0.18, 1.85, 0.06)),
        _box_mesh(center=(0.0, -0.05, 0.0), size=(0.88, 0.12, 0.12)),
        _box_mesh(center=(0.0, -0.55, 0.0), size=(0.16, 0.72, 0.16)),
    ]
    for primitive in primitives:
        offset = len(vertices) // 3
        vertices.extend(primitive["positions"])
        normals.extend(primitive["normals"])
        indices.extend(offset + index for index in primitive["indices"])

    position_bytes = struct.pack(f"<{len(vertices)}f", *vertices)
    normal_bytes = struct.pack(f"<{len(normals)}f", *normals)
    index_bytes = struct.pack(f"<{len(indices)}H", *indices)
    padded_position = _pad_glb_chunk(position_bytes)
    padded_normal = _pad_glb_chunk(normal_bytes)
    binary = padded_position + padded_normal + _pad_glb_chunk(index_bytes)
    normal_offset = len(padded_position)
    index_offset = normal_offset + len(padded_normal)
    gltf = {
        "asset": {"version": "2.0", "generator": f"Wushen Forge {stage} mock_3d"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": f"wushen_{stage}_weapon_{weapon_id}_{model_id}"}],
        "meshes": [
            {
                "name": f"{stage}_game_asset_proxy",
                "primitives": [
                    {
                        "attributes": {"POSITION": 0, "NORMAL": 1},
                        "indices": 2,
                        "material": 0,
                        "mode": 4,
                    }
                ],
            }
        ],
        "materials": [
            {
                "name": "toon_gold_red_proxy",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.82, 0.28, 0.16, 1.0],
                    "metallicFactor": 0.35,
                    "roughnessFactor": 0.55,
                },
                "emissiveFactor": [0.18, 0.04, 0.02],
            }
        ],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": len(position_bytes), "target": 34962},
            {"buffer": 0, "byteOffset": normal_offset, "byteLength": len(normal_bytes), "target": 34962},
            {"buffer": 0, "byteOffset": index_offset, "byteLength": len(index_bytes), "target": 34963},
        ],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": len(vertices) // 3,
                "type": "VEC3",
                "min": [min(vertices[0::3]), min(vertices[1::3]), min(vertices[2::3])],
                "max": [max(vertices[0::3]), max(vertices[1::3]), max(vertices[2::3])],
            },
            {"bufferView": 1, "componentType": 5126, "count": len(normals) // 3, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5123, "count": len(indices), "type": "SCALAR"},
        ],
    }
    json_chunk = _pad_glb_chunk(_canonical_json(gltf).encode("utf-8"), pad_byte=b" ")
    length = 12 + 8 + len(json_chunk) + 8 + len(binary)
    return (
        b"glTF"
        + struct.pack("<II", 2, length)
        + struct.pack("<I4s", len(json_chunk), b"JSON")
        + json_chunk
        + struct.pack("<I4s", len(binary), b"BIN\x00")
        + binary
    )


def mock_unity_material(weapon_id: str) -> Dict[str, Any]:
    return {
        "schema_version": "UnityMaterial@1",
        "weapon_id": weapon_id,
        "shader_family": "toon_weapon",
        "material_slots": [
            {"name": "dark_metal", "base_color": "#161616", "outline": True, "shadow_steps": 2},
            {"name": "gold_trim", "base_color": "#C79A3A", "metallic": 0.6, "smoothness": 0.45},
            {"name": "energy_core", "base_color": "#D8422A", "emission": "#FF7A32"},
        ],
    }


def _box_mesh(*, center: tuple[float, float, float], size: tuple[float, float, float]) -> Dict[str, list[float] | list[int]]:
    cx, cy, cz = center
    sx, sy, sz = size[0] / 2, size[1] / 2, size[2] / 2
    corners = {
        "lbn": (cx - sx, cy - sy, cz - sz),
        "rbn": (cx + sx, cy - sy, cz - sz),
        "rtn": (cx + sx, cy + sy, cz - sz),
        "ltn": (cx - sx, cy + sy, cz - sz),
        "lbf": (cx - sx, cy - sy, cz + sz),
        "rbf": (cx + sx, cy - sy, cz + sz),
        "rtf": (cx + sx, cy + sy, cz + sz),
        "ltf": (cx - sx, cy + sy, cz + sz),
    }
    faces = [
        (("lbf", "rbf", "rtf", "ltf"), (0.0, 0.0, 1.0)),
        (("rbn", "lbn", "ltn", "rtn"), (0.0, 0.0, -1.0)),
        (("rbf", "rbn", "rtn", "rtf"), (1.0, 0.0, 0.0)),
        (("lbn", "lbf", "ltf", "ltn"), (-1.0, 0.0, 0.0)),
        (("ltf", "rtf", "rtn", "ltn"), (0.0, 1.0, 0.0)),
        (("lbn", "rbn", "rbf", "lbf"), (0.0, -1.0, 0.0)),
    ]
    positions: list[float] = []
    normals: list[float] = []
    indices: list[int] = []
    for face_index, (names, normal) in enumerate(faces):
        base = face_index * 4
        for name in names:
            positions.extend(corners[name])
            normals.extend(normal)
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3])
    return {"positions": positions, "normals": normals, "indices": indices}


def _pad_glb_chunk(payload: bytes, *, pad_byte: bytes = b"\x00") -> bytes:
    padding = (4 - (len(payload) % 4)) % 4
    return payload + pad_byte * padding


def _canonical_json(value: Any) -> str:
    import json

    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
