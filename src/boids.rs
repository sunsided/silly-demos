#![allow(dead_code, unused_variables)]

use crate::vec2::Vec2;

/// Internal boid structure for calculations
#[derive(Clone, Copy, Debug)]
struct BoidState {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    flags: u32, // bitfield: 0x1 = in boundary margin
}

impl BoidState {
    pub const IN_MARGIN: u32 = 0x1;

    fn pos(&self) -> Vec2 {
        Vec2 {
            x: self.x,
            y: self.y,
        }
    }
    fn vel(&self) -> Vec2 {
        Vec2 {
            x: self.vx,
            y: self.vy,
        }
    }
    fn with_vel(&self, v: Vec2) -> Self {
        Self {
            x: self.x,
            y: self.y,
            vx: v.x,
            vy: v.y,
            flags: self.flags,
        }
    }
    fn with_pos(&self, p: Vec2) -> Self {
        Self {
            x: p.x,
            y: p.y,
            vx: self.vx,
            vy: self.vy,
            flags: self.flags,
        }
    }
    fn with_pos_vel(&self, p: Vec2, v: Vec2) -> Self {
        Self {
            x: p.x,
            y: p.y,
            vx: v.x,
            vy: v.y,
            flags: self.flags,
        }
    }
}

/// Calculate separation force - avoid crowding neighbors
fn separation(boid_idx: usize, states: &[BoidState], radius: f32) -> (f32, f32) {
    let mut steer_x = 0.0;
    let mut steer_y = 0.0;
    let mut count = 0;
    let boid = &states[boid_idx];
    for (j, other) in states.iter().enumerate() {
        if boid_idx == j {
            continue;
        }
        let dx = boid.x - other.x;
        let dy = boid.y - other.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq > 0.0 && dist_sq < radius * radius {
            let dist = dist_sq.sqrt();
            let normalized_x = dx / dist;
            let normalized_y = dy / dist;
            let weight = 1.0 / dist;
            steer_x += normalized_x * weight;
            steer_y += normalized_y * weight;
            count += 1;
        }
    }
    if count > 0 {
        steer_x /= count as f32;
        steer_y /= count as f32;
    }
    (steer_x, steer_y)
}

/// Calculate alignment force - steer towards average heading of neighbors
fn alignment(boid_idx: usize, states: &[BoidState], radius: f32) -> (f32, f32) {
    let mut avg_vx = 0.0;
    let mut avg_vy = 0.0;
    let mut count = 0;
    let boid = &states[boid_idx];
    for (j, other) in states.iter().enumerate() {
        if boid_idx == j {
            continue;
        }
        let dx = boid.x - other.x;
        let dy = boid.y - other.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq > 0.0 && dist_sq < radius * radius {
            avg_vx += other.vx;
            avg_vy += other.vy;
            count += 1;
        }
    }
    if count > 0 {
        avg_vx /= count as f32;
        avg_vy /= count as f32;
        let steer_x = avg_vx - boid.vx;
        let steer_y = avg_vy - boid.vy;
        (steer_x, steer_y)
    } else {
        (0.0, 0.0)
    }
}

/// Calculate cohesion force - steer towards average position of neighbors
fn cohesion(boid_idx: usize, states: &[BoidState], radius: f32) -> (f32, f32) {
    let mut avg_x = 0.0;
    let mut avg_y = 0.0;
    let mut count = 0;
    let boid = &states[boid_idx];
    for (j, other) in states.iter().enumerate() {
        if boid_idx == j {
            continue;
        }
        let dx = boid.x - other.x;
        let dy = boid.y - other.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq > 0.0 && dist_sq < radius * radius {
            avg_x += other.x;
            avg_y += other.y;
            count += 1;
        }
    }
    if count > 0 {
        avg_x /= count as f32;
        avg_y /= count as f32;
        let desired_x = avg_x - boid.x;
        let desired_y = avg_y - boid.y;
        let dist = (desired_x * desired_x + desired_y * desired_y).sqrt();
        if dist > 0.0 {
            let steer_x = desired_x / dist - boid.vx;
            let steer_y = desired_y / dist - boid.vy;
            (steer_x, steer_y)
        } else {
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    }
}

/// Limit the magnitude of a vector
fn limit_magnitude(x: f32, y: f32, max_mag: f32) -> (f32, f32) {
    let mag_sq = x * x + y * y;
    if mag_sq > max_mag * max_mag {
        let mag = mag_sq.sqrt();
        (x / mag * max_mag, y / mag * max_mag)
    } else {
        (x, y)
    }
}

/// Clamp a velocity vector to min/max speed progressively (accel/decel)
fn clamp_speed_progressive(
    mut v: Vec2,
    min_speed: f32,
    max_speed: f32,
    accel: f32,
    decel: f32,
) -> Vec2 {
    let speed = v.length();
    if speed < min_speed {
        let scale = min_speed / speed.max(1e-6);
        v = v * (1.0 - accel) + v * scale * accel;
    }
    let speed = v.length();
    if speed > max_speed {
        let scale = max_speed / speed;
        v = v * (1.0 - decel) + v * scale * decel;
    }
    v
}

/// Simple config struct for internal calculations
struct SimpleConfig {
    separation_radius: f32,
    alignment_radius: f32,
    cohesion_radius: f32,
    separation_strength: f32,
    alignment_strength: f32,
    cohesion_strength: f32,
    max_speed: f32,
    max_force: f32,
    boundary_margin: f32,
    boundary_strength: f32,
    world_width: f32,
    world_height: f32,
}

/// Utility function for updating a single boid
fn update_boid_state(
    i: usize,
    boid: &BoidState,
    states: &[BoidState],
    config: &SimpleConfig,
    dt: f32,
    min_speed: f32,
    jitter: f32,
) -> BoidState {
    // Margin check and strong pull to center: if in margin, override everything else
    let margin = config.boundary_margin;
    let in_margin = boid.x < margin
        || boid.x > config.world_width - margin
        || boid.y < margin
        || boid.y > config.world_height - margin;
    if in_margin {
        let mut flags = 0;
        flags |= BoidState::IN_MARGIN;
        // Pull toward the center of the world as a force, not a velocity
        let center = Vec2 {
            x: 0.5 * config.world_width,
            y: 0.5 * config.world_height,
        };
        let pos = Vec2 {
            x: boid.x,
            y: boid.y,
        };
        let dir = (center - pos).normalized();
        let margin_force = 10.0 * config.max_force;
        let force = dir * margin_force;
        let mut vel = Vec2 {
            x: boid.vx,
            y: boid.vy,
        } + force * dt;
        vel = clamp_speed_progressive(vel, min_speed, config.max_speed, 0.1, 0.1);
        let new_pos = pos + vel * dt;
        return BoidState {
            x: new_pos.x,
            y: new_pos.y,
            vx: vel.x,
            vy: vel.y,
            flags,
        };
    }

    // Calculate forces
    let (sep_x, sep_y) = separation(i, states, config.separation_radius);
    let (align_x, align_y) = alignment(i, states, config.alignment_radius);
    let (coh_x, coh_y) = cohesion(i, states, config.cohesion_radius);
    let boundary_result = boundary_avoidance_simple(boid, config, min_speed);

    // Combine flocking forces
    let mut force_x = sep_x * config.separation_strength
        + align_x * config.alignment_strength
        + coh_x * config.cohesion_strength;
    let mut force_y = sep_y * config.separation_strength
        + align_y * config.alignment_strength
        + coh_y * config.cohesion_strength;

    // Apply boundary force after flocking, so it always takes precedence
    match boundary_result {
        BoundaryResult::Force { fx, fy } => {
            // Add the margin force to flocking, so both combine
            force_x += fx;
            force_y += fy;
        }
        BoundaryResult::OverrideVelocity { vx, vy } => {
            let new_x = boid.x + vx * dt;
            let new_y = boid.y + vy * dt;
            return BoidState {
                x: new_x,
                y: new_y,
                vx,
                vy,
                flags: 0,
            };
        }
        BoundaryResult::Bounce { x, y, vx, vy } => {
            return BoidState {
                x,
                y,
                vx,
                vy,
                flags: 0,
            };
        }
    }

    // Limit force magnitude
    let (limited_fx, limited_fy) = limit_magnitude(force_x, force_y, config.max_force);
    force_x = limited_fx;
    force_y = limited_fy;

    // Update velocity
    let mut vel = Vec2 {
        x: boid.vx,
        y: boid.vy,
    } + Vec2 {
        x: force_x,
        y: force_y,
    } * dt;

    // Add random jitter
    let angle = (js_sys::Math::random() as f32) * std::f32::consts::TAU;
    let jitter_vec = Vec2 {
        x: angle.cos() * jitter,
        y: angle.sin() * jitter,
    };
    vel += jitter_vec;

    // Use helper for progressive speed clamping
    vel = clamp_speed_progressive(vel, min_speed, config.max_speed, 0.1, 0.1);

    // Update position
    let new_pos = Vec2 {
        x: boid.x,
        y: boid.y,
    } + vel * dt;

    // After updating position/velocity, set flags
    let mut boid_out = BoidState {
        x: new_pos.x,
        y: new_pos.y,
        vx: vel.x,
        vy: vel.y,
        flags: 0,
    };
    // Set flag if in boundary margin
    let margin = config.boundary_margin;
    let in_margin = boid_out.x < margin
        || boid_out.x > config.world_width - margin
        || boid_out.y < margin
        || boid_out.y > config.world_height - margin;
    if in_margin {
        boid_out.flags |= BoidState::IN_MARGIN;
    }
    boid_out
}

/// Simplified boids update using flat arrays to avoid WASM complexity
/// Input: [x1, y1, vx1, vy1, x2, y2, vx2, vy2, ...]
/// Returns: [x1, y1, vx1, vy1, x2, y2, vx2, vy2, ...]
pub fn update_boids_flat_impl(
    boids_data: &[f32],
    separation_radius: f32,
    alignment_radius: f32,
    cohesion_radius: f32,
    separation_strength: f32,
    alignment_strength: f32,
    cohesion_strength: f32,
    max_speed: f32,
    max_force: f32,
    boundary_margin: f32,
    boundary_strength: f32,
    world_width: f32,
    world_height: f32,
    dt: f32,
    min_speed: f32, // new
    jitter: f32,    // new
) -> Vec<f32> {
    // WARNING: Output stride is now 5 (x, y, vx, vy, flags)!
    // The frontend must use stride 5, not 4, when reading boid data.
    let boid_count = boids_data.len() / 4; // Always use input stride 4!
    if boid_count == 0 {
        return Vec::new();
    }

    // Convert flat array to internal boid states
    let states: Vec<BoidState> = (0..boid_count)
        .map(|i| {
            let idx = i * 4;
            BoidState {
                x: boids_data[idx],
                y: boids_data[idx + 1],
                vx: boids_data[idx + 2],
                vy: boids_data[idx + 3],
                flags: 0,
            }
        })
        .collect();

    // Create simple config struct for calculations
    let config = SimpleConfig {
        separation_radius,
        alignment_radius,
        cohesion_radius,
        separation_strength,
        alignment_strength,
        cohesion_strength,
        max_speed,
        max_force,
        boundary_margin,
        boundary_strength,
        world_width,
        world_height,
    };

    // Update each boid
    let updated_states: Vec<BoidState> = states
        .iter()
        .enumerate()
        .map(|(i, boid)| update_boid_state(i, boid, &states, &config, dt, min_speed, jitter))
        .collect();

    // Convert back to flat array (stride 5: x, y, vx, vy, flags)
    let mut result = Vec::with_capacity(boid_count * 5);
    for state in updated_states {
        result.push(state.x);
        result.push(state.y);
        result.push(state.vx);
        result.push(state.vy);
        result.push(state.flags as f32);
    }

    result
}

/// Calculate the force from all boundaries for a boid, returning the total force and the strongest wall direction.
fn boundary_forces(boid: &BoidState, config: &SimpleConfig) -> ((f32, f32), (f32, f32), f32) {
    let width = config.world_width;
    let height = config.world_height;
    let margin = config.boundary_margin;
    let strength = config.boundary_strength;
    // Increase wall_mult and cap for stronger margin repulsion
    let wall_mult = 30.0;
    let mut force_x = 0.0;
    let mut force_y = 0.0;
    let mut max_wall_force = 0.0;
    let mut wall_dir_x = 0.0;
    let mut wall_dir_y = 0.0;
    // Steeper smoothstep for sharper ramp-up
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        // Sharper: raise to higher power
        let t2 = t * t;
        t2 * t2 * (6.0 * t - 15.0 * t2 + 10.0)
    }
    // Raise the cap for wall force
    let max_wall_cap = 4.0 * strength;
    // Left
    if boid.x < margin {
        let d = (margin - boid.x).max(0.0) / margin;
        let s = smoothstep(0.0, 1.0, d);
        let f = (wall_mult * 0.3 * strength * s).min(max_wall_cap);
        if f > max_wall_force {
            max_wall_force = f;
            wall_dir_x = 1.0;
            wall_dir_y = 0.0;
        }
        force_x += f;
    }
    // Right
    if boid.x > width - margin {
        let d = (boid.x - (width - margin)).max(0.0) / margin;
        let s = smoothstep(0.0, 1.0, d);
        let f = (wall_mult * 0.3 * strength * s).min(max_wall_cap);
        if f > max_wall_force {
            max_wall_force = f;
            wall_dir_x = -1.0;
            wall_dir_y = 0.0;
        }
        force_x -= f;
    }
    // Top
    if boid.y < margin {
        let d = (margin - boid.y).max(0.0) / margin;
        let s = smoothstep(0.0, 1.0, d);
        let f = (wall_mult * 0.3 * strength * s).min(max_wall_cap);
        if f > max_wall_force {
            max_wall_force = f;
            wall_dir_x = 0.0;
            wall_dir_y = 1.0;
        }
        force_y += f;
    }
    // Bottom
    if boid.y > height - margin {
        let d = (boid.y - (height - margin)).max(0.0) / margin;
        let s = smoothstep(0.0, 1.0, d);
        let f = (wall_mult * 0.3 * strength * s).min(max_wall_cap);
        if f > max_wall_force {
            max_wall_force = f;
            wall_dir_x = 0.0;
            wall_dir_y = -1.0;
        }
        force_y -= f;
    }
    ((force_x, force_y), (wall_dir_x, wall_dir_y), max_wall_force)
}

/// Handle hard bounce if a boid is out of bounds, returning (bounced, new_x, new_y, new_vx, new_vy)
fn handle_hard_bounce(
    boid: &BoidState,
    config: &SimpleConfig,
    nudge: f32,
    rng: f32,
) -> Option<(f32, f32, f32, f32)> {
    let width = config.world_width;
    let height = config.world_height;
    let mut new_x = boid.x;
    let mut new_y = boid.y;
    let mut new_vx = boid.vx;
    let mut new_vy = boid.vy;
    let mut bounced = false;
    if boid.x < 0.0 {
        new_x = 0.0;
        new_vx = boid.vx.abs() + nudge * (0.8 + 0.4 * rng);
        bounced = true;
    } else if boid.x > width {
        new_x = width;
        new_vx = -boid.vx.abs() - nudge * (0.8 + 0.4 * rng);
        bounced = true;
    }
    if boid.y < 0.0 {
        new_y = 0.0;
        new_vy = boid.vy.abs() + nudge * (0.8 + 0.4 * rng);
        bounced = true;
    } else if boid.y > height {
        new_y = height;
        new_vy = -boid.vy.abs() - nudge * (0.8 + 0.4 * rng);
        bounced = true;
    }
    if bounced {
        Some((new_x, new_y, new_vx, new_vy))
    } else {
        None
    }
}

/// Calculate boundary avoidance force with simple config (refactored, no thread_local)
fn boundary_avoidance_simple(
    boid: &BoidState,
    config: &SimpleConfig,
    min_speed: f32,
) -> BoundaryResult {
    let nudge = 5.0;
    let rng = js_sys::Math::random() as f32;
    let ((force_x, force_y), (_wall_dir_x, _wall_dir_y), max_wall_force) =
        boundary_forces(boid, config);

    if let Some((new_x, new_y, new_vx, new_vy)) = handle_hard_bounce(boid, config, nudge, rng) {
        return BoundaryResult::Bounce {
            x: new_x,
            y: new_y,
            vx: new_vx,
            vy: new_vy,
        };
    }

    // Always apply a force in the margin, never override velocity unless hard bounce
    BoundaryResult::Force {
        fx: force_x,
        fy: force_y,
    }
}

/// Result type for boundary_avoidance_simple
enum BoundaryResult {
    Force { fx: f32, fy: f32 },
    OverrideVelocity { vx: f32, vy: f32 },
    Bounce { x: f32, y: f32, vx: f32, vy: f32 },
}
