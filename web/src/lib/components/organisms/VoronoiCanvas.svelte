<!--
  @component
  Points drifting in WASM with their Delaunay triangulation and Voronoi
  diagram; hovering highlights a cell or a triangle with its circumcircle.
-->
<script lang="ts">
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    STRIDE,
    at,
    drawHoveredCell,
    drawHoveredTriangle,
    type VoronoiStats,
  } from '../../demos/voronoi/voronoi.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    count,
    speed,
    seed,
    running,
    showDelaunay,
    showVoronoi,
    onstats,
  }: {
    wasm: Wasm
    count: number
    speed: number
    seed: number
    running: boolean
    showDelaunay: boolean
    showVoronoi: boolean
    onstats: (stats: VoronoiStats) => void
  } = $props()

  // Flat [x, y, vx, vy, ...]; owned here, stepped in WASM.
  let points: Float32Array = new Float32Array()
  let last: VoronoiStats = { triangles: 0, edges: 0, hover: '–' }

  const spawn = (n: number, v: number, s: number) => {
    const { width, height } = canvas.size
    points = wasm.VoronoiTests.voronoi_create_points(n, width, height, s >>> 0, v)
  }

  const canvas = new DrawingCanvas(() => {
    if (points.length === 0) spawn(count, speed, seed)
  })

  // Respawn when the point set's parameters change (once the canvas has a size).
  $effect(() => {
    const [n, v, s] = [count, speed, seed]
    if (canvas.size.width > 0) spawn(n, v, s)
  })

  onFrame((_, dt) => {
    const ctx = canvas.context()
    const { width, height, dpr } = canvas.size
    if (!ctx || points.length === 0) return
    const t = currentTheme()

    if (running) {
      points = wasm.VoronoiTests.voronoi_step_points(points, width, height, Math.min(dt, 0.05))
    }
    const pts = points
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
    const m = canvas.pointer
    if (m) {
      // Prefer the triangle view when only the triangulation is shown.
      hover = (showDelaunay && !showVoronoi
        ? drawHoveredTriangle(ctx, t, pts, tris, m)
        : drawHoveredCell(ctx, t, pts, tris, m)) ?? 'Outside the hull'
    }

    const next = { triangles: tris.length / 3, edges: edges.length / 4, hover }
    if (next.triangles !== last.triangles || next.edges !== last.edges || next.hover !== last.hover) {
      last = next
      onstats(next)
    }
  })
</script>

<canvas {@attach canvas.attach}>Moving points with their Delaunay triangulation and Voronoi diagram</canvas>
