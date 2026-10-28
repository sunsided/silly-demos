import type { SomData, SomLab } from 'silly_demos'
import type { Point } from '../../canvas/canvas.svelte.ts'
import { arrowhead, cross, rgba } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'

export type LatticeId = 'chain' | 'ring' | 'grid' | 'hex'
export type InitId = 'random' | 'cluster' | 'pca'
export type TrainingId = 'online' | 'batch'
export type DecayId = 'exp' | 'linear'
export type DensityId = 'uniform' | 'annulus' | 'moons' | 'blobs' | 'lshape' | 'gradient' | 'blank' | 'image'
export type ModeId = 'unfolding' | 'colors' | 'countries' | 'tours'
export type StatusId = 'training' | 'converged' | 'empty'

export const MODES: readonly { value: ModeId; label: string; detail: string }[] = [
  {
    value: 'colors',
    label: 'Colors',
    detail: 'The map sorts colors onto a 2D sheet: random colors, or the pixels of an image, which it turns into a palette.',
  },
  {
    value: 'countries',
    label: 'Countries',
    detail: 'Countries by ten development indicators in 1992 and 2022, after Kaski and Kohonen’s 1996 world poverty map.',
  },
  {
    value: 'unfolding',
    label: 'Unfolding',
    detail: 'A lattice of nodes spreads over a 2D density you pick, paint or load from an image.',
  },
  {
    value: 'tours',
    label: 'Tours',
    detail: 'A ring of nodes pulled onto cities until it reads as a traveling salesman’s tour, raced against the classical heuristics.',
  },
]

export const LATTICES: readonly { value: LatticeId; label: string }[] = [
  { value: 'grid', label: 'Grid' },
  { value: 'hex', label: 'Hex grid' },
  { value: 'chain', label: 'Chain' },
  { value: 'ring', label: 'Ring' },
]

export const INITS: readonly { value: InitId; label: string }[] = [
  { value: 'random', label: 'Random samples' },
  { value: 'cluster', label: 'Tight cluster' },
  { value: 'pca', label: 'Principal axes' },
]

export const TRAININGS: readonly { value: TrainingId; label: string; detail: string }[] = [
  {
    value: 'online',
    label: 'Online',
    detail:
      "Kohonen's rule: one sample at a time, every node moves towards it by α · h, h falling off with lattice distance from the best-matching unit.",
  },
  {
    value: 'batch',
    label: 'Batch',
    detail:
      'Each epoch assigns all samples to their nearest node, then sets every node to the kernel-weighted mean of the samples its lattice neighbors won. No learning rate, and the order of the samples does not matter.',
  },
]

export const DECAYS: readonly { value: DecayId; label: string }[] = [
  { value: 'exp', label: 'Exponential' },
  { value: 'linear', label: 'Linear' },
]

export const DENSITIES: readonly { value: DensityId; label: string }[] = [
  { value: 'uniform', label: 'Square' },
  { value: 'annulus', label: 'Annulus' },
  { value: 'moons', label: 'Two moons' },
  { value: 'blobs', label: 'Blobs' },
  { value: 'lshape', label: 'L-shape' },
  { value: 'gradient', label: 'Gradient' },
  { value: 'blank', label: 'Blank (paint)' },
  { value: 'image', label: 'Image…' },
]

/** Everything that, when changed, starts the map over. */
export interface SomConfig {
  lattice: LatticeId
  width: number
  height: number
  init: InitId
  steps: number
  alpha: [number, number]
  sigma: [number, number]
  decay: DecayId
  continuous: boolean
  seed: number
}

export interface SomStats {
  t: number
  steps: number
  alpha: number
  sigma: number
  qe: number
  te: number
  status: StatusId
  /** (t, QE, TE) triples. */
  history: Float32Array
}

export function readStats(lab: Pick<SomLab, 'progress' | 'status' | 'history'>): SomStats {
  const [t, steps, alpha, sigma, qe, te] = lab.progress()
  return { t, steps, alpha, sigma, qe, te, status: lab.status() as StatusId, history: lab.history() }
}

/** Pushes `config` into `lab` and starts over; painted density stays. */
export function applyConfig(lab: SomLab, c: SomConfig) {
  lab.set_schedule(c.steps, c.alpha[0], c.alpha[1], c.sigma[0], c.sigma[1], c.decay, c.continuous)
  lab.set_init(c.init)
  lab.set_lattice(c.lattice, c.width, c.height)
  lab.set_seed(c.seed)
}

/**
 * Pushes `config` into a data map and starts over. Data maps draw their
 * nodes as cells, so a chain or ring falls back to a grid.
 */
export function applyDataConfig(lab: SomData, c: SomConfig) {
  lab.set_schedule(c.steps, c.alpha[0], c.alpha[1], c.sigma[0], c.sigma[1], c.decay, c.continuous)
  lab.set_init(c.init)
  const kind = c.lattice === 'hex' ? 'hex' : 'grid'
  lab.set_lattice(kind, c.width, c.height)
  lab.set_seed(c.seed)
}

/** The unit square input space on the canvas: a square sheet, centered. */
export interface Sheet {
  x: number
  y: number
  size: number
}

export function fit(width: number, height: number, margin = 16): Sheet {
  const size = Math.max(1, Math.min(width, height) - 2 * margin)
  return { x: (width - size) / 2, y: (height - size) / 2, size }
}

export const toCanvas = (s: Sheet, x: number, y: number): Point => ({ x: s.x + x * s.size, y: s.y + y * s.size })
export const toUnit = (s: Sheet, p: Point): Point => ({ x: (p.x - s.x) / s.size, y: (p.y - s.y) / s.size })

export function drawFrame(ctx: CanvasRenderingContext2D, t: Theme, s: Sheet) {
  ctx.save()
  ctx.strokeStyle = t.inkFaint
  ctx.lineWidth = 1
  ctx.strokeRect(s.x, s.y, s.size, s.size)
  ctx.restore()
}

/** Renders the density into an offscreen canvas, one pixel per cell, in faint ink. */
export class DensityImage {
  // Created on first use: there is no document while prerendering.
  private canvas: HTMLCanvasElement | null = null
  private image: ImageData | null = null

  update(t: Theme, cells: Float32Array) {
    const side = Math.round(Math.sqrt(cells.length))
    this.canvas ??= document.createElement('canvas')
    if (!this.image || this.image.width !== side) {
      this.canvas.width = this.canvas.height = side
      this.image = new ImageData(side, side)
    }
    const [r, g, b] = rgba(t.ink)
    const px = this.image.data
    for (let i = 0; i < cells.length; i++) {
      px[i * 4] = r
      px[i * 4 + 1] = g
      px[i * 4 + 2] = b
      px[i * 4 + 3] = Math.round(cells[i] * 0.22 * 255)
    }
    this.canvas.getContext('2d')!.putImageData(this.image, 0, 0)
  }

  draw(ctx: CanvasRenderingContext2D, s: Sheet) {
    if (!this.canvas) return
    ctx.save()
    ctx.imageSmoothingEnabled = false
    ctx.drawImage(this.canvas, s.x, s.y, s.size, s.size)
    ctx.restore()
  }
}

/** The nodes' Voronoi regions, clipped to the sheet. */
export function drawVoronoi(ctx: CanvasRenderingContext2D, t: Theme, s: Sheet, segments: Float32Array) {
  ctx.save()
  ctx.beginPath()
  ctx.rect(s.x, s.y, s.size, s.size)
  ctx.clip()
  ctx.strokeStyle = t.inkFaint
  ctx.lineWidth = 1
  ctx.beginPath()
  for (let i = 0; i + 3 < segments.length; i += 4) {
    const a = toCanvas(s, segments[i], segments[i + 1])
    const b = toCanvas(s, segments[i + 2], segments[i + 3])
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
  }
  ctx.stroke()
  ctx.restore()
}

/** Lattice links between the nodes' weights, and the nodes as dots. */
export function drawLattice(
  ctx: CanvasRenderingContext2D,
  s: Sheet,
  weights: Float32Array,
  edges: Uint32Array,
  color: string,
  { dots = true, alpha = 1 }: { dots?: boolean; alpha?: number } = {},
) {
  ctx.save()
  ctx.globalAlpha = alpha
  ctx.strokeStyle = color
  ctx.fillStyle = color
  ctx.lineWidth = 1.25
  ctx.beginPath()
  for (let i = 0; i + 1 < edges.length; i += 2) {
    const a = toCanvas(s, weights[edges[i] * 2], weights[edges[i] * 2 + 1])
    const b = toCanvas(s, weights[edges[i + 1] * 2], weights[edges[i + 1] * 2 + 1])
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
  }
  ctx.stroke()
  if (dots) {
    const r = weights.length / 2 > 200 ? 1.5 : 2.5
    ctx.beginPath()
    for (let i = 0; i + 1 < weights.length; i += 2) {
      const p = toCanvas(s, weights[i], weights[i + 1])
      ctx.moveTo(p.x + r, p.y)
      ctx.arc(p.x, p.y, r, 0, Math.PI * 2)
    }
    ctx.fill()
  }
  ctx.restore()
}

/** Arrows shorter than this are drawn this long, so late, tiny moves stay visible. */
const MIN_ARROW = 6

export interface Trace {
  sample: Float32Array
  bmu: number
  kernel: Float32Array
  previous: Float32Array
}

export function readTrace(lab: SomLab): Trace | null {
  const kernel = lab.trace_kernel()
  if (kernel.length === 0) return null
  return { sample: lab.trace_sample(), bmu: lab.trace_bmu(), kernel, previous: lab.trace_previous() }
}

/**
 * The anatomy of the last update: the lattice before it as a ghost, every
 * node shaded by its kernel value h, arrows from old to new positions, and
 * the sample with its best-matching unit (online only).
 */
export function drawTrace(
  ctx: CanvasRenderingContext2D,
  t: Theme,
  s: Sheet,
  trace: Trace,
  weights: Float32Array,
  edges: Uint32Array,
) {
  // A trace from before the lattice changed has the wrong length; skip it.
  if (trace.kernel.length !== weights.length / 2 || trace.previous.length !== weights.length) return
  drawLattice(ctx, s, trace.previous, edges, t.amber, { dots: false, alpha: 0.45 })

  ctx.save()
  ctx.fillStyle = t.cobalt
  for (let i = 0; i < trace.kernel.length; i++) {
    const h = trace.kernel[i]
    if (h < 0.01 || trace.bmu < 0) continue
    const p = toCanvas(s, weights[i * 2], weights[i * 2 + 1])
    ctx.globalAlpha = 0.35 * h
    ctx.beginPath()
    ctx.arc(p.x, p.y, 7, 0, Math.PI * 2)
    ctx.fill()
  }
  ctx.restore()

  ctx.save()
  ctx.strokeStyle = t.amber
  ctx.fillStyle = t.amber
  ctx.lineWidth = 1
  for (let i = 0; i < trace.kernel.length; i++) {
    const from = toCanvas(s, trace.previous[i * 2], trace.previous[i * 2 + 1])
    const to = toCanvas(s, weights[i * 2], weights[i * 2 + 1])
    const dx = to.x - from.x
    const dy = to.y - from.y
    const len = Math.hypot(dx, dy)
    if (len < 1e-6 * s.size) continue
    const ux = dx / len
    const uy = dy / len
    const tip = len < MIN_ARROW ? { x: from.x + ux * MIN_ARROW, y: from.y + uy * MIN_ARROW } : to
    ctx.beginPath()
    ctx.moveTo(from.x, from.y)
    ctx.lineTo(tip.x, tip.y)
    ctx.stroke()
    arrowhead(ctx, tip, ux, uy, 4)
  }
  ctx.restore()

  if (trace.sample.length === 2 && trace.bmu >= 0) {
    const bmu = toCanvas(s, weights[trace.bmu * 2], weights[trace.bmu * 2 + 1])
    ctx.save()
    ctx.strokeStyle = t.signal
    ctx.lineWidth = 1.5
    ctx.beginPath()
    ctx.arc(bmu.x, bmu.y, 6, 0, Math.PI * 2)
    ctx.stroke()
    ctx.restore()
    cross(ctx, toCanvas(s, trace.sample[0], trace.sample[1]), t.signal, 6)
  }
}

/** The brush outline at the pointer. */
export function drawBrush(ctx: CanvasRenderingContext2D, t: Theme, s: Sheet, at: Point, radius: number) {
  ctx.save()
  ctx.strokeStyle = t.inkSoft
  ctx.setLineDash([3, 3])
  ctx.beginPath()
  ctx.arc(at.x, at.y, radius * s.size, 0, Math.PI * 2)
  ctx.stroke()
  ctx.restore()
}

export interface Scenario {
  id: string
  label: string
  config: SomConfig
  density: DensityId
  note: string
}

const classic: SomConfig = {
  lattice: 'grid',
  width: 12,
  height: 12,
  init: 'random',
  steps: 20_000,
  alpha: [0.5, 0.01],
  sigma: [6, 0.5],
  decay: 'exp',
  continuous: false,
  seed: 1,
}

/** Presets that set everything at once, then start over. */
export const SCENARIOS: readonly Scenario[] = [
  {
    id: 'classic',
    label: 'Classic unfold',
    config: classic,
    density: 'uniform',
    note: 'Nodes start on random samples, with no order at all. A wide σ first pulls the whole lattice into one ordered clump; as σ shrinks, it unfolds and spreads over the square, neighbors staying neighbors.',
  },
  {
    id: 'bowtie',
    label: 'Bow-tie',
    config: { ...classic, init: 'cluster', steps: 10_000, sigma: [1, 0.3], seed: 17 },
    density: 'uniform',
    note: 'All nodes start in a tight cluster and σ starts small, so distant parts of the lattice order themselves independently, some flipped against the others. Where they meet, the lattice folds over, and a σ that shrinks too fast freezes the fold in: the topographic error stays high.',
  },
  {
    id: 'peano',
    label: 'Space-filling',
    config: { ...classic, lattice: 'chain', width: 300, height: 1, steps: 40_000, sigma: [30, 0.5] },
    density: 'uniform',
    note: 'A one-dimensional chain covering a two-dimensional square cannot keep both its order and the square’s: it folds back and forth into a Peano-like curve. Points close in the square can be far apart on the chain.',
  },
  {
    id: 'ring',
    label: 'Ring on annulus',
    config: { ...classic, lattice: 'ring', width: 60, height: 1, sigma: [10, 0.5] },
    density: 'annulus',
    note: 'A ring has the same topology as the annulus, so it closes around the hole and keeps its order all the way round. The band has a width, though, and a one-dimensional ring covers it by zig-zagging across: those small folds are what the topographic error counts here.',
  },
  {
    id: 'magnification',
    label: 'Magnification',
    config: { ...classic, steps: 30_000 },
    density: 'gradient',
    note: 'The density rises from left to right, and the nodes crowd towards the dense side. Not in proportion, though: for a chain, node density grows with the input density to the power 2/3, so a SOM under-samples dense regions and over-samples sparse ones compared with the data.',
  },
]
