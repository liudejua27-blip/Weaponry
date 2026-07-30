import { useCallback } from 'react'

import type { ActiveDesignErrorState, ForgeApi } from '../../shared/api/forgeApi'
import type { ActiveDesignSnapshot } from '../../shared/types'
import type { CameraView, LightPreset } from './cadWorkbenchPanelTools'
import { refreshActiveDesign as refreshActiveDesignRequest } from './refreshActiveDesignLoader.js'

type ActiveDesignError = ActiveDesignErrorState | null

type RefreshActiveDesignCallbackArgs = Parameters<typeof refreshActiveDesignRequest>[1]

type UseCadWorkbenchPanelActiveDesignSyncInput = {
  api: ForgeApi
  cameraView: CameraView
  lightPreset: LightPreset
  activeDesignSnapshot: ActiveDesignSnapshot | null
  snapshotEtag: string | null
  setCameraView: (cameraView: CameraView) => void
  setLightPreset: (lightPreset: LightPreset) => void
  setAssistantNote: (note: string) => void
  callbacks: RefreshActiveDesignCallbackArgs
}

type UseCadWorkbenchPanelActiveDesignSyncResult = {
  refreshActiveDesign: (projectId: string) => Promise<void>
  updateRenderPreset: (next: { cameraView?: CameraView; lightPreset?: LightPreset }) => Promise<void>
}

export function useCadWorkbenchPanelActiveDesignSync({
  api,
  cameraView,
  lightPreset,
  activeDesignSnapshot,
  snapshotEtag,
  setCameraView,
  setLightPreset,
  callbacks,
  setAssistantNote,
}: UseCadWorkbenchPanelActiveDesignSyncInput): UseCadWorkbenchPanelActiveDesignSyncResult {
  const refreshActiveDesign = useCallback(async (projectId: string) => {
    await refreshActiveDesignRequest(api, callbacks, projectId)
  }, [api, callbacks])

  const updateRenderPreset = useCallback(async (next: { cameraView?: CameraView; lightPreset?: LightPreset }) => {
    const nextCameraView = next.cameraView ?? cameraView
    const nextLightPreset = next.lightPreset ?? lightPreset
    setCameraView(nextCameraView)
    setLightPreset(nextLightPreset)
    const snapshot = activeDesignSnapshot
    if (!snapshot || snapshot.active_design.source !== 'agent_asset' || !snapshotEtag) return
    const requestId = callbacks.startActiveDesignRequest('setting_render_preset')
    try {
      const response = await api.setActiveDesignRenderPreset(
        snapshot.project_id,
        {
          client_request_id: `render-preset-${requestId}`,
          snapshot_revision: snapshot.revision,
          camera_view: nextCameraView,
          light_preset: nextLightPreset,
        },
        { ifMatch: snapshotEtag },
      )
      callbacks.receiveActiveDesignSnapshot(snapshot.project_id, requestId, response)
    } catch (caught) {
      const error: ActiveDesignError = callbacks.failActiveDesignRequest(requestId, caught)
      if (!error) return
      if (error.shouldReloadSnapshot) void refreshActiveDesign(snapshot.project_id)
      setAssistantNote(error.message)
    }
  }, [
    api,
    callbacks,
    cameraView,
    lightPreset,
    activeDesignSnapshot,
    refreshActiveDesign,
    setAssistantNote,
    setCameraView,
    setLightPreset,
    snapshotEtag,
  ])

  return {
    refreshActiveDesign,
    updateRenderPreset,
  }
}
