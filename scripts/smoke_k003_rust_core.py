#!/usr/bin/env python3
"""Code-level acceptance gate for the K003 Rust product-state cutover.

This runner intentionally composes the code-owned Rust and Python tests rather
than reimplementing SQLite, CAS, migration, or geometry behavior in Python. It
also fails closed when a named acceptance test disappears, so a passing cargo
suite cannot silently lose a K003 exit-condition facet.

Packaged WebView/sidecar acceptance remains a separate gate.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Sequence


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
RUST_WRAPPER = ROOT / "script" / "with_rust_toolchain.sh"
PYTHON = ROOT / ".venv" / "bin" / "python"
PYTEST = ROOT / ".venv" / "bin" / "pytest"

CORE_ACCEPTANCE_TESTS = {
    "empty_library_wal_and_schema": (
        "migration::tests::empty_database_reaches_current_schema_in_wal_mode",
    ),
    "historical_fixture_semantic_hash": (
        "migration::tests::historical_legacy_rows_keep_semantic_hash_after_upgrade",
    ),
    "migration_interruption_rollback": (
        "migration::tests::interruption_rolls_back_schema_and_marker_then_retries_cleanly",
        "artifact_migration::tests::interruption_before_database_commit_removes_orphan_and_resumes",
        "artifact_migration::tests::interruption_after_database_commit_keeps_verified_object_and_clears_journal",
    ),
    "historical_artifact_adoption": (
        "artifact_migration::tests::inline_interactive_candidate_is_adopted_but_never_promoted_to_production",
        "artifact_migration::tests::imported_glb_is_adopted_in_place_as_explicit_read_only_reference",
        "artifact_migration::tests::ready_inventory_fast_path_never_reclassifies_later_rust_native_versions",
        "artifact_migration::tests::tampered_dual_profile_object_is_never_classified_ready",
        "artifact_migration::tests::wrong_profile_dual_references_are_never_classified_ready",
    ),
    "external_glb_strict_inspection_and_atomic_import": (
        "external_glb::tests::strict_external_glb_inspection_reads_only_verified_facts",
        "external_glb::tests::strict_external_glb_rejects_external_resources_compression_and_bad_accessors",
        "external_glb::tests::request_hash_matches_legacy_semantics_and_filename_is_display_only",
        "import_is_atomic_idempotent_cas_backed_and_restart_readable",
        "historical_idempotency_replay_survives_a_later_imported_head",
        "rejected_or_unowned_import_leaves_no_database_or_cas_state",
    ),
    "single_writer_cutover": (
        "ownership::tests::cutover_is_explicit_and_second_writer_is_rejected",
        "ownership::tests::unpublished_first_cutover_rolls_back_but_published_cutover_cannot",
        "repository::tests::unpublished_first_cutover_process_crash_fail_forwards_and_recovers_pending_cas",
        "repository::tests::repository_rechecks_durable_writer_epoch_inside_every_transaction",
    ),
    "bootstrap_deletion_recovery_cutover_atomicity": (
        "repository::tests::bootstrap_deletion_recovery_does_not_publish_and_allows_cutover_rollback",
        "repository::tests::bootstrap_deletion_recovery_retries_then_publishes_and_restarts_normally",
    ),
    "stale_snapshot_and_preview": (
        "repository::tests::stale_etag_rolls_back_without_partial_state",
        "concurrent_cas_project_switch_restart_and_export_hashes_are_authoritative",
    ),
    "confirm_undo_redo": (
        "repository::tests::preview_confirm_and_immutable_undo_redo_are_atomic",
        "preview_and_confirm_bundles_are_atomic_replayable_and_restart_readable",
        "confirm_failure_at_quality_insert_keeps_complete_preview_and_no_partial_result",
        "concurrent_confirm_callers_receive_one_identical_authoritative_bundle",
        "locking_after_proposal_blocks_preview_without_any_preview_side_effect",
        "a_lock_appearing_after_preview_blocks_confirm_and_preserves_preview_bundle",
        "concurrent_cas_project_switch_restart_and_export_hashes_are_authoritative",
    ),
    "project_switch_restart_and_concurrent_cas": (
        "concurrent_cas_project_switch_restart_and_export_hashes_are_authoritative",
    ),
    "object_hash_refcount_quality_glb_export": (
        "repository::tests::object_reference_counts_deduplicate_and_corruption_is_detected",
        "repository::tests::deletion_journal_recovers_crash_after_index_commit_without_scanning",
        "repository::tests::quality_is_bound_to_current_asset_and_stale_insert_rolls_back",
        "repository::tests::candidate_glb_lives_in_cas_and_commit_creates_one_authoritative_chain",
        "concurrent_cas_project_switch_restart_and_export_hashes_are_authoritative",
    ),
    "material_texture_cas_and_license_boundary": (
        "repository::tests::material_texture_registration_is_idempotent_cas_backed_and_snapshot_free",
        "repository::tests::material_texture_rejects_unlicensed_and_path_like_payloads_before_writing",
        "repository::tests::material_texture_bootstrap_adopts_valid_historical_python_cas_object",
    ),
    "component_registry_and_structure_facts": (
        "repository::tests::rust_components_and_structure_suggestions_recompute_authoritative_facts",
    ),
    "semantic_proportion_readback": (
        "eligible_active_part_uses_persisted_scale_and_exact_cas_readback_without_writes",
        "stale_q003_readback_fails_closed_instead_of_trusting_glb_appearance",
        "stale_asset_and_external_reference_fail_before_returning_controls",
        "no_binding_and_unmatched_surface_provenance_are_explicitly_unavailable",
    ),
    "legacy_read_only_hash": (
        "legacy_semantic_hash_adapter_is_read_only_across_restart",
        "repository::legacy_read::tests::legacy_read_models_preserve_semantic_hash_and_zero_write_across_restart",
    ),
    "legacy_conversion_authorization_and_consumption": (
        "authorization_is_cas_idempotent_restart_durable_and_geometry_free",
        "first_agent_bundle_requires_exact_intent_then_activates_and_consumes_it_atomically",
        "stale_authorization_and_non_legacy_projects_are_rejected_without_partial_state",
    ),
}

DESKTOP_RUNTIME_TESTS = {
    "rust_lifecycle_restart": (
        "rust_core_runtime::tests::lifecycle_port_persists_across_published_runtime_restart",
    ),
    "cancelled_lifecycle_zero_write": (
        "rust_core_runtime::tests::cancelled_lifecycle_command_never_writes",
    ),
    "rust_project_routes": (
        "rust_core_runtime::tests::project_routes_bootstrap_profile_without_creating_legacy_versions",
    ),
    "snapshot_quality_glb_export": (
        "rust_core_runtime::tests::snapshot_quality_glb_export_and_etag_contracts_are_rust_authoritative",
    ),
    "native_changeset_geometry_bundle": (
        "app_server_bridge::tests::rust_change_set_compat_seals_preview_glb_and_confirms_one_atomic_bundle",
    ),
    "native_render_and_package": (
        "app_server_bridge::tests::rust_blockout_compat_build_segment_commit_owns_glb_snapshot_quality_and_cas",
        "asset_render_compat::tests::package_is_deterministic_zip32_and_stale_fingerprint_fails_closed",
    ),
    "material_texture_routes": (
        "rust_core_runtime::tests::m103_material_texture_routes_are_rust_owned_and_catalog_preserving",
        "app_server_bridge::tests::loopback_routes_m103_registration_query_and_material_enrichment_to_rust_core",
    ),
    "external_glb_rust_routes": (
        "rust_core_runtime::tests::external_glb_import_preview_quality_and_export_are_rust_owned_read_only_references",
        "app_server_bridge::tests::loopback_routes_external_glb_import_and_readback_to_rust_without_python",
    ),
    "component_and_structure_routes": (
        "rust_core_runtime::tests::k003_product_routes_are_idempotent_validated_and_never_require_python_state",
    ),
    "semantic_proportion_route": (
        "rust_core_runtime::tests::semantic_proportion_get_is_rust_owned_and_never_falls_back_to_python",
    ),
    "legacy_read_only_http_adapter": (
        "rust_core_runtime::legacy_read_http_tests::legacy_read_only_http_adapters_are_rust_owned_bounded_and_zero_write",
    ),
    "legacy_conversion_rust_route": (
        "rust_core_runtime::tests::convert_legacy_post_is_rust_owned_and_rejects_an_agent_snapshot",
        "app_server_bridge::tests::legacy_conversion_public_http_build_segment_commit_is_restart_idempotent_and_preserves_source",
    ),
    "no_python_product_or_legacy_sse_fallback": (
        "app_server_bridge::tests::production_bridge_unknown_retired_gets_and_legacy_sse_never_reach_python",
        "tests::packaged_python_facet_never_receives_probe_or_legacy_writer_switches",
    ),
    "runtime_undo_redo": (
        "rust_core_runtime::tests::undo_and_redo_create_new_immutable_versions_and_replay_by_request_id",
    ),
}

PYTHON_BOUNDARY_TESTS = {
    "default_geometry_only_and_410": (
        "test_default_app_is_geometry_only_and_legacy_environment_cannot_reenable_product_core",
    ),
    "no_legacy_modules": (
        "test_default_import_graph_does_not_load_legacy_product_or_persistence_modules",
    ),
    "explicit_test_factory_only": (
        "test_legacy_product_core_requires_explicit_direct_test_factory",
    ),
    "no_product_authority": (
        "test_capability_is_loopback_only_and_reports_no_product_authority",
    ),
    "strict_ir_and_opaque_handle": (
        "test_profile_and_section_companions_must_be_strict_canonical_ir_witnesses",
        "test_compile_readback_then_render_uses_only_opaque_ephemeral_handle",
    ),
    "cancel_timeout_crash_late_result": (
        "test_environment_and_cancellation_boundaries_are_fail_closed",
        "test_disposable_worker_timeout_crash_and_late_result_tombstone",
    ),
    "no_db_object_provider_environment": (
        "test_worker_child_environment_retains_only_audited_bundle_root",
        "test_sidecar_entry_overrides_resource_injection_and_strips_product_authority",
    ),
    "formal_launcher_drops_legacy_writer_switches": (
        "test_sidecar_entry_overrides_resource_injection_and_strips_product_authority",
    ),
}


class GateFailure(RuntimeError):
    pass


def _display(command: Sequence[str]) -> str:
    return " ".join(command)


def _run(
    command: Sequence[str],
    *,
    capture: bool = False,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    print(f"[k003-code-gate] {_display(command)}", flush=True)
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        text=True,
        capture_output=capture,
        check=False,
    )
    if capture:
        if completed.stdout:
            print(completed.stdout, end="" if completed.stdout.endswith("\n") else "\n")
        if completed.stderr:
            print(
                completed.stderr,
                end="" if completed.stderr.endswith("\n") else "\n",
                file=sys.stderr,
            )
    if completed.returncode != 0:
        raise GateFailure(
            f"command exited with {completed.returncode}: {_display(command)}"
        )
    return completed


def _rust_command(*arguments: str) -> list[str]:
    if not arguments:
        raise GateFailure("cargo subcommand is required")
    return [
        str(RUST_WRAPPER),
        "cargo",
        arguments[0],
        "--manifest-path",
        str(MANIFEST),
        *arguments[1:],
    ]


def _listed_rust_tests(package: str) -> set[str]:
    completed = _run(
        _rust_command("test", "-p", package, "--offline", "--", "--list"),
        capture=True,
    )
    return {
        line.rsplit(": test", 1)[0].strip()
        for line in completed.stdout.splitlines()
        if line.rstrip().endswith(": test")
    }


def _listed_python_tests(environment: dict[str, str]) -> set[str]:
    completed = _run(
        [
            str(PYTEST),
            "--collect-only",
            "-q",
            "apps/agent/tests/test_k003_restricted_geometry_executor.py",
        ],
        capture=True,
        environment=environment,
    )
    return {
        line.rsplit("::", 1)[-1].strip()
        for line in completed.stdout.splitlines()
        if "::test_" in line
    }


def _require_named_tests(
    available: set[str],
    required: dict[str, tuple[str, ...]],
    suite: str,
) -> None:
    missing: list[str] = []
    for facet, names in required.items():
        for name in names:
            if not any(
                candidate == name or candidate.endswith(f"::{name}")
                for candidate in available
            ):
                missing.append(f"{facet}: {name}")
    if missing:
        raise GateFailure(
            f"{suite} lost named K003 acceptance coverage: {', '.join(missing)}"
        )


def _flatten(mapping: dict[str, tuple[str, ...]]) -> list[str]:
    return sorted({name for names in mapping.values() for name in names})


def _clean_python_environment() -> dict[str, str]:
    environment = dict(os.environ)
    for name in tuple(environment):
        upper = name.upper()
        if (
            "API_KEY" in upper
            or "PROVIDER" in upper
            or upper
            in {
                "WUSHEN_LIBRARY_ROOT",
                "WUSHEN_MIGRATIONS_DIR",
                "DATABASE_URL",
                "FORGECAD_DATABASE_PATH",
                "FORGECAD_SQLITE_PATH",
                "FORGECAD_OBJECT_STORE_ROOT",
                "FORGECAD_LIBRARY_ROOT",
                "OPENAI_BASE_URL",
            }
        ):
            environment.pop(name, None)
    environment["PYTHONPATH"] = str(ROOT / "apps" / "agent")
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    return environment


def _last_json_line(output: str) -> dict[str, object]:
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise GateFailure("restricted geometry smoke returned no JSON evidence")


def main() -> int:
    for required_path in (RUST_WRAPPER, MANIFEST, PYTHON, PYTEST):
        if not required_path.is_file():
            raise GateFailure(
                f"required workspace dependency is missing: {required_path}"
            )

    python_environment = _clean_python_environment()
    core_tests = _listed_rust_tests("forgecad-core")
    _require_named_tests(core_tests, CORE_ACCEPTANCE_TESTS, "forgecad-core")
    desktop_tests = _listed_rust_tests("wushen-forge-desktop")
    _require_named_tests(
        desktop_tests,
        DESKTOP_RUNTIME_TESTS,
        "wushen-forge-desktop RustCoreRuntime",
    )
    python_tests = _listed_python_tests(python_environment)
    _require_named_tests(
        python_tests,
        PYTHON_BOUNDARY_TESTS,
        "RestrictedGeometryExecutor",
    )

    _run(
        _rust_command("test", "-p", "forgecad-core", "--offline"),
    )
    _run(
        _rust_command("test", "-p", "wushen-forge-desktop", "--offline"),
    )
    _run(
        [
            str(PYTEST),
            "-q",
            "apps/agent/tests/test_k003_restricted_geometry_executor.py",
        ],
        environment=python_environment,
    )
    runtime_smoke = _run(
        [str(PYTHON), "scripts/smoke_k003_restricted_geometry.py"],
        capture=True,
        environment=python_environment,
    )
    runtime_evidence = _last_json_line(runtime_smoke.stdout)
    if (
        runtime_evidence.get("status") != "pass"
        or runtime_evidence.get("schema_version")
        != "K003RestrictedGeometryRuntimeSmoke@1"
    ):
        raise GateFailure("restricted geometry process evidence did not pass")

    print(
        json.dumps(
            {
                "status": "pass",
                "schema_version": "K003RustCoreCodeAcceptance@1",
                "packaged_evidence": False,
                "core_acceptance_facets": sorted(CORE_ACCEPTANCE_TESTS),
                "desktop_runtime_facets": sorted(DESKTOP_RUNTIME_TESTS),
                "python_boundary_facets": sorted(PYTHON_BOUNDARY_TESTS),
                "core_tests_required": _flatten(CORE_ACCEPTANCE_TESTS),
                "desktop_tests_required": _flatten(DESKTOP_RUNTIME_TESTS),
                "python_tests_required": _flatten(PYTHON_BOUNDARY_TESTS),
                "restricted_geometry": {
                    "glb_sha256": runtime_evidence.get("glb_sha256"),
                    "provider_calls": runtime_evidence.get("provider_calls"),
                    "persistent_product_artifacts": runtime_evidence.get(
                        "persistent_product_artifacts"
                    ),
                },
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(f"K003 Rust core code acceptance failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
