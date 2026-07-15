#!/usr/bin/env python3
"""FGC-M103: verify visual texture objects, provenance and path boundaries."""

from __future__ import annotations

import base64
import json
import tempfile
from pathlib import Path

from jsonschema import Draft202012Validator, RefResolver, ValidationError
from pydantic import ValidationError as PydanticValidationError

from forgecad_agent.application.agent_models import (
    AgentMaterialPreset,
    AgentMaterialTextureObject,
    RegisterAgentMaterialTextureRequest,
)
from forgecad_agent.application.material_catalog import list_material_presets
from forgecad_agent.application.material_textures import (
    MaterialTextureError,
    MaterialTextureIdempotencyConflict,
    MaterialTextureService,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner
from forgecad_agent.infrastructure.storage.content_addressed_store import ContentAddressedStore, ObjectStoreError


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"

# A minimal 1x1 PNG. The service reads the IHDR rather than trusting caller-supplied dimensions.
PNG_1X1 = bytes.fromhex(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
    "0000000d49444154789c6360f8cfc000000301010018dd8db40000000049454e44ae426082"
)


def schema_validator() -> Draft202012Validator:
    schema = json.loads((SCHEMA_DIR / "material-texture-object.schema.json").read_text(encoding="utf-8"))
    common = json.loads((SCHEMA_DIR / "common.schema.json").read_text(encoding="utf-8"))
    resolver = RefResolver.from_schema(schema, store={common["$id"]: common})
    return Draft202012Validator(schema, resolver=resolver)


def main() -> int:
    validator = schema_validator()
    with tempfile.TemporaryDirectory(prefix="forgecad-m103-texture-") as raw:
        root = Path(raw)
        factory = SQLiteConnectionFactory(root / "library.db")
        runner = SQLiteMigrationRunner(factory, ROOT / "migrations")
        applied = runner.run()
        assert "0029" in applied, applied
        assert "0029" not in runner.run(), "M103 migration must be idempotent"
        service = MaterialTextureService(factory, ContentAddressedStore(root))
        request = RegisterAgentMaterialTextureRequest(
            display_name="M103 预览纹理",
            texture_role="base_color",
            mime_type="image/png",
            payload_base64=base64.b64encode(PNG_1X1).decode("ascii"),
            source="user_created",
            license="self_declared_original",
        )
        created = service.register(request, "m103-register")
        payload = created.model_dump(mode="json")
        validator.validate(payload)
        assert created.texture_asset_id.startswith("asset_tex_")
        assert created.width == 1 and created.height == 1
        assert created.object_exists is True
        assert created.object_path.startswith("objects/sha256/")
        assert "/" not in created.texture_asset_id[10:]

        replay = service.register(request, "m103-register")
        assert replay.texture_asset_id == created.texture_asset_id
        try:
            service.register(request.model_copy(update={"display_name": "不同请求"}), "m103-register")
        except MaterialTextureIdempotencyConflict:
            pass
        else:
            raise AssertionError("idempotency key accepted a different texture request")

        listed = service.list(texture_role="base_color")
        assert [item.texture_asset_id for item in listed.items] == [created.texture_asset_id]
        enriched = service.enrich_catalog(list_material_presets())
        assert len(enriched) == 13
        assert all(item.thumbnail_fallback == "parameter" for item in enriched)
        assert all(item.texture_summary == [] for item in enriched)

        for bad in (
            {**request.model_dump(mode="json"), "source": "forgecad_builtin", "license": "self_declared_original"},
            {**request.model_dump(mode="json"), "source": "imported_reference", "license": "third_party", "license_ref": None},
        ):
            try:
                RegisterAgentMaterialTextureRequest.model_validate(bad)
            except PydanticValidationError:
                pass
            else:
                raise AssertionError("invalid texture provenance was accepted")

        try:
            service.register(request.model_copy(update={"payload_base64": "not-a-path:/tmp/a.png"}), "m103-bad-b64")
        except MaterialTextureError as exc:
            assert exc.code == "TEXTURE_BASE64_INVALID"
        else:
            raise AssertionError("invalid base64 was accepted")

        service.object_store.resolve(created.object_path).unlink()
        missing = service.get(created.texture_asset_id)
        assert missing.object_exists is False
        texture_bound_preset = AgentMaterialPreset.model_validate({
            "schema_version": "MaterialPreset@1",
            "material_id": "mat_m103_missing_file",
            "display_name": "缺失文件回退",
            "category": "metal",
            "pbr": {
                "base_color": "#112233",
                "metallic": 0.4,
                "roughness": 0.5,
                "opacity": 1,
                "base_color_texture_asset_id": created.texture_asset_id,
            },
            "visual_only": True,
            "allowed_domains": ["vehicle_concept"],
            "provenance": "user_created",
            "license": "self_declared_original",
        })
        assert service.enrich_catalog([texture_bound_preset])[0].texture_summary[0].exists is False

        try:
            AgentMaterialTextureObject.model_validate({**payload, "object_path": "/tmp/escape.png"})
        except PydanticValidationError:
            pass
        else:
            raise AssertionError("absolute object path was accepted")
        try:
            validator.validate({**payload, "object_path": "/tmp/escape.png"})
        except ValidationError:
            pass
        else:
            raise AssertionError("JSON Schema accepted an absolute object path")

        try:
            service.object_store.resolve("/tmp/escape.png")
        except ObjectStoreError as exc:
            assert exc.code == "OBJECT_PATH_DENIED"
        else:
            raise AssertionError("object store accepted an absolute path")

        bad_preset = AgentMaterialPreset.model_validate({
            "schema_version": "MaterialPreset@1",
            "material_id": "mat_m103_missing",
            "display_name": "缺失纹理回退",
            "category": "metal",
            "pbr": {
                "base_color": "#112233",
                "metallic": 0.4,
                "roughness": 0.5,
                "opacity": 1,
                "base_color_texture_asset_id": "asset_tex_000000000000000000000000",
            },
            "visual_only": True,
            "allowed_domains": ["vehicle_concept"],
            "provenance": "forgecad_builtin",
        })
        assert service.enrich_catalog([bad_preset])[0].texture_summary[0].exists is False

    print("FGC-M103 material texture object/provenance/path smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
