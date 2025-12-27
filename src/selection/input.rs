// Selection input handling systems
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crate::types::*;
use crate::constants::*;
use crate::terrain::TerrainHeightmap;
use crate::artillery::{ArtilleryState, ArtilleryMode};

use super::state::{SelectionState, SelectionVisual};
use super::utils::{screen_to_ground_with_heightmap, calculate_squad_centers, find_squad_at_position};

/// Click radius for turret selection (turrets are larger than squad centers)
const TURRET_CLICK_RADIUS: f32 = 8.0;

/// System: Handle left-click selection input
pub fn selection_input_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    turret_query: Query<(Entity, &Transform, &TurretBase)>,
    squad_manager: Res<SquadManager>,
    mut selection_state: ResMut<SelectionState>,
    heightmap: Option<Res<TerrainHeightmap>>,
    artillery_state: Res<ArtilleryState>,
) {
    // Skip selection input when artillery mode is active
    if artillery_state.mode != ArtilleryMode::None {
        return;
    }

    let Ok(window) = window_query.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    // Get cursor position
    let Some(cursor_pos) = window.cursor_position() else { return };

    let hm = heightmap.as_ref().map(|h| h.as_ref());

    // Handle left mouse button press - start selection or box select
    if mouse_button.just_pressed(MouseButton::Left) {
        // Get world position for potential box select start
        if let Some(world_pos) = screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, hm) {
            selection_state.drag_start_world = Some(world_pos);
            selection_state.box_select_start = Some(cursor_pos);
        }
    }

    // Handle left mouse button release - finalize selection
    if mouse_button.just_released(MouseButton::Left) {
        if let Some(start_pos) = selection_state.box_select_start {
            let drag_distance = cursor_pos.distance(start_pos);

            if drag_distance < BOX_SELECT_DRAG_THRESHOLD {
                // This is a click, not a drag - do single selection
                if let Some(world_pos) = screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, hm) {
                    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

                    // First check for turret click (only player's turrets - Team::A)
                    let clicked_turret = turret_query.iter()
                        .filter(|(_, _, turret_base)| turret_base.team == Team::A)
                        .find(|(_, transform, _)| {
                            let dx = world_pos.x - transform.translation.x;
                            let dz = world_pos.z - transform.translation.z;
                            (dx * dx + dz * dz).sqrt() < TURRET_CLICK_RADIUS
                        })
                        .map(|(entity, _, _)| entity);

                    if let Some(turret_entity) = clicked_turret {
                        // Clicked on a turret - select it
                        if !shift_held {
                            selection_state.selected_squads.clear();
                        }
                        if selection_state.selected_turret == Some(turret_entity) && shift_held {
                            // Shift-click on already selected turret - deselect
                            selection_state.selected_turret = None;
                            info!("Deselected turret");
                        } else {
                            selection_state.selected_turret = Some(turret_entity);
                            info!("Selected turret {:?}", turret_entity);
                        }
                    } else {
                        // No turret clicked - check for squad
                        // Calculate actual squad centers from unit positions
                        let squad_centers = calculate_squad_centers(&unit_query);

                        // Only select player's team (Team::A)
                        if let Some(squad_id) = find_squad_at_position(world_pos, &squad_centers, &squad_manager, SELECTION_CLICK_RADIUS, Team::A) {
                            // Clear turret selection when selecting squads
                            selection_state.selected_turret = None;

                            // Check if clicked squad is part of a group
                            let group_squads = if let Some(&group_id) = selection_state.squad_to_group.get(&squad_id) {
                                if let Some(group) = selection_state.groups.get(&group_id) {
                                    Some(group.squad_ids.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if shift_held {
                                // Toggle selection (entire group if grouped)
                                if let Some(group_squads) = group_squads {
                                    // Check if entire group is already selected
                                    let all_selected = group_squads.iter().all(|id| selection_state.selected_squads.contains(id));
                                    if all_selected {
                                        // Deselect entire group
                                        for squad_id in &group_squads {
                                            selection_state.selected_squads.retain(|&id| id != *squad_id);
                                        }
                                        info!("Deselected group with {} squads", group_squads.len());
                                    } else {
                                        // Select entire group
                                        for squad_id in group_squads {
                                            if !selection_state.selected_squads.contains(&squad_id) {
                                                selection_state.selected_squads.push(squad_id);
                                            }
                                        }
                                        info!("Added group to selection ({} total)", selection_state.selected_squads.len());
                                    }
                                } else {
                                    // Single squad toggle
                                    if let Some(pos) = selection_state.selected_squads.iter().position(|&id| id == squad_id) {
                                        selection_state.selected_squads.remove(pos);
                                        info!("Deselected squad {}", squad_id);
                                    } else {
                                        selection_state.selected_squads.push(squad_id);
                                        info!("Added squad {} to selection ({} total)", squad_id, selection_state.selected_squads.len());
                                    }
                                }
                            } else {
                                // Clear and select (entire group if grouped)
                                selection_state.selected_squads.clear();
                                if let Some(group_squads) = group_squads {
                                    selection_state.selected_squads.extend(group_squads.iter());
                                    info!("Selected group with {} squads", group_squads.len());
                                } else {
                                    selection_state.selected_squads.push(squad_id);
                                    info!("Selected squad {}", squad_id);
                                }
                            }
                        } else if !shift_held {
                            // Clicked on empty ground without shift - clear selection
                            selection_state.selected_turret = None;
                            if !selection_state.selected_squads.is_empty() {
                                selection_state.selected_squads.clear();
                                info!("Selection cleared");
                            }
                        }
                    }
                }
            }
            // Note: Box selection is handled in box_selection_update_system
            // Don't clear state here if box selecting - let that system handle it
        }

        // Only clear drag state if NOT box selecting (box_selection_update_system needs it)
        if !selection_state.is_box_selecting {
            selection_state.box_select_start = None;
            selection_state.drag_start_world = None;
        }
    }
}

/// System: Handle box/drag selection
pub fn box_selection_update_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    squad_manager: Res<SquadManager>,
    mut selection_state: ResMut<SelectionState>,
    artillery_state: Res<ArtilleryState>,
) {
    // Skip box selection when artillery mode is active
    if artillery_state.mode != ArtilleryMode::None {
        // Clear any in-progress box selection state
        if selection_state.is_box_selecting {
            selection_state.box_select_start = None;
            selection_state.drag_start_world = None;
            selection_state.is_box_selecting = false;
        }
        return;
    }

    let Ok(window) = window_query.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };
    let Some(start_pos) = selection_state.box_select_start else { return };

    // Handle release first (before early return for pressed check)
    if mouse_button.just_released(MouseButton::Left) && selection_state.is_box_selecting {
        let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

        if !shift_held {
            selection_state.selected_squads.clear();
        }

        // Calculate screen-space box
        let min_x = start_pos.x.min(cursor_pos.x);
        let max_x = start_pos.x.max(cursor_pos.x);
        let min_y = start_pos.y.min(cursor_pos.y);
        let max_y = start_pos.y.max(cursor_pos.y);

        // Select all squads whose center projects into the box (only player's team with living units)
        let mut selected_count = 0;
        for (squad_id, squad) in squad_manager.squads.iter() {
            // Skip enemy squads (only select player's Team::A)
            if squad.team != Team::A {
                continue;
            }
            // Skip dead squads (no living units)
            if squad.members.is_empty() {
                continue;
            }
            // Project squad center to screen space
            if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, squad.center_position) {
                if screen_pos.x >= min_x && screen_pos.x <= max_x
                   && screen_pos.y >= min_y && screen_pos.y <= max_y {
                    // Only add if not already selected
                    if !selection_state.selected_squads.contains(squad_id) {
                        selection_state.selected_squads.push(*squad_id);
                        selected_count += 1;
                    }
                }
            }
        }

        // Expand selection to include all grouped squads
        let mut additional_squads = Vec::new();
        for &squad_id in &selection_state.selected_squads {
            if let Some(&group_id) = selection_state.squad_to_group.get(&squad_id) {
                if let Some(group) = selection_state.groups.get(&group_id) {
                    for &grouped_squad_id in &group.squad_ids {
                        if !selection_state.selected_squads.contains(&grouped_squad_id) {
                            additional_squads.push(grouped_squad_id);
                        }
                    }
                }
            }
        }
        selection_state.selected_squads.extend(additional_squads);

        if selected_count > 0 {
            info!("Box selected {} squads ({} total)", selected_count, selection_state.selected_squads.len());
        }

        // Clear state after box selection completes
        selection_state.box_select_start = None;
        selection_state.drag_start_world = None;
        selection_state.is_box_selecting = false;
        return;
    }

    // Check if we're dragging with left mouse
    if !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    let drag_distance = cursor_pos.distance(start_pos);

    // Start box selecting if drag exceeds threshold
    if drag_distance >= BOX_SELECT_DRAG_THRESHOLD && !selection_state.is_box_selecting {
        selection_state.is_box_selecting = true;
    }
}

/// System: Toggle Hold mode for selected squads with H key
pub fn hold_command_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut droid_query: Query<(&SquadMember, &mut MovementMode), With<BattleDroid>>,
) {
    if keyboard.just_pressed(KeyCode::KeyH) {
        if selection_state.selected_squads.is_empty() {
            return;
        }

        // Count current hold/move states to determine toggle direction
        let mut hold_count = 0;
        let mut move_count = 0;

        for &squad_id in &selection_state.selected_squads {
            if squad_manager.get_squad(squad_id).is_some() {
                // Count modes for units in this squad
                for (squad_member, mode) in droid_query.iter() {
                    if squad_member.squad_id == squad_id {
                        match *mode {
                            MovementMode::Hold => hold_count += 1,
                            _ => move_count += 1,
                        }
                    }
                }
            }
        }

        // Toggle: if more are holding, switch to Move; otherwise switch to Hold
        let new_mode = if hold_count > move_count {
            MovementMode::Move
        } else {
            MovementMode::Hold
        };

        // Apply new mode to all units in selected squads
        let mut units_affected = 0;
        for (squad_member, mut mode) in droid_query.iter_mut() {
            if selection_state.selected_squads.contains(&squad_member.squad_id) {
                *mode = new_mode;
                units_affected += 1;
            }
        }

        if units_affected > 0 {
            info!(
                "Toggled {} units to {:?} mode",
                units_affected,
                new_mode
            );
        }
    }
}
