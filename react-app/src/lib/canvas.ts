import { useEffect, useRef } from 'react'

export interface CanvasSize {
  width: number
  height: number
  dpr: number
}

/**
 * Keeps a canvas' backing store in sync with its CSS size and device pixel ratio.
 * Drawing code works in CSS pixels; `size.current` always holds the latest size.
 */
export function useCanvas(onResize?: (size: CanvasSize) => void) {
  const ref = useRef<HTMLCanvasElement>(null)
  const size = useRef<CanvasSize>({ width: 0, height: 0, dpr: 1 })
  const onResizeRef = useRef(onResize)
  onResizeRef.current = onResize

  useEffect(() => {
    const canvas = ref.current
    if (!canvas) return
    const observer = new ResizeObserver(() => {
      const rect = canvas.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1
      canvas.width = Math.max(1, Math.round(rect.width * dpr))
      canvas.height = Math.max(1, Math.round(rect.height * dpr))
      size.current = { width: rect.width, height: rect.height, dpr }
      onResizeRef.current?.(size.current)
    })
    observer.observe(canvas)
    return () => observer.disconnect()
  }, [])

  return { ref, size }
}

/** Runs `frame` on every animation frame until unmount. */
export function useAnimationFrame(frame: (time: number, dt: number) => void, active = true) {
  const frameRef = useRef(frame)
  frameRef.current = frame

  useEffect(() => {
    if (!active) return
    let id = 0
    let last = 0
    const tick = (time: number) => {
      const dt = last === 0 ? 1 / 60 : (time - last) / 1000
      last = time
      frameRef.current(time, dt)
      id = requestAnimationFrame(tick)
    }
    id = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(id)
  }, [active])
}

export interface Point {
  x: number
  y: number
}

/**
 * Pointer dragging of handles on a canvas. `pick` returns the handle under the
 * pointer (or null); `move` receives the dragged handle and the pointer delta.
 */
export function useDrag<H>(
  canvas: React.RefObject<HTMLCanvasElement | null>,
  toLocal: (p: Point) => Point,
  pick: (p: Point) => H | null,
  move: (handle: H, delta: Point) => void,
) {
  const handlers = useRef({ toLocal, pick, move })
  handlers.current = { toLocal, pick, move }

  useEffect(() => {
    const el = canvas.current
    if (!el) return
    let dragging: { handle: H; origin: Point } | null = null

    const local = (e: PointerEvent): Point => {
      const rect = el.getBoundingClientRect()
      return handlers.current.toLocal({ x: e.clientX - rect.left, y: e.clientY - rect.top })
    }
    const down = (e: PointerEvent) => {
      const p = local(e)
      const handle = handlers.current.pick(p)
      if (handle === null) return
      e.preventDefault()
      el.setPointerCapture(e.pointerId)
      dragging = { handle, origin: p }
      el.style.cursor = 'grabbing'
    }
    const moveHandler = (e: PointerEvent) => {
      const p = local(e)
      if (!dragging) {
        el.style.cursor = handlers.current.pick(p) === null ? 'crosshair' : 'grab'
        return
      }
      const delta = { x: p.x - dragging.origin.x, y: p.y - dragging.origin.y }
      dragging.origin = p
      handlers.current.move(dragging.handle, delta)
    }
    const up = (e: PointerEvent) => {
      if (!dragging) return
      dragging = null
      el.style.cursor = handlers.current.pick(local(e)) === null ? 'crosshair' : 'grab'
    }

    el.addEventListener('pointerdown', down)
    el.addEventListener('pointermove', moveHandler)
    el.addEventListener('pointerup', up)
    el.addEventListener('pointercancel', up)
    return () => {
      el.removeEventListener('pointerdown', down)
      el.removeEventListener('pointermove', moveHandler)
      el.removeEventListener('pointerup', up)
      el.removeEventListener('pointercancel', up)
    }
  }, [canvas])
}
