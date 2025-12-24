//! Unit-to-unit collision system
//!
//! M2TW-style mass-based collision where heavier units push lighter units more.
//! This creates tactical gameplay where heavy platforms can push through droid lines.

use bevy::prelude::*;
use rayon::prelude::*;
use crate::types::{BattleDroid, UnitMass, SpatialGrid, KnockbackState, RagdollDeath};
use crate::constants::{
    COLLISION_ENABLED, UNIT_COLLISION_RADIUS, COLLISION_PUSH_STRENGTH,
    DEFAULT_UNIT_MASS, COLLISION_FRAME_SKIP, STATIONARY_THRESHOLD
};

/// Hard collision resolution system - pushes overlapping units apart
/// Runs after animate_march to resolve any remaining overlaps
/// Uses parallel iteration for performance with 10k+ units
/// Frame-skipped: runs every COLLISION_FRAME_SKIP frames
pub fn unit_collision_system(
    time: Res<Time>,
    spatial_grid: Res<SpatialGrid>,
    mut frame_counter: Local<u32>,
    mut droids: Query<
        (Entity, &mut Transform, &BattleDroid, Option<&UnitMass>),
        (Without<KnockbackState>, Without<RagdollDeath>)
    >,
) {
    // Master toggle - skip entirely if disabled
    if !COLLISION_ENABLED {
        return;
    }

    // Frame skipping - only run every Nth frame
    *frame_counter = (*frame_counter + 1) % COLLISION_FRAME_SKIP;
    if *frame_counter != 0 {
        return;
    }

    let delta = time.delta_secs() * COLLISION_FRAME_SKIP as f32; // Compensate for skipped frames
    if delta <= 0.0 {
        return;
    }

    // Single pass: collect all droids and determine which are moving
    // This avoids iterating over 10k units twice
    let mut moving_droids: Vec<(Entity, Vec3, f32)> = Vec::new();
    let mut pos_lookup: std::collections::HashMap<Entity, (Vec3, f32)> = std::collections::HashMap::new();

    for (entity, transform, droid, mass) in droids.iter() {
        let pos = transform.translation;
        let mass_val = mass.map(|m| m.0).unwrap_or(DEFAULT_UNIT_MASS);

        // Add to lookup (all droids)
        pos_lookup.insert(entity, (pos, mass_val));

        // Check if moving
        let dx = droid.target_position.x - droid.spawn_position.x;
        let dz = droid.target_position.z - droid.spawn_position.z;
        let target_spawn_dist = (dx * dx + dz * dz).sqrt();
        if target_spawn_dist > STATIONARY_THRESHOLD || droid.returning_to_spawn {
            moving_droids.push((entity, pos, mass_val));
        }
    }

    let collision_dist = UNIT_COLLISION_RADIUS * 2.0;
    let collision_dist_sq = collision_dist * collision_dist;

    // Parallel collision detection - only moving units initiate collision checks
    // but they still collide with stationary neighbors
    let pushes: Vec<(Entity, Vec3)> = moving_droids
        .par_iter()
        .flat_map(|(entity, pos, mass)| {
            let nearby = spatial_grid.get_nearby_droids(*pos);
            let mut local_pushes = Vec::new();

            for other_entity in nearby {
                // Only process each pair once (entity < other_entity)
                if other_entity <= *entity {
                    continue;
                }

                if let Some((other_pos, other_mass)) = pos_lookup.get(&other_entity) {
                    let dx = pos.x - other_pos.x;
                    let dz = pos.z - other_pos.z;
                    let dist_sq = dx * dx + dz * dz;

                    // Check if overlapping (using squared distance for performance)
                    if dist_sq < collision_dist_sq && dist_sq > 0.0001 {
                        let dist = dist_sq.sqrt();
                        let overlap = collision_dist - dist;

                        // Normalize direction
                        let push_dir = Vec3::new(dx / dist, 0.0, dz / dist);

                        // Mass-based push distribution (M2TW style)
                        // Heavier unit pushes lighter unit more
                        let total_mass = mass + other_mass;
                        let my_ratio = other_mass / total_mass;    // I get pushed by their mass
                        let their_ratio = mass / total_mass;        // They get pushed by my mass

                        let push_magnitude = overlap * COLLISION_PUSH_STRENGTH;

                        local_pushes.push((*entity, push_dir * push_magnitude * my_ratio));
                        local_pushes.push((other_entity, -push_dir * push_magnitude * their_ratio));
                    }
                }
            }
            local_pushes
        })
        .collect();

    // Apply all pushes (sequential - fast, just writes)
    for (entity, push) in pushes {
        if let Ok((_, mut transform, _, _)) = droids.get_mut(entity) {
            transform.translation.x += push.x * delta;
            transform.translation.z += push.z * delta;
        }
    }
}
