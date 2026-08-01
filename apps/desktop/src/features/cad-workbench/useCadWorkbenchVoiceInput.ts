import { useCallback, useEffect, useRef, useState } from 'react'

type SpeechResult = {
  0?: { transcript?: string }
}

type SpeechResultEvent = Event & {
  results: {
    length: number
    [index: number]: SpeechResult | undefined
  }
}

type SpeechRecognitionLike = {
  lang: string
  interimResults: boolean
  maxAlternatives: number
  onresult: ((event: SpeechResultEvent) => void) | null
  onerror: ((event: Event) => void) | null
  onend: (() => void) | null
  start: () => void
  stop: () => void
}

type SpeechRecognitionConstructor = new () => SpeechRecognitionLike

type SpeechRecognitionWindow = Window & {
  SpeechRecognition?: SpeechRecognitionConstructor
  webkitSpeechRecognition?: SpeechRecognitionConstructor
}

type UseCadWorkbenchVoiceInputInput = {
  onTranscript: (transcript: string) => void
  onNotice: (message: string) => void
}

export function useCadWorkbenchVoiceInput({
  onTranscript,
  onNotice,
}: UseCadWorkbenchVoiceInputInput) {
  const recognitionRef = useRef<SpeechRecognitionLike | null>(null)
  const [isListening, setIsListening] = useState(false)

  const stop = useCallback(() => {
    recognitionRef.current?.stop()
    recognitionRef.current = null
    setIsListening(false)
  }, [])

  const toggle = useCallback(() => {
    if (isListening) {
      stop()
      return
    }
    const speechWindow = window as SpeechRecognitionWindow
    const Recognition = speechWindow.SpeechRecognition ?? speechWindow.webkitSpeechRecognition
    if (!Recognition) {
      onNotice('当前运行环境不支持语音输入，请直接键入设计需求。')
      return
    }

    const recognition = new Recognition()
    recognition.lang = 'zh-CN'
    recognition.interimResults = false
    recognition.maxAlternatives = 1
    recognition.onresult = (event) => {
      const transcript = Array.from({ length: event.results.length }, (_, index) => (
        event.results[index]?.[0]?.transcript ?? ''
      )).join('').trim()
      if (transcript) {
        onTranscript(transcript)
        onNotice('语音内容已填入需求输入框，可以继续编辑后发送。')
      }
    }
    recognition.onerror = () => {
      onNotice('语音输入没有完成，请检查麦克风权限或直接键入需求。')
      setIsListening(false)
      recognitionRef.current = null
    }
    recognition.onend = () => {
      if (recognitionRef.current !== recognition) return
      recognitionRef.current = null
      setIsListening(false)
    }
    recognitionRef.current = recognition
    setIsListening(true)
    onNotice('正在听取你的设计描述，说完后会自动填入输入框。')
    recognition.start()
  }, [isListening, onNotice, onTranscript, stop])

  useEffect(() => stop, [stop])

  return { isListening, toggle }
}
