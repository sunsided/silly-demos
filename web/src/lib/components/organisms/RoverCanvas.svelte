<!--
  @component
  A rover exploring a floor plan in WASM: its map with the fog of war, the
  last laser scan, the explorer's plan and the trail it drove.
-->
<script lang="ts">
  import type { RoverLab } from 'silly_demos'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    KnowledgeImage,
    drawFloorPlan,
    drawHome,
    drawPlan,
    drawRobot,
    drawScan,
    drawTrail,
    fit,
    readStats,
    type ModeId,
    type RoverStats,
  } from '../../demos/rover/rover.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    mode,
    running,
    speed,
    samples,
    showFloorPlan,
    showScan,
    showPlan,
    showTrail,
    restart = 0,
    onstats,
  }: {
    wasm: Wasm
    mode: ModeId
    running: boolean
    /** Simulated seconds per second. */
    speed: number
    /** Laser beams per scan. */
    samples: number
    showFloorPlan: boolean
    showScan: boolean
    showPlan: boolean
    showTrail: boolean
    /** Changing this value starts the run over. */
    restart?: number
    onstats: (stats: RoverStats) => void
  } = $props()

  // The simulation lives in WASM memory; it is created once the canvas has a size.
  let lab: RoverLab | null = null
  let walls: Float32Array = new Float32Array()
  const knowledge = new KnowledgeImage()
  let since = 0

  const canvas = new DrawingCanvas(() => {
    if (lab) return
    lab = new wasm.RoverLab(mode)
    lab.set_laser_samples(samples)
    walls = lab.walls()
  })

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  // Read the props first: `lab?.` skips its arguments while `lab` is null,
  // and the effects would then never track them.
  $effect(() => {
    const m = mode
    lab?.set_mode(m)
  })

  $effect(() => {
    const n = samples
    lab?.set_laser_samples(n)
  })

  $effect(() => {
    if (restart) lab?.reset()
  })

  onFrame((time, dt) => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) lab.step(Math.min(dt, 0.05) * speed)

    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    const view = fit(width, height, lab.size())
    const [columns, rows, cell] = lab.grid()
    const pose = lab.pose()

    clear(ctx, dpr)
    knowledge.update(t, lab.knowledge(), columns, rows)
    knowledge.draw(ctx, view, cell)
    if (showFloorPlan) drawFloorPlan(ctx, t, view, walls)
    if (showScan) drawScan(ctx, t, view, pose, lab.ranges(), lab.laser())
    if (showTrail) drawTrail(ctx, t, view, lab.trail())
    if (showPlan) drawPlan(ctx, t, view, lab.path(), lab.target(), lab.given_up())
    if (mode === 'explorer') drawHome(ctx, t, view, lab.home())
    drawRobot(ctx, t, view, pose, lab.radius(), lab.bumped())

    if (time - since >= 250) {
      since = time
      onstats(readStats(lab))
    }
  })
</script>

<canvas {@attach canvas.attach}>A rover mapping and exploring a floor plan with its laser scanner</canvas>
