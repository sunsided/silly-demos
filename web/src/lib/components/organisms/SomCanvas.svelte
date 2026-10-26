<!--
  @component
  A self-organising map training in WASM on a 2D density you can paint:
  the density, the nodes' Voronoi regions, the lattice, and, while paused,
  the anatomy of the last update.
-->
<script lang="ts">
  import type { SomLab } from 'silly_demos'
  import { untrack } from 'svelte'
  import type { Attachment } from 'svelte/attachments'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    DensityImage,
    applyConfig,
    drawBrush,
    drawFrame,
    drawLattice,
    drawTrace,
    drawVoronoi,
    fit,
    readStats,
    readTrace,
    toUnit,
    type DensityId,
    type SomConfig,
    type SomStats,
    type TrainingId,
  } from '../../demos/kohonen/kohonen.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    config,
    density,
    training,
    running,
    speed,
    brush,
    show,
    restart = 0,
    densityReset = 0,
    stepRequest = 0,
    onstats,
  }: {
    wasm: Wasm
    /** Changing any of it starts the map over; a new seed makes a new lab. */
    config: SomConfig
    /** Changing it replaces the density, painted strokes included, and starts over. */
    density: DensityId
    training: TrainingId
    running: boolean
    /** Online: samples per frame. Batch: epochs per second. */
    speed: number
    /** `radius` is a fraction of the sheet. */
    brush: { mode: 'add' | 'erase'; radius: number }
    show: { density: boolean; voronoi: boolean; lattice: boolean; trace: boolean }
    /** Changing this value starts the map over. */
    restart?: number
    /** Changing this value puts the density preset back, dropping painted strokes. */
    densityReset?: number
    /** Each increment advances one update while paused. */
    stepRequest?: number
    onstats: (stats: SomStats) => void
  } = $props()

  let lab: SomLab | null = null
  let seed = -1
  const densityImage = new DensityImage()
  // The density only changes on edits; redraw its image then, not every frame.
  let densityDirty = true
  let edges: Uint32Array = new Uint32Array()
  let epochs = 0
  let since = -Infinity

  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  function report() {
    if (lab) onstats(readStats(lab))
  }

  // Read the props first: `lab?.` skips its arguments while `lab` is null,
  // and the effects would then never track them.
  $effect(() => {
    const c = $state.snapshot(config) as SomConfig
    if (!lab || c.seed !== seed) {
      lab?.free()
      const fresh = new wasm.SomLab(c.seed)
      // Density and training have effects of their own.
      untrack(() => {
        fresh.set_density(density)
        fresh.set_training(training)
      })
      lab = fresh
      seed = c.seed
      densityDirty = true
    }
    applyConfig(lab, c)
    edges = lab.edges()
    report()
  })

  $effect(() => {
    const d = density
    void densityReset
    if (!lab) return
    lab.set_density(d)
    lab.restart()
    densityDirty = true
    report()
  })

  $effect(() => {
    const m = training
    lab?.set_training(m)
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

  /** Painting: press and drag on the sheet. */
  const paint: Attachment<HTMLCanvasElement> = element => {
    let painting = false
    const stroke = (e: PointerEvent) => {
      if (!lab) return
      const rect = element.getBoundingClientRect()
      const { width, height } = canvas.size
      const p = toUnit(fit(width, height), { x: e.clientX - rect.left, y: e.clientY - rect.top })
      lab.paint(p.x, p.y, brush.radius, brush.mode === 'add' ? 0.5 : -1)
      densityDirty = true
    }
    const down = (e: PointerEvent) => {
      if (e.button !== 0) return
      e.preventDefault()
      element.setPointerCapture(e.pointerId)
      painting = true
      stroke(e)
    }
    const move = (e: PointerEvent) => {
      if (painting) stroke(e)
    }
    const up = () => {
      painting = false
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

  onFrame((time, dt) => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) {
      if (training === 'online') {
        lab.step(speed)
      } else {
        epochs += Math.min(dt, 0.1) * speed
        const n = Math.floor(epochs)
        epochs -= n
        if (n > 0) lab.step(n)
      }
    }

    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    const sheet = fit(width, height)
    const weights = lab.weights()

    clear(ctx, dpr)
    if (show.density) {
      if (densityDirty) {
        densityImage.update(t, lab.density())
        densityDirty = false
      }
      densityImage.draw(ctx, sheet)
    }
    drawFrame(ctx, t, sheet)
    if (show.voronoi) drawVoronoi(ctx, t, sheet, lab.voronoi())
    const trace = !running && show.trace ? readTrace(lab) : null
    if (trace) drawTrace(ctx, t, sheet, trace, weights, edges)
    if (show.lattice) drawLattice(ctx, sheet, weights, edges, t.cobalt)
    if (canvas.pointer) drawBrush(ctx, t, sheet, canvas.pointer, brush.radius)

    if (running && time - since >= 250) {
      since = time
      report()
    }
  })
</script>

<canvas {@attach canvas.attach} {@attach paint}>A self-organising map unfolding over a density you can paint</canvas>
