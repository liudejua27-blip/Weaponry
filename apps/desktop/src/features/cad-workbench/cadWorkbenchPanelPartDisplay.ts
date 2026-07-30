export type PartDisplayAction =
  | 'lock'
  | 'unlock'
  | 'hide'
  | 'show'
  | 'isolate'
  | 'clear_isolation'
  | 'show_all'

const PART_DISPLAY_ACTION_NOTE_BY_ACTION: Record<PartDisplayAction, string> = {
  lock: '已锁定这个部件；后续修改会被安全阻止。',
  unlock: '已解除部件锁定。',
  hide: '已隐藏这个部件；模型内容没有被删除。',
  show: '已显示这个部件。',
  isolate: '现在只显示这个部件。',
  clear_isolation: '已结束单独查看。',
  show_all: '已显示所有部件。',
}

export function partDisplayActionNote(action: PartDisplayAction): string {
  return PART_DISPLAY_ACTION_NOTE_BY_ACTION[action]
}
