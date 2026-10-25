import { useState } from 'react'
import { ControlGroup, Loading, Sheet, Slider, Toggle } from '../components/Sheet'
import { DEFAULT_FLOCK, FlockCanvas, type FlockConfig, type FlockStats } from '../components/FlockCanvas'
import { useWasm } from '../lib/wasm'

function BoidsDemo() {
  const { wasm, error } = useWasm()
  const [config, setConfig] = useState<FlockConfig>(DEFAULT_FLOCK)
  const [running, setRunning] = useState(true)
  const [showPerception, setShowPerception] = useState(true)
  const [scatterKey, setScatterKey] = useState(0)
  const [stats, setStats] = useState<FlockStats>({ fps: 0, polarization: 0, avgNeighbors: 0, stragglers: 0 })

  if (!wasm) return <Loading error={error} />

  const set = (key: keyof FlockConfig) => (value: number) =>
    setConfig(prev => ({ ...prev, [key]: value }))
  const one = (v: number) => v.toFixed(1)

  return (
    <Sheet
      title="Boids"
      summary={
        <>
          Craig Reynolds’ flocking model. Each boid only sees nearby neighbors in front of it and
          steers to keep its distance, match their heading and stay with the group. Hover a boid to
          see what it perceives.
        </>
      }
      readouts={[
        { label: 'Boids', value: config.count },
        { label: 'Alignment', value: `${Math.round(stats.polarization * 100)} %`, tone: 'cobalt' },
        { label: 'Avg. neighbors', value: stats.avgNeighbors.toFixed(1) },
        { label: 'Stragglers', value: stats.stragglers, tone: 'signal' },
      ]}
      canvas={
        <FlockCanvas
          wasm={wasm}
          config={config}
          running={running}
          showPerception={showPerception}
          scatterKey={scatterKey}
          onStats={setStats}
          className="drawing-canvas"
          label="Boids flocking simulation"
        />
      }
      notes={
        <>
          <ul className="legend">
            <li><span className="swatch" data-tone="cobalt" />In a dense flock</li>
            <li><span className="swatch" data-tone="ink" />Loosely grouped</li>
            <li><span className="swatch" data-tone="signal" />Alone</li>
          </ul>
          <p>{stats.fps} frames per second</p>
        </>
      }
    >
      <div className="button-row">
        <button type="button" className="button primary" onClick={() => setRunning(r => !r)}>
          {running ? 'Pause' : 'Resume'}
        </button>
        <button type="button" className="button" onClick={() => setScatterKey(k => k + 1)}>
          Scatter
        </button>
        <button type="button" className="button quiet" onClick={() => setConfig(DEFAULT_FLOCK)}>
          Reset settings
        </button>
      </div>
      <Toggle label="Show perception of focused boid" checked={showPerception} onChange={setShowPerception} />

      <ControlGroup title="Flock">
        <Slider label="Boids" value={config.count} min={10} max={2000} step={10} onChange={set('count')} />
      </ControlGroup>

      <ControlGroup title="Separation">
        <Slider label="Radius" unit="px" value={config.separationRadius} min={5} max={80} onChange={set('separationRadius')} />
        <Slider label="Weight" value={config.separationWeight} min={0} max={5} step={0.1} format={one} onChange={set('separationWeight')} />
      </ControlGroup>

      <ControlGroup title="Alignment">
        <Slider label="Radius" unit="px" value={config.alignmentRadius} min={10} max={150} onChange={set('alignmentRadius')} />
        <Slider label="Weight" value={config.alignmentWeight} min={0} max={5} step={0.1} format={one} onChange={set('alignmentWeight')} />
      </ControlGroup>

      <ControlGroup title="Cohesion">
        <Slider label="Radius" unit="px" value={config.cohesionRadius} min={10} max={150} onChange={set('cohesionRadius')} />
        <Slider label="Weight" value={config.cohesionWeight} min={0} max={5} step={0.1} format={one} onChange={set('cohesionWeight')} />
      </ControlGroup>

      <ControlGroup title="Motion">
        <Slider label="Min speed" unit="px/s" value={config.minSpeed} min={0} max={200} onChange={set('minSpeed')} />
        <Slider label="Max speed" unit="px/s" value={config.maxSpeed} min={20} max={300} onChange={set('maxSpeed')} />
        <Slider label="Max steering" unit="px/s²" value={config.maxForce} min={20} max={800} step={10} onChange={set('maxForce')} />
        <Slider label="Wander" unit="px/s²" value={config.wander} min={0} max={300} step={5} onChange={set('wander')} />
        <Slider label="Field of view" unit="°" value={config.viewAngle} min={30} max={360} step={5} onChange={set('viewAngle')} />
      </ControlGroup>

      <ControlGroup title="Edges">
        <Slider label="Margin" unit="px" value={config.edgeMargin} min={0} max={200} onChange={set('edgeMargin')} />
        <Slider label="Weight" value={config.edgeWeight} min={0} max={8} step={0.1} format={one} onChange={set('edgeWeight')} />
      </ControlGroup>
    </Sheet>
  )
}

export default BoidsDemo
