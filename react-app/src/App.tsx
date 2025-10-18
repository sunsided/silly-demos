import { Routes, Route, Link, useLocation } from 'react-router-dom'
import CircleCollisionDemo from './demos/CircleCollisionDemo'
import PointLineDemo from './demos/PointLineDemo'
import BoidsDemo from './demos/BoidsDemo'

function App() {
  const location = useLocation()

  return (
    <div className="app">
      <nav className="navigation">
        <div className="nav-container">
          <h1 className="nav-title">
            <span className="gradient-title">Silly Demos with WASM</span>
          </h1>
          <div className="nav-links">
            <Link 
              to="/" 
              className={`nav-link ${location.pathname === '/' ? 'active' : ''}`}
            >
              Home
            </Link>
            <Link 
              to="/circle-collision" 
              className={`nav-link ${location.pathname === '/circle-collision' ? 'active' : ''}`}
            >
              Circle Collision
            </Link>
            <Link 
              to="/point-line" 
              className={`nav-link ${location.pathname === '/point-line' ? 'active' : ''}`}
            >
              Point-Line Test
            </Link>
            <Link 
              to="/boids" 
              className={`nav-link ${location.pathname === '/boids' ? 'active' : ''}`}
            >
              Boids Simulation
            </Link>
          </div>
        </div>
      </nav>

      <main className="main-content">
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/circle-collision" element={<CircleCollisionDemo />} />
          <Route path="/point-line" element={<PointLineDemo />} />
          <Route path="/boids" element={<BoidsDemo />} />
        </Routes>
      </main>
    </div>
  )
}

function Home() {
  return (
    <div className="home-container">
      <div className="glass-panel home-panel">
        <h2 className="gradient-title">Welcome to Silly Demos with WASM</h2>
        <p className="description-text">
          This is a collection of interactive demos showcasing WebAssembly (WASM) capabilities 
          built with Rust and compiled to run in the browser.
        </p>
        
        <div className="demo-grid">
          <Link to="/circle-collision" className="demo-card">
            <h3>Circle Collision Detection</h3>
            <p>Interactive demonstration of circle collision detection and physics calculations using WASM.</p>
            <div className="demo-features">
              <span className="pill">Real-time</span>
              <span className="pill">Physics</span>
              <span className="pill">Interactive</span>
            </div>
          </Link>
          
          <Link to="/point-line" className="demo-card">
            <h3>Point-Line Distance Test</h3>
            <p>Interactive demonstration of point-to-line distance calculations and geometric relationships.</p>
            <div className="demo-features">
              <span className="pill">Geometry</span>
              <span className="pill">Real-time</span>
              <span className="pill">Visual</span>
            </div>
          </Link>

          <Link to="/boids" className="demo-card">
            <h3>Boids Flocking Simulation</h3>
            <p>Interactive simulation of flocking behavior using separation, alignment, and cohesion rules.</p>
            <div className="demo-features">
              <span className="pill">AI</span>
              <span className="pill">Simulation</span>
              <span className="pill">Interactive</span>
            </div>
          </Link>
        </div>

        <div className="tech-info">
          <h3>Technologies Used</h3>
          <div className="tech-tags">
            <span className="code-highlight">Rust</span>
            <span className="code-highlight">WebAssembly</span>
            <span className="code-highlight">React</span>
            <span className="code-highlight">TypeScript</span>
            <span className="code-highlight">FiraCode</span>
          </div>
        </div>
      </div>
    </div>
  )
}

export default App
