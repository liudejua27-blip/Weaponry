export const WEAPONRY_FRAME_BUDGET_SCHEMA = 'WeaponryThreeJsFrameBudget@1' as const
export const WEAPONRY_FRAME_BUDGET_SNAPSHOT_SCHEMA = 'WeaponryThreeJsFrameBudgetSnapshot@1' as const

export interface WeaponryFrameBudgetConfig {
  readonly target_fps?: number
  readonly sample_capacity?: number
  readonly minimum_samples?: number
}

export interface WeaponryFrameTimingSample {
  readonly simulation_ms: number
  readonly animation_ms: number
  readonly physics_ms: number
  readonly render_ms: number
  readonly total_ms: number
}

export interface WeaponryFrameBudgetSnapshot {
  readonly schema_version: typeof WEAPONRY_FRAME_BUDGET_SNAPSHOT_SCHEMA
  readonly budget_schema_version: typeof WEAPONRY_FRAME_BUDGET_SCHEMA
  readonly status: 'WARMING_UP' | 'MEASURED_WITHIN_BUDGET' | 'MEASURED_OVER_BUDGET'
  readonly target_fps: number
  readonly target_frame_ms: number
  readonly sample_count: number
  readonly sample_capacity: number
  readonly minimum_samples: number
  readonly total_ms: Readonly<{
    p50: number
    p95: number
    maximum: number
    mean: number
  }>
  readonly stages_ms: Readonly<{
    simulation_mean: number
    animation_mean: number
    physics_mean: number
    render_mean: number
  }>
  readonly over_budget_frames: number
  readonly over_budget_ratio: number
}

export interface WeaponryFrameBudgetMonitor {
  readonly schema_version: typeof WEAPONRY_FRAME_BUDGET_SCHEMA
  record(sample: WeaponryFrameTimingSample): void
  reset(): void
  snapshot(): WeaponryFrameBudgetSnapshot
}

/**
 * Bounded CPU frame sampler for the browser workbench. It deliberately does
 * not infer GPU time or commercial performance from requestAnimationFrame.
 */
export function createWeaponryFrameBudgetMonitor(
  config: WeaponryFrameBudgetConfig = {},
): WeaponryFrameBudgetMonitor {
  const targetFps = boundedInteger(config.target_fps ?? 60, 15, 240, 'target_fps')
  const capacity = boundedInteger(config.sample_capacity ?? 240, 30, 2_000, 'sample_capacity')
  const minimumSamples = boundedInteger(config.minimum_samples ?? 120, 1, capacity, 'minimum_samples')
  const targetFrameMs = 1_000 / targetFps
  const samples: WeaponryFrameTimingSample[] = []

  return {
    schema_version: WEAPONRY_FRAME_BUDGET_SCHEMA,
    record(sample) {
      validateSample(sample)
      samples.push(Object.freeze({ ...sample }))
      if (samples.length > capacity) samples.splice(0, samples.length - capacity)
    },
    reset() {
      samples.splice(0, samples.length)
    },
    snapshot() {
      const total = samples.map((sample) => sample.total_ms).sort((a, b) => a - b)
      const count = samples.length
      const overBudget = samples.filter((sample) => sample.total_ms > targetFrameMs).length
      const status = count < minimumSamples
        ? 'WARMING_UP'
        : percentile(total, 0.95) <= targetFrameMs
          ? 'MEASURED_WITHIN_BUDGET'
          : 'MEASURED_OVER_BUDGET'
      return Object.freeze({
        schema_version: WEAPONRY_FRAME_BUDGET_SNAPSHOT_SCHEMA,
        budget_schema_version: WEAPONRY_FRAME_BUDGET_SCHEMA,
        status,
        target_fps: targetFps,
        target_frame_ms: targetFrameMs,
        sample_count: count,
        sample_capacity: capacity,
        minimum_samples: minimumSamples,
        total_ms: Object.freeze({
          p50: percentile(total, 0.50),
          p95: percentile(total, 0.95),
          maximum: count === 0 ? 0 : total[count - 1],
          mean: mean(total),
        }),
        stages_ms: Object.freeze({
          simulation_mean: mean(samples.map((sample) => sample.simulation_ms)),
          animation_mean: mean(samples.map((sample) => sample.animation_ms)),
          physics_mean: mean(samples.map((sample) => sample.physics_ms)),
          render_mean: mean(samples.map((sample) => sample.render_ms)),
        }),
        over_budget_frames: overBudget,
        over_budget_ratio: count === 0 ? 0 : overBudget / count,
      })
    },
  }
}

function validateSample(sample: WeaponryFrameTimingSample): void {
  if (!sample || typeof sample !== 'object') throw new Error('WEAPONRY_FRAME_BUDGET_INVALID_SAMPLE: object required')
  for (const [field, value] of Object.entries(sample)) {
    if (!Number.isFinite(value) || value < 0) {
      throw new Error(`WEAPONRY_FRAME_BUDGET_INVALID_SAMPLE: ${field} must be finite and non-negative`)
    }
  }
  const stageTotal = sample.simulation_ms + sample.animation_ms + sample.physics_ms + sample.render_ms
  if (stageTotal > sample.total_ms + 0.25) {
    throw new Error('WEAPONRY_FRAME_BUDGET_INVALID_SAMPLE: stage durations exceed total frame duration')
  }
}

function boundedInteger(value: number, minimum: number, maximum: number, field: string): number {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    throw new Error(`WEAPONRY_FRAME_BUDGET_INVALID_CONFIG: ${field}`)
  }
  return value
}

function mean(values: readonly number[]): number {
  return values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length
}

function percentile(sortedValues: readonly number[], quantile: number): number {
  if (sortedValues.length === 0) return 0
  const index = Math.min(sortedValues.length - 1, Math.max(0, Math.ceil(sortedValues.length * quantile) - 1))
  return sortedValues[index]
}
