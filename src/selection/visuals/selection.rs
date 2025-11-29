// Selection ring visuals - cyan rings under selected squads
use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use std::collections::HashSet;
use crate::types::*;
use crate::constants::*;

use super::super::state::*;
use super::super::utils::calculate_squad_centers;

/// System: Update and cleanup selection ring visuals
pub fn selection_visual_system(
    mut commands: Commands,
    mut selection_state: ResMut<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut existing_visuals: Query<(Entity, &mut SelectionVisual, &MeshMaterial3d<StandardMaterial>)>,
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
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::new(position.x, 0.1, position.z))
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        SelectionVisual { squad_id, is_grouped },
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// System: Render box selection rectangle during left-click drag
pub fn box_selection_visual_system(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    window_query: Query<&Window, With<bevy::window::PrimaryWindow>>,
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
