import type { JobDetail } from '../shared/types'

export type DesktopJobNotification = {
  id: string
  jobId: string
  jobType: string
  status: string
  weaponId: string
  message: string
  createdAt: string
}

export function readLastJobId(): string | null {
  try {
    return localStorage.getItem('wushen.recentJobId')
  } catch {
    return null
  }
}

export function rememberLastJobId(jobId: string): void {
  try {
    localStorage.setItem('wushen.recentJobId', jobId)
  } catch {
    // Persistence failure must not block the active desktop workflow.
  }
}

export function readRecentJobIds(): string[] {
  try {
    const raw = localStorage.getItem('wushen.recentJobIds')
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === 'string').slice(0, 20)
      : []
  } catch {
    return []
  }
}

export function rememberRecentJob(jobId: string): string[] {
  try {
    const current = readRecentJobIds()
    const next = [jobId, ...current.filter((item) => item !== jobId)].slice(0, 20)
    localStorage.setItem('wushen.recentJobIds', JSON.stringify(next))
    return next
  } catch {
    const next = [jobId]
    localStorage.setItem('wushen.recentJobIds', JSON.stringify(next))
    return next
  }
}

export function readDesktopNotifications(): DesktopJobNotification[] {
  try {
    const raw = localStorage.getItem('wushen.desktopNotifications')
    const parsed = raw ? JSON.parse(raw) : []
    if (!Array.isArray(parsed)) return []
    return parsed.filter((item): item is DesktopJobNotification => (
      isRecord(item)
      && typeof item.id === 'string'
      && typeof item.jobId === 'string'
      && typeof item.jobType === 'string'
      && typeof item.status === 'string'
      && typeof item.weaponId === 'string'
      && typeof item.message === 'string'
      && typeof item.createdAt === 'string'
    )).slice(0, 20)
  } catch {
    return []
  }
}

export function rememberDesktopNotification(job: JobDetail): DesktopJobNotification[] {
  const notification: DesktopJobNotification = {
    id: `${job.job_id}:${job.status}`,
    jobId: job.job_id,
    jobType: job.type,
    status: job.status,
    weaponId: job.weapon_id,
    message: job.status === 'succeeded'
      ? `${job.type} 已完成，可回到任务中心查看交接产物。`
      : `${job.type} ${job.status}，需要查看任务中心。`,
    createdAt: new Date().toISOString(),
  }
  const current = readDesktopNotifications()
  const next = [notification, ...current.filter((item) => item.id !== notification.id)].slice(0, 20)
  localStorage.setItem('wushen.desktopNotifications', JSON.stringify(next))
  maybeSendBrowserNotification(notification)
  return next
}

export function isTerminalJobStatus(status: string): boolean {
  return ['failed', 'cancelled', 'partial_succeeded', 'succeeded'].includes(status)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function maybeSendBrowserNotification(notification: DesktopJobNotification): void {
  if (typeof window === 'undefined' || !('Notification' in window)) return
  if (Notification.permission !== 'granted') return
  try {
    new Notification('武神 Forge 任务更新', {
      body: `${notification.jobId} · ${notification.message}`,
      tag: notification.id,
      silent: true,
    })
  } catch {
    // Desktop notification failures must not break local task recovery.
  }
}
