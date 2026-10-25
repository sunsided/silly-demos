import type { Point } from '../../canvas/canvas.svelte.ts'
import { cross, label } from '../../canvas/draft.ts'
import type { Theme } from '../../canvas/theme.svelte.ts'

// Each point is [x, y, vx, vy]
export const STRIDE = 4

export interface VoronoiStats {
  triangles: number
  edges: number
  hover: string
}

function pointInTriangle(p: Point, a: Point, b: Point, c: Point): boolean {
  const s1 = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
  const s2 = (c.x - b.x) * (p.y - b.y) - (c.y - b.y) * (p.x - b.x)
  const s3 = (a.x - c.x) * (p.y - c.y) - (a.y - c.y) * (p.x - c.x)
  return (s1 >= 0 && s2 >= 0 && s3 >= 0) || (s1 <= 0 && s2 <= 0 && s3 <= 0)
}

function circumcircle(a: Point, b: Point, c: Point): { x: number; y: number; r: number } | null {
  const d = 2 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y))
  if (Math.abs(d) < 1e-8) return null
  const a2 = a.x * a.x + a.y * a.y
  const b2 = b.x * b.x + b.y * b.y
  const c2 = c.x * c.x + c.y * c.y
  const x = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d
  const y = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d
  return { x, y, r: Math.hypot(a.x - x, a.y - y) }
}

/** Position of point `i` in a flat point buffer. */
export const at = (pts: Float32Array, i: number): Point => ({ x: pts[i * STRIDE], y: pts[i * STRIDE + 1] })

/** Highlights the triangle under `m` with its circumcircle. Returns a description, or null. */
export function drawHoveredTriangle(ctx: CanvasRenderingContext2D, t: Theme, pts: Float32Array, tris: Uint32Array, m: Point) {
  for (let i = 0; i + 2 < tris.length; i += 3) {
    const a = at(pts, tris[i]), b = at(pts, tris[i + 1]), c = at(pts, tris[i + 2])
    if (!pointInTriangle(m, a, b, c)) continue
    ctx.save()
    ctx.beginPath()
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
    ctx.lineTo(c.x, c.y)
    ctx.closePath()
    ctx.fillStyle = t.signalFaint
    ctx.fill()
    ctx.lineWidth = 2
    ctx.strokeStyle = t.signal
    ctx.stroke()
    const cc = circumcircle(a, b, c)
    if (cc) {
      ctx.beginPath()
      ctx.arc(cc.x, cc.y, cc.r, 0, Math.PI * 2)
      ctx.setLineDash([4, 4])
      ctx.lineWidth = 1
      ctx.stroke()
      ctx.setLineDash([])
      cross(ctx, cc, t.signal)
    }
    ctx.restore()
    if (cc) label(ctx, t, 'empty circumcircle', { x: cc.x, y: cc.y - cc.r - 10 }, t.signal)
    return 'Triangle'
  }
  return null
}

/** Highlights the Voronoi cell of the site nearest to `m`. Returns a description. */
export function drawHoveredCell(ctx: CanvasRenderingContext2D, t: Theme, pts: Float32Array, tris: Uint32Array, m: Point) {
  let site = -1
  let best = Infinity
  for (let i = 0; i < pts.length / STRIDE; i++) {
    const d = (pts[i * STRIDE] - m.x) ** 2 + (pts[i * STRIDE + 1] - m.y) ** 2
    if (d < best) { best = d; site = i }
  }
  if (site < 0) return null
  const s = at(pts, site)

  // The cell's corners are the circumcenters of the triangles around the site.
  // The cell is bounded only if those triangles close a full fan, i.e. every
  // neighbor is shared by exactly two of them; hull sites have open cells.
  const corners: Point[] = []
  const neighborUses = new Map<number, number>()
  for (let i = 0; i + 2 < tris.length; i += 3) {
    const tri = [tris[i], tris[i + 1], tris[i + 2]]
    if (!tri.includes(site)) continue
    for (const v of tri) if (v !== site) neighborUses.set(v, (neighborUses.get(v) ?? 0) + 1)
    const cc = circumcircle(at(pts, tri[0]), at(pts, tri[1]), at(pts, tri[2]))
    if (cc) corners.push(cc)
  }
  const bounded = neighborUses.size > 0 && [...neighborUses.values()].every(n => n === 2)

  ctx.save()
  if (bounded && corners.length >= 3) {
    corners.sort((p, q) => Math.atan2(p.y - s.y, p.x - s.x) - Math.atan2(q.y - s.y, q.x - s.x))
    ctx.beginPath()
    ctx.moveTo(corners[0].x, corners[0].y)
    for (const p of corners.slice(1)) ctx.lineTo(p.x, p.y)
    ctx.closePath()
    ctx.fillStyle = t.cobaltFaint
    ctx.fill()
    ctx.lineWidth = 2
    ctx.strokeStyle = t.cobalt
    ctx.stroke()
  }
  ctx.beginPath()
  ctx.arc(s.x, s.y, 5, 0, Math.PI * 2)
  ctx.fillStyle = t.paper
  ctx.fill()
  ctx.lineWidth = 2
  ctx.strokeStyle = t.cobalt
  ctx.stroke()
  ctx.restore()
  if (!bounded) label(ctx, t, 'unbounded cell', { x: s.x + 10, y: s.y - 12 }, t.cobalt, 'left')
  return bounded ? `Cell of site ${site}` : `Open cell of site ${site}`
}
