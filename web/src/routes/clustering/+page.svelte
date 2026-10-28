<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import Legend from '#lib/components/molecules/Legend.svelte'
  import SearchChart from '#lib/components/molecules/SearchChart.svelte'
  import StreamCharts from '#lib/components/molecules/StreamCharts.svelte'
  import StreamCanvas from '#lib/components/organisms/StreamCanvas.svelte'
  import TreeCanvas from '#lib/components/organisms/TreeCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import {
    PARAM_GROUPS,
    SCENARIOS,
    emptyStreamStats,
    type ScenarioId,
    type StreamParam,
    type StreamStats,
  } from '#lib/demos/clustering/clustering.ts'
  import {
    SPACES,
    TREES,
    emptyTreeStats,
    readSearch,
    type SearchCurve,
    type SpaceId,
    type TreeParam,
    type TreeQuality,
    type TreeStats,
    type TreeTone,
    type WhichTree,
  } from '#lib/demos/clustering/tree.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'
  import type { TreeLab } from 'silly_demos'

  type ModeId = 'streams' | 'tree'
  const MODES: readonly { value: ModeId; label: string; detail: string }[] = [
    {
      value: 'streams',
      label: 'Flat streams',
      detail: 'Points in the plane, clustered as they arrive by balls, ellipses and a sketch, without being told how many.',
    },
    {
      value: 'tree',
      label: 'Hierarchy',
      detail:
        'Embeddings streamed into a tree of clusters, every node clustering its own points, so no point is ever compared with all the others.',
    },
  ]
  let mode = $state<ModeId>('streams')

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

  // Hierarchy: the embeddings, the tree's settings, and what it found.
  let space = $state<SpaceId>('sphere64')
  let treeRate = $state(100)
  let treeParams = $state<Record<string, number>>({})
  let treeParamList = $state.raw<TreeParam[]>([])
  let depth = $state(1)
  let colorBy = $state<'tree' | 'truth'>('tree')
  let drawn = $state<WhichTree>('careful')
  let buildRequest = $state(0)
  let treeStats = $state.raw<TreeStats>(emptyTreeStats())
  let treeLab = $state.raw<TreeLab | null>(null)
  let curves = $state.raw<{ label: string; tone: TreeTone; curve: SearchCurve }[] | null>(null)
  let streaming = $derived(treeStats.points < treeStats.total)

  /** Search every tree for the queries' neighbors; a few hundred ms on 20 000 points. */
  function measureSearch() {
    if (!treeLab) return
    curves = TREES.flatMap(({ id, label, tone }) => {
      const curve = readSearch(treeLab!, id)
      return curve ? [{ label, tone, curve }] : []
    })
  }

  // A new run drops the old search; a finished one measures it once.
  $effect(() => {
    if (streaming) curves = null
    else if (treeStats.quality.passes && curves === null) measureSearch()
  })

  const pct = (v: number | undefined) => (v === undefined ? '–' : v.toFixed(2))

  /** What a tree did to mend itself: splits that waited and points moved, or repairs. */
  const mended = (id: WhichTree, q: TreeQuality) =>
    id === 'careful' ? `${q.deferred} waited, ${q.moved} moved` : id === 'repaired' ? `${q.repairs} repairs` : '–'

  const tally = (counts: number[]) =>
    `${counts[0]} spawns, ${counts[1]} merges, ${counts[2]} prunes, ${counts[3]} splits`
  const checks = (m: { search: number; upkeep: number; total: number }) =>
    `${m.search.toFixed(1)} + ${m.upkeep.toFixed(1)} of ${m.total}`
</script>

<SheetLayout
  title="Clustering lab"
  summary={mode === 'streams'
    ? "Points arrive one at a time and are seen once. Three methods find the clusters as they come, including how many there are, without being told: balls that spawn, merge, split and fade, ellipses that do the same along each cluster's own axes, and a one-pass sketch refined by ball k-means. All are scored against the clusters that really made the points."
    : 'Embeddings arrive one at a time and grow a tree of clusters: a point descends to the nearest child at every node, and a leaf with enough points clusters them into children of its own. Nothing ever needs the distances between all the points, only between a point and the children on its way down. The same points are also built into a tree in passes, one level at a time, as a batch job would.'}
  readouts={mode === 'tree'
    ? [
        { label: 'Status', value: !streaming ? 'Done' : running ? 'Streaming' : 'Paused', tone: !streaming ? 'verdigris' : running ? 'cobalt' : 'ink' },
        { label: 'Points', value: `${treeStats.points.toLocaleString()} of ${treeStats.total.toLocaleString()}` },
        { label: 'Topics, online', value: pct(treeStats.quality.online?.levels[0]), tone: 'cobalt' },
        { label: 'Subtopics, online', value: pct(treeStats.quality.online?.levels[1]), tone: 'cobalt' },
        { label: 'Topics, careful', value: pct(treeStats.quality.careful?.levels[0]), tone: 'verdigris' },
        { label: 'Subtopics, careful', value: pct(treeStats.quality.careful?.levels[1]), tone: 'verdigris' },
        { label: 'Topics, repaired', value: pct(treeStats.quality.repaired?.levels[0]), tone: 'signal' },
        { label: 'Subtopics, repaired', value: pct(treeStats.quality.repaired?.levels[1]), tone: 'signal' },
        { label: 'Dimensions', value: treeStats.dim },
      ]
    : [
    { label: 'Status', value: running ? 'Streaming' : 'Paused', tone: running ? 'cobalt' : 'ink' },
    { label: 'Points', value: stats.t.toLocaleString() },
    { label: 'True clusters', value: stats.truth },
    { label: 'Balls find', value: stats.models.balls.clusters, tone: 'cobalt' },
    { label: 'Ellipses find', value: stats.models.ellipses.clusters, tone: 'verdigris' },
    { label: 'Sketch finds', value: stats.models.sketch.clusters, tone: 'amber' },
    { label: 'Ball checks per point', value: checks(stats.models.balls) },
    { label: 'Ellipse checks per point', value: checks(stats.models.ellipses) },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        {#if mode === 'tree'}
          <TreeCanvas
            {wasm}
            {space}
            {seed}
            {running}
            rate={treeRate}
            params={treeParams}
            {depth}
            {colorBy}
            {drawn}
            {restart}
            {buildRequest}
            onstats={s => (treeStats = s)}
            onparams={p => (treeParamList = p)}
            onlab={l => (treeLab = l)}
          />
        {:else}
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
        {/if}
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (running = !running)}>{running ? 'Pause' : 'Resume'}</Button>
      {#if mode === 'streams'}
        <Button disabled={running} onclick={() => stepRequest++}>Step</Button>
      {/if}
      <Button onclick={() => restart++}>Start over</Button>
      <Button variant="quiet" onclick={() => seed++}>New seed</Button>
    </ButtonRow>

    <ControlGroup title="Mode">
      <Choice
        name="mode"
        bind:value={mode}
        options={MODES.map(m => ({ ...m, detail: m.value === mode ? m.detail : undefined }))}
      />
    </ControlGroup>

    {#if mode === 'tree'}
      <ControlGroup title="Embeddings">
        <Choice
          name="space"
          bind:value={space}
          options={SPACES.map(s => ({ ...s, detail: s.value === space ? s.detail : undefined }))}
        />
        <Slider label="Points per frame" bind:value={treeRate} min={10} max={1000} step={10} />
      </ControlGroup>

      <ControlGroup title="Tree">
        {#each treeParamList as p (p.id)}
          <Slider
            label={p.label}
            bind:value={() => treeParams[p.id] ?? p.default, v => (treeParams = { ...treeParams, [p.id]: v })}
            min={p.min}
            max={p.max}
            step={p.step}
          />
        {/each}
        <p class="hint">Each setting starts the stream over.</p>
        <ButtonRow>
          <Button onclick={() => (treeParams = {})}>Defaults</Button>
          <Button onclick={() => buildRequest++}>Build in passes now</Button>
        </ButtonRow>
      </ControlGroup>

      <ControlGroup title="Show">
        <Choice
          name="drawn"
          bind:value={drawn}
          options={TREES.filter(t => t.id !== 'passes').map(t => ({ value: t.id, label: t.label }))}
        />
        <Slider label="Depth drawn" bind:value={depth} min={1} max={4} />
        <Choice
          name="color-by"
          bind:value={colorBy}
          options={[
            { value: 'tree', label: 'Color by node at that depth' },
            { value: 'truth', label: 'Color by true topic' },
          ]}
        />
      </ControlGroup>
    {:else}
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
      <Toggle label="How far each ball and ellipse reaches" bind:checked={showReach} />
      <Toggle label="How the last point was placed, while paused" bind:checked={showTrace} />
    </ControlGroup>
    {/if}
  {/snippet}

  {#snippet notes()}
    {#if mode === 'tree'}
      {#if curves}
        <SearchChart {curves} />
      {:else}
        <p class="hint">Search is measured once every point is in.</p>
      {/if}
      <ButtonRow>
        <Button onclick={measureSearch}>Measure search now</Button>
      </ButtonRow>
      <div class="scroll">
      <table class="values">
        <thead>
          <tr><th></th><th>Topics</th><th>Subtopics</th><th>Leaves</th><th>Leaf set</th><th>Nodes</th><th>Per point</th><th>Mended</th></tr>
        </thead>
        <tbody>
          {#each TREES as tree (tree.id)}
            {@const q = treeStats.quality[tree.id]}
            <tr>
              <th class="tone-{tree.tone}">{tree.label}</th>
              {#if q}
                <td>{q.levels[0].toFixed(2)}</td>
                <td>{q.levels[1].toFixed(2)}</td>
                <td>{q.levels[2].toFixed(2)}</td>
                <td>{q.leaves.toFixed(2)}</td>
                <td>{q.nodes.slice(0, 3).join('/')}</td>
                <td>{(q.place + q.split + q.recheck + q.repair).toFixed(0)}{tree.id === 'passes' ? `, ${q.passes} passes` : ''}</td>
                <td>{mended(tree.id, q)}</td>
              {:else}
                <td colspan="7">once every point is in</td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
      </div>
      <p class="credit">
        Adjusted Rand index of the tree's nodes at depths 1, 2 and 3 against the true topics, subtopics and leaves, and of
        its leaves against the true leaves (6 topics, 24 subtopics, 72 leaves). Nodes per level, and distances computed per
        point to place it, split nodes, re-check points and decide repairs; for the careful tree, the splits that waited for
        evidence and the points re-checking moved, and for the repaired tree, its repairs.
      </p>
      <p>
        <strong>Growing online.</strong> A point descends from the root to the nearest child at every node and settles in a
        leaf, moving each center on its way a little towards itself. A leaf that has gathered enough points clusters them:
        k-means for every number of children up to the cap, keeping the one the Bayesian information criterion likes best,
        squared error against the parameters more children add. So each node finds its own number of children, and placing
        a point costs only the children on its way down, a few dozen distances however many points there are.
      </p>
      <p>
        <strong>Growing carefully.</strong> Left to the criterion, a leaf splits into the finest structure it can see, and the
        subtopics inside a topic are real clusters too: the root takes seven or eight children for six topics and cuts some
        in two. The careful tree splits at the coarsest strong level instead, the number of children after which one more
        would add little next to the average gain so far. It splits only once two random halves of the leaf's points agree on
        that number, and otherwise waits for more points, after ODAC's rule of deciding once the evidence separates the
        options; and it re-checks a few points per point placed, moving those that would now descend elsewhere, after
        SPFresh's LIRE. Waiting is what makes the difference, at several times the splitting cost.
      </p>
      <p>
        <strong>Repairing.</strong> Or a tree splits as fast as the plain one and mends what it got wrong. Every node keeps
        the count, the mean and the summed squared norms of the points through it, so the squared error of any grouping of
        siblings follows exactly from theirs, without looking at a point. Each time a node's points have doubled, it
        re-decides its children from that alone, by the same coarsest-level rule: siblings that turn out to be halves of one
        cluster are merged, a split that wasn't needed is flattened, a missing level is put in, and a child that took two
        clusters for one has its children lifted up among its siblings. This costs less than a distance per point. Lower the
        points before a leaf splits to see the plain and careful trees go wrong early, and the repaired one recover.
      </p>
      <p>
        <strong>Built in passes.</strong> The batch way: cluster all points at the root, then hand each to its nearest child
        and cluster each child's points, a level per pass. It sees every point of a node at once, where the online tree
        splits a leaf from the first few hundred that reach it and lets later points follow its centers.
      </p>
      <p>
        <strong>Search.</strong> A query descends the same way, keeping the few nearest nodes at every level (the beam), and
        compares itself with the points in the leaves it reaches. A beam of one follows a single path; wider beams catch the
        neighbors that sit just across a boundary, at the cost of more distances. A scan compares with every point.
      </p>
      <p>
        Two tools from the flat streams fall short in 64 dimensions. A one-pass facility-location sketch of a leaf's points
        collapses: two points of one cluster are about as far apart as points of neighboring clusters, so the heaviest
        facility, at the leaf's mean, wins every new point. And ball k-means' trimming, which moves a center by the points
        within half the way to the next, finds hardly any points that close. Plain k-means on the leaf's points does.
      </p>
    {:else}
    <StreamCharts history={stats.history} />
    <Legend
      items={[
        { tone: 'verdigris', label: 'Spawn' },
        { tone: 'amber', label: 'Merge' },
        { tone: 'cobalt', label: 'Split' },
        { tone: 'signal', label: 'Prune' },
      ]}
    />
    <p class="credit">
      Balls: {tally(stats.models.balls.events)}. Ellipses: {tally(stats.models.ellipses.events)}. Sketch:
      {tally(stats.models.sketch.events)}.
    </p>
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
      <strong>Ellipses.</strong> The same rules with a covariance per cluster: a point’s distance to a cluster is its
      Mahalanobis distance, so the reach is a number of standard deviations along the cluster’s own axes and a long thin
      cluster is one long thin ellipse instead of a chain of balls. Among the ellipses that reach a point it joins the one
      it fits best by Gaussian likelihood, so a big loose ellipse doesn’t outbid a tight one. Two merge when the merged
      ellipse is no bigger than the two together, and one splits when its halves are four of their own standard
      deviations apart; one Gaussian’s halves sit about 2.7 apart however stretched, so a long cluster stays whole. The
      search bounds each ellipse by its longest semi-axis and so stays exact, at the price of checking more neighbors
      around thin ones.
    </p>
    <p>
      <strong>Sketch + ball k-means.</strong> Online facility location keeps a few dozen weighted centers in one pass: a point
      opens a new facility with probability d²/f, its squared distance to the nearest over the cost of opening one, and
      otherwise joins the nearest. When there are too many, f grows and the facilities are run through the same rule again.
      Every 200 points, a ball k-means with a price λ on every cluster refines the facilities into clusters, so their
      number is whatever balances error against that price. Pause and step to see the probability and the draw.
    </p>
    <p>
      Balls and the sketch struggle where clusters aren’t ball-shaped: long thin ones come apart. Ellipses hold those, but
      rings and moons, which aren’t ellipses either, are still cut into arcs by all three.
      Batch k-means, told the true number and given the recent points all at once, is the line to beat; on the rings it
      fails too. The adjusted Rand index is 1 when a method groups the newest 600 points exactly as their true clusters do,
      and about 0 when it does no better than chance.
    </p>
    {/if}
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
