// Selection and command systems for RTS controls
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;
use crate::types::*;
use crate::constants::*;
use crate::formation::calculate_formation_offset;

// Selection state resource - tracks which squads are selected (Vec preserves selection order)
#[derive(Resource, Default)]
pub struct SelectionState {
    pub selected_squads: Vec<u32>,  // First element is primary selection
    pub box_select_start: Option<Vec2>,  // Screen-space start position for box selection
    pub is_box_selecting: bool,
    pub drag_start_world: Option<Vec3>,  // World position where drag started
    // Right-click drag for orientation (CoH1-style)
    pub move_drag_start: Option<Vec3>,   // World position where right-click started
    pub move_drag_current: Option<Vec3>, // Current drag position (for arrow visual)
    pub is_orientation_dragging: bool,   // True when drag exceeds threshold
}

// Marker component for selection ring visuals
#[derive(Component)]
pub struct SelectionVisual {
    pub squad_id: u32,
}

// Marker component for move order destination indicator (circle at destination)
#[derive(Component)]
pub struct MoveOrderVisual {
    pub timer: Timer,
}

// Marker component for path line connecting squad to destination
#[derive(Component)]
pub struct MovePathVisual {
    pub timer: Timer,
}

// Marker component for orientation arrow during right-click drag
#[derive(Component)]
pub struct OrientationArrowVisual;

// Marker component for box selection rectangle visual (UI element)
#[derive(Component)]
pub struct BoxSelectionVisual;

// Threshold for orientation drag (in world units)
const ORIENTATION_DRAG_THRESHOLD: f32 = 3.0;

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
                            if let Some(pos) = selection_state.selected_squads.iter().position(|&id| id == squad_id) {
                                selection_state.selected_squads.remove(pos);
                                info!("Deselected squad {}", squad_id);
                            } else {
                                selection_state.selected_squads.push(squad_id);
                                info!("Added squad {} to selection ({} total)", squad_id, selection_state.selected_squads.len());
                            }
                        } else {
                            // Clear and select single squad
                            selection_state.selected_squads.clear();
                            selection_state.selected_squads.push(squad_id);
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
) {
    let Ok(window) = window_query.get_single() else { return };
    let Ok((camera, camera_transform)) = camera_query.get_single() else { return };
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

        // Select all squads whose center projects into the box
        let mut selected_count = 0;
        for (squad_id, squad) in squad_manager.squads.iter() {
            // Project squad center to screen space
            if let Some(screen_pos) = camera.world_to_viewport(camera_transform, squad.center_position) {
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

/// Movement assignment mode for multi-squad commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MovementAssignmentMode {
    /// Each squad picks the closest destination slot (default, more natural movement)
    #[default]
    ClosestDestination,
    /// First selected squad goes to click point, others spread in order (debug mode)
    SelectionOrder,
}

/// System: Handle right-click move commands for selected squads
/// Supports drag-to-set-orientation (CoH1-style)
pub fn move_command_system(
    mut commands: Commands,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut squad_manager: ResMut<SquadManager>,
    mut selection_state: ResMut<SelectionState>,
    mut droid_query: Query<(&mut BattleDroid, &SquadMember, &FormationOffset, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    arrow_query: Query<Entity, With<OrientationArrowVisual>>,
) {
    let Ok(window) = window_query.get_single() else { return };
    let Ok((camera, camera_transform)) = camera_query.get_single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Get current world position under cursor
    let current_world_pos = screen_to_ground(cursor_pos, camera, camera_transform);

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
            let drag_distance = Vec3::new(current.x - start.x, 0.0, current.z - start.z).length();
            if drag_distance > ORIENTATION_DRAG_THRESHOLD {
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
                let drag_dir = Vec3::new(
                    current.x - destination.x,
                    0.0,
                    current.z - destination.z,
                );
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
            &selection_state,
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

/// Calculate default facing direction (from average squad position toward destination)
fn calculate_default_facing(
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

/// Execute the move command with given destination and facing
fn execute_move_command(
    commands: &mut Commands,
    squad_manager: &mut ResMut<SquadManager>,
    selection_state: &SelectionState,
    droid_query: &mut Query<(&mut BattleDroid, &SquadMember, &FormationOffset, &Transform)>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    destination: Vec3,
    unified_facing: Vec3,
) {
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
    let mut squad_current_positions: std::collections::HashMap<u32, Vec3> = std::collections::HashMap::new();
    for (_droid, squad_member, _offset, transform) in droid_query.iter() {
        if selection_state.selected_squads.contains(&squad_member.squad_id) {
            let entry = squad_current_positions.entry(squad_member.squad_id).or_insert(Vec3::ZERO);
            *entry += transform.translation;
        }
    }
    // Average the positions
    for &squad_id in selection_state.selected_squads.iter() {
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            if let Some(pos) = squad_current_positions.get_mut(&squad_id) {
                let member_count = squad.members.len();
                if member_count > 0 {
                    *pos /= member_count as f32;
                }
            }
        }
    }

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
        let dist_a = Vec3::new(a.1.x - destination.x, 0.0, a.1.z - destination.z).length();
        let dist_b = Vec3::new(b.1.x - destination.x, 0.0, b.1.z - destination.z).length();
        dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    for (squad_id, squad_pos) in sorted_squads {
        // Find the closest available slot
        let mut best_slot_idx = 0;
        let mut best_distance = f32::MAX;

        for (idx, &slot) in available_slots.iter().enumerate() {
            let dist = Vec3::new(squad_pos.x - slot.x, 0.0, squad_pos.z - slot.z).length();
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
    for (mut droid, squad_member, _formation_offset, _transform) in droid_query.iter_mut() {
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

/// Spawn a path line connecting squad position to destination
fn spawn_path_line(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    start: Vec3,
    end: Vec3,
) {
    use bevy::pbr::{NotShadowCaster, NotShadowReceiver};

    let direction = Vec3::new(end.x - start.x, 0.0, end.z - start.z);
    let length = direction.length();

    if length < 0.5 {
        return; // Too short to draw
    }

    let normalized_dir = direction.normalize();
    let rotation = Quat::from_rotation_y(normalized_dir.x.atan2(normalized_dir.z));

    // Create a thin rectangular mesh for the path line
    let line_mesh = meshes.add(create_path_line_mesh());
    let line_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 1.0, 0.3, 0.4),
        emissive: LinearRgba::new(0.05, 0.3, 0.1, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh: line_mesh,
            material: line_material,
            transform: Transform::from_translation(Vec3::new(start.x, 0.15, start.z))
                .with_rotation(rotation)
                .with_scale(Vec3::new(1.0, 1.0, length)),
            ..default()
        },
        MovePathVisual {
            timer: Timer::from_seconds(MOVE_INDICATOR_LIFETIME, TimerMode::Once),
        },
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Create a thin line mesh for path visualization (pointing in +Z, length 1.0)
fn create_path_line_mesh() -> Mesh {
    use bevy::render::mesh::PrimitiveTopology;

    let width = 0.3; // Thin line width

    // Simple quad lying flat on XZ plane
    let vertices = vec![
        [-width / 2.0, 0.0, 0.0],  // 0: left start
        [width / 2.0, 0.0, 0.0],   // 1: right start
        [width / 2.0, 0.0, 1.0],   // 2: right end
        [-width / 2.0, 0.0, 1.0],  // 3: left end
    ];

    let indices = vec![
        0, 2, 1,
        0, 3, 2,
    ];

    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
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
    mut circle_query: Query<(Entity, &mut MoveOrderVisual, &Handle<StandardMaterial>), Without<MovePathVisual>>,
    mut path_query: Query<(Entity, &mut MovePathVisual, &Handle<StandardMaterial>), Without<MoveOrderVisual>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Handle destination circle visuals
    for (entity, mut visual, material_handle) in circle_query.iter_mut() {
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

    // Handle path line visuals
    for (entity, mut visual, material_handle) in path_query.iter_mut() {
        visual.timer.tick(time.delta());

        // Fade out based on timer progress
        if let Some(material) = materials.get_mut(material_handle) {
            let progress = visual.timer.fraction();
            let alpha = (1.0 - progress) * 0.4; // Start at 0.4 alpha (matching spawn)
            material.base_color = Color::srgba(0.2, 1.0, 0.3, alpha);
        }

        if visual.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// System: Update orientation arrow visual during right-click drag
pub fn orientation_arrow_system(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut arrow_query: Query<(Entity, &mut Transform), With<OrientationArrowVisual>>,
) {
    use bevy::pbr::{NotShadowCaster, NotShadowReceiver};

    // Check if we should show the arrow
    if !selection_state.is_orientation_dragging {
        // Remove any existing arrow
        for (entity, _) in arrow_query.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let Some(start) = selection_state.move_drag_start else {
        return;
    };
    let Some(current) = selection_state.move_drag_current else {
        return;
    };

    // Calculate arrow properties
    let direction = Vec3::new(current.x - start.x, 0.0, current.z - start.z);
    let length = direction.length();

    if length < 0.1 {
        return;
    }

    let normalized_dir = direction.normalize();
    let arrow_rotation = Quat::from_rotation_y(normalized_dir.x.atan2(normalized_dir.z));

    // Check if arrow already exists
    if let Ok((_, mut transform)) = arrow_query.get_single_mut() {
        // Update existing arrow
        transform.translation = Vec3::new(start.x, 0.2, start.z);
        transform.rotation = arrow_rotation;
        transform.scale = Vec3::new(1.0, 1.0, length);
    } else {
        // Create new arrow (elongated triangle pointing in drag direction)
        // Arrow is a simple quad that we'll scale based on drag length
        let arrow_mesh = meshes.add(create_arrow_mesh());
        let arrow_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 1.0, 0.3, 0.8),
            emissive: LinearRgba::new(0.1, 0.5, 0.15, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        });

        commands.spawn((
            PbrBundle {
                mesh: arrow_mesh,
                material: arrow_material,
                transform: Transform::from_translation(Vec3::new(start.x, 0.2, start.z))
                    .with_rotation(arrow_rotation)
                    .with_scale(Vec3::new(1.0, 1.0, length)),
                ..default()
            },
            OrientationArrowVisual,
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// Create an arrow mesh pointing in +Z direction (will be rotated and scaled)
fn create_arrow_mesh() -> Mesh {
    use bevy::render::mesh::PrimitiveTopology;

    // Arrow shape: shaft + head, lying flat on XZ plane
    // Base length is 1.0, will be scaled by drag distance
    let shaft_width = 0.8;
    let head_width = 2.0;
    let head_length = 0.15; // Proportion of total length for arrowhead

    // Vertices (Y=0, lying flat)
    let vertices = vec![
        // Shaft (from origin to 1-head_length)
        [-shaft_width / 2.0, 0.0, 0.0],           // 0: left start
        [shaft_width / 2.0, 0.0, 0.0],            // 1: right start
        [shaft_width / 2.0, 0.0, 1.0 - head_length], // 2: right shaft end
        [-shaft_width / 2.0, 0.0, 1.0 - head_length], // 3: left shaft end
        // Arrow head
        [-head_width / 2.0, 0.0, 1.0 - head_length], // 4: left head base
        [head_width / 2.0, 0.0, 1.0 - head_length],  // 5: right head base
        [0.0, 0.0, 1.0],                              // 6: tip
    ];

    let indices = vec![
        // Shaft quad (two triangles)
        0, 2, 1,
        0, 3, 2,
        // Arrow head triangle
        4, 6, 5,
    ];

    let normals = vec![[0.0, 1.0, 0.0]; 7]; // All pointing up
    let uvs = vec![[0.0, 0.0]; 7]; // Simple UVs

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
}

/// System: Render box selection rectangle during left-click drag
pub fn box_selection_visual_system(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    existing_visual: Query<Entity, With<BoxSelectionVisual>>,
) {
    let Ok(window) = window_query.get_single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        // No cursor - despawn any existing visual
        for entity in existing_visual.iter() {
            commands.entity(entity).despawn_recursive();
        }
        return;
    };

    // Check if we should show the box selection visual
    if !selection_state.is_box_selecting {
        // Not box selecting - despawn any existing visual
        for entity in existing_visual.iter() {
            commands.entity(entity).despawn_recursive();
        }
        return;
    }

    let Some(start_pos) = selection_state.box_select_start else {
        return;
    };

    // Calculate box corners (screen space)
    let min_x = start_pos.x.min(cursor_pos.x);
    let max_x = start_pos.x.max(cursor_pos.x);
    let min_y = start_pos.y.min(cursor_pos.y);
    let max_y = start_pos.y.max(cursor_pos.y);

    let width = max_x - min_x;
    let height = max_y - min_y;

    // Despawn existing visual (we'll recreate it with new dimensions)
    for entity in existing_visual.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Skip if too small
    if width < 2.0 || height < 2.0 {
        return;
    }

    // Spawn the box selection UI node
    // Using a semi-transparent green box with a border effect
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(min_x),
                top: Val::Px(min_y),
                width: Val::Px(width),
                height: Val::Px(height),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.2, 0.8, 0.3, 0.15)),
            border_color: BorderColor(Color::srgba(0.3, 1.0, 0.4, 0.8)),
            ..default()
        },
        BoxSelectionVisual,
    ));
}
