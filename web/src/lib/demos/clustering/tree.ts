import type { TreeLab } from 'silly_demos'
import { readSearchCurve, type SearchCurve } from '../search.ts'

export type SpaceId = 'sphere64' | 'plane'
export type WhichTree = 'online' | 'careful' | 'repaired' | 'passes'
/** The colors trees are told apart by. */
export type TreeTone = 'cobalt' | 'verdigris' | 'signal' | 'amber'

export const SPACES: readonly { value: SpaceId; label: string; detail: string }[] = [
  {
    value: 'sphere64',
    label: '64-D embeddings',
    detail:
      'Unit vectors in 64 dimensions, compared by cosine, like image embeddings. Drawn on their two principal axes, which shows the topics but squashes the rest.',
  },
  {
    value: 'plane',
    label: 'The same in 2D',
    detail: 'The same tree of topics, subtopics and leaf clusters laid out in the plane, where every split can be seen.',
  },
]

export const TREES: readonly { id: WhichTree; label: string; tone: TreeTone }[] = [
  { id: 'online', label: 'Grown online', tone: 'cobalt' },
  { id: 'careful', label: 'Grown carefully', tone: 'verdigris' },
  { id: 'repaired', label: 'Grown fast, repaired', tone: 'signal' },
  { id: 'passes', label: 'Built in passes', tone: 'amber' },
]

/** Values per node in TreeLab.nodes. */
export const NODE_STRIDE = 8

/** A tunable parameter, as TreeLab.params() describes it. */
export interface TreeParam {
  id: string
  label: string
  value: number
  default: number
  min: number
  max: number
  step: number
}

export const readTreeParams = (lab: Pick<TreeLab, 'params'>): TreeParam[] => JSON.parse(lab.params()) as TreeParam[]

/** How one tree matches the true hierarchy, and what it cost. */
export interface TreeQuality {
  /** Adjusted Rand index at depths 1–3 against topics, subtopics and leaves. */
  levels: [number, number, number]
  /** Adjusted Rand index of its leaves against the true leaves. */
  leaves: number
  /** Nodes at depths 1–4. */
  nodes: [number, number, number, number]
  leafCount: number
  /** Distances per point to place points. */
  place: number
  /** Distances per point to split nodes. */
  split: number
  /** Distances per point to re-check placed points. */
  recheck: number
  passes: number
  /** Splits put off for want of evidence. */
  deferred: number
  /** Points moved by re-checking. */
  moved: number
  /** Distances per point to decide repairs. */
  repair: number
  /** Repairs made: children merged, flattened, grouped or lifted. */
  repairs: number
}

export interface TreeStats {
  points: number
  total: number
  dim: number
  quality: Partial<Record<WhichTree, TreeQuality>>
}

export function readTreeStats(lab: TreeLab): TreeStats {
  const [points, total, dim] = lab.progress()
  const quality: Partial<Record<WhichTree, TreeQuality>> = {}
  for (const { id } of TREES) {
    const q = lab.quality(id)
    const c = lab.cost(id)
    if (q.length === 0) continue
    quality[id] = {
      levels: [q[0], q[1], q[2]],
      leaves: q[3],
      nodes: [q[4], q[5], q[6], q[7]],
      leafCount: q[8],
      place: c[0],
      split: c[1],
      recheck: c[2],
      passes: c[3],
      deferred: c[4],
      moved: c[5],
      repair: c[6],
      repairs: c[7],
    }
  }
  return { points, total, dim, quality }
}

/** Recall@10 against distances at beams of 1, 2, 4 and 8. */
export const readSearch = (lab: TreeLab, which: WhichTree): SearchCurve | null => readSearchCurve(lab.search(which))

export const emptyTreeStats = (): TreeStats => ({ points: 0, total: 0, dim: 64, quality: {} })
