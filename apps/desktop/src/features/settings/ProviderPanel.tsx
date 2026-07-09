import { useEffect, useState } from 'react'
import type { ForgeApiClient } from '../../shared/api/forgeApi'
import type { AgentSupervisorStatus } from '../../shared/tauri/agentSupervisor'
import type { ProviderSettings } from '../../shared/types'

type Props = {
  api: ForgeApiClient
  serviceStatus: 'checking' | 'connected' | 'offline'
  agentSupervisor: AgentSupervisorStatus | null
  agentActionError: string | null
  onRetryHealth: () => void
  onStartAgent: () => void
  onStopAgent: () => void
  onRestartAgent: () => void
}

export function ProviderPanel({
  api,
  serviceStatus,
  agentSupervisor,
  agentActionError,
  onRetryHealth,
  onStartAgent,
  onStopAgent,
  onRestartAgent,
}: Props) {
  const [providers, setProviders] = useState<ProviderSettings[]>([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.listProviders().then(setProviders).catch((caught) => {
      setProviders([])
      setError(caught instanceof Error ? caught.message : 'Provider 读取失败')
    })
  }, [api])

  return (
    <section className="panel-section">
      <h1>Provider 设置</h1>
      <div className="settings-row">
        <span>Agent API</span>
        <code>{api.getBaseUrl()}</code>
        <button onClick={onRetryHealth}>测试连接</button>
        <strong>{serviceStatus}</strong>
      </div>
      <div className="settings-row">
        <span>Desktop Supervisor</span>
        <code>{agentSupervisor ? agentSupervisor.mode : 'unknown'}</code>
        <span>{agentSupervisor ? agentSupervisor.state : 'unknown'}</span>
        <button onClick={onStartAgent} disabled={!agentSupervisor?.available}>启动</button>
        <button onClick={onStopAgent} disabled={!agentSupervisor?.managedByDesktop}>停止</button>
        <button onClick={onRestartAgent} disabled={!agentSupervisor?.available}>重启</button>
        {agentSupervisor?.pid && <code>pid {agentSupervisor.pid}</code>}
      </div>
      {agentSupervisor?.healthUrl && (
        <div className="settings-row">
          <span>Health Endpoint</span>
          <code>{agentSupervisor.healthUrl}</code>
          <strong>{agentSupervisor.running ? 'healthy' : 'not healthy'}</strong>
        </div>
      )}
      {agentSupervisor?.error && <p className="error">{agentSupervisor.error}</p>}
      {agentActionError && <p className="error">{agentActionError}</p>}
      {error && <p className="error">{error}</p>}
      <ul className="asset-list">
        {providers.map((provider) => (
          <li key={provider.provider_id}>
            <strong>{provider.display_name}</strong>
            <span>{provider.kind} · {provider.type}</span>
            <small>{provider.enabled ? 'enabled' : 'disabled'} · {provider.status} · secret {provider.has_secret ? 'set' : 'not set'}</small>
            {provider.base_url && <code>{provider.base_url}</code>}
            <small>{provider.updated_at ? new Date(provider.updated_at).toLocaleString() : 'not updated yet'}</small>
          </li>
        ))}
      </ul>
    </section>
  )
}
