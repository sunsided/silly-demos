<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Select from '#lib/components/atoms/Select.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import StatusMessage from '#lib/components/atoms/StatusMessage.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import Legend from '#lib/components/molecules/Legend.svelte'
  import ImagePicker from '#lib/components/molecules/ImagePicker.svelte'
  import MetricChart from '#lib/components/molecules/MetricChart.svelte'
  import SomCanvas from '#lib/components/organisms/SomCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import type { Tone } from '#lib/components/types.ts'
  import {
    DECAYS,
    DENSITIES,
    INITS,
    LATTICES,
    MODES,
    SCENARIOS,
    TRAININGS,
    type DecayId,
    type DensityId,
    type InitId,
    type LatticeId,
    type ModeId,
    type Scenario,
    type SomConfig,
    type SomStats,
    type TrainingId,
  } from '#lib/demos/kohonen/kohonen.ts'
  import { BUNDLED, loadImage, toDensity } from '#lib/demos/kohonen/image.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let mode = $state<ModeId>('unfolding')
  let scenario = $state<Scenario | null>(SCENARIOS[0])

  let lattice = $state<LatticeId>('grid')
  let nodes = $state(60)
  let gridWidth = $state(12)
  let gridHeight = $state(12)
  let init = $state<InitId>('random')
  let steps = $state(20_000)
  let alphaStart = $state(0.5)
  let alphaEnd = $state(0.01)
  let sigmaStart = $state(6)
  let sigmaEnd = $state(0.5)
  let decay = $state<DecayId>('exp')
  let continuous = $state(false)
  let seed = $state(1)
  let density = $state<DensityId>('uniform')
  let training = $state<TrainingId>('online')

  // Images: the one picked for the density, and whether dark or light pixels count.
  let densityImageId = $state('')
  let densityImageEl = $state.raw<HTMLImageElement | null>(null)
  let invertImage = $state(false)
  let imageError = $state('')
  let densityImage = $derived(
    densityImageEl ? { cells: toDensity(densityImageEl, 128, invertImage), side: 128 } : null,
  )

  function pickDensityImage(img: HTMLImageElement, id: string) {
    densityImageEl = img
    densityImageId = id
    imageError = ''
  }

  // Picking "Image…" starts with the first bundled image.
  $effect(() => {
    if (density !== 'image' || densityImageEl) return
    loadImage(BUNDLED[0].src).then(
      img => pickDensityImage(img, BUNDLED[0].id),
      () => (imageError = "Couldn't read that image"),
    )
  })

  let running = $state(true)
  let onlineSpeed = $state(25)
  let batchSpeed = $state(4)
  let brushMode = $state<'add' | 'erase'>('add')
  let brushRadius = $state(0.08)
  let showDensity = $state(true)
  let showVoronoi = $state(false)
  let showLattice = $state(true)
  let showTrace = $state(true)
  let restart = $state(0)
  let densityReset = $state(0)
  let stepRequest = $state(0)
  let stats = $state<SomStats>({
    t: 0,
    steps: 20_000,
    alpha: 0.5,
    sigma: 6,
    qe: 0,
    te: 0,
    status: 'training',
    history: new Float32Array(),
  })

  let oneDimensional = $derived(lattice === 'chain' || lattice === 'ring')
  let config = $derived<SomConfig>({
    lattice,
    width: oneDimensional ? nodes : gridWidth,
    height: oneDimensional ? 1 : gridHeight,
    init,
    steps,
    alpha: [alphaStart, alphaEnd],
    sigma: [sigmaStart, sigmaEnd],
    decay,
    continuous,
    seed,
  })

  function pick(s: Scenario) {
    const c = s.config
    lattice = c.lattice
    if (c.lattice === 'chain' || c.lattice === 'ring') nodes = c.width
    else [gridWidth, gridHeight] = [c.width, c.height]
    init = c.init
    steps = c.steps
    ;[alphaStart, alphaEnd] = c.alpha
    ;[sigmaStart, sigmaEnd] = c.sigma
    decay = c.decay
    continuous = c.continuous
    seed = c.seed
    density = s.density
    training = 'online'
    running = true
    scenario = s
    // Even when nothing changed (the same scenario again): a clean preset, from the start.
    densityReset++
    restart++
  }

  const statusText = { training: 'Training', converged: 'Done', empty: 'No density' }
  let statusTone = $derived<Tone>(({ training: 'cobalt', converged: 'verdigris', empty: 'signal' } as const)[stats.status])
  let progress = $derived(
    continuous ? stats.t.toLocaleString() : `${stats.t.toLocaleString()} / ${stats.steps.toLocaleString()}`,
  )
</script>

<SheetLayout
  title="Kohonen maps"
  summary="A self-organising map is a lattice of nodes, each with a position in input space. Shown a sample, the nearest node and its lattice neighbours move towards it; as the neighbourhood shrinks, the lattice unfolds over the data while keeping its order. Paint the density it learns from by dragging on the sheet."
  readouts={[
    { label: 'Status', value: statusText[stats.status], tone: statusTone },
    { label: 'Samples', value: progress },
    { label: 'α', value: stats.alpha.toFixed(3) },
    { label: 'σ', value: stats.sigma.toFixed(2) },
    { label: 'QE', value: stats.qe.toFixed(3), tone: 'cobalt' },
    { label: 'TE', value: `${(stats.te * 100).toFixed(1)} %`, tone: 'amber' },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        {#if mode === 'unfolding'}
          <SomCanvas
            {wasm}
            {config}
            {density}
            {training}
            {running}
            speed={training === 'online' ? onlineSpeed : batchSpeed}
            brush={{ mode: brushMode, radius: brushRadius }}
            show={{ density: showDensity, voronoi: showVoronoi, lattice: showLattice, trace: showTrace }}
            image={densityImage}
            {restart}
            {densityReset}
            {stepRequest}
            onstats={s => (stats = s)}
          />
        {:else}
          <StatusMessage>This mode is on its way.</StatusMessage>
        {/if}
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

    <ControlGroup title="Mode">
      <Choice
        name="mode"
        bind:value={mode}
        options={MODES.map(m => ({ ...m, detail: m.value === mode ? m.detail : undefined }))}
      />
    </ControlGroup>

    {#if mode === 'unfolding'}
      <ControlGroup title="Scenario">
        <ButtonRow>
          {#each SCENARIOS as s (s.id)}
            <Button variant={scenario?.id === s.id ? 'primary' : 'default'} onclick={() => pick(s)}>{s.label}</Button>
          {/each}
        </ButtonRow>
      </ControlGroup>
    {/if}

    <ControlGroup title="Lattice">
      <Select label="Kind" bind:value={lattice} options={LATTICES} />
      {#if oneDimensional}
        <Slider label="Nodes" bind:value={nodes} min={3} max={400} />
      {:else}
        <Slider label="Columns" bind:value={gridWidth} min={2} max={20} />
        <Slider label="Rows" bind:value={gridHeight} min={2} max={20} />
      {/if}
      <Select label="Start" bind:value={init} options={INITS} />
    </ControlGroup>

    <ControlGroup title="Training">
      <Choice
        name="training"
        bind:value={training}
        options={TRAININGS.map(m => ({ ...m, detail: m.value === training ? m.detail : undefined }))}
      />
      {#if training === 'online'}
        <Slider label="Samples per frame" bind:value={onlineSpeed} min={1} max={2000} />
      {:else}
        <Slider label="Epochs per second" bind:value={batchSpeed} min={1} max={30} />
      {/if}
    </ControlGroup>

    <ControlGroup title="Schedule">
      <Slider label="Samples T" bind:value={steps} min={1000} max={100_000} step={1000} />
      <Slider label="α start" bind:value={alphaStart} min={0.05} max={1} step={0.05} />
      <Slider label="α end" bind:value={alphaEnd} min={0.001} max={0.2} step={0.001} />
      <Slider label="σ start" bind:value={sigmaStart} min={0.5} max={30} step={0.5} />
      <Slider label="σ end" bind:value={sigmaEnd} min={0.1} max={5} step={0.1} />
      <Select label="Decay" bind:value={decay} options={DECAYS} />
      <Toggle label="Continuous (keep learning at the end values)" bind:checked={continuous} />
    </ControlGroup>

    {#if mode === 'unfolding'}
      <ControlGroup title="Density">
        <Select label="Preset" bind:value={density} options={DENSITIES} />
        {#if density === 'image'}
          <ImagePicker selected={densityImageId} onpick={pickDensityImage} onerror={m => (imageError = m)} />
          <Toggle label="Light pixels are dense" bind:checked={invertImage} />
          {#if imageError}
            <StatusMessage>{imageError}</StatusMessage>
          {/if}
        {/if}
        <Choice
          name="brush"
          bind:value={brushMode}
          options={[
            { value: 'add', label: 'Paint' },
            { value: 'erase', label: 'Erase' },
          ]}
        />
        <Slider label="Brush radius" bind:value={brushRadius} min={0.02} max={0.25} step={0.01} />
        <ButtonRow>
          <Button onclick={() => densityReset++}>Reset painting</Button>
        </ButtonRow>
      </ControlGroup>

      <ControlGroup title="Show">
        <Toggle label="Density" bind:checked={showDensity} />
        <Toggle label="Voronoi regions" bind:checked={showVoronoi} />
        <Toggle label="Lattice" bind:checked={showLattice} />
        <Toggle label="Last update, while paused" bind:checked={showTrace} />
      </ControlGroup>
    {/if}
  {/snippet}

  {#snippet notes()}
    {#if stats.status === 'empty'}
      <StatusMessage>Paint some density or pick a preset.</StatusMessage>
    {/if}
    <MetricChart history={stats.history} />
    <Legend
      items={[
        { tone: 'cobalt', label: 'Lattice; shading is the kernel h' },
        { tone: 'amber', label: 'Before the last update, and the moves' },
        { tone: 'signal', label: 'Sample and its best-matching unit' },
      ]}
    />
    {#if scenario}
      <p><strong>{scenario.label}.</strong> {scenario.note}</p>
    {/if}
    <p>
      Quantization error (QE) is the mean distance from a sample to its nearest node: how well the nodes cover the data.
      Topographic error (TE) is the share of samples whose nearest and second-nearest nodes are not lattice
      neighbours: how often the map's order breaks, at a fold or a twist. Pause and step to see a single update.
    </p>
  {/snippet}
</SheetLayout>
