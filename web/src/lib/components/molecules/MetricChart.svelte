<!--
  @component
  Quantization and topographic error over training time, each on its own
  scale from 0 to its largest value so far.
-->
<script lang="ts">
  let { history }: { history: Float32Array } = $props()

  const WIDTH = 300
  const HEIGHT = 90

  interface Series {
    points: string
    last: number
  }

  let lines = $derived.by(() => {
    const n = Math.floor(history.length / 3)
    if (n === 0) return null
    const tMax = Math.max(history[(n - 1) * 3], 1)
    const series = (offset: number, floor: number): Series => {
      let max = floor
      for (let i = 0; i < n; i++) max = Math.max(max, history[i * 3 + offset])
      let points = ''
      for (let i = 0; i < n; i++) {
        const x = (history[i * 3] / tMax) * WIDTH
        const y = HEIGHT - (history[i * 3 + offset] / max) * HEIGHT
        points += `${x.toFixed(1)},${y.toFixed(1)} `
      }
      return { points, last: history[(n - 1) * 3 + offset] }
    }
    // TE is a share; a floor keeps a flat-zero curve from filling the chart.
    return { qe: series(1, 1e-6), te: series(2, 0.05), t: tMax }
  })
</script>

<figure>
  <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="none" role="img" aria-label="Quantization and topographic error over time">
    <line class="axis" x1="0" y1={HEIGHT} x2={WIDTH} y2={HEIGHT} />
    {#if lines}
      <polyline class="qe" points={lines.qe.points} />
      <polyline class="te" points={lines.te.points} />
    {/if}
  </svg>
  <figcaption>
    <span class="qe">QE {lines ? lines.qe.last.toFixed(3) : '–'}</span>
    <span class="te">TE {lines ? lines.te.last.toFixed(2) : '–'}</span>
    <span class="t">t = {lines ? Math.round(lines.t).toLocaleString() : 0}</span>
  </figcaption>
</figure>

<style>
  figure {
    margin: 0 0 var(--space-md);
  }

  svg {
    display: block;
    width: 100%;
    height: 90px;
    overflow: visible;
  }

  polyline,
  line {
    fill: none;
    vector-effect: non-scaling-stroke;
    stroke-width: 1.5;
  }

  .axis {
    stroke: var(--color-ink-faint);
    stroke-width: 1;
  }

  polyline.qe {
    stroke: var(--color-cobalt);
  }

  polyline.te {
    stroke: var(--color-amber);
  }

  figcaption {
    display: flex;
    gap: var(--space-md);
    margin-top: var(--space-xs);
    font-size: var(--font-size-sm);
    font-variant-numeric: tabular-nums;
  }

  span.qe {
    color: var(--color-cobalt);
  }

  span.te {
    color: var(--color-amber);
  }

  span.t {
    margin-left: auto;
    color: var(--color-ink-soft);
  }
</style>
