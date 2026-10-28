<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import SearchChart from '#lib/components/molecules/SearchChart.svelte'
  import VectorCanvas from '#lib/components/organisms/VectorCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import { SPACES, type SpaceId } from '#lib/demos/clustering/tree.ts'
  import type { ChartCurve, SearchCurve } from '#lib/demos/search.ts'
  import {
    INDEXES,
    emptyVectorStats,
    indexInfo,
    readCurve,
    type IndexId,
    type VectorParam,
    type VectorStats,
  } from '#lib/demos/vector/vector.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'
  import type { VectorLab } from 'silly_demos'

  let space = $state<SpaceId>('sphere64')
  let seed = $state(1)
  let running = $state(true)
  let rate = $state(100)
  let restart = $state(0)
  let params = $state<Record<string, number>>({})
  let paramList = $state.raw<VectorParam[]>([])
  let stats = $state.raw<VectorStats>(emptyVectorStats())
  let lab = $state.raw<VectorLab | null>(null)

  let index = $state<IndexId>('hnsw')
  /** The effort drawn, as a position in the index's efforts. */
  let effortAt = $state(2)
  let query = $state(0)
  let efforts = $derived(lab ? Array.from(lab.efforts(index)) : [])
  let effort = $derived(efforts[Math.min(effortAt, efforts.length - 1)] ?? 1)
  let queries = $derived(lab ? lab.queries() : 100)

  let curves = $state.raw<(ChartCurve & { id: IndexId })[] | null>(null)

  /** Search every index there is for every query at every effort; a second or so on 20 000 points. */
  function measureSearch() {
    if (!lab) return
    curves = INDEXES.flatMap(({ id, label, tone, effort }) => {
      const curve = readCurve(lab!, id)
      return curve ? [{ id, label, tone, effort, curve }] : []
    })
  }

  // A new run drops the old measurement; a finished one measures once.
  $effect(() => {
    if (stats.phase !== 'done') curves = null
    else if (curves === null) measureSearch()
  })

  let groups = $derived(
    (['stream', ...INDEXES.map(i => i.id)] as const).map(id => ({
      id,
      title: id === 'stream' ? 'Stream' : indexInfo(id).label,
      params: paramList.filter(p => p.index === id),
    })),
  )

  /** The fewest distances per query an index needs for recall `at`, or the best it reaches. */
  function costFor(curve: SearchCurve | undefined, at: number): string {
    if (!curve) return '–'
    const p = curve.points.find(p => p.recall >= at)
    return p ? Math.round(p.cost).toLocaleString() : `tops at ${Math.max(...curve.points.map(p => p.recall)).toFixed(2)}`
  }

  const curveOf = (id: IndexId) => curves?.find(c => c.id === id)?.curve

  let status = $derived(
    stats.phase === 'streaming'
      ? running
        ? 'Streaming'
        : 'Paused'
      : stats.phase === 'building'
        ? `Building, ${Math.round(stats.built * 100)} %`
        : 'Done',
  )

  const fmt = (v: number | undefined, digits = 0) =>
    v === undefined ? '–' : v.toLocaleString(undefined, { maximumFractionDigits: digits })
</script>

<SheetLayout
  title="Vector search lab"
  summary="Embeddings arrive one at a time and five indexes are built over them to find a query's ten nearest neighbors without comparing it with every point: HNSW's layered graph, an inverted file that splits its postings as they fill, after SPFresh, and the repaired tree of clusters, all grown online; then, once every point is in, IVF-PQ's compressed codes and DiskANN's Vamana graph, built in a batch. Each is measured by the recall it reaches against the distances it computes."
  readouts={[
    {
      label: 'Status',
      value: status,
      tone: stats.phase === 'done' ? 'verdigris' : running ? 'cobalt' : 'ink',
    },
    { label: 'Points', value: `${stats.points.toLocaleString()} of ${stats.total.toLocaleString()}` },
    ...INDEXES.map(i => ({ label: `${i.label}, 0.95 at`, value: costFor(curveOf(i.id), 0.95), tone: i.tone })),
    { label: 'Dimensions', value: stats.dim },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <VectorCanvas
          {wasm}
          {space}
          {seed}
          {running}
          {rate}
          {params}
          {index}
          {effort}
          {query}
          {restart}
          onstats={s => (stats = s)}
          onparams={p => (paramList = p)}
          onlab={l => (lab = l)}
        />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (running = !running)}>{running ? 'Pause' : 'Resume'}</Button>
      <Button onclick={() => restart++}>Start over</Button>
      <Button variant="quiet" onclick={() => seed++}>New seed</Button>
    </ButtonRow>

    <ControlGroup title="Embeddings">
      <Choice
        name="space"
        bind:value={space}
        options={SPACES.map(s => ({ ...s, detail: s.value === space ? s.detail : undefined }))}
      />
      <Slider label="Points per frame" bind:value={rate} min={10} max={1000} step={10} />
    </ControlGroup>

    <ControlGroup title="Search drawn">
      <Choice
        name="index"
        bind:value={index}
        options={INDEXES.map(i => ({ value: i.id, label: i.label }))}
      />
      <Slider
        label="Effort"
        bind:value={effortAt}
        min={0}
        max={Math.max(efforts.length - 1, 0)}
        format={() => `${indexInfo(index).effort} ${effort}`}
      />
      <Slider label="Query" bind:value={query} min={0} max={queries - 1} format={v => `${v + 1} of ${queries}`} />
    </ControlGroup>

    <ControlGroup title="Settings">
      {#each groups as group (group.id)}
        {#if group.params.length > 0}
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
        {/if}
      {/each}
      <p class="hint">Each setting starts the stream over.</p>
      <ButtonRow>
        <Button onclick={() => (params = {})}>Defaults</Button>
      </ButtonRow>
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    {#if curves}
      <SearchChart {curves} note="a point per search effort, ef or L for the graphs, postings or lists probed, beam for the tree." />
    {:else}
      <p class="hint">Search is measured once every index is built.</p>
    {/if}
    <ButtonRow>
      <Button onclick={measureSearch}>Measure search now</Button>
    </ButtonRow>
    <div class="scroll">
      <table class="values">
        <thead>
          <tr><th></th><th>Build</th><th>Bytes</th><th>Of its own</th></tr>
        </thead>
        <tbody>
          {#each INDEXES as i (i.id)}
            {@const c = stats.cost[i.id]}
            <tr>
              <th class="tone-{i.tone}">{i.label}</th>
              {#if c}
                <td>{fmt(c.build)}</td>
                <td>{fmt(c.bytes)}</td>
                <td>
                  {fmt(c.counts[0], 1)}
                  {i.counts[0]}, {i.id === 'vamana' ? `${Math.round(c.counts[1] * 100)} %` : fmt(c.counts[1], 1)}
                  {i.counts[1]}
                </td>
              {:else}
                <td colspan="3">once every point is in</td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="credit">
      HNSW, the inverted file split online and the tree grow as the points arrive; IVF-PQ and Vamana are built once
      they are all in. Distances computed per point to build each index, and the bytes it holds in memory per point: the vector itself,
      256 bytes in 64 dimensions, and its links or ids, with the centroids spread over the points. IVF-PQ holds codes
      instead of vectors and reads the full vectors only of the candidates it compares exactly. A distance between parts
      of vectors counts as the share of the vector they cover.
    </p>
    <p>
      <strong>HNSW.</strong> Every point is a node linked to a few near ones, and a random few are also nodes of a
      sparser graph above, fewer still of one above that. A search walks greedily down from the top, then on the bottom
      layer keeps the <em>ef</em> nearest nodes it has met and expands them until none could still be nearer. A point is
      inserted by searching for its own neighbors and keeps those that spread out in different directions, so walks can
      cross from cluster to cluster. It grows online, but each insertion is a search with a wide <em>ef</em>: it is the
      most expensive index to build here.
    </p>
    <p>
      <strong>An inverted file, split online.</strong> The points sit in postings around centroids; a query scans the
      postings of its nearest few centroids. The postings grow with the stream, and one that grows too long is split in two
      by 2-means. The split moves centroids, so some points of the split posting and of its neighbors are no longer in the
      nearest posting. SPFresh's LIRE finds the few that may be from two cheap conditions on their distances to the old and
      new centroids, and checks and moves only those. That keeps about half as many points misplaced as splitting alone,
      for a fraction of the cost of placing the points in the first place.
    </p>
    <p>
      <strong>The repaired tree.</strong> The hierarchy of clusters from the Clustering lab: postings within postings,
      every node's children found by k-means and the Bayesian information criterion, and repaired from their statistics
      when a split came too early. A search descends with a beam. Its leaves hold whole leaf clusters, so a beam of one
      already finds most neighbors, and a wider beam pays for whole leaves at a time.
    </p>
    <p>
      <strong>IVF-PQ.</strong> The batch inverted file with product quantization, as FAISS builds it: k-means on a sample
      makes the lists, and what is left of each point after its centroid is cut into 16 parts of four dimensions, each
      replaced by the nearest of 16 learned centers: 8 bytes for a vector of 256. A query's distance to every code is a
      sum of 16 lookups in a small table. That estimate is coarse, so the best hundred are compared exactly, and recall
      stops short of 1 wherever a true neighbor's code ranks it below the hundredth.
    </p>
    <p>
      <strong>Vamana.</strong> DiskANN's graph: one layer, built in two passes over all the points. Each point searches
      the graph for itself and keeps, of the nodes it visited, the nearest, then any not already nearer to one kept, and
      so on. DiskANN relaxes that by a factor α of 1.2 so long links survive. On these embeddings the topics lie far apart
      and each topic's points close together, so the relaxed prune keeps more near links, they fill every node's links,
      and the few links between topics are crowded out: queries far from the starting point are never reached. Raise α to
      see it. With α of 1, it is the cheapest graph to search here, at the price of passes over every point.
    </p>
    <p>
      The graphs reach the highest recall for the fewest distances, but cost the most to build, and HNSW keeps the
      vectors and its links in memory. The inverted files and the tree are cheap to grow and scan whole postings, which a
      disk reads well. Only IVF-PQ holds less than the vectors. The points come from the same tree of topics, subtopics
      and leaf clusters as the Clustering lab's, which is kinder to inverted files and trees than real embeddings are.
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

  .scroll {
    overflow-x: auto;
  }

  .values {
    width: 100%;
    margin: 0 0 var(--space-xs);
    border-collapse: collapse;
    font-size: var(--font-size-xs);
    font-variant-numeric: tabular-nums;
  }

  .values th,
  .values td {
    padding: var(--space-3xs) var(--space-2xs) var(--space-3xs) 0;
    text-align: right;
    font-weight: var(--font-weight-regular);
    white-space: nowrap;
  }

  .values tbody th {
    text-align: left;
  }

  .values .tone-ink {
    color: var(--color-ink);
  }

  .values .tone-cobalt {
    color: var(--color-cobalt);
  }

  .values .tone-verdigris {
    color: var(--color-verdigris);
  }

  .values .tone-amber {
    color: var(--color-amber);
  }

  .values .tone-signal {
    color: var(--color-signal);
  }

  .credit {
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
  }
</style>
