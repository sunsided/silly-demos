<!--
  @component
  Quantization error over training for several models on one shared scale,
  one line each in its model's tone.
-->
<script lang="ts">
  import type { Tone } from '../types.ts'

  let {
    series,
  }: {
    /** One line per model: (t, QE, TE) triples, as SuccessorLab.history returns. */
    series: readonly { label: string; tone: Tone; history: Float32Array }[]
  } = $props()

  const WIDTH = 300
  const HEIGHT = 110

  let lines = $derived.by(() => {
    let tMax = 1
    let qeMax = 1e-6
    for (const { history } of series) {
      for (let i = 0; i + 2 < history.length; i += 3) {
        tMax = Math.max(tMax, history[i])
        qeMax = Math.max(qeMax, history[i + 1])
      }
    }
    return series.map(({ label, tone, history }) => {
      let points = ''
      for (let i = 0; i + 2 < history.length; i += 3) {
        const x = (history[i] / tMax) * WIDTH
        const y = HEIGHT - (history[i + 1] / qeMax) * HEIGHT
        points += `${x.toFixed(1)},${y.toFixed(1)} `
      }
      const last = history.length >= 3 ? history[history.length - 2] : NaN
      return { label, tone, points, last }
    })
  })
</script>

<figure>
  <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="none" role="img" aria-label="Quantization error over time, per model">
    <line class="axis" x1="0" y1={HEIGHT} x2={WIDTH} y2={HEIGHT} />
    {#each lines as line (line.label)}
      <polyline class="tone-{line.tone}" points={line.points} />
    {/each}
  </svg>
  <figcaption>Quantization error over samples, on one scale</figcaption>
</figure>

<style>
  figure {
    margin: 0 0 var(--space-md);
  }

  svg {
    display: block;
    width: 100%;
    height: 110px;
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

  .tone-ink {
    stroke: var(--color-ink);
  }

  .tone-cobalt {
    stroke: var(--color-cobalt);
  }

  .tone-signal {
    stroke: var(--color-signal);
  }

  .tone-verdigris {
    stroke: var(--color-verdigris);
  }

  .tone-amber {
    stroke: var(--color-amber);
  }

  figcaption {
    margin-top: var(--space-xs);
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }
</style>
