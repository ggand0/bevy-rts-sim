// Movement command systems
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crate::types::*;
use crate::constants::*;
use crate::formation::calculate_formation_offset;
use crate::terrain::TerrainHeightmap;

use super::state::{SelectionState, OrientationArrowVisual};
use super::groups::check_is_complete_group;
use super::utils::{screen_to_ground_with_heightmap, calculate_default_facing, calculate_filtered_squad_centers, horizontal_distance, horizontal_direction};
use super::visuals::{spawn_move_indicator, spawn_move_indicator_with_color, spawn_path_line};

/// System: Handle right-click move commands for selected squads
/// Supports drag-to-set-orientation (CoH1-style)
pub fn move_command_system(
    mut commands: Commands,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut squad_manager: ResMut<SquadManager>,
    mut selection_state: ResMut<SelectionState>,
    mut droid_query: Query<(Entity, &mut BattleDroid, &SquadMember, &FormationOffset, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    arrow_query: Query<Entity, With<OrientationArrowVisual>>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    let Ok(window) = window_query.get_single() else { return };
    let Ok((camera, camera_transform)) = camera_query.get_single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    let hm = heightmap.as_ref().map(|h| h.as_ref());

    // Get current world position under cursor
    let current_world_pos = screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, hm);

    // Handle right mouse button press - start potential drag
    if mouse_button.just_pressed(MouseButton::Right) {
        if !selection_state.selected_squads.is_empty() {
            if let Some(pos) = current_world_pos {
                selection_state.move_drag_start = Some(pos);
                selection_state.move_drag_current = Some(pos);
                selection_state.is_orientation_dragging = false;
            }
        }
    }

    // Handle right mouse button held - update drag position
    if mouse_button.pressed(MouseButton::Right) {
        if let (Some(start), Some(current)) = (selection_state.move_drag_start, current_world_pos) {
            selection_state.move_drag_current = Some(current);

            // Check if drag exceeds threshold
            let drag_distance = horizontal_distance(start, current);
            if drag_distance > super::state::ORIENTATION_DRAG_THRESHOLD {
                selection_state.is_orientation_dragging = true;
            }
        }
    }

    // Handle right mouse button release - execute move command
    if mouse_button.just_released(MouseButton::Right) {
        // Clean up any existing arrow visual
        for entity in arrow_query.iter() {
            commands.entity(entity).despawn();
        }

        let Some(destination) = selection_state.move_drag_start else {
            // Clear state and return
            selection_state.move_drag_start = None;
            selection_state.move_drag_current = None;
            selection_state.is_orientation_dragging = false;
            return;
        };

        if selection_state.selected_squads.is_empty() {
            selection_state.move_drag_start = None;
            selection_state.move_drag_current = None;
            selection_state.is_orientation_dragging = false;
            return;
        }

        // Determine facing direction
        let unified_facing = if selection_state.is_orientation_dragging {
            // Use drag direction for orientation
            if let Some(current) = selection_state.move_drag_current {
                let drag_dir = horizontal_direction(destination, current);
                if drag_dir.length() > 0.1 {
                    drag_dir.normalize()
                } else {
                    // Fallback to movement direction
                    calculate_default_facing(&selection_state.selected_squads, &squad_manager, destination)
                }
            } else {
                calculate_default_facing(&selection_state.selected_squads, &squad_manager, destination)
            }
        } else {
            // No drag - use default facing (toward destination from average position)
            calculate_default_facing(&selection_state.selected_squads, &squad_manager, destination)
        };

        info!("Move command to ({:.1}, {:.1}) for {} squads, orientation: ({:.2}, {:.2})",
              destination.x, destination.z, selection_state.selected_squads.len(),
              unified_facing.x, unified_facing.z);

        // Execute the move command
        execute_move_command(
            &mut commands,
            &mut squad_manager,
            &mut selection_state,
            &mut droid_query,
            &mut meshes,
            &mut materials,
            destination,
            unified_facing,
        );

        // Clear drag state
        selection_state.move_drag_start = None;
        selection_state.move_drag_current = None;
        selection_state.is_orientation_dragging = false;
    }
}

/// Execute a group move - maintains relative formation positions
fn execute_group_move(
    commands: &mut Commands,
    squad_manager: &mut ResMut<SquadManager>,
    selection_state: &mut SelectionState,
    droid_query: &mut Query<(Entity, &mut BattleDroid, &SquadMember, &FormationOffset, &Transform)>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    destination: Vec3,
    unified_facing: Vec3,
    group_id: u32,
) {
    let Some(group) = selection_state.groups.get_mut(&group_id) else { return };

    // Get the ORIGINAL formation facing (from when group was created - this never changes)
    let original_facing = group.original_formation_facing;

    // Calculate rotation from ORIGINAL facing to the NEW unified facing
    // This ensures we always rotate from the same base coordinate system
    let rotation = if original_facing.length() > 0.1 && unified_facing.length() > 0.1 {
        // Calculate the rotation quaternion that rotates original_facing to unified_facing
        // For XZ plane: angle from +Z axis is atan2(x, z)
        let original_angle = original_facing.x.atan2(original_facing.z);
        let new_angle = unified_facing.x.atan2(unified_facing.z);
        Quat::from_rotation_y(new_angle - original_angle)
    } else {
        Quat::IDENTITY
    };

    // Update the group's CURRENT formation_facing (for orientation indicator)
    if unified_facing.length() > 0.1 {
        group.formation_facing = unified_facing;
    }

    // Calculate actual current positions for path visuals
    let group_squad_ids = group.squad_ids.clone();
    let squad_current_positions = calculate_filtered_squad_centers(
        droid_query.iter().map(|(_, _, sm, _, t)| (sm.squad_id, t.translation)),
        |id| group_squad_ids.contains(&id),
        squad_manager,
    );

    // Apply rotated offsets to each squad
    for (&squad_id, &offset) in &group.squad_offsets {
        // Rotate the offset
        let rotated_offset = rotation * offset;
        let squad_dest = destination + rotated_offset;

        // Check if squad is alive (has members)
        let is_alive = squad_manager.get_squad(squad_id)
            .map_or(false, |s| !s.members.is_empty());

        if is_alive {
            if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
                // Set facing direction
                if unified_facing.length() > 0.1 {
                    squad.target_facing_direction = unified_facing;
                    squad.facing_direction = unified_facing;
                }

                // Set target position
                squad.target_position = squad_dest;

                // Spawn green visual indicator for living squad
                spawn_move_indicator(commands, meshes, materials, squad_dest);

                if let Some(&start_pos) = squad_current_positions.get(&squad_id) {
                    spawn_path_line(commands, meshes, materials, start_pos, squad_dest);
                }
            }
        } else {
            // Dead squad - spawn grey indicator to show where it would have been
            let dead_color = Color::srgba(0.4, 0.4, 0.4, 0.8);
            spawn_move_indicator_with_color(commands, meshes, materials, squad_dest, Some(dead_color));
        }
    }

    // Update individual unit targets using standard formation calculation
    for (_entity, mut droid, squad_member, _formation_offset, _transform) in droid_query.iter_mut() {
        if group.squad_ids.contains(&squad_member.squad_id) {
            if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
                // Calculate formation offset with new facing direction
                let new_offset = calculate_formation_offset(
                    squad.formation_type,
                    squad_member.formation_position.0,
                    squad_member.formation_position.1,
                    squad.facing_direction,
                );

                // Set target position with formation offset, preserving the unit's spawn Y height
                let target_xz = squad.target_position + new_offset;
                droid.target_position = Vec3::new(target_xz.x, droid.spawn_position.y, target_xz.z);
                droid.returning_to_spawn = false;
            }
        }
    }

    info!("Group {} moved to destination with maintained formation", group_id);
}

fn execute_move_command(
    commands: &mut Commands,
    squad_manager: &mut ResMut<SquadManager>,
    selection_state: &mut SelectionState,
    droid_query: &mut Query<(Entity, &mut BattleDroid, &SquadMember, &FormationOffset, &Transform)>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    destination: Vec3,
    unified_facing: Vec3,
) {
    // Check if this is a complete group move
    if let Some(group_id) = check_is_complete_group(selection_state, squad_manager) {
        // Group move - maintain relative positions
        execute_group_move(
            commands,
            squad_manager,
            selection_state,
            droid_query,
            meshes,
            materials,
            destination,
            unified_facing,
            group_id,
        );
        return;
    }

    // Regular move - create line formation
    // Perpendicular direction for spreading squads in a line (orthogonal to facing)
    let spread_direction = Vec3::new(unified_facing.z, 0.0, -unified_facing.x);

    let num_squads = selection_state.selected_squads.len();

    // Pre-calculate all destination slots
    let destination_slots: Vec<Vec3> = (0..num_squads)
        .map(|index| {
            let offset = if num_squads > 1 {
                let centered_index = index as f32 - (num_squads - 1) as f32 / 2.0;
                spread_direction * centered_index * MULTI_SQUAD_SPACING
            } else {
                Vec3::ZERO
            };
            destination + offset
        })
        .collect();

    // Calculate actual current positions of squads from unit transforms (not squad.center_position which lags)
    let selected = selection_state.selected_squads.clone();
    let squad_current_positions = calculate_filtered_squad_centers(
        droid_query.iter().map(|(_, _, sm, _, t)| (sm.squad_id, t.translation)),
        |id| selected.contains(&id),
        squad_manager,
    );

    // Collect squad IDs and their current positions
    let squad_positions: Vec<(u32, Vec3)> = selection_state.selected_squads.iter()
        .filter_map(|&id| squad_current_positions.get(&id).map(|&pos| (id, pos)))
        .collect();

    // Greedy assignment: assign each squad to its closest available destination
    let mut assigned_destinations: Vec<(u32, Vec3)> = Vec::with_capacity(num_squads);
    let mut available_slots: Vec<Vec3> = destination_slots.clone();

    // Sort squads by distance to destination center (closest first gets priority)
    let mut sorted_squads = squad_positions.clone();
    sorted_squads.sort_by(|a, b| {
        let dist_a = horizontal_distance(a.1, destination);
        let dist_b = horizontal_distance(b.1, destination);
        dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    for (squad_id, squad_pos) in sorted_squads {
        // Find the closest available slot
        let mut best_slot_idx = 0;
        let mut best_distance = f32::MAX;

        for (idx, &slot) in available_slots.iter().enumerate() {
            let dist = horizontal_distance(squad_pos, slot);
            if dist < best_distance {
                best_distance = dist;
                best_slot_idx = idx;
            }
        }

        if !available_slots.is_empty() {
            let chosen_slot = available_slots.remove(best_slot_idx);
            assigned_destinations.push((squad_id, chosen_slot));
        }
    }

    // Use actual current positions for path visuals (already calculated above)
    let squad_start_positions = squad_current_positions.clone();

    // Apply the assignments
    for (squad_id, squad_destination) in assigned_destinations.iter() {
        if let Some(squad) = squad_manager.get_squad_mut(*squad_id) {
            // ALL squads face the same unified direction
            if unified_facing.length() > 0.1 {
                squad.target_facing_direction = unified_facing;
                squad.facing_direction = unified_facing;
            }

            // Set target position
            squad.target_position = *squad_destination;
        }
    }

    // Update individual unit targets
    for (_entity, mut droid, squad_member, _formation_offset, _transform) in droid_query.iter_mut() {
        if selection_state.selected_squads.contains(&squad_member.squad_id) {
            if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
                // Calculate new formation offset with new facing direction
                let new_offset = calculate_formation_offset(
                    squad.formation_type,
                    squad_member.formation_position.0,
                    squad_member.formation_position.1,
                    squad.facing_direction,
                );

                // Set target position with formation offset, preserving the unit's spawn Y height
                let target_xz = squad.target_position + new_offset;
                droid.target_position = Vec3::new(target_xz.x, droid.spawn_position.y, target_xz.z);
                droid.returning_to_spawn = false;
            }
        }
    }

    // Spawn move indicator visuals for each squad
    for (squad_id, squad_destination) in assigned_destinations.iter() {
        // Spawn destination circle
        spawn_move_indicator(commands, meshes, materials, *squad_destination);

        // Spawn path line from squad current position to destination
        if let Some(&start_pos) = squad_start_positions.get(squad_id) {
            spawn_path_line(commands, meshes, materials, start_pos, *squad_destination);
        }
    }
}
