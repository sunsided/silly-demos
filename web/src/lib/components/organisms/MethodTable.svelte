<!--
  @component
  Every collision method for the current pair with its verdict, distance and
  time; rows are clickable, and verdicts that disagree with the reference are flagged.
-->
<script lang="ts">
  import {
    FULL_TEST,
    STAGES,
    formatDistance,
    formatTime,
    type Row,
  } from '../../demos/collision/results.ts'

  let {
    rows,
    selected,
    reference,
    timings,
    name,
    onselect,
  }: {
    rows: readonly Row[]
    selected: string | undefined
    reference: Row | undefined
    timings: Record<string, number>
    name: (id: string) => string
    onselect: (id: string) => void
  } = $props()
</script>

<table>
  <caption>All methods for this pair</caption>
  <thead>
    <tr>
      <th scope="col">Method</th>
      <th scope="col">Hit</th>
      <th scope="col">Distance</th>
      <th scope="col">Time</th>
    </tr>
  </thead>
  <tbody>
    {#each rows as row (row.id)}
      {@const disagrees = reference && row.stage === FULL_TEST && row.intersect !== reference.intersect}
      <tr class={{ selected: row.id === selected }} onclick={() => onselect(row.id)}>
        <th scope="row">
          <button type="button" aria-pressed={row.id === selected}>{name(row.id)}</button>
        </th>
        <td class={{ hit: row.intersect }}>
          {row.intersect ? 'yes' : 'no'}
          {#if disagrees}<span title="Disagrees with GJK + EPA"> ≠</span>{/if}
        </td>
        <td>{row.stage === FULL_TEST ? formatDistance(row.distance) : STAGES[row.stage].toLowerCase()}</td>
        <td>{formatTime(timings[row.id])}</td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  table {
    width: 100%;
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
    text-align: left;
    vertical-align: top;
    font-weight: var(--font-weight-regular);
  }

  thead th {
    border-bottom: var(--border-hairline) solid var(--color-ink-faint);
  }

  tbody tr {
    cursor: pointer;
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

  .hit {
    color: var(--color-signal);
  }
</style>
