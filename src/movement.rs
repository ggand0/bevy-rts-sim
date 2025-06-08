// Movement and animation systems module
use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel, MouseMotion};
use crate::types::*;
use crate::constants::*;

pub fn animate_march(
    time: Res<Time>,
    mut query: Query<(&mut BattleDroid, &mut Transform), With<SquadMember>>,
) {
    let time_seconds = time.elapsed_seconds();
    let delta_time = time.delta_seconds();
    
    for (droid, mut transform) in query.iter_mut() {
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
            
            // Face movement direction with sway
            if direction.length() > 0.1 {
                let forward_rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
                let sway = (time_seconds * droid.march_speed * 2.0 + droid.march_offset).sin() * 0.01;
                transform.rotation = forward_rotation * Quat::from_rotation_y(sway);
            }
            
            // Color shifting effect during movement - simulate battle droid energy glow
            let march_cycle = (time_seconds * 3.0 + droid.march_offset).sin();
            let energy_pulse = march_cycle * 0.003; // Much more subtle energy pulse effect
            
            // Apply color shifting as position modulation (simulates energy field)
            transform.translation.y += energy_pulse;
            
            // Add marching bob
            let bob_cycle = (time_seconds * 4.0 + droid.march_offset).sin();
            let bob_height = bob_cycle * 0.004; // Reduced marching bob to prevent excessive movement
            transform.translation.y += bob_height;
        } else {
            // Even when stationary, add very subtle idle animation
            let idle_cycle = (time_seconds * 1.5 + droid.march_offset).sin();
            let idle_bob = idle_cycle * 0.002; // Very subtle idle movement
            transform.translation.y += idle_bob;
        }
        
        // Formation correction color shift - units show energy during reformation
        // This creates a visual effect when units are adjusting to formation positions
        let formation_stress = (time_seconds * 5.0 + droid.march_offset * 2.0).sin();
        let formation_energy = formation_stress * 0.002; // Much more subtle formation adjustment glow
        transform.translation.y += formation_energy;
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
            "{} vs {} Units ({} squads/team) | FPS: {:.1}\nWSAD: Move | Mouse: Rotate | Scroll: Zoom | F: Volley Fire\nRectangle Formation Only | G: Advance | H: Retreat",
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
        
        // Mouse drag rotation
        if mouse_button_input.pressed(MouseButton::Left) {
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


