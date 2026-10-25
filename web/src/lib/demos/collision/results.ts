/** Broadphase flags, as in `src/collision/broadphase.rs`. */
export const BOUNDING_CIRCLE = 1
export const BOUNDING_BOX = 2
export const QUADTREE = 4

/** Indexed by `CollisionReport.stage`. */
export const STAGES = ['Bounding circle', 'Bounding box', 'Full test', 'Quadtree'] as const
export const FULL_TEST = 2

/** The method every other one is compared against. */
export const REFERENCE = 'gjk-epa'

/** One method's result for the current pose. */
export interface Row {
  id: string
  intersect: boolean
  distance: number | undefined
  stage: number
  /** `[dots, sqrts, divs, axes, supports, iterations]` */
  counts: number[]
}

export const sameRows = (a: readonly Row[], b: readonly Row[]) =>
  a.length === b.length &&
  a.every((r, i) => {
    const o = b[i]
    return (
      r.id === o.id &&
      r.intersect === o.intersect &&
      r.distance === o.distance &&
      r.stage === o.stage &&
      r.counts.every((c, k) => c === o.counts[k])
    )
  })

/** Quadtree numbers, as returned by `CollisionReport.scene()`. */
export interface SceneStats {
  bodies: number
  nodes: number
  depth: number
  checks: number
  candidates: number
  allPairs: number
  partners: number
  boxOverlaps: number
  collisions: number
}

export function readScene(raw: Uint32Array): SceneStats | null {
  if (raw.length < 9) return null
  const [bodies, nodes, depth, checks, candidates, allPairs, partners, boxOverlaps, collisions] = raw
  return { bodies, nodes, depth, checks, candidates, allPairs, partners, boxOverlaps, collisions }
}

export const sameScene = (a: SceneStats | null, b: SceneStats | null) =>
  a === b || (a !== null && b !== null && (Object.keys(a) as (keyof SceneStats)[]).every(k => a[k] === b[k]))

/**
 * Nanoseconds per call of `run(n)`, which must do its work `n` times.
 * Browsers coarsen performance.now(), so the batch grows until it takes a few ms.
 */
export function timePerRun(run: (n: number) => void) {
  let n = 16
  for (;;) {
    const t0 = performance.now()
    run(n)
    const elapsed = performance.now() - t0
    if (elapsed > 4 || n >= 1 << 22) return (elapsed * 1e6) / n
    n *= 4
  }
}

export const formatDistance = (d: number | undefined) =>
  d === undefined ? '—' : d > 0 ? `gap ${d.toFixed(1)}` : `overlap ${(-d).toFixed(1)}`

export const formatTime = (ns: number | undefined) =>
  ns === undefined
    ? '…'
    : ns >= 1000
      ? `${(ns / 1000).toFixed(ns >= 10000 ? 0 : 1)} µs`
      : ns < 10
        ? `${ns.toFixed(1)} ns`
        : `${Math.round(ns)} ns`

/** Percentage with a non-breaking space, so the sign never wraps onto its own line. */
export const percent = (part: number, whole: number) => `${((100 * part) / Math.max(1, whole)).toFixed(1)} %`
