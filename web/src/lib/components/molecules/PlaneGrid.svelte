<!--
  @component
  Component planes as small multiples: one tiny map per indicator, each cell
  shaded by that indicator's value at the node. Click one to show it large;
  click again to go back to the U-matrix.
-->
<script lang="ts">
  import { cellLayout, cellPath } from '../../demos/kohonen/cells.ts'
  import type { Planes } from '../../demos/kohonen/countries.ts'
  import { currentTheme } from '../../canvas/theme.svelte.ts'

  let { planes, selected = $bindable(null) }: { planes: Planes; selected?: number | null } = $props()

  const WIDTH = 120

  let height = $derived(planes.kind === 'hex' ? Math.round(WIDTH * 0.62) : Math.round((WIDTH * planes.height) / planes.width))

  function draw(canvas: HTMLCanvasElement, values: Float32Array) {
    const dpr = window.devicePixelRatio || 1
    canvas.width = WIDTH * dpr
    canvas.height = height * dpr
    const ctx = canvas.getContext('2d')!
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, WIDTH, height)
    const layout = cellLayout(planes.kind, planes.width, planes.height, { x: 1, y: 1, width: WIDTH - 2, height: height - 2 })
    const lo = Math.min(...values)
    const range = Math.max(Math.max(...values) - lo, 1e-9)
    const t = currentTheme()
    ctx.fillStyle = t.cobalt
    for (let i = 0; i < layout.centres.length; i++) {
      ctx.globalAlpha = 0.08 + 0.92 * ((values[i] - lo) / range)
      ctx.beginPath()
      cellPath(ctx, layout, i)
      ctx.fill()
    }
  }
</script>

<div class="planes">
  {#each planes.indicators as indicator, k (indicator.code)}
    <button
      type="button"
      class:selected={selected === k}
      aria-pressed={selected === k}
      onclick={() => (selected = selected === k ? null : k)}
    >
      <canvas
        style:width="{WIDTH}px"
        style:height="{height}px"
        {@attach c => draw(c, planes.values[k])}
      ></canvas>
      <span>{indicator.name}</span>
    </button>
  {/each}
</div>

<style>
  .planes {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-sm);
    margin: 0 0 var(--space-md);
  }

  button {
    display: grid;
    justify-items: center;
    gap: var(--space-3xs);
    padding: var(--space-2xs);
    border: var(--border-hairline) solid transparent;
    border-radius: var(--border-radius);
    background: none;
    color: var(--color-ink-soft);
    font: inherit;
    font-size: var(--font-size-xs);
    cursor: pointer;
  }

  button:hover,
  .selected {
    border-color: var(--color-cobalt);
    color: var(--color-ink);
  }

  canvas {
    display: block;
  }
</style>
