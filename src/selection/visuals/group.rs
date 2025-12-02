// Group visuals - orientation markers and bounding box debug
use bevy::prelude::*;
use bevy::pbr::NotShadowCaster;
use crate::types::*;
use crate::terrain::TerrainHeightmap;

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
    heightmap: Option<Res<TerrainHeightmap>>,
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
        let arrow_base_xz = obb.front_edge_center(0.0) + obb.facing * 5.0;

        // Sample terrain height at arrow position
        let arrow_y = if let Some(hm) = heightmap.as_deref() {
            hm.sample_height(arrow_base_xz.x, arrow_base_xz.z) + 0.2
        } else {
            arrow_base_xz.y
        };
        let arrow_base = Vec3::new(arrow_base_xz.x, arrow_y, arrow_base_xz.z);

        // Check if marker already exists for this group
        let mut found = false;
        for (_entity, marker, mut transform) in existing_markers.iter_mut() {
            if marker.group_id == group_id {
                // Update position smoothly to reduce twitching
                let target_pos = Vec3::new(arrow_base_xz.x, arrow_y, arrow_base_xz.z);
                transform.translation = transform.translation.lerp(target_pos, 0.1);

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
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: arrow_color,
                    emissive: LinearRgba::rgb(2.0, 2.0, 0.0),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None, // Visible from both sides
                    ..default()
                })),
                Transform::from_translation(arrow_base)
                    .with_rotation(target_rotation),
                Visibility::Visible,
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
    heightmap: Option<Res<TerrainHeightmap>>,
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

        // Get the 4 corners of the OBB (base positions without terrain adjustment)
        let y_offset = 0.2;
        let corners_base = obb.corners(y_offset);
        // corners: [back-left, back-right, front-right, front-left]

        // Sample terrain heights at all four corners
        let sample_corner_y = |corner: Vec3| {
            if let Some(hm) = heightmap.as_deref() {
                hm.sample_height(corner.x, corner.z) + y_offset
            } else {
                corner.y
            }
        };

        let bl_y = sample_corner_y(corners_base[0]); // back-left
        let br_y = sample_corner_y(corners_base[1]); // back-right
        let fr_y = sample_corner_y(corners_base[2]); // front-right
        let fl_y = sample_corner_y(corners_base[3]); // front-left

        // Terrain-adjusted corners
        let bl = Vec3::new(corners_base[0].x, bl_y, corners_base[0].z);
        let br = Vec3::new(corners_base[1].x, br_y, corners_base[1].z);
        let fr = Vec3::new(corners_base[2].x, fr_y, corners_base[2].z);
        let fl = Vec3::new(corners_base[3].x, fl_y, corners_base[3].z);

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

        // Update existing edges by despawning and recreating (simpler than updating mesh geometry)
        if found {
            // Despawn existing debug boxes for this group
            for (entity, debug_marker, _) in existing_debug.iter() {
                if debug_marker.group_id == group_id {
                    commands.entity(entity).despawn();
                }
            }
        }

        // Create/recreate debug box - wireframe outline using line meshes
        {
            // Always create edges (either first time or after despawning for update)
            let debug_color = Color::srgba(1.0, 0.0, 1.0, 0.8); // Magenta with some transparency

            // Create shared material for all edges
            let debug_material = materials.add(StandardMaterial {
                base_color: debug_color,
                emissive: LinearRgba::rgb(2.0, 0.0, 2.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });

            let line_thickness = 0.5;

            // Create line mesh for each edge connecting the terrain-adjusted corners
            // Pass heightmap to actually sample terrain along the line
            // Front edge: front-left to front-right
            let front_mesh = create_line_mesh(fl, fr, line_thickness, heightmap.as_deref());
            commands.spawn((
                Mesh3d(meshes.add(front_mesh)),
                MeshMaterial3d(debug_material.clone()),
                Transform::IDENTITY,
                Visibility::Visible,
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));

            // Back edge: back-left to back-right
            let back_mesh = create_line_mesh(bl, br, line_thickness, heightmap.as_deref());
            commands.spawn((
                Mesh3d(meshes.add(back_mesh)),
                MeshMaterial3d(debug_material.clone()),
                Transform::IDENTITY,
                Visibility::Visible,
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));

            // Left edge: back-left to front-left
            let left_mesh = create_line_mesh(bl, fl, line_thickness, heightmap.as_deref());
            commands.spawn((
                Mesh3d(meshes.add(left_mesh)),
                MeshMaterial3d(debug_material.clone()),
                Transform::IDENTITY,
                Visibility::Visible,
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));

            // Right edge: back-right to front-right
            let right_mesh = create_line_mesh(br, fr, line_thickness, heightmap.as_deref());
            commands.spawn((
                Mesh3d(meshes.add(right_mesh)),
                MeshMaterial3d(debug_material.clone()),
                Transform::IDENTITY,
                Visibility::Visible,
                NotShadowCaster,
                GroupBoundingBoxDebug { group_id },
            ));
        }
    }
}

/// Create a terrain-conforming line mesh connecting two points
/// The line is subdivided into segments and follows the terrain between the points
fn create_line_mesh(start: Vec3, end: Vec3, thickness: f32, heightmap: Option<&TerrainHeightmap>) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;

    const LENGTH_SEGMENTS: usize = 16; // Segments along the line length for terrain conforming
    const RADIUS_SEGMENTS: usize = 6;  // Segments around the line thickness

    let direction_xz = Vec3::new(end.x - start.x, 0.0, end.z - start.z);
    let length_xz = direction_xz.length();

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices along the line, sampling terrain at each point
    for seg_i in 0..=LENGTH_SEGMENTS {
        let t = seg_i as f32 / LENGTH_SEGMENTS as f32;

        // Interpolate position along the line (in XZ plane)
        let center_x = start.x + (end.x - start.x) * t;
        let center_z = start.z + (end.z - start.z) * t;

        // ACTUALLY SAMPLE THE TERRAIN HEIGHT HERE instead of interpolating!
        let center_y = if let Some(hm) = heightmap {
            hm.sample_height(center_x, center_z) + 0.2
        } else {
            start.y + (end.y - start.y) * t // Fallback to linear interpolation
        };

        let center = Vec3::new(center_x, center_y, center_z);

        // Calculate perpendicular direction for the circle cross-section
        let forward = if length_xz > 0.001 {
            Vec3::new(end.x - start.x, 0.0, end.z - start.z).normalize()
        } else {
            Vec3::X
        };
        let right = Vec3::new(-forward.z, 0.0, forward.x); // Perpendicular in XZ plane

        // Create a circle of vertices around this center point
        for rad_i in 0..RADIUS_SEGMENTS {
            let angle = (rad_i as f32 / RADIUS_SEGMENTS as f32) * std::f32::consts::TAU;
            let cos = angle.cos();
            let sin = angle.sin();

            let offset = right * cos * thickness + Vec3::Y * sin * thickness;
            let pos = center + offset;

            positions.push([pos.x, pos.y, pos.z]);

            // Normal points outward from the cylinder axis
            let normal = offset.normalize();
            normals.push([normal.x, normal.y, normal.z]);
        }
    }

    // Generate triangle indices connecting the segments
    for seg_i in 0..LENGTH_SEGMENTS {
        for rad_i in 0..RADIUS_SEGMENTS {
            let current_ring_start = (seg_i * RADIUS_SEGMENTS) as u32;
            let next_ring_start = ((seg_i + 1) * RADIUS_SEGMENTS) as u32;

            let current_idx = current_ring_start + rad_i as u32;
            let next_radial_idx = current_ring_start + ((rad_i + 1) % RADIUS_SEGMENTS) as u32;
            let next_seg_idx = next_ring_start + rad_i as u32;
            let next_both_idx = next_ring_start + ((rad_i + 1) % RADIUS_SEGMENTS) as u32;

            // First triangle
            indices.push(current_idx);
            indices.push(next_seg_idx);
            indices.push(next_radial_idx);

            // Second triangle
            indices.push(next_radial_idx);
            indices.push(next_seg_idx);
            indices.push(next_both_idx);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}
