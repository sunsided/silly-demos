<!--
  @component
  One stream of points clustered two ways, side by side: the balls with
  their radii and reach, and the sketch with its facilities and clusters.
  Points fade with age and take their cluster's color; spawns, merges,
  prunes and splits flash where they happen. While paused, the last
  point's placement is drawn. Press and drag on either panel to drop
  points by hand.
-->
<script lang="ts">
  import type { ClusterLab } from 'silly_demos'
  import { untrack } from 'svelte'
  import type { Attachment } from 'svelte/attachments'
  import { DrawingCanvas, onFrame, type Point } from '../../canvas/canvas.svelte.ts'
  import { clear, label } from '../../canvas/draft.ts'
  import { currentTheme, darkScheme, type Theme } from '../../canvas/theme.svelte.ts'
  import {
    EVENT_KINDS,
    MODELS,
    clusterColor,
    readStreamParams,
    readStreamStats,
    type BallTrace,
    type ScenarioId,
    type SketchTrace,
    type StreamParam,
    type StreamStats,
  } from '../../demos/clustering/clustering.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    scenario,
    seed,
    running,
    rate,
    params,
    showReach,
    showTrace,
    restart = 0,
    stepRequest = 0,
    onstats,
    onparams,
  }: {
    wasm: Wasm
    /** Changing it starts over. */
    scenario: ScenarioId
    seed: number
    running: boolean
    /** Points per frame. */
    rate: number
    /** Parameters that differ from their defaults, by id. */
    params: Record<string, number>
    /** Draws how far out each ball takes points, besides its radius. */
    showReach: boolean
    /** Draws how the last point was placed, while paused. */
    showTrace: boolean
    /** Changing this value starts over. */
    restart?: number
    /** Each increment adds one point while paused. */
    stepRequest?: number
    onstats: (stats: StreamStats) => void
    onparams: (params: StreamParam[]) => void
  } = $props()

  /** Room above each panel for its title, px. */
  const TITLE = 22
  /** Space around each panel, px. */
  const GAP = 12
  /** Points an event keeps flashing for. */
  const FLASH = 400
  /** Points that are this old are drawn faintest. */
  const FADE = 1500
  /** A drag drops a point every this many px. */
  const PAINT_STEP = 5

  let lab: ClusterLab | null = null
  let since = -Infinity
  let known: StreamParam[] = []
  let applied: Record<string, number> = {}
  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  function report() {
    if (lab) onstats(readStreamStats(lab))
  }

  // Read the props first: `lab?.` skips its arguments while `lab` is null.
  $effect(() => {
    const s = seed
    const sc = scenario
    if (!lab) {
      lab = new wasm.ClusterLab(s)
      known = readStreamParams(lab)
      applied = Object.fromEntries(known.map(p => [p.id, p.value]))
      untrack(() => onparams(known))
    }
    lab.set_seed(s)
    lab.set_scenario(sc)
    report()
  })

  // Only parameters that changed go to the lab; all apply at once.
  $effect(() => {
    const wanted = $state.snapshot(params) as Record<string, number>
    if (!lab) return
    for (const p of known) {
      const v = wanted[p.id] ?? p.default
      if (applied[p.id] !== v && lab.set_param(p.id, v)) applied[p.id] = v
    }
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

  interface Panel {
    x: number
    y: number
    size: number
  }

  /** Two square panels, side by side when wide and stacked when tall. */
  function layout(width: number, height: number): Panel[] {
    const across = width >= height
    const cellW = across ? width / 2 : width
    const cellH = across ? height : height / 2
    const size = Math.max(1, Math.min(cellW - 2 * GAP, cellH - 2 * GAP - TITLE))
    return [0, 1].map(i => ({
      x: (across ? i * cellW : 0) + (cellW - size) / 2,
      y: (across ? 0 : i * cellH) + (cellH - size - TITLE) / 2 + TITLE,
      size,
    }))
  }

  const at = (p: Panel, x: number, y: number): Point => ({ x: p.x + x * p.size, y: p.y + y * p.size })

  /** The point under `q` in a panel's unit square, if any. */
  function hit(q: Point): Point | null {
    for (const p of layout(canvas.size.width, canvas.size.height)) {
      const u = { x: (q.x - p.x) / p.size, y: (q.y - p.y) / p.size }
      if (u.x >= 0 && u.x <= 1 && u.y >= 0 && u.y <= 1) return u
    }
    return null
  }

  /** Painting: press and drag to drop points. */
  const paint: Attachment<HTMLCanvasElement> = element => {
    let last: Point | null = null
    const local = (e: PointerEvent): Point => {
      const rect = element.getBoundingClientRect()
      return { x: e.clientX - rect.left, y: e.clientY - rect.top }
    }
    const drop = (q: Point) => {
      const u = hit(q)
      if (lab && u) lab.paint(u.x, u.y)
    }
    const down = (e: PointerEvent) => {
      if (e.button !== 0) return
      e.preventDefault()
      element.setPointerCapture(e.pointerId)
      last = local(e)
      drop(last)
    }
    const move = (e: PointerEvent) => {
      if (!last) return
      const q = local(e)
      if (Math.hypot(q.x - last.x, q.y - last.y) < PAINT_STEP) return
      last = q
      drop(q)
    }
    const up = () => {
      last = null
      report()
    }
    element.style.cursor = 'crosshair'
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

  function drawPoints(ctx: CanvasRenderingContext2D, t: Theme, p: Panel, points: Float32Array, assigned: Uint32Array, dark: boolean) {
    for (let i = 0; i < points.length / 4; i++) {
      const age = points[i * 4 + 2]
      const alpha = Math.max(0.12, 1 - age / FADE)
      const id = assigned[i]
      ctx.fillStyle = id === 4294967295 ? t.inkFaint : clusterColor(id, dark, alpha)
      const q = at(p, points[i * 4], points[i * 4 + 1])
      ctx.fillRect(q.x - 1.5, q.y - 1.5, 3, 3)
    }
  }

  function circle(ctx: CanvasRenderingContext2D, q: Point, r: number) {
    ctx.beginPath()
    ctx.arc(q.x, q.y, Math.max(r, 1), 0, Math.PI * 2)
    ctx.stroke()
  }

  /** Clusters as circles of their radius; for the balls also, faintly, their reach. */
  function drawClusters(ctx: CanvasRenderingContext2D, t: Theme, p: Panel, clusters: Float32Array, reach: number | null, dark: boolean) {
    ctx.save()
    for (let i = 0; i < clusters.length / 6; i++) {
      const [id, x, y, r, , counted] = clusters.subarray(i * 6, i * 6 + 6)
      const q = at(p, x, y)
      ctx.strokeStyle = counted ? clusterColor(id, dark) : t.inkFaint
      ctx.lineWidth = counted ? 1.75 : 1
      ctx.setLineDash(counted ? [] : [3, 3])
      circle(ctx, q, r * p.size)
      if (reach !== null && counted) {
        ctx.lineWidth = 1
        ctx.setLineDash([2, 4])
        ctx.strokeStyle = clusterColor(id, dark, 0.45)
        circle(ctx, q, r * reach * p.size)
      }
      ctx.setLineDash([])
      ctx.fillStyle = counted ? clusterColor(id, dark) : t.inkFaint
      ctx.fillRect(q.x - 2, q.y - 2, 4, 4)
    }
    ctx.restore()
  }

  function drawFacilities(ctx: CanvasRenderingContext2D, t: Theme, p: Panel, facs: Float32Array) {
    ctx.save()
    ctx.strokeStyle = t.inkSoft
    ctx.lineWidth = 1
    for (let i = 0; i < facs.length / 3; i++) {
      const q = at(p, facs[i * 3], facs[i * 3 + 1])
      const s = 2 + Math.min(5, Math.sqrt(facs[i * 3 + 2]) / 4)
      ctx.strokeRect(q.x - s, q.y - s, 2 * s, 2 * s)
    }
    ctx.restore()
  }

  /** Events of the last FLASH points as rings that widen and fade. */
  function drawEvents(ctx: CanvasRenderingContext2D, t: Theme, p: Panel, events: Float32Array) {
    const tones = { spawn: t.verdigris, merge: t.amber, prune: t.signal, split: t.cobalt }
    ctx.save()
    for (let i = 0; i < events.length / 4; i++) {
      const [kind, x, y, ago] = events.subarray(i * 4, i * 4 + 4)
      if (ago > FLASH) continue
      const f = ago / FLASH
      ctx.globalAlpha = 1 - f
      ctx.strokeStyle = tones[EVENT_KINDS[kind] ?? 'spawn']
      ctx.lineWidth = 2
      circle(ctx, at(p, x, y), 6 + 24 * f)
    }
    ctx.restore()
  }

  function drawBallTrace(ctx: CanvasRenderingContext2D, t: Theme, p: Panel, trace: BallTrace, clusters: Float32Array) {
    const centers: Record<number, Point> = {}
    for (let i = 0; i < clusters.length / 6; i++) centers[clusters[i * 6]] = at(p, clusters[i * 6 + 1], clusters[i * 6 + 2])
    const q = at(p, trace.point[0], trace.point[1])
    ctx.save()
    ctx.strokeStyle = t.inkFaint
    ctx.lineWidth = 1
    for (const [id] of trace.checked) {
      const c = centers[id]
      if (!c) continue
      ctx.beginPath()
      ctx.moveTo(q.x, q.y)
      ctx.lineTo(c.x, c.y)
      ctx.stroke()
    }
    ctx.strokeStyle = t.signal
    ctx.lineWidth = 2
    ctx.beginPath()
    trace.path.forEach((id, k) => {
      const c = centers[id]
      if (!c) return
      if (k === 0) ctx.moveTo(c.x, c.y)
      else ctx.lineTo(c.x, c.y)
    })
    ctx.stroke()
    ctx.fillStyle = t.signal
    ctx.beginPath()
    ctx.arc(q.x, q.y, 4, 0, Math.PI * 2)
    ctx.fill()
    ctx.restore()
    const what = trace.spawned ? 'spawned a ball' : `joined ball ${trace.ball}`
    const how = trace.fullScan ? 'after a full scan' : `walk of ${trace.path.length}`
    label(ctx, t, `${trace.checked.length} distances, ${how}: ${what}`, { x: q.x + 8, y: q.y - 12 }, t.signal, 'left')
  }

  function drawSketchTrace(ctx: CanvasRenderingContext2D, t: Theme, p: Panel, trace: SketchTrace) {
    const q = at(p, trace.point[0], trace.point[1])
    ctx.save()
    if (trace.nearest) {
      const n = at(p, trace.nearest[0], trace.nearest[1])
      ctx.strokeStyle = t.signal
      ctx.lineWidth = 1.5
      ctx.setLineDash([4, 3])
      ctx.beginPath()
      ctx.moveTo(q.x, q.y)
      ctx.lineTo(n.x, n.y)
      ctx.stroke()
    }
    ctx.fillStyle = t.signal
    ctx.beginPath()
    ctx.arc(q.x, q.y, 4, 0, Math.PI * 2)
    ctx.fill()
    ctx.restore()
    const odds = `p = min(d²/f, 1) = ${trace.p.toFixed(3)}, draw ${trace.draw.toFixed(3)}`
    const what = trace.opened ? 'opened a facility' : 'joined the nearest'
    label(ctx, t, `${odds}: ${what}${trace.collapsed ? ', then collapsed' : ''}`, { x: q.x + 8, y: q.y - 12 }, t.signal, 'left')
  }

  onFrame(time => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) lab.step(rate)

    const t = currentTheme()
    const dark = darkScheme()
    const { width, height, dpr } = canvas.size
    const panels = layout(width, height)
    const points = lab.points()
    const stats = lab.stats()
    clear(ctx, dpr)
    MODELS.forEach((model, i) => {
      const p = panels[i]
      const clusters = lab!.clusters(model.id)
      ctx.strokeStyle = t.rule
      ctx.lineWidth = 1
      ctx.strokeRect(p.x, p.y, p.size, p.size)
      drawPoints(ctx, t, p, points, lab!.assignments(model.id), dark)
      if (model.id === 'sketch') drawFacilities(ctx, t, p, lab!.facilities())
      drawClusters(ctx, t, p, clusters, model.id === 'balls' && showReach ? reachOf() : null, dark)
      drawEvents(ctx, t, p, lab!.events(model.id))
      if (!running && showTrace) {
        const raw = JSON.parse(lab!.trace(model.id))
        if (raw && model.id === 'balls') drawBallTrace(ctx, t, p, raw as BallTrace, clusters)
        if (raw && model.id === 'sketch') drawSketchTrace(ctx, t, p, raw as SketchTrace)
      }
      const k = model.id === 'balls' ? stats[2] : stats[3]
      label(ctx, t, `${model.label} · ${k} clusters`, { x: p.x, y: p.y - TITLE / 2 }, t.ink, 'left')
    })

    if (running && time - since >= 250) {
      since = time
      report()
    }
  })

  /** The balls' reach in radii, as currently set. */
  function reachOf(): number {
    return params['balls.reach'] ?? known.find(p => p.id === 'balls.reach')?.default ?? 2.5
  }

</script>

<canvas {@attach canvas.attach} {@attach paint}>
  A stream of points clustered as it arrives, by balls that spawn, merge, split and fade, and by a sketch refined with
  ball k-means
</canvas>
