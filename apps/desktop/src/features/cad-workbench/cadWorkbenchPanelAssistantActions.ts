export const ASSISTANT_EMPTY_INSTRUCTION_NOTICE = '请先在输入框描述想生成的 3D 概念，再发送给 Agent。'

export type AssistantMode = 'brief' | 'change'
type AssistantSubmitter = () => Promise<void> | void

export function trimAssistantInstruction(rawInstruction: string): string | null {
  const instruction = rawInstruction.trim()
  return instruction.length === 0 ? null : instruction
}

export function resolveAssistantActionRunner(
  mode: AssistantMode,
  submitBriefInstruction: AssistantSubmitter,
  previewChangeInstruction: AssistantSubmitter,
): AssistantSubmitter {
  return mode === 'brief'
    ? submitBriefInstruction
    : previewChangeInstruction
}
