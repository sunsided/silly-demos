import { useEffect, useRef, useState } from 'react'

// WASM module types
interface WasmModule {
  circle_collision: (x1: number, y1: number, r1: number, x2: number, y2: number, r2: number) => {
    intersect: boolean
    distance: number
    penetration: number
    dx: number
    dy: number
  }
  run: () => void
}

interface Circle {
  x: number
  y: number
  r: number
  offx: number
  offy: number
}

function CircleCollisionDemo() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [wasm, setWasm] = useState<WasmModule | null>(null)
  const [stats, setStats] = useState({
    intersect: false,
    distance: 0,
    penetration: 0
  })

  // Circle state - using refs to avoid React re-rendering issues
  const circlesRef = useRef<{A: Circle, B: Circle}>({
    A: { x: -140, y: 0, r: 90, offx: 0, offy: 0 },
    B: { x: 140, y: 0, r: 110, offx: 0, offy: 0 }
  })

  const draggingRef = useRef<Circle | null>(null)
  const animationRef = useRef<number>(0)
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null)

  // Load WASM module
  useEffect(() => {
    let mounted = true
    
    async function loadWasm() {
      try {
        // @ts-ignore - WASM module types
        const wasmModule = await import('../pkg/silly_demos.js')
        await wasmModule.default()
        // Call run() like in the original HTML
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

  // Canvas setup and animation
  useEffect(() => {
    if (!wasm || !canvasRef.current) return

    const canvas = canvasRef.current
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    function setupCanvas() {
      const rect = canvas.getBoundingClientRect()
      const displayWidth = rect.width
      const displayHeight = rect.height
      
      if (displayWidth <= 0 || displayHeight <= 0 || !ctx) return
      
      const dpr = window.devicePixelRatio || 1
      
      canvas.width = displayWidth * dpr
      canvas.height = displayHeight * dpr
      
      ctx.setTransform(dpr, 0, 0, dpr, canvas.width / 2, canvas.height / 2)
      
      // @ts-ignore - adding custom properties
      canvas._displayWidth = displayWidth
      // @ts-ignore - adding custom properties
      canvas._displayHeight = displayHeight
    }

    function getCSSVar(name: string): string {
      return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
    }

    function drawCircle(c: Circle, fillVar: string, strokeVar: string) {
      if (!ctx) return
      
      const gradient = ctx.createRadialGradient(
        c.x - c.r * 0.3, c.y - c.r * 0.3, 0,
        c.x, c.y, c.r
      )
      const fillColor = getCSSVar(fillVar)
      gradient.addColorStop(0, fillColor.replace(/\/[^)]*\)/, '/ 0.95)'))
      gradient.addColorStop(1, fillColor)
      
      ctx.beginPath()
      ctx.arc(c.x, c.y, c.r, 0, Math.PI * 2)
      ctx.fillStyle = gradient
      ctx.fill()
      
      ctx.shadowColor = getCSSVar(strokeVar)
      ctx.shadowBlur = 8
      ctx.lineWidth = 3
      ctx.strokeStyle = getCSSVar(strokeVar)
      ctx.stroke()
      
      ctx.shadowColor = 'transparent'
      ctx.shadowBlur = 0
    }

    function draw() {
      if (!wasm || !ctx) return
      
      const { A, B } = circlesRef.current
      const result = wasm.circle_collision(A.x, A.y, A.r, B.x, B.y, B.r)
      
      setStats({
        intersect: result.intersect,
        distance: result.distance,
        penetration: result.penetration
      })

      // @ts-ignore - custom canvas properties
      const width = canvas._displayWidth || canvas.getBoundingClientRect().width
      // @ts-ignore - custom canvas properties
      const height = canvas._displayHeight || canvas.getBoundingClientRect().height
      ctx.clearRect(-width / 2, -height / 2, width, height)
      
      // Draw connecting line
      ctx.beginPath()
      ctx.moveTo(A.x, A.y)
      ctx.lineTo(B.x, B.y)
      
      ctx.shadowColor = getCSSVar('--line-glow')
      ctx.shadowBlur = 6
      ctx.lineWidth = 2
      ctx.strokeStyle = getCSSVar('--line-color')
      ctx.setLineDash(getCSSVar('--line-style').split(',').map(parseFloat))
      ctx.stroke()
      
      ctx.shadowColor = 'transparent'
      ctx.shadowBlur = 0
      ctx.setLineDash([])

      // Draw circles
      if (result.intersect) {
        drawCircle(A, '--circle-a-hit', '--circle-a-stroke-hit')
        drawCircle(B, '--circle-b-hit', '--circle-b-stroke-hit')
      } else {
        drawCircle(A, '--circle-a-normal', '--circle-a-stroke')
        drawCircle(B, '--circle-b-normal', '--circle-b-stroke')
      }

      animationRef.current = requestAnimationFrame(draw)
    }

    setupCanvas()
    
    const handleResize = () => {
      setupCanvas()
    }

    window.addEventListener('resize', handleResize)
    draw()

    return () => {
      window.removeEventListener('resize', handleResize)
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
      }
    }
  }, [wasm])

  // Mouse/touch event handlers
  useEffect(() => {
    if (!canvasRef.current || !wasm) return

    const canvas = canvasRef.current

    function getPoint(e: PointerEvent | MouseEvent) {
      const rect = canvas.getBoundingClientRect()
      return {
        x: (e.clientX - rect.left) - rect.width / 2,
        y: (e.clientY - rect.top) - rect.height / 2
      }
    }

    function hitTest(c: Circle, p: { x: number, y: number }) {
      const dx = p.x - c.x
      const dy = p.y - c.y
      return dx * dx + dy * dy <= c.r * c.r
    }

    function updateCursor(e: MouseEvent) {
      const point = getPoint(e)
      const { A, B } = circlesRef.current
      
      if (hitTest(A, point) || hitTest(B, point)) {
        canvas.style.cursor = 'pointer'
      } else {
        canvas.style.cursor = 'default'
      }
    }

    function handlePointerDown(e: PointerEvent) {
      e.preventDefault()
      const point = getPoint(e)
      const { A, B } = circlesRef.current

      if (hitTest(A, point)) {
        draggingRef.current = A
        A.offx = point.x - A.x
        A.offy = point.y - A.y
        canvas.setPointerCapture(e.pointerId)
        canvas.style.cursor = 'grabbing'
      } else if (hitTest(B, point)) {
        draggingRef.current = B
        B.offx = point.x - B.x
        B.offy = point.y - B.y
        canvas.setPointerCapture(e.pointerId)
        canvas.style.cursor = 'grabbing'
      }
    }

    function handlePointerMove(e: PointerEvent) {
      if (!draggingRef.current) return
      e.preventDefault()

      const point = getPoint(e)
      draggingRef.current.x = point.x - draggingRef.current.offx
      draggingRef.current.y = point.y - draggingRef.current.offy
    }

    function handlePointerUp(e: PointerEvent) {
      if (draggingRef.current) {
        e.preventDefault()
        draggingRef.current = null
        // Reset cursor based on current mouse position
        updateCursor(e as any)
      }
    }

    // Add event listeners
    canvas.addEventListener('pointerdown', handlePointerDown)
    canvas.addEventListener('pointermove', handlePointerMove)
    canvas.addEventListener('pointerup', handlePointerUp)
    canvas.addEventListener('pointercancel', handlePointerUp)
    canvas.addEventListener('mousemove', updateCursor)

    return () => {
      canvas.removeEventListener('pointerdown', handlePointerDown)
      canvas.removeEventListener('pointermove', handlePointerMove)
      canvas.removeEventListener('pointerup', handlePointerUp)
      canvas.removeEventListener('pointercancel', handlePointerUp)
      canvas.removeEventListener('mousemove', updateCursor)
    }
  }, [wasm]) // Depend on wasm being loaded

  if (!wasm) {
    return (
      <div className="loading-container">
        <div className="glass-panel">
          <h3 className="gradient-title">Loading WASM module...</h3>
          <p className="description-text">Please wait while the WebAssembly module loads.</p>
        </div>
      </div>
    )
  }

  return (
    <div className="grid-layout two-column">
      <canvas ref={canvasRef} className="modern-canvas" />
      <div className="glass-panel">
        <h3 className="gradient-title">WASM circle collision</h3>
        <div className="stat-row">
          Intersect: <span className="pill">{stats.intersect ? 'yes' : 'no'}</span>
        </div>
        <div className="stat-row">
          Distance: <span className="pill">{stats.distance.toFixed(2)}</span>
        </div>
        <div className="stat-row">
          Penetration: <span className="pill">{stats.penetration.toFixed(2)}</span>
        </div>
        <p className="description-text">
          Drag the circles. Colors switch on intersection.<br/>
          Call boundary: <code className="code-highlight">circle_collision(x1,y1,r1,x2,y2,r2)</code> (1 call/frame).
        </p>
      </div>
    </div>
  )
}

export default CircleCollisionDemo
