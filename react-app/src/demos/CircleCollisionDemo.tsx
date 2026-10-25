import { useRef, useState } from 'react'
import { ControlGroup, Loading, Sheet, Slider } from '../components/Sheet'
import { useAnimationFrame, useCanvas, useDrag, type Point } from '../lib/canvas'
import { clear, cross, dimension, label } from '../lib/draft'
import { useTheme, type Theme } from '../lib/theme'
import { useWasm, type Wasm } from '../lib/wasm'

interface Circle {
  x: number
  y: number
  r: number
}

const patterns = new Map<string, CanvasPattern>()

function hatchPattern(ctx: CanvasRenderingContext2D, color: string, dpr: number) {
  const key = `${color}|${dpr}`
  const cached = patterns.get(key)
  if (cached) return cached
  const size = Math.round(8 * dpr)
  const tile = document.createElement('canvas')
  tile.width = tile.height = size
  const t = tile.getContext('2d')!
  t.strokeStyle = color
  t.lineWidth = dpr
  t.beginPath()
  t.moveTo(0, size)
  t.lineTo(size, 0)
  t.moveTo(-size / 2, size / 2)
  t.lineTo(size / 2, -size / 2)
  t.moveTo(size / 2, size * 1.5)
  t.lineTo(size * 1.5, size / 2)
  t.stroke()
  const pattern = ctx.createPattern(tile, 'repeat')!
  pattern.setTransform(new DOMMatrix().scale(1 / dpr))
  patterns.set(key, pattern)
  return pattern
}

function drawCircle(ctx: CanvasRenderingContext2D, theme: Theme, c: Circle, hit: boolean, name: string) {
  ctx.beginPath()
  ctx.arc(c.x, c.y, c.r, 0, Math.PI * 2)
  ctx.fillStyle = hit ? theme.signalFaint : theme.cobaltFaint
  ctx.fill()
  ctx.lineWidth = 2
  ctx.strokeStyle = hit ? theme.signal : theme.ink
  ctx.stroke()
  cross(ctx, c, hit ? theme.signal : theme.ink)
  label(ctx, theme, name, { x: c.x + 10, y: c.y - 12 }, hit ? theme.signal : theme.ink, 'left')
  // Radius dimension pointing up-left.
  const a = { x: c.x - c.r * Math.SQRT1_2, y: c.y + c.r * Math.SQRT1_2 }
  dimension(ctx, theme, c, a, `r ${Math.round(c.r)}`)
}

function CircleCollisionDemo() {
  const { wasm, error } = useWasm()
  return wasm ? <CircleCollision wasm={wasm} /> : <Loading error={error} />
}

function CircleCollision({ wasm }: { wasm: Wasm }) {
  const theme = useTheme()
  const [radii, setRadii] = useState({ a: 90, b: 110 })
  const [stats, setStats] = useState({ intersect: false, distance: 0, penetration: 0 })
  const circles = useRef<{ a: Circle; b: Circle }>({
    a: { x: -140, y: 0, r: radii.a },
    b: { x: 140, y: 0, r: radii.b },
  })
  circles.current.a.r = radii.a
  circles.current.b.r = radii.b

  // Fit the initial layout to narrow screens.
  const placed = useRef(false)
  const { ref, size } = useCanvas(({ width }) => {
    if (placed.current) return
    placed.current = true
    const k = Math.min(1, width / 640)
    if (k === 1) return
    circles.current.a.x *= k
    circles.current.b.x *= k
    setRadii({ a: Math.round(90 * k), b: Math.round(110 * k) })
  })
  const toLocal = (p: Point) => ({ x: p.x - size.current.width / 2, y: p.y - size.current.height / 2 })
  const inside = (c: Circle, p: Point) => (p.x - c.x) ** 2 + (p.y - c.y) ** 2 <= c.r * c.r

  useDrag(
    ref,
    toLocal,
    // Prefer the circle whose center is closer when both contain the pointer.
    p => {
      const { a, b } = circles.current
      const hits = [a, b].filter(c => inside(c, p))
      hits.sort((m, n) => Math.hypot(p.x - m.x, p.y - m.y) - Math.hypot(p.x - n.x, p.y - n.y))
      return hits[0] ?? null
    },
    (c, d) => { c.x += d.x; c.y += d.y },
  )

  useAnimationFrame(() => {
    const ctx = ref.current?.getContext('2d')
    if (!ctx || size.current.width === 0) return
    const t = theme.current
    const { a, b } = circles.current

    const result = wasm.CollisionTests.circle_collision(a.x, a.y, a.r, b.x, b.y, b.r)
    const next = { intersect: result.intersect, distance: result.distance, penetration: result.penetration }
    result.free()
    setStats(prev =>
      prev.intersect === next.intersect && prev.distance === next.distance && prev.penetration === next.penetration
        ? prev
        : next,
    )

    clear(ctx, size.current.dpr)
    ctx.translate(size.current.width / 2, size.current.height / 2)

    drawCircle(ctx, t, a, next.intersect, 'A')
    drawCircle(ctx, t, b, next.intersect, 'B')

    if (next.intersect) {
      // Hatch the overlapping lens.
      ctx.save()
      ctx.beginPath()
      ctx.arc(a.x, a.y, a.r, 0, Math.PI * 2)
      ctx.clip()
      ctx.beginPath()
      ctx.arc(b.x, b.y, b.r, 0, Math.PI * 2)
      ctx.fillStyle = hatchPattern(ctx, t.signal, size.current.dpr)
      ctx.fill()
      ctx.restore()
    }

    // Center distance, offset away from the circles' centers.
    dimension(ctx, t, a, b, `d ${next.distance.toFixed(1)}`, { offset: -Math.max(a.r, b.r) - 28 })

    if (next.distance > 0) {
      const ux = (b.x - a.x) / next.distance
      const uy = (b.y - a.y) / next.distance
      const edgeA = { x: a.x + ux * a.r, y: a.y + uy * a.r }
      const edgeB = { x: b.x - ux * b.r, y: b.y - uy * b.r }
      const text = next.intersect ? `overlap ${next.penetration.toFixed(1)}` : `gap ${(next.distance - a.r - b.r).toFixed(1)}`
      // Below both circles, opposite the center distance.
      dimension(ctx, t, edgeA, edgeB, text, {
        offset: Math.max(a.r, b.r) + 28,
        color: next.intersect ? t.signal : t.cobalt,
      })
    }
  })

  return (
    <Sheet
      title="Circle collision"
      summary="Two circles intersect when the distance between their centers is at most the sum of their radii. Drag either circle."
      readouts={[
        { label: 'Intersecting', value: stats.intersect ? 'Yes' : 'No', tone: stats.intersect ? 'signal' : undefined },
        { label: 'Center distance', value: stats.distance.toFixed(1) },
        { label: 'Sum of radii', value: radii.a + radii.b },
        { label: 'Penetration', value: stats.penetration.toFixed(1), tone: stats.intersect ? 'signal' : undefined },
      ]}
      canvas={<canvas ref={ref} className="drawing-canvas" role="img" aria-label="Two draggable circles with distance dimensions" />}
    >
      <ControlGroup title="Radii">
        <Slider label="Circle A" unit="px" value={radii.a} min={10} max={200} onChange={a => setRadii(r => ({ ...r, a }))} />
        <Slider label="Circle B" unit="px" value={radii.b} min={10} max={200} onChange={b => setRadii(r => ({ ...r, b }))} />
      </ControlGroup>
    </Sheet>
  )
}

export default CircleCollisionDemo
