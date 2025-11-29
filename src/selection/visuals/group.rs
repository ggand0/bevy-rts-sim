// Group visuals - orientation markers and bounding box debug
use bevy::prelude::*;
use bevy::pbr::NotShadowCaster;
use crate::types::*;

use super::super::state::*;
use super::super::groups::check_is_complete_group;
use super::super::obb::OrientedBoundingBox;

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
