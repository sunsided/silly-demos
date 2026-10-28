<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import Legend from '#lib/components/molecules/Legend.svelte'
  import StreamCharts from '#lib/components/molecules/StreamCharts.svelte'
  import StreamCanvas from '#lib/components/organisms/StreamCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import {
    PARAM_GROUPS,
    SCENARIOS,
    emptyStreamStats,
    type ScenarioId,
    type StreamParam,
    type StreamStats,
  } from '#lib/demos/clustering/clustering.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let scenario = $state<ScenarioId>('blobs')
  let seed = $state(1)
  let running = $state(true)
  let rate = $state(10)
  let showReach = $state(true)
  let showTrace = $state(true)
  let restart = $state(0)
  let stepRequest = $state(0)
  let params = $state<Record<string, number>>({})
  let paramList = $state.raw<StreamParam[]>([])
  let stats = $state.raw<StreamStats>(emptyStreamStats())

  let groups = $derived(
    (Object.keys(PARAM_GROUPS) as StreamParam['model'][]).map(model => ({
      model,
      title: PARAM_GROUPS[model],
      params: paramList.filter(p => p.model === model),
    })),
  )

  const tally = (counts: number[]) =>
    `${counts[0]} spawns, ${counts[1]} merges, ${counts[2]} prunes, ${counts[3]} splits`
</script>

<SheetLayout
  title="Clustering lab"
  summary="Points arrive one at a time and are seen once. Two methods find the clusters as they come, including how many there are, without being told: balls that spawn, merge, split and fade, and a one-pass sketch refined by ball k-means. Both are scored against the clusters that really made the points."
  readouts={[
    { label: 'Status', value: running ? 'Streaming' : 'Paused', tone: running ? 'cobalt' : 'ink' },
    { label: 'Points', value: stats.t.toLocaleString() },
    { label: 'True clusters', value: stats.truth },
    { label: 'Balls find', value: stats.balls, tone: 'cobalt' },
    { label: 'Sketch finds', value: stats.sketch, tone: 'amber' },
    {
      label: 'Ball checks per point',
      value: `${stats.ballSearch.toFixed(1)} + ${stats.ballUpkeep.toFixed(1)} of ${stats.ballCount}`,
    },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <StreamCanvas
          {wasm}
          {scenario}
          {seed}
          {running}
          {rate}
          {params}
          {showReach}
          {showTrace}
          {restart}
          {stepRequest}
          onstats={s => (stats = s)}
          onparams={p => (paramList = p)}
        />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (running = !running)}>{running ? 'Pause' : 'Resume'}</Button>
      <Button disabled={running} onclick={() => stepRequest++}>Step</Button>
      <Button onclick={() => restart++}>Start over</Button>
      <Button variant="quiet" onclick={() => seed++}>New seed</Button>
    </ButtonRow>

    <ControlGroup title="Stream">
      <Choice
        name="scenario"
        bind:value={scenario}
        options={SCENARIOS.map(s => ({ ...s, detail: s.value === scenario ? s.detail : undefined }))}
      />
      <Slider label="Points per frame" bind:value={rate} min={1} max={200} />
      <p class="hint">Press and drag on either panel to drop points of your own.</p>
    </ControlGroup>

    <ControlGroup title="Settings">
      {#each groups as group (group.model)}
        <p class="subhead">{group.title}</p>
        {#each group.params as p (p.id)}
          <Slider
            label={p.label}
            bind:value={() => params[p.id] ?? p.default, v => (params = { ...params, [p.id]: v })}
            min={p.min}
            max={p.max}
            step={p.step}
          />
        {/each}
      {/each}
      <p class="hint">Settings apply at once, while the stream runs.</p>
      <ButtonRow>
        <Button onclick={() => (params = {})}>Defaults</Button>
      </ButtonRow>
    </ControlGroup>

    <ControlGroup title="Show">
      <Toggle label="How far each ball reaches" bind:checked={showReach} />
      <Toggle label="How the last point was placed, while paused" bind:checked={showTrace} />
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    <StreamCharts history={stats.history} />
    <Legend
      items={[
        { tone: 'verdigris', label: 'Spawn' },
        { tone: 'amber', label: 'Merge' },
        { tone: 'cobalt', label: 'Split' },
        { tone: 'signal', label: 'Prune' },
      ]}
    />
    <p class="credit">Balls: {tally(stats.events.balls)}. Sketch: {tally(stats.events.sketch)}.</p>
    <p>
      <strong>Balls.</strong> Each cluster is a ball: a center and a radius, the root mean square distance of its points.
      A point joins the nearest ball if it lies within that ball’s reach, a few radii out, and otherwise spawns a ball of
      its own. A ball merges into a neighbor when that leaves the neighbor about as tight as it was, and splits when a tiny
      two-means over its own points finds two halves well apart, which is how it notices a cluster pulling slowly in two.
      Forgetting lets old points fade; a ball that no longer gets points stops counting and is eventually pruned. A
      cluster counts once it holds a small share of the recent points, 2 % by default, which also caps how many can
      count at once: lower it for many small clusters.
    </p>
    <p>
      Finding the nearest ball is where the balls earn their name. After Xia et al.’s ball k-means (2020): if a point lies
      within ball b’s reach, any nearer center is less than twice that reach from b, so only b’s neighbors need checking.
      Each ball also links to its nearest few, so a greedy walk from the last point’s ball can cross to the right one; the
      readout counts the distances that takes against the number of balls. Points arrive in no particular order, so the walk
      starts anywhere, which is what a hierarchy would fix.
    </p>
    <p>
      <strong>Sketch + ball k-means.</strong> Online facility location keeps a few dozen weighted centers in one pass: a point
      opens a new facility with probability d²/f, its squared distance to the nearest over the cost of opening one, and
      otherwise joins the nearest. When there are too many, f grows and the facilities are run through the same rule again.
      Every 200 points, a ball k-means with a price λ on every cluster refines the facilities into clusters, so their
      number is whatever balances error against that price. Pause and step to see the probability and the draw.
    </p>
    <p>
      Both struggle where clusters aren’t ball-shaped: long thin ones come apart, and rings and moons are cut into arcs.
      Batch k-means, told the true number and given the recent points all at once, is the line to beat; on the rings it
      fails too. The adjusted Rand index is 1 when a method groups the newest 600 points exactly as their true clusters do,
      and about 0 when it does no better than chance.
    </p>
  {/snippet}
</SheetLayout>

<style>
  .subhead {
    margin: var(--space-xs) 0 0;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-bold);
    color: var(--color-ink-soft);
  }

  .hint {
    margin: 0;
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }

  .credit {
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }
</style>
