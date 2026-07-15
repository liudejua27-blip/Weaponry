import type { AgentProviderCheckResponse } from '../../shared/types.js'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor.js'
import {
  allowsLegacyPlannerFallback,
  providerCheckPresentation,
  providerConfigPresentation,
} from './providerConnectionPresentation.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const readyConfig: ProviderConfigMetadata = {
  base_url: 'https://api.deepseek.com',
  model: 'deepseek-v4-pro',
  configured: true,
  storage: 'macos-keychain',
  metadata_status: 'valid',
  secret_status: 'available',
  supervisor_status: 'running',
  capability_status: 'ready',
  failure_code: null,
}

function failedCheck(networkCallMade: boolean): AgentProviderCheckResponse {
  return {
    status: 'failed',
    provider_id: 'openai_compatible_mechanical_planner',
    model: 'deepseek-v4-pro',
    message: 'auth failed',
    network_call_made: networkCallMade,
    connection: {
      schema_version: 'ProviderConnectionState@1',
      status: 'failed',
      provider_id: 'openai_compatible_mechanical_planner',
      configured: true,
      metadata_status: 'valid',
      secret_status: 'available',
      supervisor_status: 'not_checked',
      capability_status: 'ready',
      network_call_made: networkCallMade,
      failure_code: 'DEEPSEEK_AUTH_FAILED',
      message: 'auth failed',
    },
    execution_trace: [{
      schema_version: 'ProviderExecutionTrace@1',
      trace_id: 'ptrace_0123456789abcdef0123456789abcdef',
      phase: 'failed',
      provider_id: 'openai_compatible_mechanical_planner',
      attempt: 1,
      network_call_made: networkCallMade,
      latency_ms: 8,
      error_code: 'DEEPSEEK_AUTH_FAILED',
      message: 'auth failed',
    }],
  }
}

export function runProviderConnectionPresentationSmoke(): void {
  assert(providerConfigPresentation(null).label.includes('未调用 DeepSeek'), 'missing metadata must be explicit offline state')
  assert(providerConfigPresentation(readyConfig).ready, 'metadata + Keychain + supervisor + capability must all be ready')
  const restartFailed = providerConfigPresentation({ ...readyConfig, supervisor_status: 'restart_failed', failure_code: 'PROVIDER_SUPERVISOR_RESTART_FAILED' })
  assert(!restartFailed.ready && restartFailed.label.includes('PROVIDER_SUPERVISOR_RESTART_FAILED'), 'saved config with failed restart must not appear ready')
  const failed = providerCheckPresentation(failedCheck(true))
  assert(failed.includes('DEEPSEEK_AUTH_FAILED') && failed.includes('network_call_made=true'), 'DeepSeek failure and actual network marker must remain visible')
  assert(!allowsLegacyPlannerFallback('DEEPSEEK_AUTH_FAILED'), 'real Provider failure must never fall back to legacy success')
  assert(!allowsLegacyPlannerFallback('PROVIDER_EMPTY_CONTENT'), 'structured output failure must never fall back to legacy success')
  assert(allowsLegacyPlannerFallback('HTTP_404_OLD_AGENT'), 'only a non-Provider compatibility failure may use the existing legacy path')
  const serialized = JSON.stringify({ config: readyConfig, check: failedCheck(true) })
  assert(!serialized.includes('api_key') && !serialized.includes('reasoning_content'), 'presentation contracts must remain redaction-safe')
}
