<!--
  @component
  A planar chain pinned at a draggable base, solved in WASM towards a
  draggable (or animated) target with the chosen IK solver.
-->
<script lang="ts">
  import type { IkChain } from 'silly_demos'
  import { DrawingCanvas, draggable, onFrame, type Point } from '../../canvas/canvas.svelte.ts'
  import { clear, cross, dimension, handle, label } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    CLEARANCE,
    HIT_RADIUS,
    drawBase,
    drawChain,
    drawJointAngles,
    joint,
    type HandleName,
    type IkStats,
    type SolverId,
  } from '../../demos/ik/ik.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    solver,
    segments,
    length,
    iterations,
    damping,
    smoothing,
    animate,
    avoidCrossing,
    showReach,
    showAngles,
    straighten = 0,
    onstats,
  }: {
    wasm: Wasm
    solver: SolverId
    segments: number
    length: number
    /** Solver iterations allowed per frame. */
    iterations: number
    /** Damped least-squares λ in px. */
    damping: number
    /** Time constant in ms with which the drawn arm eases after the solved pose. */
    smoothing: number
    /** Moves the target along a Lissajous curve that leaves the reach now and then. */
    animate: boolean
    /** Keeps the links from passing through each other. */
    avoidCrossing: boolean
    showReach: boolean
    showAngles: boolean
    /** Changing this value straightens the chain. */
    straighten?: number
    onstats: (stats: IkStats) => void
  } = $props()

  // The chain lives in WASM memory; it is created once the canvas has a size.
  let chain: IkChain | null = null
  const pts: Record<HandleName, Point> = {
    base: { x: 0, y: 40 },
    target: { x: 110, y: -150 },
  }
  let clock = 0
  let last: IkStats = { error: 0, iterations: 0, reachable: true, reach: 0, contact: false, rerouting: false }

  const canvas = new DrawingCanvas(() => {
    if (chain) return
    chain = new wasm.IkChain(segments, length)
    chain.set_base(pts.base.x, pts.base.y)
  })

  $effect(() => () => {
    chain?.free()
    chain = null
  })

  $effect(() => {
    // Read both first so they are tracked even before the chain exists.
    const [n, l] = [segments, length]
    chain?.resize(n, l)
  })

  $effect(() => {
    if (straighten) chain?.straighten()
  })

  // Shrink the drawing on screens too small for a full reach circle.
  const scale = () => Math.min(1, canvas.size.width / 700, canvas.size.height / 700)

  const drag = draggable<HandleName>({
    toLocal: p => {
      const c = canvas.centered(p)
      const k = scale()
      return { x: c.x / k, y: c.y / k }
    },
    // An animated target can't be grabbed; the base still can.
    pick: p => {
      for (const name of animate ? (['base'] as const) : (['target', 'base'] as const)) {
        const h = pts[name]
        if (Math.hypot(p.x - h.x, p.y - h.y) <= HIT_RADIUS) return name
      }
      return null
    },
    move: (name, d) => {
      pts[name].x += d.x
      pts[name].y += d.y
      if (name === 'base') chain?.set_base(pts.base.x, pts.base.y)
    },
  })

  onFrame((_, dt) => {
    const ctx = canvas.context()
    if (!chain || !ctx) return
    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    const { base, target } = pts
    const reach = chain.reach()

    if (animate) {
      clock += Math.min(dt, 0.05)
      target.x = base.x + reach * 1.05 * Math.sin(clock * 0.6)
      target.y = base.y - reach * 0.55 - reach * 0.6 * Math.sin(clock * 1.1)
    }

    const clearance = avoidCrossing ? CLEARANCE : undefined
    const used = chain.solve(solver, target.x, target.y, iterations, damping, clearance)
    chain.follow(Math.min(dt, 0.05), smoothing / 1000, clearance)
    const contact = chain.contact()
    const rerouting = chain.rerouting()
    const error = chain.error(target.x, target.y)
    const joints = chain.joints()
    const end = joint(joints, joints.length / 2 - 1)
    const reachable = Math.hypot(target.x - base.x, target.y - base.y) <= reach

    const next = { error: Math.round(error * 10) / 10, iterations: used, reachable, reach, contact, rerouting }
    if (
      next.error !== last.error ||
      next.iterations !== last.iterations ||
      next.reachable !== last.reachable ||
      next.reach !== last.reach ||
      next.contact !== last.contact ||
      next.rerouting !== last.rerouting
    ) {
      last = next
      onstats(next)
    }

    clear(ctx, dpr)
    const k = scale()
    ctx.translate(width / 2, height / 2)
    ctx.scale(k, k)

    if (showReach) {
      ctx.beginPath()
      ctx.arc(base.x, base.y, reach, 0, Math.PI * 2)
      ctx.setLineDash([10, 6, 2, 6])
      ctx.lineWidth = 1
      ctx.strokeStyle = t.inkFaint
      ctx.stroke()
      ctx.setLineDash([])
      label(ctx, t, `reach ${Math.round(reach)}`, { x: base.x, y: base.y + reach + 12 }, t.inkSoft)
    }

    drawBase(ctx, t, base, dpr)
    drawChain(ctx, t, joints)
    if (showAngles) drawJointAngles(ctx, t, joints)

    const tc = reachable ? t.signal : t.amber
    if (error > 1.5) dimension(ctx, t, end, target, error.toFixed(1), { color: tc })
    cross(ctx, end, t.ink, 5)
    handle(ctx, base, t.ink, t.paper, 6)
    handle(ctx, target, tc, t.paper, 8)
    label(ctx, t, reachable ? 'target' : 'out of reach', { x: target.x + 16, y: target.y - 14 }, tc, 'left')
  })
</script>

<canvas {@attach canvas.attach} {@attach drag}>Jointed arm reaching for a draggable target</canvas>
