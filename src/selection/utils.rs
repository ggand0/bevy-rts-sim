// Shared utility functions for selection module
use bevy::prelude::*;
use std::collections::HashMap;
use crate::types::*;
use crate::terrain::TerrainHeightmap;

/// Calculate horizontal distance between two points (ignoring Y axis)
#[inline]
pub fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Calculate horizontal direction from point a to point b (ignoring Y axis)
#[inline]
pub fn horizontal_direction(from: Vec3, to: Vec3) -> Vec3 {
    Vec3::new(to.x - from.x, 0.0, to.z - from.z)
}

/// Convert screen cursor position to world position on the terrain surface
/// Uses iterative raycasting against the terrain heightmap for accurate placement
#[allow(dead_code)]
pub fn screen_to_ground(
    cursor_pos: Vec2,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec3> {
    screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, None)
}

/// Convert screen cursor position to world position on the terrain surface
/// Uses the terrain heightmap for accurate Y positioning
pub fn screen_to_ground_with_heightmap(
    cursor_pos: Vec2,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    heightmap: Option<&TerrainHeightmap>,
) -> Option<Vec3> {
    // Get ray from camera through cursor position
    let ray = camera.viewport_to_world(camera_transform, cursor_pos).ok()?;

    if ray.direction.y.abs() < 0.0001 {
        // Ray is parallel to ground, no intersection
        return None;
    }

    // First, get approximate intersection with Y=0 plane
    let t_ground = -ray.origin.y / ray.direction.y;
    if t_ground <= 0.0 {
        return None;
    }

    let mut hit_point = ray.origin + ray.direction * t_ground;

    // If we have a heightmap, refine the hit point using iterative search
    if let Some(hm) = heightmap {
        // Binary search along the ray to find terrain intersection
        let mut t_min = 0.0f32;
        let mut t_max = t_ground * 2.0; // Search a bit beyond the flat ground intersection

        for _ in 0..16 { // 16 iterations should be plenty for good accuracy
            let t_mid = (t_min + t_max) / 2.0;
            let test_point = ray.origin + ray.direction * t_mid;
            let terrain_height = hm.sample_height(test_point.x, test_point.z);

            if test_point.y > terrain_height {
                // We're above terrain, search further along ray
                t_min = t_mid;
            } else {
                // We're below terrain, search closer
                t_max = t_mid;
            }
        }

        // Final intersection point
        hit_point = ray.origin + ray.direction * ((t_min + t_max) / 2.0);
        hit_point.y = hm.sample_height(hit_point.x, hit_point.z);
    }

    Some(hit_point)
}

/// Calculate actual squad centers from unit positions
pub fn calculate_squad_centers(
    unit_query: &Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<super::state::SelectionVisual>)>,
) -> HashMap<u32, Vec3> {
    let mut squad_positions: HashMap<u32, Vec<Vec3>> = HashMap::new();

    for (transform, squad_member) in unit_query.iter() {
        squad_positions.entry(squad_member.squad_id)
            .or_insert_with(Vec::new)
            .push(transform.translation);
    }

    let mut centers = HashMap::new();
    for (squad_id, positions) in squad_positions {
        if !positions.is_empty() {
            let sum: Vec3 = positions.iter().sum();
            centers.insert(squad_id, sum / positions.len() as f32);
        }
    }
    centers
}

/// Find the squad closest to a world position (for click selection)
/// Uses actual unit positions, not anchored squad.center_position
/// Only considers squads from the specified team (player team)
pub fn find_squad_at_position(
    world_pos: Vec3,
    squad_centers: &HashMap<u32, Vec3>,
    squad_manager: &SquadManager,
    max_distance: f32,
    player_team: Team,
) -> Option<u32> {
    let mut closest_squad: Option<u32> = None;
    let mut closest_distance = max_distance;

    for (squad_id, center) in squad_centers.iter() {
        // Only allow selecting player's team
        if let Some(squad) = squad_manager.get_squad(*squad_id) {
            if squad.team != player_team {
                continue;
            }
        }

        let distance = horizontal_distance(world_pos, *center);

        if distance < closest_distance {
            closest_distance = distance;
            closest_squad = Some(*squad_id);
        }
    }

    closest_squad
}

/// Calculate squad centers from unit positions for a filtered set of squads.
/// Takes an iterator of (squad_id, position) and a filter closure for which squads to include.
/// Uses squad_manager to get member counts for proper averaging.
pub fn calculate_filtered_squad_centers<I, F>(
    positions: I,
    filter: F,
    _squad_manager: &SquadManager,
) -> HashMap<u32, Vec3>
where
    I: Iterator<Item = (u32, Vec3)>,
    F: Fn(u32) -> bool,
{
    let mut squad_sums: HashMap<u32, Vec3> = HashMap::new();
    let mut squad_counts: HashMap<u32, usize> = HashMap::new();

    for (squad_id, position) in positions {
        if filter(squad_id) {
            *squad_sums.entry(squad_id).or_insert(Vec3::ZERO) += position;
            *squad_counts.entry(squad_id).or_insert(0) += 1;
        }
    }

    let mut centers = HashMap::new();
    for (squad_id, sum) in squad_sums {
        if let Some(&count) = squad_counts.get(&squad_id) {
            if count > 0 {
                centers.insert(squad_id, sum / count as f32);
            }
        }
    }
    centers
}

/// Calculate default facing direction (from average squad position toward destination)
pub fn calculate_default_facing(
    selected_squads: &[u32],
    squad_manager: &SquadManager,
    destination: Vec3,
) -> Vec3 {
    let mut avg_pos = Vec3::ZERO;
    let mut count = 0;
    for &squad_id in selected_squads.iter() {
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            avg_pos += squad.center_position;
            count += 1;
        }
    }
    if count > 0 {
        avg_pos /= count as f32;
    }

    horizontal_direction(avg_pos, destination).normalize_or_zero()
}
