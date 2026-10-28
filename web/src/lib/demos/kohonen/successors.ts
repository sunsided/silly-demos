import type { SuccessorLab } from 'silly_demos'
import type { Tone } from '../../components/types.ts'
import type { StatusId } from './kohonen.ts'

export type ModelId = 'som' | 'ng' | 'gng' | 'gng-u' | 'kmeans'

/** The models SuccessorLab trains, in layout order, each with the tone it is drawn in. */
export const MODELS: readonly { id: ModelId; label: string; tone: Tone; detail: string }[] = [
  {
    id: 'som',
    label: 'Kohonen map',
    tone: 'cobalt',
    detail:
      'The map itself: nodes on a fixed grid, neighbors by grid position. The grid is the point and the problem: it orders the data, but it can fold or twist, and it has to stretch over gaps in the data.',
  },
  {
    id: 'ng',
    label: 'Neural gas',
    tone: 'verdigris',
    detail:
      'Martinetz and Schulten, 1991. No grid: every node moves by a share that falls with its distance rank, so nothing can fold. Joining each sample’s two nearest nodes learns edges that follow the data’s own shape.',
  },
  {
    id: 'gng',
    label: 'Growing neural gas',
    tone: 'amber',
    detail:
      'Fritzke, 1995. Starts with two nodes and inserts one every 300 samples beside the node with the most error, so nodes appear where the data is worst covered. Edges no sample renews age and go.',
  },
  {
    id: 'gng-u',
    label: 'GNG with utility',
    tone: 'signal',
    detail:
      'Fritzke, 1997. Also tracks how much each node is worth and removes one whose worth falls far below the worst error, so it follows a density that moves. Erase part of the density while it runs to see it leave.',
  },
  {
    id: 'kmeans',
    label: 'k-means',
    tone: 'ink',
    detail:
      'MacQueen, 1967, seeded by k-means++: each sample moves only its nearest node, by one over the samples it has won. No neighbors and no edges, the baseline the others are measured against.',
  },
]

/** One model's latest errors and its history. */
export interface ModelStats {
  qe: number
  /** NaN for k-means, which has no links. */
  te: number
  nodes: number
  edges: number
  /** (t, QE, TE) triples. */
  history: Float32Array
}

export interface SuccessorStats {
  t: number
  steps: number
  status: StatusId
  models: Record<ModelId, ModelStats>
}

export function readSuccessorStats(lab: SuccessorLab): SuccessorStats {
  const [t, steps] = lab.progress()
  const models = {} as Record<ModelId, ModelStats>
  for (const { id } of MODELS) {
    const [qe, te, nodes, edges] = lab.errors(id)
    models[id] = { qe, te, nodes, edges, history: lab.history(id) }
  }
  return { t, steps, status: lab.status() as StatusId, models }
}

export function emptySuccessorStats(): SuccessorStats {
  const models = {} as Record<ModelId, ModelStats>
  for (const { id } of MODELS) models[id] = { qe: 0, te: 0, nodes: 0, edges: 0, history: new Float32Array() }
  return { t: 0, steps: 20_000, status: 'training', models }
}
