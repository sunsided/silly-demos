<!--
  @component
  How a search trades distances for recall: recall@10 against distances
  per query, one line per index with a point per search effort, on a log
  scale reaching to what a scan of every point takes. Below it, the same
  numbers as a table.
-->
<script lang="ts">
  import type { ChartCurve } from '../../demos/search.ts'

  let {
    curves,
    note = '',
  }: {
    /** One curve per index, with the tone it is drawn in. */
    curves: readonly ChartCurve[]
    /** Appended to the caption, as what the points are. */
    note?: string
  } = $props()

  const WIDTH = 300
  const HEIGHT = 130
  const PAD = 4
  /** Room below the plot for the axis labels. */
  const AXIS = 10

  let chart = $derived.by(() => {
    if (curves.length === 0) return null
    const scan = Math.max(...curves.map(c => c.curve.scan))
    // From the power of ten below the cheapest point, at least 10, to the scan.
    const cheapest = Math.min(...curves.flatMap(c => c.curve.points.map(p => p.cost)))
    const from = 10 ** Math.max(1, Math.floor(Math.log10(Math.max(cheapest, 10))))
    const to = Math.max(scan, from * 10)
    const lo = Math.log10(from)
    const hi = Math.log10(to)
    const bottom = HEIGHT - PAD - AXIS
    const x = (cost: number) => PAD + ((Math.log10(Math.max(cost, from)) - lo) / (hi - lo)) * (WIDTH - 2 * PAD)
    // From the tenth below the worst recall, at most 0.5, to 1.
    const worst = Math.min(...curves.flatMap(c => c.curve.points.map(p => p.recall)))
    const floor = Math.min(0.5, Math.floor(worst * 10) / 10)
    const y = (recall: number) => bottom - ((recall - floor) / (1 - floor)) * (bottom - PAD)
    const ticks = [10, 100, 1000, 10_000, 100_000].filter(t => t >= from && t <= to)
    // One header row when every curve was measured at the same efforts.
    const first = curves[0]
    const shared = curves.every(
      c =>
        c.effort === first.effort &&
        c.curve.points.length === first.curve.points.length &&
        c.curve.points.every((p, i) => p.effort === first.curve.points[i].effort),
    )
    return {
      bottom,
      from,
      scanX: x(scan),
      scan,
      ticks: ticks.map(t => ({ t, x: x(t) })),
      floor,
      grid: [...new Set([floor, 0.5, 0.9, 1])].filter(r => r >= floor).map(r => ({ r, y: y(r) })),
      shared,
      lines: curves.map(c => ({
        ...c,
        points: c.curve.points.map(p => ({ x: x(p.cost), y: y(p.recall), p })),
      })),
    }
  })

  const short = (t: number) => (t >= 1000 ? `${t / 1000}k` : `${t}`)
</script>

{#if chart}
  <figure>
    <svg viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="Recall against distances per query">
      {#each chart.grid as g (g.r)}
        <line class="grid" x1={PAD} y1={g.y} x2={WIDTH - PAD} y2={g.y} />
        <text class="tick" x={PAD} y={g.y - 1.5}>{g.r}</text>
      {/each}
      <line class="axis" x1={PAD} y1={chart.bottom} x2={WIDTH - PAD} y2={chart.bottom} />
      {#each chart.ticks as tick (tick.t)}
        <line class="axis" x1={tick.x} y1={chart.bottom} x2={tick.x} y2={chart.bottom + 2} />
        <text class="tick" x={tick.x} y={HEIGHT - PAD} text-anchor="middle">{short(tick.t)}</text>
      {/each}
      <line class="scan" x1={chart.scanX} y1={PAD} x2={chart.scanX} y2={chart.bottom} />
      {#each chart.lines as line (line.label)}
        <polyline class="tone-{line.tone}" points={line.points.map(p => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')} />
        {#each line.points as p (p.p.effort)}
          <circle class="tone-{line.tone}" cx={p.x} cy={p.y} r="2.5" />
        {/each}
      {/each}
    </svg>
    <figcaption>
      Recall@10, {chart.floor} to 1, against distances per query, {chart.from.toLocaleString()} to {chart.scan.toLocaleString()} (a
      scan, dashed) on a log scale{note ? `; ${note}` : '.'}
    </figcaption>
  </figure>
  <div class="scroll">
    <table>
      {#if chart.shared}
        <thead>
          <tr>
            <th></th>
            {#each chart.lines[0].points as p (p.p.effort)}<th>{chart.lines[0].effort} {p.p.effort}</th>{/each}
          </tr>
        </thead>
      {/if}
      <tbody>
        {#each chart.lines as line (line.label)}
          <tr>
            <th class="tone-{line.tone}">{line.label}</th>
            {#each line.points as p (p.p.effort)}
              <td>
                {#if !chart.shared}<span>{line.effort} {p.p.effort}</span><br />{/if}
                {p.p.recall.toFixed(2)} <span>@{Math.round(p.p.cost).toLocaleString()}</span>
              </td>
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

  .grid {
    stroke: var(--color-ink-faint);
    stroke-width: 0.5;
    stroke-dasharray: 1 2;
  }

  .tick {
    fill: var(--color-ink-soft);
    font-size: 7px;
    font-variant-numeric: tabular-nums;
  }

  .scan {
    stroke: var(--color-ink-soft);
    stroke-dasharray: 3 3;
    stroke-width: 1;
  }

  polyline.tone-ink {
    stroke: var(--color-ink);
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

  polyline.tone-signal {
    stroke: var(--color-signal);
  }

  circle.tone-ink {
    fill: var(--color-ink);
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

  circle.tone-signal {
    fill: var(--color-signal);
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

  th.tone-ink {
    color: var(--color-ink);
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

  th.tone-signal {
    color: var(--color-signal);
  }
</style>
