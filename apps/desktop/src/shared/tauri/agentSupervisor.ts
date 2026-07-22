export type SupervisorState =
  | 'unsupported'
  | 'unknown'
  | 'stopped'
  | 'running'
  | 'starting'
  | 'stopping'
  | 'wrong_service'
  | 'capability_mismatch'
  | 'error'

export type AgentSupervisorStatus = {
  available: boolean
  baseUrl: string
  healthUrl: string
  state: SupervisorState
  endpoint: string
  running: boolean
  managedByDesktop: boolean
  pid?: number | null
  mode: string
  error?: string | null
}

export type ProviderConfigMetadata = {
  base_url: string
  model: string
  configured: boolean
  storage: string
  metadata_status: 'not_checked' | 'missing' | 'valid' | 'invalid' | 'unavailable'
  secret_status: 'not_checked' | 'missing' | 'available' | 'invalid' | 'unavailable'
  supervisor_status: 'not_checked' | 'running' | 'restart_failed' | 'unavailable' | 'mismatch'
  capability_status: 'offline' | 'ready' | 'mismatch' | 'unavailable'
  failure_code?: string | null
}

type TauriAgentServiceStatus = {
  base_url: string
  health_url: string
  endpoint: string
  running: boolean
  managed_by_desktop: boolean
  pid?: number | null
  mode: string
  state: SupervisorState
  last_error?: string | null
}

const UNSUPPORTED_STATUS: AgentSupervisorStatus = {
  available: false,
  baseUrl: 'http://127.0.0.1:8000',
  healthUrl: 'http://127.0.0.1:8000/api/health',
  state: 'unsupported',
  endpoint: 'http://127.0.0.1:8000/api/health',
  running: false,
  managedByDesktop: false,
  mode: 'browser-fallback',
}

export function isTauriRuntime(): boolean {
  if (typeof window === 'undefined') return false
  // Tauri's optional `withGlobalTauri` flag can leave `isTauri()` false even
  // though the native WebView IPC bridge is present. The `tauri:` protocol is
  // the stable packaged-runtime signal; browser Vite previews remain http(s).
  return isTauri() || window.location.protocol === 'tauri:'
}

export async function getAgentSupervisorStatus(): Promise<AgentSupervisorStatus> {
  if (!isTauriRuntime()) return UNSUPPORTED_STATUS
  return invokeSupervisor('agent_service_status', 'unknown')
}

export async function startAgentSupervisor(): Promise<AgentSupervisorStatus> {
  if (!isTauriRuntime()) return UNSUPPORTED_STATUS
  return invokeSupervisor('start_agent_service', 'starting')
}

export async function stopAgentSupervisor(): Promise<AgentSupervisorStatus> {
  if (!isTauriRuntime()) return UNSUPPORTED_STATUS
  return invokeSupervisor('stop_agent_service', 'stopping')
}

export async function restartAgentSupervisor(): Promise<AgentSupervisorStatus> {
  if (!isTauriRuntime()) return UNSUPPORTED_STATUS
  await stopAgentSupervisor()
  return startAgentSupervisor()
}

export async function getProviderConfig(): Promise<ProviderConfigMetadata | null> {
  if (!isTauriRuntime()) return null
  return invokeSupervisorCommand<ProviderConfigMetadata>('get_provider_config')
}

export async function saveProviderConfig(input: { base_url: string; model: string; api_key: string }): Promise<ProviderConfigMetadata> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不支持安全保存 API Key，请使用 secret file 启动 Agent。')
  return invokeSupervisorCommand<ProviderConfigMetadata>('save_provider_config', input)
}

export async function clearProviderConfig(): Promise<ProviderConfigMetadata> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不支持清除桌面 Provider 配置。')
  return invokeSupervisorCommand<ProviderConfigMetadata>('clear_provider_config')
}

async function invokeSupervisor(command: string, pendingState: SupervisorState): Promise<AgentSupervisorStatus> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<TauriAgentServiceStatus>(command)
    return normalizeStatus(status)
  } catch (caught) {
    return {
      ...UNSUPPORTED_STATUS,
      available: true,
      state: 'error',
      mode: pendingState,
      error: caught instanceof Error ? caught.message : String(caught),
    }
  }
}

async function invokeSupervisorCommand<T>(command: string, payload?: unknown): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, payload === undefined ? undefined : { request: payload })
}

function normalizeStatus(status: TauriAgentServiceStatus): AgentSupervisorStatus {
  return {
    available: true,
    baseUrl: status.base_url,
    healthUrl: status.health_url,
    state: status.state,
    endpoint: status.endpoint,
    running: status.running,
    managedByDesktop: status.managed_by_desktop,
    pid: status.pid,
    mode: status.mode,
    error: status.last_error,
  }
}
import { isTauri } from '@tauri-apps/api/core'
