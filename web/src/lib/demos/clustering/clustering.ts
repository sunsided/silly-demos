import type { ClusterLab } from 'silly_demos'

export type ScenarioId = 'blobs' | 'anisotropic' | 'rings' | 'moons' | 'drift'
export type ModelId = 'balls' | 'sketch'

export const SCENARIOS: readonly { value: ScenarioId; label: string; detail: string }[] = [
  {
    value: 'blobs',
    label: 'Blobs',
    detail: 'Five round Gaussian clusters, well apart: what every method should get right.',
  },
  {
    value: 'anisotropic',
    label: 'Long and thin',
    detail: 'Four stretched, rotated Gaussians. A round ball fits them badly, so they come apart into several.',
  },
  {
    value: 'rings',
    label: 'Rings',
    detail: 'Two concentric rings: one cluster each, but no center inside either. Balls cut them into arcs.',
  },
  {
    value: 'moons',
    label: 'Moons',
    detail: 'Two interleaved half circles: close, curved, and not ball-shaped at all.',
  },
  {
    value: 'drift',
    label: 'Drifting',
    detail:
      'Over a cycle of 12 000 points one cluster circles the middle, the center one splits in two, one dies and a new one appears.',
  },
]

export const MODELS: readonly { id: ModelId; label: string }[] = [
  { id: 'balls', label: 'Balls' },
  { id: 'sketch', label: 'Sketch + ball k-means' },
]

/** The event kinds in the order ClusterLab numbers them. */
export const EVENT_KINDS = ['spawn', 'merge', 'prune', 'split'] as const
export type EventKind = (typeof EVENT_KINDS)[number]

/** A tunable parameter, as ClusterLab.params() describes it. */
export interface StreamParam {
  id: string
  model: ModelId | 'both'
  label: string
  value: number
  default: number
  min: number
  max: number
  step: number
}

export const readStreamParams = (lab: Pick<ClusterLab, 'params'>): StreamParam[] =>
  JSON.parse(lab.params()) as StreamParam[]

export const PARAM_GROUPS: Record<StreamParam['model'], string> = {
  balls: 'Balls',
  sketch: 'Sketch',
  both: 'Both',
}

export interface StreamStats {
  t: number
  /** True clusters making points now. */
  truth: number
  balls: number
  sketch: number
  /** Distances per point the balls computed to place points. */
  ballSearch: number
  /** Distances per point the balls computed to keep their links. */
  ballUpkeep: number
  /** Distances per point the sketch computed. */
  sketchSearch: number
  /** Balls in all, light ones included. */
  ballCount: number
  /** Spawns, merges, prunes and splits so far, per model. */
  events: Record<ModelId, number[]>
  /** Flat rows: t, true k, balls k, sketch k, ARI balls, ARI sketch, ARI batch k-means. */
  history: Float32Array
}

export function readStreamStats(lab: ClusterLab): StreamStats {
  const [t, truth, balls, sketch, ballSearch, ballUpkeep, sketchSearch, ballCount] = lab.stats()
  return {
    t,
    truth,
    balls,
    sketch,
    ballSearch,
    ballUpkeep,
    sketchSearch,
    ballCount,
    events: { balls: [...lab.event_counts('balls')], sketch: [...lab.event_counts('sketch')] },
    history: lab.history(),
  }
}

export const emptyStreamStats = (): StreamStats => ({
  t: 0,
  truth: 0,
  balls: 0,
  sketch: 0,
  ballSearch: 0,
  ballUpkeep: 0,
  sketchSearch: 0,
  ballCount: 0,
  events: { balls: [0, 0, 0, 0], sketch: [0, 0, 0, 0] },
  history: new Float32Array(),
})

/** The balls' trace of the last point, from ClusterLab.trace('balls'). */
export interface BallTrace {
  point: [number, number]
  path: number[]
  checked: [number, number][]
  fullScan: boolean
  ball: number
  spawned: boolean
}

/** The sketch's trace of the last point, from ClusterLab.trace('sketch'). */
export interface SketchTrace {
  point: [number, number]
  nearest: [number, number] | null
  d2: number | null
  f: number
  p: number
  draw: number
  opened: boolean
  collapsed: boolean
}

/**
 * A color per cluster id: hues a golden angle apart, so neighboring ids
 * differ, in a lightness that reads on the theme's paper.
 */
export function clusterColor(id: number, dark: boolean, alpha = 1): string {
  const hue = (id * 137.508) % 360
  return `hsl(${hue.toFixed(1)} 70% ${dark ? 62 : 44}% / ${alpha})`
}

/** The label points from no true cluster carry (u32::MAX). */
export const PAINTED = 4294967295
