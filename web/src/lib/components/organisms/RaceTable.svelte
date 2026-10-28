<!--
  @component
  Every method on the same cities, run side by side to the end: tour
  length, gap, time and iterations. The gap is to the known optimum, or
  else to the shortest tour in the race. Rows are clickable.
-->
<script lang="ts">
  import { untrack } from 'svelte'
  import { onFrame } from '../../canvas/canvas.svelte.ts'
  import {
    MAX_EXACT,
    formatGap,
    formatLength,
    methodLabel,
    Race,
    type CitySet,
    type MethodId,
    type RaceRow,
  } from '../../demos/kohonen/tours.ts'
  import type { Wasm } from '../../wasm/wasm.ts'

  let {
    wasm,
    cities,
    seed,
    request,
    selected,
    onselect,
  }: {
    wasm: Wasm
    cities: CitySet
    seed: number
    /** Each increment starts a race; 0 means none yet. */
    request: number
    selected: MethodId | null
    /** A row was picked: its method, with the ring and tour it ended with. */
    onselect: (method: MethodId, shape: { ring: Float32Array; tour: Uint32Array } | null) => void
  } = $props()

  /** Stepping time per frame, ms. */
  const FRAME_BUDGET = 12

  let race: Race | null = null
  let rows = $state.raw<RaceRow[]>([])
  let racing = $state(false)

  $effect(() => () => {
    race?.free()
    race = null
  })

  // A new set of cities ends the race and clears the table.
  $effect(() => {
    void cities
    void seed
    race?.free()
    race = null
    rows = []
    racing = false
  })

  $effect(() => {
    if (!request) return
    untrack(() => {
      race?.free()
      race = new Race(wasm, cities, seed)
      rows = race.rows
      racing = true
    })
  })

  onFrame(
    () => {
      if (!race) return
      race.run(FRAME_BUDGET)
      rows = race.rows
      if (race.done) racing = false
    },
    () => racing,
  )

  let best = $derived(Math.min(...rows.filter(r => r.status === 'done').map(r => r.length)))
  let reference = $derived(cities.optimum ?? (Number.isFinite(best) ? best : null))

  function pick(method: MethodId) {
    onselect(method, race?.shape(method) ?? null)
  }

  const formatTime = (ms: number) => (ms < 10 ? `${ms.toFixed(1)} ms` : `${Math.round(ms).toLocaleString()} ms`)
</script>

{#if rows.length > 0}
  <table>
    <caption>{racing ? 'Racing…' : 'Race'} on {cities.label}</caption>
    <thead>
      <tr>
        <th scope="col">Method</th>
        <th scope="col">Length</th>
        <th scope="col">{cities.optimum === null ? 'Gap to best' : 'Gap to optimum'}</th>
        <th scope="col">Time</th>
        <th scope="col">Iterations</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as row (row.method)}
        <tr class={{ selected: row.method === selected, running: row.status !== 'done' }} onclick={() => pick(row.method)}>
          <th scope="row">
            <button type="button" aria-pressed={row.method === selected}>{methodLabel(row.method)}</button>
          </th>
          <td>{formatLength(row.length, cities.metric)}</td>
          <td>{row.status === 'done' ? formatGap(row.length, reference) : '…'}</td>
          <td>{formatTime(row.time)}</td>
          <td>{row.iterations.toLocaleString()}</td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if cities.xy.length / 2 > MAX_EXACT}
    <p class="note">Exact sits out above {MAX_EXACT} cities.</p>
  {/if}
{/if}

<style>
  table {
    width: 100%;
    margin-bottom: var(--space-xs);
    border-collapse: collapse;
    font-size: var(--font-size-xs);
    font-variant-numeric: tabular-nums;
  }

  caption {
    margin-bottom: var(--space-xs);
    text-align: left;
    color: var(--color-ink);
    font-weight: var(--font-weight-bold);
  }

  th,
  td {
    padding: var(--space-2xs) var(--space-xs) var(--space-2xs) 0;
    text-align: right;
    vertical-align: top;
    white-space: nowrap;
    font-weight: var(--font-weight-regular);
  }

  th:first-child {
    text-align: left;
    white-space: normal;
  }

  thead th {
    border-bottom: var(--border-hairline) solid var(--color-ink-faint);
  }

  tbody tr {
    cursor: pointer;
  }

  .running td {
    color: var(--color-ink-soft);
  }

  /* The method name is the row's keyboard target; it looks like plain text. */
  button {
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-ink);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  tbody tr:hover button {
    text-decoration: underline;
  }

  .selected button {
    font-weight: var(--font-weight-bold);
  }

  .note {
    margin: 0 0 var(--space-md);
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }
</style>
