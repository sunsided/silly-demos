import { useState } from 'react'
import { Link, NavLink, Route, Routes } from 'react-router-dom'
import CircleCollisionDemo from './demos/CircleCollisionDemo'
import PointLineDemo from './demos/PointLineDemo'
import BoidsDemo from './demos/BoidsDemo'
import VoronoiDemo from './demos/VoronoiDemo'
import { DEFAULT_FLOCK, FlockCanvas } from './components/FlockCanvas'
import { useWasm } from './lib/wasm'

const DEMOS = [
  {
    path: '/boids',
    name: 'Boids',
    blurb: 'A flock that emerges from three local steering rules: keep your distance, match heading, stay together.',
    rust: 'BoidsSim::step',
  },
  {
    path: '/circle-collision',
    name: 'Circle collision',
    blurb: 'Drag two circles and watch the center distance, the gap between them and how far they overlap.',
    rust: 'CollisionTests::circle_collision',
  },
  {
    path: '/point-line',
    name: 'Point to segment',
    blurb: 'The shortest distance from a point to a line segment, and which side of the line the point is on.',
    rust: 'GeometryTests::point_line_test',
  },
  {
    path: '/voronoi',
    name: 'Voronoi / Delaunay',
    blurb: 'Moving points with their Delaunay triangulation and the Voronoi diagram it implies.',
    rust: 'VoronoiTests::voronoi_edges',
  },
]

function App() {
  return (
    <div className="app">
      <header className="masthead">
        <Link to="/" className="wordmark">silly-demos</Link>
        <nav className="masthead-nav" aria-label="Demos">
          {DEMOS.map(d => (
            <NavLink key={d.path} to={d.path} className="nav-link">{d.name}</NavLink>
          ))}
        </nav>
        <a className="nav-link source-link" href="https://github.com/sunsided/silly-demos">Source</a>
      </header>

      <main className="main">
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/circle-collision" element={<CircleCollisionDemo />} />
          <Route path="/point-line" element={<PointLineDemo />} />
          <Route path="/boids" element={<BoidsDemo />} />
          <Route path="/voronoi" element={<VoronoiDemo />} />
        </Routes>
      </main>
    </div>
  )
}

const HERO_FLOCK = { ...DEFAULT_FLOCK, count: 260 }

function Home() {
  const { wasm } = useWasm()
  const [reducedMotion] = useState(() => window.matchMedia('(prefers-reduced-motion: reduce)').matches)

  return (
    <div className="home">
      <section className="hero grid-paper">
        {wasm && (
          <FlockCanvas
            wasm={wasm}
            config={HERO_FLOCK}
            running={!reducedMotion}
            className="hero-canvas"
            label="A flock of boids drifting across the page"
          />
        )}
        <div className="hero-text">
          <h1 className="hero-title">silly-demos</h1>
          <p className="hero-lede">
            Small geometry and simulation sketches. The math runs in Rust compiled to WebAssembly;
            the browser only draws.
          </p>
        </div>
      </section>

      <ol className="index" aria-label="Demos">
        {DEMOS.map(d => (
          <li key={d.path}>
            <Link to={d.path} className="index-row">
              <span className="index-name">{d.name}</span>
              <span className="index-blurb">{d.blurb}</span>
              <code className="index-rust">{d.rust}</code>
            </Link>
          </li>
        ))}
      </ol>
    </div>
  )
}

export default App
