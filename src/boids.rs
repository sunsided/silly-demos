// Simple internal structures - not exported to WASM

/// Internal boid structure for calculations
#[derive(Clone, Copy)]
struct BoidState {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// Calculate separation force - avoid crowding neighbors
fn separation(boid: &BoidState, neighbors: &[BoidState], radius: f32) -> (f32, f32) {
    let mut steer_x = 0.0;
    let mut steer_y = 0.0;
    let mut count = 0;

    for other in neighbors {
        let dx = boid.x - other.x;
        let dy = boid.y - other.y;
        let dist_sq = dx * dx + dy * dy;
        
        if dist_sq > 0.0 && dist_sq < radius * radius {
            let dist = dist_sq.sqrt();
            let normalized_x = dx / dist;
            let normalized_y = dy / dist;
            
            // Weight by distance (closer = stronger avoidance)
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
fn alignment(boid: &BoidState, neighbors: &[BoidState], radius: f32) -> (f32, f32) {
    let mut avg_vx = 0.0;
    let mut avg_vy = 0.0;
    let mut count = 0;

    for other in neighbors {
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
        
        // Desired velocity is the average velocity
        let steer_x = avg_vx - boid.vx;
        let steer_y = avg_vy - boid.vy;
        
        (steer_x, steer_y)
    } else {
        (0.0, 0.0)
    }
}

/// Calculate cohesion force - steer towards average position of neighbors
fn cohesion(boid: &BoidState, neighbors: &[BoidState], radius: f32) -> (f32, f32) {
    let mut avg_x = 0.0;
    let mut avg_y = 0.0;
    let mut count = 0;

    for other in neighbors {
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
        
        // Desired velocity is towards the center of mass
        let desired_x = avg_x - boid.x;
        let desired_y = avg_y - boid.y;
        
        // Normalize and scale to max speed
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

// Thread-local to store bounce result for the current boid
thread_local! {
    static BOUNCE_RESULT: std::cell::RefCell<Option<(f32, f32, f32, f32)>> = std::cell::RefCell::new(None);
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
    jitter: f32     // new
) -> Vec<f32> {
    let boid_count = boids_data.len() / 4;
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

    use std::f32::consts::TAU;
    use js_sys::Math;

    // Update each boid
    let updated_states: Vec<BoidState> = states
        .iter()
        .enumerate()
        .map(|(i, boid)| {
            // Create a slice of all other boids for neighbor calculations
            let mut neighbors = Vec::with_capacity(states.len() - 1);
            for (j, other) in states.iter().enumerate() {
                if i != j {
                    neighbors.push(*other);
                }
            }

            // Calculate forces
            let (sep_x, sep_y) = separation(boid, &neighbors, config.separation_radius);
            let (align_x, align_y) = alignment(boid, &neighbors, config.alignment_radius);
            let (coh_x, coh_y) = cohesion(boid, &neighbors, config.cohesion_radius);
            let (bound_x, bound_y) = boundary_avoidance_simple(boid, &config, min_speed);

            // Combine forces
            let mut force_x = sep_x * config.separation_strength
                + align_x * config.alignment_strength
                + coh_x * config.cohesion_strength;
            let mut force_y = sep_y * config.separation_strength
                + align_y * config.alignment_strength
                + coh_y * config.cohesion_strength;

            let mut bounced = false;
            let mut bounce_vals = None;
            let mut override_velocity = false;
            if bound_x.is_nan() || bound_y.is_nan() {
                bounced = true;
                bounce_vals = BOUNCE_RESULT.with(|cell| cell.borrow().clone());
            } else if bound_x.is_infinite() || bound_y.is_infinite() {
                override_velocity = true;
                bounce_vals = BOUNCE_RESULT.with(|cell| cell.borrow().clone());
            } else {
                force_x += bound_x;
                force_y += bound_y;
            }

            // Limit force magnitude
            let (limited_fx, limited_fy) = limit_magnitude(force_x, force_y, config.max_force);
            force_x = limited_fx;
            force_y = limited_fy;

            // Update velocity
            let new_vx = boid.vx + force_x * dt;
            let new_vy = boid.vy + force_y * dt;

            // Add random jitter
            let angle = (Math::random() as f32) * TAU;
            let jitter_x = angle.cos() * jitter;
            let jitter_y = angle.sin() * jitter;
            let mut vx = new_vx + jitter_x;
            let mut vy = new_vy + jitter_y;

            // Clamp to min speed
            let speed = (vx * vx + vy * vy).sqrt();
            if speed < min_speed {
                let scale = min_speed / speed.max(1e-6);
                vx *= scale;
                vy *= scale;
            }

            // Clamp to max speed
            let (limited_vx, limited_vy) = limit_magnitude(vx, vy, config.max_speed);

            // Update position
            let new_x = boid.x + limited_vx * dt;
            let new_y = boid.y + limited_vy * dt;

            if bounced {
                if let Some((bx, by, bvx, bvy)) = bounce_vals {
                    return BoidState {
                        x: bx,
                        y: by,
                        vx: bvx,
                        vy: bvy,
                    };
                }
            }

            if override_velocity {
                if let Some((_, _, vx, vy)) = bounce_vals {
                    // Override velocity, but keep position
                    return BoidState {
                        x: boid.x,
                        y: boid.y,
                        vx,
                        vy,
                    };
                }
            }

            BoidState {
                x: new_x,
                y: new_y,
                vx: limited_vx,
                vy: limited_vy,
            }
        })
        .collect();

    // Convert back to flat array
    let mut result = Vec::with_capacity(boid_count * 4);
    for state in updated_states {
        result.push(state.x);
        result.push(state.y);
        result.push(state.vx);
        result.push(state.vy);
    }

    result
}

/// Calculate boundary avoidance force with simple config
fn boundary_avoidance_simple(boid: &BoidState, config: &SimpleConfig, min_speed: f32) -> (f32, f32) {
    let width = config.world_width;
    let height = config.world_height;
    let margin = config.boundary_margin;
    let strength = config.boundary_strength;
    let mut force_x = 0.0;
    let mut force_y = 0.0;
    let mut bounced = false;
    let mut new_x = boid.x;
    let mut new_y = boid.y;
    let mut new_vx = boid.vx;
    let mut new_vy = boid.vy;
    let nudge = 5.0;
    let rng = js_sys::Math::random() as f32;
    let wall_mult = 10.0;
    let mut override_velocity = false;
    let mut override_vx = boid.vx;
    let mut override_vy = boid.vy;

    // Compute the strongest wall force direction (for override)
    let mut max_wall_force = 0.0;
    let mut wall_dir_x = 0.0;
    let mut wall_dir_y = 0.0;
    // Left
    if boid.x < margin {
        let d = (margin - boid.x).max(0.0) / margin;
        let f = wall_mult * strength * d.powi(4);
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
        let f = wall_mult * strength * d.powi(4);
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
        let f = wall_mult * strength * d.powi(4);
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
        let f = wall_mult * strength * d.powi(4);
        if f > max_wall_force {
            max_wall_force = f;
            wall_dir_x = 0.0;
            wall_dir_y = -1.0;
        }
        force_y -= f;
    }
    // If in the margin, blend wall force with current velocity instead of hard override
    if max_wall_force > 0.0 {
        // Compute desired direction away from wall
        let desired_mag = (max_wall_force + min_speed).max(min_speed);
        let desired_vx = wall_dir_x * desired_mag;
        let desired_vy = wall_dir_y * desired_mag;
        // Blend: new velocity = 50% current + 50% wall direction
        override_velocity = true;
        override_vx = 0.5 * boid.vx + 0.5 * desired_vx;
        override_vy = 0.5 * boid.vy + 0.5 * desired_vy;
        // Project blended velocity onto wall direction
        let wall_dot = override_vx * wall_dir_x + override_vy * wall_dir_y;
        if wall_dot < min_speed {
            // Set the wall direction component to min_speed, preserve tangential component
            let tangential_x = override_vx - wall_dot * wall_dir_x;
            let tangential_y = override_vy - wall_dot * wall_dir_y;
            override_vx = wall_dir_x * min_speed + tangential_x;
            override_vy = wall_dir_y * min_speed + tangential_y;
            // Optionally, renormalize if total speed is less than min_speed
            let speed = (override_vx * override_vx + override_vy * override_vy).sqrt();
            if speed < min_speed {
                let scale = min_speed / speed.max(1e-6);
                override_vx *= scale;
                override_vy *= scale;
            }
        }
    }

    // Hard bounce if out of bounds
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
        BOUNCE_RESULT.with(|cell| {
            *cell.borrow_mut() = Some((new_x, new_y, new_vx, new_vy));
        });
        (f32::NAN, f32::NAN)
    } else if override_velocity {
        // Use a special marker: force_x = INF, force_y = INF, and store override_vx/vy in BOUNCE_RESULT
        BOUNCE_RESULT.with(|cell| {
            *cell.borrow_mut() = Some((boid.x, boid.y, override_vx, override_vy));
        });
        (f32::INFINITY, f32::INFINITY)
    } else {
        (force_x, force_y)
    }
}
