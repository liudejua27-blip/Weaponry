export type SupervisorState =
  | 'unsupported'
  | 'unknown'
  | 'stopped'
  | 'running'
  | 'starting'
  | 'stopping'
  | 'wrong_service'
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
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
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
