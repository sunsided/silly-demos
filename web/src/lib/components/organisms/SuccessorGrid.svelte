<!--
  @component
  The map and its successors side by side, all trained on the same samples
  from one density you can paint on any panel: each panel draws one model's
  nodes and links over the density, and a last one shows the density alone.
-->
<script lang="ts">
  import type { SuccessorLab } from 'silly_demos'
  import { untrack } from 'svelte'
  import type { Attachment } from 'svelte/attachments'
  import { DrawingCanvas, onFrame, type Point } from '../../canvas/canvas.svelte.ts'
  import { clear, label } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import {
    DensityImage,
    drawBrush,
    drawFrame,
    drawLattice,
    drawVoronoi,
    isMoving,
    toUnit,
    type DensityId,
    type Sheet,
  } from '../../demos/kohonen/kohonen.ts'
  import {
    MODELS,
    readSuccessorParams,
    readSuccessorStats,
    type SuccessorParam,
    type SuccessorStats,
  } from '../../demos/kohonen/successors.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    density,
    image = null,
    nodes,
    steps,
    continuous,
    seed,
    running,
    speed,
    brush,
    showVoronoi,
    params,
    motionPeriod,
    restart = 0,
    densityReset = 0,
    stepRequest = 0,
    onstats,
    onparams,
  }: {
    wasm: Wasm
    /** Changing it replaces the density, painted strokes included. */
    density: DensityId
    /** The density for `density === 'image'`: `side`×`side` weights. */
    image?: { cells: Float32Array; side: number } | null
    /** Nodes per model; the map rounds it to a grid. Changing it starts over. */
    nodes: number
    /** Samples per run. Changing it starts over. */
    steps: number
    /** Keep learning past the end of the run. */
    continuous: boolean
    seed: number
    running: boolean
    /** Samples per frame. */
    speed: number
    /** `radius` is a fraction of a panel's sheet. */
    brush: { mode: 'add' | 'erase'; radius: number }
    /** Draws every model's Voronoi regions, not only k-means'. */
    showVoronoi: boolean
    /** Method parameters that differ from their defaults, by id. */
    params: Record<string, number>
    /** Samples per cycle of a moving density. */
    motionPeriod: number
    /** Changing this value starts over. */
    restart?: number
    /** Changing this value reloads the density, dropping painted strokes. */
    densityReset?: number
    /** Each increment advances one sample while paused. */
    stepRequest?: number
    onstats: (stats: SuccessorStats) => void
    /** The method parameters, once the lab exists. */
    onparams: (params: SuccessorParam[]) => void
  } = $props()

  /** Room above each sheet for its title, px. */
  const TITLE = 22
  /** Space around each panel, px. */
  const GAP = 12
  /** Panels narrower than this title themselves with the model name only, px. */
  const WIDE_TITLES = 240

  let lab: SuccessorLab | null = null
  /** The parameter values last given to the lab, to change only what changed. */
  let applied: Record<string, number> = {}
  let known: SuccessorParam[] = []
  let since = -Infinity
  let densityDirty = true
  const densityImage = new DensityImage()
  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  function report() {
    if (lab) onstats(readSuccessorStats(lab))
  }

  // Read the props first: `lab?.` skips its arguments while `lab` is null,
  // and the effects would then never track them.
  $effect(() => {
    const n = nodes
    const t = steps
    const s = seed
    if (!lab) {
      lab = new wasm.SuccessorLab(s)
      known = readSuccessorParams(lab)
      applied = Object.fromEntries(known.map(p => [p.id, p.value]))
      untrack(() => {
        loadDensity()
        lab!.set_continuous(continuous)
        onparams(known)
      })
    }
    lab.set_nodes(n)
    lab.set_steps(t)
    lab.set_seed(s)
    densityDirty = true
    report()
  })

  function loadDensity() {
    if (!lab) return
    if (isMoving(density)) {
      lab.set_motion(density, motionPeriod)
    } else if (density === 'image') {
      if (!image || !lab.set_density_cells(image.cells, image.side)) lab.set_density('blank')
    } else {
      lab.set_density(density)
    }
    densityDirty = true
  }

  $effect(() => {
    void density
    void image
    void densityReset
    untrack(() => {
      loadDensity()
      lab?.restart()
      report()
    })
  })

  $effect(() => {
    const on = continuous
    lab?.set_continuous(on)
  })

  // A new period keeps the run going.
  $effect(() => {
    const period = motionPeriod
    untrack(() => {
      if (lab && isMoving(density)) lab.set_motion(density, period)
    })
  })

  // Only parameters that changed go to the lab: the scheduled ones start over.
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
    if (stepRequest && lab) {
      lab.step(1)
      report()
    }
  })

  /** The sheets: five models and the density, three across when wide, two when tall. */
  function layout(width: number, height: number): Sheet[] {
    const cols = width >= height ? 3 : 2
    const rows = Math.ceil(6 / cols)
    const cellW = width / cols
    const cellH = height / rows
    const size = Math.max(1, Math.min(cellW - 2 * GAP, cellH - 2 * GAP - TITLE))
    return Array.from({ length: 6 }, (_, i) => {
      const col = i % cols
      const row = Math.floor(i / cols)
      return {
        x: col * cellW + (cellW - size) / 2,
        y: row * cellH + (cellH - size - TITLE) / 2 + TITLE,
        size,
      }
    })
  }

  /** The sheet under `p` and the point in its unit square, if any. */
  function hit(p: Point): Point | null {
    for (const s of layout(canvas.size.width, canvas.size.height)) {
      const u = toUnit(s, p)
      if (u.x >= 0 && u.x <= 1 && u.y >= 0 && u.y <= 1) return u
    }
    return null
  }

  /** Painting: press and drag on any panel. */
  const paint: Attachment<HTMLCanvasElement> = element => {
    let painting = false
    const local = (e: PointerEvent): Point => {
      const rect = element.getBoundingClientRect()
      return { x: e.clientX - rect.left, y: e.clientY - rect.top }
    }
    const stroke = (e: PointerEvent) => {
      const u = hit(local(e))
      if (!lab || !u) return
      lab.paint(u.x, u.y, brush.radius, brush.mode === 'add' ? 0.5 : -1)
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

  onFrame(time => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) lab.step(speed)

    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    const sheets = layout(width, height)
    if (densityDirty || (running && isMoving(density))) {
      densityImage.update(t, lab.density())
      densityDirty = false
    }

    clear(ctx, dpr)
    sheets.forEach((s, i) => {
      densityImage.draw(ctx, s)
      drawFrame(ctx, t, s)
      const model = MODELS[i]
      if (!model) {
        label(ctx, t, s.size >= WIDE_TITLES ? 'Density · paint on any panel' : 'Density', { x: s.x, y: s.y - TITLE / 2 }, t.inkSoft, 'left')
        return
      }
      const color = t[model.tone]
      if (model.id === 'kmeans' || showVoronoi) drawVoronoi(ctx, t, s, lab!.voronoi(model.id))
      drawLattice(ctx, s, lab!.weights(model.id), lab!.edges(model.id), color)
      const [qe, , n] = lab!.errors(model.id)
      // Narrow panels keep to the name; the numbers are in the table below.
      const tag = Number.isFinite(qe) && s.size >= WIDE_TITLES ? ` · ${n} nodes · QE ${qe.toFixed(3)}` : ''
      label(ctx, t, `${model.label}${tag}`, { x: s.x, y: s.y - TITLE / 2 }, color, 'left')
    })
    if (canvas.pointer) {
      const p = canvas.pointer
      const s = sheets.find(s => p.x >= s.x && p.x <= s.x + s.size && p.y >= s.y && p.y <= s.y + s.size)
      if (s) drawBrush(ctx, t, s, p, brush.radius)
    }

    if (running && time - since >= 250) {
      since = time
      report()
    }
  })
</script>

<canvas {@attach canvas.attach} {@attach paint}>
  A self-organizing map, neural gas, growing neural gas with and without utility, and k-means, side by side on a
  density you can paint
</canvas>
