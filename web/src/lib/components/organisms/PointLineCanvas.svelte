<!--
  @component
  A draggable segment AB and test point P: the shortest distance to the
  segment, the perpendicular to the infinite line, and which side P is on.
-->
<script lang="ts">
  import { DrawingCanvas, draggable, onFrame, type Point } from '../../canvas/canvas.svelte.ts'
  import { clear, cross, dimension, handle, label } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    HIT_RADIUS,
    rightAngle,
    sideColor,
    type Closest,
    type HandleName,
    type PointLineStats,
  } from '../../demos/point-line/point-line.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let { wasm, onstats }: { wasm: Wasm; onstats: (stats: PointLineStats) => void } = $props()

  const pts: Record<HandleName, Point> = {
    a: { x: -160, y: 60 },
    b: { x: 160, y: -40 },
    p: { x: 30, y: -110 },
  }
  let last: PointLineStats = { distance: 0, perpendicular: 0, side: 0, closest: 'interior' }

  // Fit the initial layout to narrow screens.
  let placed = false
  const canvas = new DrawingCanvas(({ width }) => {
    // A canvas laid out at zero width (e.g. hidden) would scale everything to the origin.
    if (placed || width === 0) return
    placed = true
    const k = Math.min(1, width / 480)
    for (const q of Object.values(pts)) {
      q.x *= k
      q.y *= k
    }
  })

  const drag = draggable<HandleName>({
    toLocal: canvas.centered,
    // The test point wins over the segment's end points when they overlap.
    pick: p => {
      for (const name of ['p', 'a', 'b'] as const) {
        const h = pts[name]
        if (Math.hypot(p.x - h.x, p.y - h.y) <= HIT_RADIUS) return name
      }
      return null
    },
    move: (name, d) => {
      pts[name].x += d.x
      pts[name].y += d.y
    },
  })

  onFrame(() => {
    const ctx = canvas.context()
    if (!ctx) return
    const t = currentTheme()
    const { a, b, p } = pts

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
    if (
      distance !== last.distance ||
      perpendicular !== last.perpendicular ||
      side !== last.side ||
      closest !== last.closest
    ) {
      last = { distance, perpendicular, side, closest }
      onstats(last)
    }

    clear(ctx, canvas.size.dpr)
    const w = canvas.size.width
    const h = canvas.size.height
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
</script>

<canvas {@attach canvas.attach} {@attach drag}>Draggable line segment and test point with distance dimensions</canvas>
