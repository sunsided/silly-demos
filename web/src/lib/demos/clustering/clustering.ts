import type { ClusterLab } from 'silly_demos'

export type ScenarioId = 'blobs' | 'anisotropic' | 'rings' | 'moons' | 'drift'
export type ModelId = 'balls' | 'ellipses' | 'sketch'

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
  { id: 'ellipses', label: 'Ellipses' },
  { id: 'sketch', label: 'Sketch + ball k-means' },
]

/** The event kinds in the order ClusterLab numbers them. */
export const EVENT_KINDS = ['spawn', 'merge', 'prune', 'split'] as const
export type EventKind = (typeof EVENT_KINDS)[number]

/** A tunable parameter, as ClusterLab.params() describes it. */
export interface StreamParam {
  id: string
  model: ModelId | 'all'
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
  ellipses: 'Ellipses',
  sketch: 'Sketch',
  all: 'All three',
}

/** What one model has found, and what it cost. */
export interface ModelStats {
  /** Clusters it counts. */
  clusters: number
  /** Distances per point computed to place points. */
  search: number
  /** Distances per point computed to keep its links (0 for the sketch). */
  upkeep: number
  /** Clusters in all, light ones included. */
  total: number
  /** Spawns, merges, prunes and splits so far. */
  events: number[]
}

export interface StreamStats {
  t: number
  /** True clusters making points now. */
  truth: number
  models: Record<ModelId, ModelStats>
  /**
   * Flat rows of nine: t, true k, k of balls, ellipses and sketch, the
   * adjusted Rand index of balls, ellipses and sketch, and of batch k-means.
   */
  history: Float32Array
}

export const HISTORY_STRIDE = 9

export function readStreamStats(lab: ClusterLab): StreamStats {
  const s = lab.stats()
  const models = {} as Record<ModelId, ModelStats>
  MODELS.forEach(({ id }, i) => {
    const [clusters, search, upkeep, total] = s.subarray(2 + 4 * i, 6 + 4 * i)
    models[id] = { clusters, search, upkeep, total, events: [...lab.event_counts(id)] }
  })
  return { t: s[0], truth: s[1], models, history: lab.history() }
}

export function emptyStreamStats(): StreamStats {
  const models = {} as Record<ModelId, ModelStats>
  for (const { id } of MODELS) models[id] = { clusters: 0, search: 0, upkeep: 0, total: 0, events: [0, 0, 0, 0] }
  return { t: 0, truth: 0, models, history: new Float32Array() }
}

/** The balls' or ellipses' trace of the last point, from ClusterLab.trace. */
export interface WalkTrace {
  point: [number, number]
  path: number[]
  /** Cluster id and distance (Mahalanobis for ellipses). */
  checked: [number, number][]
  fullScan: boolean
  cluster: number
  spawned: boolean
}

/** Values per cluster in ClusterLab.clusters: id, x, y, long and short standard deviation, angle, weight, counted. */
export const CLUSTER_STRIDE = 8

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
