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
    route: '/inverse-kinematics',
    name: 'Inverse kinematics',
    blurb: 'A jointed arm reaching for a target, solved with FABRIK, CCD or the Jacobian, with the error and iterations each needs.',
    rust: 'IkChain::solve',
  },
  {
    route: '/kohonen',
    name: 'Kohonen maps',
    blurb:
      'Self-organizing maps that sort colors into a palette, map the world’s countries by ten indicators from 1992 to 2022, and unfold over a density you paint.',
    rust: 'SomData::step',
  },
  {
    route: '/point-line',
    name: 'Point to segment',
    blurb: 'The shortest distance from a point to a line segment, and which side of the line the point is on.',
    rust: 'GeometryTests::point_line_test',
  },
  {
    route: '/rover',
    name: 'Rover exploration',
    blurb:
      'A rover with a laser scanner maps a floor plan it has never seen, clearing the fog of war frontier by frontier, then drives home.',
    rust: 'RoverLab::step',
  },
  {
    route: '/voronoi',
    name: 'Voronoi / Delaunay',
    blurb: 'Moving points with their Delaunay triangulation and the Voronoi diagram it implies.',
    rust: 'VoronoiTests::voronoi_edges',
  },
] as const

export type Demo = (typeof DEMOS)[number]
