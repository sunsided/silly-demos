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
  export namespace VoronoiTests {
    function voronoi_create_points(count: number, width: number, height: number, seed: number, speed: number): Float32Array | number[]
    function voronoi_step_points(points: Float32Array | number[], width: number, height: number, dt: number): Float32Array | number[]
    function delaunay_indices(points: Float32Array | number[]): Uint32Array | number[]
    function voronoi_edges(points: Float32Array | number[]): Float32Array | number[]
  }
}

declare module "silly_demos" {
  const mod: any;
  export = mod;
}

// Extend HTMLCanvasElement to include custom properties
declare global {
  interface HTMLCanvasElement {
    _displayWidth?: number
    _displayHeight?: number
  }
}
