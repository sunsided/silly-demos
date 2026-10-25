/** The demos, in navigation order. `route` is the SvelteKit route ID. */
export const DEMOS = [
  {
    route: '/boids',
    name: 'Boids',
    blurb: 'A flock that emerges from three local steering rules: keep your distance, match heading, stay together.',
    rust: 'BoidsSim::step',
  },
  {
    route: '/collision',
    name: 'Collision lab',
    blurb:
      'Pick two shapes and compare the collision tests that fit them: clamping, separating axes, GJK and EPA, with their costs.',
    rust: 'CollisionLab::run',
  },
  {
    route: '/point-line',
    name: 'Point to segment',
    blurb: 'The shortest distance from a point to a line segment, and which side of the line the point is on.',
    rust: 'GeometryTests::point_line_test',
  },
  {
    route: '/voronoi',
    name: 'Voronoi / Delaunay',
    blurb: 'Moving points with their Delaunay triangulation and the Voronoi diagram it implies.',
    rust: 'VoronoiTests::voronoi_edges',
  },
] as const

export type Demo = (typeof DEMOS)[number]
