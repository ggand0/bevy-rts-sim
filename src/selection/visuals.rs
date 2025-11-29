// Visual systems and spawn functions for selection feedback
use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::window::PrimaryWindow;
use std::collections::HashSet;
use crate::types::*;
use crate::constants::*;

use super::state::*;
use super::groups::check_is_complete_group;
use super::obb::OrientedBoundingBox;
use super::utils::calculate_squad_centers;

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
    let squad_actual_centers = calculate_squad_centers(&unit_query);

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
) {
    // Check if any group is currently selected
    let selected_group_id = check_is_complete_group(&selection_state, &squad_manager);

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

        // Collect squad positions (only living squads with members)
        let squad_positions: Vec<Vec3> = group.squad_ids.iter()
            .filter_map(|&squad_id| squad_manager.get_squad(squad_id))
            .filter(|squad| !squad.members.is_empty())
            .map(|squad| squad.center_position)
            .collect();

        if squad_positions.is_empty() {
            return;
        }

        // Calculate OBB aligned to the group's facing direction
        let facing = group.formation_facing;
        let padding = 15.0;
        let Some(obb) = OrientedBoundingBox::from_squads(&squad_positions, facing, padding) else {
            return;
        };

        // Position arrow at the front edge of the OBB, slightly ahead
        let arrow_base = obb.front_edge_center(0.0) + obb.facing * 5.0;

        // Check if marker already exists for this group
        let mut found = false;
        for (_entity, marker, mut transform) in existing_markers.iter_mut() {
            if marker.group_id == group_id {
                // Update position smoothly to reduce twitching
                transform.translation = transform.translation.lerp(arrow_base, 0.1);

                // Update rotation to face the group's facing direction
                let target_rotation = Quat::from_rotation_y(obb.facing.x.atan2(obb.facing.z));
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
            let target_rotation = Quat::from_rotation_y(obb.facing.x.atan2(obb.facing.z));

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

/// System to visualize group bounding boxes for debugging
/// Uses an Oriented Bounding Box (OBB) that rotates with the group's facing direction
pub fn update_group_bounding_box_debug(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut existing_debug: Query<(Entity, &GroupBoundingBoxDebug, &mut Transform)>,
) {
    // Check if any group is currently selected
    let selected_group_id = check_is_complete_group(&selection_state, &squad_manager);

    // Remove debug boxes for groups that no longer exist or are not selected
    for (entity, debug_marker, _) in existing_debug.iter() {
        let should_remove = !selection_state.groups.contains_key(&debug_marker.group_id)
            || selected_group_id != Some(debug_marker.group_id);
        if should_remove {
            commands.entity(entity).despawn();
        }
    }

    // Only create/update debug box if a complete group is selected
    let Some(active_group_id) = selected_group_id else { return };

    // Create or update debug box for the selected group
    if let Some(group) = selection_state.groups.get(&active_group_id) {
        let group_id = active_group_id;

        // Collect squad positions (only living squads with members)
        let squad_positions: Vec<Vec3> = group.squad_ids.iter()
            .filter_map(|&squad_id| squad_manager.get_squad(squad_id))
            .filter(|squad| !squad.members.is_empty())
            .map(|squad| squad.center_position)
            .collect();

        if squad_positions.is_empty() {
            return;
        }

        // Calculate OBB aligned to the group's facing direction
        let facing = group.formation_facing;
        let padding = 15.0;
        let Some(obb) = OrientedBoundingBox::from_squads(&squad_positions, facing, padding) else {
            return;
        };

        // Get the 4 corners of the OBB
        let y_offset = 0.2;
        let corners = obb.corners(y_offset);
        // corners: [back-left, back-right, front-right, front-left]

        // Calculate edge midpoints and lengths
        // Front edge: front-left to front-right (corners[3] to corners[2])
        // Back edge: back-left to back-right (corners[0] to corners[1])
        // Left edge: back-left to front-left (corners[0] to corners[3])
        // Right edge: back-right to front-right (corners[1] to corners[2])

        let front_mid = (corners[3] + corners[2]) / 2.0;
        let back_mid = (corners[0] + corners[1]) / 2.0;
        let left_mid = (corners[0] + corners[3]) / 2.0;
        let right_mid = (corners[1] + corners[2]) / 2.0;

        let width = obb.half_extents.x * 2.0;  // Full width (perpendicular to facing)
        let depth = obb.half_extents.y * 2.0;  // Full depth (along facing)

        // Calculate rotation from facing direction
        let rotation = Quat::from_rotation_y(obb.facing.x.atan2(obb.facing.z));

        // Check if debug box already exists for this group
        let existing_count = existing_debug.iter()
            .filter(|(_, marker, _)| marker.group_id == group_id)
            .count();
        let found = existing_count > 0;

        // Debug logging (only when creating)
        if !found {
            info!("Group {} OBB: center=({:.2}, {:.2}), half_extents=({:.2}, {:.2}), facing=({:.2}, {:.2})",
                group_id, obb.center.x, obb.center.z, obb.half_extents.x, obb.half_extents.y,
                obb.facing.x, obb.facing.z);
        }

        let line_thickness = 0.5;
        let line_height = 0.3;

        // Update existing edges if they exist
        if found {
            let mut edge_index = 0;
            for (_entity, debug_marker, mut transform) in existing_debug.iter_mut() {
                if debug_marker.group_id == group_id {
                    match edge_index {
                        0 => {
                            // Front edge
                            transform.translation = front_mid;
                            transform.rotation = rotation;
                            transform.scale = Vec3::new(width, line_height, line_thickness);
                        }
                        1 => {
                            // Back edge
                            transform.translation = back_mid;
                            transform.rotation = rotation;
                            transform.scale = Vec3::new(width, line_height, line_thickness);
                        }
                        2 => {
                            // Left edge
                            transform.translation = left_mid;
                            transform.rotation = rotation;
                            transform.scale = Vec3::new(line_thickness, line_height, depth);
                        }
                        3 => {
                            // Right edge
                            transform.translation = right_mid;
                            transform.rotation = rotation;
                            transform.scale = Vec3::new(line_thickness, line_height, depth);
                        }
                        _ => {}
                    }
                    edge_index += 1;
                }
            }
        }

        if !found {
            // Create new debug box - wireframe outline using OBB
            let debug_color = Color::srgba(1.0, 0.0, 1.0, 0.8); // Magenta with some transparency

            // Create a unit cube that we'll scale and rotate
            let unit_cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

            // Create shared material for all edges
            let debug_material = materials.add(StandardMaterial {
                base_color: debug_color,
                emissive: LinearRgba::rgb(2.0, 0.0, 2.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });

            // Front edge (at the front of the OBB, where the orientation indicator is)
            commands.spawn((
                PbrBundle {
                    mesh: unit_cube.clone(),
                    material: debug_material.clone(),
                    transform: Transform::from_translation(front_mid)
                        .with_rotation(rotation)
                        .with_scale(Vec3::new(width, line_height, line_thickness)),
                    visibility: Visibility::Visible,
                    ..default()
                },
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));

            // Back edge
            commands.spawn((
                PbrBundle {
                    mesh: unit_cube.clone(),
                    material: debug_material.clone(),
                    transform: Transform::from_translation(back_mid)
                        .with_rotation(rotation)
                        .with_scale(Vec3::new(width, line_height, line_thickness)),
                    visibility: Visibility::Visible,
                    ..default()
                },
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));

            // Left edge
            commands.spawn((
                PbrBundle {
                    mesh: unit_cube.clone(),
                    material: debug_material.clone(),
                    transform: Transform::from_translation(left_mid)
                        .with_rotation(rotation)
                        .with_scale(Vec3::new(line_thickness, line_height, depth)),
                    visibility: Visibility::Visible,
                    ..default()
                },
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));

            // Right edge
            commands.spawn((
                PbrBundle {
                    mesh: unit_cube.clone(),
                    material: debug_material.clone(),
                    transform: Transform::from_translation(right_mid)
                        .with_rotation(rotation)
                        .with_scale(Vec3::new(line_thickness, line_height, depth)),
                    visibility: Visibility::Visible,
                    ..default()
                },
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));
        }
    }
}

/// Spawn a visual indicator at the move destination
pub fn spawn_move_indicator(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
) {
    spawn_move_indicator_with_color(commands, meshes, materials, position, None);
}

/// Spawn a visual indicator at the move destination with custom color
/// If color is None, uses default green color
pub fn spawn_move_indicator_with_color(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    color: Option<Color>,
) {
    // Create a flat circle on the ground
    let mesh = meshes.add(Circle::new(MOVE_INDICATOR_RADIUS));

    let (base_color, emissive) = match color {
        Some(c) => (c.with_alpha(0.6), LinearRgba::from(c) * 0.5),
        None => (Color::srgba(0.2, 1.0, 0.3, 0.6), LinearRgba::new(0.1, 0.5, 0.15, 1.0)),
    };

    let material = materials.add(StandardMaterial {
        base_color,
        emissive,
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
pub fn spawn_path_line(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    start: Vec3,
    end: Vec3,
) {
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
