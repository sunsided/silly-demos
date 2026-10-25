<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import Legend from '#lib/components/molecules/Legend.svelte'
  import FlockCanvas from '#lib/components/organisms/FlockCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import { DEFAULT_FLOCK, type FlockStats } from '#lib/demos/boids/flock.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let config = $state({ ...DEFAULT_FLOCK })
  let running = $state(true)
  let showPerception = $state(true)
  let scatter = $state(0)
  let stats = $state<FlockStats>({ fps: 0, polarization: 0, avgNeighbors: 0, stragglers: 0 })

  const one = (v: number) => v.toFixed(1)
</script>

<SheetLayout
  title="Boids"
  summary="Craig Reynolds’ flocking model. Each boid only sees nearby neighbors in front of it and steers to keep its distance, match their heading and stay with the group. Hover a boid to see what it perceives."
  readouts={[
    { label: 'Boids', value: config.count },
    { label: 'Alignment', value: `${Math.round(stats.polarization * 100)} %`, tone: 'cobalt' },
    { label: 'Avg. neighbors', value: stats.avgNeighbors.toFixed(1) },
    { label: 'Stragglers', value: stats.stragglers, tone: 'signal' },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <FlockCanvas
          {wasm}
          {config}
          {running}
          {showPerception}
          {scatter}
          onstats={s => (stats = s)}
          label="Boids flocking simulation"
        />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (running = !running)}>{running ? 'Pause' : 'Resume'}</Button>
      <Button onclick={() => scatter++}>Scatter</Button>
      <Button variant="quiet" onclick={() => (config = { ...DEFAULT_FLOCK })}>Reset settings</Button>
    </ButtonRow>
    <Toggle label="Show perception of focused boid" bind:checked={showPerception} />

    <ControlGroup title="Flock">
      <Slider label="Boids" bind:value={config.count} min={10} max={2000} step={10} />
    </ControlGroup>

    <ControlGroup title="Separation">
      <Slider label="Radius" unit="px" bind:value={config.separationRadius} min={5} max={80} />
      <Slider label="Weight" bind:value={config.separationWeight} min={0} max={5} step={0.1} format={one} />
    </ControlGroup>

    <ControlGroup title="Alignment">
      <Slider label="Radius" unit="px" bind:value={config.alignmentRadius} min={10} max={150} />
      <Slider label="Weight" bind:value={config.alignmentWeight} min={0} max={5} step={0.1} format={one} />
    </ControlGroup>

    <ControlGroup title="Cohesion">
      <Slider label="Radius" unit="px" bind:value={config.cohesionRadius} min={10} max={150} />
      <Slider label="Weight" bind:value={config.cohesionWeight} min={0} max={5} step={0.1} format={one} />
    </ControlGroup>

    <ControlGroup title="Motion">
      <Slider label="Min speed" unit="px/s" bind:value={config.minSpeed} min={0} max={200} />
      <Slider label="Max speed" unit="px/s" bind:value={config.maxSpeed} min={20} max={300} />
      <Slider label="Max steering" unit="px/s²" bind:value={config.maxForce} min={20} max={800} step={10} />
      <Slider label="Wander" unit="px/s²" bind:value={config.wander} min={0} max={300} step={5} />
      <Slider label="Field of view" unit="°" bind:value={config.viewAngle} min={30} max={360} step={5} />
    </ControlGroup>

    <ControlGroup title="Edges">
      <Slider label="Margin" unit="px" bind:value={config.edgeMargin} min={0} max={200} />
      <Slider label="Weight" bind:value={config.edgeWeight} min={0} max={8} step={0.1} format={one} />
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    <Legend
      items={[
        { tone: 'cobalt', label: 'In a dense flock' },
        { tone: 'ink', label: 'Loosely grouped' },
        { tone: 'signal', label: 'Alone' },
      ]}
    />
    <p>{stats.fps} frames per second</p>
  {/snippet}
</SheetLayout>
