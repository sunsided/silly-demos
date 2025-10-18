import { useEffect, useRef, useState } from 'react'

// WASM module types
interface WasmModule {
  BoidsTests: typeof import('../pkg/silly_demos').BoidsTests
  run: () => void
}

function BoidsDemo() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [wasm, setWasm] = useState<WasmModule | null>(null)

  // Load WASM module
  useEffect(() => {
    let mounted = true
    
    async function loadWasm() {
      try {
        // @ts-ignore - WASM module types
        const wasmModule = await import('../pkg/silly_demos.js')
        await wasmModule.default()
        await wasmModule.run()
        
        if (mounted) {
          setWasm(wasmModule)
        }
      } catch (error) {
        console.error('Failed to load WASM module:', error)
      }
    }

    loadWasm()
    return () => { mounted = false }
  }, [])

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
        </div>
      </div>
      
      <div className="demo-info glass-panel">
        <h3>Boids Simulation</h3>
        <p>Boids simulation will be implemented here.</p>
      </div>
    </div>
  )
}

export default BoidsDemo
