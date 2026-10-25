import { useEffect, useRef, useState } from 'react'
import { ControlGroup, Loading, Sheet, Slider, Toggle } from '../components/Sheet'
import { useAnimationFrame, useCanvas, type Point } from '../lib/canvas'
import { clear, cross, label } from '../lib/draft'
import { useTheme, type Theme } from '../lib/theme'
import { useWasm, type Wasm } from '../lib/wasm'

// Each point is [x, y, vx, vy]
const STRIDE = 4

interface Stats {
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

const at = (pts: Float32Array, i: number): Point => ({ x: pts[i * STRIDE], y: pts[i * STRIDE + 1] })

/** Highlights the triangle under `m` with its circumcircle. Returns a description, or null. */
function drawHoveredTriangle(ctx: CanvasRenderingContext2D, t: Theme, pts: Float32Array, tris: Uint32Array, m: Point) {
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
function drawHoveredCell(ctx: CanvasRenderingContext2D, t: Theme, pts: Float32Array, tris: Uint32Array, m: Point) {
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

function VoronoiDemo() {
  const { wasm, error } = useWasm()
  return wasm ? <Voronoi wasm={wasm} /> : <Loading error={error} />
}

function Voronoi({ wasm }: { wasm: Wasm }) {
  const theme = useTheme()
  const [count, setCount] = useState(120)
  const [speed, setSpeed] = useState(12)
  const [seed, setSeed] = useState(1337)
  const [running, setRunning] = useState(true)
  const [showDelaunay, setShowDelaunay] = useState(true)
  const [showVoronoi, setShowVoronoi] = useState(true)
  const [stats, setStats] = useState<Stats>({ triangles: 0, edges: 0, hover: '–' })

  const points = useRef<Float32Array>(new Float32Array())
  const pointer = useRef<Point | null>(null)
  const params = useRef({ count, speed, seed })
  params.current = { count, speed, seed }

  const spawn = (width: number, height: number) => {
    const { count, speed, seed } = params.current
    points.current = wasm.VoronoiTests.voronoi_create_points(count, width, height, seed >>> 0, speed)
  }

  const { ref, size } = useCanvas(({ width, height }) => {
    if (points.current.length === 0) spawn(width, height)
  })

  useEffect(() => {
    if (size.current.width > 0) spawn(size.current.width, size.current.height)
    // `spawn` reads the latest params from a ref.
  }, [count, speed, seed])

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const move = (e: PointerEvent) => {
      const rect = el.getBoundingClientRect()
      pointer.current = { x: e.clientX - rect.left, y: e.clientY - rect.top }
    }
    const leave = () => { pointer.current = null }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerleave', leave)
    return () => {
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerleave', leave)
    }
  }, [ref])

  useAnimationFrame((_, dt) => {
    const ctx = ref.current?.getContext('2d')
    const { width, height, dpr } = size.current
    if (!ctx || width === 0 || points.current.length === 0) return
    const t = theme.current

    if (running) {
      points.current = wasm.VoronoiTests.voronoi_step_points(points.current, width, height, Math.min(dt, 0.05))
    }
    const pts = points.current
    const tris = wasm.VoronoiTests.delaunay_indices(pts)
    const edges = showVoronoi ? wasm.VoronoiTests.voronoi_edges(pts) : new Float32Array()

    clear(ctx, dpr)

    if (showDelaunay) {
      ctx.beginPath()
      for (let i = 0; i + 2 < tris.length; i += 3) {
        const a = at(pts, tris[i]), b = at(pts, tris[i + 1]), c = at(pts, tris[i + 2])
        ctx.moveTo(a.x, a.y)
        ctx.lineTo(b.x, b.y)
        ctx.lineTo(c.x, c.y)
        ctx.closePath()
      }
      ctx.lineWidth = 1
      ctx.strokeStyle = t.inkFaint
      ctx.stroke()
    }

    if (showVoronoi) {
      ctx.beginPath()
      for (let i = 0; i + 3 < edges.length; i += 4) {
        ctx.moveTo(edges[i], edges[i + 1])
        ctx.lineTo(edges[i + 2], edges[i + 3])
      }
      ctx.lineWidth = 1.5
      ctx.strokeStyle = t.cobalt
      ctx.stroke()
    }

    ctx.beginPath()
    for (let i = 0; i < pts.length; i += STRIDE) {
      ctx.moveTo(pts[i] + 2.5, pts[i + 1])
      ctx.arc(pts[i], pts[i + 1], 2.5, 0, Math.PI * 2)
    }
    ctx.fillStyle = t.ink
    ctx.fill()

    let hover = '–'
    const m = pointer.current
    if (m) {
      // Prefer the triangle view when only the triangulation is shown.
      hover = (showDelaunay && !showVoronoi
        ? drawHoveredTriangle(ctx, t, pts, tris, m)
        : drawHoveredCell(ctx, t, pts, tris, m)) ?? 'Outside the hull'
    }

    const next = { triangles: tris.length / 3, edges: edges.length / 4, hover }
    setStats(prev =>
      prev.triangles === next.triangles && prev.edges === next.edges && prev.hover === next.hover ? prev : next,
    )
  })

  return (
    <Sheet
      title="Voronoi & Delaunay"
      summary={
        <>
          The Delaunay triangulation connects points so that no point lies inside any triangle’s
          circumcircle. Joining the circumcenters of neighboring triangles gives the Voronoi diagram:
          each cell holds the area closest to one point. Hover to inspect a cell or triangle.
        </>
      }
      readouts={[
        { label: 'Moving points', value: count },
        { label: 'Triangles', value: stats.triangles },
        { label: 'Voronoi edges', value: showVoronoi ? stats.edges : '–', tone: 'cobalt' },
        { label: 'Under pointer', value: stats.hover },
      ]}
      canvas={<canvas ref={ref} className="drawing-canvas" role="img" aria-label="Moving points with their Delaunay triangulation and Voronoi diagram" />}
      notes={
        <p>
          Show only the triangulation to inspect triangles and their circumcircles; with the
          Voronoi diagram shown, hovering picks the cell of the nearest point.
        </p>
      }
    >
      <div className="button-row">
        <button type="button" className="button primary" onClick={() => setRunning(r => !r)}>
          {running ? 'Pause' : 'Resume'}
        </button>
        <button type="button" className="button" onClick={() => setSeed(s => (s * 1103515245 + 12345) >>> 0)}>
          New points
        </button>
      </div>
      <Toggle label="Delaunay triangulation" checked={showDelaunay} onChange={setShowDelaunay} />
      <Toggle label="Voronoi diagram" checked={showVoronoi} onChange={setShowVoronoi} />

      <ControlGroup title="Points">
        <Slider label="Count" value={count} min={3} max={2000} step={1} onChange={setCount} />
        <Slider label="Speed" unit="px/s" value={speed} min={0} max={200} onChange={setSpeed} />
      </ControlGroup>
    </Sheet>
  )
}

export default VoronoiDemo
