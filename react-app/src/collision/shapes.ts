import type { Point } from '../lib/canvas'

export const SHAPE_KINDS = ['circle', 'aabb', 'obb', 'triangle', 'polygon', 'ellipse', 'capsule'] as const
export type ShapeKind = (typeof SHAPE_KINDS)[number]

export const SHAPE_NAMES: Record<ShapeKind, string> = {
  circle: 'Circle',
  aabb: 'Axis-aligned box',
  obb: 'Oriented box',
  triangle: 'Triangle',
  polygon: 'Convex polygon',
  ellipse: 'Ellipse',
  capsule: 'Capsule',
}

export const isShapeKind = (v: string | null): v is ShapeKind => SHAPE_KINDS.includes(v as ShapeKind)

/**
 * Editable shape. Every kind keeps a center `c` the body drags by; the other
 * fields are used as the kind needs them.
 */
export interface ShapeState {
  kind: ShapeKind
  c: Point
  /** Rotation in radians (OBB, ellipse); for polygons only where the rotation handle sits. */
  angle: number
  /** Half extents (boxes) or semi-axes (ellipse). */
  hw: number
  hh: number
  /** Radius (circle, capsule). */
  r: number
  /** Vertices (triangle, polygon) or core end points (capsule), relative to `c`. */
  pts: Point[]
}

/** A draggable control point; `set` moves it to `p`. */
export interface Handle {
  at: Point
  set: (s: ShapeState, p: Point) => void
  /** Draw a guide line from the center to the handle. */
  guide?: boolean
}

const MIN_SIZE = 6

const rotate = (v: Point, a: number): Point => ({
  x: v.x * Math.cos(a) - v.y * Math.sin(a),
  y: v.x * Math.sin(a) + v.y * Math.cos(a),
})
const add = (a: Point, b: Point): Point => ({ x: a.x + b.x, y: a.y + b.y })
const sub = (a: Point, b: Point): Point => ({ x: a.x - b.x, y: a.y - b.y })

/** A fresh shape of `kind` around `c`, scaled by `k` for small screens. */
export function defaultShape(kind: ShapeKind, c: Point, k = 1): ShapeState {
  const s = (v: number) => Math.round(v * k)
  const base: ShapeState = { kind, c: { ...c }, angle: 0, hw: s(90), hh: s(65), r: s(90), pts: [] }
  switch (kind) {
    case 'obb':
      return { ...base, hw: s(95), hh: s(55), angle: 0.5 }
    case 'ellipse':
      return { ...base, hw: s(110), hh: s(60), angle: -0.4 }
    case 'triangle':
      return { ...base, pts: centered([{ x: 0, y: s(-90) }, { x: s(85), y: s(60) }, { x: s(-85), y: s(60) }]) }
    case 'polygon':
      return {
        ...base,
        pts: centered([0, 1, 2, 3, 4, 5].map(i => {
          const t = (i / 6) * Math.PI * 2 + 0.3
          const r = s(i % 2 ? 95 : 75)
          return { x: Math.cos(t) * r, y: Math.sin(t) * r }
        })),
      }
    case 'capsule':
      return { ...base, r: s(40), pts: [{ x: s(-70), y: s(-40) }, { x: s(70), y: s(40) }] }
    default:
      return base
  }
}

/** Flat parameters in the layout `CollisionLab.set_shape` expects. */
export function params(s: ShapeState): Float32Array {
  const world = s.pts.flatMap(p => [s.c.x + p.x, s.c.y + p.y])
  switch (s.kind) {
    case 'circle':
      return new Float32Array([s.c.x, s.c.y, s.r])
    case 'aabb':
      return new Float32Array([s.c.x, s.c.y, s.hw, s.hh])
    case 'obb':
    case 'ellipse':
      return new Float32Array([s.c.x, s.c.y, s.hw, s.hh, s.angle])
    case 'triangle':
    case 'polygon':
      return new Float32Array(world)
    case 'capsule':
      return new Float32Array([...world, s.r])
  }
}

/** Rotation handle sitting `reach` px out along the local "up" direction. */
function rotator(s: ShapeState, reach: number): Handle {
  return {
    at: add(s.c, rotate({ x: 0, y: -reach }, s.angle)),
    set: (s, p) => {
      const d = sub(p, s.c)
      s.angle = Math.atan2(d.y, d.x) + Math.PI / 2
    },
    guide: true,
  }
}

export function handles(s: ShapeState): Handle[] {
  const local = (p: Point) => rotate(sub(p, s.c), -s.angle)
  switch (s.kind) {
    case 'circle':
      return [{
        at: add(s.c, { x: s.r * Math.SQRT1_2, y: -s.r * Math.SQRT1_2 }),
        set: (s, p) => { s.r = Math.max(MIN_SIZE, Math.hypot(p.x - s.c.x, p.y - s.c.y)) },
      }]
    case 'aabb':
      return [{
        at: add(s.c, { x: s.hw, y: -s.hh }),
        set: (s, p) => {
          s.hw = Math.max(MIN_SIZE, Math.abs(p.x - s.c.x))
          s.hh = Math.max(MIN_SIZE, Math.abs(p.y - s.c.y))
        },
      }]
    case 'obb':
      return [
        {
          at: add(s.c, rotate({ x: s.hw, y: s.hh }, s.angle)),
          set: (s, p) => {
            const l = local(p)
            s.hw = Math.max(MIN_SIZE, Math.abs(l.x))
            s.hh = Math.max(MIN_SIZE, Math.abs(l.y))
          },
        },
        rotator(s, s.hh + 32),
      ]
    case 'ellipse':
      return [
        {
          at: add(s.c, rotate({ x: s.hw, y: 0 }, s.angle)),
          set: (s, p) => { s.hw = Math.max(MIN_SIZE, Math.abs(local(p).x)) },
        },
        {
          at: add(s.c, rotate({ x: 0, y: s.hh }, s.angle)),
          set: (s, p) => { s.hh = Math.max(MIN_SIZE, Math.abs(local(p).y)) },
        },
        rotator(s, s.hh + 32),
      ]
    case 'triangle':
    case 'polygon': {
      const reach = Math.max(...s.pts.map(v => Math.hypot(v.x, v.y))) + 28
      return [
        ...s.pts.map((v, i): Handle => ({ at: add(s.c, v), set: (s, p) => moveVertex(s, i, p) })),
        {
          at: add(s.c, rotate({ x: 0, y: -reach }, s.angle)),
          set: (s, p) => {
            const d = sub(p, s.c)
            const angle = Math.atan2(d.y, d.x) + Math.PI / 2
            s.pts = s.pts.map(v => rotate(v, angle - s.angle))
            s.angle = angle
          },
          guide: true,
        },
      ]
    }
    case 'capsule': {
      const [p0, p1] = s.pts
      const d = sub(p1, p0)
      const len = Math.hypot(d.x, d.y) || 1
      const mid = add(s.c, { x: (p0.x + p1.x) / 2, y: (p0.y + p1.y) / 2 })
      return [
        ...s.pts.map((v, i): Handle => ({ at: add(s.c, v), set: (s, p) => moveVertex(s, i, p) })),
        {
          at: add(mid, { x: (-d.y / len) * s.r, y: (d.x / len) * s.r }),
          set: (s, p) => {
            const [a, b] = s.pts.map(v => add(s.c, v))
            const e = sub(b, a)
            const l = Math.hypot(e.x, e.y) || 1
            s.r = Math.max(MIN_SIZE, Math.abs((e.x * (p.y - a.y) - e.y * (p.x - a.x)) / l))
          },
        },
      ]
    }
  }
}

/**
 * Moves vertex `i` to `p`, then re-centers so `c` stays the vertex mean the
 * body drags and rotates about.
 */
function moveVertex(s: ShapeState, i: number, p: Point) {
  const world = s.pts.map(v => add(s.c, v))
  world[i] = p
  s.c = mean(world)
  s.pts = world.map(v => sub(v, s.c))
}

const mean = (pts: Point[]): Point =>
  pts.reduce((m, v) => ({ x: m.x + v.x / pts.length, y: m.y + v.y / pts.length }), { x: 0, y: 0 })

/** Shifts points so their mean is the origin. */
function centered(pts: Point[]): Point[] {
  const m = mean(pts)
  return pts.map(v => sub(v, m))
}

/** Moves the whole shape. Vertices are stored relative to the center, so only it changes. */
export function translate(s: ShapeState, d: Point) {
  s.c.x += d.x
  s.c.y += d.y
}

/** Even-odd point-in-polygon test against a flat outline. */
export function insideOutline(outline: Float32Array, p: Point) {
  let inside = false
  const n = outline.length / 2
  for (let i = 0, j = n - 1; i < n; j = i++) {
    const [xi, yi, xj, yj] = [outline[i * 2], outline[i * 2 + 1], outline[j * 2], outline[j * 2 + 1]]
    if (yi > p.y !== yj > p.y && p.x < ((xj - xi) * (p.y - yi)) / (yj - yi) + xi) inside = !inside
  }
  return inside
}
