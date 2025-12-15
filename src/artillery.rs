// Artillery barrage system - player-controlled artillery strikes
// Three variants: Single shot, Scatter barrage, Line barrage

use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::window::PrimaryWindow;
use rand::Rng;

use crate::constants::*;
use crate::ground_explosion::{spawn_ground_explosion, FlipbookMaterial, GroundExplosionAssets};
use crate::selection::utils::screen_to_ground_with_heightmap;
use crate::selection::visuals::movement::create_arrow_mesh;
use crate::terrain::TerrainHeightmap;
use crate::types::*;
use crate::wfx_materials::AdditiveMaterial;

// ===== RESOURCES & COMPONENTS =====

/// Artillery mode selection
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArtilleryMode {
    #[default]
    None,
    SingleShot,     // F5: Single explosion at cursor (debug)
    ScatterBarrage, // F6: 6-10 shells scattered around cursor
    LineBarrage,    // F7: Shells along a dragged line
}

/// Artillery state resource
#[derive(Resource, Default)]
pub struct ArtilleryState {
    pub mode: ArtilleryMode,
    pub line_start: Option<Vec3>,
    pub line_current: Option<Vec3>,
    pub is_dragging: bool,
    pub pending_shells: Vec<PendingShell>,
}

/// A pending artillery shell waiting to land
pub struct PendingShell {
    pub position: Vec3,
    pub delay: f32, // Countdown timer
    pub scale: f32,
}

/// Marker for artillery line visual arrow
#[derive(Component)]
pub struct ArtilleryLineArrow;

// ===== SYSTEMS =====

/// Handle artillery hotkeys and input
pub fn artillery_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    heightmap: Option<Res<TerrainHeightmap>>,
    mut artillery_state: ResMut<ArtilleryState>,
) {
    // Toggle modes with V/B/N
    if keyboard.just_pressed(KeyCode::KeyV) {
        artillery_state.mode = if artillery_state.mode == ArtilleryMode::SingleShot {
            info!("Artillery: OFF");
            ArtilleryMode::None
        } else {
            info!("Artillery: Single Shot mode (click to fire)");
            ArtilleryMode::SingleShot
        };
        // Reset state when changing modes
        artillery_state.line_start = None;
        artillery_state.line_current = None;
        artillery_state.is_dragging = false;
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        artillery_state.mode = if artillery_state.mode == ArtilleryMode::ScatterBarrage {
            info!("Artillery: OFF");
            ArtilleryMode::None
        } else {
            info!("Artillery: Scatter Barrage mode (click to call barrage)");
            ArtilleryMode::ScatterBarrage
        };
        artillery_state.line_start = None;
        artillery_state.line_current = None;
        artillery_state.is_dragging = false;
    }

    if keyboard.just_pressed(KeyCode::KeyN) {
        artillery_state.mode = if artillery_state.mode == ArtilleryMode::LineBarrage {
            info!("Artillery: OFF");
            ArtilleryMode::None
        } else {
            info!("Artillery: Line Barrage mode (drag to set line)");
            ArtilleryMode::LineBarrage
        };
        artillery_state.line_start = None;
        artillery_state.line_current = None;
        artillery_state.is_dragging = false;
    }

    // Early exit if no mode active
    if artillery_state.mode == ArtilleryMode::None {
        return;
    }

    // Get cursor position
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let hm = heightmap.as_ref().map(|h| h.as_ref());
    let current_world_pos = screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, hm);

    match artillery_state.mode {
        ArtilleryMode::SingleShot => {
            // Left click to fire single shell
            if mouse_button.just_pressed(MouseButton::Left) {
                if let Some(pos) = current_world_pos {
                    artillery_state.pending_shells.push(PendingShell {
                        position: pos,
                        delay: 0.0, // Immediate
                        scale: 1.0,
                    });
                    info!("Artillery: Single shell at {:?}", pos);
                }
            }
        }
        ArtilleryMode::ScatterBarrage => {
            // Left click to call scatter barrage
            if mouse_button.just_pressed(MouseButton::Left) {
                if let Some(center) = current_world_pos {
                    let mut rng = rand::thread_rng();
                    let shell_count =
                        rng.gen_range(ARTILLERY_SHELL_COUNT_MIN..=ARTILLERY_SHELL_COUNT_MAX);

                    for _ in 0..shell_count {
                        let offset = Vec3::new(
                            rng.gen_range(-ARTILLERY_SCATTER_RADIUS..ARTILLERY_SCATTER_RADIUS),
                            0.0,
                            rng.gen_range(-ARTILLERY_SCATTER_RADIUS..ARTILLERY_SCATTER_RADIUS),
                        );
                        let delay =
                            rng.gen_range(ARTILLERY_SHELL_DELAY_MIN..ARTILLERY_SHELL_DELAY_MAX);

                        // Sample terrain height at shell position
                        let shell_pos = center + offset;
                        let y = hm
                            .map(|h| h.sample_height(shell_pos.x, shell_pos.z))
                            .unwrap_or(0.0);

                        artillery_state.pending_shells.push(PendingShell {
                            position: Vec3::new(shell_pos.x, y, shell_pos.z),
                            delay,
                            scale: 1.0,
                        });
                    }
                    info!(
                        "Artillery: Scatter barrage ({} shells) around {:?}",
                        shell_count, center
                    );
                }
            }
        }
        ArtilleryMode::LineBarrage => {
            // Left click to start drag
            if mouse_button.just_pressed(MouseButton::Left) {
                if let Some(pos) = current_world_pos {
                    artillery_state.line_start = Some(pos);
                    artillery_state.line_current = Some(pos);
                    artillery_state.is_dragging = true;
                }
            }

            // Update drag position
            if mouse_button.pressed(MouseButton::Left) && artillery_state.is_dragging {
                if let Some(pos) = current_world_pos {
                    artillery_state.line_current = Some(pos);
                }
            }

            // Release to fire line barrage
            if mouse_button.just_released(MouseButton::Left) && artillery_state.is_dragging {
                if let (Some(start), Some(end)) =
                    (artillery_state.line_start, artillery_state.line_current)
                {
                    let direction = end - start;
                    let mut line_length = direction.length();

                    // Clamp to max length
                    if line_length > ARTILLERY_LINE_MAX_LENGTH {
                        line_length = ARTILLERY_LINE_MAX_LENGTH;
                    }

                    if line_length > 1.0 {
                        let dir_normalized = direction.normalize();
                        let shell_count =
                            (line_length / ARTILLERY_LINE_SHELL_SPACING).ceil() as usize;
                        let shell_count = shell_count.max(2); // At least 2 shells

                        let mut rng = rand::thread_rng();

                        for i in 0..shell_count {
                            let t = if shell_count > 1 {
                                i as f32 / (shell_count - 1) as f32
                            } else {
                                0.5
                            };
                            let base_pos = start + dir_normalized * (t * line_length);

                            // Add small random scatter perpendicular to line
                            let perp = Vec3::new(-dir_normalized.z, 0.0, dir_normalized.x);
                            let scatter = perp * rng.gen_range(-3.0..3.0);
                            let shell_pos = base_pos + scatter;

                            // Sample terrain height
                            let y = hm
                                .map(|h| h.sample_height(shell_pos.x, shell_pos.z))
                                .unwrap_or(0.0);

                            // Stagger timing along line
                            let delay = i as f32 * 0.25 + rng.gen_range(0.0..0.1);

                            artillery_state.pending_shells.push(PendingShell {
                                position: Vec3::new(shell_pos.x, y, shell_pos.z),
                                delay,
                                scale: 1.0,
                            });
                        }
                        info!(
                            "Artillery: Line barrage ({} shells) from {:?} to {:?}",
                            shell_count, start, end
                        );
                    }
                }

                // Reset drag state
                artillery_state.line_start = None;
                artillery_state.line_current = None;
                artillery_state.is_dragging = false;
            }
        }
        ArtilleryMode::None => {}
    }
}

/// Update artillery line visual (red arrow for line barrage)
pub fn artillery_visual_system(
    mut commands: Commands,
    artillery_state: Res<ArtilleryState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    arrow_query: Query<Entity, With<ArtilleryLineArrow>>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    // Clean up existing arrow if not in line barrage drag
    if artillery_state.mode != ArtilleryMode::LineBarrage || !artillery_state.is_dragging {
        for entity in arrow_query.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let Some(start) = artillery_state.line_start else {
        return;
    };
    let Some(current) = artillery_state.line_current else {
        return;
    };

    // Calculate arrow properties
    let direction = current - start;
    let length = direction.length();

    if length < 1.0 {
        // Too short, remove arrow
        for entity in arrow_query.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Clamp end position to max length
    let clamped_end = if length > ARTILLERY_LINE_MAX_LENGTH {
        start + direction.normalize() * ARTILLERY_LINE_MAX_LENGTH
    } else {
        current
    };

    // Remove existing arrow and recreate (mesh needs regeneration)
    for entity in arrow_query.iter() {
        commands.entity(entity).despawn();
    }

    // Get base terrain height
    let start_terrain_y = heightmap
        .as_deref()
        .map(|hm| hm.sample_height(start.x, start.z))
        .unwrap_or(-1.0);

    // Create arrow mesh
    let head_length = (clamped_end - start).length() * 0.2;
    let arrow_mesh = meshes.add(create_arrow_mesh(
        1.0,  // shaft_width
        3.0,  // head_width
        head_length,
        start,
        clamped_end,
        heightmap.as_deref(),
    ));

    // Red material for artillery indicator
    let arrow_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.2, 0.2, 0.8),
        emissive: LinearRgba::new(0.5, 0.1, 0.1, 1.0),
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
        ArtilleryLineArrow,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Process pending shells and spawn explosions
pub fn artillery_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    mut artillery_state: ResMut<ArtilleryState>,
    ground_assets: Option<Res<GroundExplosionAssets>>,
    mut flipbook_materials: ResMut<Assets<FlipbookMaterial>>,
    mut additive_materials: ResMut<Assets<AdditiveMaterial>>,
    mut area_damage_events: EventWriter<AreaDamageEvent>,
    camera_query: Query<&GlobalTransform, With<RtsCamera>>,
    audio_assets: Option<Res<AudioAssets>>,
) {
    let dt = time.delta_secs();

    // Update timers and collect ready shells
    let mut shells_to_spawn = Vec::new();
    artillery_state.pending_shells.retain_mut(|shell| {
        shell.delay -= dt;
        if shell.delay <= 0.0 {
            shells_to_spawn.push((shell.position, shell.scale));
            false // Remove from pending
        } else {
            true // Keep in pending
        }
    });

    // Spawn explosions for ready shells
    if !shells_to_spawn.is_empty() {
        let Some(assets) = ground_assets.as_ref() else {
            return;
        };

        let camera_transform = camera_query.single().ok();

        for (position, scale) in shells_to_spawn {
            // Spawn ground explosion
            spawn_ground_explosion(
                &mut commands,
                assets,
                &mut flipbook_materials,
                &mut additive_materials,
                position,
                scale,
                camera_transform,
                audio_assets.as_deref(),
            );

            // Fire area damage event
            area_damage_events.write(AreaDamageEvent { position, scale });
        }
    }
}
