<!--
  @component
  A boids flock simulated in WASM and drawn on a canvas that fills its
  container. Optionally shows what the boid nearest the pointer perceives.
-->
<script lang="ts">
  import type { BoidsSim } from 'silly_demos'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    STRIDE,
    applyConfig,
    drawBoids,
    drawPerception,
    type FlockConfig,
    type FlockStats,
  } from '../../demos/boids/flock.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    config,
    running = true,
    showPerception = false,
    scatter = 0,
    onstats,
    label,
  }: {
    wasm: Wasm
    config: FlockConfig
    running?: boolean
    showPerception?: boolean
    /** Changing this value respawns the flock. */
    scatter?: number
    onstats?: (stats: FlockStats) => void
    label: string
  } = $props()

  // The simulation lives in WASM memory; it is created once the canvas has a size.
  let sim: BoidsSim | null = null
  const fps = { frames: 0, since: 0, value: 0 }

  const canvas = new DrawingCanvas(({ width, height }) => {
    if (!sim) {
      sim = new wasm.BoidsSim(config.count, width, height, (Math.random() * 2 ** 32) >>> 0)
      applyConfig(wasm, sim, config)
    } else {
      sim.resize(width, height)
    }
  })

  $effect(() => () => {
    sim?.free()
    sim = null
  })

  $effect(() => {
    // Copy first so every field is tracked, even before the simulation exists.
    const next = { ...config }
    if (sim) applyConfig(wasm, sim, next)
  })

  $effect(() => {
    if (scatter) sim?.scatter()
  })

  onFrame((time, dt) => {
    const ctx = canvas.context()
    if (!sim || !ctx) return
    if (running) sim.step(Math.min(dt, 1 / 15))

    const theme = currentTheme()
    const state = sim.state()
    clear(ctx, canvas.size.dpr)

    // Focus the boid nearest the pointer, or the first one.
    let focus = -1
    if (showPerception && state.length > 0) {
      focus = 0
      const p = canvas.pointer
      if (p) {
        let best = Infinity
        for (let i = 0; i < state.length; i += STRIDE) {
          const d = (state[i] - p.x) ** 2 + (state[i + 1] - p.y) ** 2
          if (d < best) {
            best = d
            focus = i / STRIDE
          }
        }
      }
      drawPerception(ctx, theme, state, focus, config)
    }
    drawBoids(ctx, theme, state, focus)

    fps.frames++
    if (time - fps.since >= 500) {
      fps.value = Math.round((fps.frames * 1000) / (time - fps.since))
      fps.frames = 0
      fps.since = time
      if (onstats) {
        let neighbors = 0
        let stragglers = 0
        for (let i = 4; i < state.length; i += STRIDE) {
          neighbors += state[i]
          if (state[i] === 0) stragglers++
        }
        const n = state.length / STRIDE
        onstats({
          fps: fps.value,
          polarization: sim.polarization(),
          avgNeighbors: n ? neighbors / n : 0,
          stragglers,
        })
      }
    }
  })
</script>

<canvas {@attach canvas.attach}>{label}</canvas>
