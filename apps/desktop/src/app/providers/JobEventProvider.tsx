import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import type { JobEvent } from '../../shared/types'
import { useRuntime } from './RuntimeProvider'

export type JobStreamStatus = 'idle' | 'connecting' | 'live' | 'reconnecting' | 'closed'

type SubscribeOptions = {
  onTerminal?: (event: JobEvent) => void
  onStreamError?: (error: { code: string; message: string }) => void
}

type JobEventContextValue = {
  events: JobEvent[]
  streamStatus: JobStreamStatus
  replaceEvents: (events: JobEvent[]) => void
  resetEvents: () => void
  subscribe: (jobId: string, options?: SubscribeOptions) => () => void
}

const JobEventContext = createContext<JobEventContextValue | null>(null)

export function JobEventProvider({ children }: { children: ReactNode }) {
  const { api } = useRuntime()
  const [events, setEvents] = useState<JobEvent[]>([])
  const eventsRef = useRef<JobEvent[]>([])
  const [streamStatus, setStreamStatus] = useState<JobStreamStatus>('idle')

  useEffect(() => {
    eventsRef.current = events
  }, [events])

  const replaceEvents = useCallback((nextEvents: JobEvent[]) => {
    setEvents([...nextEvents].sort(compareJobEvents))
  }, [])

  const resetEvents = useCallback(() => {
    setEvents([])
    setStreamStatus('idle')
  }, [])

  const mergeEvents = useCallback((incoming: JobEvent[]) => {
    setEvents((current) => {
      const byId = new Map(current.map((item) => [item.id, item]))
      for (const event of incoming) byId.set(event.id, event)
      return Array.from(byId.values()).sort(compareJobEvents)
    })
  }, [])

  const subscribe = useCallback((jobId: string, options: SubscribeOptions = {}) => {
    setStreamStatus('connecting')
    const after = eventsRef.current.filter((event) => event.job_id === jobId).at(-1)?.id
    return api.subscribeJobEvents(jobId, {
      onOpen: () => setStreamStatus('live'),
      onEvent: (event) => {
        if (event.job_id !== jobId) return
        mergeEvents([event])
        if (isTerminalJobEvent(event)) {
          setStreamStatus('closed')
          options.onTerminal?.(event)
        }
      },
      onStreamError: (error) => {
        setStreamStatus('closed')
        options.onStreamError?.(error)
      },
      onError: () => setStreamStatus('reconnecting'),
    }, after)
  }, [api, mergeEvents])

  const value = useMemo<JobEventContextValue>(() => ({
    events,
    streamStatus,
    replaceEvents,
    resetEvents,
    subscribe,
  }), [events, replaceEvents, resetEvents, streamStatus, subscribe])

  return <JobEventContext.Provider value={value}>{children}</JobEventContext.Provider>
}

export function useJobEvents(): JobEventContextValue {
  const value = useContext(JobEventContext)
  if (value === null) throw new Error('useJobEvents must be used inside JobEventProvider.')
  return value
}

function compareJobEvents(a: JobEvent, b: JobEvent): number {
  if (a.seq !== b.seq) return a.seq - b.seq
  const aSeq = parseEventSequence(a.id)
  const bSeq = parseEventSequence(b.id)
  if (aSeq !== null && bSeq !== null && aSeq !== bSeq) return aSeq - bSeq
  return (a.created_at ?? '').localeCompare(b.created_at ?? '')
}

function isTerminalJobEvent(event: JobEvent): boolean {
  if (['failed', 'cancelled', 'partial_succeeded'].includes(event.status)) return true
  return event.status === 'succeeded' && event.step === 'finalize_job'
}

function parseEventSequence(eventId: string): number | null {
  const match = eventId.match(/_(\d+)$/)
  return match ? Number(match[1]) : null
}
