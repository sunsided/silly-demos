import { useEffect, useRef, useState } from 'react'
import init, * as silly_demos from 'silly_demos'

// Each point is [x, y, vx, vy]
const STRIDE = 4

type Mode = 'triangulation' | 'voronoi'

function toF32(arr: Float32Array | number[]): Float32Array {
    return arr instanceof Float32Array ? arr : new Float32Array(arr)
}

function useHiDPICanvas(canvasRef: React.RefObject<HTMLCanvasElement | null>) {
  useEffect(() => {
    const initial = canvasRef.current
    if (!initial) return

    function resize() {
      const c = canvasRef.current
      if (!c) return
      const dpr = Math.max(1, Math.min(window.devicePixelRatio || 1, 2))
      const rect = c.getBoundingClientRect()
      const w = Math.max(1, Math.floor(rect.width))
      const h = Math.max(1, Math.floor(rect.height))
      if (c.width !== Math.floor(w * dpr) || c.height !== Math.floor(h * dpr)) {
        c.width = Math.floor(w * dpr)
        c.height = Math.floor(h * dpr)
        const ctx = c.getContext('2d')!
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      }
      ;(c as any)._displayWidth = w
      ;(c as any)._displayHeight = h
    }

    resize()
    const ro = new ResizeObserver(() => resize())
    const observed = canvasRef.current
    if (observed) ro.observe(observed)
    window.addEventListener('resize', resize)
    return () => {
      ro.disconnect()
      window.removeEventListener('resize', resize)
    }
  }, [canvasRef])
}

export default function VoronoiDemo() {
    const canvasRef = useRef<HTMLCanvasElement | null>(null)
    const animRef = useRef<number>(0)
    const lastTimeRef = useRef<number>(0)
    const [wasmReady, setWasmReady] = useState(false)

    const [mode, setMode] = useState<Mode>('triangulation')
    const [paused, setPaused] = useState(false)
    const [seed, setSeed] = useState(1337)
    const [count, setCount] = useState(300)
    const [speed, setSpeed] = useState(35)

    // simulation buffer
    const pointsRef = useRef<Float32Array>(new Float32Array())

    useHiDPICanvas(canvasRef)

    // Init WASM
    useEffect(() => {
        let cancelled = false
        ;(async () => {
            try {
                await init()
                if (!cancelled) setWasmReady(true)
            } catch (e) {
                console.error('WASM init failed', e)
            }
        })()
        return () => { cancelled = true }
    }, [])

    // (Re)create points upon param changes
    useEffect(() => {
        if (!wasmReady) return
        const canvas = canvasRef.current
        if (!canvas) return
        const width = (canvas as any)._displayWidth || canvas.clientWidth || 800
        const height = (canvas as any)._displayHeight || canvas.clientHeight || 600

        const out = silly_demos.VoronoiTests?.voronoi_create_points?.(count, width, height, seed >>> 0, speed) || []
        pointsRef.current = toF32(out)
    }, [wasmReady, count, seed, speed])

    // Animation loop
    useEffect(() => {
        if (!wasmReady) return
        const outerCanvas = canvasRef.current
        if (!outerCanvas) return
        const ctx = outerCanvas.getContext('2d')!

        function drawPoints(pts: Float32Array) {
            ctx.fillStyle = '#ffffff'
            const r = 2.2
            for (let i = 0; i < pts.length; i += STRIDE) {
                const x = pts[i]
                const y = pts[i + 1]
                ctx.beginPath()
                ctx.arc(x, y, r, 0, Math.PI * 2)
                ctx.fill()
            }
        }

        function drawTriangulation(pts: Float32Array) {
            const idxs = silly_demos.VoronoiTests?.delaunay_indices?.(pts) as (Uint32Array | number[] | undefined)
            if (!idxs) return
            const I = idxs instanceof Uint32Array ? idxs : new Uint32Array(idxs)
            ctx.strokeStyle = '#66ccff'
            ctx.lineWidth = 1
            ctx.beginPath()
            for (let i = 0; i + 2 < I.length; i += 3) {
                const i0 = I[i] * STRIDE
                const i1 = I[i + 1] * STRIDE
                const i2 = I[i + 2] * STRIDE
                const x0 = pts[i0], y0 = pts[i0 + 1]
                const x1 = pts[i1], y1 = pts[i1 + 1]
                const x2 = pts[i2], y2 = pts[i2 + 1]
                ctx.moveTo(x0, y0); ctx.lineTo(x1, y1)
                ctx.moveTo(x1, y1); ctx.lineTo(x2, y2)
                ctx.moveTo(x2, y2); ctx.lineTo(x0, y0)
            }
            ctx.stroke()
            drawPoints(pts)
        }

        function drawVoronoi(pts: Float32Array) {
            const segs = silly_demos.VoronoiTests?.voronoi_edges?.(pts) as (Float32Array | number[] | undefined)
            if (!segs) return
            const S = segs instanceof Float32Array ? segs : new Float32Array(segs)
            ctx.strokeStyle = '#ffaa66'
            ctx.lineWidth = 1
            ctx.beginPath()
            for (let i = 0; i + 3 < S.length; i += 4) {
                const x1 = S[i], y1 = S[i + 1]
                const x2 = S[i + 2], y2 = S[i + 3]
                ctx.moveTo(x1, y1)
                ctx.lineTo(x2, y2)
            }
            ctx.stroke()
            drawPoints(pts)
        }

        function frame(ts: number) {
      const c = canvasRef.current
      if (!c) {
        animRef.current = requestAnimationFrame(frame)
        return
      }
      const width = (c as any)._displayWidth || c.clientWidth || 800
      const height = (c as any)._displayHeight || c.clientHeight || 600
            const dt = lastTimeRef.current ? Math.min(0.05, (ts - lastTimeRef.current) / 1000) : 0
            lastTimeRef.current = ts

            // Step
            if (!paused && pointsRef.current.length > 0) {
                const next = silly_demos.VoronoiTests?.voronoi_step_points?.(pointsRef.current, width, height, dt) || pointsRef.current
                pointsRef.current = toF32(next)
            }

            // Clear
            ctx.clearRect(0, 0, width, height)

            // Render
            if (mode === 'triangulation') drawTriangulation(pointsRef.current)
            else drawVoronoi(pointsRef.current)

            animRef.current = requestAnimationFrame(frame)
        }

        animRef.current = requestAnimationFrame(frame)
        return () => cancelAnimationFrame(animRef.current)
  }, [wasmReady, mode, paused])

    // UI
    return (
        <div className="demo-container">
            <div className="controls glass-panel" style={{ marginBottom: 12 }}>
                <div className="row" style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
                    <button className="control-button" onClick={() => setMode(m => m === 'triangulation' ? 'voronoi' : 'triangulation')}>
                        Mode: {mode === 'triangulation' ? 'Delaunay (triangulation)' : 'Voronoi'}
                    </button>
                    <button className="control-button" onClick={() => setPaused(p => !p)}>
                        {paused ? 'Resume' : 'Pause'}
                    </button>
                    <label className="pill">Seed
                        <input type="number" value={seed} onChange={e => setSeed(parseInt(e.target.value || '0', 10))} style={{ marginLeft: 8, width: 100 }} />
                    </label>
                    <label className="pill">Points
                        <input type="number" min={3} max={5000} value={count} onChange={e => setCount(Math.max(3, Math.min(5000, parseInt(e.target.value || '0', 10))))} style={{ marginLeft: 8, width: 100 }} />
                    </label>
                    <label className="pill">Speed
                        <input type="number" min={1} max={200} value={speed} onChange={e => setSpeed(Math.max(1, Math.min(200, parseInt(e.target.value || '0', 10))))} style={{ marginLeft: 8, width: 100 }} />
                    </label>
                    <button className="control-button" onClick={() => {
                        // Recreate points explicitly
                        const canvas = canvasRef.current
                        if (!canvas) return
                        const width = (canvas as any)._displayWidth || canvas.clientWidth || 800
                        const height = (canvas as any)._displayHeight || canvas.clientHeight || 600
                        const out = silly_demos.VoronoiTests?.voronoi_create_points?.(count, width, height, seed >>> 0, speed) || []
                        pointsRef.current = toF32(out)
                    }}>Recreate</button>
                </div>
            </div>

            <div className="canvas-wrapper glass-panel" style={{ height: 600 }}>
                <canvas ref={canvasRef} style={{ width: '100%', height: '100%', display: 'block' }} />
            </div>

            {!wasmReady && (
                <div className="glass-panel" style={{ marginTop: 12 }}>
                    <em>Loading WASM…</em>
                </div>
            )}
        </div>
    )
}
