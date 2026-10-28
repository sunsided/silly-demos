<!--
  @component
  How a search through a tree trades distances for recall: recall@10
  against distances per query, one line per tree with a point per beam
  width, on a log scale reaching to what a scan of every point takes.
-->
<script lang="ts">
  import type { SearchCurve } from '../../demos/clustering/tree.ts'

  let {
    curves,
  }: {
    /** One curve per tree, with the CSS class it is drawn in. */
    curves: readonly { label: string; tone: 'cobalt' | 'verdigris' | 'amber'; curve: SearchCurve }[]
  } = $props()

  const WIDTH = 300
  const HEIGHT = 120
  const PAD = 4

  let chart = $derived.by(() => {
    if (curves.length === 0) return null
    const scan = Math.max(...curves.map(c => c.curve.scan))
    const lo = Math.log10(10)
    const hi = Math.log10(Math.max(scan, 100))
    const x = (cost: number) => PAD + ((Math.log10(Math.max(cost, 10)) - lo) / (hi - lo)) * (WIDTH - 2 * PAD)
    const y = (recall: number) => HEIGHT - PAD - recall * (HEIGHT - 2 * PAD)
    return {
      scanX: x(scan),
      scan,
      lines: curves.map(c => ({
        ...c,
        points: c.curve.beams.map(b => ({ x: x(b.cost), y: y(b.recall), b })),
      })),
    }
  })
</script>

{#if chart}
  <figure>
    <svg viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="Recall against distances per query">
      <line class="axis" x1={PAD} y1={HEIGHT - PAD} x2={WIDTH - PAD} y2={HEIGHT - PAD} />
      <line class="scan" x1={chart.scanX} y1={PAD} x2={chart.scanX} y2={HEIGHT - PAD} />
      {#each chart.lines as line (line.label)}
        <polyline class="tone-{line.tone}" points={line.points.map(p => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')} />
        {#each line.points as p (p.b.beam)}
          <circle class="tone-{line.tone}" cx={p.x} cy={p.y} r="2.5" />
        {/each}
      {/each}
    </svg>
    <figcaption>
      Recall@10, 0 to 1, against distances per query, 10 to {chart.scan.toLocaleString()} (a scan) on a log scale; a
      point per beam of 1, 2, 4 and 8.
    </figcaption>
  </figure>
  <div class="scroll">
  <table>
    <thead>
      <tr><th></th>{#each chart.lines[0].points as p (p.b.beam)}<th>beam {p.b.beam}</th>{/each}</tr>
    </thead>
    <tbody>
      {#each chart.lines as line (line.label)}
        <tr>
          <th class="tone-{line.tone}">{line.label}</th>
          {#each line.points as p (p.b.beam)}
            <td>{p.b.recall.toFixed(2)} <span>@{Math.round(p.b.cost).toLocaleString()}</span></td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
  </div>
{/if}

<style>
  figure {
    margin: 0 0 var(--space-md);
  }

  svg {
    display: block;
    width: 100%;
    height: auto;
    overflow: visible;
  }

  polyline,
  line {
    fill: none;
    stroke-width: 1.5;
  }

  .axis {
    stroke: var(--color-ink-faint);
    stroke-width: 1;
  }

  .scan {
    stroke: var(--color-ink-soft);
    stroke-dasharray: 3 3;
    stroke-width: 1;
  }

  polyline.tone-cobalt {
    stroke: var(--color-cobalt);
  }

  polyline.tone-amber {
    stroke: var(--color-amber);
  }

  polyline.tone-verdigris {
    stroke: var(--color-verdigris);
  }

  circle.tone-cobalt {
    fill: var(--color-cobalt);
  }

  circle.tone-amber {
    fill: var(--color-amber);
  }

  circle.tone-verdigris {
    fill: var(--color-verdigris);
  }

  figcaption {
    margin-top: var(--space-xs);
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }

  .scroll {
    overflow-x: auto;
  }

  table {
    width: 100%;
    margin: 0 0 var(--space-md);
    border-collapse: collapse;
    font-size: var(--font-size-xs);
    font-variant-numeric: tabular-nums;
  }

  th,
  td {
    padding: var(--space-3xs) var(--space-2xs) var(--space-3xs) 0;
    text-align: right;
    font-weight: var(--font-weight-regular);
    white-space: nowrap;
  }

  th:first-child {
    text-align: left;
  }

  td span {
    color: var(--color-ink-soft);
  }

  th.tone-cobalt {
    color: var(--color-cobalt);
  }

  th.tone-amber {
    color: var(--color-amber);
  }

  th.tone-verdigris {
    color: var(--color-verdigris);
  }
</style>
