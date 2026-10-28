<!--
  @component
  A ring pulled onto a set of cities until it reads as a tour, or a
  classical heuristic building one, drawn as it works. Editable sets take
  a click to add a city, a drag to move one and Alt-click to remove one.
-->
<script lang="ts">
  import type { TspLab } from 'silly_demos'
  import type { Attachment } from 'svelte/attachments'
  import { DrawingCanvas, onFrame, type Point } from '../../canvas/canvas.svelte.ts'
  import { clear } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import type { World } from '../../demos/kohonen/countries.ts'
  import {
    BASE_SPEED,
    makeLab,
    readParams,
    readTourStats,
    type Bounds,
    type CitySet,
    type MethodId,
    type Param,
    type TourStats,
  } from '../../demos/kohonen/tours.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    cities,
    method,
    params,
    seed,
    running,
    speed,
    world = null,
    overlay = null,
    restart = 0,
    stepRequest = 0,
    onstats,
    onparams,
    onedit,
  }: {
    wasm: Wasm
    cities: CitySet
    method: MethodId
    /** Parameter values that differ from the method's defaults, by id. */
    params: Record<string, number>
    seed: number
    running: boolean
    /** A multiple of each method's watchable pace. */
    speed: number
    /** Drawn under real cities. */
    world?: World | null
    /** A ring and tour to show instead of this lab's, such as a race's result. */
    overlay?: { ring: Float32Array; tour: Uint32Array } | null
    /** Changing this value starts the method over. */
    restart?: number
    /** Each increment advances one iteration while paused. */
    stepRequest?: number
    onstats: (stats: TourStats) => void
    onparams: (params: Param[]) => void
    /** The cities after an edit. */
    onedit: (xy: Float32Array) => void
  } = $props()

  /** No frame spends longer than this stepping, ms. */
  const FRAME_BUDGET = 12
  /** How near a press must be to a city to pick it, px. */
  const PICK_RADIUS = 9

  let lab: TspLab | null = null
  let since = -Infinity
  let reportedDone = false
  let worldPaths: { world: World; paths: Path2D[] } | null = null

  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  function report() {
    if (!lab) return
    const stats = readTourStats(lab)
    reportedDone = stats.status === 'done'
    onstats(stats)
  }

  // A new lab whenever the cities, the method, the seed or a parameter
  // changes; building one is cheap next to running it. Read the props
  // before anything can return early, so the effect tracks them all.
  $effect(() => {
    const set = cities
    const m = method
    const s = seed
    const values = $state.snapshot(params) as Record<string, number>
    lab?.free()
    lab = makeLab(wasm, set, m, s)
    if (!lab) return
    for (const [id, v] of Object.entries(values)) lab.set_param(id, v)
    onparams(readParams(lab))
    report()
  })

  $effect(() => {
    if (restart && lab) {
      lab.restart()
      report()
    }
  })

  $effect(() => {
    if (stepRequest && lab) {
      lab.step(1)
      report()
    }
  })

  interface View {
    scale: number
    ox: number
    oy: number
  }

  /** The bounds fitted into the canvas, centered, aspect kept. */
  function view(b: Bounds, width: number, height: number): View {
    const scale = Math.min(width / b.width, height / b.height)
    return {
      scale,
      ox: (width - b.width * scale) / 2 - b.x * scale,
      oy: (height - b.height * scale) / 2 - b.y * scale,
    }
  }

  const toScreen = (v: View, x: number, y: number): Point => ({ x: v.ox + x * v.scale, y: v.oy + y * v.scale })
  const toData = (v: View, p: Point): Point => ({ x: (p.x - v.ox) / v.scale, y: (p.y - v.oy) / v.scale })

  /** The city within PICK_RADIUS of `p` (screen), nearest first; -1 for none. */
  function cityAt(v: View, xy: Float32Array, p: Point): number {
    let best = -1
    let bestD = PICK_RADIUS * PICK_RADIUS
    for (let i = 0; i < xy.length / 2; i++) {
      const s = toScreen(v, xy[2 * i], xy[2 * i + 1])
      const d = (s.x - p.x) ** 2 + (s.y - p.y) ** 2
      if (d <= bestD) [best, bestD] = [i, d]
    }
    return best
  }

  /** Editing: click to add, drag to move, Alt-click to remove. */
  const edit: Attachment<HTMLCanvasElement> = element => {
    let dragging = -1
    let xy = new Float32Array()
    const local = (e: PointerEvent): Point => {
      const rect = element.getBoundingClientRect()
      return { x: e.clientX - rect.left, y: e.clientY - rect.top }
    }
    const current = () => view(cities.bounds, canvas.size.width, canvas.size.height)
    const down = (e: PointerEvent) => {
      if (e.button !== 0 || !cities.editable) return
      e.preventDefault()
      const v = current()
      const p = local(e)
      const hit = cityAt(v, cities.xy, p)
      if (e.altKey) {
        if (hit >= 0 && cities.xy.length / 2 > 3) {
          onedit(Float32Array.from([...cities.xy.subarray(0, 2 * hit), ...cities.xy.subarray(2 * hit + 2)]))
        }
        return
      }
      if (hit >= 0) {
        element.setPointerCapture(e.pointerId)
        dragging = hit
        xy = Float32Array.from(cities.xy)
        element.style.cursor = 'grabbing'
        return
      }
      if (cities.xy.length / 2 >= 1000) return
      const d = toData(v, p)
      onedit(Float32Array.from([...cities.xy, d.x, d.y]))
    }
    const move = (e: PointerEvent) => {
      if (!cities.editable) {
        element.style.cursor = 'default'
        return
      }
      const v = current()
      const p = local(e)
      if (dragging < 0) {
        element.style.cursor = cityAt(v, cities.xy, p) >= 0 ? 'grab' : 'crosshair'
        return
      }
      const d = toData(v, p)
      xy[2 * dragging] = d.x
      xy[2 * dragging + 1] = d.y
      onedit(Float32Array.from(xy))
    }
    const up = () => {
      dragging = -1
    }
    element.style.touchAction = 'none'
    element.addEventListener('pointerdown', down)
    element.addEventListener('pointermove', move)
    element.addEventListener('pointerup', up)
    element.addEventListener('pointercancel', up)
    return () => {
      element.removeEventListener('pointerdown', down)
      element.removeEventListener('pointermove', move)
      element.removeEventListener('pointerup', up)
      element.removeEventListener('pointercancel', up)
    }
  }

  /** Runs about `n` iterations, in chunks, stopping early past the frame budget. */
  function advance(l: TspLab, n: number) {
    const end = performance.now() + FRAME_BUDGET
    const chunk = Math.max(1, Math.ceil(n / 8))
    let left = n
    while (left > 0 && performance.now() < end) {
      const ran = l.step(Math.min(chunk, left))
      if (ran === 0) break
      left -= ran
    }
  }

  function closedPath(ctx: CanvasRenderingContext2D, v: View, points: ArrayLike<number>, order?: ArrayLike<number>) {
    const n = order ? order.length : points.length / 2
    if (n === 0) return
    ctx.beginPath()
    for (let k = 0; k < n; k++) {
      const i = order ? order[k] : k
      const p = toScreen(v, points[2 * i], points[2 * i + 1])
      if (k === 0) ctx.moveTo(p.x, p.y)
      else ctx.lineTo(p.x, p.y)
    }
    ctx.closePath()
  }

  onFrame(time => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running && !overlay) advance(lab, Math.max(1, Math.round(BASE_SPEED[method] * speed)))

    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    const v = view(cities.bounds, width, height)
    clear(ctx, dpr)

    if (world && cities.metric === 'geo') {
      if (worldPaths?.world !== world) {
        worldPaths = { world, paths: Object.values(world.countries).map(d => new Path2D(d)) }
      }
      ctx.save()
      ctx.setTransform(dpr * v.scale, 0, 0, dpr * v.scale, dpr * v.ox, dpr * v.oy)
      ctx.fillStyle = t.paperRaised
      ctx.strokeStyle = t.rule
      ctx.lineWidth = 0.8 / v.scale
      for (const p of worldPaths.paths) {
        ctx.fill(p)
        ctx.stroke(p)
      }
      ctx.restore()
    }

    if (cities.editable) {
      const a = toScreen(v, cities.bounds.x, cities.bounds.y)
      ctx.strokeStyle = t.rule
      ctx.lineWidth = 1
      ctx.strokeRect(a.x, a.y, cities.bounds.width * v.scale, cities.bounds.height * v.scale)
    }

    const ring = overlay ? overlay.ring : lab.ring()
    const tour = overlay ? overlay.tour : lab.tour()
    const status = lab.status()
    const settled = ring.length === 0 || overlay !== null || status === 'done'

    if (tour.length > 0 && settled) {
      closedPath(ctx, v, cities.xy, tour)
      ctx.strokeStyle = t.amber
      ctx.lineWidth = 2
      ctx.lineJoin = 'round'
      ctx.stroke()
    }

    if (ring.length > 0 && !(overlay === null && status === 'done')) {
      closedPath(ctx, v, ring)
      ctx.strokeStyle = t.cobalt
      ctx.lineWidth = 1.5
      ctx.lineJoin = 'round'
      ctx.stroke()
      ctx.fillStyle = t.cobalt
      for (let j = 0; j < ring.length / 2; j++) {
        const p = toScreen(v, ring[2 * j], ring[2 * j + 1])
        ctx.fillRect(p.x - 1.5, p.y - 1.5, 3, 3)
      }
    }

    const n = cities.xy.length / 2
    const r = n > 300 ? 2 : 3
    ctx.fillStyle = t.ink
    for (let i = 0; i < n; i++) {
      const p = toScreen(v, cities.xy[2 * i], cities.xy[2 * i + 1])
      ctx.beginPath()
      ctx.arc(p.x, p.y, r, 0, Math.PI * 2)
      ctx.fill()
    }

    if (running && time - since >= 200) {
      since = time
      report()
    } else if (status === 'done' && !reportedDone) {
      report()
    }
  })
</script>

<canvas {@attach canvas.attach} {@attach edit}>Cities and a ring pulled onto them until it reads as a tour</canvas>
