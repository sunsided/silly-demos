<!--
  @component
  The world poverty map: countries in 1992 and 2022 on one self-organising
  map, drawn as hex cells with their U-matrix (or one indicator's plane), and
  a world map with every country in the colour of its cell. Hovering a cell
  outlines its countries on the world map; hovering a country marks its cell.
-->
<script lang="ts">
  import type { SomData } from 'silly_demos'
  import { untrack } from 'svelte'
  import { DrawingCanvas, onFrame } from '../../canvas/canvas.svelte.ts'
  import { arrowhead, clear, label } from '../../canvas/draft.ts'
  import { currentTheme, darkScheme } from '../../canvas/theme.svelte.ts'
  import { cellAt, cellLayout, cellPath, type CellLayout } from '../../demos/kohonen/cells.ts'
  import {
    joinShapes,
    loadCountries,
    loadWorld,
    trainingRows,
    wheelColour,
    type Country,
    type CountryData,
    type Planes,
    type World,
    type YearView,
  } from '../../demos/kohonen/countries.ts'
  import { applyDataConfig, readStats, type SomConfig, type SomStats, type TrainingId } from '../../demos/kohonen/kohonen.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    config,
    training,
    running,
    speed,
    view,
    plane = null,
    restart = 0,
    stepRequest = 0,
    onstats,
    onselect,
    onplanes,
  }: {
    wasm: Wasm
    /** Changing any of it starts the map over; chains and rings become grids. */
    config: SomConfig
    training: TrainingId
    running: boolean
    /** Online: samples per frame. Batch: epochs per second. */
    speed: number
    view: YearView
    /** An indicator whose plane shades the cells instead of the U-matrix. */
    plane?: number | null
    /** Changing this value starts the map over. */
    restart?: number
    /** Each increment advances one update while paused. */
    stepRequest?: number
    onstats: (stats: SomStats) => void
    /** The country under the pointer, or the countries in the cell under it. */
    onselect: (selection: { country: Country | null; cell: Country[] }) => void
    /** The component planes, a few times a second while training. */
    onplanes: (planes: Planes) => void
  } = $props()

  let data = $state.raw<CountryData | null>(null)
  let world = $state.raw<World | null>(null)
  let error = $state('')
  let lab: SomData | null = null
  /** Each row's cell: countries in 1992, then in 2022. */
  let bmus = $state.raw<Uint32Array>(new Uint32Array())
  let hoverIso = $state<string | null>(null)
  let hoverCell = $state(-1)
  let epochs = 0
  let since = -Infinity

  const canvas = new DrawingCanvas()

  $effect(() => {
    Promise.all([loadCountries(), loadWorld()]).then(
      ([d, w]) => {
        data = d
        world = w
      },
      () => (error = "Couldn't load the country data"),
    )
  })

  $effect(() => () => {
    lab?.free()
    lab = null
  })

  let kind = $derived<'grid' | 'hex'>(config.lattice === 'hex' ? 'hex' : 'grid')
  let shapes = $derived(data && world ? joinShapes(data, world) : null)
  let index = $derived(new Map(data?.countries.map((c, i) => [c.iso3, i]) ?? []))
  /** The colour of every cell: hue by direction from the map's centre, saturation by distance. */
  let wheel = $derived.by(() => {
    const unit = cellLayout(kind, config.width, config.height, { x: 0, y: 0, width: 1, height: 1 })
    const n = unit.centres.length
    const cx = unit.centres.reduce((s, p) => s + p.x, 0) / n
    const cy = unit.centres.reduce((s, p) => s + p.y, 0) / n
    const maxR = Math.max(...unit.centres.map(p => Math.hypot(p.x - cx, p.y - cy)), 1e-9)
    const dark = darkScheme()
    return unit.centres.map(p => wheelColour(p.x - cx, p.y - cy, maxR, dark))
  })

  /** A country's cell in the year shown (2022 when both are). */
  function cellOf(i: number): number {
    const n = data?.countries.length ?? 0
    const row = view === '1992' ? i : n + i
    return bmus[row] ?? -1
  }

  function report() {
    if (!lab || !data) return
    onstats(readStats(lab))
    bmus = Uint32Array.from(lab.bmus())
    onplanes({
      kind,
      width: config.width,
      height: config.height,
      indicators: data.indicators,
      values: data.indicators.map((_, k) => lab!.plane(k)),
    })
  }

  // The lab, once the data is in.
  $effect(() => {
    const d = data
    if (!d || lab) return
    const fresh = new wasm.SomData(config.seed)
    const { rows, dim } = trainingRows(d)
    untrack(() => {
      fresh.set_training(training)
      applyDataConfig(fresh, $state.snapshot(config) as SomConfig)
    })
    fresh.set_data(rows, dim)
    lab = fresh
    untrack(report)
  })

  // Read the props first: `lab?.` skips its arguments while `lab` is null,
  // and the effects would then never track them.
  $effect(() => {
    const c = $state.snapshot(config) as SomConfig
    if (!lab) return
    applyDataConfig(lab, c)
    untrack(report)
  })

  $effect(() => {
    const m = training
    lab?.set_training(m)
  })

  $effect(() => {
    if (restart && lab) {
      lab.restart()
      untrack(report)
    }
  })

  $effect(() => {
    if (stepRequest && lab) {
      lab.step(1)
      untrack(report)
    }
  })

  // Tell the page what is under the pointer.
  $effect(() => {
    const d = data
    if (!d) return
    const country = hoverIso !== null ? (d.countries[index.get(hoverIso) ?? -1] ?? null) : null
    const cell = hoverCell >= 0 ? d.countries.filter((_, i) => cellOf(i) === hoverCell) : []
    untrack(() => onselect({ country, cell }))
  })

  function shade(ctx: CanvasRenderingContext2D, layout: CellLayout, values: ArrayLike<number>, color: string, max: number) {
    ctx.save()
    ctx.fillStyle = color
    for (let i = 0; i < layout.centres.length; i++) {
      ctx.globalAlpha = Math.max(0, Math.min(1, values[i] / max))
      ctx.beginPath()
      cellPath(ctx, layout, i)
      ctx.fill()
    }
    ctx.restore()
  }

  onFrame((time, dt) => {
    const ctx = canvas.context()
    if (!ctx) return
    if (lab && running) {
      if (training === 'online') lab.step(speed)
      else {
        epochs += Math.min(dt, 0.1) * speed
        const whole = Math.floor(epochs)
        epochs -= whole
        if (whole > 0) lab.step(whole)
      }
    }

    const t = currentTheme()
    const { width, height, dpr } = canvas.size
    clear(ctx, dpr)
    if (!lab || !data) return
    const m = 16
    const layout = cellLayout(kind, config.width, config.height, { x: m, y: m, width: width - 2 * m, height: height - 2 * m })
    const n = data.countries.length

    // Cells: the position colour, darkened by the U-matrix; or one indicator's plane.
    ctx.save()
    ctx.globalAlpha = plane === null ? 0.5 : 0.12
    for (let i = 0; i < layout.centres.length; i++) {
      ctx.beginPath()
      cellPath(ctx, layout, i)
      ctx.fillStyle = wheel[i] ?? t.inkFaint
      ctx.fill()
    }
    ctx.restore()
    if (plane === null) {
      const u = lab.umatrix()
      shade(ctx, layout, u, t.ink, Math.max(...u, 1e-9) / 0.6)
    } else {
      const p = lab.plane(plane)
      const lo = Math.min(...p)
      const hi = Math.max(...p)
      shade(ctx, layout, p.map(v => v - lo), t.cobalt, Math.max(hi - lo, 1e-9))
    }
    ctx.save()
    ctx.strokeStyle = t.paper
    ctx.lineWidth = 1
    for (let i = 0; i < layout.centres.length; i++) {
      ctx.beginPath()
      cellPath(ctx, layout, i)
      ctx.stroke()
    }
    ctx.restore()

    // Where each country moved from 1992 to 2022.
    if (view === 'both' && bmus.length === 2 * n) {
      ctx.save()
      ctx.strokeStyle = t.amber
      ctx.fillStyle = t.amber
      ctx.lineWidth = 1
      // Two hundred arrows at once are a thicket; faint, they show the drift, and a hovered one stands out.
      ctx.globalAlpha = hoverIso === null ? 0.3 : 0.12
      for (let i = 0; i < n; i++) {
        const a = layout.centres[bmus[i]]
        const b = layout.centres[bmus[n + i]]
        if (!a || !b || bmus[i] === bmus[n + i]) continue
        if (hoverIso !== null && data.countries[i].iso3 === hoverIso) continue
        drawArrow(ctx, a, b)
      }
      if (hoverIso !== null) {
        const i = index.get(hoverIso)
        if (i !== undefined && bmus[i] !== bmus[n + i]) {
          ctx.globalAlpha = 1
          ctx.lineWidth = 2
          drawArrow(ctx, layout.centres[bmus[i]], layout.centres[bmus[n + i]])
        }
      }
      ctx.restore()
    }

    // Country codes in their cells: two per cell, then a count.
    const perCell = new Map<number, string[]>()
    data.countries.forEach((c, i) => {
      const cell = cellOf(i)
      if (cell >= 0) perCell.set(cell, [...(perCell.get(cell) ?? []), c.iso3])
    })
    ctx.save()
    ctx.font = `500 ${Math.max(8, Math.min(11, layout.radius * 0.42))}px ${t.font}`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    for (const [cell, codes] of perCell) {
      const { x, y } = layout.centres[cell]
      const lines = codes.length <= 2 ? codes : [codes[0], `+${codes.length - 1}`]
      const lh = layout.radius * 0.5
      lines.forEach((code, k) => {
        ctx.fillStyle = code === hoverIso ? t.signal : t.ink
        ctx.fillText(code, x, y + (k - (lines.length - 1) / 2) * lh)
      })
    }
    ctx.restore()

    // The hovered cell, and the hovered country's cell.
    const marked = [hoverCell, hoverIso !== null ? cellOf(index.get(hoverIso) ?? -1) : -1]
    ctx.save()
    ctx.strokeStyle = t.signal
    ctx.lineWidth = 2
    for (const cell of marked) {
      if (cell < 0 || cell >= layout.centres.length) continue
      ctx.beginPath()
      cellPath(ctx, layout, cell)
      ctx.stroke()
    }
    ctx.restore()

    const under = canvas.pointer ? cellAt(layout, canvas.pointer) : -1
    if (under !== hoverCell) hoverCell = under
    if (hoverCell >= 0) {
      const count = perCell.get(hoverCell)?.length ?? 0
      const { x, y } = layout.centres[hoverCell]
      label(ctx, t, `${count} ${count === 1 ? 'country' : 'countries'}`, { x, y: y - layout.radius - 8 }, t.signal)
    }

    if (running && time - since >= 250) {
      since = time
      report()
    }
  })

  function drawArrow(ctx: CanvasRenderingContext2D, a: { x: number; y: number }, b: { x: number; y: number }) {
    const dx = b.x - a.x
    const dy = b.y - a.y
    const len = Math.hypot(dx, dy)
    ctx.beginPath()
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(b.x, b.y)
    ctx.stroke()
    arrowhead(ctx, b, dx / len, dy / len, 5)
  }

  /** Fill for a country on the world map: its cell's colour, or none without data. */
  function fill(iso: string): string | null {
    const i = index.get(iso)
    if (i === undefined) return null
    const cell = cellOf(i)
    return cell >= 0 ? (wheel[cell] ?? null) : null
  }

  let cellCountries = $derived(
    data && hoverCell >= 0 ? new Set(data.countries.filter((_, i) => cellOf(i) === hoverCell).map(c => c.iso3)) : new Set(),
  )
</script>

<div class="countries">
  <div class="panel map">
    {#if error}
      <p class="error">{error}</p>
    {:else}
      <canvas {@attach canvas.attach}>The countries on a hexagonal self-organising map</canvas>
    {/if}
  </div>
  <div class="panel world">
    {#if world && shapes}
      <svg viewBox="0 0 {world.width} {world.height}" role="img" aria-label="World map, countries coloured by their cell on the map">
        <defs>
          <pattern id="no-data" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <line x1="0" y1="0" x2="0" y2="6" class="hatch" />
          </pattern>
        </defs>
        {#each Object.entries(world.countries) as [iso, d] (iso)}
          {@const color = fill(iso)}
          <path
            {d}
            class:no-data={color === null}
            class:hovered={hoverIso === iso}
            class:in-cell={cellCountries.has(iso)}
            style:fill={color ?? undefined}
            role="presentation"
            onpointerenter={() => (hoverIso = index.has(iso) ? iso : null)}
            onpointerleave={() => (hoverIso = null)}
          />
        {/each}
      </svg>
    {/if}
  </div>
</div>

<style>
  .countries {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-rows: minmax(0, 3fr) minmax(0, 2fr);
  }

  .panel {
    position: relative;
    min-height: 0;
  }

  .world {
    padding: 0 16px 16px;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  path {
    fill: var(--color-ink-faint);
    stroke: var(--color-paper);
    stroke-width: 0.5;
  }

  path.no-data {
    fill: url(#no-data);
  }

  .hatch {
    stroke: var(--color-ink-faint);
    stroke-width: 2;
  }

  path.in-cell {
    stroke: var(--color-signal);
    stroke-width: 1.5;
  }

  path.hovered {
    stroke: var(--color-signal);
    stroke-width: 2;
  }

  .error {
    padding: var(--space-lg);
    color: var(--color-signal);
  }
</style>
