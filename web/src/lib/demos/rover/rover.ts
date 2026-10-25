import type { RoverLab } from 'silly_demos'
import type { Point } from '../../canvas/canvas.svelte.ts'
import { cross, label } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'

export type ModeId = 'explorer' | 'reactive'
export type StatusId = 'exploring' | 'returning' | 'done'

export const MODES: readonly { value: ModeId; label: string; detail: string }[] = [
  {
    value: 'explorer',
    label: 'Frontier explorer',
    detail:
      'Plans the shortest drive through known free space to a spot where its laser can see into the unknown behind a frontier, follows the path with pure pursuit, and drives home once no frontier is left.',
  },
  {
    value: 'reactive',
    label: 'Reactive (2013)',
    detail:
      'The original lab controller: proportional terms on averaged laser sectors keep it moving and off the walls. Its steering only looks at the current scan; the map and its frontiers only tell it when to stop.',
  },
]

export interface RoverStats {
  coverage: number
  distance: number
  time: number
  frontiers: number
  status: StatusId
}

/** World (metres) to canvas (CSS px): uniform scale, centred, with a margin. */
export interface View {
  scale: number
  x: number
  y: number
}

export function fit(width: number, height: number, size: ArrayLike<number>, margin = 16): View {
  const scale = Math.max(0.01, Math.min((width - 2 * margin) / size[0], (height - 2 * margin) / size[1]))
  return { scale, x: (width - size[0] * scale) / 2, y: (height - size[1] * scale) / 2 }
}

const rgbaCache = new Map<string, [number, number, number, number]>()

/** A CSS color as RGBA bytes, for writing pixels. */
function rgba(color: string): [number, number, number, number] {
  const cached = rgbaCache.get(color)
  if (cached) return cached
  const c = document.createElement('canvas')
  c.width = c.height = 1
  const ctx = c.getContext('2d', { willReadFrequently: true })!
  ctx.fillStyle = color
  ctx.fillRect(0, 0, 1, 1)
  const d = ctx.getImageData(0, 0, 1, 1).data
  const value: [number, number, number, number] = [d[0], d[1], d[2], d[3]]
  rgbaCache.set(color, value)
  return value
}

/**
 * Renders the robot's map into an offscreen canvas, one pixel per cell:
 * unknown cells fogged, free cells clear, known walls in ink, frontiers in amber.
 */
export class KnowledgeImage {
  // Created on first use: there is no document while prerendering.
  private canvas: HTMLCanvasElement | null = null
  private image: ImageData | null = null

  update(t: Theme, cells: Uint8Array, columns: number, rows: number) {
    this.canvas ??= document.createElement('canvas')
    if (!this.image || this.image.width !== columns || this.image.height !== rows) {
      this.canvas.width = columns
      this.canvas.height = rows
      this.image = new ImageData(columns, rows)
    }
    const palette = [rgba(t.inkFaint), [0, 0, 0, 0], rgba(t.ink), rgba(t.amber)]
    const px = this.image.data
    for (let i = 0; i < cells.length; i++) {
      const c = palette[cells[i]] ?? palette[0]
      px[i * 4] = c[0]
      px[i * 4 + 1] = c[1]
      px[i * 4 + 2] = c[2]
      px[i * 4 + 3] = c[3]
    }
    this.canvas.getContext('2d')!.putImageData(this.image, 0, 0)
  }

  /** Draws the map over the floor at `view`, cell `cell` m wide. */
  draw(ctx: CanvasRenderingContext2D, view: View, cell: number) {
    if (!this.canvas) return
    ctx.save()
    ctx.imageSmoothingEnabled = false
    ctx.drawImage(
      this.canvas,
      view.x,
      view.y,
      this.canvas.width * cell * view.scale,
      this.canvas.height * cell * view.scale,
    )
    ctx.restore()
  }
}

/** Width of one clearance band, m. */
const BAND = 0.1
/** Clearance at which the shading fades out, m; the field is capped there. */
const CLEARANCE_CAP = 2

/**
 * Renders the clearance field into an offscreen canvas, one pixel per cell:
 * known free cells shaded in cobalt, darker nearer to a wall, in 0.1 m bands;
 * the edge where it drops below the hard limit (the robot's centre can't go
 * nearer) outlined in signal red, and where it drops below the soft limit
 * (paths cost extra) in cobalt.
 */
export class ClearanceImage {
  // Created on first use: there is no document while prerendering.
  private canvas: HTMLCanvasElement | null = null
  private image: ImageData | null = null

  update(t: Theme, clear: Float32Array, columns: number, rows: number, room: ArrayLike<number>) {
    this.canvas ??= document.createElement('canvas')
    if (!this.image || this.image.width !== columns || this.image.height !== rows) {
      this.canvas.width = columns
      this.canvas.height = rows
      this.image = new ImageData(columns, rows)
    }
    const hard = room[0]
    const soft = room[1]
    const fill = rgba(t.cobalt)
    const hardEdge = rgba(t.signal)
    const bands = CLEARANCE_CAP / BAND
    const px = this.image.data
    // Whether cell i is free but below `limit` while an edge neighbour is free at or above it.
    const edge = (i: number, x: number, y: number, limit: number) => {
      const d = clear[i]
      if (d < 0 || d >= limit) return false
      const above = (j: number) => clear[j] >= limit
      return (x > 0 && above(i - 1)) || (x + 1 < columns && above(i + 1)) ||
        (y > 0 && above(i - columns)) || (y + 1 < rows && above(i + columns))
    }
    for (let y = 0, i = 0; y < rows; y++) {
      for (let x = 0; x < columns; x++, i++) {
        const d = clear[i]
        let c: readonly number[] = [0, 0, 0, 0]
        if (edge(i, x, y, hard)) c = hardEdge
        else if (edge(i, x, y, soft)) c = fill
        else if (d >= 0) {
          const band = Math.min(bands, Math.floor(d / BAND))
          c = [fill[0], fill[1], fill[2], Math.round(150 * (1 - band / bands) ** 1.6)]
        }
        px[i * 4] = c[0]
        px[i * 4 + 1] = c[1]
        px[i * 4 + 2] = c[2]
        px[i * 4 + 3] = c[3]
      }
    }
    this.canvas.getContext('2d')!.putImageData(this.image, 0, 0)
  }

  draw(ctx: CanvasRenderingContext2D, view: View, cell: number) {
    if (!this.canvas) return
    ctx.save()
    ctx.imageSmoothingEnabled = false
    ctx.drawImage(
      this.canvas,
      view.x,
      view.y,
      this.canvas.width * cell * view.scale,
      this.canvas.height * cell * view.scale,
    )
    ctx.restore()
  }
}

/** The clearance under the pointer, labelled beside it. */
export function drawClearanceAt(
  ctx: CanvasRenderingContext2D,
  t: Theme,
  view: View,
  pointer: Point,
  clear: Float32Array,
  grid: ArrayLike<number>,
) {
  const columns = grid[0]
  const rows = grid[1]
  const cell = grid[2]
  const x = Math.floor((pointer.x - view.x) / view.scale / cell)
  const y = Math.floor((pointer.y - view.y) / view.scale / cell)
  if (x < 0 || y < 0 || x >= columns || y >= rows) return
  const d = clear[y * columns + x]
  const text = d < 0 ? 'not known free' : d >= CLEARANCE_CAP ? `≥ ${CLEARANCE_CAP} m clear` : `${d.toFixed(2)} m clear`
  label(ctx, t, text, { x: pointer.x + 12, y: pointer.y - 12 }, t.ink, 'left')
}

const at = (view: View, x: number, y: number): Point => ({ x: view.x + x * view.scale, y: view.y + y * view.scale })

/** The true floor plan, as faint dashed lines. */
export function drawFloorPlan(ctx: CanvasRenderingContext2D, t: Theme, view: View, walls: Float32Array) {
  ctx.beginPath()
  for (let i = 0; i + 3 < walls.length; i += 4) {
    const a = at(view, walls[i], walls[i + 1])
    const b = at(view, walls[i + 2], walls[i + 3])
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
  }
  ctx.setLineDash([6, 4])
  ctx.lineWidth = 1
  ctx.strokeStyle = t.inkSoft
  ctx.stroke()
  ctx.setLineDash([])
}

/** The last scan: the area it covered, and a dot where each beam hit a wall. */
export function drawScan(
  ctx: CanvasRenderingContext2D,
  t: Theme,
  view: View,
  pose: Float32Array,
  ranges: Float32Array,
  laser: Float32Array,
) {
  const [fov, range] = laser
  const n = ranges.length
  if (n < 2) return
  const origin = at(view, pose[0], pose[1])
  const end = (i: number) => {
    const a = pose[2] - fov / 2 + (fov * i) / (n - 1)
    return at(view, pose[0] + Math.cos(a) * ranges[i], pose[1] + Math.sin(a) * ranges[i])
  }
  ctx.save()
  ctx.beginPath()
  ctx.moveTo(origin.x, origin.y)
  for (let i = 0; i < n; i++) {
    const p = end(i)
    ctx.lineTo(p.x, p.y)
  }
  ctx.closePath()
  ctx.fillStyle = t.cobaltFaint
  ctx.fill()
  ctx.beginPath()
  for (let i = 0; i < n; i++) {
    if (ranges[i] >= range - 1e-3) continue
    const p = end(i)
    ctx.moveTo(p.x + 1.5, p.y)
    ctx.arc(p.x, p.y, 1.5, 0, Math.PI * 2)
  }
  ctx.fillStyle = t.cobalt
  ctx.fill()
  ctx.restore()
}

function polyline(ctx: CanvasRenderingContext2D, view: View, pts: Float32Array) {
  ctx.beginPath()
  for (let i = 0; i + 1 < pts.length; i += 2) {
    const p = at(view, pts[i], pts[i + 1])
    if (i === 0) ctx.moveTo(p.x, p.y)
    else ctx.lineTo(p.x, p.y)
  }
}

export function drawTrail(ctx: CanvasRenderingContext2D, t: Theme, view: View, trail: Float32Array) {
  if (trail.length < 4) return
  polyline(ctx, view, trail)
  ctx.lineWidth = 1.5
  ctx.lineJoin = 'round'
  ctx.strokeStyle = t.signal
  ctx.stroke()
}

/** The explorer's plan: the path, the frontier it heads for, and the ones it gave up on. */
export function drawPlan(
  ctx: CanvasRenderingContext2D,
  t: Theme,
  view: View,
  path: Float32Array,
  target: Float32Array,
  givenUp: Float32Array,
) {
  if (path.length >= 4) {
    polyline(ctx, view, path)
    ctx.setLineDash([5, 4])
    ctx.lineWidth = 1.5
    ctx.strokeStyle = t.cobalt
    ctx.stroke()
    ctx.setLineDash([])
    for (let i = 2; i + 1 < path.length; i += 2) {
      const p = at(view, path[i], path[i + 1])
      ctx.beginPath()
      ctx.arc(p.x, p.y, 2, 0, Math.PI * 2)
      ctx.fillStyle = t.cobalt
      ctx.fill()
    }
  }
  for (let i = 0; i + 1 < givenUp.length; i += 2) {
    cross(ctx, at(view, givenUp[i], givenUp[i + 1]), t.inkSoft, 3)
  }
  if (target.length === 2) {
    const p = at(view, target[0], target[1])
    ctx.beginPath()
    ctx.arc(p.x, p.y, 6, 0, Math.PI * 2)
    ctx.lineWidth = 1.5
    ctx.strokeStyle = t.amber
    ctx.stroke()
    cross(ctx, p, t.amber, 3)
  }
}

/** The start pose the explorer returns to. */
export function drawHome(ctx: CanvasRenderingContext2D, t: Theme, view: View, home: Float32Array) {
  const p = at(view, home[0], home[1])
  ctx.save()
  ctx.beginPath()
  ctx.rect(p.x - 5, p.y - 5, 10, 10)
  ctx.lineWidth = 1.5
  ctx.strokeStyle = t.verdigris
  ctx.stroke()
  ctx.restore()
  label(ctx, t, 'home', { x: p.x, y: p.y + 14 }, t.verdigris)
}

/** The robot body: a disc with a heading line and the scanner at the front. */
export function drawRobot(
  ctx: CanvasRenderingContext2D,
  t: Theme,
  view: View,
  pose: Float32Array,
  radius: number,
  bumped: boolean,
) {
  const p = at(view, pose[0], pose[1])
  const r = Math.max(4, radius * view.scale)
  const ux = Math.cos(pose[2])
  const uy = Math.sin(pose[2])
  const color = bumped ? t.signal : t.ink
  ctx.save()
  ctx.beginPath()
  ctx.arc(p.x, p.y, r, 0, Math.PI * 2)
  ctx.fillStyle = t.paper
  ctx.fill()
  ctx.lineWidth = 2
  ctx.strokeStyle = color
  ctx.stroke()
  // Wheels across the axle.
  ctx.beginPath()
  for (const side of [-1, 1]) {
    const cx = p.x - uy * side * r * 0.85
    const cy = p.y + ux * side * r * 0.85
    ctx.moveTo(cx - ux * r * 0.35, cy - uy * r * 0.35)
    ctx.lineTo(cx + ux * r * 0.35, cy + uy * r * 0.35)
  }
  ctx.lineWidth = 3
  ctx.stroke()
  ctx.beginPath()
  ctx.moveTo(p.x, p.y)
  ctx.lineTo(p.x + ux * r, p.y + uy * r)
  ctx.lineWidth = 1.5
  ctx.stroke()
  ctx.beginPath()
  ctx.arc(p.x + ux * r * 0.6, p.y + uy * r * 0.6, Math.max(1.5, r * 0.18), 0, Math.PI * 2)
  ctx.fillStyle = t.cobalt
  ctx.fill()
  ctx.restore()
}

/** Reads the stats the readouts show. */
export function readStats(lab: RoverLab): RoverStats {
  const [coverage, distance, time, frontiers] = lab.stats()
  return { coverage, distance, time, frontiers, status: lab.status() as StatusId }
}
