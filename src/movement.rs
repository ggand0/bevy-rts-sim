// Movement and animation systems module
use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel, MouseMotion};
use crate::types::*;
use crate::constants::*;
use crate::terrain::TerrainHeightmap;

/// Unit Y offset above terrain (mesh origin is now at feet, so no offset needed)

/// Updates MovementTracker for all droids - tracks stationary state for accuracy bonuses
pub fn update_movement_tracker(
    time: Res<Time>,
    mut query: Query<(&Transform, &mut MovementTracker), With<BattleDroid>>,
) {
    let delta_time = time.delta_secs();

    for (transform, mut tracker) in query.iter_mut() {
        let current_pos = transform.translation;
        let distance_moved = (current_pos - tracker.last_position).length();

        if distance_moved < ACCURACY_MOVEMENT_THRESHOLD {
            // Unit hasn't moved significantly - accumulate stationary time
            tracker.stationary_timer += delta_time;
            tracker.is_stationary = tracker.stationary_timer >= ACCURACY_STATIONARY_TIME_THRESHOLD;
        } else {
            // Unit moved - reset stationary timer
            tracker.stationary_timer = 0.0;
            tracker.is_stationary = false;
        }

        tracker.last_position = current_pos;
    }
}

pub fn animate_march(
    time: Res<Time>,
    squad_manager: Res<SquadManager>,
    heightmap: Option<Res<TerrainHeightmap>>,
    spatial_grid: Res<SpatialGrid>,
    mut query: Query<(Entity, &mut BattleDroid, &mut Transform, &SquadMember, &MovementMode, &CombatUnit), (Without<KnockbackState>, Without<RagdollDeath>)>,
) {
    let time_seconds = time.elapsed_secs();
    let delta_time = time.delta_secs();

    // Collect all droid positions first for soft avoidance lookups (avoids query conflicts)
    // Only build HashMap if soft avoidance is enabled
    let droid_positions: std::collections::HashMap<Entity, Vec3> = if SOFT_AVOIDANCE_STRENGTH > 0.0 {
        query.iter().map(|(e, _, t, _, _, _)| (e, t.translation)).collect()
    } else {
        std::collections::HashMap::new()
    };

    for (entity, droid, mut transform, squad_member, movement_mode, combat_unit) in query.iter_mut() {
        // Check movement mode - Hold always stops, AttackMove stops when engaged
        let mode_allows_movement = match movement_mode {
            MovementMode::Hold => false,
            MovementMode::AttackMove => combat_unit.current_target.is_none(), // Stop if engaged
            MovementMode::Move => true, // Always allow movement
        };

        // Only move if explicitly commanded (no automatic cycling)
        let should_move = if !mode_allows_movement {
            false
        } else if droid.returning_to_spawn {
            // Moving back to spawn position (use horizontal distance)
            let dx = transform.translation.x - droid.spawn_position.x;
            let dz = transform.translation.z - droid.spawn_position.z;
            (dx * dx + dz * dz).sqrt() > 1.0
        } else {
            // Moving to target position (only if target is different from spawn)
            let dx = transform.translation.x - droid.target_position.x;
            let dz = transform.translation.z - droid.target_position.z;
            let distance_to_target = (dx * dx + dz * dz).sqrt();
            let dx_spawn = droid.target_position.x - droid.spawn_position.x;
            let dz_spawn = droid.target_position.z - droid.spawn_position.z;
            let target_spawn_dist = (dx_spawn * dx_spawn + dz_spawn * dz_spawn).sqrt();
            distance_to_target > 1.0 && target_spawn_dist > 0.1
        };

        if should_move {
            let current_target = if droid.returning_to_spawn {
                droid.spawn_position
            } else {
                droid.target_position
            };

            // Calculate horizontal direction to target (ignore Y for direction)
            let horizontal_dir = Vec3::new(
                current_target.x - transform.translation.x,
                0.0,
                current_target.z - transform.translation.z,
            ).normalize_or_zero();

            // Calculate effective speed with soft avoidance
            let mut effective_speed = MARCH_SPEED * droid.march_speed;

            // Soft avoidance - slow down when near other units (configurable, can be turned off)
            if SOFT_AVOIDANCE_STRENGTH > 0.0 {
                let current_pos = transform.translation;
                let nearby = spatial_grid.get_nearby_droids(current_pos);
                let mut speed_multiplier: f32 = 1.0;

                for other_entity in nearby {
                    if other_entity == entity { continue; }
                    if let Some(&other_pos) = droid_positions.get(&other_entity) {
                        let dx = current_pos.x - other_pos.x;
                        let dz = current_pos.z - other_pos.z;
                        let dist = (dx * dx + dz * dz).sqrt();

                        if dist < SOFT_AVOIDANCE_RADIUS && dist > 0.001 {
                            let factor = dist / SOFT_AVOIDANCE_RADIUS;
                            // Blend based on strength setting
                            let adjusted = 1.0 - (1.0 - factor) * SOFT_AVOIDANCE_STRENGTH;
                            speed_multiplier = speed_multiplier.min(adjusted);
                        }
                    }
                }
                effective_speed *= speed_multiplier;
            }

            // Move horizontally towards target
            let movement = horizontal_dir * effective_speed * delta_time;
            transform.translation.x += movement.x;
            transform.translation.z += movement.z;

            // Sample terrain height at new position and set base Y
            // Subtract bob amplitude so the bob animation can go both up and down from visual center
            let bob_amplitude = 0.1;
            let base_y = if let Some(ref hm) = heightmap {
                hm.sample_height(transform.translation.x, transform.translation.z) - bob_amplitude
            } else {
                droid.spawn_position.y - bob_amplitude
            };

            // Add marching animation - bobbing motion (sin goes -1 to +1, so bob goes -amplitude to +amplitude)
            let march_cycle = (time_seconds * droid.march_speed * 4.0 + droid.march_offset).sin();
            let bob_height = march_cycle * bob_amplitude;
            transform.translation.y = base_y + bob_amplitude + bob_height;

            // Slight rotation for more natural look and face movement direction
            let sway = (time_seconds * droid.march_speed * 2.0 + droid.march_offset).sin() * 0.01;
            if horizontal_dir.length() > 0.1 {
                let forward_rotation = Quat::from_rotation_y(horizontal_dir.x.atan2(horizontal_dir.z));
                transform.rotation = forward_rotation * Quat::from_rotation_y(sway);
            }
        } else {
            // When stationary, still update terrain height (important for map switching)
            if let Some(ref hm) = heightmap {
                let terrain_y = hm.sample_height(transform.translation.x, transform.translation.z);
                transform.translation.y = terrain_y;
            }

            // When stationary, smoothly rotate to face squad's facing direction
            if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
                let facing = squad.facing_direction;
                if facing.length() > 0.1 {
                    let target_rotation = Quat::from_rotation_y(facing.x.atan2(facing.z));
                    // Smoothly interpolate toward target rotation
                    transform.rotation = transform.rotation.slerp(target_rotation, 5.0 * delta_time);
                }
            }
        }
    }
}

pub fn update_fps_display(
    mut query: Query<&mut Text, With<FpsText>>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    if let Ok(mut text) = query.single_mut() {
        let fps = diagnostics
            .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
            .unwrap_or(0.0);

        **text = format!("FPS: {:.0}", fps);
    }
}

pub fn rts_camera_movement(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut scroll_events: EventReader<MouseWheel>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut camera_query: Query<(&mut Transform, &mut RtsCamera)>,
) {
    if let Ok((mut transform, mut camera)) = camera_query.single_mut() {
        let delta_time = time.delta_secs();
        
        // Mouse drag rotation (middle mouse button - left click is for selection)
        if mouse_button_input.pressed(MouseButton::Middle) {
            for motion in mouse_motion_events.read() {
                camera.yaw -= motion.delta.x * CAMERA_ROTATION_SPEED;
                camera.pitch = (camera.pitch - motion.delta.y * CAMERA_ROTATION_SPEED)
                    .clamp(-1.5, -0.1); // Limit pitch to reasonable RTS angles
            }
        } else {
            // Clear mouse motion events if not dragging to prevent accumulation
            mouse_motion_events.clear();
        }
        
        // WASD movement (relative to camera's view direction)
        let mut movement = Vec3::ZERO;
        
        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
            movement.z -= 1.0; // Move North (away from camera in world space)
        }
        if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
            movement.z += 1.0; // Move South (toward camera in world space)
        }
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            movement.x -= 1.0; // Move West (left from camera perspective)
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            movement.x += 1.0; // Move East (right from camera perspective)
        }
        
        // Apply movement relative to camera rotation
        if movement.length() > 0.0 {
            movement = movement.normalize() * CAMERA_SPEED * delta_time;
            
            // Rotate movement vector by camera yaw to make it relative to camera facing
            // Only rotate around Y axis (yaw) to keep movement on the ground plane
            let yaw_rotation = Mat3::from_rotation_y(camera.yaw);
            let rotated_movement = yaw_rotation * movement;
            
            camera.focus_point += rotated_movement;
        }
        
        // Mouse wheel zoom
        for scroll in scroll_events.read() {
            let zoom_delta = match scroll.unit {
                MouseScrollUnit::Line => scroll.y * CAMERA_ZOOM_SPEED,
                MouseScrollUnit::Pixel => scroll.y * CAMERA_ZOOM_SPEED * 0.1,
            };
            
            camera.distance = (camera.distance - zoom_delta)
                .clamp(CAMERA_MIN_HEIGHT, CAMERA_MAX_HEIGHT);
        }
        
        // Update camera transform based on focus point, yaw, pitch, and distance
        let rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
        let offset = rotation * Vec3::new(0.0, 0.0, camera.distance);
        
        transform.translation = camera.focus_point + offset;
        transform.rotation = rotation;
    }
}


