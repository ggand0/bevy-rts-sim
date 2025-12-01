// Selection ring visuals - cyan rings under selected squads
use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use std::collections::HashSet;
use crate::types::*;
use crate::constants::*;
use crate::terrain::TerrainHeightmap;

use super::super::state::*;
use super::super::utils::calculate_squad_centers;

/// Small offset above terrain for visual markers to prevent z-fighting
const VISUAL_TERRAIN_OFFSET: f32 = 0.5;

/// System: Update and cleanup selection ring visuals
pub fn selection_visual_system(
    mut commands: Commands,
    mut selection_state: ResMut<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut existing_visuals: Query<(Entity, &mut SelectionVisual, &MeshMaterial3d<StandardMaterial>, &Mesh3d)>,
    mut visual_transforms: Query<&mut Transform, With<SelectionVisual>>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Option<Res<TerrainHeightmap>>,
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
    for (entity, visual, _, _) in existing_visuals.iter() {
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
        .map(|(_, v, _, _)| v.squad_id)
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

            spawn_selection_ring(
                &mut commands,
                &mut meshes,
                &mut materials,
                squad_id,
                position,
                is_grouped,
                heightmap.as_deref()
            );
        }
    }

    // Update positions of existing visuals
    // Regenerate mesh when squad moves to resample terrain at new position
    for (entity, mut visual, material_handle, mesh_handle) in existing_visuals.iter_mut() {
        if let Some(&actual_center) = squad_actual_centers.get(&visual.squad_id) {
            if let Ok(mut transform) = visual_transforms.get_mut(entity) {
                // Check if position has changed significantly (more than 0.1 units)
                let position_changed = transform.translation.distance(actual_center) > 0.1;

                if position_changed {
                    // Regenerate mesh with new terrain sampling
                    let new_mesh = create_terrain_conforming_ring(
                        actual_center,
                        SELECTION_RING_INNER_RADIUS,
                        SELECTION_RING_OUTER_RADIUS,
                        heightmap.as_deref(),
                    );

                    // Update the mesh asset
                    if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                        *mesh = new_mesh;
                    }

                    // Update transform position
                    let base_y = if let Some(hm) = heightmap.as_deref() {
                        hm.sample_height(actual_center.x, actual_center.z) + VISUAL_TERRAIN_OFFSET
                    } else {
                        actual_center.y
                    };
                    transform.translation = Vec3::new(actual_center.x, base_y, actual_center.z);
                }
            }
        }

        // Check if group status changed and update color
        let is_now_grouped = selection_state.squad_to_group.contains_key(&visual.squad_id);
        if visual.is_grouped != is_now_grouped {
            visual.is_grouped = is_now_grouped;
            // Update material color
            if let Some(material) = materials.get_mut(&material_handle.0) {
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

/// Spawn a terrain-conforming selection ring under a squad
/// Creates a subdivided ring that deforms to follow terrain contours
fn spawn_selection_ring(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    squad_id: u32,
    position: Vec3,
    is_grouped: bool,
    heightmap: Option<&TerrainHeightmap>,
) {
    // Create a subdivided ring mesh that conforms to terrain
    let mesh = create_terrain_conforming_ring(
        position,
        SELECTION_RING_INNER_RADIUS,
        SELECTION_RING_OUTER_RADIUS,
        heightmap,
    );

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

    // Calculate the base position at center's terrain height
    let base_y = if let Some(hm) = heightmap {
        hm.sample_height(position.x, position.z) + VISUAL_TERRAIN_OFFSET
    } else {
        position.y
    };

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::new(position.x, base_y, position.z)),
        SelectionVisual { squad_id, is_grouped },
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Create a ring mesh that conforms to terrain by sampling heightmap at multiple points
fn create_terrain_conforming_ring(
    center: Vec3,
    inner_radius: f32,
    outer_radius: f32,
    heightmap: Option<&TerrainHeightmap>,
) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;

    const SEGMENTS: usize = 32; // Number of segments around the ring
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Sample the base terrain height at the center for reference
    let center_terrain_y = if let Some(hm) = heightmap {
        hm.sample_height(center.x, center.z) + VISUAL_TERRAIN_OFFSET
    } else {
        center.y
    };

    // Generate vertices for each segment
    // Create mesh in local space with Y relative to center's terrain height
    for i in 0..=SEGMENTS {
        let angle = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
        let cos = angle.cos();
        let sin = angle.sin();

        // Inner vertex - sample world position but store in local space
        let inner_x_world = center.x + cos * inner_radius;
        let inner_z_world = center.z + sin * inner_radius;
        let inner_y_world = if let Some(hm) = heightmap {
            hm.sample_height(inner_x_world, inner_z_world) + VISUAL_TERRAIN_OFFSET
        } else {
            center_terrain_y
        };
        // Store in local space (Y relative to center's terrain height)
        positions.push([cos * inner_radius, inner_y_world - center_terrain_y, sin * inner_radius]);
        normals.push([0.0, 1.0, 0.0]); // Up normal for simplicity
        uvs.push([0.0, i as f32 / SEGMENTS as f32]);

        // Outer vertex - sample world position but store in local space
        let outer_x_world = center.x + cos * outer_radius;
        let outer_z_world = center.z + sin * outer_radius;
        let outer_y_world = if let Some(hm) = heightmap {
            hm.sample_height(outer_x_world, outer_z_world) + VISUAL_TERRAIN_OFFSET
        } else {
            center_terrain_y
        };
        // Store in local space (Y relative to center's terrain height)
        positions.push([cos * outer_radius, outer_y_world - center_terrain_y, sin * outer_radius]);
        normals.push([0.0, 1.0, 0.0]); // Up normal for simplicity
        uvs.push([1.0, i as f32 / SEGMENTS as f32]);
    }

    // Generate triangle indices
    for i in 0..SEGMENTS {
        let base = (i * 2) as u32;
        // First triangle
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 1);
        // Second triangle
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// System: Render box selection rectangle during left-click drag
pub fn box_selection_visual_system(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    window_query: Query<&Window, With<bevy::window::PrimaryWindow>>,
    existing_visual: Query<Entity, With<BoxSelectionVisual>>,
) {
    let Ok(window) = window_query.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        // No cursor - despawn any existing visual
        for entity in existing_visual.iter() {
            commands.entity(entity).despawn();
        }
        return;
    };

    // Check if we should show the box selection visual
    if !selection_state.is_box_selecting {
        // Not box selecting - despawn any existing visual
        for entity in existing_visual.iter() {
            commands.entity(entity).despawn();
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
        commands.entity(entity).despawn();
    }

    // Skip if too small
    if width < 2.0 || height < 2.0 {
        return;
    }

    // Spawn the box selection UI node
    // Using a semi-transparent green box with a border effect
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(min_x),
            top: Val::Px(min_y),
            width: Val::Px(width),
            height: Val::Px(height),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.8, 0.3, 0.15)),
        BorderColor(Color::srgba(0.3, 1.0, 0.4, 0.8)),
        BoxSelectionVisual,
    ));
}
