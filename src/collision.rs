//! Unit-to-unit collision system
//!
//! M2TW-style mass-based collision where heavier units push lighter units more.
//! This creates tactical gameplay where heavy platforms can push through droid lines.

use bevy::prelude::*;
use crate::types::{BattleDroid, UnitMass, SpatialGrid, KnockbackState, RagdollDeath};
use crate::constants::{UNIT_COLLISION_RADIUS, COLLISION_PUSH_STRENGTH, DEFAULT_UNIT_MASS};

/// Hard collision resolution system - pushes overlapping units apart
/// Runs after animate_march to resolve any remaining overlaps
pub fn unit_collision_system(
    time: Res<Time>,
    spatial_grid: Res<SpatialGrid>,
    mut droids: Query<
        (Entity, &mut Transform, Option<&UnitMass>),
        (With<BattleDroid>, Without<KnockbackState>, Without<RagdollDeath>)
    >,
) {
    let delta = time.delta_secs();
    if delta <= 0.0 {
        return;
    }

    // Collect all droid positions and masses first (avoid borrow conflicts)
    let droid_data: Vec<(Entity, Vec3, f32)> = droids
        .iter()
        .map(|(e, t, m)| (e, t.translation, m.map(|m| m.0).unwrap_or(DEFAULT_UNIT_MASS)))
        .collect();

    // Build a lookup for positions by entity
    let pos_lookup: std::collections::HashMap<Entity, (Vec3, f32)> = droid_data
        .iter()
        .map(|(e, pos, mass)| (*e, (*pos, *mass)))
        .collect();

    // Collect all push forces to apply
    let mut pushes: Vec<(Entity, Vec3)> = Vec::new();
    let collision_dist = UNIT_COLLISION_RADIUS * 2.0;

    for (entity, pos, mass) in &droid_data {
        let nearby = spatial_grid.get_nearby_droids(*pos);

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
                if dist_sq < collision_dist * collision_dist && dist_sq > 0.0001 {
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

                    pushes.push((*entity, push_dir * push_magnitude * my_ratio));
                    pushes.push((other_entity, -push_dir * push_magnitude * their_ratio));
                }
            }
        }
    }

    // Apply all pushes
    for (entity, push) in pushes {
        if let Ok((_, mut transform, _)) = droids.get_mut(entity) {
            transform.translation.x += push.x * delta;
            transform.translation.z += push.z * delta;
        }
    }
}
