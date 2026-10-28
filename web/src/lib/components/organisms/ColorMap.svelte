<!--
  @component
  A self-organizing map of colors, drawn as its cells: trained on random
  colors, or on an image's pixels, whose palette it becomes; the image is
  shown next to it recolored with only the map's colors.
-->
<script lang="ts">
  import type { SomData } from 'silly_demos'
  import { untrack } from 'svelte'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { clear, label } from '../../canvas/draft.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'
  import { cellLayout, drawCells, type Box } from '../../demos/kohonen/cells.ts'
  import type { Pixels } from '../../demos/kohonen/image.ts'
  import { applyDataConfig, readStats, type SomConfig, type SomStats, type TrainingId } from '../../demos/kohonen/kohonen.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    source,
    pixels,
    config,
    training,
    running,
    speed,
    showUmatrix,
    restart = 0,
    stepRequest = 0,
    onstats,
  }: {
    wasm: Wasm
    /** Random colors, or the pixels of `pixels`. */
    source: 'rgb' | 'image'
    pixels: Pixels | null
    /** Changing any of it starts the map over; chains and rings become grids. */
    config: SomConfig
    training: TrainingId
    running: boolean
    /** Online: samples per frame. Batch: epochs per second. */
    speed: number
    /** Shades the cells by their distance to their neighbors. */
    showUmatrix: boolean
    /** Changing this value starts the map over. */
    restart?: number
    /** Each increment advances one update while paused. */
    stepRequest?: number
    onstats: (stats: SomStats) => void
  } = $props()

  /** Random colors to train on in the RGB source. */
  const RANDOM_COLORS = 2000
  /** How often the recolored image is redrawn while training, ms. */
  const QUANTIZE_EVERY = 250

  let lab: SomData | null = null
  let epochs = 0
  let since = -Infinity
  let quantizedAt = -Infinity
  let quantizedDirty = true
  // Offscreen canvases for the original and the recolored image.
  let original: HTMLCanvasElement | null = null
  let quantized: HTMLCanvasElement | null = null

  const canvas = new DrawingCanvas()

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  function report() {
    if (lab) onstats(readStats(lab))
  }

  /** Uniform random colors from a seeded generator (mulberry32). */
  function randomColors(n: number, seed: number): Float32Array {
    let a = seed >>> 0
    const next = () => {
      a = (a + 0x6d2b79f5) >>> 0
      let t = a
      t = Math.imul(t ^ (t >>> 15), t | 1)
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296
    }
    return Float32Array.from({ length: n * 3 }, () => next() * 255)
  }

  /** The image's opaque pixels as RGB rows. */
  function pixelRows(p: Pixels): Float32Array {
    const rows: number[] = []
    for (let i = 0; i < p.rgba.length; i += 4) {
      if (p.rgba[i + 3] < 128) continue
      rows.push(p.rgba[i], p.rgba[i + 1], p.rgba[i + 2])
    }
    return Float32Array.from(rows)
  }

  function toCanvas(p: Pixels, data: Uint8ClampedArray, target: HTMLCanvasElement | null): HTMLCanvasElement {
    const c = target ?? document.createElement('canvas')
    c.width = p.width
    c.height = p.height
    c.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(data), p.width, p.height), 0, 0)
    return c
  }

  // Only the seed, not the whole config: the data needn't be rebuilt on every slider tick.
  let dataSeed = $derived(config.seed)

  // The data: random colors (seeded by the config's seed) or the image's pixels.
  $effect(() => {
    const s = source
    const p = pixels
    const seed = dataSeed
    if (!lab) {
      lab = new wasm.SomData(seed)
      lab.set_normalize(false)
      untrack(() => {
        lab!.set_training(training)
        applyDataConfig(lab!, $state.snapshot(config) as SomConfig)
      })
    }
    if (s === 'rgb') lab.set_data(randomColors(RANDOM_COLORS, seed), 3)
    else if (p) {
      lab.set_data(pixelRows(p), 3)
      original = toCanvas(p, p.rgba, original)
    }
    quantizedDirty = true
    report()
  })

  // Read the props first: `lab?.` skips its arguments while `lab` is null,
  // and the effects would then never track them.
  $effect(() => {
    const c = $state.snapshot(config) as SomConfig
    if (!lab) return
    applyDataConfig(lab, c)
    quantizedDirty = true
    report()
  })

  $effect(() => {
    const m = training
    lab?.set_training(m)
  })

  $effect(() => {
    if (restart && lab) {
      lab.restart()
      quantizedDirty = true
      report()
    }
  })

  $effect(() => {
    if (stepRequest && lab) {
      lab.step(1)
      quantizedDirty = true
      report()
    }
  })

  onFrame((time, dt) => {
    const ctx = canvas.context()
    if (!lab || !ctx) return
    if (running) {
      let n = 0
      if (training === 'online') n = lab.step(speed)
      else {
        epochs += Math.min(dt, 0.1) * speed
        const whole = Math.floor(epochs)
        epochs -= whole
        if (whole > 0) n = lab.step(whole)
      }
      if (n > 0) quantizedDirty = true
    }

    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    const margin = 16
    const withImage = source === 'image' && pixels !== null
    // The map on the left; with an image, the original and its recoloring stacked on the right.
    const mapBox: Box = withImage
      ? { x: margin, y: margin, width: width * 0.6 - 1.5 * margin, height: height - 2 * margin }
      : { x: margin, y: margin, width: width - 2 * margin, height: height - 2 * margin }

    clear(ctx, dpr)
    const codebook = lab.codebook()
    const nodes = codebook.length / 3
    if (nodes > 0) {
      const kind = config.lattice === 'hex' ? 'hex' : 'grid'
      const w = config.width
      const h = nodes / w
      const layout = cellLayout(kind, w, h, mapBox)
      const rgb = (i: number) =>
        `rgb(${Math.round(codebook[i * 3])} ${Math.round(codebook[i * 3 + 1])} ${Math.round(codebook[i * 3 + 2])})`
      drawCells(ctx, layout, rgb, t.paper)
      if (showUmatrix) {
        const u = lab.umatrix()
        const max = Math.max(...u, 1e-9)
        ctx.save()
        drawCells(ctx, layout, i => `rgb(0 0 0 / ${((u[i] / max) * 0.7).toFixed(3)})`)
        ctx.restore()
      }
    }

    if (withImage && pixels) {
      if (quantizedDirty && (!running || time - quantizedAt >= QUANTIZE_EVERY)) {
        quantizedAt = time
        quantizedDirty = false
        quantized = toCanvas(pixels, Uint8ClampedArray.from(lab.quantize(new Uint8Array(pixels.rgba))), quantized)
      }
      const x = width * 0.6 + margin / 2
      const boxW = width - x - margin
      const boxH = (height - 3 * margin) / 2
      const panels: [HTMLCanvasElement | null, string, number][] = [
        [original, 'Original', margin],
        [quantized, `${nodes} colors`, 2 * margin + boxH],
      ]
      for (const [img, text, y] of panels) {
        if (!img) continue
        const scale = Math.min(boxW / img.width, (boxH - 20) / img.height)
        const iw = img.width * scale
        const ih = img.height * scale
        ctx.save()
        ctx.imageSmoothingEnabled = scale < 1
        ctx.drawImage(img, x + (boxW - iw) / 2, y + 20, iw, ih)
        ctx.restore()
        label(ctx, t, text, { x: x + boxW / 2, y: y + 8 }, t.inkSoft)
      }
    }

    if (running && time - since >= 250) {
      since = time
      report()
    }
  })
</script>

<canvas {@attach canvas.attach}>A self-organizing map of colors, drawn as a grid of colored cells</canvas>
