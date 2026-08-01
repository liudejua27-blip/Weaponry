import { FileArrowDown } from '@phosphor-icons/react'
import type { RefObject } from 'react'
import type { AgentAssetRenderSet, AgentAssetRenderView, AgentAssetVersion } from '../../shared/types'

export type ExportDrawerProps = {
  activeAgentAssetVersion: AgentAssetVersion | null
  activeDesignIdle: boolean
  drawerRef?: RefObject<HTMLElement | null>
  onClose: () => void
  onDownloadAgentGlb: () => void
  renderSet: AgentAssetRenderSet | null
  renderLoading: boolean
  renderPackageLoading: boolean
  onRenderViews: () => void
  onDownloadRenderView: (view: AgentAssetRenderView) => void
  onDownloadRenderPackage: () => void
}

const RENDER_VIEW_LABELS: Record<AgentAssetRenderView['view_id'], string> = {
  iso: '透视',
  front: '正面',
  side: '侧面',
  top: '顶部',
  exploded_iso: '爆炸概念图',
}

export function ExportDrawer({
  activeAgentAssetVersion,
  activeDesignIdle,
  drawerRef,
  onClose,
  onDownloadAgentGlb,
  renderSet,
  renderLoading,
  renderPackageLoading,
  onRenderViews,
  onDownloadRenderView,
  onDownloadRenderPackage,
}: ExportDrawerProps) {
  return (
    <div className="workbench-overlay" role="presentation" onMouseDown={onClose}>
      <section ref={drawerRef} className="workbench-drawer export-drawer" role="dialog" aria-modal="true" aria-labelledby="forgecad-export-drawer-title" data-forgecad-drawer="export" tabIndex={-1} onMouseDown={(event) => event.stopPropagation()}>
        <div className="drawer-heading"><div><span id="forgecad-export-drawer-title">下载当前设计</span><strong>选择你现在需要的内容</strong><small>只使用当前已保存版本</small></div><button type="button" data-dialog-initial-focus="true" onClick={onClose} aria-label="关闭导出">×</button></div>
        {activeAgentAssetVersion ? (
          <>
            <div className="agent-export-summary" aria-label="当前设计可用下载">
              <strong>当前设计 v{activeAgentAssetVersion.version_no}</strong>
              <span>这是用于展示和继续编辑的概念级模型，不提供制造、性能或工程结论。</span>
            </div>
            <button type="button" className="drawer-primary-action" onClick={onDownloadAgentGlb} disabled={!activeAgentAssetVersion || !activeDesignIdle}>
              <FileArrowDown size={16} /> 下载 3D 模型 (GLB)
            </button>
          </>
        ) : (
          <div className="export-ready-summary" data-testid="agent-export-unavailable">
            <span><strong>当前没有可导出的设计</strong></span>
            <small>旧版格式暂不支持下载；请先创建或打开可编辑设计。</small>
          </div>
        )}
        {activeAgentAssetVersion && (
          <div className="agent-concept-views" aria-label="概念视图">
            <div className="agent-concept-views-heading">
              <div>
                <strong>概念视图</strong>
                <small>
                  {renderSet?.renderer_id === 'forgecad-workbench-pbr@1'
                    ? '来自当前工作台 GPU/PBR 渲染器，与用户看到的模型同源；不会创建或修改模型版本。'
                    : '用于确认外观方向；旧兼容视图不会创建或修改模型版本。'}
                </small>
              </div>
              <button type="button" className="drawer-secondary-action" onClick={onRenderViews} disabled={!activeAgentAssetVersion || !activeDesignIdle || renderLoading}>
                {renderLoading ? '生成中…' : renderSet ? '重新生成' : '生成概念图'}
              </button>
            </div>
            {renderSet && (
              <div className="agent-concept-view-grid">
                {renderSet.views.map((view) => (
                  <button type="button" className="agent-concept-view-card" key={view.view_id} onClick={() => onDownloadRenderView(view)} title={`下载${view.view_id}视图 PNG`}>
                    <img src={`data:image/png;base64,${view.png_base64}`} alt={`${view.view_id}视图`} />
                    <span>{RENDER_VIEW_LABELS[view.view_id]} · 下载 PNG</span>
                    {view.presentation_mode === 'exploded' ? <small>透明背景 · 仅展示部件层级</small> : null}
                  </button>
                ))}
              </div>
            )}
            {renderSet && !renderSet.exploded_view_available && (
              <p className="agent-exploded-view-note">该模型没有可安全一一对应的部件几何组，因此未生成爆炸概念图。</p>
            )}
            {renderSet?.renderer_id !== 'forgecad-workbench-pbr@1' && renderSet && (
              <button type="button" className="drawer-secondary-action agent-render-package-action" onClick={onDownloadRenderPackage} disabled={renderPackageLoading || renderLoading}>
                {renderPackageLoading ? '正在准备概念图包…' : '下载概念图包'}
              </button>
            )}
            {renderSet?.renderer_id === 'forgecad-workbench-pbr@1' && (
              <p className="agent-exploded-view-note">当前 GPU/PBR 结果支持逐张下载；概念图包仍保留给旧兼容渲染路径。</p>
            )}
          </div>
        )}
        <button type="button" className="drawer-secondary-action" onClick={onClose}>取消</button>
      </section>
    </div>
  )
}
