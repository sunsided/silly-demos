import { useEffect, useRef, useState } from 'react'

// WASM module types
interface WasmModule {
  GeometryTests: typeof import('../pkg/silly_demos').GeometryTests
  run: () => void
}

interface Point {
  x: number
  y: number
  offx: number
  offy: number
}

interface LineSegment {
  start: Point
  end: Point
}

function PointLineDemo() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [wasm, setWasm] = useState<WasmModule | null>(null)
  const [stats, setStats] = useState({
    distance: 0,
    closest_x: 0,
    closest_y: 0,
    on_segment: false,
    side: 0,
    ortho_distance: 0 // Distance to infinite line (orthogonal projection)
  })

  // State - using refs to avoid React re-rendering issues
  const lineRef = useRef<LineSegment>({
    start: { x: -150, y: -50, offx: 0, offy: 0 },
    end: { x: 150, y: 50, offx: 0, offy: 0 }
  })
  
  const pointRef = useRef<Point>({ x: 0, y: -100, offx: 0, offy: 0 })

  const draggingRef = useRef<Point | null>(null)
  const animationRef = useRef<number>(0)

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

    function drawPoint(p: Point, color: string, strokeColor: string, radius = 8) {
      if (!ctx) return
      
      ctx.beginPath()
      ctx.arc(p.x, p.y, radius, 0, Math.PI * 2)
      ctx.fillStyle = color
      ctx.fill()
      
      ctx.lineWidth = 2
      ctx.strokeStyle = strokeColor
      ctx.stroke()
    }

    function drawLineSegment(line: LineSegment, color: string, width = 3) {
      if (!ctx) return
      
      const dx = line.end.x - line.start.x
      const dy = line.end.y - line.start.y
      const length = Math.sqrt(dx * dx + dy * dy)
      
      if (length < 1) return
      
      // Normalize direction vector
      const ux = dx / length
      const uy = dy / length
      
      // Draw main line segment
      ctx.beginPath()
      ctx.moveTo(line.start.x, line.start.y)
      ctx.lineTo(line.end.x, line.end.y)
      ctx.lineWidth = width
      ctx.strokeStyle = color
      ctx.setLineDash([])
      ctx.stroke()
      
      // Draw arrowhead at end
      const arrowSize = 12
      const arrowX = line.end.x - arrowSize * ux
      const arrowY = line.end.y - arrowSize * uy
      
      ctx.beginPath()
      ctx.moveTo(line.end.x, line.end.y)
      ctx.lineTo(arrowX - arrowSize * 0.5 * uy, arrowY + arrowSize * 0.5 * ux)
      ctx.moveTo(line.end.x, line.end.y)
      ctx.lineTo(arrowX + arrowSize * 0.5 * uy, arrowY - arrowSize * 0.5 * ux)
      ctx.lineWidth = width
      ctx.stroke()
    }

    function drawInfiniteLine(line: LineSegment, color: string, width = 1) {
      if (!ctx) return
      
      const dx = line.end.x - line.start.x
      const dy = line.end.y - line.start.y
      const length = Math.sqrt(dx * dx + dy * dy)
      
      if (length < 1) return
      
      // Normalize direction vector
      const ux = dx / length
      const uy = dy / length
      
      // Extend line to canvas bounds (approximate)
      // @ts-ignore - custom canvas properties
      const width_canvas = canvas._displayWidth || 800
      // @ts-ignore - custom canvas properties
      const height_canvas = canvas._displayHeight || 600
      const extension = Math.max(width_canvas, height_canvas)
      
      const startExt = {
        x: line.start.x - extension * ux,
        y: line.start.y - extension * uy
      }
      
      const endExt = {
        x: line.end.x + extension * ux,
        y: line.end.y + extension * uy
      }
      
      // Draw extended dashed line
      ctx.beginPath()
      ctx.moveTo(startExt.x, startExt.y)
      ctx.lineTo(line.start.x, line.start.y)
      ctx.moveTo(line.end.x, line.end.y)
      ctx.lineTo(endExt.x, endExt.y)
      
      ctx.lineWidth = width
      ctx.strokeStyle = color
      ctx.setLineDash([8, 8])
      ctx.stroke()
      ctx.setLineDash([])
    }

    function draw() {
      if (!wasm || !ctx) return
      
      const line = lineRef.current
      const point = pointRef.current
      
      const result = wasm.GeometryTests.point_line_test(
        line.start.x, line.start.y,
        line.end.x, line.end.y,
        point.x, point.y
      )
      
      // Calculate orthogonal distance to infinite line
      let ortho_distance = 0
      const dx = line.end.x - line.start.x
      const dy = line.end.y - line.start.y
      const line_length_sq = dx * dx + dy * dy
      
      if (line_length_sq > 1e-6) {
        const t = ((point.x - line.start.x) * dx + (point.y - line.start.y) * dy) / line_length_sq
        const ortho_x = line.start.x + t * dx
        const ortho_y = line.start.y + t * dy
        const ortho_dx = point.x - ortho_x
        const ortho_dy = point.y - ortho_y
        ortho_distance = Math.sqrt(ortho_dx * ortho_dx + ortho_dy * ortho_dy)
      }
      
      setStats({
        distance: result.distance,
        closest_x: result.closest_x,
        closest_y: result.closest_y,
        on_segment: result.on_segment,
        side: result.side,
        ortho_distance: ortho_distance
      })

      // @ts-ignore - custom canvas properties
      const width = canvas._displayWidth || canvas.getBoundingClientRect().width
      // @ts-ignore - custom canvas properties
      const height = canvas._displayHeight || canvas.getBoundingClientRect().height
      ctx.clearRect(-width / 2, -height / 2, width, height)
      
      // Draw infinite line (dashed, toned down)
      const dashedColor = getCSSVar('--line-dashed')
      drawInfiniteLine(line, dashedColor, 1)
      
      // Draw main line segment with arrow
      const lineColor = getCSSVar('--line-color')
      drawLineSegment(line, lineColor, 3)
      
      // Draw line segment endpoints (handles)
      const handleColor = getCSSVar('--circle-a-normal')
      const handleStroke = getCSSVar('--circle-a-stroke')
      drawPoint(line.start, handleColor, handleStroke, 6)
      drawPoint(line.end, handleColor, handleStroke, 6)
      
      // Draw test point
      const pointColor = result.side > 0 ? getCSSVar('--circle-b-normal') : 
                        result.side < 0 ? getCSSVar('--circle-a-hit') : 
                        getCSSVar('--circle-b-hit')
      const pointStroke = result.side > 0 ? getCSSVar('--circle-b-stroke') : 
                         result.side < 0 ? getCSSVar('--circle-a-stroke-hit') : 
                         getCSSVar('--circle-b-stroke-hit')
      drawPoint(point, pointColor, pointStroke, 8)
      
      // Draw closest point and distance line
      const closestColor = getCSSVar('--line-glow')
      drawPoint({ x: result.closest_x, y: result.closest_y, offx: 0, offy: 0 }, 
                closestColor, closestColor, 4)
      
      // Draw distance line to closest point
      ctx.beginPath()
      ctx.moveTo(point.x, point.y)
      ctx.lineTo(result.closest_x, result.closest_y)
      ctx.lineWidth = 2
      ctx.strokeStyle = closestColor
      ctx.setLineDash([4, 4])
      ctx.stroke()
      ctx.setLineDash([])
      
      // If point is outside segment, also show orthogonal projection to infinite line
      if (!result.on_segment) {
        // Calculate orthogonal projection manually
        const dx = line.end.x - line.start.x
        const dy = line.end.y - line.start.y
        const line_length_sq = dx * dx + dy * dy
        
        if (line_length_sq > 1e-6) {
          const t = ((point.x - line.start.x) * dx + (point.y - line.start.y) * dy) / line_length_sq
          const ortho_x = line.start.x + t * dx
          const ortho_y = line.start.y + t * dy
          
          // Choose color based on which side of the line the point is on
          const orthoColor = result.side > 0 ? getCSSVar('--ortho-right-side') : 
                            result.side < 0 ? getCSSVar('--ortho-left-side') : 
                            getCSSVar('--ortho-on-line')
          
          // Draw orthogonal projection point
          drawPoint({ x: ortho_x, y: ortho_y, offx: 0, offy: 0 }, 
                    orthoColor, orthoColor, 3)
          
          // Draw orthogonal line (perpendicular to infinite line) with matching color
          ctx.beginPath()
          ctx.moveTo(point.x, point.y)
          ctx.lineTo(ortho_x, ortho_y)
          ctx.lineWidth = 1
          ctx.strokeStyle = orthoColor
          ctx.setLineDash([2, 2])
          ctx.stroke()
          ctx.setLineDash([])
        }
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

    function hitTest(p: Point, testPoint: { x: number, y: number }, radius = 12) {
      const dx = testPoint.x - p.x
      const dy = testPoint.y - p.y
      return dx * dx + dy * dy <= radius * radius
    }

    function updateCursor(e: MouseEvent) {
      const testPoint = getPoint(e)
      const line = lineRef.current
      const point = pointRef.current
      
      if (hitTest(line.start, testPoint, 12) || 
          hitTest(line.end, testPoint, 12) || 
          hitTest(point, testPoint, 12)) {
        canvas.style.cursor = 'pointer'
      } else {
        canvas.style.cursor = 'default'
      }
    }

    function handlePointerDown(e: PointerEvent) {
      e.preventDefault()
      const testPoint = getPoint(e)
      const line = lineRef.current
      const point = pointRef.current

      if (hitTest(line.start, testPoint, 12)) {
        draggingRef.current = line.start
        line.start.offx = testPoint.x - line.start.x
        line.start.offy = testPoint.y - line.start.y
        canvas.setPointerCapture(e.pointerId)
        canvas.style.cursor = 'grabbing'
      } else if (hitTest(line.end, testPoint, 12)) {
        draggingRef.current = line.end
        line.end.offx = testPoint.x - line.end.x
        line.end.offy = testPoint.y - line.end.y
        canvas.setPointerCapture(e.pointerId)
        canvas.style.cursor = 'grabbing'
      } else if (hitTest(point, testPoint, 12)) {
        draggingRef.current = point
        point.offx = testPoint.x - point.x
        point.offy = testPoint.y - point.y
        canvas.setPointerCapture(e.pointerId)
        canvas.style.cursor = 'grabbing'
      }
    }

    function handlePointerMove(e: PointerEvent) {
      if (!draggingRef.current) return
      e.preventDefault()

      const testPoint = getPoint(e)
      draggingRef.current.x = testPoint.x - draggingRef.current.offx
      draggingRef.current.y = testPoint.y - draggingRef.current.offy
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
  }, [wasm])

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
        <h3 className="gradient-title">Point-line distance test</h3>
        <div className="stat-row">
          Distance: <span className="pill">{stats.distance.toFixed(2)}</span>
        </div>
        <div className="stat-row">
          Orthogonal distance: <span className="pill">{stats.ortho_distance.toFixed(2)}</span>
        </div>
        <div className="stat-row">
          Closest point: <span className="pill">({stats.closest_x.toFixed(1)}, {stats.closest_y.toFixed(1)})</span>
        </div>
        <div className="stat-row">
          On segment: <span className="pill">{stats.on_segment ? 'yes' : 'no'}</span>
        </div>
        <div className="stat-row">
          Side: <span className="pill">
            {stats.side > 0 ? 'right' : stats.side < 0 ? 'left' : 'on line'}
          </span>
        </div>
        <p className="description-text">
          Drag the line endpoints (small circles) or the test point (large circle). 
          The dashed line shows the infinite extension of the line segment.
          Point color indicates which side of the line it's on.
          <br/><br/>
          <strong>Blue dot:</strong> Closest point on segment<br/>
          <strong>Color-coded dot:</strong> Orthogonal projection (when outside segment)<br/>
          • <span style={{color: 'rgb(102, 187, 106)'}}>Green</span> = right side<br/>
          • <span style={{color: 'rgb(255, 152, 102)'}}>Orange</span> = left side<br/>
          • <span style={{color: 'rgb(229, 115, 115)'}}>Red</span> = on line<br/>
          Distance = actual distance to segment, Orthoginal distance = perpendicular distance to infinite line.
        </p>
      </div>
    </div>
  )
}

export default PointLineDemo
