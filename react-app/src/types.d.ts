/// <reference types="vite/client" />

// TypeScript declarations for the WASM module
declare module '../pkg/silly_demos.js' {
  export default function init(): Promise<void>
  export function run(): Promise<void>
  export function circle_collision(
    x1: number, y1: number, r1: number, 
    x2: number, y2: number, r2: number
  ): {
    intersect: boolean
    distance: number
    penetration: number
    dx: number
    dy: number
  }
}

// Extend HTMLCanvasElement to include custom properties
declare global {
  interface HTMLCanvasElement {
    _displayWidth?: number
    _displayHeight?: number
  }
}
