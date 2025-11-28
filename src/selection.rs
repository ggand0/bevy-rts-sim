// Selection and command systems for RTS controls
use bevy::prelude::*;
use bevy::pbr::NotShadowCaster;
use bevy::window::PrimaryWindow;
use std::collections::{HashSet, HashMap};
use crate::types::*;
use crate::constants::*;
use crate::formation::calculate_formation_offset;

// Squad group - Total War style formation preservation
#[derive(Clone)]
pub struct SquadGroup {
    pub id: u32,
    pub squad_ids: Vec<u32>,
    pub squad_offsets: HashMap<u32, Vec3>,  // Relative offsets from group center
    pub formation_facing: Vec3,              // Facing direction when group was formed
}

// Selection state resource - tracks which squads are selected (Vec preserves selection order)
#[derive(Resource)]
pub struct SelectionState {
    pub selected_squads: Vec<u32>,  // First element is primary selection
    pub box_select_start: Option<Vec2>,  // Screen-space start position for box selection
    pub is_box_selecting: bool,
    pub drag_start_world: Option<Vec3>,  // World position where drag started
    // Right-click drag for orientation (CoH1-style)
    pub move_drag_start: Option<Vec3>,   // World position where right-click started
    pub move_drag_current: Option<Vec3>, // Current drag position (for arrow visual)
    pub is_orientation_dragging: bool,   // True when drag exceeds threshold
    // Squad grouping
    pub groups: HashMap<u32, SquadGroup>,
    pub squad_to_group: HashMap<u32, u32>,
    pub next_group_id: u32,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected_squads: Vec::new(),
            box_select_start: None,
            is_box_selecting: false,
            drag_start_world: None,
            move_drag_start: None,
            move_drag_current: None,
            is_orientation_dragging: false,
            groups: HashMap::new(),
            squad_to_group: HashMap::new(),
            next_group_id: 1,
        }
    }
}

// Marker component for selection ring visuals
#[derive(Component)]
pub struct SelectionVisual {
    pub squad_id: u32,
    pub is_grouped: bool,  // Track if currently showing grouped color
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

// Marker component for group orientation indicator
#[derive(Component)]
pub struct GroupOrientationMarker {
    pub group_id: u32,
}

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
/// Only considers squads from the specified team (player team)
pub fn find_squad_at_position(
    world_pos: Vec3,
    squad_centers: &std::collections::HashMap<u32, Vec3>,
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

/// System: Handle left-click selection input
pub fn selection_input_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    squad_manager: Res<SquadManager>,
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

                    // Only select player's team (Team::A)
                    if let Some(squad_id) = find_squad_at_position(world_pos, &squad_centers, &squad_manager, SELECTION_CLICK_RADIUS, Team::A) {
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
    mut droid_query: Query<(Entity, &mut BattleDroid, &SquadMember, &FormationOffset, &Transform)>,
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

    // Save the original formation_facing before updating
    let original_facing = group.formation_facing;

    // Calculate rotation from original formation facing to new unified facing
    let angle_diff = if original_facing.length() > 0.1 && unified_facing.length() > 0.1 {
        // Calculate angle between the two vectors
        let dot = original_facing.x * unified_facing.x + original_facing.z * unified_facing.z;
        let det = original_facing.x * unified_facing.z - original_facing.z * unified_facing.x;
        det.atan2(dot)
    } else {
        0.0
    };

    // Update the group's formation_facing to the new unified_facing
    if unified_facing.length() > 0.1 {
        group.formation_facing = unified_facing;
    }

    let rotation = Quat::from_rotation_y(angle_diff);

    // Calculate actual current positions for path visuals
    let mut squad_current_positions: std::collections::HashMap<u32, Vec3> = std::collections::HashMap::new();
    for (_entity, _droid, squad_member, _offset, transform) in droid_query.iter() {
        if group.squad_ids.contains(&squad_member.squad_id) {
            let entry = squad_current_positions.entry(squad_member.squad_id).or_insert(Vec3::ZERO);
            *entry += transform.translation;
        }
    }
    // Average the positions
    for &squad_id in &group.squad_ids {
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            if let Some(pos) = squad_current_positions.get_mut(&squad_id) {
                let member_count = squad.members.len();
                if member_count > 0 {
                    *pos /= member_count as f32;
                }
            }
        }
    }

    // Apply rotated offsets to each squad
    for (&squad_id, &offset) in &group.squad_offsets {
        // Rotate the offset
        let rotated_offset = rotation * offset;
        let squad_dest = destination + rotated_offset;

        if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
            // Set facing direction
            if unified_facing.length() > 0.1 {
                squad.target_facing_direction = unified_facing;
                squad.facing_direction = unified_facing;
            }

            // Set target position
            squad.target_position = squad_dest;

            // Spawn visual indicators
            spawn_move_indicator(commands, meshes, materials, squad_dest);

            if let Some(&start_pos) = squad_current_positions.get(&squad_id) {
                spawn_path_line(commands, meshes, materials, start_pos, squad_dest);
            }
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

/// Execute the move command with given destination and facing
/// Check if the current selection is exactly one complete group (all squads in a group are selected)
fn check_is_complete_group(selection_state: &SelectionState) -> Option<u32> {
    if selection_state.selected_squads.is_empty() {
        return None;
    }

    // Check if all selected squads belong to the same group
    let mut group_id: Option<u32> = None;
    for &squad_id in &selection_state.selected_squads {
        if let Some(&gid) = selection_state.squad_to_group.get(&squad_id) {
            if group_id.is_none() {
                group_id = Some(gid);
            } else if group_id != Some(gid) {
                // Squads belong to different groups
                return None;
            }
        } else {
            // At least one squad is not in a group
            return None;
        }
    }

    // Check if all squads in the group are selected
    if let Some(gid) = group_id {
        if let Some(group) = selection_state.groups.get(&gid) {
            let all_in_selection = group.squad_ids.iter().all(|id| selection_state.selected_squads.contains(id));
            if all_in_selection {
                return Some(gid);
            }
        }
    }

    None
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
    if let Some(group_id) = check_is_complete_group(selection_state) {
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
    let mut squad_current_positions: std::collections::HashMap<u32, Vec3> = std::collections::HashMap::new();
    for (_entity, _droid, squad_member, _offset, transform) in droid_query.iter() {
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

/// System: Handle group toggle with G key (group if ungrouped, ungroup if grouped)
pub fn group_command_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection_state: ResMut<SelectionState>,
    squad_manager: Res<SquadManager>,
) {
    // Toggle group with G (or U to ungroup)
    if keyboard.just_pressed(KeyCode::KeyG) || keyboard.just_pressed(KeyCode::KeyU) {
        if selection_state.selected_squads.is_empty() {
            return;
        }

        let selected_squads = selection_state.selected_squads.clone();

        // Check if all selected squads are already in the same group
        let all_in_same_group = if let Some(&first_group) = selection_state.squad_to_group.get(&selected_squads[0]) {
            selected_squads.iter().all(|&id| {
                selection_state.squad_to_group.get(&id) == Some(&first_group)
            })
        } else {
            false
        };

        // If U is pressed or all are already grouped together, ungroup
        if keyboard.just_pressed(KeyCode::KeyU) || all_in_same_group {
            let mut ungrouped_count = 0;

            for &squad_id in &selected_squads {
                if let Some(&group_id) = selection_state.squad_to_group.get(&squad_id) {
                    selection_state.squad_to_group.remove(&squad_id);
                    ungrouped_count += 1;

                    // Remove from group
                    if let Some(group) = selection_state.groups.get_mut(&group_id) {
                        group.squad_ids.retain(|&id| id != squad_id);
                        group.squad_offsets.remove(&squad_id);
                    }
                }
            }

            // Clean up empty groups
            selection_state.groups.retain(|_, group| !group.squad_ids.is_empty());

            if ungrouped_count > 0 {
                info!("Ungrouped {} squads", ungrouped_count);
            }
        } else if selected_squads.len() >= 2 {
            // Group the selected squads

            // Remove existing group memberships for selected squads
            for &squad_id in &selected_squads {
                if let Some(&old_group_id) = selection_state.squad_to_group.get(&squad_id) {
                    selection_state.squad_to_group.remove(&squad_id);

                    // Remove from old group's squad list
                    if let Some(old_group) = selection_state.groups.get_mut(&old_group_id) {
                        old_group.squad_ids.retain(|&id| id != squad_id);
                        old_group.squad_offsets.remove(&squad_id);
                    }
                }
            }

            // Clean up empty groups
            selection_state.groups.retain(|_, group| !group.squad_ids.is_empty());

            // Calculate group center from squad positions
            let mut group_center = Vec3::ZERO;
            let mut valid_squad_count = 0;
            let mut avg_facing = Vec3::ZERO;

            for &squad_id in &selected_squads {
                if let Some(squad) = squad_manager.get_squad(squad_id) {
                    group_center += squad.center_position;
                    avg_facing += squad.facing_direction;
                    valid_squad_count += 1;
                }
            }

            if valid_squad_count > 0 {
                group_center /= valid_squad_count as f32;
                avg_facing = avg_facing.normalize_or_zero();

                // Calculate offsets for each squad
                let mut squad_offsets = HashMap::new();
                let mut squad_ids = Vec::new();

                for &squad_id in &selected_squads {
                    if let Some(squad) = squad_manager.get_squad(squad_id) {
                        let offset = squad.center_position - group_center;
                        squad_offsets.insert(squad_id, offset);
                        squad_ids.push(squad_id);
                    }
                }

                // Create new group
                let group_id = selection_state.next_group_id;
                selection_state.next_group_id += 1;

                let group = SquadGroup {
                    id: group_id,
                    squad_ids: squad_ids.clone(),
                    squad_offsets,
                    formation_facing: avg_facing,
                };

                selection_state.groups.insert(group_id, group);

                // Update squad-to-group mapping
                for squad_id in squad_ids {
                    selection_state.squad_to_group.insert(squad_id, group_id);
                }

                info!("Created group {} with {} squads", group_id, valid_squad_count);
            }
        } else if selected_squads.len() == 1 {
            info!("Need at least 2 squads to create a group");
        }
    }
}

/// System: Update and cleanup selection ring visuals
pub fn selection_visual_system(
    mut commands: Commands,
    mut selection_state: ResMut<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut existing_visuals: Query<(Entity, &mut SelectionVisual, &Handle<StandardMaterial>)>,
    mut visual_transforms: Query<&mut Transform, With<SelectionVisual>>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Clean up dead squads from selection (squads with no living units)
    selection_state.selected_squads.retain(|&squad_id| {
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            !squad.members.is_empty()
        } else {
            false // Squad doesn't exist anymore
        }
    });

    // Remove visuals for deselected squads or squads with no living units
    for (entity, visual, _) in existing_visuals.iter() {
        let should_remove = !selection_state.selected_squads.contains(&visual.squad_id)
            || squad_manager.get_squad(visual.squad_id).map_or(true, |s| s.members.is_empty());
        if should_remove {
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
        .map(|(_, v, _)| v.squad_id)
        .collect();

    // Create visuals for newly selected squads
    for &squad_id in selection_state.selected_squads.iter() {
        if !existing_squad_ids.contains(&squad_id) {
            // Use actual center if available, otherwise fall back to squad manager
            let position = squad_actual_centers.get(&squad_id)
                .copied()
                .or_else(|| squad_manager.get_squad(squad_id).map(|s| s.center_position))
                .unwrap_or(Vec3::ZERO);
            let is_grouped = selection_state.squad_to_group.contains_key(&squad_id);
            spawn_selection_ring(&mut commands, &mut meshes, &mut materials, squad_id, position, is_grouped);
        }
    }

    // Update positions and colors of existing visuals
    for (entity, mut visual, material_handle) in existing_visuals.iter_mut() {
        // Update position
        if let Some(&actual_center) = squad_actual_centers.get(&visual.squad_id) {
            if let Ok(mut transform) = visual_transforms.get_mut(entity) {
                transform.translation.x = actual_center.x;
                transform.translation.z = actual_center.z;
            }
        }

        // Check if group status changed and update color
        let is_now_grouped = selection_state.squad_to_group.contains_key(&visual.squad_id);
        if visual.is_grouped != is_now_grouped {
            visual.is_grouped = is_now_grouped;
            // Update material color
            if let Some(material) = materials.get_mut(material_handle) {
                if is_now_grouped {
                    // Yellow for grouped
                    material.base_color = Color::srgba(1.0, 0.9, 0.2, 0.7);
                    material.emissive = LinearRgba::new(0.8, 0.7, 0.1, 1.0);
                } else {
                    // Cyan for ungrouped (default)
                    material.base_color = SELECTION_RING_COLOR;
                    material.emissive = LinearRgba::new(0.1, 0.6, 0.8, 1.0);
                }
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
    is_grouped: bool,
) {
    use bevy::pbr::{NotShadowCaster, NotShadowReceiver};

    // Create a flat annulus (2D ring) mesh instead of 3D torus
    let mesh = meshes.add(Annulus::new(SELECTION_RING_INNER_RADIUS, SELECTION_RING_OUTER_RADIUS));

    // Yellow for grouped, cyan for ungrouped
    let (base_color, emissive) = if is_grouped {
        (Color::srgba(1.0, 0.9, 0.2, 0.7), LinearRgba::new(0.8, 0.7, 0.1, 1.0))
    } else {
        (SELECTION_RING_COLOR, LinearRgba::new(0.1, 0.6, 0.8, 1.0))
    };

    let material = materials.add(StandardMaterial {
        base_color,
        emissive,
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
        SelectionVisual { squad_id, is_grouped },
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

/// System to visualize group orientation with a yellow arrow marker
pub fn update_group_orientation_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut existing_markers: Query<(Entity, &GroupOrientationMarker, &mut Transform)>,
    droid_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<GroupOrientationMarker>)>,
) {
    // Check if any group is currently selected
    let selected_group_id = check_is_complete_group(&selection_state);

    // Remove markers for groups that no longer exist or are not selected
    for (entity, marker, _) in existing_markers.iter() {
        let should_remove = !selection_state.groups.contains_key(&marker.group_id)
            || selected_group_id != Some(marker.group_id);
        if should_remove {
            commands.entity(entity).despawn();
        }
    }

    // Only create/update marker if a complete group is selected
    let Some(active_group_id) = selected_group_id else { return };

    // Create or update marker for the selected group
    if let Some(group) = selection_state.groups.get(&active_group_id) {
        let group_id = active_group_id;
        // Calculate group bounding box from squad centers
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        let mut group_center = Vec3::ZERO;
        let mut squad_count = 0;

        for &squad_id in &group.squad_ids {
            if let Some(squad) = squad_manager.get_squad(squad_id) {
                let pos = squad.center_position;
                min_x = min_x.min(pos.x);
                max_x = max_x.max(pos.x);
                min_z = min_z.min(pos.z);
                max_z = max_z.max(pos.z);
                group_center += pos;
                squad_count += 1;
            }
        }

        if squad_count == 0 {
            return;
        }

        group_center /= squad_count as f32;

        // Calculate the front edge position based on facing direction
        // Project bounding box corners onto the facing direction to find the furthest point
        let facing = group.formation_facing;
        let _right = Vec3::new(facing.z, 0.0, -facing.x).normalize();

        // Get the forward-most point along the facing direction
        let corners = [
            Vec3::new(min_x, group_center.y, min_z),
            Vec3::new(max_x, group_center.y, min_z),
            Vec3::new(min_x, group_center.y, max_z),
            Vec3::new(max_x, group_center.y, max_z),
        ];

        let mut max_forward = f32::MIN;
        for corner in &corners {
            let forward_dist = (*corner - group_center).dot(facing);
            max_forward = max_forward.max(forward_dist);
        }

        // Position arrow at the front edge, slightly ahead
        let front_edge_offset = max_forward + 5.0; // 5 units ahead of front edge
        let arrow_base = group_center + facing * front_edge_offset;

        // Check if marker already exists for this group
        let mut found = false;
        for (_entity, marker, mut transform) in existing_markers.iter_mut() {
            if marker.group_id == group_id {
                // Update position smoothly to reduce twitching
                transform.translation = transform.translation.lerp(arrow_base, 0.1);

                // Update rotation to face the group's facing direction
                let target_rotation = Quat::from_rotation_y(facing.x.atan2(facing.z));
                transform.rotation = transform.rotation.slerp(target_rotation, 0.1);

                found = true;
                break;
            }
        }

        if !found {
            // Create new marker - 2D triangle arrow on the ground
            let arrow_color = Color::srgb(1.0, 1.0, 0.0);

            // Create a triangle mesh in LOCAL space (centered at origin, pointing forward along +Z)
            let triangle_size = 3.0;

            // Triangle vertices in local space: tip at front (+Z), base at back
            let tip = Vec3::new(0.0, 0.05, triangle_size);
            let base_left = Vec3::new(-triangle_size * 0.6, 0.05, 0.0);
            let base_right = Vec3::new(triangle_size * 0.6, 0.05, 0.0);

            // Create mesh with positions and normals (in local space)
            let positions = vec![
                [tip.x, tip.y, tip.z],
                [base_left.x, base_left.y, base_left.z],
                [base_right.x, base_right.y, base_right.z],
            ];
            let normals = vec![[0.0, 1.0, 0.0]; 3];
            let uvs = vec![[0.5, 1.0], [0.0, 0.0], [1.0, 0.0]];
            let indices = vec![0, 1, 2];

            let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

            // Calculate initial rotation to point in the facing direction
            let target_rotation = Quat::from_rotation_y(facing.x.atan2(facing.z));

            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(mesh),
                    material: materials.add(StandardMaterial {
                        base_color: arrow_color,
                        emissive: LinearRgba::rgb(2.0, 2.0, 0.0),
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        cull_mode: None, // Visible from both sides
                        ..default()
                    }),
                    transform: Transform::from_translation(arrow_base)
                        .with_rotation(target_rotation),
                    visibility: Visibility::Visible,
                    ..default()
                },
                NotShadowCaster,
                GroupOrientationMarker {
                    group_id,
                },
            ));
        }
    }
}

