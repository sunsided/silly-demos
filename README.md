<div align="center">

  <h1><code>silly-demos</code></h1>

<strong>Small interactive geometry and simulation demos in Rust/WebAssembly, drawn as engineering sketches in the browser</strong>

  <p>
    <img src="https://img.shields.io/badge/rust-1.70+-orange.svg?style=flat-square" alt="Rust Version" />
    <img src="https://img.shields.io/badge/wasm--bindgen-0.2-blue.svg?style=flat-square" alt="wasm-bindgen" />
    <img src="https://img.shields.io/badge/react-19.3-61dafb.svg?style=flat-square" alt="React" />
    <img src="https://img.shields.io/badge/vite-8.3-646cff.svg?style=flat-square" alt="Vite" />
    <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg?style=flat-square" alt="License" />
  </p>

</div>

## About

A growing collection of small geometry, collision and simulation sketches. The math runs in Rust compiled to
WebAssembly; a React/Vite frontend draws each demo as an annotated drafting sheet with live dimensions and readouts.

See the demos at <https://sunsided.github.io/silly-demos/>

### Features

- **Circle-Circle Collision Detection**: Fast collision detection with distance and penetration calculations
- **Point-Line Distance Demo**: Visualizes the shortest distance from a point to a line segment
- **Boids Simulation**: Reynolds flocking (separation, alignment, cohesion) with field of view, look-ahead edge avoidance and a spatial grid for neighbor lookups; the flock state lives in WASM
- **Voronoi / Delaunay**: Interactive Voronoi diagram / Delaunay triangulation
- **Real-time Visualization**: Interactive demos built with React and HTML5 Canvas
- **High Performance**: Rust WebAssembly core for computational heavy lifting
- **Modern Frontend**: Vite-powered React application with TypeScript support
- **Seamless Rust-JS Interop**: Uses wasm-bindgen for efficient communication between Rust and JavaScript
- **SPA Routing**: Supports client-side routing for multi-page demos on GitHub Pages

## Project Structure

```
silly-demos/
├── src/                   # Rust WebAssembly core
│   ├── lib.rs             # wasm-bindgen exports
│   ├── collision.rs       # Collision tests
│   ├── geometry.rs        # Point/segment geometry
│   ├── boids.rs           # Flocking simulation
│   ├── voronoi.rs         # Delaunay triangulation / Voronoi edges
│   └── vec2.rs            # 2D vector math
├── pkg/                   # Generated WebAssembly package (wasm-pack)
├── react-app/             # React frontend application
│   ├── src/
│   │   ├── components/    # Sheet layout, shared canvases
│   │   ├── demos/         # Interactive demos
│   │   └── lib/           # Canvas, drafting, theme and WASM helpers
│   └── vite.config.ts     # Vite configuration
├── Cargo.toml             # Rust crate manifest
└── Taskfile.dist.yaml     # Task runner entry point
```

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.70 or later)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (20.19 or later)
- [Yarn](https://classic.yarnpkg.com/) (classic, e.g. via `corepack enable`)
- [Task](https://taskfile.dev/installation/) runner

### Quick Start

Build the WASM package, install frontend dependencies and start the dev server:

```bash
task dev
```

The application will be available at `http://localhost:3000`.

### Building for Production

```bash
# WASM package (pkg/) and frontend bundle (react-app/dist/)
task build

# Build and serve the production bundle
task preview
```

## Current Status

- **Automated Deployment**: Every push to `main` triggers a GitHub Actions workflow that builds the Rust WASM core, installs frontend dependencies, builds the React app with the correct base path, and deploys to GitHub Pages. No manual steps are required for deployment.
- **Base Path Handling**: The React app's base path is set automatically in CI to match the repository name, ensuring assets and routing work correctly on GitHub Pages.
- **Client-Side Routing**: SPA routing is supported on GitHub Pages using a `404.html` redirect solution in `react-app/public/404.html`.
- **Project Structure**: The workspace is organized with a Rust core (`src/`), a React frontend (`react-app/`), and the generated WASM package (`pkg/`).
- **Development & Build**: Use the provided Task commands and scripts for local development, building, and testing. The workflow and scripts are up-to-date and stable.
- **Actively Maintained**: The project is maintained and the deployment pipeline is stable and reproducible.

## GitHub Pages Deployment

The project uses GitHub Actions for fully automated deployment:

- On every push to `main`, the workflow builds the Rust WASM, installs frontend dependencies, builds the React app with the correct base path, and deploys to GitHub Pages.
- The base path (`VITE_BASE_PATH`) is set automatically in CI to `/${{ github.event.repository.name }}`.
- For local builds, you can set `VITE_BASE_PATH` manually if needed.
- SPA routing is handled by `react-app/public/404.html` for compatibility with GitHub Pages.

See `.github/workflows/deploy.yml` for details.

**Configuration**: The base path is automatically set to the repository name in CI. For manual builds, set the `VITE_BASE_PATH` environment variable:

```bash
# Custom base path
export VITE_BASE_PATH=/my-custom-path
./build-github-pages.sh
```

The application will be available at: `https://your-username.github.io/[repository-name]/`

**Automatic Deployment**: Push to the main branch triggers automatic build and deployment via GitHub Actions, with the base path automatically configured to match the repository name.

**Client-Side Routing**: The app uses a 404.html redirect solution to handle direct navigation to React Router routes on GitHub Pages, ensuring URLs like `/circle-collision` work correctly when accessed directly.

## Available Demos

- **Boids**: Flocking simulation with live controls for every rule and a view of what a single boid perceives
- **Circle Collision**: Drag two circles; dimensions show center distance, gap and overlap
- **Point to Segment**: Shortest distance from a point to a line segment versus the perpendicular distance to the infinite line

## Technology Stack

### Rust WebAssembly Core

- **wasm-bindgen**: Seamless interop between Rust and JavaScript
- **console_error_panic_hook**: Enhanced error logging for development

### React Frontend

- **Vite**: Fast build tool and development server
- **TypeScript**: Type-safe JavaScript development
- **React Router**: Client-side routing for multi-page demos
- **HTML5 Canvas**: High-performance 2D graphics rendering

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

`task dev` hot reloads React changes. After changing Rust code, run `task build:wasm`;
Vite picks up the rebuilt `pkg/`.

### Available Tasks

Run `task` to list everything. Frontend tasks live in `react-app/Taskfile.dist.yaml` and are
available from the root with a `web:` prefix (e.g. `task web:check`).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE_APACHE](LICENSE_APACHE))
- MIT License ([LICENSE_MIT](LICENSE_MIT))

at your option.
