import { useEffect, useRef, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Choice, ControlGroup, Loading, Select, Sheet, Slider, Toggle } from '../components/Sheet'
import { drawNotes } from '../collision/notes'
import {
  SHAPE_KINDS,
  SHAPE_NAMES,
  defaultShape,
  handles,
  insideOutline,
  isShapeKind,
  params,
  translate,
  type ShapeKind,
  type ShapeState,
} from '../collision/shapes'
import { useAnimationFrame, useCanvas, useDrag, type Point } from '../lib/canvas'
import { clear, cross, handle, hatchPattern, label } from '../lib/draft'
import { useTheme, type Theme } from '../lib/theme'
import { useWasm, type Wasm } from '../lib/wasm'

type Lab = InstanceType<Wasm['CollisionLab']>

/** Broadphase flags, as in `src/collision/broadphase.rs`. */
const BOUNDING_CIRCLE = 1
const BOUNDING_BOX = 2
const QUADTREE = 4
/** Indexed by `CollisionReport.stage`. */
const STAGES = ['Bounding circle', 'Bounding box', 'Full test', 'Quadtree']
const FULL_TEST = 2
/** Ground truth for the comparison table. */
const REFERENCE = 'gjk-epa'
const HANDLE_RADIUS = 10

interface Row {
  id: string
  intersect: boolean
  distance: number | undefined
  stage: number
  /** `[dots, sqrts, divs, axes, supports, iterations]` */
  counts: number[]
}

interface Drag {
  slot: number
  /** Index into `handles(shape)`, or null to move the whole shape. */
  handle: number | null
  pos: Point
}

const sameRows = (a: Row[], b: Row[]) =>
  a.length === b.length &&
  a.every((r, i) => {
    const o = b[i]
    return r.id === o.id && r.intersect === o.intersect && r.distance === o.distance && r.stage === o.stage &&
      r.counts.every((c, k) => c === o.counts[k])
  })

const formatDistance = (d: number | undefined) =>
  d === undefined ? '—' : d > 0 ? `gap ${d.toFixed(1)}` : `overlap ${(-d).toFixed(1)}`

/** Quadtree numbers, as returned by `CollisionReport.scene()`. */
interface SceneStats {
  bodies: number
  nodes: number
  depth: number
  checks: number
  candidates: number
  allPairs: number
  partners: number
  boxOverlaps: number
  collisions: number
}

function readScene(raw: Uint32Array): SceneStats | null {
  if (raw.length < 9) return null
  const [bodies, nodes, depth, checks, candidates, allPairs, partners, boxOverlaps, collisions] = raw
  return { bodies, nodes, depth, checks, candidates, allPairs, partners, boxOverlaps, collisions }
}

/**
 * Nanoseconds per call of `run(n)`, which must do its work `n` times.
 * Browsers coarsen performance.now(), so the batch grows until it takes a few ms.
 */
function timePerRun(run: (n: number) => void) {
  let n = 16
  for (;;) {
    const t0 = performance.now()
    run(n)
    const elapsed = performance.now() - t0
    if (elapsed > 4 || n >= 1 << 22) return (elapsed * 1e6) / n
    n *= 4
  }
}

function drawCrowd(ctx: CanvasRenderingContext2D, t: Theme, buf: Float32Array) {
  // Ghosted: only the quadtree's notes pick out the bodies that matter to A and B.
  ctx.save()
  ctx.strokeStyle = t.inkFaint
  ctx.lineWidth = 1
  for (let i = 0; i < buf.length;) {
    const n = buf[i]
    tracePath(ctx, buf.subarray(i + 1, i + 1 + n * 2))
    ctx.stroke()
    i += 1 + n * 2
  }
  ctx.restore()
}

/** Percentage with a non-breaking space, so the sign never wraps onto its own line. */
const percent = (part: number, whole: number) => `${((100 * part) / Math.max(1, whole)).toFixed(1)}\u00a0%`

const formatTime = (ns: number | undefined) =>
  ns === undefined ? '…'
    : ns >= 1000 ? `${(ns / 1000).toFixed(ns >= 10000 ? 0 : 1)} µs`
    : ns < 10 ? `${ns.toFixed(1)} ns`
    : `${Math.round(ns)} ns`

function tracePath(ctx: CanvasRenderingContext2D, outline: Float32Array) {
  ctx.beginPath()
  ctx.moveTo(outline[0], outline[1])
  for (let i = 2; i < outline.length; i += 2) ctx.lineTo(outline[i], outline[i + 1])
  ctx.closePath()
}

function drawShape(ctx: CanvasRenderingContext2D, t: Theme, outline: Float32Array, s: ShapeState, hit: boolean, name: string) {
  if (outline.length < 4) return
  tracePath(ctx, outline)
  ctx.fillStyle = hit ? t.signalFaint : t.cobaltFaint
  ctx.fill()
  ctx.lineWidth = 2
  ctx.lineJoin = 'round'
  ctx.strokeStyle = hit ? t.signal : t.ink
  ctx.stroke()
  if (s.kind === 'capsule') {
    // Core segment, the capsule's skeleton.
    const [p0, p1] = s.pts
    ctx.save()
    ctx.setLineDash([2, 4])
    ctx.lineWidth = 1
    ctx.beginPath()
    ctx.moveTo(s.c.x + p0.x, s.c.y + p0.y)
    ctx.lineTo(s.c.x + p1.x, s.c.y + p1.y)
    ctx.stroke()
    ctx.restore()
  }
  cross(ctx, s.c, hit ? t.signal : t.ink)
  let top = Infinity
  for (let i = 1; i < outline.length; i += 2) top = Math.min(top, outline[i])
  label(ctx, t, name, { x: s.c.x + 10, y: top - 12 }, hit ? t.signal : t.ink, 'left')
}

function drawHandles(ctx: CanvasRenderingContext2D, t: Theme, s: ShapeState) {
  for (const h of handles(s)) {
    if (h.guide) {
      ctx.save()
      ctx.setLineDash([2, 4])
      ctx.strokeStyle = t.inkSoft
      ctx.beginPath()
      ctx.moveTo(s.c.x, s.c.y)
      ctx.lineTo(h.at.x, h.at.y)
      ctx.stroke()
      ctx.restore()
    }
    handle(ctx, h.at, t.ink, t.paper, 6)
  }
}

function CollisionLabDemo() {
  const { wasm, error } = useWasm()
  return wasm ? <CollisionLab wasm={wasm} /> : <Loading error={error} />
}

function CollisionLab({ wasm }: { wasm: Wasm }) {
  const theme = useTheme()
  const [search, setSearch] = useSearchParams()
  const kinds: [ShapeKind, ShapeKind] = [
    isShapeKind(search.get('a')) ? (search.get('a') as ShapeKind) : 'circle',
    isShapeKind(search.get('b')) ? (search.get('b') as ShapeKind) : 'obb',
  ]
  const [broadphase, setBroadphase] = useState(0)
  const [rows, setRows] = useState<Row[]>([])
  const [timings, setTimings] = useState<Record<string, number>>({})
  const [crowdSetting, setCrowd] = useState(150)
  const [scene, setScene] = useState<SceneStats | null>(null)
  const [sceneTimes, setSceneTimes] = useState<{ quadtree: number; brute: number } | null>(null)
  const [reducedMotion] = useState(() => window.matchMedia('(prefers-reduced-motion: reduce)').matches)

  const ids = rows.map(r => r.id)
  const wanted = search.get('m')
  const method = wanted && ids.includes(wanted) ? wanted : ids[0]

  const setParam = (key: string, value: string) =>
    setSearch(prev => {
      const next = new URLSearchParams(prev)
      next.set(key, value)
      return next
    }, { replace: true })

  // Bumped on every shape edit; the frame loop only recomputes when it changed.
  const version = useRef(0)
  /** Results for the last computed state, redrawn as-is while nothing changes. */
  const computed = useRef<{ key: string; rows: Row[] } | null>(null)

  // Created in an effect so StrictMode's mount/unmount/mount cycle frees and recreates it.
  const lab = useRef<Lab | null>(null)
  useEffect(() => {
    const l = new wasm.CollisionLab()
    lab.current = l
    computed.current = null
    return () => {
      lab.current = null
      l.free()
    }
  }, [wasm])

  const scale = useRef(1)
  const shapes = useRef<ShapeState[]>([
    defaultShape(kinds[0], { x: -140, y: 0 }),
    defaultShape(kinds[1], { x: 140, y: 0 }),
  ])
  // A new kind replaces the shape in place.
  for (const slot of [0, 1]) {
    const s = shapes.current[slot]
    if (s.kind !== kinds[slot]) {
      shapes.current[slot] = defaultShape(kinds[slot], s.c, scale.current)
      version.current++
    }
  }
  const outlines = useRef<Float32Array[]>([new Float32Array(), new Float32Array()])

  // Fit the initial layout to narrow screens.
  const placed = useRef(false)
  const { ref, size } = useCanvas(({ width }) => {
    if (placed.current) return
    placed.current = true
    const k = Math.min(1, width / 700)
    if (k === 1) return
    scale.current = k
    shapes.current = shapes.current.map(s => defaultShape(s.kind, { x: s.c.x * k, y: s.c.y * k }, k))
    version.current++
  })
  const toLocal = (p: Point) => ({ x: p.x - size.current.width / 2, y: p.y - size.current.height / 2 })

  useDrag<Drag>(
    ref,
    toLocal,
    p => {
      // Handles win over bodies; B is drawn on top, so it wins ties.
      for (const slot of [1, 0]) {
        const hs = handles(shapes.current[slot])
        const i = hs.findIndex(h => Math.hypot(h.at.x - p.x, h.at.y - p.y) <= HANDLE_RADIUS)
        if (i >= 0) return { slot, handle: i, pos: hs[i].at }
      }
      for (const slot of [1, 0]) {
        if (insideOutline(outlines.current[slot], p)) return { slot, handle: null, pos: p }
      }
      return null
    },
    (drag, d) => {
      const s = shapes.current[drag.slot]
      version.current++
      if (drag.handle === null) return translate(s, d)
      drag.pos = { x: drag.pos.x + d.x, y: drag.pos.y + d.y }
      handles(s)[drag.handle].set(s, drag.pos)
    },
  )

  // Timing waits until the pose has settled, then runs every method in a tight loop.
  const pose = useRef('')
  const benchTimer = useRef(0)
  useEffect(() => () => clearTimeout(benchTimer.current), [])
  const scheduleBench = (methods: string[], mask: number) => {
    clearTimeout(benchTimer.current)
    setTimings(prev => (Object.keys(prev).length ? {} : prev))
    benchTimer.current = window.setTimeout(() => {
      const l = lab.current
      if (!l) return
      const result: Record<string, number> = {}
      for (const id of methods) result[id] = timePerRun(n => l.bench(id, mask, n))
      setTimings(result)
    }, 250)
  }

  const notes = useRef<{ buf: Float32Array; labels: string[] }>({ buf: new Float32Array(), labels: [] })

  // The crowd only exists for the quadtree stage; without it there is nothing to prune.
  const quadtreeOn = (broadphase & QUADTREE) !== 0
  const crowd = quadtreeOn ? crowdSetting : 0

  // The crowd is rebuilt when its size changes and keeps drifting afterwards.
  useEffect(() => {
    const l = lab.current
    if (!l) return
    l.set_crowd(crowd, size.current.width || 800, size.current.height || 600, 0x5eed)
  }, [crowd, size, wasm])

  // The scene keeps moving, so the quadtree is re-timed on an interval rather than on settle.
  useEffect(() => {
    if (!quadtreeOn) {
      setSceneTimes(null)
      return
    }
    const measure = () => {
      const l = lab.current
      if (!l) return
      setSceneTimes({
        quadtree: timePerRun(n => l.bench_quadtree(n)),
        brute: timePerRun(n => l.bench_brute_force(n)),
      })
    }
    const first = window.setTimeout(measure, 300)
    const every = window.setInterval(measure, 1500)
    return () => {
      clearTimeout(first)
      clearInterval(every)
    }
  }, [quadtreeOn, crowd])

  useAnimationFrame((_, dt) => {
    const ctx = ref.current?.getContext('2d')
    const l = lab.current
    if (!ctx || !l || size.current.width === 0) return
    const t = theme.current
    const [a, b] = shapes.current

    // A drifting crowd changes the scene every frame, so it always recomputes.
    const crowdMoving = crowd > 0 && !reducedMotion
    if (crowdMoving) l.step_crowd(Math.min(dt, 0.1))

    // Otherwise recompute only when the shapes, broadphase, crowd or selected method changed.
    const poseKey = `${version.current}|${broadphase}`
    const key = `${poseKey}|${crowd}|${method}`
    if (crowdMoving || computed.current?.key !== key) {
      l.set_shape(0, a.kind, params(a))
      l.set_shape(1, b.kind, params(b))
      outlines.current = [l.outline(0), l.outline(1)]

      const rows: Row[] = []
      let stats: SceneStats | null = null
      for (const id of l.methods()) {
        const r = l.run(id, broadphase)
        if (!r) continue
        rows.push({ id, intersect: r.intersect, distance: r.distance, stage: r.stage, counts: Array.from(r.counts()) })
        if (id === method || (!method && rows.length === 1)) {
          notes.current = { buf: r.notes(), labels: r.labels() }
          stats = readScene(r.scene())
        }
        r.free()
      }
      computed.current = { key, rows }
      setRows(prev => (sameRows(prev, rows) ? prev : rows))
      setScene(prev => (JSON.stringify(prev) === JSON.stringify(stats) ? prev : stats))

      if (poseKey !== pose.current) {
        pose.current = poseKey
        scheduleBench(rows.map(r => r.id), broadphase)
      }
    }
    const next = computed.current.rows

    clear(ctx, size.current.dpr)
    ctx.translate(size.current.width / 2, size.current.height / 2)

    const overlap = next.find(r => r.id === REFERENCE)?.intersect ?? false
    const selected = next.find(r => r.id === method) ?? next[0]
    const hit = selected?.intersect ?? false
    if (crowd > 0) drawCrowd(ctx, t, l.crowd_outlines())
    drawShape(ctx, t, outlines.current[0], a, hit, 'A')
    drawShape(ctx, t, outlines.current[1], b, hit, 'B')

    if (overlap) {
      // Hatch the true intersection, whatever the selected method claims.
      ctx.save()
      tracePath(ctx, outlines.current[0])
      ctx.clip()
      tracePath(ctx, outlines.current[1])
      ctx.fillStyle = hatchPattern(ctx, t.signal, size.current.dpr)
      ctx.fill()
      ctx.restore()
    }

    drawNotes(ctx, t, notes.current.buf, notes.current.labels)
    drawHandles(ctx, t, a)
    drawHandles(ctx, t, b)
  })

  const selected = rows.find(r => r.id === method)
  const reference = rows.find(r => r.id === REFERENCE)
  const [dots, sqrts, divs, axes, supports, iterations] = selected?.counts ?? [0, 0, 0, 0, 0, 0]
  const shapeOptions = SHAPE_KINDS.map(k => ({ value: k, label: SHAPE_NAMES[k] }))
  const toggle = (flag: number) => (on: boolean) => setBroadphase(m => (on ? m | flag : m & ~flag))

  return (
    <Sheet
      title="Collision lab"
      summary="Pick two shapes, then switch between the methods that can test them. Each method sketches its own construction. Drag shapes and their handles."
      readouts={[
        {
          label: 'Intersecting',
          value: selected ? (selected.intersect ? 'Yes' : 'No') : '—',
          tone: selected?.intersect ? 'signal' : undefined,
        },
        {
          label: 'Distance',
          value: selected?.stage === FULL_TEST ? formatDistance(selected.distance) : '—',
          tone: selected?.distance !== undefined && selected.distance <= 0 ? 'signal' : undefined,
        },
        { label: 'Decided by', value: selected ? STAGES[selected.stage] : '—', tone: selected?.stage === FULL_TEST ? undefined : 'cobalt' },
        { label: 'Time per test', value: method ? formatTime(timings[method]) : '—' },
        { label: 'Products', value: dots },
        { label: 'Roots / divisions', value: `${sqrts} / ${divs}` },
        { label: 'Axes / supports', value: `${axes} / ${supports}` },
        { label: 'Iterations', value: iterations },
      ]}
      canvas={<canvas ref={ref} className="drawing-canvas" role="img" aria-label="Two draggable shapes with the selected collision method's construction" />}
      notes={
        <>
        {scene && (
          <p className="scene-stats">
            <strong>Quadtree</strong> over {scene.bodies} bodies: {scene.nodes} cells, depth {scene.depth}.{' '}
            {scene.candidates.toLocaleString()} candidate pairs instead of {scene.allPairs.toLocaleString()}
            {' '}({percent(scene.candidates, scene.allPairs)}); A and B have {scene.partners}.{' '}
            Of the candidates, {scene.boxOverlaps} have overlapping boxes and{' '}
            <strong>{scene.collisions} really collide</strong> ({percent(scene.collisions, scene.candidates)}).
            {sceneTimes && (
              <> Build and collect pairs {formatTime(sceneTimes.quadtree)}, brute force {formatTime(sceneTimes.brute)}.</>
            )}
          </p>
        )}
        <table className="compare">
          <caption>All methods for this pair</caption>
          <thead>
            <tr>
              <th scope="col">Method</th>
              <th scope="col">Hit</th>
              <th scope="col">Distance</th>
              <th scope="col">Time</th>
            </tr>
          </thead>
          <tbody>
            {rows.map(r => {
              const disagrees = reference && r.stage === FULL_TEST && r.intersect !== reference.intersect
              return (
                <tr key={r.id} data-selected={r.id === method || undefined} onClick={() => setParam('m', r.id)}>
                  <th scope="row">{wasm.CollisionLab.method_name(r.id)}</th>
                  <td data-tone={r.intersect ? 'signal' : undefined}>
                    {r.intersect ? 'yes' : 'no'}
                    {disagrees && <span title="Disagrees with GJK + EPA"> ≠</span>}
                  </td>
                  <td>{r.stage === FULL_TEST ? formatDistance(r.distance) : STAGES[r.stage].toLowerCase()}</td>
                  <td>{formatTime(timings[r.id])}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
        </>
      }
    >
      <ControlGroup title="Shapes">
        <Select label="Shape A" value={kinds[0]} options={shapeOptions} onChange={v => setParam('a', v)} />
        <Select label="Shape B" value={kinds[1]} options={shapeOptions} onChange={v => setParam('b', v)} />
      </ControlGroup>
      <ControlGroup title="Method">
        {method && (
          <Choice
            name="method"
            value={method}
            options={ids.map(id => ({
              value: id,
              label: wasm.CollisionLab.method_name(id),
              detail: id === method ? wasm.CollisionLab.method_summary(id) : undefined,
            }))}
            onChange={id => setParam('m', id)}
          />
        )}
      </ControlGroup>
      <ControlGroup title="Broadphase">
        <Toggle label="Bounding circles first" checked={(broadphase & BOUNDING_CIRCLE) !== 0} onChange={toggle(BOUNDING_CIRCLE)} />
        <Toggle label="Bounding boxes first" checked={(broadphase & BOUNDING_BOX) !== 0} onChange={toggle(BOUNDING_BOX)} />
        <Toggle label="Quadtree over the scene" checked={quadtreeOn} onChange={toggle(QUADTREE)} />
        {quadtreeOn && (
          <Slider label="Crowd" unit="shapes" value={crowdSetting} min={0} max={400} step={10} onChange={setCrowd} />
        )}
      </ControlGroup>
    </Sheet>
  )
}

export default CollisionLabDemo
