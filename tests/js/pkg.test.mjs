// The generated package as the frontend sees it: run with `node --test tests/js/*.test.mjs`
// (`task test:pkg`) after `task build:wasm`.
//
// tests/web.rs drives the API from Rust; this checks what crosses the JS
// boundary: the exported class names, typed-array return types, Option
// arguments passed as undefined, string returns, and freed objects.

import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { before, test } from 'node:test'

const pkg = new URL('../../pkg/', import.meta.url)
const wasm = await import(new URL('silly_demos.js', pkg))

let memory
before(async () => {
  const exports = await wasm.default({ module_or_path: await readFile(new URL('silly_demos_bg.wasm', pkg)) })
  memory = exports.memory
})

test('the classes and functions the pages use are exported', () => {
  for (const name of ['BoidsParams', 'BoidsSim', 'ClusterLab', 'CollisionLab', 'GeometryTests', 'IkChain', 'RoverLab', 'SomData', 'SomLab', 'SuccessorLab', 'TspLab', 'VoronoiTests']) {
    assert.equal(typeof wasm[name], 'function', name)
  }
  assert.equal(typeof wasm.run, 'function')
})

test('numeric arrays come back as typed-array copies', () => {
  const lab = new wasm.RoverLab('explorer')
  lab.step(1)
  assert.ok(lab.knowledge() instanceof Uint8Array)
  assert.ok(lab.pose() instanceof Float32Array)
  const walls = lab.walls()
  assert.ok(walls instanceof Float32Array)
  const length = walls.length
  // A copy owned by JS, not a view into WASM memory: growing the memory (each
  // lab allocates its grids) would leave a view detached and empty.
  const size = memory.buffer.byteLength
  const view = new Float32Array(memory.buffer, 0, 4)
  const more = []
  while (memory.buffer.byteLength === size && more.length < 500) more.push(new wasm.RoverLab('explorer'))
  assert.ok(memory.buffer.byteLength > size, 'memory grew')
  assert.equal(view.length, 0, 'a view into the old memory detached')
  assert.ok(length > 0)
  assert.equal(walls.length, length)
  assert.ok(walls.every(Number.isFinite))
  for (const m of more) m.free()
  const tris = wasm.VoronoiTests.delaunay_indices(wasm.VoronoiTests.voronoi_create_points(20, 400, 300, 1, 10))
  assert.ok(tris instanceof Uint32Array && tris.length % 3 === 0)
  lab.free()
})

test('the Kohonen labs take and return the typed arrays the pages use', () => {
  // The pages hand over pixels as Uint8ClampedArray and rows as Float32Array.
  const data = new wasm.SomData(1)
  data.set_normalize(false)
  assert.ok(data.set_lattice('grid', 4, 4))
  assert.ok(data.set_data(Float32Array.from({ length: 300 }, (_, i) => (i * 37) % 256), 3))
  data.step(500)
  assert.ok(data.codebook() instanceof Float32Array && data.codebook().length === 48)
  assert.ok(data.bmus() instanceof Uint32Array && data.bmus().length === 100)
  assert.ok(data.hits() instanceof Uint32Array)
  const pixels = new Uint8ClampedArray([10, 20, 30, 255, 200, 100, 50, 128])
  const out = data.quantize(new Uint8Array(pixels))
  assert.ok(out instanceof Uint8Array && out.length === 8)
  assert.equal(out[7], 128)
  data.free()

  const lab = new wasm.SomLab(1)
  assert.ok(lab.set_density_cells(new Float32Array(16).fill(1), 4))
  assert.equal(lab.density_side(), 4)
  assert.ok(lab.trace_kernel() instanceof Float32Array)
  assert.equal(lab.trace_bmu(), -1)
  lab.step(1)
  assert.ok(lab.trace_bmu() >= 0)
  lab.free()
})

test('strings and string arrays cross as JS strings', () => {
  const lab = new wasm.RoverLab('reactive')
  assert.equal(lab.status(), 'exploring')
  lab.free()
  const kinds = wasm.CollisionLab.shape_kinds()
  assert.ok(Array.isArray(kinds) && kinds.includes('circle'))
  assert.ok(wasm.IkChain.solvers().includes('fabrik'))
})

test('Option arguments accept undefined', () => {
  const chain = new wasm.IkChain(6, 45)
  chain.set_limits(undefined, undefined)
  chain.set_limits(16, undefined)
  chain.set_limits(16, 11)
  assert.ok(chain.solve('fabrik', 100, -80, 10, 20) > 0)
  chain.free()
})

test('Option returns are undefined or a value', () => {
  const lab = new wasm.CollisionLab()
  const id = lab.methods()[0]
  const report = lab.run(id, 0)
  assert.equal(typeof report.intersect, 'boolean')
  assert.ok(report.distance === undefined || typeof report.distance === 'number')
  assert.equal(lab.run('no-such-method', 0), undefined)
  report.free()
  lab.free()
})

test('struct fields read as numbers, and writes reach the simulation', () => {
  const params = new wasm.BoidsParams()
  assert.equal(typeof params.max_speed, 'number')
  params.max_speed = 50
  params.min_speed = 0
  const sim = new wasm.BoidsSim(40, 400, 300, 9)
  sim.set_params(params)
  for (let i = 0; i < 60; i++) sim.step(1 / 60)
  const state = sim.state()
  for (let i = 0; i < state.length; i += 5) assert.ok(Math.hypot(state[i + 2], state[i + 3]) <= 50 + 1e-3)
  sim.free()
  params.free()
})

test('the Tours lab takes cities as typed arrays and returns its ring and tour', () => {
  const xy = wasm.TspLab.shape('circle', 30, 1)
  assert.ok(xy instanceof Float32Array)
  assert.equal(xy.length, 60)
  assert.equal(wasm.TspLab.shape('blob', 30, 1).length, 0)
  assert.ok(Number.isNaN(wasm.TspLab.shape_optimum('uniform', 30)))
  const lab = new wasm.TspLab(1)
  assert.equal(lab.set_cities(xy, 'euclid', new Float32Array()), true)
  assert.equal(lab.set_method('ring-som'), true)
  const params = JSON.parse(lab.params())
  assert.deepEqual(params.map(p => p.id), ['nodes', 'samples', 'alpha', 'sigma'])
  lab.step(500)
  assert.equal(lab.status(), 'running')
  const ring = lab.ring()
  assert.ok(ring instanceof Float32Array)
  assert.equal(ring.length, 2 * 60)
  const tour = lab.tour()
  assert.ok(tour instanceof Uint32Array)
  assert.deepEqual([...tour].sort((a, b) => a - b), [...Array(30).keys()])
  assert.equal(typeof lab.length(), 'number')
  assert.equal(typeof lab.readout(), 'number')
  lab.free()
})

test('the Successors lab returns each model as typed arrays', () => {
  const lab = new wasm.SuccessorLab(1)
  assert.equal(lab.set_nodes(25), true)
  lab.step(2000)
  for (const id of ['som', 'ng', 'gng', 'gng-u', 'kmeans']) {
    const w = lab.weights(id)
    assert.ok(w instanceof Float32Array, id)
    assert.ok(lab.edges(id) instanceof Uint32Array, id)
    const errors = lab.errors(id)
    assert.ok(errors instanceof Float32Array && errors.length === 4, id)
    assert.equal(errors[2], w.length / 2, id)
  }
  assert.equal(lab.weights('som').length, 50)
  assert.ok(Number.isNaN(lab.errors('kmeans')[1]))
  assert.equal(lab.weights('lvq').length, 0)
  assert.equal(lab.status(), 'training')
  lab.free()
})

test('the clustering lab returns points, clusters and traces as typed arrays and JSON', () => {
  const lab = new wasm.ClusterLab(1)
  lab.step(500)
  const points = lab.points()
  assert.ok(points instanceof Float32Array)
  assert.equal(points.length, 500 * 4)
  const assigned = lab.assignments('balls')
  assert.ok(assigned instanceof Uint32Array)
  assert.equal(assigned.length, 500)
  for (const model of ['balls', 'ellipses', 'sketch']) {
    const clusters = lab.clusters(model)
    assert.ok(clusters instanceof Float32Array && clusters.length % 8 === 0, model)
    assert.equal(typeof JSON.parse(lab.trace(model)).point[0], 'number', model)
    assert.equal(lab.event_counts(model).length, 4, model)
  }
  assert.ok(lab.facilities() instanceof Float32Array)
  assert.equal(lab.history().length % 9, 0)
  assert.equal(lab.stats().length, 14)
  assert.equal(JSON.parse(lab.params()).length, 17)
  assert.equal(lab.paint(0.5, 0.5), true)
  lab.free()
})

test('a freed object refuses further use instead of touching freed memory', () => {
  const lab = new wasm.RoverLab('explorer')
  lab.free()
  assert.throws(() => lab.step(1))
})
