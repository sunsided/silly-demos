import type { Attachment } from 'svelte/attachments'

export interface Point {
  x: number
  y: number
}

export interface CanvasSize {
  width: number
  height: number
  dpr: number
}

/**
 * A canvas whose backing store follows its CSS size and device pixel ratio.
 * Drawing code works in CSS pixels. Attach with `{@attach canvas.attach}`.
 *
 * `size`, `pointer` and `element` are plain fields, not reactive state: the
 * frame loop reads them every frame and nothing in the markup depends on them.
 */
export class DrawingCanvas {
  element: HTMLCanvasElement | null = null
  size: CanvasSize = { width: 0, height: 0, dpr: 1 }
  /** Pointer position in CSS pixels relative to the canvas, while it hovers. */
  pointer: Point | null = null

  constructor(private onResize?: (size: CanvasSize) => void) {}

  attach: Attachment<HTMLCanvasElement> = canvas => {
    this.element = canvas
    const observer = new ResizeObserver(() => {
      const rect = canvas.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1
      canvas.width = Math.max(1, Math.round(rect.width * dpr))
      canvas.height = Math.max(1, Math.round(rect.height * dpr))
      this.size = { width: rect.width, height: rect.height, dpr }
      this.onResize?.(this.size)
    })
    observer.observe(canvas)

    const move = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect()
      this.pointer = { x: e.clientX - rect.left, y: e.clientY - rect.top }
    }
    const leave = () => {
      this.pointer = null
    }
    canvas.addEventListener('pointermove', move)
    canvas.addEventListener('pointerleave', leave)
    return () => {
      observer.disconnect()
      canvas.removeEventListener('pointermove', move)
      canvas.removeEventListener('pointerleave', leave)
      this.element = null
    }
  }

  /** The 2D context, once mounted and laid out. */
  context(): CanvasRenderingContext2D | null {
    if (!this.element || this.size.width === 0) return null
    return this.element.getContext('2d')
  }

  /** Converts canvas coordinates to coordinates centered on the canvas. */
  centered = (p: Point): Point => ({ x: p.x - this.size.width / 2, y: p.y - this.size.height / 2 })
}

/**
 * Calls `frame(time, dt)` on every animation frame while `active()` is true.
 * Call during component initialization.
 */
export function onFrame(frame: (time: number, dt: number) => void, active: () => boolean = () => true) {
  $effect(() => {
    if (!active()) return
    let id = 0
    let last = 0
    const tick = (time: number) => {
      const dt = last === 0 ? 1 / 60 : (time - last) / 1000
      last = time
      frame(time, dt)
      id = requestAnimationFrame(tick)
    }
    id = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(id)
  })
}

/**
 * Pointer dragging of handles. `pick` returns the handle under the pointer
 * (or null); `move` receives the dragged handle and the pointer delta.
 * Positions are mapped through `toLocal` first.
 */
export function draggable<H>(options: {
  toLocal: (p: Point) => Point
  pick: (p: Point) => H | null
  move: (handle: H, delta: Point) => void
}): Attachment<HTMLCanvasElement> {
  return canvas => {
    let dragging: { handle: H; origin: Point } | null = null

    const local = (e: PointerEvent): Point => {
      const rect = canvas.getBoundingClientRect()
      return options.toLocal({ x: e.clientX - rect.left, y: e.clientY - rect.top })
    }
    const cursor = (p: Point) => {
      canvas.style.cursor = options.pick(p) === null ? 'crosshair' : 'grab'
    }
    const down = (e: PointerEvent) => {
      const p = local(e)
      const handle = options.pick(p)
      if (handle === null) return
      e.preventDefault()
      canvas.setPointerCapture(e.pointerId)
      dragging = { handle, origin: p }
      canvas.style.cursor = 'grabbing'
    }
    const move = (e: PointerEvent) => {
      const p = local(e)
      if (!dragging) return cursor(p)
      const delta = { x: p.x - dragging.origin.x, y: p.y - dragging.origin.y }
      dragging.origin = p
      options.move(dragging.handle, delta)
    }
    const up = (e: PointerEvent) => {
      if (!dragging) return
      dragging = null
      cursor(local(e))
    }

    canvas.addEventListener('pointerdown', down)
    canvas.addEventListener('pointermove', move)
    canvas.addEventListener('pointerup', up)
    canvas.addEventListener('pointercancel', up)
    return () => {
      canvas.removeEventListener('pointerdown', down)
      canvas.removeEventListener('pointermove', move)
      canvas.removeEventListener('pointerup', up)
      canvas.removeEventListener('pointercancel', up)
    }
  }
}
