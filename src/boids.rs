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
            let (bound_x, bound_y) = boundary_avoidance_simple(boid, &config);

            // Combine forces
            let mut force_x = sep_x * config.separation_strength
                + align_x * config.alignment_strength
                + coh_x * config.cohesion_strength
                + bound_x;
            let mut force_y = sep_y * config.separation_strength
                + align_y * config.alignment_strength
                + coh_y * config.cohesion_strength
                + bound_y;

            // Limit force magnitude
            let (limited_fx, limited_fy) = limit_magnitude(force_x, force_y, config.max_force);
            force_x = limited_fx;
            force_y = limited_fy;

            // Update velocity
            let mut new_vx = boid.vx + force_x * dt;
            let mut new_vy = boid.vy + force_y * dt;

            // Limit velocity magnitude
            let (limited_vx, limited_vy) = limit_magnitude(new_vx, new_vy, config.max_speed);
            new_vx = limited_vx;
            new_vy = limited_vy;

            // Update position
            let new_x = boid.x + new_vx * dt;
            let new_y = boid.y + new_vy * dt;

            BoidState {
                x: new_x,
                y: new_y,
                vx: new_vx,
                vy: new_vy,
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

/// Calculate boundary avoidance force with simple config
fn boundary_avoidance_simple(boid: &BoidState, config: &SimpleConfig) -> (f32, f32) {
    let mut force_x = 0.0;
    let mut force_y = 0.0;

    // Left boundary
    if boid.x < config.boundary_margin {
        force_x += (config.boundary_margin - boid.x) / config.boundary_margin;
    }
    // Right boundary
    if boid.x > config.world_width - config.boundary_margin {
        force_x -= (boid.x - (config.world_width - config.boundary_margin)) / config.boundary_margin;
    }
    // Top boundary
    if boid.y < config.boundary_margin {
        force_y += (config.boundary_margin - boid.y) / config.boundary_margin;
    }
    // Bottom boundary
    if boid.y > config.world_height - config.boundary_margin {
        force_y -= (boid.y - (config.world_height - config.boundary_margin)) / config.boundary_margin;
    }

    (force_x * config.boundary_strength, force_y * config.boundary_strength)
}
