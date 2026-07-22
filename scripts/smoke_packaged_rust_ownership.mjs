import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8')
const assert = (condition, message) => {
  if (!condition) throw new Error(message)
}

const k001Probe = read('apps/desktop/src/shared/api/packagedK001Probe.ts')
for (const required of [
  'new ForgeApiClient()',
  'createAgentThread(',
  'startAgentTurn(',
  'subscribeAgentThreadEvents(',
  'observeNativeThread(api, expected.thread_id, checkpointSequence)',
  "product_state_owner: 'rust_app_server'",
  'python_product_api_used: false',
  'requireRustProductStateOwner()',
  'nativeReplayProbeError(error)',
  'PROBE_NATIVE_ITEM_REPLAY_${cause}',
]) {
  assert(k001Probe.includes(required), `K001 packaged probe is missing ${required}`)
}

const forgeApi = read('apps/desktop/src/shared/api/forgeApi.ts')
for (const required of [
  'NATIVE_REPLAY_TRANSIENT_RECOVERY_ATTEMPTS = 1',
  'replayWithBoundedTransientRecovery',
  'isTransientNativeReplayFailure(error)',
  "error.error.data?.application_code === 'ADAPTER_UNAVAILABLE'",
  'throw firstTransientError ?? error',
]) {
  assert(forgeApi.includes(required), `native replay recovery contract is missing ${required}`)
}
const transport = read('apps/desktop/src/shared/api/appServerTransport.ts')
assert(
  transport.includes("if (isForgeApiContractError(error)) return false"),
  'closed ForgeApi contract failures must not reconnect and retry',
)
for (const forbidden of [
  'subscribeSse(',
  "jsonRequest('/api/v1/agent/threads'",
  'deterministic_kernel',
  'FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE',
  'FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE',
]) {
  assert(!k001Probe.includes(forbidden), `K001 packaged probe still depends on ${forbidden}`)
}

assert(
  k001Probe.includes("directions: [direction('direction_primary', 'Primary')]")
    && !k001Probe.includes("direction('direction_secondary'")
    && !k001Probe.includes("direction('direction_tertiary'"),
  'K001 packaged probe must obey the V003 single-synthesis Product Tool schema',
)

const startupScripts = [
  'scripts/smoke_packaged_sidecar_alpha.py',
  'scripts/smoke_packaged_tauri_alpha.py',
  'scripts/smoke_k002_packaged_tauri_native.py',
  'scripts/smoke_k003_packaged_tauri_native.py',
]
for (const relative of startupScripts) {
  const source = read(relative)
  for (const oracle of [
    'FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE',
    'FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE',
  ]) {
    const assignment = new RegExp(`environment\\s*\\[\\s*["']${oracle}["']\\s*\\]\\s*=`)
    assert(!assignment.test(source), `${relative} enables legacy writer oracle ${oracle}`)
  }
}

const k003PackagedSmoke = read('scripts/smoke_k003_packaged_tauri_native.py')
for (const required of [
  '"FORGECAD_K001_PACKAGED_PROBE": "1"',
  '"FORGECAD_K002_PACKAGED_PROBE": "1"',
  '"FORGECAD_K003_PACKAGED_PROBE": "1"',
  '_validate_k001_probe_report(initial_k001, "initial")',
  'validate_k002_probe_report(initial_k002, "initial")',
  '_validate_k003_probe_report(initial_k003, "initial")',
  '_assert_semantic_recovery(initial_k001_facts, restart_k001_facts)',
]) {
  assert(k003PackagedSmoke.includes(required), `K003 packaged gate aggregation is missing ${required}`)
}

const main = read('apps/desktop/src-tauri/src/main.rs')
const allowlistStart = main.indexOf('const SIDECAR_SAFE_INHERITED_ENVIRONMENT_KEYS')
const allowlistEnd = main.indexOf('];', allowlistStart)
assert(allowlistStart >= 0 && allowlistEnd > allowlistStart, 'sidecar environment allowlist is missing')
const sidecarAllowlist = main.slice(allowlistStart, allowlistEnd)
for (const forbidden of [
  'FORGECAD_K001_PACKAGED_PROBE',
  'FORGECAD_K002_PACKAGED_PROBE',
  'FORGECAD_K003_PACKAGED_PROBE',
  'FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE',
  'FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE',
]) {
  assert(!sidecarAllowlist.includes(forbidden), `formal sidecar allowlist forwards ${forbidden}`)
}
assert(
  main.includes('fn packaged_python_facet_never_receives_probe_or_legacy_writer_switches()'),
  'named Rust environment-boundary test is missing',
)
assert(
  main.includes('fn packaged_k001_probe_requires_native_replay_rust_product_and_one_glb()'),
  'named Rust K001 native ownership test is missing',
)

const packageJson = JSON.parse(read('package.json'))
const scripts = packageJson.scripts ?? {}
assert(
  scripts['k001:packaged-gate']?.includes('desktop:packaged-tauri-alpha-smoke'),
  'K001 packaged gate no longer runs the Rust-owned WebView probe',
)
assert(
  scripts['k002:packaged-gate']?.includes('desktop:k002-packaged-native-smoke'),
  'K002 packaged gate no longer runs the native lifecycle probe',
)
assert(
  scripts['k003:packaged-gate'] === 'npm run desktop:packaged-sidecar-build && npm run desktop:tauri-build-app && npm run desktop:k003-packaged-native-smoke',
  'K003 packaged gate must run one non-oracle build/app/native proof chain',
)

console.log(JSON.stringify({
  ok: true,
  k001_native_thread_turn: true,
  k001_native_item_replay: true,
  rust_product_compatibility: true,
  python_writer_oracles_enabled: false,
  k003_aggregates_k001_k002_k003_native_probes: true,
  packaged_gate_chain_non_oracle: true,
}))
