# Silly Demos with WASM - React App

A React application showcasing WebAssembly (WASM) demos built with Rust. Currently features an interactive circle collision detection demo.

## Features

- **Interactive Circle Collision Demo**: Drag circles around and watch real-time collision detection
- **Modern UI**: Glass morphism design with gradient accents
- **Responsive**: Works on desktop and mobile devices
- **TypeScript**: Full type safety
- **Navigation**: Easy switching between different demos

## Getting Started

### Prerequisites

- Node.js (16+)
- Yarn package manager
- The WASM package in `../pkg/` directory (built from the Rust project)

### Installation

1. Install dependencies:
```bash
yarn install
```

2. Start the development server:
```bash
yarn dev
```

3. Open your browser to the URL shown in the terminal (usually `http://localhost:3000`)

### Available Scripts

- `yarn dev` - Start development server
- `yarn build` - Build for production
- `yarn preview` - Preview production build
- `yarn lint` - Run TypeScript type checking

### Configuration

The application supports configurable base paths for deployment to different environments:

#### Environment Variables

- `VITE_BASE_PATH`: Override the base path for routing and assets (e.g., `/my-repo-name`)

#### Local Development
For local development, no configuration is needed. The app runs at the root path `/`.

#### Production/CI Deployment
For GitHub Pages or similar deployments, set the `VITE_BASE_PATH` environment variable to your repository name:

```bash
# In CI or production environment
export VITE_BASE_PATH=/your-repo-name
yarn build
```

#### Environment Files
You can also use environment files:
- `.env.example` - Template showing available configuration options
- `.env.production` - Production-specific environment variables
- `.env.local` - Local overrides (not committed to git)

### GitHub Pages Routing

The app includes a solution for client-side routing on GitHub Pages:

- `public/404.html` - Redirects unknown routes to the main app
- Script in `index.html` - Handles the redirected URLs and restores the correct route

This ensures that direct navigation to routes like `/circle-collision` works correctly on GitHub Pages, not just when navigating from within the app.

## Project Structure

```
src/
├── components/          # Reusable React components
├── demos/              # Individual demo components
│   └── CircleCollisionDemo.tsx
├── App.tsx             # Main app component with routing
├── main.tsx            # Application entry point
├── styles.css          # Global styles (from original project)
└── types.d.ts          # TypeScript type declarations
```

## Adding New Demos

1. Create a new component in `src/demos/`
2. Add the route to `App.tsx`
3. Add navigation link to the nav menu
4. Import any needed WASM functions from `/pkg/silly_demos.js`

## Technologies Used

- **React 19** - UI framework
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **React Router** - Client-side routing
- **WebAssembly** - High-performance computations
- **CSS3** - Modern styling with custom properties

## WASM Integration

The app loads the WASM module from `/pkg/silly_demos.js` which contains the Rust-compiled functions. The WASM module provides:

- `circle_collision(x1, y1, r1, x2, y2, r2)` - Circle collision detection

## Performance

- WASM functions run at near-native speed
- 60 FPS rendering with RequestAnimationFrame
- Efficient canvas rendering with device pixel ratio support
- Minimal React re-renders through refs and useCallback

## Browser Support

- Modern browsers with WebAssembly support
- Chrome 57+, Firefox 52+, Safari 11+, Edge 16+
