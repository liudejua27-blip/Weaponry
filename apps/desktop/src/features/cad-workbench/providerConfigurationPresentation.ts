import { ForgeApiError } from '../../shared/api/forgeApi'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor'

type SavedProviderConfigText = {
  note: string
  shouldCloseSetup: boolean
}

export function buildProviderSaveFeedback(config: ProviderConfigMetadata): SavedProviderConfigText {
  if (config.metadata_status !== 'valid' || config.secret_status !== 'available') {
    return {
      shouldCloseSetup: false,
      note: `配置尚未启用：${config.failure_code ?? 'Provider metadata 或私密凭据未通过验证'}。没有发起 DeepSeek 请求。`,
    }
  }
  if (config.supervisor_status !== 'running' || config.capability_status !== 'ready') {
    return {
      shouldCloseSetup: false,
      note: `密钥已安全保存，但 Agent 尚未载入新配置：${config.failure_code ?? '本地 capability 不匹配'}。没有发起 DeepSeek 请求，请先修复服务状态。`,
    }
  }
  return {
    shouldCloseSetup: true,
    note: '模型服务配置、私密凭据、Agent 重启和本地 capability 均已验证；尚未发起收费请求，可点击“测试连接”。',
  }
}

export function buildProviderTestFailureMessage(caught: unknown): string {
  if (caught instanceof ForgeApiError) {
    return `${caught.message}（${caught.code}，network_call_made=${caught.details?.network_call_made === true ? 'true' : 'unknown'}）`
  }
  return caught instanceof Error ? caught.message : '模型服务测试失败'
}
