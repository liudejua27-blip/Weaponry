#!/usr/bin/env bash
set -euo pipefail

# Local macOS verification entrypoint for the Tauri workbench.
#
# `run` and `--verify` deliberately preserve the historic source-Python
# developer loop.  `--mvp` and `--mvp-verify` instead exercise the actual
# release-shaped macOS path: a Rust-owned app-server/core plus the bundled,
# capability-gated restricted-geometry sidecar.  The MVP modes never prepare
# the legacy visual pack and never make a Provider request.

MODE="${1:-run}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/wushen-forge-desktop"

RUSTC_BIN="$(rustup which rustc 2>/dev/null || true)"
if [[ -n "$RUSTC_BIN" && -x "$RUSTC_BIN" ]]; then
  export PATH="$(dirname "$RUSTC_BIN"):$PATH"
fi
export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
# The historic development loop builds a cached Blender Pack from the current
# authored source. Keep that dependency lazy: the Rust-first MVP must neither
# prepare nor require this legacy visual-pack path.
VISUAL_SOURCE_FINGERPRINT=""
VISUAL_PACK_CACHE=""
ORIGINAL_AUTHOR_PACK=""
CURRENT_FORMAL_PACK=""
LOCAL_TEST_MODULE_PACK="${FORGECAD_LOCAL_TEST_MODULE_PACK:-}"
LOCAL_TEST_LIBRARY_ROOT="${WUSHEN_LOCAL_TEST_LIBRARY_ROOT:-}"

initialize_legacy_visual_paths() {
  if [[ -n "$VISUAL_SOURCE_FINGERPRINT" ]]; then
    return 0
  fi
  VISUAL_SOURCE_FINGERPRINT="$(shasum -a 256 "$ROOT_DIR/scripts/blender/weapon_concept_starter.py" "$ROOT_DIR/scripts/stage_original_author_visual_pack.py" | shasum -a 256 | cut -c1-16)"
  VISUAL_PACK_CACHE="$HOME/Library/Caches/ForgeCAD/OriginalAuthorVisualPacks/$VISUAL_SOURCE_FINGERPRINT"
  ORIGINAL_AUTHOR_PACK="$VISUAL_PACK_CACHE/final-pack"
  CURRENT_FORMAL_PACK="$HOME/Library/Caches/ForgeCAD/Formalization/current/final-pack"
}

prepare_local_visual_pack() {
  if [[ -f "$ORIGINAL_AUTHOR_PACK/pack.json" ]]; then
    mkdir -p "$(dirname "$CURRENT_FORMAL_PACK")"
    ln -sfn "$ORIGINAL_AUTHOR_PACK" "$CURRENT_FORMAL_PACK"
    return 0
  fi
  mkdir -p "$VISUAL_PACK_CACHE"
  "$ROOT_DIR/.venv/bin/python" "$ROOT_DIR/scripts/build_blender_starter_pack.py" \
    --module-set full_candidate --require-blender \
    --output-root "$VISUAL_PACK_CACHE/candidate-pack"
  "$ROOT_DIR/.venv/bin/python" "$ROOT_DIR/scripts/stage_original_author_visual_pack.py" \
    --candidate-root "$VISUAL_PACK_CACHE/candidate-pack" \
    --output-root "$ORIGINAL_AUTHOR_PACK"
  mkdir -p "$(dirname "$CURRENT_FORMAL_PACK")"
  ln -sfn "$ORIGINAL_AUTHOR_PACK" "$CURRENT_FORMAL_PACK"
}

configure_legacy_runtime() {
  export WUSHEN_AGENT_RUNTIME_MODE="local-dev-python"
  export WUSHEN_REPO_ROOT="$ROOT_DIR"
  initialize_legacy_visual_paths

  if [[ -z "$LOCAL_TEST_MODULE_PACK" && "${FORGECAD_LOCAL_VISUAL_PACK:-1}" != "0" ]]; then
    prepare_local_visual_pack
    LOCAL_TEST_LIBRARY_ROOT="${LOCAL_TEST_LIBRARY_ROOT:-$HOME/Library/Caches/ForgeCAD/LocalWorkbenchLibraries/$VISUAL_SOURCE_FINGERPRINT}"
  fi
  if [[ -n "$LOCAL_TEST_MODULE_PACK" ]]; then
    if [[ ! -f "$LOCAL_TEST_MODULE_PACK/pack.json" ]]; then
      echo "FORGECAD_LOCAL_TEST_MODULE_PACK must point to a ModulePackManifest@1 directory" >&2
      exit 2
    fi
    # launchctl encodes a non-ASCII workspace path incorrectly for the Python
    # child on some macOS versions. Pass an ASCII-only symlink in the user cache
    # so a native Tauri candidate test is not dependent on the repository name.
    LOCAL_TEST_PACK_LINK_ROOT="$HOME/Library/Caches/ForgeCAD/LocalTestPacks"
    mkdir -p "$LOCAL_TEST_PACK_LINK_ROOT"
    LOCAL_TEST_PACK_LINK="$LOCAL_TEST_PACK_LINK_ROOT/$(printf '%s' "$LOCAL_TEST_MODULE_PACK" | shasum -a 256 | cut -c1-16)"
    ln -sfn "$LOCAL_TEST_MODULE_PACK" "$LOCAL_TEST_PACK_LINK"
    export FORGECAD_BUNDLED_MODULE_PACK="$LOCAL_TEST_PACK_LINK"
  elif [[ -f "$ORIGINAL_AUTHOR_PACK/pack.json" ]]; then
    export FORGECAD_BUNDLED_MODULE_PACK="$ORIGINAL_AUTHOR_PACK"
  fi
  if [[ -n "$LOCAL_TEST_LIBRARY_ROOT" ]]; then
    export WUSHEN_LIBRARY_ROOT="$LOCAL_TEST_LIBRARY_ROOT"
  fi
}

configure_mvp_runtime() {
  local enable_offline_arm="${1:-0}"
  # The packaged app must not fall back to repository Python, test or legacy
  # product-state hooks.  The Rust supervisor itself performs the private
  # capability ownership handshake before it exposes geometry as ready.
  unset WUSHEN_REPO_ROOT WUSHEN_AGENT_PYTHON WUSHEN_AGENT_SIDE_CAR
  unset WUSHEN_LIBRARY_ROOT FORGECAD_BUNDLED_MODULE_PACK
  unset FORGECAD_LOCAL_TEST_MODULE_PACK WUSHEN_LOCAL_TEST_LIBRARY_ROOT
  unset FORGECAD_K001_PACKAGED_PROBE FORGECAD_K002_PACKAGED_PROBE FORGECAD_K003_PACKAGED_PROBE
  unset FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE
  export WUSHEN_AGENT_RUNTIME_MODE="packaged-sidecar"
  # This local proof is intentionally offline. It validates the production
  # ownership path, not a paid model invocation or a user credential.
  export FORGECAD_DISABLE_PROVIDER_CONFIG="1"
  export FORGECAD_CONCEPT_WORKER_ENABLED="0"
  export WUSHEN_LOCAL_WORKER_ENABLED="0"
  if [[ "$enable_offline_arm" == "1" ]]; then
    # `--mvp` is an explicit local product demo: the Rust-owned Provider
    # performs one deterministic mechanical-arm Turn without reading Keychain
    # or opening a network transport. Keep this off for the generic K003
    # lifecycle verifier, whose fixture intentionally proves a separate path.
    export FORGECAD_MVP_OFFLINE_ARM="1"
  else
    unset FORGECAD_MVP_OFFLINE_ARM
  fi
  if [[ -n "${FORGECAD_MVP_LIBRARY_ROOT:-}" ]]; then
    export WUSHEN_LIBRARY_ROOT="$FORGECAD_MVP_LIBRARY_ROOT"
  fi
}

# The opt-in local mechanical-arm MVP remains deterministic: it never reads a
# model credential or opens a network transport. Live-provider checks are a
# separate, user-initiated operation with a newly issued key.

build_app() {
  (cd "$ROOT_DIR" && npm --workspace apps/desktop run tauri -- build --bundles app)
}

build_mvp_app() {
  # Build the target-suffixed externalBin first. Tauri then copies that exact
  # frozen sidecar into the .app, so the launch cannot accidentally resolve a
  # repository-local Python interpreter.
  (cd "$ROOT_DIR" && npm run desktop:packaged-sidecar-build)
  (cd "$ROOT_DIR" && npm run desktop:tauri-build-app)
}

stop_local_app() {
  pkill -TERM -f "$APP_BINARY" >/dev/null 2>&1 || true
  # `pkill` does not emit a macOS close event, so the app-managed Python child
  # can otherwise outlive the bundle and cause the next test run to reuse old
  # code or old provider configuration on port 8000.
  if [[ "${WUSHEN_KEEP_EXISTING_AGENT:-0}" != "1" ]]; then
    pkill -TERM -f 'wushen_agent.main:create_app.*--port 8000' >/dev/null 2>&1 || true
  fi
}

launch_app() {
  if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "Expected app bundle was not created: $APP_BUNDLE" >&2
    exit 1
  fi
  # Run the bundle binary directly for verification so the Rust supervisor
  # receives the same non-secret runtime paths as this shell. LaunchServices
  # drops shell variables in CI/headless sessions and can leave a GUI process
  # alive without starting its managed Agent. Credentials are never exported.
  "$APP_BINARY" >"$ROOT_DIR/.wushen-tauri.log" 2>&1 &
  TAURI_APP_PID=$!
  disown "$TAURI_APP_PID" 2>/dev/null || true
}

launch_mvp_app() {
  if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "Expected app bundle was not created: $APP_BUNDLE" >&2
    exit 1
  fi
  # LaunchServices is the user-facing macOS lifecycle. Forward only the
  # code-owned MVP switches; neither a Provider credential nor a repository
  # path is placed in the launched process environment.
  local launch=(
    /usr/bin/open -n
    --env "WUSHEN_AGENT_RUNTIME_MODE=packaged-sidecar"
    --env "FORGECAD_DISABLE_PROVIDER_CONFIG=1"
    --env "FORGECAD_MVP_OFFLINE_ARM=1"
    --env "FORGECAD_CONCEPT_WORKER_ENABLED=0"
    --env "WUSHEN_LOCAL_WORKER_ENABLED=0"
  )
  if [[ -n "${WUSHEN_LIBRARY_ROOT:-}" ]]; then
    launch+=(--env "WUSHEN_LIBRARY_ROOT=$WUSHEN_LIBRARY_ROOT")
  fi
  launch+=("$APP_BUNDLE")
  "${launch[@]}"
}

wait_for_agent() {
  for _ in {1..25}; do
    if curl -fsS http://127.0.0.1:8000/api/health >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "local Agent did not become healthy; inspect $ROOT_DIR/.wushen-agent.log" >&2
  return 1
}

wait_for_packaged_agent() {
  for _ in {1..35}; do
    if curl -fsS http://127.0.0.1:8000/api/health 2>/dev/null | node -e '
      let raw = "";
      process.stdin.setEncoding("utf8");
      process.stdin.on("data", (chunk) => { raw += chunk; });
      process.stdin.on("end", () => {
        try {
          const payload = JSON.parse(raw);
          const expected = {
            status: "ok",
            service: "forgecad-restricted-geometry-executor",
            mode: "restricted_geometry_executor",
            schema_version: "RestrictedGeometryExecutorHealth@1",
            python_role: "restricted_geometry_executor",
            database_access: false,
            object_store_access: false,
            provider_access: false,
            snapshot_write: false,
            persistent_state_writer: false,
          };
          process.exit(JSON.stringify(payload) === JSON.stringify(expected) ? 0 : 1);
        } catch (_) { process.exit(1); }
      });
    '; then
      return 0
    fi
    sleep 1
  done
  echo "packaged restricted-geometry sidecar did not become healthy; inspect the app supervisor log" >&2
  return 1
}

stop_mvp_app() {
  pkill -TERM -f "$APP_BINARY" >/dev/null 2>&1 || true
  # The bundled PyInstaller sidecar can outlive a force-terminated desktop
  # window while it is still running from this exact app bundle. This is an
  # owned packaged child path, not a broad `wushen-agent` match.
  pkill -TERM -f "$APP_BUNDLE/Contents/MacOS/wushen-agent agent serve" >/dev/null 2>&1 || true
  for _ in {1..10}; do
    if ! lsof -nP -iTCP:8000 -sTCP:LISTEN >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  # Never kill an unknown listener: a remaining port can belong to another
  # desktop session and is an ownership failure, not a reason to broaden
  # process matching.
  echo "port 8000 remained occupied after stopping the MVP desktop; refusing to kill an unowned listener" >&2
  return 1
}

verify_packaged_mvp() {
  stop_mvp_app
  build_mvp_app
  # This native two-launch gate uses LaunchServices, a temporary Rust Library
  # and a fresh sidecar capability per launch. It verifies exact health,
  # desktop-child ownership, the Python no-state/no-provider boundary, Rust
  # Core Snapshot/CAS/version ownership, a GLB export, and restart recovery.
  # It intentionally makes zero Provider calls and terminates its test apps.
  (cd "$ROOT_DIR" && npm run desktop:k003-packaged-native-smoke)
  echo "mvp_verification: passed"
  echo "runtime_owner: rust-app-server + rust-core"
  echo "python_role: restricted_geometry_executor"
  echo "provider_calls: 0"
  echo "restart_recovery: passed"
}

verify_packaged_arm_mvp() {
  stop_mvp_app
  build_mvp_app
  local probe_root probe_report probe_log probe_pid resume_report resume_log resume_pid evidence_root
  probe_root="$(mktemp -d "${TMPDIR:-/tmp}/forgecad-mvp-arm-library.XXXXXX")"
  # BSD mktemp requires the X run at the end of its template. Keep the
  # caller-owned evidence and log inside the already unique temporary library
  # instead of using suffix-bearing templates that fail on macOS.
  probe_report="$probe_root/packaged-protocol-proof.json"
  probe_log="$probe_root/packaged-app.log"
  resume_report="$probe_root/packaged-resume-proof.json"
  resume_log="$probe_root/packaged-resume-app.log"
  evidence_root="$ROOT_DIR/output/arm-mvp-golden-path"
  env \
    WUSHEN_AGENT_RUNTIME_MODE="packaged-sidecar" \
    FORGECAD_DISABLE_PROVIDER_CONFIG="1" \
    FORGECAD_MVP_OFFLINE_ARM="1" \
    FORGECAD_CONCEPT_WORKER_ENABLED="0" \
    WUSHEN_LOCAL_WORKER_ENABLED="0" \
    WUSHEN_LIBRARY_ROOT="$probe_root" \
    FORGECAD_MVP_ARM_PACKAGED_PROBE="1" \
    FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT="$probe_report" \
    "$APP_BINARY" >"$probe_log" 2>&1 &
  probe_pid=$!
  # The golden path includes an initial production Turn, several lightweight
  # ChangeSet previews, and a final production export. Keep the outer
  # lifecycle bound above two cold production compiles plus interactive edits;
  # this is a probe ceiling, not a geometry or Provider budget.
  for _ in {1..1200}; do
    if [[ -f "$probe_report" ]]; then
      break
    fi
    sleep 1
  done
  if [[ ! -f "$probe_report" ]]; then
    kill -TERM "$probe_pid" >/dev/null 2>&1 || true
    wait "$probe_pid" >/dev/null 2>&1 || true
    tail -120 "$probe_log" >&2 || true
    rm -rf "$probe_root"
    echo "packaged mechanical-arm MVP probe did not produce evidence" >&2
    return 1
  fi
  .venv/bin/python - "$probe_report" <<'PY'
import json, sys
report = json.load(open(sys.argv[1], encoding="utf-8"))
provider = report.get("provider", {})
preview = report.get("preview", {})
a005 = report.get("a005", {})
active = report.get("active_design", {})
export = report.get("export", {})
assert report.get("schema_version") == "ForgeCADArmMvpPackagedProtocolProof@3", report
assert report.get("status") == "pass", report
assert all(report.get(key) for key in ("project_id", "thread_id", "turn_id", "v1_asset_version_id")), report
assert report.get("root_recipe_id") == "recipe_c106_arm_service_display", report
assert preview.get("artifact_profile_id") == "production_concept", report
assert isinstance(preview.get("glb_sha256"), str) and len(preview["glb_sha256"]) == 64, report
# C108 keeps the lightweight preview separate and promotes the same recipe to
# the reviewed 80k–150k production envelope. The packaged proof must reject a
# sparse preview artifact as well as an over-budget export.
assert 80000 <= preview.get("triangle_count", 0) <= 150000, report
assert a005.get("parent_asset_version_id") == report.get("v1_asset_version_id"), report
assert a005.get("v2_asset_version_id") != active.get("asset_version_id"), report
assert a005.get("surface_adornment_count", 0) >= 1, report
assert report.get("c110c", {}).get("parent_asset_version_id") == a005.get("v2_asset_version_id"), report
assert report.get("c110c", {}).get("v3_asset_version_id") == report.get("c110d", {}).get("parent_asset_version_id"), report
assert report.get("c110c", {}).get("added_part_id") == "part_c110c_sensor_pod", report
assert report.get("c110c", {}).get("operation_count") == 3, report
assert isinstance(report.get("c110c", {}).get("preview_glb_sha256"), str) and len(report["c110c"]["preview_glb_sha256"]) == 64, report
assert report.get("c110c", {}).get("preview_triangle_count", 0) > 0, report
assert report.get("c110d", {}).get("parent_asset_version_id") == report.get("c110c", {}).get("v3_asset_version_id"), report
assert report.get("c110d", {}).get("v4_asset_version_id") == active.get("asset_version_id"), report
assert report.get("c110d", {}).get("added_part_ids") == ["part_c110d_actuator_cover", "part_c110d_cable_guide"], report
assert report.get("c110d", {}).get("recipe_ids") == ["recipe_c110d_arm_actuator_cover", "recipe_c110d_arm_cable_guide"], report
assert report.get("c110d", {}).get("operation_count") == 2, report
assert isinstance(report.get("c110d", {}).get("preview_glb_sha256"), str) and len(report["c110d"]["preview_glb_sha256"]) == 64, report
assert report.get("c110d", {}).get("preview_triangle_count", 0) > report.get("c110c", {}).get("preview_triangle_count", 0), report
assert active.get("snapshot_revision", 0) >= 1, report
assert export.get("asset_version_id") == report.get("c110d", {}).get("v4_asset_version_id"), report
assert 80000 <= export.get("triangle_count", 0) <= 150000, report
assert export.get("x_forgecad_glb_sha256") == export.get("glb_sha256"), report
assert provider == {
    "source_kind": "offline_deterministic",
    "internal_subrequests": 8,
    "action_loop_steps": 8,
    "product_tool_calls": 7,
    "external_network_calls": 0,
    "credential_reads": 0,
}, report
print(json.dumps(report, ensure_ascii=False, sort_keys=True))
PY
  kill -TERM "$probe_pid" >/dev/null 2>&1 || true
  wait "$probe_pid" >/dev/null 2>&1 || true
  # A force-terminated desktop may leave its exact bundled sidecar alive for
  # a short time. Phase 2 must acquire a fresh capability, so wait until the
  # owned listener is gone instead of racing it and receiving HTTP 403.
  stop_mvp_app

  env \
    WUSHEN_AGENT_RUNTIME_MODE="packaged-sidecar" \
    FORGECAD_DISABLE_PROVIDER_CONFIG="1" \
    FORGECAD_MVP_OFFLINE_ARM="1" \
    FORGECAD_CONCEPT_WORKER_ENABLED="0" \
    WUSHEN_LOCAL_WORKER_ENABLED="0" \
    WUSHEN_LIBRARY_ROOT="$probe_root" \
    FORGECAD_MVP_ARM_PACKAGED_PROBE="1" \
    FORGECAD_MVP_ARM_PACKAGED_RESUME="1" \
    FORGECAD_MVP_ARM_PACKAGED_RESUME_INPUT="$probe_report" \
    FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT="$resume_report" \
    "$APP_BINARY" >"$resume_log" 2>&1 &
  resume_pid=$!
  for _ in {1..90}; do
    if [[ -f "$resume_report" ]]; then
      break
    fi
    sleep 1
  done
  if [[ ! -f "$resume_report" ]]; then
    kill -TERM "$resume_pid" >/dev/null 2>&1 || true
    wait "$resume_pid" >/dev/null 2>&1 || true
    tail -120 "$resume_log" >&2 || true
    rm -rf "$probe_root"
    echo "packaged mechanical-arm MVP resume probe did not produce evidence" >&2
    return 1
  fi
  .venv/bin/python - "$probe_report" "$resume_report" <<'PY'
import json, sys
phase1 = json.load(open(sys.argv[1], encoding="utf-8"))
resume = json.load(open(sys.argv[2], encoding="utf-8"))
v3 = phase1["c110c"]["v3_asset_version_id"]
v4 = phase1["c110d"]["v4_asset_version_id"]
export = phase1["export"]
assert resume.get("schema_version") == "ForgeCADArmMvpPackagedResumeProof@3", resume
assert resume.get("status") == "pass", resume
assert resume.get("project_id") == phase1.get("project_id"), resume
assert resume.get("expected_asset_version_id") == v4, resume
assert resume.get("active_design", {}).get("asset_version_id") == v4, resume
assert resume.get("active_design", {}).get("snapshot_revision") == phase1.get("active_design", {}).get("snapshot_revision"), resume
assert resume.get("export", {}).get("asset_version_id") == v4, resume
assert resume.get("export", {}).get("glb_sha256") == export.get("glb_sha256"), resume
assert resume.get("export", {}).get("x_forgecad_glb_sha256") == export.get("glb_sha256"), resume
assert resume.get("export", {}).get("glb_byte_size") == export.get("glb_byte_size"), resume
assert resume.get("export", {}).get("triangle_count") == export.get("triangle_count"), resume
print(json.dumps({"phase1": phase1, "resume": resume}, ensure_ascii=False, sort_keys=True))
PY
  mkdir -p "$evidence_root"
  install -m 600 "$probe_report" "$evidence_root/packaged-protocol-proof.json"
  install -m 600 "$resume_report" "$evidence_root/packaged-resume-proof.json"
  kill -TERM "$resume_pid" >/dev/null 2>&1 || true
  wait "$resume_pid" >/dev/null 2>&1 || true
  # The desktop can exit before its supervised PyInstaller child has observed
  # the shutdown signal.  Finish by clearing only this exact bundle's owned
  # processes so a following packaged WebView run cannot inherit port 8000.
  stop_mvp_app
  rm -rf "$probe_root"
}

case "$MODE" in
  run)
    configure_legacy_runtime
    stop_local_app
    build_app
    launch_app
    ;;
  --verify|verify)
    configure_legacy_runtime
    stop_local_app
    build_app
    launch_app
    sleep 2
    pgrep -f "$APP_BINARY" >/dev/null
    wait_for_agent
    echo "local_tauri_app_running: true"
    echo "local_agent_healthy: true"
    echo "agent_mode: local-dev-python"
    ;;
  --mvp|mvp)
    configure_mvp_runtime 1
    stop_mvp_app
    build_mvp_app
    launch_mvp_app
    pgrep -f "$APP_BINARY" >/dev/null
    wait_for_packaged_agent
    echo "mvp_tauri_app_running: true"
    echo "mvp_agent_healthy: true"
    echo "agent_mode: packaged-sidecar"
    echo "state_owner: rust-core"
    echo "python_role: restricted_geometry_executor"
    echo "provider_calls: 0 (this launch performed no model turn)"
    ;;
  --mvp-verify|mvp-verify)
    configure_mvp_runtime
    verify_packaged_mvp
    ;;
  --mvp-arm-verify|mvp-arm-verify)
    configure_mvp_runtime 1
    verify_packaged_arm_mvp
    ;;
  --debug|debug)
    configure_legacy_runtime
    stop_local_app
    build_app
    lldb -- "$APP_BINARY"
    ;;
  --logs|logs)
    configure_legacy_runtime
    stop_local_app
    build_app
    launch_app
    /usr/bin/log stream --info --style compact --predicate 'process == "wushen-forge-desktop"'
    ;;
  --telemetry|telemetry)
    configure_legacy_runtime
    stop_local_app
    build_app
    launch_app
    /usr/bin/log stream --info --style compact --predicate 'process == "wushen-forge-desktop"'
    ;;
  *)
    echo "usage: $0 [run|--verify|--mvp|--mvp-verify|--mvp-arm-verify|--debug|--logs|--telemetry]" >&2
    exit 2
    ;;
esac
