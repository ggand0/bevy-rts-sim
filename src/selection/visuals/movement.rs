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
    mut arrow_query: Query<(Entity, &mut Transform), With<OrientationArrowVisual>>,
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

    let normalized_dir = direction.normalize();
    let arrow_rotation = Quat::from_rotation_y(normalized_dir.x.atan2(normalized_dir.z));

    // Get terrain height at start position
    let terrain_y = heightmap.as_ref()
        .map(|hm| hm.sample_height(start.x, start.z))
        .unwrap_or(-1.0);
    let arrow_y = terrain_y + VISUAL_TERRAIN_OFFSET;

    // Check if arrow already exists
    if let Ok((_, mut transform)) = arrow_query.single_mut() {
        // Update existing arrow
        transform.translation = Vec3::new(start.x, arrow_y, start.z);
        transform.rotation = arrow_rotation;
        transform.scale = Vec3::new(1.0, 1.0, length);
    } else {
        // Create new arrow (elongated triangle pointing in drag direction)
        // Arrow is a simple quad that we'll scale based on drag length
        let arrow_mesh = meshes.add(create_arrow_mesh(0.8, 2.0, 0.15));
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
            Transform::from_translation(Vec3::new(start.x, arrow_y, start.z))
                .with_rotation(arrow_rotation)
                .with_scale(Vec3::new(1.0, 1.0, length)),
            OrientationArrowVisual,
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// System: Update persistent path arrows for selected squads that are moving
pub fn update_squad_path_arrows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    unit_query: Query<(&Transform, &SquadMember), (With<BattleDroid>, Without<SelectionVisual>)>,
    mut existing_arrows: Query<(Entity, &SquadPathArrowVisual, &mut Transform), Without<BattleDroid>>,
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

        let normalized_dir = direction.normalize();
        let rotation = Quat::from_rotation_y(normalized_dir.x.atan2(normalized_dir.z));

        // Get terrain height at current position
        let terrain_y = heightmap.as_ref()
            .map(|hm| hm.sample_height(current_pos.x, current_pos.z))
            .unwrap_or(-1.0);
        let arrow_y = terrain_y + VISUAL_TERRAIN_OFFSET;

        // Check if arrow already exists for this squad
        let mut found = false;
        for (_entity, arrow, mut transform) in existing_arrows.iter_mut() {
            if arrow.squad_id == squad_id {
                // Update existing arrow position/rotation/scale
                transform.translation = Vec3::new(current_pos.x, arrow_y, current_pos.z);
                transform.rotation = rotation;
                transform.scale = Vec3::new(1.0, 1.0, length);
                found = true;
                break;
            }
        }

        if !found {
            // Create new arrow
            let arrow_mesh = meshes.add(create_arrow_mesh(0.5, 1.5, 0.1));
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
                Transform::from_translation(Vec3::new(current_pos.x, arrow_y, current_pos.z))
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(1.0, 1.0, length)),
                SquadPathArrowVisual { squad_id },
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
}

/// Create an arrow mesh pointing in +Z direction (will be rotated and scaled)
/// Parameters control the arrow dimensions for different use cases
pub fn create_arrow_mesh(shaft_width: f32, head_width: f32, head_length: f32) -> Mesh {
    use bevy::render::mesh::PrimitiveTopology;

    // Arrow shape: shaft + head, lying flat on XZ plane
    // Base length is 1.0, will be scaled by drag distance

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
    terrain_y: f32,
) {
    spawn_move_indicator_with_color(commands, meshes, materials, position, None, terrain_y);
}

/// Spawn a visual indicator at the move destination with custom color
/// If color is None, uses default green color
pub fn spawn_move_indicator_with_color(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    color: Option<Color>,
    terrain_y: f32,
) {
    let mesh = meshes.add(Circle::new(MOVE_INDICATOR_RADIUS));

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

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::new(position.x, terrain_y + VISUAL_TERRAIN_OFFSET, position.z))
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        MoveOrderVisual {
            timer: Timer::from_seconds(MOVE_INDICATOR_LIFETIME, TimerMode::Once),
            base_color,  // Store original color for fade-out
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
    terrain_y: f32,
) {
    let direction = horizontal_direction(start, end);
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
        Mesh3d(line_mesh),
        MeshMaterial3d(line_material),
        Transform::from_translation(Vec3::new(start.x, terrain_y + VISUAL_TERRAIN_OFFSET, start.z))
            .with_rotation(rotation)
            .with_scale(Vec3::new(1.0, 1.0, length)),
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

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    mesh
}
