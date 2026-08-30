#!/usr/bin/env python3
"""Fail-closed structural gate for WPN-ARCH-SURFACE-001."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PROFILE = ROOT / "packages/forgecad-contracts/profiles/weaponry-knife-p0.json"
RUNTIME = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src"
STORE = ROOT / "apps/desktop/src-tauri/crates/forgecad-store/src"
CONTRACT_MAP = (
    ROOT
    / "apps/desktop/src-tauri/crates/forgecad-contracts/src/weaponry_domain_map.rs"
)

EXPECTED_READ = [
    "appearance_source_lineage_get",
    "hero_uv_durable_get",
    "low_quad_draft_durable_get",
    "production_weapon_form_quality_v2_preflight_get",
    "production_weapon_formal_high_get",
    "production_weapon_high_low_bake_get",
    "production_weapon_high_low_bake_preflight_get",
    "production_weapon_retopology_cage_source_get",
]
EXPECTED_WRITE = [
    "appearance_prepare",
    "appearance_source_lineage_prepare",
    "hero_uv_durable_prepare",
    "low_quad_draft_durable_prepare",
    "production_weapon_formal_high_prepare",
    "production_weapon_high_low_bake_prepare",
    "production_weapon_retopology_cage_source_prepare",
]
COMPATIBILITY_ONLY = [
    "production_weapon_retopology_cage_source_bundle_prepare",
    "production_weapon_retopology_cage_source_bundle_get",
]
BAKE_OPERATIONS = [
    "production_weapon_high_low_bake_get",
    "production_weapon_high_low_bake_preflight_get",
    "production_weapon_high_low_bake_prepare",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"WPN-ARCH-SURFACE-001 FAIL: {message}")


def read(path: Path) -> str:
    require(path.is_file(), f"missing {path.relative_to(ROOT)}")
    return path.read_text(encoding="utf-8")


def main() -> int:
    profile = json.loads(read(PROFILE))
    surface = profile["facades"]["surface_pipeline"]
    require(surface["read_tools"] == EXPECTED_READ, "Surface read operation set drifted")
    require(surface["write_tools"] == EXPECTED_WRITE, "Surface write operation set drifted")
    active = surface["underlying_operations"]
    require(
        sorted(active) == sorted(EXPECTED_READ + EXPECTED_WRITE),
        "Surface active operation allowlist is not the exact 15-operation set",
    )
    require(
        not set(active).intersection(COMPATIBILITY_ONLY),
        "retopology bundle aliases leaked into the active Knife profile",
    )

    service = read(RUNTIME / "surface_service.rs")
    service_implementation = service.split("#[cfg(test)]", 1)[0]
    for operation in active + COMPATIBILITY_ONLY:
        require(f'"{operation}"' in service, f"Surface service omits {operation}")
    require(
        ".dispatch_ipc(" not in service_implementation,
        "Surface service calls the generic Runtime dispatcher",
    )

    router = read(RUNTIME / "runtime_operation_router.rs")
    require(
        "WeaponryServiceDomain::Surface =>" in router
        and "surface_service::invoke(self.runtime, operation, payload)" in router,
        "typed Runtime router does not invoke Surface service directly",
    )
    require(
        "WeaponryServiceDomain::Surface =>" in router
        and "WeaponryServiceDomain::Surface |" not in router
        and "| WeaponryServiceDomain::Surface" not in router,
        "Surface returned to a generic dispatch_ipc fallback",
    )

    runtime_root = read(RUNTIME / "lib.rs")
    require(
        "runtime_services::surface_service::is_surface_operation(method)" in runtime_root
        and "runtime_services::surface_service::invoke(self, method, payload)" in runtime_root,
        "compatibility IPC does not bridge to the typed Surface service",
    )
    require(
        "mod surface_service;" not in runtime_root,
        "Surface service became a new Runtime root module instead of a domain child",
    )

    runtime_services = read(RUNTIME / "runtime_services.rs")
    require(
        '#[path = "surface_service.rs"]\npub(crate) mod surface_service;' in runtime_services,
        "Surface service is not owned by the Runtime services domain module",
    )

    bake = read(RUNTIME / "production_weapon_high_low_bake.rs")
    preflight = read(RUNTIME / "production_weapon_high_low_bake_preflight.rs")
    require(
        bake.count(".surface_repository()") >= 2,
        "formal Bake prepare/get bypass SurfaceRepository",
    )
    require(
        ".surface_repository()" in preflight,
        "formal Bake preflight bypasses SurfaceRepository",
    )

    store_root = read(STORE / "lib.rs")
    repository = read(STORE / "surface_repository.rs")
    for symbol in [
        "ProductionWeaponHighLowBakeCommitBundle",
        "ProductionWeaponHighLowBakePreflightSourceSummary",
        "ProductionWeaponHighLowBakePreflightSources",
    ]:
        require(f"pub struct {symbol}" not in store_root, f"{symbol} returned to Store root")
        require(f"pub struct {symbol}" in repository, f"repository omits {symbol}")
    for method in [
        "get_production_weapon_high_low_bake_preflight_sources",
        "commit_production_weapon_high_low_bake",
        "get_production_weapon_high_low_bake",
    ]:
        require(f"pub fn {method}" not in store_root, f"{method} returned to Store root")
        require(f"pub fn {method}" in repository, f"repository omits {method}")

    contract_map = read(CONTRACT_MAP)
    require(
        'capability: "formal_high_low_cage_bake"' in contract_map,
        "central Contract map omits the formal Surface aggregate",
    )
    for operation in BAKE_OPERATIONS:
        require(f'"{operation}"' in contract_map, f"Contract map omits {operation}")

    boundaries = read(STORE / "repository_boundaries.rs")
    require(
        'capability: "formal_high_low_bake"' not in boundaries,
        "resolved formal Surface aggregate remains listed as a Store mapping gap",
    )

    print(
        json.dumps(
            {
                "schema_version": "WeaponrySurfaceArchitectureCheck@1",
                "status": "PASS",
                "active_surface_operations": len(active),
                "active_read_operations": len(EXPECTED_READ),
                "active_write_operations": len(EXPECTED_WRITE),
                "compatibility_only_aliases": len(COMPATIBILITY_ONLY),
                "runtime_router": "typed_surface_service",
                "store_repository": "borrowed_single_owner",
                "formal_bake_mapping": "contract-runtime-store-mcp-bound",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
