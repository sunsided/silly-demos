<!--
  @component
  Embeddings streamed into five nearest-neighbor indexes, and one search
  through the chosen one: every point on the embeddings' two principal
  axes, colored faintly by true topic, the points the search compared the
  query with in the index's color, what it found ringed and the true ten
  nearest crossed; beside it the same around the query, enlarged.
-->
<script lang="ts">
  import type { VectorLab } from 'silly_demos'
  import { untrack } from 'svelte'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear, label } from '../../canvas/draft.ts'
  import { currentTheme, darkScheme, type Theme } from '../../canvas/theme.svelte.ts'
  import type { SpaceId } from '../../demos/clustering/tree.ts'
  import {
    indexInfo,
    readTrace,
    readVectorParams,
    readVectorStats,
    type IndexId,
    type Trace,
    type VectorParam,
    type VectorStats,
  } from '../../demos/vector/vector.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    space,
    seed,
    running,
    rate,
    params,
    index,
    effort,
    query,
    restart = 0,
    onstats,
    onparams,
    onlab,
  }: {
    wasm: Wasm
    /** Changing it starts over. */
    space: SpaceId
    seed: number
    running: boolean
    /** Points per frame, streamed or built. */
    rate: number
    /** Parameters that differ from their defaults, by id; changing one starts over. */
    params: Record<string, number>
    /** The index whose search is drawn. */
    index: IndexId
    /** The search effort drawn: ef, lists or postings probed, L or beam. */
    effort: number
    /** The query drawn, of the lab's queries. */
    query: number
    /** Changing this value starts over. */
    restart?: number
    onstats: (stats: VectorStats) => void
    onparams: (params: VectorParam[]) => void
    /** The lab, for measurements the page asks for itself. */
    onlab: (lab: VectorLab | null) => void
  } = $props()

  const GAP = 12
  const TITLE = 22
  /** Half the side of the enlarged view, in the unit square, at least. */
  const ZOOM = 0.06

  let lab: VectorLab | null = null
  let since = -Infinity
  let known: VectorParam[] = []
  let applied: Record<string, number> = {}
  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
    onlab(null)
  })

  function report() {
    if (lab) onstats(readVectorStats(lab))
  }

  $effect(() => {
    const s = seed
    const sp = space
    if (!lab) {
      lab = new wasm.VectorLab(s)
      known = readVectorParams(lab)
      applied = Object.fromEntries(known.map(p => [p.id, p.value]))
      untrack(() => {
        onparams(known)
        onlab(lab)
      })
    }
    lab.set_seed(s)
    lab.set_space(sp)
    report()
  })

  // Only parameters that changed go to the lab; each starts it over.
  $effect(() => {
    const wanted = $state.snapshot(params) as Record<string, number>
    if (!lab) return
    for (const p of known) {
      const v = wanted[p.id] ?? p.default
      if (applied[p.id] !== v && lab.set_param(p.id, v)) applied[p.id] = v
    }
    report()
  })

  $effect(() => {
    if (restart && lab) {
      lab.restart()
      report()
    }
  })

  interface Box {
    x: number
    y: number
    w: number
    h: number
  }

  /** Two squares, side by side when wide and one above the other when tall. */
  function layout(width: number, height: number): [Box, Box] {
    if (width >= height * 1.15) {
      const size = Math.min(height - 2 * GAP - TITLE, (width - 3 * GAP) / 2)
      const y = GAP + TITLE
      return [
        { x: GAP, y, w: size, h: size },
        { x: 2 * GAP + size, y, w: size, h: size },
      ]
    }
    const size = Math.min(width - 2 * GAP, (height - 2 * TITLE - 3 * GAP) / 2)
    const x = (width - size) / 2
    return [
      { x, y: GAP + TITLE, w: size, h: size },
      { x, y: 2 * (GAP + TITLE) + size, w: size, h: size },
    ]
  }

  /** A topic's color: six hues a sixth apart. */
  const topicColor = (topic: number, dark: boolean, alpha: number) =>
    `hsl(${(topic * 60 + 15) % 360} 50% ${dark ? 62 : 45}% / ${alpha})`

  /** The view of the unit square that `box` shows: all of it, or a window. */
  interface View {
    x0: number
    y0: number
    span: number
  }

  function drawPanel(
    ctx: CanvasRenderingContext2D,
    t: Theme,
    box: Box,
    view: View,
    pts: Float32Array,
    topics: Uint32Array,
    trace: Trace | null,
    truth: Uint32Array,
    q: Float32Array,
    color: string,
    dark: boolean,
  ) {
    const at = (x: number, y: number) => ({
      x: box.x + ((x - view.x0) / view.span) * box.w,
      y: box.y + ((y - view.y0) / view.span) * box.h,
    })
    ctx.save()
    ctx.strokeStyle = t.rule
    ctx.strokeRect(box.x, box.y, box.w, box.h)
    ctx.beginPath()
    ctx.rect(box.x, box.y, box.w, box.h)
    ctx.clip()
    const n = pts.length / 2
    const zoomed = view.span < 1
    const size = zoomed ? 3 : n > 15_000 ? 1.5 : 2
    for (let i = 0; i < n; i++) {
      const p = at(pts[2 * i], pts[2 * i + 1])
      ctx.fillStyle = topicColor(topics[i], dark, 0.3)
      ctx.fillRect(p.x - size / 2, p.y - size / 2, size, size)
    }
    if (trace) {
      ctx.fillStyle = color
      const s = size + 1
      for (const id of trace.seen) {
        const p = at(pts[2 * id], pts[2 * id + 1])
        ctx.fillRect(p.x - s / 2, p.y - s / 2, s, s)
      }
      ctx.strokeStyle = color
      ctx.lineWidth = 1.5
      for (const id of trace.found) {
        const p = at(pts[2 * id], pts[2 * id + 1])
        ctx.beginPath()
        ctx.arc(p.x, p.y, zoomed ? 6 : 4, 0, Math.PI * 2)
        ctx.stroke()
      }
    }
    ctx.strokeStyle = t.ink
    ctx.lineWidth = 1
    const c = zoomed ? 4 : 3
    for (const id of truth) {
      const p = at(pts[2 * id], pts[2 * id + 1])
      ctx.beginPath()
      ctx.moveTo(p.x - c, p.y - c)
      ctx.lineTo(p.x + c, p.y + c)
      ctx.moveTo(p.x - c, p.y + c)
      ctx.lineTo(p.x + c, p.y - c)
      ctx.stroke()
    }
    if (q.length === 2) {
      const p = at(q[0], q[1])
      ctx.fillStyle = t.paper
      ctx.strokeStyle = t.ink
      ctx.lineWidth = 2
      ctx.beginPath()
      ctx.arc(p.x, p.y, zoomed ? 7 : 5, 0, Math.PI * 2)
      ctx.fill()
      ctx.stroke()
    }
    ctx.restore()
  }

  /** A window around the query that holds it, the true neighbors and what was found. */
  function around(q: Float32Array, pts: Float32Array, ids: Iterable<number>): View {
    let half = ZOOM
    for (const id of ids) {
      half = Math.max(half, Math.abs(pts[2 * id] - q[0]), Math.abs(pts[2 * id + 1] - q[1]))
    }
    half = Math.min(half * 1.15, 0.5)
    return { x0: q[0] - half, y0: q[1] - half, span: 2 * half }
  }

  onFrame(time => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) lab.step(rate)
    const t = currentTheme()
    const dark = darkScheme()
    const { width, height, dpr } = canvas.size
    const [all, near] = layout(width, height)
    clear(ctx, dpr)
    const pts = lab.projected()
    const topics = lab.true_labels(0)
    const info = indexInfo(index)
    const trace = pts.length > 0 ? readTrace(lab, index, query, effort) : null
    const truth = pts.length > 0 ? lab.neighbors(query) : new Uint32Array()
    const q = lab.query_point(query)
    const color = t[info.tone]
    drawPanel(ctx, t, all, { x0: 0, y0: 0, span: 1 }, pts, topics, trace, truth, q, color, dark)
    const view = q.length === 2 ? around(q, pts, [...truth, ...(trace?.found ?? [])]) : { x0: 0, y0: 0, span: 1 }
    drawPanel(ctx, t, near, view, pts, topics, trace, truth, q, color, dark)
    // The enlarged window, outlined on the full view.
    if (q.length === 2) {
      ctx.save()
      ctx.strokeStyle = t.inkSoft
      ctx.setLineDash([3, 3])
      ctx.strokeRect(all.x + view.x0 * all.w, all.y + view.y0 * all.h, view.span * all.w, view.span * all.h)
      ctx.restore()
    }
    const [points, total, dim] = lab.progress()
    label(
      ctx,
      t,
      `${dim}-D points on two axes · ${points.toLocaleString()} of ${total.toLocaleString()}`,
      { x: all.x, y: all.y - TITLE / 2 },
      t.ink,
      'left',
    )
    const hit = trace ? truth.filter(id => trace.found.includes(id)).length : 0
    const what = trace
      ? `${info.label}, ${info.effort} ${effort} · compared ${trace.seen.length.toLocaleString()} · found ${hit} of 10`
      : `${info.label} · once it is built`
    label(ctx, t, `Query ${query + 1}, enlarged · ${what}`, { x: near.x, y: near.y - TITLE / 2 }, t.ink, 'left')
    if (time - since >= 250) {
      since = time
      report()
    }
  })
</script>

<canvas {@attach canvas.attach}>
  Embeddings on two principal axes, and the points one search through the chosen index compared the query with
</canvas>
