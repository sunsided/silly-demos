<div align="center">

  <h1><code>silly-demos</code></h1>

<strong>Small interactive geometry and simulation demos in Rust/WebAssembly, drawn as engineering sketches in the browser</strong>

  <p>
    <img src="https://img.shields.io/badge/rust-1.70+-orange.svg?style=flat-square" alt="Rust Version" />
    <img src="https://img.shields.io/badge/wasm--bindgen-0.2-blue.svg?style=flat-square" alt="wasm-bindgen" />
    <img src="https://img.shields.io/badge/sveltekit-3.1-ff3e00.svg?style=flat-square" alt="SvelteKit" />
    <img src="https://img.shields.io/badge/svelte-5-ff3e00.svg?style=flat-square" alt="Svelte" />
    <img src="https://img.shields.io/badge/vite-8.3-646cff.svg?style=flat-square" alt="Vite" />
    <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0%20OR%20EUPL--1.2-green.svg?style=flat-square" alt="License" />
  </p>

</div>

## About

A growing collection of small geometry, collision and simulation sketches. The math runs in Rust compiled to
WebAssembly; a SvelteKit frontend, prerendered to static pages, draws each demo as an annotated drafting sheet with
live dimensions and readouts.

See the demos at <https://sunsided.github.io/silly-demos/>

### Features

- **Collision Lab**: Pick two shapes (circle, boxes, triangle, convex polygon, ellipse, capsule) and switch between the methods that can test them, from pair-specific shortcuts to SAT and GJK/EPA, each with its own construction drawing, operation counts and timing; a quadtree over a drifting crowd of shapes shows scene-wide pair pruning against brute force
- **Inverse Kinematics**: A planar arm reaching for a draggable or animated target, solved by FABRIK, CCD, Jacobian transpose or damped least squares, with the remaining error and iterations per frame; optional limits keep the links from crossing and the arm above the floor, rerouting it when it gets stuck
- **Point-Line Distance Demo**: Visualizes the shortest distance from a point to a line segment
- **Boids Simulation**: Reynolds flocking (separation, alignment, cohesion) with field of view, look-ahead edge avoidance and a spatial grid for neighbor lookups; the flock state lives in WASM
- **Voronoi / Delaunay**: Interactive Voronoi diagram / Delaunay triangulation
- **Real-time Visualization**: Interactive demos drawn on HTML5 Canvas from Svelte 5 components
- **High Performance**: Rust WebAssembly core for computational heavy lifting
- **Static Site**: SvelteKit 3 with `adapter-static` prerenders every page; the WASM module loads after hydration
- **Design Tokens**: Colors, type, spacing and sizes defined once in W3C Design Tokens format and generated into CSS custom properties and a typed module the canvas code reads
- **Atomic Design**: Components organised as atoms, molecules, organisms and templates; routes are the pages
- **Seamless Rust-JS Interop**: Uses wasm-bindgen for efficient communication between Rust and JavaScript

## Project Structure

```
silly-demos/
├── src/                   # Rust WebAssembly core
│   ├── lib.rs             # wasm-bindgen exports
│   ├── collision/         # Shapes, collision methods, broadphase, method registry
│   ├── geometry.rs        # Point/segment geometry
│   ├── ik.rs              # Inverse kinematics solvers
│   ├── boids.rs           # Flocking simulation
│   ├── voronoi.rs         # Delaunay triangulation / Voronoi edges
│   └── vec2.rs            # 2D vector math
├── pkg/                   # Generated WebAssembly package (wasm-pack)
├── web/                   # SvelteKit frontend (static site)
│   ├── tokens/tokens.json # Design tokens (W3C DTCG format)
│   ├── scripts/           # Token generator
│   ├── src/
│   │   ├── routes/        # Pages: home and one route per demo
│   │   └── lib/
│   │       ├── components/  # atoms/, molecules/, organisms/, templates/
│   │       ├── demos/       # Per-demo drawing code and state
│   │       ├── canvas/      # Canvas sizing, dragging, drafting helpers, theme
│   │       ├── styles/      # Global base styles
│   │       └── wasm/        # WASM loader
│   └── vite.config.ts     # SvelteKit and Vite configuration
├── Cargo.toml             # Rust crate manifest
└── Taskfile.dist.yaml     # Task runner entry point
```

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.70 or later)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (22.17 or later)
- [Yarn](https://yarnpkg.com/) 4, via `corepack enable` (pinned in `web/package.json`; on Node 25+, install corepack first with `npm i -g corepack`)
- [Task](https://taskfile.dev/installation/) runner

### Quick Start

Build the WASM package, install frontend dependencies and start the dev server:

```bash
task dev
```

The application will be available at `http://localhost:3000`.

### Building for Production

```bash
# WASM package (pkg/) and prerendered site (web/build/)
task build

# Build and serve the production site
task preview
```

## GitHub Pages Deployment

Every push to `main` runs `.github/workflows/deploy.yml`: it builds the WASM package, installs the frontend with
Yarn 4, typechecks, prerenders the site with `BASE_PATH` set to `/<repository name>`, and deploys `web/build/` to
GitHub Pages. Each route is its own static HTML file, so direct links such as `/collision?a=obb&b=triangle` work
without a single-page-app redirect; `404.html` is SvelteKit's fallback for unknown paths.

For a manual build under a sub-path, set `BASE_PATH`:

```bash
BASE_PATH=/my-custom-path task build
```

## Available Demos

- **Boids**: Flocking simulation with live controls for every rule and a view of what a single boid perceives
- **Collision Lab**: Two shapes, every applicable collision method, its construction and its cost
- **Inverse Kinematics**: FABRIK, CCD and Jacobian solvers moving a jointed arm onto a target
- **Voronoi / Delaunay**: Moving points with their Delaunay triangulation and Voronoi diagram
- **Point to Segment**: Shortest distance from a point to a line segment versus the perpendicular distance to the infinite line

## Technology Stack

### Rust WebAssembly Core

- **wasm-bindgen**: Seamless interop between Rust and JavaScript
- **console_error_panic_hook**: Enhanced error logging for development

### Web Frontend

- **SvelteKit 3** with **Svelte 5** runes, prerendered by `@sveltejs/adapter-static`
- **Vite 8**: Build tool and development server
- **TypeScript 6**: Type-checked with `svelte-check`
- **Yarn 4** via corepack, with the `node-modules` linker
- **Design tokens**: `web/tokens/tokens.json` → `src/lib/tokens/generated/` (`yarn tokens`, run automatically by
  `dev`, `build` and `check`)
- **HTML5 Canvas**: 2D drawing for every demo

## Development

### Testing

```bash
# Rust unit tests, run natively (the default build target is wasm32)
task test

# wasm-bindgen tests in headless Firefox
task test:wasm

# Everything CI cares about: formatting, clippy, typecheck, tests, build
task ci
```

### Hot Reloading

`task dev` hot reloads Svelte changes. After changing Rust code, run `task build:wasm`;
Vite picks up the rebuilt `pkg/`. After editing `web/tokens/tokens.json`, run `task web:tokens` (or restart the dev
server) to regenerate the CSS and TS.

### Available Tasks

Run `task` to list everything. Frontend tasks live in `web/Taskfile.dist.yaml` and are
available from the root with a `web:` prefix (e.g. `task web:check`).

## License

Licensed under any of

- Apache License, Version 2.0 ([LICENSE_APACHE](LICENSE_APACHE))
- MIT License ([LICENSE_MIT](LICENSE_MIT))
- European Union Public Licence, Version 1.2 ([LICENSE_EUPL](LICENSE_EUPL))

at your option.
