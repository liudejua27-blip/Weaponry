from __future__ import annotations

import json
import os
from functools import lru_cache
from pathlib import Path
from typing import Any, Dict
from urllib.parse import urljoin

from jsonschema import Draft202012Validator, RefResolver


class WeaponSpecValidationError(Exception):
    """Raised when a provider returns a WeaponDesignSpec that cannot be committed."""


def validate_weapon_design_spec(spec: Dict[str, Any], *, provider_id: str) -> Dict[str, Any]:
    if not isinstance(spec, dict):
        raise WeaponSpecValidationError(f"{provider_id} returned a non-object WeaponDesignSpec.")

    validator = schema_validator("weapon-design-spec.schema.json")
    errors = sorted(validator.iter_errors(spec), key=lambda error: list(error.path))
    if errors:
        details = []
        for error in errors[:5]:
            path = "/" + "/".join(str(part) for part in error.path) if error.path else "/"
            details.append(f"{path}: {error.message}")
        suffix = "" if len(errors) <= 5 else f"; +{len(errors) - 5} more"
        raise WeaponSpecValidationError(
            f"{provider_id} returned invalid WeaponDesignSpec: {'; '.join(details)}{suffix}"
        )

    return spec


def validate_quality_report(report: Dict[str, Any], *, provider_id: str) -> Dict[str, Any]:
    if not isinstance(report, dict):
        raise WeaponSpecValidationError(f"{provider_id} returned a non-object QualityReport.")

    validator = schema_validator("quality-report.schema.json")
    errors = sorted(validator.iter_errors(report), key=lambda error: list(error.path))
    if errors:
        details = []
        for error in errors[:5]:
            path = "/" + "/".join(str(part) for part in error.path) if error.path else "/"
            details.append(f"{path}: {error.message}")
        suffix = "" if len(errors) <= 5 else f"; +{len(errors) - 5} more"
        raise WeaponSpecValidationError(f"{provider_id} returned invalid QualityReport: {'; '.join(details)}{suffix}")

    return report


def validate_patch_manifest(manifest: Dict[str, Any], *, provider_id: str) -> Dict[str, Any]:
    if not isinstance(manifest, dict):
        raise WeaponSpecValidationError(f"{provider_id} returned a non-object PatchManifest.")

    validator = schema_validator("patch-manifest.schema.json")
    errors = sorted(validator.iter_errors(manifest), key=lambda error: list(error.path))
    if errors:
        details = []
        for error in errors[:5]:
            path = "/" + "/".join(str(part) for part in error.path) if error.path else "/"
            details.append(f"{path}: {error.message}")
        suffix = "" if len(errors) <= 5 else f"; +{len(errors) - 5} more"
        raise WeaponSpecValidationError(f"{provider_id} returned invalid PatchManifest: {'; '.join(details)}{suffix}")

    return manifest


@lru_cache(maxsize=8)
def schema_validator(schema_name: str) -> Draft202012Validator:
    schema_dir = schema_root()
    schema_store = {}
    for schema_path in schema_dir.glob("*.schema.json"):
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        schema_store[schema_path.name] = schema
        schema_store[schema_path.resolve().as_uri()] = schema
        if "$id" in schema:
            schema_store[str(schema["$id"])] = schema
            schema_store[urljoin(str(schema["$id"]), schema_path.name)] = schema

    root_schema = schema_store.get(schema_name)
    if root_schema is None:
        raise RuntimeError(f"{schema_name} not found in {schema_dir}")

    resolver = RefResolver.from_schema(root_schema, store=schema_store)
    validator = Draft202012Validator(root_schema, resolver=resolver)
    Draft202012Validator.check_schema(root_schema)
    return validator


def schema_root() -> Path:
    if value := os.environ.get("WUSHEN_SCHEMA_DIR"):
        return Path(value).expanduser().resolve()
    repo_root = Path(__file__).resolve().parents[3]
    return repo_root / "packages" / "weapon-spec" / "schemas"
