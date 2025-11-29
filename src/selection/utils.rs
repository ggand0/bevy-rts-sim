// Shared utility functions for selection module
use bevy::prelude::*;
use std::collections::HashMap;
use crate::types::*;

/// Convert screen cursor position to world position on the ground plane (Y = -1.0)
pub fn screen_to_ground(
    cursor_pos: Vec2,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec3> {
    // Get ray from camera through cursor position
    let ray = camera.viewport_to_world(camera_transform, cursor_pos)?;

    // Intersect with ground plane at Y = -1.0
    let ground_y = -1.0;

    // Ray equation: P = origin + t * direction
    // Plane equation: P.y = ground_y
    // Solve: origin.y + t * direction.y = ground_y
    // t = (ground_y - origin.y) / direction.y

    if ray.direction.y.abs() < 0.0001 {
        // Ray is parallel to ground, no intersection
        return None;
    }

    let t = (ground_y - ray.origin.y) / ray.direction.y;

    if t > 0.0 {
        Some(ray.origin + ray.direction * t)
    } else {
        // Intersection is behind camera
        None
    }
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

        let distance = Vec3::new(
            world_pos.x - center.x,
            0.0,  // Ignore Y for horizontal distance
            world_pos.z - center.z,
        ).length();

        if distance < closest_distance {
            closest_distance = distance;
            closest_squad = Some(*squad_id);
        }
    }

    closest_squad
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

    Vec3::new(
        destination.x - avg_pos.x,
        0.0,
        destination.z - avg_pos.z,
    ).normalize_or_zero()
}
