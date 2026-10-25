import { useRef, useState } from 'react'
import { Loading, Sheet } from '../components/Sheet'
import { useAnimationFrame, useCanvas, useDrag, type Point } from '../lib/canvas'
import { clear, cross, dimension, handle, label } from '../lib/draft'
import { useTheme, type Theme } from '../lib/theme'
import { useWasm, type Wasm } from '../lib/wasm'

type HandleName = 'a' | 'b' | 'p'
type Closest = 'interior' | 'a' | 'b'

interface Stats {
  distance: number
  perpendicular: number
  side: number
  closest: Closest
}

const HIT_RADIUS = 14

/** Small square marking a right angle at `at`, between directions `u` and `v`. */
function rightAngle(ctx: CanvasRenderingContext2D, at: Point, u: Point, v: Point, color: string, s = 9) {
  ctx.beginPath()
  ctx.moveTo(at.x + u.x * s, at.y + u.y * s)
  ctx.lineTo(at.x + (u.x + v.x) * s, at.y + (u.y + v.y) * s)
  ctx.lineTo(at.x + v.x * s, at.y + v.y * s)
  ctx.strokeStyle = color
  ctx.lineWidth = 1
  ctx.stroke()
}

function sideColor(theme: Theme, side: number) {
  return side > 0 ? theme.verdigris : side < 0 ? theme.cobalt : theme.signal
}

function PointLineDemo() {
  const { wasm, error } = useWasm()
  return wasm ? <PointLine wasm={wasm} /> : <Loading error={error} />
}

function PointLine({ wasm }: { wasm: Wasm }) {
  const theme = useTheme()
  const [stats, setStats] = useState<Stats>({ distance: 0, perpendicular: 0, side: 0, closest: 'interior' })
  const pts = useRef<Record<HandleName, Point>>({
    a: { x: -160, y: 60 },
    b: { x: 160, y: -40 },
    p: { x: 30, y: -110 },
  })

  // Fit the initial layout to narrow screens.
  const placed = useRef(false)
  const { ref, size } = useCanvas(({ width }) => {
    if (placed.current) return
    placed.current = true
    const k = Math.min(1, width / 480)
    for (const q of Object.values(pts.current)) {
      q.x *= k
      q.y *= k
    }
  })
  useDrag<HandleName>(
    ref,
    p => ({ x: p.x - size.current.width / 2, y: p.y - size.current.height / 2 }),
    p => {
      // The test point wins over the segment's end points when they overlap.
      for (const name of ['p', 'a', 'b'] as const) {
        const h = pts.current[name]
        if (Math.hypot(p.x - h.x, p.y - h.y) <= HIT_RADIUS) return name
      }
      return null
    },
    (name, d) => {
      pts.current[name].x += d.x
      pts.current[name].y += d.y
    },
  )

  useAnimationFrame(() => {
    const ctx = ref.current?.getContext('2d')
    if (!ctx || size.current.width === 0) return
    const t = theme.current
    const { a, b, p } = pts.current

    const r = wasm.GeometryTests.point_line_test(a.x, a.y, b.x, b.y, p.x, p.y)
    const c = { x: r.closest_x, y: r.closest_y }
    const distance = r.distance
    const side = r.side
    const onSegment = r.on_segment
    r.free()

    // Foot of the perpendicular on the infinite line.
    const dx = b.x - a.x
    const dy = b.y - a.y
    const len = Math.hypot(dx, dy)
    const u = len > 0 ? { x: dx / len, y: dy / len } : { x: 1, y: 0 }
    const along = (p.x - a.x) * u.x + (p.y - a.y) * u.y
    const foot = { x: a.x + u.x * along, y: a.y + u.y * along }
    const perpendicular = Math.hypot(p.x - foot.x, p.y - foot.y)

    const closest: Closest = onSegment ? 'interior' : along < 0 ? 'a' : 'b'
    setStats(prev =>
      prev.distance === distance && prev.perpendicular === perpendicular && prev.side === side && prev.closest === closest
        ? prev
        : { distance, perpendicular, side, closest },
    )

    clear(ctx, size.current.dpr)
    const w = size.current.width
    const h = size.current.height
    ctx.translate(w / 2, h / 2)

    // Infinite line through the segment.
    const ext = Math.hypot(w, h)
    ctx.beginPath()
    ctx.moveTo(a.x - u.x * ext, a.y - u.y * ext)
    ctx.lineTo(b.x + u.x * ext, b.y + u.y * ext)
    ctx.setLineDash([10, 6, 2, 6])
    ctx.lineWidth = 1
    ctx.strokeStyle = t.inkFaint
    ctx.stroke()
    ctx.setLineDash([])

    // The segment itself, with direction arrow.
    ctx.beginPath()
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
    ctx.lineWidth = 2.5
    ctx.strokeStyle = t.ink
    ctx.stroke()
    if (len > 40) {
      const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }
      ctx.beginPath()
      ctx.moveTo(mid.x + u.x * 8, mid.y + u.y * 8)
      ctx.lineTo(mid.x - u.x * 4 - u.y * 6, mid.y - u.y * 4 + u.x * 6)
      ctx.lineTo(mid.x - u.x * 4 + u.y * 6, mid.y - u.y * 4 - u.x * 6)
      ctx.closePath()
      ctx.fillStyle = t.ink
      ctx.fill()
    }

    const pc = sideColor(t, side)

    // Perpendicular to the infinite line, when it misses the segment.
    if (!onSegment && perpendicular > 1) {
      ctx.beginPath()
      ctx.moveTo(p.x, p.y)
      ctx.lineTo(foot.x, foot.y)
      ctx.setLineDash([3, 4])
      ctx.strokeStyle = t.inkSoft
      ctx.lineWidth = 1
      ctx.stroke()
      ctx.setLineDash([])
      const n = { x: (p.x - foot.x) / perpendicular, y: (p.y - foot.y) / perpendicular }
      rightAngle(ctx, foot, n, along < 0 ? u : { x: -u.x, y: -u.y }, t.inkSoft)
      cross(ctx, foot, t.inkSoft, 4)
      label(ctx, t, `⊥ ${perpendicular.toFixed(1)}`, {
        x: (p.x + foot.x) / 2 + u.x * 34 * (along < 0 ? -1 : 1),
        y: (p.y + foot.y) / 2 + u.y * 34 * (along < 0 ? -1 : 1),
      }, t.inkSoft)
    }

    // Shortest distance to the segment.
    if (distance > 1) {
      dimension(ctx, t, p, c, distance.toFixed(1), { color: pc })
      if (onSegment) {
        const n = { x: (p.x - c.x) / distance, y: (p.y - c.y) / distance }
        rightAngle(ctx, c, n, u, pc)
      }
    }

    handle(ctx, a, t.ink, t.paper, 6)
    handle(ctx, b, t.ink, t.paper, 6)
    handle(ctx, p, pc, t.paper, 8)
    cross(ctx, c, pc, 4)

    const off = (q: Point, k = 20) => ({ x: q.x - u.y * k, y: q.y + u.x * k })
    label(ctx, t, 'A', off(a, -18))
    label(ctx, t, 'B', off(b, -18))
    label(ctx, t, 'P', { x: p.x + 16, y: p.y - 14 }, pc, 'left')
  })

  const sideText = stats.side > 0 ? 'Right' : stats.side < 0 ? 'Left' : 'On the line'
  const closestText = { interior: 'Between A, B', a: 'End A', b: 'End B' }[stats.closest]

  return (
    <Sheet
      title="Point to segment"
      summary="The shortest distance from P to the segment AB, and how it differs from the perpendicular distance to the infinite line. Drag A, B or P."
      readouts={[
        { label: 'Distance', value: stats.distance.toFixed(1), tone: stats.side > 0 ? 'verdigris' : stats.side < 0 ? 'cobalt' : 'signal' },
        { label: 'Perpendicular', value: stats.perpendicular.toFixed(1) },
        { label: 'Side of A→B', value: sideText, tone: stats.side > 0 ? 'verdigris' : stats.side < 0 ? 'cobalt' : 'signal' },
        { label: 'Closest at', value: closestText },
      ]}
      canvas={<canvas ref={ref} className="drawing-canvas" role="img" aria-label="Draggable line segment and test point with distance dimensions" />}
      notes={
        <p>
          When P projects past an end point, the nearest point is that end point, and the
          dotted perpendicular to the infinite line is shorter than the true distance.
        </p>
      }
    />
  )
}

export default PointLineDemo
