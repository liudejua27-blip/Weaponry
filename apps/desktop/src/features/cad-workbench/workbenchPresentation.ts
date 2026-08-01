/**
 * Presentation-only state for the CAD shell. These helpers deliberately do
 * not own a Project, Version, Snapshot, ChangeSet, Quality or Export value.
 */
export function hasVisibleWorkbenchError(
  latestAgentRequestId: number,
  conceptError: string | null | undefined,
  activeDesignError: string | null | undefined,
): boolean {
  return latestAgentRequestId > 0 && Boolean(conceptError || activeDesignError)
}

export function shouldShowWorkbenchHistory(input: {
  hasAgentAsset: boolean
  hasCandidateResult: boolean
  hasHistoryPreview: boolean
}): boolean {
  return input.hasAgentAsset || input.hasCandidateResult || input.hasHistoryPreview
}

export function workbenchLayoutClassName(input: {
  dockState: 'docked' | 'focus'
  compact: boolean
  sidebarOpen: boolean
  assistantOpen: boolean
  sidebarCollapsed: boolean
  assistantCollapsed: boolean
  showHistory: boolean
}): string {
  return [
    'cad-layout',
    'f026-layout',
    input.dockState === 'focus' ? 'is-viewport-focus' : '',
    input.compact ? 'is-mobile' : '',
    input.sidebarOpen ? 'is-sidebar-open' : '',
    input.assistantOpen ? 'is-assistant-open' : '',
    input.sidebarCollapsed ? 'is-sidebar-collapsed' : '',
    input.assistantCollapsed ? 'is-assistant-collapsed' : '',
    input.showHistory ? 'has-history' : 'without-history',
  ].filter(Boolean).join(' ')
}
