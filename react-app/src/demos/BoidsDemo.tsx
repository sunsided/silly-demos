import { useEffect, useRef, useState } from 'react'
import init, * as silly_demos from 'silly_demos';

interface BoidsStats {
  boidCount: number
  fps: number
}

function BoidsDemo() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [wasm, setWasm] = useState<typeof silly_demos | null>(null)
  const [stats, setStats] = useState<BoidsStats>({
    boidCount: 0,
    fps: 0
  })
  const [isRunning, setIsRunning] = useState(true)

  // Simulation state - using refs to avoid React re-rendering issues
  const boidsRef = useRef<Float32Array>(new Float32Array(0))
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

    // Create boids using flat array
    const boids = wasm.BoidsTests.create_boids_flat(
      config.boidCount,
      0,
      width,
      0,
      height,
      config.maxSpeed
    )
    
    boidsRef.current = new Float32Array(boids)
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
      
      // Clamp dt to avoid large jumps
      const clampedDt = Math.min(dt, 1/30)

      // Update boids simulation
      if (boidsRef.current.length > 0) {
        const updatedBoids = wasm.BoidsTests.update_boids_flat(
          boidsRef.current,
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
        boidsRef.current = new Float32Array(updatedBoids)
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
      const boidCount = boidsRef.current.length / 4
      for (let i = 0; i < boidCount; i++) {
        const idx = i * 4
        const x = boidsRef.current[idx]
        const y = boidsRef.current[idx + 1]
        const vx = boidsRef.current[idx + 2]
        const vy = boidsRef.current[idx + 3]

        // Calculate heading angle
        const angle = Math.atan2(vy, vx)
        const speed = Math.sqrt(vx * vx + vy * vy)
        
        // Color based on speed
        const normalizedSpeed = Math.min(speed / config.maxSpeed, 1)
        const hue = 180 + normalizedSpeed * 120  // Blue to green based on speed
        
        ctx.save()
        ctx.translate(x, y)
        ctx.rotate(angle)
        
        // Draw boid as small triangle/arrow
        ctx.fillStyle = `hsl(${hue}, 70%, 60%)`
        ctx.beginPath()
        ctx.moveTo(6, 0)
        ctx.lineTo(-3, -2)
        ctx.lineTo(-1, 0)
        ctx.lineTo(-3, 2)
        ctx.closePath()
        ctx.fill()
        
        // Draw small circle for body
        ctx.fillStyle = `hsl(${hue}, 50%, 80%)`
        ctx.beginPath()
        ctx.arc(-1, 0, 1.5, 0, Math.PI * 2)
        ctx.fill()
        
        ctx.restore()
      }

      // Update FPS
      fpsCounterRef.current.frames++
      if (currentTime - fpsCounterRef.current.lastTime >= 1000) {
        setStats(prev => ({ 
          ...prev, 
          fps: Math.round(fpsCounterRef.current.frames * 1000 / (currentTime - fpsCounterRef.current.lastTime))
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
  }, [wasm, isRunning, config])

  // Config change handler
  const handleConfigChange = (key: keyof typeof config, value: number) => {
    setConfig(prev => ({ ...prev, [key]: value }))
  }

  // Reset simulation
  const resetSimulation = () => {
    if (!wasm || !canvasRef.current) return
    
    const canvas = canvasRef.current
    const boids = wasm.BoidsTests.create_boids_flat(
      config.boidCount,
      config.boundaryMargin,
      canvas.width - config.boundaryMargin,
      config.boundaryMargin, 
      canvas.height - config.boundaryMargin,
      config.maxSpeed * 0.5
    )
    boidsRef.current = new Float32Array(boids)
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
