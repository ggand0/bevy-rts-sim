// Movement and animation systems module
use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel, MouseMotion};
use crate::types::*;
use crate::constants::*;

pub fn animate_march(
    time: Res<Time>,
    squad_manager: Res<SquadManager>,
    mut query: Query<(&mut BattleDroid, &mut Transform, &SquadMember)>,
) {
    let time_seconds = time.elapsed_seconds();
    let delta_time = time.delta_seconds();

    for (droid, mut transform, squad_member) in query.iter_mut() {
        // Only move if explicitly commanded (no automatic cycling)
        let should_move = if droid.returning_to_spawn {
            // Moving back to spawn position
            let distance_to_spawn = transform.translation.distance(droid.spawn_position);
            distance_to_spawn > 1.0
        } else {
            // Moving to target position (only if target is different from spawn)
            let distance_to_target = transform.translation.distance(droid.target_position);
            distance_to_target > 1.0 && droid.target_position != droid.spawn_position
        };

        if should_move {
            let current_target = if droid.returning_to_spawn {
                droid.spawn_position
            } else {
                droid.target_position
            };

            // Calculate direction to target
            let direction = (current_target - transform.translation).normalize_or_zero();

            // Move towards target
            let movement = direction * MARCH_SPEED * delta_time * droid.march_speed;
            transform.translation += movement;

            // Add marching animation - subtle bobbing motion
            let march_cycle = (time_seconds * droid.march_speed * 4.0 + droid.march_offset).sin();
            let bob_height = march_cycle * 0.008; // Very subtle up/down movement
            transform.translation.y += bob_height;

            // Prevent units from sinking below their spawn height
            transform.translation.y = transform.translation.y.max(droid.spawn_position.y);

            // Slight rotation for more natural look and face movement direction
            let sway = (time_seconds * droid.march_speed * 2.0 + droid.march_offset).sin() * 0.01;
            if direction.length() > 0.1 {
                let forward_rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
                transform.rotation = forward_rotation * Quat::from_rotation_y(sway);
            }
        } else {
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

pub fn update_camera_info(
    mut query: Query<&mut Text>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    if let Ok(mut text) = query.get_single_mut() {
        let fps = diagnostics
            .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
            .unwrap_or(0.0);
            
        text.sections[0].value = format!(
            "{} vs {} Units ({} squads/team) | FPS: {:.1}\nLeft-click: Select | Right-click: Move | Middle-drag: Rotate | Scroll: Zoom\nShift+click: Add to selection | G: Advance All | H: Retreat All | F: Volley Fire",
            ARMY_SIZE_PER_TEAM, ARMY_SIZE_PER_TEAM, ARMY_SIZE_PER_TEAM / SQUAD_SIZE, fps
        );
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
    if let Ok((mut transform, mut camera)) = camera_query.get_single_mut() {
        let delta_time = time.delta_seconds();
        
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


