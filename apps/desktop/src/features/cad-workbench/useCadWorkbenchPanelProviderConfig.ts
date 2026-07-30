import { useCallback, useEffect, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import type { AgentProviderCheckResponse } from '../../shared/types'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor'
import {
  getProviderConfig as getTauriProviderConfig,
  saveProviderConfig as saveTauriProviderConfig,
} from '../../shared/tauri/agentSupervisor'
import { buildProviderSaveFeedback, buildProviderTestFailureMessage } from './providerConfigurationPresentation.js'
import { providerCheckPresentation } from './providerConnectionPresentation.js'

type ProviderApi = {
  checkAgentProvider: (requestId: string) => Promise<AgentProviderCheckResponse>
  cancelAgentProviderCheck: (checkId: string) => Promise<unknown>
  cancelAgentTurn: (turnId: string, note: string) => Promise<unknown>
}

type UseCadWorkbenchPanelProviderConfigInput = {
  api: ProviderApi
  checkService: () => void | Promise<unknown>
  setAssistantNote: (note: string) => void
  errorText: (error: unknown) => string
}

type UseCadWorkbenchPanelProviderConfigResult = {
  providerConfig: ProviderConfigMetadata | null
  providerSetupOpen: boolean
  setProviderSetupOpen: Dispatch<SetStateAction<boolean>>
  providerBaseUrl: string
  setProviderBaseUrl: Dispatch<SetStateAction<string>>
  providerModel: string
  setProviderModel: Dispatch<SetStateAction<string>>
  providerApiKey: string
  setProviderApiKey: Dispatch<SetStateAction<string>>
  providerSaving: boolean
  activeProviderTurnId: string | null
  setActiveProviderTurnId: Dispatch<SetStateAction<string | null>>
  activeProviderCheckId: string | null
  saveProvider: () => Promise<void>
  testProvider: () => Promise<void>
  cancelActiveProviderTurn: () => Promise<void>
}

const DEFAULT_PROVIDER_BASE_URL = 'https://api.deepseek.com'
const DEFAULT_PROVIDER_MODEL = 'deepseek-v4-pro'

export function useCadWorkbenchPanelProviderConfig({
  api,
  checkService,
  setAssistantNote,
  errorText,
}: UseCadWorkbenchPanelProviderConfigInput): UseCadWorkbenchPanelProviderConfigResult {
  const [providerConfig, setProviderConfig] = useState<ProviderConfigMetadata | null>(null)
  const [providerSetupOpen, setProviderSetupOpen] = useState(false)
  const [providerBaseUrl, setProviderBaseUrl] = useState(DEFAULT_PROVIDER_BASE_URL)
  const [providerModel, setProviderModel] = useState(DEFAULT_PROVIDER_MODEL)
  const [providerApiKey, setProviderApiKey] = useState('')
  const [providerSaving, setProviderSaving] = useState(false)
  const [activeProviderTurnId, setActiveProviderTurnId] = useState<string | null>(null)
  const [activeProviderCheckId, setActiveProviderCheckId] = useState<string | null>(null)

  useEffect(() => {
    void getTauriProviderConfig()
      .then((config) => {
        if (!config) return
        setProviderConfig(config)
        setProviderBaseUrl(config.base_url)
        setProviderModel(config.model)
        if (config.failure_code === 'PROVIDER_CREDENTIAL_MIGRATION_REQUIRED') {
          setProviderSetupOpen(true)
          setAssistantNote('本机 Alpha 已改用不触发系统密码弹窗的私密凭据存储；请重新输入一次 DeepSeek API Key 完成迁移。')
        }
      })
      .catch((caught) => {
        setAssistantNote(`无法读取模型服务配置：${errorText(caught)}。当前不会假定 DeepSeek 已配置。`)
      })
  }, [setAssistantNote, errorText])

  const saveProvider = useCallback(async () => {
    if (!providerApiKey.trim()) {
      setAssistantNote('请填写 API Key；密钥只会保存到本机权限受限的私密文件，不会写入项目。')
      return
    }
    setProviderSaving(true)
    try {
      const saved = await saveTauriProviderConfig({
        base_url: providerBaseUrl,
        model: providerModel,
        api_key: providerApiKey,
      })
      setProviderConfig(saved)
      setProviderApiKey('')
      void checkService()
      const { note, shouldCloseSetup } = buildProviderSaveFeedback(saved)
      if (shouldCloseSetup) setProviderSetupOpen(false)
      setAssistantNote(note)
    } catch (caught) {
      setAssistantNote(`模型服务配置失败：${errorText(caught)}`)
    } finally {
      setProviderSaving(false)
    }
  }, [checkService, errorText, providerApiKey, providerBaseUrl, providerModel, setAssistantNote, setProviderSetupOpen, setProviderApiKey, setProviderConfig])

  const testProvider = useCallback(async () => {
    setProviderSaving(true)
    const checkId = `provider-check-${Date.now()}`
    setActiveProviderCheckId(checkId)
    try {
      const result = await api.checkAgentProvider(checkId)
      setAssistantNote(providerCheckPresentation(result))
    } catch (caught) {
      const detail = buildProviderTestFailureMessage(caught)
      setAssistantNote(`模型服务测试未完成：${detail}。不会静默切换为离线成功，已保存设计没有变化。`)
    } finally {
      setActiveProviderCheckId(null)
      setProviderSaving(false)
    }
  }, [api, setAssistantNote])

  const cancelActiveProviderTurn = useCallback(async () => {
    if (!activeProviderTurnId && !activeProviderCheckId) return
    try {
      if (activeProviderCheckId) {
        await api.cancelAgentProviderCheck(activeProviderCheckId)
      } else if (activeProviderTurnId) {
        await api.cancelAgentTurn(activeProviderTurnId, `agent-turn-cancel-${Date.now()}`)
      }
      setAssistantNote('正在取消本次模型请求；已保存资产不会变化。')
    } catch (caught) {
      setAssistantNote(`取消请求失败：${errorText(caught)}。请等待当前请求结束后再试。`)
    }
  }, [activeProviderCheckId, activeProviderTurnId, api, errorText, setAssistantNote])

  return {
    providerConfig,
    providerSetupOpen,
    setProviderSetupOpen,
    providerBaseUrl,
    setProviderBaseUrl,
    providerModel,
    setProviderModel,
    providerApiKey,
    setProviderApiKey,
    providerSaving,
    activeProviderTurnId,
    setActiveProviderTurnId,
    activeProviderCheckId,
    saveProvider,
    testProvider,
    cancelActiveProviderTurn,
  }
}
