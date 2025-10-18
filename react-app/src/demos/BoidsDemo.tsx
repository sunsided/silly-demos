import { useEffect, useRef, useState } from 'react'
import init, * as silly_demos from 'silly_demos';

interface BoidsStats {
  boidCount: number
  fps: number
}

// Each boid is [x, y, vx, vy]
interface Boid {
  x: number;
  y: number;
  vx: number;
  vy: number;
}

function boidsToFloat32Array(boids: Boid[]): Float32Array {
  const arr = new Float32Array(boids.length * 4);
  for (let i = 0; i < boids.length; i++) {
    arr[i * 4] = boids[i].x;
    arr[i * 4 + 1] = boids[i].y;
    arr[i * 4 + 2] = boids[i].vx;
    arr[i * 4 + 3] = boids[i].vy;
  }
  return arr;
}

function float32ArrayToBoids(arr: Float32Array): Boid[] {
  const boids: Boid[] = [];
  for (let i = 0; i < arr.length; i += 4) {
    boids.push({
      x: arr[i],
      y: arr[i + 1],
      vx: arr[i + 2],
      vy: arr[i + 3],
    });
  }
  return boids;
}

function createRandomBoids(count: number, width: number, height: number, maxSpeed: number): Boid[] {
  const boids: Boid[] = [];
  for (let i = 0; i < count; i++) {
    boids.push({
      x: Math.random() * width,
      y: Math.random() * height,
      vx: (Math.random() - 0.5) * maxSpeed,
      vy: (Math.random() - 0.5) * maxSpeed,
    });
  }
  return boids;
}

function BoidsDemo() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [wasm, setWasm] = useState<typeof silly_demos | null>(null)
  const [stats, setStats] = useState<BoidsStats>({
    boidCount: 0,
    fps: 0
  })
  const [isRunning, setIsRunning] = useState(true)

  // Visualization toggles
  const [showSeparation, setShowSeparation] = useState(true)
  const [showAlignment, setShowAlignment] = useState(true)
  const [showCohesion, setShowCohesion] = useState(true)

  // Simulation state - using refs to avoid React re-rendering issues
  const boidsRef = useRef<Boid[]>([])
  const lastTimeRef = useRef<number>(0)
  const animationRef = useRef<number>(0)
  const fpsCounterRef = useRef({ frames: 0, lastTime: 0 })

  // Configuration
  const [config, setConfig] = useState({
    boidCount: 150,
    separationRadius: 25,
    alignmentRadius: 50,
    cohesionRadius: 50,
    separationStrength: 1.5,
    alignmentStrength: 1.0,
    cohesionStrength: 1.0,
    maxSpeed: 100,
    maxForce: 3.0,
    boundaryMargin: 50,
    boundaryStrength: 2.0
  })

  // Load WASM module
  useEffect(() => {
    let cancel = false;
    init()
      .then(() => {
        if (!cancel) {
          setWasm(silly_demos);
        }
      })
      .catch((error: unknown) => {
        if (!cancel) {
          console.error('Failed to load WASM module:', error);
        }
      });
    return () => { cancel = true; };
  }, []);

  // Initialize simulation when WASM loads
  useEffect(() => {
    if (!wasm || !canvasRef.current) return

    const canvas = canvasRef.current
    const width = canvas.width
    const height = canvas.height

    // Create boids as array of struct
    const boids = createRandomBoids(config.boidCount, width, height, config.maxSpeed)
    boidsRef.current = boids
    setStats(prev => ({ ...prev, boidCount: config.boidCount }))
  }, [wasm, config.boidCount])

  // Animation loop
  useEffect(() => {
    if (!wasm || !canvasRef.current) return

    function animate(currentTime: number) {
      if (!canvasRef.current || !wasm || !isRunning) {
        animationRef.current = requestAnimationFrame(animate)
        return
      }

      const canvas = canvasRef.current
      const ctx = canvas.getContext('2d')!
      const width = canvas.width
      const height = canvas.height

      // Calculate delta time
      const dt = lastTimeRef.current === 0 ? 1/60 : (currentTime - lastTimeRef.current) / 1000
      lastTimeRef.current = currentTime
      const clampedDt = Math.min(dt, 1/30)

      // Update boids simulation
      if (boidsRef.current.length > 0) {
        // Convert to flat array for WASM
        const flat = boidsToFloat32Array(boidsRef.current)
        const updatedFlat = wasm.BoidsTests.update_boids_flat(
          flat,
          config.separationRadius,
          config.alignmentRadius,
          config.cohesionRadius,
          config.separationStrength,
          config.alignmentStrength,
          config.cohesionStrength,
          config.maxSpeed,
          config.maxForce,
          config.boundaryMargin,
          config.boundaryStrength,
          width,
          height,
          clampedDt
        )
        boidsRef.current = float32ArrayToBoids(new Float32Array(updatedFlat))
      }

      // Clear canvas with slight trail effect
      ctx.fillStyle = 'rgba(10, 10, 20, 0.1)'
      ctx.fillRect(0, 0, width, height)

      // Draw boundary visualization
      ctx.strokeStyle = 'rgba(100, 100, 150, 0.3)'
      ctx.lineWidth = 1
      ctx.strokeRect(config.boundaryMargin, config.boundaryMargin, 
                    width - 2 * config.boundaryMargin, height - 2 * config.boundaryMargin)

      // Draw boids
      const boids = boidsRef.current
      for (let i = 0; i < boids.length; i++) {
        const { x, y, vx, vy } = boids[i]
        const angle = Math.atan2(vy, vx)
        const speed = Math.sqrt(vx * vx + vy * vy)
        const normalizedSpeed = Math.min(speed / config.maxSpeed, 1)
        const hue = 180 + normalizedSpeed * 120
        ctx.save()
        ctx.translate(x, y)
        ctx.rotate(angle)
        // Visualize radii (separation, alignment, cohesion)
        ctx.save()
        ctx.setTransform(1, 0, 0, 1, 0, 0) // Reset transform for circles
        ctx.globalAlpha = 0.08
        if (showCohesion) {
          ctx.beginPath()
          ctx.arc(x, y, config.cohesionRadius, 0, Math.PI * 2)
          ctx.fillStyle = '#00bcd4' // Cyan for cohesion
          ctx.fill()
        }
        if (showAlignment) {
          ctx.beginPath()
          ctx.arc(x, y, config.alignmentRadius, 0, Math.PI * 2)
          ctx.fillStyle = '#8bc34a' // Light green for alignment
          ctx.fill()
        }
        if (showSeparation) {
          ctx.beginPath()
          ctx.arc(x, y, config.separationRadius, 0, Math.PI * 2)
          ctx.fillStyle = '#ff9800' // Orange for separation
          ctx.fill()
        }
        ctx.globalAlpha = 1
        ctx.restore()
        // Draw boid as small triangle/arrow
        ctx.fillStyle = `hsl(${hue}, 70%, 60%)`
        ctx.beginPath()
        ctx.moveTo(6, 0)
        ctx.lineTo(-3, -2)
        ctx.lineTo(-1, 0)
        ctx.lineTo(-3, 2)
        ctx.closePath()
        ctx.fill()
        ctx.fillStyle = `hsl(${hue}, 50%, 80%)`
        ctx.beginPath()
        ctx.arc(-1, 0, 1.5, 0, Math.PI * 2)
        ctx.fill()
        ctx.restore()
      }

      // Update FPS
      fpsCounterRef.current.frames++
      if (currentTime - fpsCounterRef.current.lastTime >= 1000) {
        const elapsed = currentTime - fpsCounterRef.current.lastTime;
        const fps = elapsed > 0
          ? Math.round(fpsCounterRef.current.frames * 1000 / elapsed)
          : 0;
        setStats(prev => ({ 
          ...prev, 
          fps
        }))
        fpsCounterRef.current.frames = 0
        fpsCounterRef.current.lastTime = currentTime
      }

      animationRef.current = requestAnimationFrame(animate)
    }

    animationRef.current = requestAnimationFrame(animate)

    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
      }
    }
  }, [wasm, isRunning, config, showSeparation, showAlignment, showCohesion])

  // Config change handler
  const handleConfigChange = (key: keyof typeof config, value: number) => {
    setConfig(prev => ({ ...prev, [key]: value }))
  }

  // Reset simulation
  const resetSimulation = () => {
    if (!wasm || !canvasRef.current) return
    const canvas = canvasRef.current
    boidsRef.current = createRandomBoids(config.boidCount, canvas.width, canvas.height, config.maxSpeed * 0.5)
    setStats(prev => ({ ...prev, boidCount: config.boidCount }))
  }

  if (!wasm) {
    return (
      <div className="demo-container">
        <div className="glass-panel">
          <div className="loading">Loading WASM module...</div>
        </div>
      </div>
    )
  }

  return (
    <div className="demo-container">
      <div className="demo-content">
        <div className="canvas-container">
          <canvas 
            ref={canvasRef}
            width={800}
            height={600}
            className="demo-canvas"
          />
          
          <div className="canvas-overlay">
            <div className="stats-panel">
              <div className="stat-item">
                <span className="stat-label">Boids:</span>
                <span className="stat-value">{stats.boidCount}</span>
              </div>
              <div className="stat-item">
                <span className="stat-label">FPS:</span>
                <span className="stat-value">{stats.fps}</span>
              </div>
            </div>
          </div>
        </div>

        <div className="controls-panel glass-panel">
          <h3>Boids Simulation Controls</h3>
          
          <div className="control-section">
            <h4>Simulation</h4>
            <div className="control-row">
              <button 
                className={`control-button ${isRunning ? 'active' : ''}`}
                onClick={() => setIsRunning(!isRunning)}
              >
                {isRunning ? 'Pause' : 'Play'}
              </button>
              <button className="control-button" onClick={resetSimulation}>
                Reset
              </button>
            </div>
            
            <div className="control-group">
              <label>
                Boid Count: {config.boidCount}
                <input
                  type="range"
                  min="10"
                  max="300"
                  value={config.boidCount}
                  onChange={(e) => handleConfigChange('boidCount', parseInt(e.target.value))}
                />
              </label>
            </div>
          </div>

          <div className="control-section">
            <h4>Visualization</h4>
            <div className="control-group">
              <label>
                <input type="checkbox" checked={showSeparation} onChange={e => setShowSeparation(e.target.checked)} />
                Show Separation Radius
              </label>
              <label>
                <input type="checkbox" checked={showAlignment} onChange={e => setShowAlignment(e.target.checked)} />
                Show Alignment Radius
              </label>
              <label>
                <input type="checkbox" checked={showCohesion} onChange={e => setShowCohesion(e.target.checked)} />
                Show Cohesion Radius
              </label>
            </div>
          </div>

          <div className="control-section">
            <h4>Flocking Behavior</h4>
            <div className="control-group">
              <label>
                Separation Radius: {config.separationRadius}px
                <input
                  type="range"
                  min="10"
                  max="100"
                  value={config.separationRadius}
                  onChange={(e) => handleConfigChange('separationRadius', parseFloat(e.target.value))}
                />
              </label>
              <label>
                Separation Strength: {config.separationStrength.toFixed(1)}
                <input
                  type="range"
                  min="0"
                  max="5"
                  step="0.1"
                  value={config.separationStrength}
                  onChange={(e) => handleConfigChange('separationStrength', parseFloat(e.target.value))}
                />
              </label>
            </div>
            
            <div className="control-group">
              <label>
                Alignment Radius: {config.alignmentRadius}px
                <input
                  type="range"
                  min="10"
                  max="150"
                  value={config.alignmentRadius}
                  onChange={(e) => handleConfigChange('alignmentRadius', parseFloat(e.target.value))}
                />
              </label>
              <label>
                Alignment Strength: {config.alignmentStrength.toFixed(1)}
                <input
                  type="range"
                  min="0"
                  max="3"
                  step="0.1"
                  value={config.alignmentStrength}
                  onChange={(e) => handleConfigChange('alignmentStrength', parseFloat(e.target.value))}
                />
              </label>
            </div>
            
            <div className="control-group">
              <label>
                Cohesion Radius: {config.cohesionRadius}px
                <input
                  type="range"
                  min="10"
                  max="150"
                  value={config.cohesionRadius}
                  onChange={(e) => handleConfigChange('cohesionRadius', parseFloat(e.target.value))}
                />
              </label>
              <label>
                Cohesion Strength: {config.cohesionStrength.toFixed(1)}
                <input
                  type="range"
                  min="0"
                  max="3"
                  step="0.1"
                  value={config.cohesionStrength}
                  onChange={(e) => handleConfigChange('cohesionStrength', parseFloat(e.target.value))}
                />
              </label>
            </div>
          </div>

          <div className="control-section">
            <h4>Movement</h4>
            <div className="control-group">
              <label>
                Max Speed: {config.maxSpeed}px/s
                <input
                  type="range"
                  min="20"
                  max="200"
                  value={config.maxSpeed}
                  onChange={(e) => handleConfigChange('maxSpeed', parseFloat(e.target.value))}
                />
              </label>
              <label>
                Max Force: {config.maxForce.toFixed(1)}
                <input
                  type="range"
                  min="0.5"
                  max="10"
                  step="0.1"
                  value={config.maxForce}
                  onChange={(e) => handleConfigChange('maxForce', parseFloat(e.target.value))}
                />
              </label>
            </div>
          </div>

          <div className="control-section">
            <h4>Boundaries</h4>
            <div className="control-group">
              <label>
                Boundary Margin: {config.boundaryMargin}px
                <input
                  type="range"
                  min="10"
                  max="100"
                  value={config.boundaryMargin}
                  onChange={(e) => handleConfigChange('boundaryMargin', parseFloat(e.target.value))}
                />
              </label>
              <label>
                Boundary Strength: {config.boundaryStrength.toFixed(1)}
                <input
                  type="range"
                  min="0.5"
                  max="10"
                  step="0.1"
                  value={config.boundaryStrength}
                  onChange={(e) => handleConfigChange('boundaryStrength', parseFloat(e.target.value))}
                />
              </label>
            </div>
          </div>
        </div>
      </div>

      <div className="demo-info glass-panel">
        <h3>About Boids Simulation</h3>
        <p>
          This simulation implements Craig Reynolds' classic boids algorithm with three fundamental rules:
        </p>
        <ul>
          <li><strong>Separation:</strong> Avoid crowding neighbors</li>
          <li><strong>Alignment:</strong> Steer towards average heading of neighbors</li>
          <li><strong>Cohesion:</strong> Steer towards average position of neighbors</li>
        </ul>
        <p>
          The simulation runs entirely in WebAssembly using Rust for maximum performance, 
          allowing for smooth real-time flocking behavior with hundreds of boids.
        </p>
        <p>
          Adjust the parameters to see how they affect the emergent flocking behavior!
        </p>
      </div>
    </div>
  )
}

export default BoidsDemo
