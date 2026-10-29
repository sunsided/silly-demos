import type { VectorLab } from 'silly_demos'
import type { Tone } from '../../components/types.ts'
import { readSearchCurve, type SearchCurve } from '../search.ts'

export type IndexId = 'hnsw' | 'spfresh' | 'pq' | 'vamana' | 'tree'
export type Phase = 'streaming' | 'building' | 'done'

export interface IndexInfo {
  id: IndexId
  label: string
  tone: Tone
  /** What its search effort is called. */
  effort: string
  /** What its two counts of VectorLab.cost are, after build cost and bytes. */
  counts: [string, string]
}

export const INDEXES: readonly IndexInfo[] = [
  { id: 'hnsw', label: 'HNSW', tone: 'cobalt', effort: 'ef', counts: ['layers', 'links per point'] },
  { id: 'spfresh', label: 'IVF, split online', tone: 'verdigris', effort: 'probe', counts: ['postings', 'points moved'] },
  { id: 'tree', label: 'Tree, repaired', tone: 'ink', effort: 'beam', counts: ['nodes', 'repairs'] },
  { id: 'pq', label: 'IVF-PQ', tone: 'amber', effort: 'probe', counts: ['lists', 'bytes of code'] },
  { id: 'vamana', label: 'Vamana', tone: 'signal', effort: 'L', counts: ['links per point', 'built'] },
]

export const indexInfo = (id: IndexId): IndexInfo => INDEXES.find(i => i.id === id) ?? INDEXES[0]

/** A tunable parameter, as VectorLab.params() describes it. */
export interface VectorParam {
  id: string
  label: string
  /** The index it tunes, or 'stream'. */
  index: IndexId | 'stream'
  value: number
  default: number
  min: number
  max: number
  step: number
}

export const readVectorParams = (lab: Pick<VectorLab, 'params'>): VectorParam[] =>
  JSON.parse(lab.params()) as VectorParam[]

/** What an index cost to build and holds. */
export interface IndexCost {
  /** Distances per point to build it. */
  build: number
  /** Bytes held in memory per point. */
  bytes: number
  /** Its own two counts, as IndexInfo.counts names them. */
  counts: [number, number]
}

export interface VectorStats {
  phase: Phase
  points: number
  total: number
  dim: number
  /** Share of Vamana's two passes done. */
  built: number
  cost: Partial<Record<IndexId, IndexCost>>
}

export function readVectorStats(lab: VectorLab): VectorStats {
  const [points, total, dim, built] = lab.progress()
  const cost: Partial<Record<IndexId, IndexCost>> = {}
  for (const { id } of INDEXES) {
    const c = lab.cost(id)
    if (c.length === 4) cost[id] = { build: c[0], bytes: c[1], counts: [c[2], c[3]] }
  }
  return { phase: lab.phase() as Phase, points, total, dim, built, cost }
}

export const emptyVectorStats = (): VectorStats => ({
  phase: 'streaming',
  points: 0,
  total: 0,
  dim: 64,
  built: 0,
  cost: {},
})

/** Recall@10 against distances at each of the index's efforts; null until it can be searched. */
export const readCurve = (lab: VectorLab, id: IndexId): SearchCurve | null => readSearchCurve(lab.search(id))

/** One search: what it found and every point it compared the query with. */
export interface Trace {
  found: Uint32Array
  seen: Uint32Array
}

export function readTrace(lab: VectorLab, id: IndexId, query: number, effort: number): Trace | null {
  const t = lab.trace(id, query, effort)
  if (t.length === 0) return null
  const n = t[0]
  return { found: t.subarray(1, 1 + n), seen: t.subarray(1 + n) }
}
