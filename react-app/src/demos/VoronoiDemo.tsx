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

// Geometry helpers
function pointInTriangle(px: number, py: number, ax: number, ay: number, bx: number, by: number, cx: number, cy: number): boolean {
  // Using barycentric technique with same-side test
  const v0x = cx - ax, v0y = cy - ay
  const v1x = bx - ax, v1y = by - ay
  const v2x = px - ax, v2y = py - ay
  const dot00 = v0x * v0x + v0y * v0y
  const dot01 = v0x * v1x + v0y * v1y
  const dot02 = v0x * v2x + v0y * v2y
  const dot11 = v1x * v1x + v1y * v1y
  const dot12 = v1x * v2x + v1y * v2y
  const denom = dot00 * dot11 - dot01 * dot01
  if (Math.abs(denom) < 1e-8) return false
  const inv = 1 / denom
  const u = (dot11 * dot02 - dot01 * dot12) * inv
  const v = (dot00 * dot12 - dot01 * dot02) * inv
  return u >= -1e-6 && v >= -1e-6 && (u + v) <= 1 + 1e-6
}

function circumcenter(ax: number, ay: number, bx: number, by: number, cx: number, cy: number): [number, number] | null {
  // Robust-ish circumcenter; returns null for near-collinear
  const d = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by))
  if (Math.abs(d) < 1e-8) return null
  const a2 = ax * ax + ay * ay
  const b2 = bx * bx + by * by
  const c2 = cx * cx + cy * cy
  const ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d
  const uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d
  return [ux, uy]
}

export default function VoronoiDemo() {
    const canvasRef = useRef<HTMLCanvasElement | null>(null)
    const animRef = useRef<number>(0)
    const lastTimeRef = useRef<number>(0)
    const [wasmReady, setWasmReady] = useState(false)

    const [mode, setMode] = useState<Mode>('triangulation')
    const [paused, setPaused] = useState(false)
    const [seed, setSeed] = useState(1337)
    const [count, setCount] = useState(100)
    const [speed, setSpeed] = useState(5)

    // simulation buffer
    const pointsRef = useRef<Float32Array>(new Float32Array())

    // mouse tracking (CSS pixel coordinates in canvas local space)
    const mouseRef = useRef<{ x: number; y: number; inside: boolean }>({x: 0, y: 0, inside: false})

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
        return () => {
            cancelled = true
        }
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

        // Mouse listeners
        function onMove(e: MouseEvent) {
            const c = canvasRef.current
            if (!c) return
            const rect = c.getBoundingClientRect()
            mouseRef.current.x = e.clientX - rect.left
            mouseRef.current.y = e.clientY - rect.top
            mouseRef.current.inside = true
        }

        function onLeave() {
            mouseRef.current.inside = false
        }

        outerCanvas.addEventListener('mousemove', onMove)
        outerCanvas.addEventListener('mouseleave', onLeave)

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

        function drawTriangulation(pts: Float32Array, indices?: Uint32Array) {
            const idxs = indices ?? (silly_demos.VoronoiTests?.delaunay_indices?.(pts) as (Uint32Array | number[] | undefined))
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
                ctx.moveTo(x0, y0);
                ctx.lineTo(x1, y1)
                ctx.moveTo(x1, y1);
                ctx.lineTo(x2, y2)
                ctx.moveTo(x2, y2);
                ctx.lineTo(x0, y0)
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

            const pts = pointsRef.current
            // Precompute delaunay indices if needed for highlighting
            let I: Uint32Array | undefined
            if (mouseRef.current.inside) {
                const idxs = silly_demos.VoronoiTests?.delaunay_indices?.(pts) as (Uint32Array | number[] | undefined)
                if (idxs) I = idxs instanceof Uint32Array ? idxs : new Uint32Array(idxs)
            }

            // Render base
            if (mode === 'triangulation') drawTriangulation(pts, I)
            else drawVoronoi(pts)

            // Overlay highlight if mouse is inside
            if (mouseRef.current.inside && pts.length >= STRIDE * 3) {
                const mx = mouseRef.current.x
                const my = mouseRef.current.y
                if (mode === 'triangulation') {
                    if (!I) {
                        const idxs = silly_demos.VoronoiTests?.delaunay_indices?.(pts) as (Uint32Array | number[] | undefined)
                        if (idxs) I = idxs instanceof Uint32Array ? idxs : new Uint32Array(idxs)
                    }
                    if (I) {
                        // find the first triangle containing mouse
                        for (let i = 0; i + 2 < I.length; i += 3) {
                            const a = I[i] * STRIDE
                            const b = I[i + 1] * STRIDE
                            const cidx = I[i + 2] * STRIDE
                            const ax = pts[a], ay = pts[a + 1]
                            const bx = pts[b], by = pts[b + 1]
                            const cx = pts[cidx], cy = pts[cidx + 1]
                            if (pointInTriangle(mx, my, ax, ay, bx, by, cx, cy)) {
                                ctx.save()
                                ctx.beginPath()
                                ctx.moveTo(ax, ay)
                                ctx.lineTo(bx, by)
                                ctx.lineTo(cx, cy)
                                ctx.closePath()
                                ctx.fillStyle = 'rgba(255, 215, 0, 0.18)'
                                ctx.strokeStyle = '#ffd700'
                                ctx.lineWidth = 2
                                ctx.fill()
                                ctx.stroke()
                                ctx.restore()
                                break
                            }
                        }
                    }
                } else {
                    // Voronoi mode: highlight cell for nearest site
                    let bestIdx = -1
                    let bestD2 = Infinity
                    for (let i = 0; i < pts.length; i += STRIDE) {
                        const dx = pts[i] - mx
                        const dy = pts[i + 1] - my
                        const d2 = dx * dx + dy * dy
                        if (d2 < bestD2) {
                            bestD2 = d2;
                            bestIdx = i / STRIDE | 0
                        }
                    }
                    if (bestIdx >= 0) {
                        if (!I) {
                            const idxs = silly_demos.VoronoiTests?.delaunay_indices?.(pts) as (Uint32Array | number[] | undefined)
                            if (idxs) I = idxs instanceof Uint32Array ? idxs : new Uint32Array(idxs)
                        }
                        if (I) {
                            const cxys: [number, number][] = []
                            for (let i = 0; i + 2 < I.length; i += 3) {
                                const ia = I[i], ib = I[i + 1], ic = I[i + 2]
                                if (ia === bestIdx || ib === bestIdx || ic === bestIdx) {
                                    const a = ia * STRIDE, b = ib * STRIDE, c3 = ic * STRIDE
                                    const cc = circumcenter(pts[a], pts[a + 1], pts[b], pts[b + 1], pts[c3], pts[c3 + 1])
                                    if (cc) cxys.push(cc)
                                }
                            }
                            // sort circumcenters around the site
                            const sx = pts[bestIdx * STRIDE]
                            const sy = pts[bestIdx * STRIDE + 1]
                            cxys.sort((p, q) => Math.atan2(p[1] - sy, p[0] - sx) - Math.atan2(q[1] - sy, q[0] - sx))
                            if (cxys.length >= 2) {
                                ctx.save()
                                ctx.beginPath()
                                ctx.moveTo(cxys[0][0], cxys[0][1])
                                for (let k = 1; k < cxys.length; k++) ctx.lineTo(cxys[k][0], cxys[k][1])
                                ctx.closePath()
                                ctx.fillStyle = 'rgba(102, 187, 255, 0.15)'
                                ctx.strokeStyle = '#4af'
                                ctx.lineWidth = 2
                                ctx.fill()
                                ctx.stroke()
                                // also emphasize the site point
                                ctx.beginPath()
                                ctx.fillStyle = '#fff'
                                ctx.strokeStyle = '#4af'
                                ctx.lineWidth = 2
                                ctx.arc(sx, sy, 3, 0, Math.PI * 2)
                                ctx.fill()
                                ctx.stroke()
                                ctx.restore()
                            }
                        }
                    }
                }
            }

            animRef.current = requestAnimationFrame(frame)
        }

        animRef.current = requestAnimationFrame(frame)
        return () => {
            cancelAnimationFrame(animRef.current)
            outerCanvas.removeEventListener('mousemove', onMove)
            outerCanvas.removeEventListener('mouseleave', onLeave)
        }
    }, [wasmReady, mode, paused])

    // UI
    return (
        <div className="demo-container">
            <div className="controls glass-panel" style={{marginBottom: 12}}>
                <div className="row" style={{display: 'flex', gap: 12, flexWrap: 'wrap'}}>
                    <button className="control-button"
                            onClick={() => setMode(m => m === 'triangulation' ? 'voronoi' : 'triangulation')}>
                        Mode: {mode === 'triangulation' ? 'Delaunay (triangulation)' : 'Voronoi'}
                    </button>
                    <button className="control-button" onClick={() => setPaused(p => !p)}>
                        {paused ? 'Resume' : 'Pause'}
                    </button>
                    <label className="pill">Seed
                        <input type="number" value={seed} onChange={e => setSeed(parseInt(e.target.value || '0', 10))}
                               style={{marginLeft: 8, width: 100}}/>
                    </label>
                    <label className="pill">Moving Points
                        <input type="number" min={3} max={5000} value={count}
                               onChange={e => setCount(Math.max(3, Math.min(5000, parseInt(e.target.value || '0', 10))))}
                               style={{marginLeft: 8, width: 100}}/>
                    </label>
                    <label className="pill">Speed
                        <input type="number" min={1} max={200} value={speed}
                               onChange={e => setSpeed(Math.max(1, Math.min(200, parseInt(e.target.value || '0', 10))))}
                               style={{marginLeft: 8, width: 100}}/>
                    </label>
                    <button className="control-button" onClick={() => {
                        // Recreate points explicitly
                        const canvas = canvasRef.current
                        if (!canvas) return
                        const width = (canvas as any)._displayWidth || canvas.clientWidth || 800
                        const height = (canvas as any)._displayHeight || canvas.clientHeight || 600
                        const out = silly_demos.VoronoiTests?.voronoi_create_points?.(count, width, height, seed >>> 0, speed) || []
                        pointsRef.current = toF32(out)
                    }}>Recreate
                    </button>
                </div>
            </div>

            <div className="canvas-wrapper glass-panel" style={{height: 600}}>
                <canvas ref={canvasRef} style={{width: '100%', height: '100%', display: 'block'}}/>
            </div>

            {!wasmReady && (
                <div className="glass-panel" style={{marginTop: 12}}>
                    <em>Loading WASM…</em>
                </div>
            )}
        </div>
    )
}
