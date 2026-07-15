"""Bounded, deterministic assembly of a safe Provider conversation.

This module deliberately compiles context only.  It never decides the active
asset, writes a version, or sends a network request.  Those remain owned by
ActiveDesignSnapshot and AgentKernel respectively.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Any, Iterable, Mapping, Optional


PROMPT_CONTRACT_VERSION = "ForgeCADProviderConversation@1"
_MAX_RECENT_MESSAGES = 8


@dataclass(frozen=True)
class ProviderConversation:
    """Dynamic messages appended after the Planner's stable cache prefix."""

    messages: tuple[dict[str, str], ...]
    context_hash: str
    prompt_contract_version: str = PROMPT_CONTRACT_VERSION
    routing_mode: str = "concept_planning"
    thinking_enabled: bool = True
    max_output_tokens: int = 1800


def compile_provider_conversation(
    *,
    prior_items: Iterable[Mapping[str, Any]],
    current_request: str,
    memory_summary: Optional[Mapping[str, Any]],
    snapshot: Optional[Mapping[str, Any]],
) -> ProviderConversation:
    """Return a cache-friendly history followed by one current request.

    The caller supplies only already persisted items.  Therefore every later
    turn contains the exact previous request/answer sequence as a prefix.  A
    summary is an append-only, disposable anchor and never replaces the
    authoritative Project or Snapshot records.
    """

    messages: list[dict[str, str]] = []
    summary_sequence = 0
    if memory_summary:
        summary_sequence = int(memory_summary.get("up_to_sequence", 0) or 0)
        summary_text = str(memory_summary.get("summary_text", "")).strip()
        if summary_text:
            messages.append(
                {
                    "role": "user",
                    "content": _canonical_json(
                        {
                            "schema_version": "ThreadMemorySummary@1",
                            "summary": summary_text,
                            "up_to_sequence": summary_sequence,
                        }
                    ),
                }
            )

    history: list[dict[str, str]] = []
    for item in prior_items:
        if int(item.get("sequence", 0) or 0) <= summary_sequence:
            continue
        item_type = item.get("item_type")
        payload = item.get("payload")
        if not isinstance(payload, Mapping):
            continue
        text = str(payload.get("text", "")).strip()
        if not text:
            continue
        if item_type == "user_message":
            history.append({"role": "user", "content": text})
        elif item_type == "assistant_message":
            history.append({"role": "assistant", "content": text})
    messages.extend(history[-_MAX_RECENT_MESSAGES:])

    messages.append(
        {
            "role": "user",
            "content": _canonical_json(
                {
                    "schema_version": "ForgeCADTurnRequest@1",
                    "active_design_snapshot": _snapshot_digest(snapshot),
                    "request": current_request,
                }
            ),
        }
    )
    return ProviderConversation(
        messages=tuple(messages),
        context_hash=_hash_json(
            {
                "prompt_contract_version": PROMPT_CONTRACT_VERSION,
                "messages": messages,
            }
        ),
    )


def make_deterministic_memory_summary(items: Iterable[Mapping[str, Any]]) -> Optional[dict[str, Any]]:
    """Compact old completed user/assistant text without a second model call."""

    relevant = [
        item
        for item in items
        if item.get("item_type") in {"user_message", "assistant_message"}
        and isinstance(item.get("payload"), Mapping)
        and str(item["payload"].get("text", "")).strip()
    ]
    if len(relevant) <= 24:
        return None
    compacted = relevant[:-_MAX_RECENT_MESSAGES]
    chunks: list[str] = []
    for item in compacted:
        role = "用户" if item["item_type"] == "user_message" else "助手"
        text = str(item["payload"]["text"]).strip().replace("\n", " ")
        chunks.append(f"{role}：{text[:240]}")
    text = "\n".join(chunks)
    # A conservative character cap keeps this well below the 1,000-token
    # product limit for the supported Chinese-first UI.
    return {
        "summary_text": text[:3200],
        "up_to_sequence": int(compacted[-1]["sequence"]),
    }


def _snapshot_digest(snapshot: Optional[Mapping[str, Any]]) -> dict[str, Any]:
    if not snapshot:
        return {"status": "unavailable"}
    active = snapshot.get("active_design")
    if not isinstance(active, Mapping):
        return {"status": "unavailable"}
    digest = {
        "status": "available",
        "project_id": snapshot.get("project_id"),
        "revision": snapshot.get("revision"),
        "source": "agent_asset" if "asset_version_id" in active else "legacy_concept_read_only",
        "asset_version_id": active.get("asset_version_id"),
        "selected_part_id": snapshot.get("selected_part_id"),
        "selected_material_zone_id": snapshot.get("selected_material_zone_id"),
        "preview_change_set_id": (snapshot.get("preview") or {}).get("change_set_id")
        if isinstance(snapshot.get("preview"), Mapping)
        else None,
    }
    return {key: value for key, value in digest.items() if value is not None}


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()
