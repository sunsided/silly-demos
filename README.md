<div align="center">

  <h1><code>silly-demos</code></h1>

<strong>Interactive physics and collision detection demos built with Rust WebAssembly and React</strong>

  <p>
    <img src="https://img.shields.io/badge/rust-1.70+-orange.svg?style=flat-square" alt="Rust Version" />
    <img src="https://img.shields.io/badge/wasm--bindgen-0.2-blue.svg?style=flat-square" alt="wasm-bindgen" />
    <img src="https://img.shields.io/badge/react-19.2-61dafb.svg?style=flat-square" alt="React" />
    <img src="https://img.shields.io/badge/vite-7.1-646cff.svg?style=flat-square" alt="Vite" />
    <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg?style=flat-square" alt="License" />
  </p>

</div>

## About

This project demonstrates real-time physics simulations by combining the performance of Rust compiled to WebAssembly
with a modern React frontend. The Rust core provides silly little algorithms, while the React/Vite
frontend offers interactive visualizations and demos.

See the demos at <https://sunsided.github.io/silly-demos/>

### Features

- **Circle-Circle Collision Detection**: Fast collision detection with distance and penetration calculations
- **Point-Line Distance Demo**: Visualizes the shortest distance from a point to a line segment
- **Boids Simulation**: Interactive flocking simulation based on classic boids algorithm
- **Real-time Visualization**: Interactive demos built with React and HTML5 Canvas
- **High Performance**: Rust WebAssembly core for computational heavy lifting
- **Modern Frontend**: Vite-powered React application with TypeScript support
- **Seamless Rust-JS Interop**: Uses wasm-bindgen for efficient communication between Rust and JavaScript
- **SPA Routing**: Supports client-side routing for multi-page demos on GitHub Pages

## Project Structure

```
silly-demos/
├── src/                    # Rust WebAssembly core
│   ├── lib.rs             # Main library with collision detection
│   └── utils.rs           # Utility functions
├── pkg/                   # Generated WebAssembly package
├── react-app/             # React frontend application
│   ├── src/
│   │   ├── components/    # React components
│   │   ├── demos/         # Interactive demos
│   │   └── pkg/          # Copied WASM package
│   └── vite.config.ts    # Vite configuration
├── Cargo.toml            # Rust dependencies
└── package.json          # NPM package metadata
```

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.70 or later)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (18 or later)
- [just](https://github.com/casey/just) command runner

### Quick Start

For a full development setup (builds WASM, sets up React app, and starts dev server):

```bash
just react-full-dev
```

### Building the Rust/WASM Core

```bash
# Build the WebAssembly package
just build

# Or build with optimizations for production
just build --release
```

### Running the React Frontend

```bash
# Setup React dependencies (first time only)
just react-setup

# Copy WASM package to React app
just react-copy-wasm

# Start the development server
just react-dev
```

The application will be available at `http://localhost:5173`.

### Building for Production

```bash
# Full production build (WASM + React)
just react-full-build
```

## Current Status

- **Automated Deployment**: Every push to `main` triggers a GitHub Actions workflow that builds the Rust WASM core, installs frontend dependencies, builds the React app with the correct base path, and deploys to GitHub Pages. No manual steps are required for deployment.
- **Base Path Handling**: The React app's base path is set automatically in CI to match the repository name, ensuring assets and routing work correctly on GitHub Pages.
- **Client-Side Routing**: SPA routing is supported on GitHub Pages using a `404.html` redirect solution in `react-app/public/404.html`.
- **Project Structure**: The workspace is organized with a Rust core (`src/`), a React frontend (`react-app/`), and the generated WASM package (`pkg/`).
- **Development & Build**: Use the provided `just` commands and scripts for local development, building, and testing. The workflow and scripts are up-to-date and stable.
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

For detailed deployment instructions, see [GITHUB_PAGES.md](GITHUB_PAGES.md).

## Available Demos

- **Circle Collision Demo**: Interactive visualization of circle-circle collision detection with real-time physics

## Technology Stack

### Rust WebAssembly Core

- **wasm-bindgen**: Seamless interop between Rust and JavaScript
- **console_error_panic_hook**: Enhanced error logging for development
- **wee_alloc**: Memory-efficient allocator optimized for WebAssembly

### React Frontend

- **Vite**: Fast build tool and development server
- **TypeScript**: Type-safe JavaScript development
- **React Router**: Client-side routing for multi-page demos
- **HTML5 Canvas**: High-performance 2D graphics rendering

## Development

### Testing

```bash
# Test the Rust core
cargo test

# Test WebAssembly bindings in headless browsers
just test

# Lint the React frontend
just react-lint
```

### Hot Reloading

The development setup supports hot reloading for both Rust and React code:

1. Run `just react-full-dev` for the complete development setup
2. Changes to React code will hot reload automatically
3. Changes to Rust code require rebuilding with `just build` and copying with `just react-copy-wasm`

### Available Just Commands

Run `just` to see all available commands:

- `just build` - Build WASM package
- `just test` - Run WASM tests
- `just react-setup` - Install React dependencies
- `just react-dev` - Start React development server
- `just react-build` - Build React for production
- `just react-copy-wasm` - Copy WASM package to React app
- `just react-full-dev` - Complete development setup
- `just react-full-build` - Complete production build
- `just react-clean` - Clean React build artifacts

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE_APACHE](LICENSE_APACHE))
- MIT License ([LICENSE_MIT](LICENSE_MIT))

at your option.
