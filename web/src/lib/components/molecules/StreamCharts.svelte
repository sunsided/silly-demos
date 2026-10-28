<!--
  @component
  A stream clustering's record: how many clusters each method finds over
  time against the true number, and how well each matches the true
  clusters (adjusted Rand index), beside batch k-means told the true k.
-->
<script lang="ts">
  let {
    history,
  }: {
    /** Flat rows of seven: t, true k, balls k, sketch k, ARI balls, ARI sketch, ARI batch k-means. */
    history: Float32Array
  } = $props()

  const WIDTH = 300
  const HEIGHT = 90
  const STRIDE = 7

  let charts = $derived.by(() => {
    const n = Math.floor(history.length / STRIDE)
    if (n === 0) return null
    const t0 = history[0]
    const span = Math.max(history[(n - 1) * STRIDE] - t0, 1)
    let kMax = 1
    for (let i = 0; i < n; i++) for (const c of [1, 2, 3]) kMax = Math.max(kMax, history[i * STRIDE + c])
    const line = (col: number, lo: number, hi: number) => {
      let points = ''
      for (let i = 0; i < n; i++) {
        const x = ((history[i * STRIDE] - t0) / span) * WIDTH
        const v = Math.min(hi, Math.max(lo, history[i * STRIDE + col]))
        const y = HEIGHT - ((v - lo) / (hi - lo)) * HEIGHT
        points += `${x.toFixed(1)},${y.toFixed(1)} `
      }
      return points
    }
    const last = (col: number) => history[(n - 1) * STRIDE + col]
    return {
      k: { truth: line(1, 0, kMax + 1), balls: line(2, 0, kMax + 1), sketch: line(3, 0, kMax + 1), max: kMax + 1 },
      ari: { balls: line(4, 0, 1), sketch: line(5, 0, 1), batch: line(6, 0, 1) },
      last: { truth: last(1), balls: last(2), sketch: last(3), ariBalls: last(4), ariSketch: last(5), ariBatch: last(6) },
    }
  })
</script>

{#if charts}
  <figure>
    <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="none" role="img" aria-label="Clusters found over time">
      <line class="axis" x1="0" y1={HEIGHT} x2={WIDTH} y2={HEIGHT} />
      <polyline class="truth" points={charts.k.truth} />
      <polyline class="balls" points={charts.k.balls} />
      <polyline class="sketch" points={charts.k.sketch} />
    </svg>
    <figcaption>
      <span>Clusters, 0 to {charts.k.max}:</span>
      <span class="truth">true {charts.last.truth}</span>
      <span class="balls">balls {charts.last.balls}</span>
      <span class="sketch">sketch {charts.last.sketch}</span>
    </figcaption>
  </figure>
  <figure>
    <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="none" role="img" aria-label="Agreement with the true clusters over time">
      <line class="axis" x1="0" y1={HEIGHT} x2={WIDTH} y2={HEIGHT} />
      <polyline class="batch" points={charts.ari.batch} />
      <polyline class="balls" points={charts.ari.balls} />
      <polyline class="sketch" points={charts.ari.sketch} />
    </svg>
    <figcaption>
      <span>Adjusted Rand index, 0 to 1:</span>
      <span class="balls">balls {charts.last.ariBalls.toFixed(2)}</span>
      <span class="sketch">sketch {charts.last.ariSketch.toFixed(2)}</span>
      <span class="batch">k-means at the true k {charts.last.ariBatch.toFixed(2)}</span>
    </figcaption>
  </figure>
{/if}

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

  polyline.truth {
    stroke: var(--color-ink);
    stroke-dasharray: 4 3;
  }

  polyline.balls {
    stroke: var(--color-cobalt);
  }

  polyline.sketch {
    stroke: var(--color-amber);
  }

  polyline.batch {
    stroke: var(--color-ink-faint);
  }

  figcaption {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2xs) var(--space-md);
    margin-top: var(--space-xs);
    font-size: var(--font-size-xs);
    font-variant-numeric: tabular-nums;
    color: var(--color-ink-soft);
  }

  span.truth {
    color: var(--color-ink);
  }

  span.balls {
    color: var(--color-cobalt);
  }

  span.sketch {
    color: var(--color-amber);
  }
</style>
