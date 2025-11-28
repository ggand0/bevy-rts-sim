// Selection and command systems for RTS controls
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;
use crate::types::*;
use crate::constants::*;
use crate::formation::calculate_formation_offset;

// Selection state resource - tracks which squads are selected
#[derive(Resource, Default)]
pub struct SelectionState {
    pub selected_squads: HashSet<u32>,
    pub box_select_start: Option<Vec2>,  // Screen-space start position for box selection
    pub is_box_selecting: bool,
    pub drag_start_world: Option<Vec3>,  // World position where drag started
}

// Marker component for selection ring visuals
#[derive(Component)]
pub struct SelectionVisual {
    pub squad_id: u32,
}

// Marker component for move order destination indicator
#[derive(Component)]
pub struct MoveOrderVisual {
    pub timer: Timer,
}

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
fn calculate_squad_centers(
    unit_query: &Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
) -> std::collections::HashMap<u32, Vec3> {
    let mut squad_positions: std::collections::HashMap<u32, Vec<Vec3>> = std::collections::HashMap::new();

    for (transform, squad_member) in unit_query.iter() {
        squad_positions.entry(squad_member.squad_id)
            .or_insert_with(Vec::new)
            .push(transform.translation);
    }

    let mut centers = std::collections::HashMap::new();
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
pub fn find_squad_at_position(
    world_pos: Vec3,
    squad_centers: &std::collections::HashMap<u32, Vec3>,
    max_distance: f32,
) -> Option<u32> {
    let mut closest_squad: Option<u32> = None;
    let mut closest_distance = max_distance;

    for (squad_id, center) in squad_centers.iter() {
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

/// System: Handle left-click selection input
pub fn selection_input_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    mut selection_state: ResMut<SelectionState>,
) {
    let Ok(window) = window_query.get_single() else { return };
    let Ok((camera, camera_transform)) = camera_query.get_single() else { return };

    // Get cursor position
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Handle left mouse button press - start selection or box select
    if mouse_button.just_pressed(MouseButton::Left) {
        // Get world position for potential box select start
        if let Some(world_pos) = screen_to_ground(cursor_pos, camera, camera_transform) {
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
                if let Some(world_pos) = screen_to_ground(cursor_pos, camera, camera_transform) {
                    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

                    // Calculate actual squad centers from unit positions
                    let squad_centers = calculate_squad_centers(&unit_query);

                    if let Some(squad_id) = find_squad_at_position(world_pos, &squad_centers, SELECTION_CLICK_RADIUS) {
                        if shift_held {
                            // Toggle selection
                            if selection_state.selected_squads.contains(&squad_id) {
                                selection_state.selected_squads.remove(&squad_id);
                                info!("Deselected squad {}", squad_id);
                            } else {
                                selection_state.selected_squads.insert(squad_id);
                                info!("Added squad {} to selection ({} total)", squad_id, selection_state.selected_squads.len());
                            }
                        } else {
                            // Clear and select single squad
                            selection_state.selected_squads.clear();
                            selection_state.selected_squads.insert(squad_id);
                            info!("Selected squad {}", squad_id);
                        }
                    } else if !shift_held {
                        // Clicked on empty ground without shift - clear selection
                        if !selection_state.selected_squads.is_empty() {
                            selection_state.selected_squads.clear();
                            info!("Selection cleared");
                        }
                    }
                }
            }
            // Box selection handled in box_selection_update_system
        }

        // Clear drag state
        selection_state.box_select_start = None;
        selection_state.drag_start_world = None;
        selection_state.is_box_selecting = false;
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
) {
    let Ok(window) = window_query.get_single() else { return };
    let Ok((camera, camera_transform)) = camera_query.get_single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Check if we're dragging with left mouse
    if !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    let Some(start_pos) = selection_state.box_select_start else { return };
    let drag_distance = cursor_pos.distance(start_pos);

    // Start box selecting if drag exceeds threshold
    if drag_distance >= BOX_SELECT_DRAG_THRESHOLD && !selection_state.is_box_selecting {
        selection_state.is_box_selecting = true;
    }

    // If box selecting, select squads on release (handled in selection_input_system)
    // Here we just track that we're in box select mode

    // When released with box select active, select all squads in the box
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

        // Select all squads whose center projects into the box
        let mut selected_count = 0;
        for (squad_id, squad) in squad_manager.squads.iter() {
            // Project squad center to screen space
            if let Some(screen_pos) = camera.world_to_viewport(camera_transform, squad.center_position) {
                if screen_pos.x >= min_x && screen_pos.x <= max_x
                   && screen_pos.y >= min_y && screen_pos.y <= max_y {
                    selection_state.selected_squads.insert(*squad_id);
                    selected_count += 1;
                }
            }
        }

        if selected_count > 0 {
            info!("Box selected {} squads ({} total)", selected_count, selection_state.selected_squads.len());
        }
    }
}

/// System: Handle right-click move commands for selected squads
pub fn move_command_system(
    mut commands: Commands,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut squad_manager: ResMut<SquadManager>,
    selection_state: Res<SelectionState>,
    mut droid_query: Query<(&mut BattleDroid, &SquadMember, &FormationOffset)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Only process if right mouse button just pressed
    if !mouse_button.just_pressed(MouseButton::Right) {
        return;
    }

    // Need selected squads to command
    if selection_state.selected_squads.is_empty() {
        return;
    }

    let Ok(window) = window_query.get_single() else { return };
    let Ok((camera, camera_transform)) = camera_query.get_single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Raycast to ground
    let Some(destination) = screen_to_ground(cursor_pos, camera, camera_transform) else { return };

    info!("Move command to ({:.1}, {:.1}) for {} squads",
          destination.x, destination.z, selection_state.selected_squads.len());

    // Issue move commands to all selected squads
    for &squad_id in selection_state.selected_squads.iter() {
        if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
            // Calculate target facing direction (toward destination) for smooth rotation
            let direction = Vec3::new(
                destination.x - squad.center_position.x,
                0.0,
                destination.z - squad.center_position.z,
            );

            if direction.length() > 0.1 {
                // Set target facing for smooth interpolation (don't change facing_direction directly)
                squad.target_facing_direction = direction.normalize();
            }

            // Set target position
            squad.target_position = destination;
        }
    }

    // Update individual unit targets
    for (mut droid, squad_member, _formation_offset) in droid_query.iter_mut() {
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

    // Spawn move indicator visual
    spawn_move_indicator(&mut commands, &mut meshes, &mut materials, destination);
}

/// Spawn a visual indicator at the move destination
fn spawn_move_indicator(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
) {
    use bevy::pbr::{NotShadowCaster, NotShadowReceiver};

    // Create a flat circle on the ground
    let mesh = meshes.add(Circle::new(MOVE_INDICATOR_RADIUS));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 1.0, 0.3, 0.6),
        emissive: LinearRgba::new(0.1, 0.5, 0.15, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,  // Visible from both sides
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh,
            material,
            transform: Transform::from_translation(Vec3::new(position.x, 0.0, position.z))
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ..default()
        },
        MoveOrderVisual {
            timer: Timer::from_seconds(MOVE_INDICATOR_LIFETIME, TimerMode::Once),
        },
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// System: Update and cleanup selection ring visuals
pub fn selection_visual_system(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    existing_visuals: Query<(Entity, &SelectionVisual)>,
    mut visual_transforms: Query<&mut Transform, With<SelectionVisual>>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Remove visuals for deselected squads
    for (entity, visual) in existing_visuals.iter() {
        if !selection_state.selected_squads.contains(&visual.squad_id) {
            commands.entity(entity).despawn();
        }
    }

    // Calculate actual squad centers from unit positions (not the anchored squad.center_position)
    let mut squad_actual_centers: std::collections::HashMap<u32, Vec3> = std::collections::HashMap::new();
    for (transform, squad_member) in unit_query.iter() {
        let entry = squad_actual_centers.entry(squad_member.squad_id).or_insert(Vec3::ZERO);
        *entry += transform.translation;
    }
    // Count units per squad and compute average
    let mut squad_unit_counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (_, squad_member) in unit_query.iter() {
        *squad_unit_counts.entry(squad_member.squad_id).or_insert(0) += 1;
    }
    for (squad_id, total) in squad_actual_centers.iter_mut() {
        if let Some(&count) = squad_unit_counts.get(squad_id) {
            if count > 0 {
                *total /= count as f32;
            }
        }
    }

    // Find which selected squads need visuals
    let existing_squad_ids: HashSet<u32> = existing_visuals.iter()
        .map(|(_, v)| v.squad_id)
        .collect();

    // Create visuals for newly selected squads
    for &squad_id in selection_state.selected_squads.iter() {
        if !existing_squad_ids.contains(&squad_id) {
            // Use actual center if available, otherwise fall back to squad manager
            let position = squad_actual_centers.get(&squad_id)
                .copied()
                .or_else(|| squad_manager.get_squad(squad_id).map(|s| s.center_position))
                .unwrap_or(Vec3::ZERO);
            spawn_selection_ring(&mut commands, &mut meshes, &mut materials, squad_id, position);
        }
    }

    // Update positions of existing visuals using actual unit positions
    for (entity, visual) in existing_visuals.iter() {
        if let Some(&actual_center) = squad_actual_centers.get(&visual.squad_id) {
            if let Ok(mut transform) = visual_transforms.get_mut(entity) {
                transform.translation.x = actual_center.x;
                transform.translation.z = actual_center.z;
            }
        }
    }
}

/// Spawn a selection ring under a squad
fn spawn_selection_ring(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    squad_id: u32,
    position: Vec3,
) {
    use bevy::pbr::{NotShadowCaster, NotShadowReceiver};

    // Create a flat annulus (2D ring) mesh instead of 3D torus
    let mesh = meshes.add(Annulus::new(SELECTION_RING_INNER_RADIUS, SELECTION_RING_OUTER_RADIUS));
    let material = materials.add(StandardMaterial {
        base_color: SELECTION_RING_COLOR,
        emissive: LinearRgba::new(0.1, 0.6, 0.8, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,  // Visible from both sides
        ..default()
    });

    // Place ring flat on the ground (Y=0.1 to avoid z-fighting with ground at Y=-1)
    // Rotate -90 degrees around X to lay flat (circle faces up instead of forward)
    commands.spawn((
        PbrBundle {
            mesh,
            material,
            transform: Transform::from_translation(Vec3::new(position.x, 0.1, position.z))
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ..default()
        },
        SelectionVisual { squad_id },
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// System: Fade out and cleanup move order visuals
pub fn move_visual_cleanup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut MoveOrderVisual, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut visual, material_handle) in query.iter_mut() {
        visual.timer.tick(time.delta());

        // Fade out based on timer progress
        if let Some(material) = materials.get_mut(material_handle) {
            let progress = visual.timer.fraction();
            let alpha = (1.0 - progress) * 0.6;
            material.base_color = Color::srgba(0.2, 1.0, 0.3, alpha);
        }

        if visual.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}
