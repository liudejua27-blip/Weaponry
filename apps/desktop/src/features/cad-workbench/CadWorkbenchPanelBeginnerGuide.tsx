import { memo } from 'react'

type CadWorkbenchPanelBeginnerGuideProps = {
  showComposerAdvancedActions: boolean
  onToggleAdvancedActions: () => void
  onFocusComposerInput: () => void
  isVisible: boolean
}

export const CadWorkbenchPanelBeginnerGuide = memo(function CadWorkbenchPanelBeginnerGuide({
  showComposerAdvancedActions,
  onToggleAdvancedActions,
  onFocusComposerInput,
  isVisible,
}: CadWorkbenchPanelBeginnerGuideProps) {
  if (!isVisible) {
    return null
  }

  return (
    <>
      <p className="f026-beginner-tip" role="note">
        零基础优先：先写一句想法生成预览，再用下方按钮补充细节；需要进阶功能再开“进阶模式”。
      </p>
      {!showComposerAdvancedActions ? (
        <section className="f026-beginner-guide" aria-label="新手上手步骤">
          <h3>新手起步</h3>
          <p>先选一个示例描述或直接写一句你想要的效果，再点“开始生成”。</p>
          <ol>
            <li>先点一个示例需求，系统会把它放进输入框。</li>
            <li>点击“开始生成”，看结果后再细化。</li>
            <li>需要风格、材质或贴花，直接使用页面下方主工具栏。</li>
          </ol>
          <div className="f026-beginner-actions">
            <button
              type="button"
              className="f026-composer-advanced-toggle"
              onClick={onFocusComposerInput}
            >
              开始创作
            </button>
            <button
              type="button"
              className="f026-composer-advanced-toggle"
              aria-label="打开进阶模式"
              onClick={onToggleAdvancedActions}
            >
              打开进阶模式
            </button>
          </div>
        </section>
      ) : (
        <button
          type="button"
          className="f026-composer-advanced-toggle"
          onClick={onToggleAdvancedActions}
        >
          回到新手模式
        </button>
      )}
    </>
  )
}, (prev, next) => prev.showComposerAdvancedActions === next.showComposerAdvancedActions
  && prev.isVisible === next.isVisible
  && prev.onFocusComposerInput === next.onFocusComposerInput
  && prev.onToggleAdvancedActions === next.onToggleAdvancedActions)
