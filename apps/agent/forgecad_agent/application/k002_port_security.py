"""Fail-closed helpers for the transitional K002 Python ports.

The Rust Agent runtime owns lifecycle and provider decisions during K002.  The
Python process is only a persistence/execution facet, so these helpers reject
provider context, hidden reasoning, authority paths, and unbounded JSON before
an injected Python writer or a code-owned Product Tool can observe the input.
"""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from typing import Any, Iterable


MAX_PORT_JSON_BYTES = 1_048_576
MAX_PORT_JSON_DEPTH = 32
MAX_PORT_JSON_NODES = 100_000
MAX_PORT_STRING_CHARS = 1_000_000


class K002PortBoundaryError(RuntimeError):
    """Stable, non-secret error returned before persistence or tool execution."""

    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


_ENV_CONTEXT_MARKERS = (
    "provider",
    "deepseek",
    "openai",
    "anthropic",
    "dashscope",
    "qwen",
    "gemini",
    "googleai",
    "azureai",
    "cohere",
    "mistral",
    "groq",
    "moonshot",
    "openrouter",
    "conceptplanner",
    "wushenllm",
    "comfyui",
)
_ENV_DETAIL_MARKERS = (
    "key",
    "token",
    "secret",
    "credential",
    "password",
    "authorization",
    "url",
    "endpoint",
    "model",
)
_SECRET_NAME_PARTS = {
    "key",
    "token",
    "secret",
    "credential",
    "credentials",
    "password",
    "authorization",
}
_ALLOWED_INTERNAL_SECRET_ENVIRONMENT = {
    # This random one-process capability authenticates Rust-to-Python internal
    # ports. It is not a Provider credential and must remain available after
    # the supervisor's env_clear boundary.
    "FORGECAD_K002_INTERNAL_CAPABILITY_TOKEN",
}
_EXACT_FORBIDDEN_ENVIRONMENT = {
    "FORGECAD_AGENT_PROVIDER",
    "FORGECAD_AGENT_BASE_URL",
    "FORGECAD_AGENT_MODEL",
    "FORGECAD_AGENT_API_KEY",
    "DEEPSEEK_API_KEY",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "ANTHROPIC_API_KEY",
    "DASHSCOPE_API_KEY",
    "FORGECAD_AGENT_API_KEY_FILE",
    "FORGECAD_CONCEPT_PLANNER_PROVIDER",
    "FORGECAD_CONCEPT_PLANNER_BASE_URL",
    "FORGECAD_CONCEPT_PLANNER_MODEL",
    "FORGECAD_CONCEPT_PLANNER_API_KEY",
    "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE",
    "WUSHEN_LLM_PROVIDER",
    "WUSHEN_LLM_BASE_URL",
    "WUSHEN_LLM_MODEL",
    "WUSHEN_LLM_API_KEY",
    "WUSHEN_LLM_API_KEY_FILE",
    "WUSHEN_OPENAI_BASE_URL",
    "WUSHEN_OPENAI_MODEL",
    "WUSHEN_OPENAI_API_KEY",
    "WUSHEN_OPENAI_API_KEY_FILE",
    "WUSHEN_3D_HTTP_API_KEY",
}
_SECRET_VALUE_PATTERNS = (
    re.compile(r"^Bearer\s+\S+", re.IGNORECASE),
    re.compile(r"^sk-[A-Za-z0-9_\-]{16,}$"),
)
_WINDOWS_ABSOLUTE_PATH = re.compile(r"^[A-Za-z]:[/\\]")


def canonical_json_sha256(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def ensure_isolated_environment(environment: Mapping[str, str]) -> None:
    """Reject Provider configuration before the Python facet handles a request."""

    for name, value in environment.items():
        upper_name = name.upper()
        if upper_name in _ALLOWED_INTERNAL_SECRET_ENVIRONMENT:
            continue
        normalized = _normalize_key(name)
        name_parts = {
            part.casefold()
            for part in re.split(r"[^A-Za-z0-9]+", name)
            if part
        }
        forbidden = (
            upper_name in _EXACT_FORBIDDEN_ENVIRONMENT
            or bool(name_parts & _SECRET_NAME_PARTS)
            or (
                any(marker in normalized for marker in _ENV_CONTEXT_MARKERS)
                and any(marker in normalized for marker in _ENV_DETAIL_MARKERS)
            )
            or any(pattern.match(value) for pattern in _SECRET_VALUE_PATTERNS)
        )
        if forbidden and value:
            raise K002PortBoundaryError(
                "K002_PROVIDER_ENVIRONMENT_FORBIDDEN",
                "The transitional Python port process must not receive Provider configuration.",
            )


def validate_bounded_json(
    value: Any,
    *,
    forbidden_keys: Iterable[str],
    boundary_name: str,
) -> None:
    """Validate JSON shape, budget, and forbidden nested fields without logging values."""

    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as exc:
        raise K002PortBoundaryError(
            "K002_PORT_JSON_INVALID",
            f"{boundary_name} must contain only finite JSON values.",
        ) from exc
    if len(encoded) > MAX_PORT_JSON_BYTES:
        raise K002PortBoundaryError(
            "K002_PORT_JSON_TOO_LARGE",
            f"{boundary_name} exceeds the bounded Python port payload size.",
        )

    denied = {_normalize_key(key) for key in forbidden_keys}
    stack: list[tuple[Any, int]] = [(value, 0)]
    visited = 0
    while stack:
        current, depth = stack.pop()
        visited += 1
        if visited > MAX_PORT_JSON_NODES:
            raise K002PortBoundaryError(
                "K002_PORT_JSON_TOO_COMPLEX",
                f"{boundary_name} exceeds the bounded JSON node count.",
            )
        if depth > MAX_PORT_JSON_DEPTH:
            raise K002PortBoundaryError(
                "K002_PORT_JSON_TOO_DEEP",
                f"{boundary_name} exceeds the bounded JSON nesting depth.",
            )
        if isinstance(current, Mapping):
            for key, child in current.items():
                if not isinstance(key, str):
                    raise K002PortBoundaryError(
                        "K002_PORT_JSON_INVALID",
                        f"{boundary_name} object keys must be strings.",
                    )
                if _normalize_key(key) in denied:
                    raise K002PortBoundaryError(
                        "K002_FORBIDDEN_CONTEXT_FIELD",
                        f"{boundary_name} contains a field forbidden at the Python port boundary.",
                    )
                stack.append((child, depth + 1))
        elif isinstance(current, (list, tuple)):
            stack.extend((child, depth + 1) for child in current)
        elif isinstance(current, str):
            if len(current) > MAX_PORT_STRING_CHARS:
                raise K002PortBoundaryError(
                    "K002_PORT_STRING_TOO_LARGE",
                    f"{boundary_name} contains an oversized string.",
                )
            if any(pattern.match(current) for pattern in _SECRET_VALUE_PATTERNS):
                raise K002PortBoundaryError(
                    "K002_PROVIDER_SECRET_FORBIDDEN",
                    f"{boundary_name} contains secret-like Provider material.",
                )
        elif current is None or isinstance(current, (bool, int, float)):
            continue
        else:
            raise K002PortBoundaryError(
                "K002_PORT_JSON_INVALID",
                f"{boundary_name} contains a non-JSON value.",
            )


def reject_machine_locations(value: Any, *, boundary_name: str) -> None:
    """Reject machine/file/network locations while allowing geometric path data."""

    stack = [value]
    while stack:
        current = stack.pop()
        if isinstance(current, Mapping):
            stack.extend(current.values())
        elif isinstance(current, (list, tuple)):
            stack.extend(current)
        elif isinstance(current, str) and (
            current.startswith("/")
            or current.startswith("~/")
            or current.startswith("file://")
            or current.startswith("http://")
            or current.startswith("https://")
            or _WINDOWS_ABSOLUTE_PATH.match(current)
        ):
            raise K002PortBoundaryError(
                "K002_MACHINE_LOCATION_FORBIDDEN",
                f"{boundary_name} cannot contain machine paths, file URLs or network URLs.",
            )


def _normalize_key(value: str) -> str:
    return "".join(character for character in value.casefold() if character.isalnum())
