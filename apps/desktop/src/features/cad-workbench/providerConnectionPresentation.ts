import type { AgentProviderCheckResponse } from '../../shared/types.js'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor.js'

export type ProviderConnectionPresentation = {
  ready: boolean
  canTest: boolean
  tone: 'ready' | 'offline' | 'error'
  label: string
}

export function providerConfigPresentation(config: ProviderConfigMetadata | null): ProviderConnectionPresentation {
  const ready = config?.configured === true
    && config.metadata_status === 'valid'
    && config.secret_status === 'available'
    && config.supervisor_status === 'running'
    && config.capability_status === 'ready'
  if (ready) {
    return { ready: true, canTest: true, tone: 'ready', label: '模型服务已配置，等待显式调用' }
  }
  if (config?.configured === true
    && config.metadata_status === 'valid'
    && config.secret_status === 'not_checked'
    && config.supervisor_status === 'running'
    && config.capability_status === 'ready') {
    return {
      ready: false,
      canTest: true,
      tone: 'offline',
      label: '模型服务已配置；发送请求或测试连接时再由 macOS 钥匙串授权',
    }
  }
  if (config?.failure_code) {
    return { ready: false, canTest: false, tone: 'error', label: `模型服务未就绪 · ${config.failure_code}` }
  }
  return { ready: false, canTest: false, tone: 'offline', label: '当前使用本机离线规划 · 未调用 DeepSeek' }
}

export function providerCheckPresentation(result: AgentProviderCheckResponse): string {
  const trace = result.execution_trace?.at(-1)
  if (result.status === 'ready') {
    return `模型服务连接成功；真实网络请求已完成并通过结构化校验（${trace?.latency_ms ?? 0} ms）。`
  }
  if (result.status === 'offline' || result.status === 'not_configured') {
    return '当前没有调用 DeepSeek：Provider metadata、Keychain 或 Agent capability 尚未就绪。'
  }
  const code = trace?.error_code ?? result.connection.failure_code ?? 'PROVIDER_FAILED'
  return `模型服务测试失败：${code}。network_call_made=${result.network_call_made ? 'true' : 'false'}；已保存设计没有变化。`
}

export function isProviderExecutionError(code: string): boolean {
  return code.startsWith('PROVIDER_') || code.startsWith('DEEPSEEK_')
}

export function allowsLegacyPlannerFallback(code: string): boolean {
  return !isProviderExecutionError(code)
}
