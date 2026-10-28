<!--
  @component
  A tree of clusters growing from a stream of embeddings: the points on
  their two principal axes, colored by their node at the chosen depth,
  with the tree's centers and links over them; and both trees, grown
  online and built in passes, as icicles, every node as wide as the points
  it holds, colored by the true topic most of them come from.
-->
<script lang="ts">
  import type { TreeLab } from 'silly_demos'
  import { untrack } from 'svelte'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear, label } from '../../canvas/draft.ts'
  import { currentTheme, darkScheme, type Theme } from '../../canvas/theme.svelte.ts'
  import { clusterColor } from '../../demos/clustering/clustering.ts'
  import {
    NODE_STRIDE,
    TREES,
    readTreeParams,
    readTreeStats,
    type SpaceId,
    type TreeParam,
    type TreeStats,
    type WhichTree,
  } from '../../demos/clustering/tree.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    space,
    seed,
    running,
    rate,
    params,
    depth,
    colorBy,
    drawn,
    restart = 0,
    buildRequest = 0,
    onstats,
    onparams,
    onlab,
  }: {
    wasm: Wasm
    /** Changing it starts over. */
    space: SpaceId
    seed: number
    running: boolean
    /** Points per frame. */
    rate: number
    /** Parameters that differ from their defaults, by id; changing one starts over. */
    params: Record<string, number>
    /** The depth whose nodes color the points and are drawn. */
    depth: number
    /** Color points by their node in the drawn tree, or by their true topic. */
    colorBy: 'tree' | 'truth'
    /** The tree drawn over the points. */
    drawn: WhichTree
    /** Changing this value starts over. */
    restart?: number
    /** Each increment builds the tree in passes from the points so far. */
    buildRequest?: number
    onstats: (stats: TreeStats) => void
    onparams: (params: TreeParam[]) => void
    /** The lab, for measurements the page asks for itself. */
    onlab: (lab: TreeLab | null) => void
  } = $props()

  const GAP = 12
  const TITLE = 22

  let lab: TreeLab | null = null
  let since = -Infinity
  let known: TreeParam[] = []
  let applied: Record<string, number> = {}
  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
    onlab(null)
  })

  function report() {
    if (lab) onstats(readTreeStats(lab))
  }

  $effect(() => {
    const s = seed
    const sp = space
    if (!lab) {
      lab = new wasm.TreeLab(s)
      known = readTreeParams(lab)
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

  $effect(() => {
    if (buildRequest && lab) {
      lab.build_passes()
      report()
    }
  })

  interface Box {
    x: number
    y: number
    w: number
    h: number
  }

  /** The scatter as a square, one icicle per tree beside it when wide and below when tall. */
  function layout(width: number, height: number): { scatter: Box; icicles: Box[] } {
    const n = TREES.length
    if (width >= height * 1.2) {
      const size = Math.min(height - 2 * GAP - TITLE, width * 0.55 - 2 * GAP)
      const scatter = { x: GAP, y: GAP + TITLE, w: size, h: size }
      const left = scatter.x + size + 2 * GAP
      const w = width - left - GAP
      const h = (height - GAP - n * (TITLE + GAP)) / n
      return {
        scatter,
        icicles: TREES.map((_, i) => ({ x: left, y: GAP + TITLE + i * (h + TITLE + GAP), w, h })),
      }
    }
    const size = Math.min(width - 2 * GAP, height * 0.5)
    const scatter = { x: (width - size) / 2, y: GAP + TITLE, w: size, h: size }
    const top = scatter.y + size + GAP + TITLE
    const h = (height - top - GAP) / n - TITLE
    return {
      scatter,
      icicles: TREES.map((_, i) => ({ x: GAP, y: top + i * (h + TITLE), w: width - 2 * GAP, h: h - GAP })),
    }
  }

  /** A topic's color: six hues a sixth apart. */
  const topicColor = (topic: number, dark: boolean, alpha = 1) =>
    `hsl(${(topic * 60 + 15) % 360} 65% ${dark ? 62 : 45}% / ${alpha})`

  function drawScatter(ctx: CanvasRenderingContext2D, t: Theme, box: Box, l: TreeLab, dark: boolean) {
    const pts = l.projected()
    const colors = colorBy === 'truth' ? l.true_labels(0) : l.assignments(drawn, depth)
    const at = (x: number, y: number) => ({ x: box.x + x * box.w, y: box.y + y * box.h })
    ctx.strokeStyle = t.rule
    ctx.strokeRect(box.x, box.y, box.w, box.h)
    const n = pts.length / 2
    const size = n > 15_000 ? 1.5 : 2
    for (let i = 0; i < n; i++) {
      const q = at(pts[2 * i], pts[2 * i + 1])
      ctx.fillStyle = colorBy === 'truth' ? topicColor(colors[i], dark, 0.55) : clusterColor(colors[i], dark, 0.55)
      ctx.fillRect(q.x - size / 2, q.y - size / 2, size, size)
    }
    // The tree down to the chosen depth: links, then centers sized by their points.
    const nodes = l.nodes(drawn)
    const count = nodes.length / NODE_STRIDE
    const total = Math.max(1, nodes[2])
    ctx.save()
    ctx.strokeStyle = t.inkSoft
    ctx.lineWidth = 1
    for (let i = 1; i < count; i++) {
      const k = i * NODE_STRIDE
      if (nodes[k + 1] > depth) continue
      const p = nodes[k] * NODE_STRIDE
      const a = at(nodes[p + 3], nodes[p + 4])
      const b = at(nodes[k + 3], nodes[k + 4])
      ctx.beginPath()
      ctx.moveTo(a.x, a.y)
      ctx.lineTo(b.x, b.y)
      ctx.stroke()
    }
    for (let i = 0; i < count; i++) {
      const k = i * NODE_STRIDE
      if (nodes[k + 1] > depth) continue
      const q = at(nodes[k + 3], nodes[k + 4])
      const r = 2 + 14 * Math.sqrt(nodes[k + 2] / total)
      ctx.fillStyle = nodes[k + 1] === depth ? clusterColor(i, dark) : t.paper
      ctx.strokeStyle = t.ink
      ctx.beginPath()
      ctx.arc(q.x, q.y, r, 0, Math.PI * 2)
      ctx.fill()
      ctx.stroke()
    }
    ctx.restore()
  }

  /** An icicle: a row per depth, each node as wide as its share of its parent's points. */
  function drawIcicle(ctx: CanvasRenderingContext2D, t: Theme, box: Box, nodes: Float32Array, dark: boolean) {
    ctx.strokeStyle = t.rule
    ctx.strokeRect(box.x, box.y, box.w, box.h)
    const count = nodes.length / NODE_STRIDE
    if (count === 0) return
    let deepest = 0
    const children: number[][] = Array.from({ length: count }, () => [])
    for (let i = 0; i < count; i++) {
      const k = i * NODE_STRIDE
      deepest = Math.max(deepest, nodes[k + 1])
      if (nodes[k] >= 0) children[nodes[k]].push(i)
    }
    const rowH = box.h / (deepest + 1)
    const place = (i: number, x: number, w: number) => {
      const k = i * NODE_STRIDE
      const y = box.y + nodes[k + 1] * rowH
      // The root holds every topic; it gets no topic's color.
      ctx.fillStyle = nodes[k + 1] === 0 ? t.inkFaint : topicColor(nodes[k + 5], dark, 0.25 + 0.65 * nodes[k + 6])
      ctx.fillRect(x, y, w, rowH)
      if (w > 2) {
        ctx.strokeStyle = t.paper
        ctx.lineWidth = 1
        ctx.strokeRect(x, y, w, rowH)
      }
      const total = children[i].reduce((s, c) => s + nodes[c * NODE_STRIDE + 2], 0)
      let cx = x
      for (const c of children[i]) {
        const cw = total > 0 ? (w * nodes[c * NODE_STRIDE + 2]) / total : 0
        place(c, cx, cw)
        cx += cw
      }
    }
    place(0, box.x, box.w)
  }

  onFrame(time => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) lab.step(rate)
    const t = currentTheme()
    const dark = darkScheme()
    const { width, height, dpr } = canvas.size
    const { scatter, icicles } = layout(width, height)
    clear(ctx, dpr)
    drawScatter(ctx, t, scatter, lab, dark)
    const [points, total, dim] = lab.progress()
    label(
      ctx,
      t,
      `${dim}-D points on two axes · ${points.toLocaleString()} of ${total.toLocaleString()} · ${TREES.find(t => t.id === drawn)?.label.toLowerCase()} to depth ${depth}`,
      { x: scatter.x, y: scatter.y - TITLE / 2 },
      t.ink,
      'left',
    )
    TREES.forEach((tree, i) => {
      const nodes = lab!.nodes(tree.id)
      drawIcicle(ctx, t, icicles[i], nodes, dark)
      const leaves = nodes.length === 0 ? 0 : countLeaves(nodes)
      const what = nodes.length === 0 ? 'once every point is in' : `${nodes.length / NODE_STRIDE} nodes, ${leaves} leaves`
      label(ctx, t, `${tree.label} · ${what}`, { x: icicles[i].x, y: icicles[i].y - TITLE / 2 }, t.ink, 'left')
    })
    if (time - since >= 250) {
      since = time
      report()
    }
  })

  function countLeaves(nodes: Float32Array) {
    let n = 0
    for (let i = 7; i < nodes.length; i += NODE_STRIDE) n += nodes[i]
    return n
  }

</script>

<canvas {@attach canvas.attach}>
  A tree of clusters growing from a stream of embeddings, drawn on two principal axes, with both trees as icicles
</canvas>
