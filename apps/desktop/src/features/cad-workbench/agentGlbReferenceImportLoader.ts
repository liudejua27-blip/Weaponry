import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet } from '../../shared/types'
import { arrayBufferToBase64 } from './cadWorkbenchPanelFileUtils'
import type { AgentBlockoutGlbPayload } from './agentBlockoutDisplayState'

type AgentGlbImportApi = Pick<ForgeApi, 'importAgentGlb'>

type ImportAgentGlbResponse = Awaited<ReturnType<AgentGlbImportApi['importAgentGlb']>>
type ImportedAgentAssetVersion = ImportAgentGlbResponse['asset_version']

type AgentGlbImportCallbacks = {
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  clearAgentAssetWorkspaceQuality: (projectId: string | null) => void
  hydrateBlockoutDisplay: (projectId: string | null, data: {
    glbBase64: AgentBlockoutGlbPayload
    glbKind: 'external_reference' | 'compiled_agent_preview_pbr' | 'compiled_agent_production_pbr' | null
    shapeProgram: null
    segmentation: {
      artifact_id: ImportedAgentAssetVersion['artifact_id']
      plan_id: ImportedAgentAssetVersion['plan_id']
      direction_id: ImportedAgentAssetVersion['direction_id']
      domain_pack_id: ImportedAgentAssetVersion['domain_pack_id']
      segmentation_status: 'candidate'
      parts: ImportedAgentAssetVersion['parts']
      assembly_graph: ImportedAgentAssetVersion['assembly_graph']
    }
  }) => number | null
  refreshActiveDesign: (projectId: string) => Promise<unknown>
}

type AgentGlbImportInput = {
  projectId: string
  file: File
  domainPackId: string | null
}

// The C111B production concept remains a bounded self-contained GLB while
// carrying 80k–150k triangles and five-channel 1K PBR. Keep the import gate
// lightweight, but do not reject the 34 MB golden asset before the server can
// perform its normal metadata-only inspection.
const IMPORT_GLB_SIZE_LIMIT_BYTES = 48 * 1024 * 1024

export async function importAgentGlbReference(
  api: AgentGlbImportApi,
  callbacks: AgentGlbImportCallbacks,
  input: AgentGlbImportInput,
): Promise<{ fileName: string; triangleCount: number; materialCount: number }> {
  const {
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    clearAgentAssetWorkspaceQuality,
    hydrateBlockoutDisplay,
    refreshActiveDesign,
  } = callbacks

  const { projectId, file, domainPackId } = input
  const fileName = file.name
  if (!fileName.toLowerCase().endsWith('.glb')) {
    throw new Error('当前只支持自包含的 .glb 文件。')
  }
  if (file.size > IMPORT_GLB_SIZE_LIMIT_BYTES) {
    throw new Error('GLB 超过 48 MB 轻量导入限制；请先在 DCC 软件中简化。')
  }

  const payload = arrayBufferToBase64(await file.arrayBuffer())
  const response = await api.importAgentGlb({
    client_request_id: `agent-glb-import-${Date.now()}`,
    project_id: projectId,
    // GLB evidence is category-open. Rust binds the real evidence lineage and
    // the later universal author turn chooses a representation capability;
    // never classify the object from a filename.
    domain_pack_id: domainPackId ?? 'pack_unclassified',
    file_name: fileName,
    glb_base64: payload,
    summary: `导入参考模型：${fileName}`,
  })
  const version = response.asset_version
  const segmentation = {
    artifact_id: version.artifact_id,
    plan_id: version.plan_id,
    direction_id: version.direction_id,
    domain_pack_id: version.domain_pack_id,
    segmentation_status: 'candidate',
    parts: version.parts,
    assembly_graph: version.assembly_graph,
  } as const

  setAgentAssetChangeSet(null)
  hydrateBlockoutDisplay(projectId, {
    glbBase64: payload,
    glbKind: 'external_reference',
    shapeProgram: null,
    segmentation,
  })
  setAgentCandidateSelectedPartId(null)
  clearAgentAssetWorkspaceQuality(projectId)
  await refreshActiveDesign(projectId)

  return {
    fileName,
    triangleCount: response.inspection.triangle_count,
    materialCount: response.inspection.material_count,
  }
}
