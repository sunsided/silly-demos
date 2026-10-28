//! Reynolds-style boids flocking.
//!
//! Each boid computes three steering forces from the neighbors it can see
//! (separation, alignment and cohesion) plus a containment force near the
//! world edges and a small random wander. Every steering force follows
//! Reynolds' formulation: `steer = desired_velocity - velocity`, limited to
//! `max_force`. All quantities are in pixels and seconds, so the result is
//! independent of the frame rate.
//!
//! Neighbor lookups use a uniform grid whose cell size is the largest
//! perception radius, which keeps a step roughly O(n) for evenly spread flocks.

use crate::vec2::Vec2;

/// Tunable flocking parameters. Distances in px, speeds in px/s, forces in px/s².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlockParams {
    /// Radius within which other boids are steered away from.
    pub separation_radius: f32,
    /// Radius within which other boids' velocities are matched.
    pub alignment_radius: f32,
    /// Radius within which boids steer towards the center of the others.
    pub cohesion_radius: f32,
    /// Scale of the separation force.
    pub separation_weight: f32,
    /// Scale of the alignment force.
    pub alignment_weight: f32,
    /// Scale of the cohesion force.
    pub cohesion_weight: f32,
    /// Slowest a boid may fly; slower boids are sped up along their heading.
    pub min_speed: f32,
    /// Fastest a boid may fly, and the speed every steering force aims for.
    pub max_speed: f32,
    /// Cap on each steering force before it is weighted.
    pub max_force: f32,
    /// Width of the band along each edge in which boids steer back inwards.
    pub edge_margin: f32,
    /// Scale of the containment force at full depth into the margin.
    pub edge_weight: f32,
    /// Magnitude of the random acceleration applied each step.
    pub wander: f32,
    /// Full field-of-view angle in degrees; neighbors behind the boid are ignored.
    pub view_angle: f32,
}

impl Default for FlockParams {
    fn default() -> Self {
        Self {
            separation_radius: 28.0,
            alignment_radius: 75.0,
            cohesion_radius: 75.0,
            separation_weight: 2.5,
            alignment_weight: 1.4,
            cohesion_weight: 0.5,
            min_speed: 60.0,
            max_speed: 140.0,
            max_force: 240.0,
            edge_margin: 70.0,
            edge_weight: 2.5,
            wander: 40.0,
            view_angle: 270.0,
        }
    }
}

/// A single boid.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boid {
    /// Position in px, within the world.
    pub pos: Vec2,
    /// Velocity in px/s.
    pub vel: Vec2,
    /// Number of neighbors within the cohesion radius during the last step.
    pub neighbors: u32,
}

/// Small xorshift PRNG so the simulation is deterministic and testable natively.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Seeds the generator; any seed, including zero, is valid.
    pub fn new(seed: u64) -> Self {
        // Zero is a fixed point of xorshift; nudge it away.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    /// Advances the state by one xorshift64 step and returns it.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Unit vector in a uniformly random direction.
    pub fn unit_vec(&mut self) -> Vec2 {
        let angle = self.next_f32() * std::f32::consts::TAU;
        Vec2::new(angle.cos(), angle.sin())
    }
}

/// Uniform spatial grid holding boid indices, rebuilt every step.
#[derive(Debug, Default)]
struct Grid {
    /// Cell edge length in px, at least 1.
    cell: f32,
    /// Number of cell columns.
    cols: usize,
    /// Number of cell rows.
    rows: usize,
    /// `starts[c]..starts[c + 1]` indexes `items` for cell `c`.
    starts: Vec<usize>,
    /// Boid indices, grouped by cell.
    items: Vec<usize>,
}

impl Grid {
    /// Column and row of the cell holding `p`; points outside the world map to the border cells.
    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let cx = ((p.x / self.cell).floor().max(0.0) as usize).min(self.cols - 1);
        let cy = ((p.y / self.cell).floor().max(0.0) as usize).min(self.rows - 1);
        (cx, cy)
    }

    /// Re-buckets `boids` into cells of edge `cell` covering a `width` × `height` world,
    /// by counting sort.
    fn rebuild(&mut self, boids: &[Boid], width: f32, height: f32, cell: f32) {
        self.cell = cell.max(1.0);
        self.cols = ((width / self.cell).ceil() as usize).max(1);
        self.rows = ((height / self.cell).ceil() as usize).max(1);
        let n_cells = self.cols * self.rows;

        // Counting sort of boid indices by cell.
        self.starts.clear();
        self.starts.resize(n_cells + 1, 0);
        let cells: Vec<usize> = boids
            .iter()
            .map(|b| {
                let (cx, cy) = self.cell_of(b.pos);
                cy * self.cols + cx
            })
            .collect();
        for &c in &cells {
            self.starts[c + 1] += 1;
        }
        for c in 0..n_cells {
            self.starts[c + 1] += self.starts[c];
        }
        let mut fill = self.starts.clone();
        self.items.clear();
        self.items.resize(boids.len(), 0);
        for (i, &c) in cells.iter().enumerate() {
            self.items[fill[c]] = i;
            fill[c] += 1;
        }
    }

    /// Calls `f` with every boid index in the 3×3 block of cells around `p`.
    fn for_each_near(&self, p: Vec2, mut f: impl FnMut(usize)) {
        let (cx, cy) = self.cell_of(p);
        for y in cy.saturating_sub(1)..=(cy + 1).min(self.rows - 1) {
            for x in cx.saturating_sub(1)..=(cx + 1).min(self.cols - 1) {
                let c = y * self.cols + x;
                for &i in &self.items[self.starts[c]..self.starts[c + 1]] {
                    f(i);
                }
            }
        }
    }
}

/// How far ahead (in seconds of travel) boids look for walls.
const EDGE_LOOKAHEAD: f32 = 0.4;

/// Reynolds steering towards `desired` direction at full speed.
fn steer(desired: Vec2, vel: Vec2, params: &FlockParams) -> Vec2 {
    if desired.length_squared() == 0.0 {
        return Vec2::default();
    }
    (desired.normalized() * params.max_speed - vel).limit(params.max_force)
}

/// Inward push from walls `p` is within `margin` of (or beyond); each component is in `[-1, 1]`.
fn edge_push(p: Vec2, width: f32, height: f32, margin: f32) -> Vec2 {
    if margin <= 0.0 {
        return Vec2::default();
    }
    let ramp = |d: f32| ((margin - d) / margin).clamp(0.0, 1.0);
    Vec2::new(
        ramp(p.x) - ramp(width - p.x),
        ramp(p.y) - ramp(height - p.y),
    )
}

/// Desired heading near a wall: inwards, but keeping the boid's motion along
/// the wall so it turns rather than brakes. Head-on, it picks a side.
fn edge_desired(push: Vec2, vel: Vec2, max_speed: f32) -> Vec2 {
    let n = push.normalized();
    let tangent = vel - n * vel.dot(n);
    let tangent = if tangent.length_squared() < 1e-6 {
        Vec2::new(-n.y, n.x) * max_speed
    } else {
        tangent
    };
    n * max_speed + tangent
}

/// A flock of boids in the world `[0, width] × [0, height]`.
pub struct Flock {
    /// The boids as of the last step.
    pub boids: Vec<Boid>,
    /// Parameters the next step uses.
    pub params: FlockParams,
    /// World width in px.
    pub width: f32,
    /// World height in px.
    pub height: f32,
    /// Source of spawn positions and wander.
    rng: Rng,
    /// Neighbor lookup grid, kept between steps to reuse its buffers.
    grid: Grid,
    /// Scratch buffer the next state is written into, then swapped with `boids`.
    next: Vec<Boid>,
}

impl Flock {
    /// Creates `count` randomly placed boids with the default parameters; `seed`
    /// makes the run reproducible.
    pub fn new(count: usize, width: f32, height: f32, seed: u64) -> Self {
        let mut flock = Self {
            boids: Vec::new(),
            params: FlockParams::default(),
            width,
            height,
            rng: Rng::new(seed),
            grid: Grid::default(),
            next: Vec::new(),
        };
        flock.set_count(count);
        flock
    }

    /// A boid at a random position, flying in a random direction at the middle of
    /// the speed range.
    fn spawn(&mut self) -> Boid {
        let speed = 0.5 * (self.params.min_speed + self.params.max_speed);
        Boid {
            pos: Vec2::new(
                self.rng.next_f32() * self.width,
                self.rng.next_f32() * self.height,
            ),
            vel: self.rng.unit_vec() * speed,
            neighbors: 0,
        }
    }

    /// Grows or shrinks the flock, keeping existing boids in place.
    pub fn set_count(&mut self, count: usize) {
        self.boids.truncate(count);
        while self.boids.len() < count {
            let boid = self.spawn();
            self.boids.push(boid);
        }
    }

    /// Respawns all boids at random positions.
    pub fn scatter(&mut self) {
        let count = self.boids.len();
        self.boids.clear();
        self.set_count(count);
    }

    /// Changes the world size, at least 1 px each way. Boids left outside are put
    /// back on the wall during the next step.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.width = width.max(1.0);
        self.height = height.max(1.0);
    }

    /// Advances the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let p = self.params;
        let sep_r2 = p.separation_radius * p.separation_radius;
        let ali_r2 = p.alignment_radius * p.alignment_radius;
        let coh_r2 = p.cohesion_radius * p.cohesion_radius;
        let max_r = p
            .separation_radius
            .max(p.alignment_radius)
            .max(p.cohesion_radius);
        let cos_half_fov = (0.5 * p.view_angle.clamp(0.0, 360.0)).to_radians().cos();

        self.grid
            .rebuild(&self.boids, self.width, self.height, max_r);

        // Read from the previous state, write into `next`, so update order doesn't matter.
        self.next.clear();
        for (i, boid) in self.boids.iter().enumerate() {
            let heading = boid.vel.normalized();
            let mut sep = Vec2::default();
            let mut ali = Vec2::default();
            let mut coh = Vec2::default();
            let (mut ali_n, mut coh_n) = (0u32, 0u32);

            self.grid.for_each_near(boid.pos, |j| {
                if i == j {
                    return;
                }
                let other = &self.boids[j];
                let d = other.pos - boid.pos;
                let d2 = d.length_squared();
                if d2 == 0.0 || d2 >= coh_r2.max(ali_r2).max(sep_r2) {
                    return;
                }
                let dist = d2.sqrt();
                if heading.dot(d) < cos_half_fov * dist {
                    return;
                }
                if d2 < sep_r2 {
                    // Away from the neighbor, stronger the closer it is.
                    sep -= d / d2;
                }
                if d2 < ali_r2 {
                    ali += other.vel;
                    ali_n += 1;
                }
                if d2 < coh_r2 {
                    coh += other.pos;
                    coh_n += 1;
                }
            });

            let mut accel = steer(sep, boid.vel, &p) * p.separation_weight;
            if ali_n > 0 {
                accel += steer(ali / ali_n as f32, boid.vel, &p) * p.alignment_weight;
            }
            if coh_n > 0 {
                let center = coh / coh_n as f32;
                accel += steer(center - boid.pos, boid.vel, &p) * p.cohesion_weight;
            }

            // Probe ahead so faster boids start turning earlier.
            let probe = boid.pos + boid.vel * EDGE_LOOKAHEAD;
            let push = edge_push(probe, self.width, self.height, p.edge_margin);
            if push.length_squared() > 0.0 {
                let depth = push.length().min(1.0);
                let desired = edge_desired(push, boid.vel, p.max_speed);
                accel += steer(desired, boid.vel, &p) * (p.edge_weight * depth);
            }

            accel += self.rng.unit_vec() * p.wander;

            let mut vel = boid.vel + accel * dt;
            let speed = vel.length();
            if speed > p.max_speed {
                vel *= p.max_speed / speed;
            } else if speed < p.min_speed {
                vel = if speed > 0.0 {
                    vel * (p.min_speed / speed)
                } else {
                    heading * p.min_speed
                };
            }

            let mut pos = boid.pos + vel * dt;
            // Hard walls as a safety net (e.g. after the world shrinks).
            if pos.x < 0.0 || pos.x > self.width {
                pos.x = pos.x.clamp(0.0, self.width);
                vel.x = if pos.x == 0.0 {
                    vel.x.abs()
                } else {
                    -vel.x.abs()
                };
            }
            if pos.y < 0.0 || pos.y > self.height {
                pos.y = pos.y.clamp(0.0, self.height);
                vel.y = if pos.y == 0.0 {
                    vel.y.abs()
                } else {
                    -vel.y.abs()
                };
            }

            self.next.push(Boid {
                pos,
                vel,
                neighbors: coh_n,
            });
        }
        std::mem::swap(&mut self.boids, &mut self.next);
    }

    /// Mean of unit headings: ~0 for random motion, 1 when everyone flies the same way.
    pub fn polarization(&self) -> f32 {
        if self.boids.is_empty() {
            return 0.0;
        }
        let sum = self
            .boids
            .iter()
            .fold(Vec2::default(), |acc, b| acc + b.vel.normalized());
        sum.length() / self.boids.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn only(f: impl FnOnce(&mut FlockParams)) -> FlockParams {
        let mut p = FlockParams {
            separation_weight: 0.0,
            alignment_weight: 0.0,
            cohesion_weight: 0.0,
            edge_weight: 0.0,
            wander: 0.0,
            min_speed: 0.0,
            view_angle: 360.0,
            ..FlockParams::default()
        };
        f(&mut p);
        p
    }

    fn flock_of(boids: &[(f32, f32, f32, f32)], params: FlockParams) -> Flock {
        let mut flock = Flock::new(0, 1000.0, 1000.0, 1);
        flock.params = params;
        flock.boids = boids
            .iter()
            .map(|&(x, y, vx, vy)| Boid {
                pos: Vec2::new(x, y),
                vel: Vec2::new(vx, vy),
                neighbors: 0,
            })
            .collect();
        flock
    }

    fn dist(f: &Flock) -> f32 {
        (f.boids[0].pos - f.boids[1].pos).length()
    }

    #[test]
    fn separation_pushes_close_boids_apart() {
        let mut f = flock_of(
            &[(500.0, 500.0, 50.0, 0.0), (500.0, 510.0, 50.0, 0.0)],
            only(|p| p.separation_weight = 1.0),
        );
        for _ in 0..30 {
            f.step(DT);
        }
        assert!(dist(&f) > 15.0, "distance {}", dist(&f));
    }

    #[test]
    fn cohesion_pulls_boids_together() {
        let mut f = flock_of(
            &[(500.0, 470.0, 0.0, 0.0), (500.0, 530.0, 0.0, 0.0)],
            only(|p| p.cohesion_weight = 1.0),
        );
        for _ in 0..20 {
            f.step(DT);
        }
        assert!(dist(&f) < 50.0, "distance {}", dist(&f));
    }

    #[test]
    fn alignment_matches_headings() {
        let mut f = flock_of(
            &[(500.0, 500.0, 80.0, 0.0), (520.0, 500.0, 0.0, 80.0)],
            only(|p| p.alignment_weight = 1.0),
        );
        let angle = |f: &Flock| {
            let (a, b) = (f.boids[0].vel.normalized(), f.boids[1].vel.normalized());
            a.dot(b).clamp(-1.0, 1.0).acos()
        };
        let before = angle(&f);
        for _ in 0..60 {
            f.step(DT);
        }
        assert!(angle(&f) < 0.2 * before, "angle {}", angle(&f));
    }

    #[test]
    fn cohesion_does_not_brake_a_boid_moving_towards_its_neighbors() {
        // Regression: cohesion used to subtract the full velocity from a unit
        // direction, which acted as drag on every boid with neighbors.
        let mut f = flock_of(
            &[(500.0, 500.0, 100.0, 0.0), (540.0, 500.0, 100.0, 0.0)],
            only(|p| p.cohesion_weight = 1.0),
        );
        f.step(DT);
        assert!(f.boids[0].vel.x >= 100.0);
    }

    #[test]
    fn neighbors_behind_are_ignored() {
        let mut f = flock_of(
            &[(500.0, 500.0, 100.0, 0.0), (460.0, 500.0, 0.0, 100.0)],
            only(|p| {
                p.alignment_weight = 1.0;
                p.view_angle = 180.0;
            }),
        );
        f.step(DT);
        assert_eq!(f.boids[0].vel, Vec2::new(100.0, 0.0));
        assert_eq!(f.boids[0].neighbors, 0);
    }

    #[test]
    fn boids_stay_inside_world_and_respect_speed_limits() {
        let mut f = Flock::new(300, 800.0, 500.0, 7);
        for _ in 0..1200 {
            f.step(DT);
            for b in &f.boids {
                assert!((0.0..=800.0).contains(&b.pos.x));
                assert!((0.0..=500.0).contains(&b.pos.y));
                let s = b.vel.length();
                assert!(s >= f.params.min_speed - 1e-3 && s <= f.params.max_speed + 1e-3);
            }
        }
    }

    #[test]
    fn edge_force_turns_boid_before_the_wall() {
        let mut f = flock_of(
            &[(900.0, 500.0, 140.0, 0.0)],
            only(|p| {
                p.edge_weight = 2.5;
                p.min_speed = 60.0;
            }),
        );
        let mut max_x: f32 = 0.0;
        for _ in 0..240 {
            f.step(DT);
            max_x = max_x.max(f.boids[0].pos.x);
        }
        assert!(max_x < 1000.0, "reached wall at {max_x}");
        assert!(f.boids[0].vel.x < 0.0);
    }

    #[test]
    fn grid_finds_the_same_neighbors_as_brute_force() {
        let f = Flock::new(400, 900.0, 600.0, 3);
        let r = 75.0;
        let mut grid = Grid::default();
        grid.rebuild(&f.boids, f.width, f.height, r);
        let near = |i: usize, j: usize| {
            i != j && (f.boids[i].pos - f.boids[j].pos).length_squared() < r * r
        };
        for i in 0..f.boids.len() {
            let mut via_grid = Vec::new();
            grid.for_each_near(f.boids[i].pos, |j| {
                if near(i, j) {
                    via_grid.push(j);
                }
            });
            via_grid.sort_unstable();
            let brute: Vec<usize> = (0..f.boids.len()).filter(|&j| near(i, j)).collect();
            assert_eq!(via_grid, brute, "boid {i}");
        }
    }

    #[test]
    fn random_flock_self_organizes() {
        let mut f = Flock::new(250, 1200.0, 800.0, 42);
        let before = f.polarization();
        for _ in 0..1800 {
            f.step(DT);
        }
        let after = f.polarization();
        let avg_neighbors =
            f.boids.iter().map(|b| b.neighbors as f32).sum::<f32>() / f.boids.len() as f32;
        assert!(before < 0.2, "initial polarization {before}");
        assert!(after > 0.5, "polarization {after}");
        assert!(avg_neighbors > 4.0, "avg neighbors {avg_neighbors}");
    }
}
