<script lang="ts">
  import Button from '#lib/components/atoms/Button.svelte'
  import Slider from '#lib/components/atoms/Slider.svelte'
  import Toggle from '#lib/components/atoms/Toggle.svelte'
  import ButtonRow from '#lib/components/molecules/ButtonRow.svelte'
  import Choice from '#lib/components/molecules/Choice.svelte'
  import ControlGroup from '#lib/components/molecules/ControlGroup.svelte'
  import Legend from '#lib/components/molecules/Legend.svelte'
  import RoverCanvas from '#lib/components/organisms/RoverCanvas.svelte'
  import SheetLayout from '#lib/components/templates/SheetLayout.svelte'
  import type { Tone } from '#lib/components/types.ts'
  import { MODES, type ModeId, type RoverStats } from '#lib/demos/rover/rover.ts'
  import WasmGate from '#lib/wasm/WasmGate.svelte'

  let mode = $state<ModeId>('explorer')
  let running = $state(true)
  let speed = $state(4)
  let samples = $state(241)
  let showFloorPlan = $state(true)
  let showScan = $state(true)
  let showPlan = $state(true)
  let showTrail = $state(true)
  let restart = $state(0)
  let stats = $state<RoverStats>({ coverage: 0, distance: 0, time: 0, frontiers: 0, status: 'exploring' })

  const statusText = { exploring: 'Exploring', returning: 'Returning', done: 'Done' }
  let statusTone = $derived<Tone>({ exploring: 'cobalt', returning: 'amber', done: 'verdigris' }[stats.status] as Tone)
  let clock = $derived(
    `${Math.floor(stats.time / 60)}:${String(Math.floor(stats.time % 60)).padStart(2, '0')}`,
  )
</script>

<SheetLayout
  title="Rover exploration"
  summary="A differential-drive rover with a 240° laser scanner explores a floor plan it has never seen. Each scan clears the fog of war; frontiers mark where seen free space meets the unknown. The run is done when no frontier is left."
  readouts={[
    { label: 'Explored', value: `${Math.round(stats.coverage * 100)} %`, tone: 'cobalt' },
    { label: 'Status', value: statusText[stats.status], tone: statusTone },
    { label: 'Driven', value: `${stats.distance.toFixed(1)} m` },
    { label: 'Time', value: clock },
  ]}
>
  {#snippet drawing()}
    <WasmGate>
      {#snippet children(wasm)}
        <RoverCanvas
          {wasm}
          {mode}
          {running}
          {speed}
          {samples}
          {showFloorPlan}
          {showScan}
          {showPlan}
          {showTrail}
          {restart}
          onstats={s => (stats = s)}
        />
      {/snippet}
    </WasmGate>
  {/snippet}

  {#snippet controls()}
    <ButtonRow>
      <Button variant="primary" onclick={() => (running = !running)}>{running ? 'Pause' : 'Resume'}</Button>
      <Button onclick={() => restart++}>Start over</Button>
    </ButtonRow>

    <ControlGroup title="Controller">
      <Choice
        name="mode"
        bind:value={mode}
        options={MODES.map(m => ({ ...m, detail: m.value === mode ? m.detail : undefined }))}
      />
    </ControlGroup>

    <ControlGroup title="Simulation">
      <Slider label="Speed" unit="×" bind:value={speed} min={1} max={20} />
      <Slider label="Laser beams" bind:value={samples} min={21} max={681} step={20} />
    </ControlGroup>

    <ControlGroup title="Show">
      <Toggle label="True floor plan" bind:checked={showFloorPlan} />
      <Toggle label="Laser scan" bind:checked={showScan} />
      <Toggle label="Plan and frontiers given up" bind:checked={showPlan} />
      <Toggle label="Trail" bind:checked={showTrail} />
    </ControlGroup>
  {/snippet}

  {#snippet notes()}
    <Legend
      items={[
        { tone: 'amber', label: 'Frontier' },
        { tone: 'cobalt', label: 'Laser scan and plan' },
        { tone: 'signal', label: 'Trail' },
        { tone: 'verdigris', label: 'Home' },
      ]}
    />
    <p>
      {stats.frontiers} frontier {stats.frontiers === 1 ? 'cluster' : 'clusters'} reachable. The explorer drives,
      through free space it has seen and with room for its body, to the nearest spot with a clear view into the
      unknown behind one, and looks round once there; the rare spot that still stays hidden is crossed out. Switch to the 2013 controller to compare:
      it found the nearest frontier too, but only drew an arrow to it; its steering never used it.
    </p>
  {/snippet}
</SheetLayout>
