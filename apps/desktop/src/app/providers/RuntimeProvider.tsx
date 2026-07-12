import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import { forgeApi } from '../../shared/api/forgeApi'
import {
  getAgentSupervisorStatus,
  isTauriRuntime,
  restartAgentSupervisor,
  startAgentSupervisor,
  stopAgentSupervisor,
  type AgentSupervisorStatus,
} from '../../shared/tauri/agentSupervisor'

export type ServiceStatus = 'checking' | 'connected' | 'offline'

type RuntimeContextValue = {
  api: typeof forgeApi
  serviceStatus: ServiceStatus
  agentSupervisor: AgentSupervisorStatus | null
  agentActionError: string | null
  checkService: () => void
  startLocalAgent: () => void
  stopLocalAgent: () => void
  restartLocalAgent: () => void
}

const RuntimeContext = createContext<RuntimeContextValue | null>(null)

export function RuntimeProvider({ children }: { children: ReactNode }) {
  const api = useMemo(() => forgeApi, [])
  const [serviceStatus, setServiceStatus] = useState<ServiceStatus>('checking')
  const [agentSupervisor, setAgentSupervisor] = useState<AgentSupervisorStatus | null>(null)
  const [agentActionError, setAgentActionError] = useState<string | null>(null)

  const applyStartStatus = useCallback((status: AgentSupervisorStatus) => {
    setAgentSupervisor(status)
    if (status.state === 'error') {
      setAgentActionError(status.error ?? '本地 Agent 启动失败。')
      return false
    }
    return true
  }, [])

  const checkService = useCallback(() => {
    setServiceStatus('checking')
    getAgentSupervisorStatus()
      .then((status) => {
        setAgentSupervisor(status)
        if (status.available && status.baseUrl) api.setBaseUrl(status.baseUrl)
        return api.checkHealth()
      })
      .then(() => setServiceStatus('connected'))
      .catch((caught) => {
        setServiceStatus('offline')
        if (caught instanceof Error) setAgentActionError(caught.message)
      })
  }, [api])

  const startLocalAgent = useCallback(() => {
    setAgentActionError(null)
    startAgentSupervisor()
      .then((status) => {
        if (applyStartStatus(status)) checkService()
      })
      .catch((caught) => setAgentActionError(caught instanceof Error ? caught.message : String(caught)))
  }, [checkService])

  const stopLocalAgent = useCallback(() => {
    setAgentActionError(null)
    stopAgentSupervisor()
      .then((status) => {
        setAgentSupervisor(status)
        checkService()
      })
      .catch((caught) => setAgentActionError(caught instanceof Error ? caught.message : String(caught)))
  }, [checkService])

  const restartLocalAgent = useCallback(() => {
    setAgentActionError(null)
    restartAgentSupervisor()
      .then((status) => {
        if (applyStartStatus(status)) checkService()
      })
      .catch((caught) => setAgentActionError(caught instanceof Error ? caught.message : String(caught)))
  }, [checkService])

  useEffect(() => {
    if (!isTauriRuntime()) {
      checkService()
      return
    }
    // The desktop product opens on the CAD workbench, so a user should never
    // have to discover a separate Settings screen before their local Agent is
    // available. The Rust supervisor safely reuses an already healthy Agent.
    startAgentSupervisor()
      .then((status) => {
        if (applyStartStatus(status)) checkService()
      })
      .catch((caught) => {
        setAgentActionError(caught instanceof Error ? caught.message : String(caught))
        checkService()
      })
  }, [applyStartStatus, checkService])

  const value = useMemo<RuntimeContextValue>(() => ({
    api,
    serviceStatus,
    agentSupervisor,
    agentActionError,
    checkService,
    startLocalAgent,
    stopLocalAgent,
    restartLocalAgent,
  }), [
    agentActionError,
    agentSupervisor,
    api,
    checkService,
    restartLocalAgent,
    serviceStatus,
    startLocalAgent,
    stopLocalAgent,
  ])

  return <RuntimeContext.Provider value={value}>{children}</RuntimeContext.Provider>
}

export function useRuntime(): RuntimeContextValue {
  const value = useContext(RuntimeContext)
  if (value === null) throw new Error('useRuntime must be used inside RuntimeProvider.')
  return value
}
