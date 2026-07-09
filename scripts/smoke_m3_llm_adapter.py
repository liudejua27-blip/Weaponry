#!/usr/bin/env python3
"""M3 smoke checks for LLM provider adapter boundaries."""

from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest  # noqa: E402
from wushen_agent.providers.llm import LLMProviderError, llm_provider_from_env, llm_provider_settings_from_env  # noqa: E402


class InvalidSpecProvider:
    provider_id = "invalid_spec_llm"

    def plan_weapon_spec(self, _request: CreateWeaponRequest, *, weapon_id: str) -> dict:
        return {
            "schema_version": "WeaponDesignSpec@1",
            "id": weapon_id,
            "name": "坏输出测试",
            "style": "3渲2国风神兵",
            "weapon_family": "sword",
            "safety_boundary": {"real_world_manufacturing_details": False},
        }


def main() -> int:
    clear_provider_env()
    os.environ["WUSHEN_LLM_PROVIDER"] = "mock"

    request = CreateWeaponRequest(
        client_request_id="m3-llm-smoke",
        text="青玉雷纹长枪，3渲2国风神兵，逼真外观，仅作为虚构 Unity 游戏资产",
    )
    provider = llm_provider_from_env()
    spec = provider.plan_weapon_spec(request, weapon_id="weapon_m3_smoke")
    assert spec["schema_version"] == "WeaponDesignSpec@1"
    assert spec["safety_boundary"]["real_world_manufacturing_details"] is False
    assert "manufacturing" in spec["generation"]["negative_prompt"]

    with tempfile.TemporaryDirectory(prefix="wushen_m3_llm_") as tmp:
        store = SQLiteAssetStore(
            library_root=Path(tmp) / "WushenForgeLibrary",
            migrations_dir=ROOT / "migrations",
            llm_provider=provider,
        )
        job = store.create_weapon(request, idempotency_key="m3-llm-smoke-key")
        assert job.status == "succeeded"
        assert any(event.step == "weapon_spec_planner" for event in job.events)

    with tempfile.TemporaryDirectory(prefix="wushen_m3_bad_spec_") as tmp:
        store = SQLiteAssetStore(
            library_root=Path(tmp) / "WushenForgeLibrary",
            migrations_dir=ROOT / "migrations",
            llm_provider=InvalidSpecProvider(),
        )
        try:
            store.create_weapon(request, idempotency_key="m3-invalid-spec-key")
        except LLMProviderError as exc:
            assert exc.code == "PROVIDER_BAD_OUTPUT"
            assert "invalid WeaponDesignSpec" in str(exc)
        else:
            raise AssertionError("invalid WeaponDesignSpec was committed")

    os.environ["WUSHEN_LLM_PROVIDER"] = "openai_compatible"
    os.environ["WUSHEN_LLM_MODEL"] = "test-model"
    missing_key_provider = llm_provider_from_env()
    try:
        missing_key_provider.plan_weapon_spec(request, weapon_id="weapon_missing_key")
    except LLMProviderError as exc:
        assert exc.code == "PROVIDER_UNCONFIGURED"
    else:
        raise AssertionError("openai_compatible provider without key did not fail")

    settings_json = json.dumps([item.model_dump() for item in llm_provider_settings_from_env()], ensure_ascii=False)
    forbidden = ["sk-", "bearer", "authorization", "api_key", "secret_value"]
    assert not any(marker in settings_json.lower() for marker in forbidden), settings_json

    print(json.dumps({"ok": True, "mock_provider": provider.provider_id, "settings_safe": True}, ensure_ascii=False, indent=2))
    return 0


def clear_provider_env() -> None:
    for key in [
        "OPENAI_API_KEY",
        "DEEPSEEK_API_KEY",
        "ANTHROPIC_API_KEY",
        "WUSHEN_LLM_API_KEY",
        "WUSHEN_LLM_API_KEY_FILE",
        "WUSHEN_OPENAI_API_KEY",
        "WUSHEN_LLM_PROVIDER",
        "WUSHEN_LLM_MODEL",
        "WUSHEN_LLM_BASE_URL",
    ]:
        os.environ.pop(key, None)


if __name__ == "__main__":
    sys.exit(main())
