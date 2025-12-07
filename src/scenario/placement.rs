// Turret placement system for preparation phase

use bevy::prelude::*;
use std::collections::HashMap;

use crate::terrain::TerrainHeightmap;
use crate::types::*;
use crate::turrets::{spawn_mg_turret_at, spawn_heavy_turret_at};
use crate::selection::screen_to_ground_with_heightmap;

use super::{ScenarioState, WaveManager, WaveState};

/// Turret placement system - handles mouse clicks during Preparation phase
/// LMB: Place turret (only if not clicking on a unit), RMB: Undo last placement (only if no squads selected)
pub fn turret_placement_system(
    mut wave_manager: ResMut<WaveManager>,
    scenario_state: Res<ScenarioState>,
    selection_state: Res<crate::selection::SelectionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    heightmap: Res<TerrainHeightmap>,
    unit_query: Query<(&Transform, &SquadMember), With<BattleDroid>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Only active during preparation phase
    if !scenario_state.active || wave_manager.wave_state != WaveState::Preparation {
        return;
    }

    // RMB: Undo last turret placement - but only if no squads are selected (let movement handle it)
    if mouse_button.just_pressed(MouseButton::Right) {
        // Only undo if no squads are selected - otherwise let movement system handle RMB
        if selection_state.selected_squads.is_empty() {
            if let Some(turret_entity) = wave_manager.placed_turrets.pop() {
                commands.entity(turret_entity).despawn();
                wave_manager.turrets_remaining += 1;
                info!("Undid turret placement ({} remaining)", wave_manager.turrets_remaining);
            }
        }
        return;
    }

    // No turrets left to place
    if wave_manager.turrets_remaining == 0 {
        return;
    }

    // Check for left mouse button click
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // Get cursor position
    let Ok(window) = window_query.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Convert to world position
    let Some(world_pos) = screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, Some(&heightmap)) else {
        return;
    };

    // Check if clicking on a friendly unit - if so, don't place turret (let selection handle it)
    let squad_centers = calculate_squad_centers_for_team(&unit_query, Team::A);
    if is_position_near_squad(world_pos, &squad_centers, crate::constants::SELECTION_CLICK_RADIUS) {
        // User clicked on a unit, let selection system handle it
        return;
    }

    // Spawn the selected turret type at click position and track it
    let turret_entity = if wave_manager.place_mg_turret {
        let entity = spawn_mg_turret_at(&mut commands, &mut meshes, &mut materials, world_pos);
        info!("Placed MG turret at {:?} ({} remaining)", world_pos, wave_manager.turrets_remaining - 1);
        entity
    } else {
        let entity = spawn_heavy_turret_at(&mut commands, &mut meshes, &mut materials, world_pos);
        info!("Placed Heavy turret at {:?} ({} remaining)", world_pos, wave_manager.turrets_remaining - 1);
        entity
    };

    wave_manager.placed_turrets.push(turret_entity);
    wave_manager.turrets_remaining -= 1;
}

/// Calculate squad centers for all friendly (Team::A) squads
/// Only calculates centers for squads with members present
fn calculate_squad_centers_for_team(
    unit_query: &Query<(&Transform, &SquadMember), With<BattleDroid>>,
    _team: Team,
) -> HashMap<u32, Vec3> {
    let mut squad_positions: HashMap<u32, Vec<Vec3>> = HashMap::new();

    // Collect all unit positions by squad
    // We'll filter by team A squads later (squads 0-5 are player garrison)
    for (transform, squad_member) in unit_query.iter() {
        // Player garrison squads have IDs 0-5 (6 squads of 50 = 300 units)
        // This is simpler than passing SquadManager through
        if squad_member.squad_id < 100 {
            squad_positions.entry(squad_member.squad_id)
                .or_default()
                .push(transform.translation);
        }
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

/// Check if a world position is near any squad center
fn is_position_near_squad(
    world_pos: Vec3,
    squad_centers: &HashMap<u32, Vec3>,
    max_distance: f32,
) -> bool {
    for center in squad_centers.values() {
        let dx = world_pos.x - center.x;
        let dz = world_pos.z - center.z;
        let distance = (dx * dx + dz * dz).sqrt();
        if distance < max_distance {
            return true;
        }
    }
    false
}
