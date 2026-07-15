from __future__ import annotations

import uuid
from typing import Literal, Optional

from pydantic import Field

from .concept_models import StrictApiModel


ProviderConnectionStatus = Literal["offline", "unconfigured", "ready", "degraded", "failed"]
ProviderTracePhase = Literal[
    "preflight",
    "request_started",
    "streaming",
    "validating",
    "completed",
    "failed",
    "cancelled",
]


class ProviderConnectionState(StrictApiModel):
    """Redaction-safe facts about the Provider selected by this Agent process."""

    schema_version: Literal["ProviderConnectionState@1"] = "ProviderConnectionState@1"
    status: ProviderConnectionStatus
    provider_id: str = Field(min_length=1, max_length=120)
    configured: bool
    metadata_status: Literal["not_checked", "missing", "valid", "invalid", "unavailable"]
    secret_status: Literal["not_checked", "missing", "available", "invalid", "unavailable"]
    supervisor_status: Literal["not_checked", "running", "restart_failed", "unavailable"]
    capability_status: Literal["offline", "ready", "mismatch", "unavailable"]
    network_call_made: bool = False
    failure_code: Optional[str] = Field(default=None, max_length=120)
    message: str = Field(min_length=1, max_length=500)


class ProviderExecutionTrace(StrictApiModel):
    """One sanitized Provider lifecycle observation.

    Prompt text, response text, reasoning content, authorization headers, base
    URLs, API keys and raw model identifiers are intentionally not fields of
    this contract.
    """

    schema_version: Literal["ProviderExecutionTrace@1"] = "ProviderExecutionTrace@1"
    trace_id: str = Field(pattern=r"^ptrace_[a-f0-9]{32}$")
    phase: ProviderTracePhase
    provider_id: str = Field(min_length=1, max_length=120)
    attempt: int = Field(default=1, ge=1, le=2)
    network_call_made: bool
    latency_ms: int = Field(default=0, ge=0)
    input_tokens: Optional[int] = Field(default=None, ge=0)
    output_tokens: Optional[int] = Field(default=None, ge=0)
    total_tokens: Optional[int] = Field(default=None, ge=0)
    prompt_cache_hit_tokens: Optional[int] = Field(default=None, ge=0)
    prompt_cache_miss_tokens: Optional[int] = Field(default=None, ge=0)
    error_code: Optional[str] = Field(default=None, max_length=120)
    message: str = Field(min_length=1, max_length=500)

    @classmethod
    def new(
        cls,
        *,
        phase: ProviderTracePhase,
        provider_id: str,
        network_call_made: bool,
        message: str,
        trace_id: Optional[str] = None,
        **values: object,
    ) -> "ProviderExecutionTrace":
        return cls(
            trace_id=trace_id or f"ptrace_{uuid.uuid4().hex}",
            phase=phase,
            provider_id=provider_id,
            network_call_made=network_call_made,
            message=message,
            **values,
        )
