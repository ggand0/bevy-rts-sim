// Formation systems module
use bevy::prelude::*;
use std::collections::HashMap;
use crate::types::*;
use crate::constants::*;

// Formation calculation functions
pub fn calculate_formation_offset(
    formation_type: FormationType,
    row: usize,
    column: usize,
    facing_direction: Vec3,
) -> Vec3 {
    match formation_type {
        FormationType::Rectangle => calculate_rectangle_offset(row, column, facing_direction),
    }
}

fn calculate_rectangle_offset(row: usize, column: usize, facing_direction: Vec3) -> Vec3 {
    // Standard rectangular formation (10 wide x 5 deep)
    // Row 0 is front (faces facing_direction), row 4 is rear (commander position)
    let x_offset = (column as f32 - (SQUAD_WIDTH as f32 - 1.0) / 2.0) * SQUAD_HORIZONTAL_SPACING;
    let z_offset = (row as f32 - (SQUAD_DEPTH as f32 - 1.0) / 2.0) * SQUAD_VERTICAL_SPACING;

    // Calculate perpendicular direction for width
    let right = Vec3::new(facing_direction.z, 0.0, -facing_direction.x).normalize();

    // Negate z_offset so row 0 is in front (positive facing_direction) and commander is rear
    right * x_offset - facing_direction * z_offset
}

pub fn assign_formation_positions(formation_type: FormationType) -> Vec<(usize, usize)> {
    match formation_type {
        FormationType::Rectangle => {
            let mut positions = Vec::new();
            for row in 0..SQUAD_DEPTH {
                for col in 0..SQUAD_WIDTH {
                    positions.push((row, col));
                    if positions.len() >= SQUAD_SIZE {
                        break;
                    }
                }
                if positions.len() >= SQUAD_SIZE {
                    break;
                }
            }
            positions
        },
    }
}

pub fn get_commander_position(formation_type: FormationType) -> (usize, usize) {
    match formation_type {
        FormationType::Rectangle => {
            // Rear-center: back row, middle column
            (SQUAD_DEPTH - 1, SQUAD_WIDTH / 2)
        },
    }
}

// Squad formation maintenance system
pub fn squad_formation_system(
    time: Res<Time>,
    mut squad_manager: ResMut<SquadManager>,
    mut unit_query: Query<(&mut Transform, &SquadMember, &mut FormationOffset, &BattleDroid), With<BattleDroid>>,
) {
    // Only update squad centers periodically, not every frame
    static mut LAST_UPDATE_TIME: f32 = 0.0;
    let current_time = time.elapsed_seconds();
    let should_update_centers = unsafe {
        if current_time - LAST_UPDATE_TIME < 0.1 { // Update only 10 times per second
            false
        } else {
            LAST_UPDATE_TIME = current_time;
            true
        }
    };
    
    if should_update_centers {
        // Calculate current average positions of units per squad
        let mut squad_current_centers: HashMap<u32, Vec<Vec3>> = HashMap::new();

        for (transform, squad_member, _, _droid) in unit_query.iter() {
            squad_current_centers.entry(squad_member.squad_id)
                              .or_insert_with(Vec::new)
                              .push(transform.translation);
        }

        // Update squad centers - use target_position as anchor when squads have a move target
        for (squad_id, current_positions) in squad_current_centers.iter() {
            if let Some(squad) = squad_manager.get_squad_mut(*squad_id) {
                if !current_positions.is_empty() {
                    let current_center = current_positions.iter().sum::<Vec3>() / current_positions.len() as f32;

                    // Check if squad is actively moving toward a target
                    let distance_to_target = Vec3::new(
                        current_center.x - squad.target_position.x,
                        0.0,
                        current_center.z - squad.target_position.z,
                    ).length();

                    if distance_to_target > 5.0 {
                        // Squad is moving - smoothly transition center toward target
                        // Use current average as the center (tracks actual unit positions during movement)
                        squad.center_position = current_center;
                    } else {
                        // Squad has arrived - anchor to target position for clean formation
                        // Blend toward target to ensure proper line alignment
                        let blend = 0.3; // Gradual snap to target
                        squad.center_position = squad.center_position * (1.0 - blend) + squad.target_position * blend;
                    }
                }
            }
        }
    }
    
    // Now update unit formation positions with cached formation offsets
    for (mut transform, squad_member, mut formation_offset, droid) in unit_query.iter_mut() {
        if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
            // Only update formation targets when not retreating to prevent interference
            if !droid.returning_to_spawn {
                // Recalculate formation offset with current facing direction
                let new_offset = calculate_formation_offset(
                    squad.formation_type,
                    squad_member.formation_position.0,
                    squad_member.formation_position.1,
                    squad.facing_direction,
                );
                formation_offset.local_offset = new_offset;

                // Use target_position as the formation anchor for clean final alignment
                let correct_target_position = squad.target_position + formation_offset.local_offset;
                formation_offset.target_world_position = correct_target_position;
            }

            // Apply formation correction - stronger during advance to maintain formation
            let direction = formation_offset.target_world_position - transform.translation;
            let distance = direction.length();

            // Check if unit has arrived at its target position
            let distance_to_target = Vec3::new(
                transform.translation.x - droid.target_position.x,
                0.0,
                transform.translation.z - droid.target_position.z,
            ).length();
            let has_arrived = distance_to_target < 2.0;
            let is_actively_moving = !has_arrived && !droid.returning_to_spawn;
            
            // DISABLE FORMATION CORRECTION DURING MOVEMENT: Don't apply formation correction if unit is actively marching or retreating
            let should_apply_formation_correction = if is_actively_moving || droid.returning_to_spawn {
                // COMPLETELY DISABLE formation correction during active march/retreat to prevent interference
                false // No formation correction at all during advance or retreat
            } else {
                // Normal formation correction when arrived at destination
                distance > 0.2
            };

            // Apply formation correction if needed - much more careful during movement
            if should_apply_formation_correction && distance < 50.0 {
                // Get squad size to adapt correction strength
                let squad_size = if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
                    squad.members.len()
                } else {
                    SQUAD_SIZE
                };
                let squad_strength_ratio = squad_size as f32 / SQUAD_SIZE as f32;

                // Strong correction when arrived for rapid formation completion
                let base_correction_strength = 3.0;

                // Reduce correction for smaller squads to prevent over-correction and drift
                let size_modifier = if squad_strength_ratio < 0.3 {
                    0.8 // Still decent correction for very small squads
                } else if squad_strength_ratio < 0.6 {
                    0.9 // Good correction for reduced squads
                } else {
                    1.0 // Normal correction for healthy squads
                };

                // Slight variation to prevent units from all moving identically
                let variation_factor = droid.march_offset.sin() * 0.1 + 1.0; // 0.9 to 1.1 multiplier

                let final_strength = base_correction_strength * size_modifier * variation_factor;
                let movement = direction.normalize() * (distance * final_strength) * time.delta_seconds();

                // CRITICAL: Only apply horizontal corrections to avoid interfering with Y-axis animations
                let correction_movement = Vec3::new(movement.x, 0.0, movement.z);

                transform.translation += correction_movement;
            }
        }
    }
}

/// System: Smoothly rotate squad facing direction toward target facing direction
/// and update unit formation positions accordingly
pub fn squad_rotation_system(
    time: Res<Time>,
    mut squad_manager: ResMut<SquadManager>,
    mut droid_query: Query<(&mut BattleDroid, &SquadMember)>,
) {
    let delta_time = time.delta_seconds();

    // Collect squads that need rotation updates
    let mut squads_to_update: Vec<(u32, Vec3)> = Vec::new();

    for (squad_id, squad) in squad_manager.squads.iter_mut() {
        // Check if squad needs to rotate
        let dot = squad.facing_direction.dot(squad.target_facing_direction);

        if dot < 0.9999 {
            // Smoothly interpolate facing direction toward target
            // Use slerp-like behavior via cross product and angle
            let cross = squad.facing_direction.cross(squad.target_facing_direction);
            let angle_sign = if cross.y >= 0.0 { 1.0 } else { -1.0 };

            // Calculate rotation amount this frame
            let max_rotation = SQUAD_ROTATION_SPEED * delta_time;
            let angle_diff = squad.facing_direction.angle_between(squad.target_facing_direction);
            let rotation_amount = angle_diff.min(max_rotation);

            // Rotate facing direction
            let rotation = Quat::from_rotation_y(angle_sign * rotation_amount);
            squad.facing_direction = (rotation * squad.facing_direction).normalize();

            // Mark this squad for unit position update
            squads_to_update.push((*squad_id, squad.facing_direction));
        }
    }

    // Update unit target positions for rotated squads
    for (squad_id, new_facing) in squads_to_update {
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            let target_pos = squad.target_position;
            let formation_type = squad.formation_type;

            for (mut droid, squad_member) in droid_query.iter_mut() {
                if squad_member.squad_id == squad_id && !droid.returning_to_spawn {
                    // Recalculate formation offset with new facing direction
                    let new_offset = calculate_formation_offset(
                        formation_type,
                        squad_member.formation_position.0,
                        squad_member.formation_position.1,
                        new_facing,
                    );

                    // Preserve unit's spawn Y height when updating target position
                    let target_xz = target_pos + new_offset;
                    droid.target_position = Vec3::new(target_xz.x, droid.spawn_position.y, target_xz.z);
                }
            }
        }
    }
}

// Squad casualty management system
pub fn squad_casualty_management_system(
    _commands: Commands,
    mut squad_manager: ResMut<SquadManager>,
    mut removed_units: RemovedComponents<BattleDroid>,
    squad_query: Query<&SquadMember>,
) {
    // Handle unit deaths/removals
    for entity in removed_units.read() {
        if let Ok(squad_member) = squad_query.get(entity) {
            squad_manager.remove_unit_from_squad(entity);
            
            // Check if we need to reform the squad
            if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
                if squad.members.len() < 10 { // Less than 20% strength
                    // Could trigger squad merge or retreat behavior here
                }
            }
        }
    }
}

// Squad movement control system (separate from formation switching)
pub fn squad_movement_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut squad_manager: ResMut<SquadManager>,
    mut droid_query: Query<&mut BattleDroid, With<SquadMember>>,
) {
    let mut should_advance = false;
    let mut should_retreat = false;
    
    if keyboard_input.just_pressed(KeyCode::KeyG) {
        should_advance = true;
        info!("All squads advance!");
    } else if keyboard_input.just_pressed(KeyCode::KeyH) {
        should_retreat = true;
        info!("All squads retreat!");
    }
    
    if should_advance || should_retreat {
        // Update all squad target positions
        for squad in squad_manager.squads.values_mut() {
            if should_advance {
                squad.target_position = squad.center_position + squad.facing_direction * MARCH_DISTANCE;
            } else {
                squad.target_position = squad.center_position;
            }
        }
        
        // Update individual unit targets
        for mut droid in droid_query.iter_mut() {
            if should_advance {
                // Calculate advance direction based on team facing
                let team_direction = if droid.team == Team::A {
                    Vec3::new(1.0, 0.0, 0.0) // Team A faces right
                } else {
                    Vec3::new(-1.0, 0.0, 0.0) // Team B faces left
                };
                droid.target_position = droid.spawn_position + team_direction * MARCH_DISTANCE;
                droid.returning_to_spawn = false;
            } else {
                droid.target_position = droid.spawn_position;
                droid.returning_to_spawn = true;
            }
        }
    }
}



// Commander visual debug marker system
#[derive(Component)]
pub struct CommanderMarker {
    commander_entity: Entity,
}

pub fn commander_visual_marker_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing_markers_query: Query<(Entity, &CommanderMarker)>,
    all_commanders_query: Query<(Entity, &Transform, &SquadMember, &BattleDroid), With<BattleDroid>>,
) {
    // Remove markers for units that are no longer commanders
    for (marker_entity, marker) in existing_markers_query.iter() {
        let commander_still_exists = all_commanders_query.iter()
            .any(|(entity, _, squad_member, _)| entity == marker.commander_entity && squad_member.is_commander);
        
        if !commander_still_exists {
            commands.entity(marker_entity).despawn();
        }
    }
    
    // Add markers for new commanders or commanders that changed
    for (entity, transform, squad_member, droid) in all_commanders_query.iter() {
        if squad_member.is_commander {
            // Check if this commander already has a marker
            let has_marker = existing_markers_query.iter()
                .any(|(_, marker)| marker.commander_entity == entity);
            
            if !has_marker {
                // Create debug marker above commander
                let marker_color = match droid.team {
                    Team::A => Color::srgb(1.0, 1.0, 0.0), // Bright yellow for Team A commanders
                    Team::B => Color::srgb(1.0, 0.0, 0.0), // Bright red for Team B commanders
                };
                
                let marker_mesh = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
                let marker_material = materials.add(StandardMaterial {
                    base_color: marker_color,
                    emissive: marker_color.into(),
                    unlit: true, // Make it glow
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
                
                // Spawn marker above commander
                commands.spawn((
                    PbrBundle {
                        mesh: marker_mesh,
                        material: marker_material,
                        transform: Transform::from_translation(transform.translation + Vec3::new(0.0, 3.0, 0.0))
                            .with_scale(Vec3::splat(0.8)),
                        ..default()
                    },
                    CommanderMarker {
                        commander_entity: entity,
                    },
                ));
            }
        }
    }
}

// Update commander marker positions to follow commanders
pub fn update_commander_markers_system(
    commander_query: Query<(Entity, &Transform, &SquadMember), (With<BattleDroid>, With<SquadMember>)>,
    mut marker_query: Query<(&mut Transform, &CommanderMarker), (With<CommanderMarker>, Without<BattleDroid>)>,
) {
    for (mut marker_transform, marker) in marker_query.iter_mut() {
        if let Ok((_, commander_transform, squad_member)) = commander_query.get(marker.commander_entity) {
            if squad_member.is_commander {
                // Update marker position to stay above commander
                marker_transform.translation = commander_transform.translation + Vec3::new(0.0, 3.0, 0.0);
                
                // Add a slight rotation animation for visibility
                marker_transform.rotation *= Quat::from_rotation_y(0.02);
            }
        }
    }
} 