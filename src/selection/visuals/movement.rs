// Movement visuals - move indicators, path lines, orientation arrows, path arrows
use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use std::collections::HashSet;
use crate::types::*;
use crate::constants::*;
use crate::terrain::TerrainHeightmap;

use super::super::state::*;
use super::super::utils::{calculate_squad_centers, horizontal_distance, horizontal_direction};

/// Small offset above terrain for visual markers to prevent z-fighting
const VISUAL_TERRAIN_OFFSET: f32 = 0.5;

/// System: Fade out and cleanup move order visuals
pub fn move_visual_cleanup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut circle_query: Query<(Entity, &mut MoveOrderVisual, &MeshMaterial3d<StandardMaterial>), Without<MovePathVisual>>,
    mut path_query: Query<(Entity, &mut MovePathVisual, &MeshMaterial3d<StandardMaterial>), Without<MoveOrderVisual>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Handle destination circle visuals
    for (entity, mut visual, material_handle) in circle_query.iter_mut() {
        visual.timer.tick(time.delta());

        // Fade out based on timer progress, preserving original color
        if let Some(material) = materials.get_mut(&material_handle.0) {
            let progress = visual.timer.fraction();
            let alpha = (1.0 - progress) * 0.6;
            // Use the stored base color, just update alpha
            material.base_color = visual.base_color.with_alpha(alpha);
        }

        if visual.timer.finished() {
            commands.entity(entity).despawn();
        }
    }

    // Handle path line visuals
    for (entity, mut visual, material_handle) in path_query.iter_mut() {
        visual.timer.tick(time.delta());

        // Fade out based on timer progress
        if let Some(material) = materials.get_mut(&material_handle.0) {
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
    arrow_query: Query<(Entity, &mut Transform), With<OrientationArrowVisual>>,
    heightmap: Option<Res<TerrainHeightmap>>,
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
    let direction = horizontal_direction(start, current);
    let length = direction.length();

    if length < 0.1 {
        return;
    }

    // Get base terrain height at start position
    let start_terrain_y = heightmap.as_deref()
        .map(|hm| hm.sample_height(start.x, start.z))
        .unwrap_or(-1.0);

    // Check if arrow already exists - if so, despawn and recreate since mesh needs regeneration
    for (entity, _) in arrow_query.iter() {
        commands.entity(entity).despawn();
    }

    // Create new terrain-conforming arrow mesh
    // Head length is 20% of arrow length for better proportions
    let head_length = (current - start).length() * 0.2;
    let arrow_mesh = meshes.add(create_arrow_mesh(
        0.8,
        2.5,
        head_length,
        start,
        current,
        heightmap.as_deref(),
    ));

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
        Mesh3d(arrow_mesh),
        MeshMaterial3d(arrow_material),
        Transform::from_translation(Vec3::new(start.x, start_terrain_y, start.z)),
        OrientationArrowVisual,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// System: Update persistent path arrows for selected squads that are moving
pub fn update_squad_path_arrows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    existing_arrows: Query<(Entity, &SquadPathArrowVisual, &mut Transform), Without<BattleDroid>>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    // Calculate actual squad centers from unit positions
    let squad_actual_centers = calculate_squad_centers(&unit_query);

    // Collect selected squads that have a target position different from current position
    let mut squads_needing_arrows: Vec<(u32, Vec3, Vec3)> = Vec::new(); // (squad_id, current_pos, target_pos)

    for &squad_id in &selection_state.selected_squads {
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            // Get actual center position from unit transforms
            let current_pos = squad_actual_centers.get(&squad_id)
                .copied()
                .unwrap_or(squad.center_position);

            let target_pos = squad.target_position;

            // Check if squad is moving (target is significantly different from current)
            let distance = horizontal_distance(current_pos, target_pos);

            if distance > SQUAD_ARRIVAL_THRESHOLD {
                // Squad is still moving toward a target
                squads_needing_arrows.push((squad_id, current_pos, target_pos));
            }
        }
    }

    // Remove arrows for squads that are no longer selected or no longer moving
    let squads_with_arrows: HashSet<u32> = squads_needing_arrows.iter().map(|(id, _, _)| *id).collect();
    for (entity, arrow, _) in existing_arrows.iter() {
        if !squads_with_arrows.contains(&arrow.squad_id) {
            commands.entity(entity).despawn();
        }
    }

    // Update or create arrows for squads that need them
    for (squad_id, current_pos, target_pos) in squads_needing_arrows {
        let direction = horizontal_direction(current_pos, target_pos);
        let length = direction.length();

        if length < SQUAD_ARRIVAL_THRESHOLD {
            continue;
        }

        // Get base terrain height at start position
        let start_terrain_y = heightmap.as_deref()
            .map(|hm| hm.sample_height(current_pos.x, current_pos.z))
            .unwrap_or(-1.0);

        // Check if arrow already exists for this squad - despawn and recreate for mesh regeneration
        for (entity, arrow, _transform) in existing_arrows.iter() {
            if arrow.squad_id == squad_id {
                commands.entity(entity).despawn();
                break;
            }
        }

        // Always create/recreate arrow with terrain-conforming mesh
        // Head length is 15% of arrow length for better proportions
        let head_length = length * 0.15;
        let arrow_mesh = meshes.add(create_arrow_mesh(
            0.6,
            2.0,
            head_length,
            current_pos,
            target_pos,
            heightmap.as_deref(),
        ));

        let arrow_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.9, 0.4, 0.5),
            emissive: LinearRgba::new(0.1, 0.4, 0.15, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        });

        commands.spawn((
            Mesh3d(arrow_mesh),
            MeshMaterial3d(arrow_material),
            Transform::from_translation(Vec3::new(current_pos.x, start_terrain_y, current_pos.z)),
            SquadPathArrowVisual { squad_id },
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// Create an arrow mesh pointing in +Z direction (will be rotated and scaled)
/// Parameters control the arrow dimensions for different use cases
/// Now creates a terrain-conforming arrow by sampling heightmap along its length
pub fn create_arrow_mesh(
    shaft_width: f32,
    head_width: f32,
    head_length: f32,
    start_pos: Vec3,
    end_pos: Vec3,
    heightmap: Option<&TerrainHeightmap>,
) -> Mesh {
    use bevy::render::mesh::PrimitiveTopology;

    // Calculate arrow direction and length in XZ plane
    let direction = Vec3::new(end_pos.x - start_pos.x, 0.0, end_pos.z - start_pos.z);
    let length = direction.length();

    if length < 0.01 {
        // Fallback to flat arrow if too short
        return create_flat_arrow_mesh(shaft_width, head_width, head_length);
    }

    let dir_normalized = direction.normalize();

    // Perpendicular direction for shaft width (rotate 90 degrees in XZ plane)
    let perp = Vec3::new(-dir_normalized.z, 0.0, dir_normalized.x);

    // Sample heights along the arrow's path
    const SEGMENTS: usize = 8; // Number of segments along shaft
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Get base height at start position for local space reference
    let base_y = if let Some(hm) = heightmap {
        hm.sample_height(start_pos.x, start_pos.z) + VISUAL_TERRAIN_OFFSET
    } else {
        start_pos.y
    };

    // Build shaft with multiple segments
    let shaft_length = length - head_length;
    for i in 0..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let segment_dist = t * shaft_length;

        // Calculate world center position of this segment
        let world_center = start_pos + dir_normalized * segment_dist;
        let world_y = if let Some(hm) = heightmap {
            hm.sample_height(world_center.x, world_center.z) + VISUAL_TERRAIN_OFFSET
        } else {
            base_y
        };

        // Create left and right edge positions in world space
        let left_world = world_center + perp * shaft_width / 2.0;
        let right_world = world_center - perp * shaft_width / 2.0;

        // Convert to local space (relative to start_pos with base_y as reference)
        let left_local = left_world - Vec3::new(start_pos.x, base_y - 0.5, start_pos.z);
        let right_local = right_world - Vec3::new(start_pos.x, base_y - 0.5, start_pos.z);

        // Add vertices
        vertices.push([left_local.x, world_y - base_y + 0.5, left_local.z]);
        vertices.push([right_local.x, world_y - base_y + 0.5, right_local.z]);
    }

    // Build indices for shaft
    for i in 0..SEGMENTS {
        let base = (i * 2) as u32;
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 1);
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    // Add arrow head at the end
    let head_base_world = start_pos + dir_normalized * shaft_length;
    let head_base_y = if let Some(hm) = heightmap {
        hm.sample_height(head_base_world.x, head_base_world.z) + VISUAL_TERRAIN_OFFSET
    } else {
        base_y
    };

    let tip_y = if let Some (hm) = heightmap {
        hm.sample_height(end_pos.x, end_pos.z) + VISUAL_TERRAIN_OFFSET
    } else {
        base_y
    };

    let head_base_idx = vertices.len() as u32;

    // Head base left and right points
    let left_head_world = head_base_world + perp * head_width / 2.0;
    let right_head_world = head_base_world - perp * head_width / 2.0;

    // Convert to local space
    let left_head_local = left_head_world - Vec3::new(start_pos.x, base_y - 0.5, start_pos.z);
    let right_head_local = right_head_world - Vec3::new(start_pos.x, base_y - 0.5, start_pos.z);
    let tip_local = end_pos - Vec3::new(start_pos.x, base_y - 0.5, start_pos.z);

    // Add head vertices
    vertices.push([left_head_local.x, head_base_y - base_y + 0.5, left_head_local.z]);
    vertices.push([right_head_local.x, head_base_y - base_y + 0.5, right_head_local.z]);
    vertices.push([tip_local.x, tip_y - base_y + 0.5, tip_local.z]);

    // Head triangle (counter-clockwise winding)
    indices.push(head_base_idx);
    indices.push(head_base_idx + 1);
    indices.push(head_base_idx + 2);

    let normals = vec![[0.0, 1.0, 0.0]; vertices.len()];
    let uvs = vec![[0.0, 0.0]; vertices.len()];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
}

/// Fallback flat arrow mesh for when heightmap isn't available
fn create_flat_arrow_mesh(shaft_width: f32, head_width: f32, head_length: f32) -> Mesh {
    use bevy::render::mesh::PrimitiveTopology;

    let vertices = vec![
        [-shaft_width / 2.0, 0.5, 0.0],
        [shaft_width / 2.0, 0.5, 0.0],
        [shaft_width / 2.0, 0.5, 1.0 - head_length],
        [-shaft_width / 2.0, 0.5, 1.0 - head_length],
        [-head_width / 2.0, 0.5, 1.0 - head_length],
        [head_width / 2.0, 0.5, 1.0 - head_length],
        [0.0, 0.5, 1.0],
    ];

    let indices = vec![
        0, 2, 1,
        0, 3, 2,
        4, 6, 5,
    ];

    let normals = vec![[0.0, 1.0, 0.0]; 7];
    let uvs = vec![[0.0, 0.0]; 7];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
}

/// Spawn a visual indicator at the move destination
pub fn spawn_move_indicator(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    heightmap: Option<&TerrainHeightmap>,
) {
    spawn_move_indicator_with_color(commands, meshes, materials, position, None, heightmap);
}

/// Spawn a visual indicator at the move destination with custom color
/// If color is None, uses default green color
pub fn spawn_move_indicator_with_color(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    color: Option<Color>,
    heightmap: Option<&TerrainHeightmap>,
) {
    // Create a terrain-conforming circle mesh
    let mesh = create_terrain_conforming_circle(position, MOVE_INDICATOR_RADIUS, heightmap);

    // Determine base color - grey for dead squads, green for living
    let base_color = if color.is_some() {
        // Dead squad: brighter grey
        Color::srgba(0.7, 0.7, 0.7, 0.7)
    } else {
        // Living squad: green
        Color::srgba(0.2, 1.0, 0.3, 0.6)
    };

    let emissive = if color.is_some() {
        LinearRgba::new(0.5, 0.5, 0.5, 1.0)
    } else {
        LinearRgba::new(0.1, 0.5, 0.15, 1.0)
    };

    let material = materials.add(StandardMaterial {
        base_color,
        emissive,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        double_sided: true,
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
        MoveOrderVisual {
            timer: Timer::from_seconds(MOVE_INDICATOR_LIFETIME, TimerMode::Once),
            base_color,  // Store original color for fade-out
        },
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Spawn a path line connecting squad position to destination
#[allow(dead_code)]
pub fn spawn_path_line(
    _commands: &mut Commands,
    _meshes: &mut ResMut<Assets<Mesh>>,
    _materials: &mut ResMut<Assets<StandardMaterial>>,
    start: Vec3,
    end: Vec3,
    _terrain_y: f32,
) {
    let direction = horizontal_direction(start, end);
    let length = direction.length();

    if length < 0.5 {
        return; // Too short to draw
    }

    // Path line spawning disabled - using arrows instead
    // let normalized_dir = direction.normalize();
    // let rotation = Quat::from_rotation_y(normalized_dir.x.atan2(normalized_dir.z));

    // // Create a thin rectangular mesh for the path line
    // let line_mesh = meshes.add(create_path_line_mesh());
    // let line_material = materials.add(StandardMaterial {
    //     base_color: Color::srgba(0.2, 1.0, 0.3, 0.4),
    //     emissive: LinearRgba::new(0.05, 0.3, 0.1, 1.0),
    //     alpha_mode: AlphaMode::Blend,
    //     unlit: true,
    //     cull_mode: None,
    //     double_sided: true,
    //     ..default()
    // });

    // commands.spawn((
    //     Mesh3d(line_mesh),
    //     MeshMaterial3d(line_material),
    //     Transform::from_translation(Vec3::new(start.x, terrain_y + VISUAL_TERRAIN_OFFSET, start.z))
    //         .with_rotation(rotation)
    //         .with_scale(Vec3::new(1.0, 1.0, length)),
    //     MovePathVisual {
    //         timer: Timer::from_seconds(MOVE_INDICATOR_LIFETIME, TimerMode::Once),
    //     },
    //     NotShadowCaster,
    //     NotShadowReceiver,
    // ));
}

/// Create a thin line mesh for path visualization (pointing in +Z, length 1.0)
#[allow(dead_code)]
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

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
}

/// Create a circle mesh that conforms to terrain by sampling heightmap at multiple points
fn create_terrain_conforming_circle(
    center: Vec3,
    radius: f32,
    heightmap: Option<&TerrainHeightmap>,
) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;

    const SEGMENTS: usize = 24; // Number of segments around the circle
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

    // Add center vertex
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    // Generate vertices around the circle
    for i in 0..=SEGMENTS {
        let angle = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
        let cos = angle.cos();
        let sin = angle.sin();

        // Sample world position but store in local space
        let x_world = center.x + cos * radius;
        let z_world = center.z + sin * radius;
        let y_world = if let Some(hm) = heightmap {
            hm.sample_height(x_world, z_world) + VISUAL_TERRAIN_OFFSET
        } else {
            center_terrain_y
        };

        // Store in local space (Y relative to center's terrain height)
        positions.push([cos * radius, y_world - center_terrain_y, sin * radius]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([cos * 0.5 + 0.5, sin * 0.5 + 0.5]);
    }

    // Generate triangle indices (fan from center)
    for i in 0..SEGMENTS {
        indices.push(0); // Center vertex
        indices.push((i + 1) as u32);
        indices.push((i + 2) as u32);
    }

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}
