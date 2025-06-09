// Enhanced sprite-based explosion effects with real sprite sheets and normal maps
use bevy::prelude::*;
use std::time::Duration;
use bevy::render::alpha::AlphaMode;



// ===== PLUGIN =====

pub struct ExplosionShaderPlugin;

impl Plugin for ExplosionShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_explosion_assets)
            .add_systems(Update, (
                update_explosion_timers,
                animate_sprite_explosions,
                animate_uv_sprite_explosions,
                cleanup_finished_explosions,
                debug_test_explosions,
            ));
    }
}

// ===== EXPLOSION COMPONENTS =====

#[derive(Component)]
pub struct ExplosionTimer {
    timer: Timer,
}

#[derive(Component)]
pub struct SpriteExplosion {
    pub explosion_type: ExplosionType,
    pub current_phase: ExplosionPhase,
    pub frame_count: usize,
    pub current_frame: usize,
    pub frame_duration: f32,
    pub frame_timer: f32,
    pub scale: f32,
    pub fade_alpha: f32,
    pub phase_transition_timer: f32,
}

#[derive(Component)]
pub struct AnimatedSpriteExplosion {
    pub explosion_type: ExplosionType,
    pub current_phase: ExplosionPhase,
    pub frame_count: usize,
    pub current_frame: usize,
    pub frame_duration: f32,
    pub frame_timer: f32,
    pub scale: f32,
    pub fade_alpha: f32,
}

#[derive(PartialEq, Clone)]
pub enum ExplosionType {
    Fire,
    Smoke,
    Nuclear,
    Impact,
}

#[derive(PartialEq, Clone)]
pub enum ExplosionPhase {
    Initial,    // normal+ - bright intense phase
    Secondary,  // normal- - dimmer cooling phase  
    Smoke,      // smoke - aftermath
}

#[derive(Resource)]
pub struct ExplosionAssets {
    // Real sprite sheet textures
    pub normal_plus_texture: Handle<Image>,     // Bright explosion sprite sheet
    pub normal_minus_texture: Handle<Image>,    // Dimmer explosion sprite sheet
    pub smoke_texture: Handle<Image>,           // Smoke sprite sheet
    
    // TextureAtlas layouts for sprite animation (8x8 grids)
    pub normal_plus_atlas: Handle<TextureAtlasLayout>,
    pub normal_minus_atlas: Handle<TextureAtlasLayout>,
    pub smoke_atlas: Handle<TextureAtlasLayout>,
    
    // Materials for different phases
    pub explosion_bright_material: Handle<StandardMaterial>,
    pub explosion_dim_material: Handle<StandardMaterial>,
    pub smoke_material: Handle<StandardMaterial>,
}

// ===== SETUP EXPLOSION ASSETS =====

fn setup_explosion_assets(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    asset_server: Res<AssetServer>,
) {
    // Load the actual sprite sheet textures from assets/textures
    let normal_plus_texture = asset_server.load("textures/normal+.png");
    let normal_minus_texture = asset_server.load("textures/normal-.png");
    let smoke_texture = asset_server.load("textures/smokesprite.png");
    
    // Create TextureAtlas layouts for 8x8 sprite grids (64 frames each)
    let atlas_layout = TextureAtlasLayout::from_grid(UVec2::splat(8), 8, 8, None, None);
    let normal_plus_atlas = texture_atlas_layouts.add(atlas_layout.clone());
    let normal_minus_atlas = texture_atlas_layouts.add(atlas_layout.clone());
    let smoke_atlas = texture_atlas_layouts.add(atlas_layout);
    
    // Create materials that use the sprite textures
    let explosion_bright_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgb(2.0, 1.5, 0.8).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: false, // Enable lighting for depth
        cull_mode: None, // Disable backface culling for billboards
        ..default()
    });
    
    let explosion_dim_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.85),
        emissive: Color::srgb(1.0, 0.8, 0.4).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: false,
        cull_mode: None,
        ..default()
    });
    
    let smoke_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.7),
        alpha_mode: AlphaMode::Blend,
        unlit: false,
        cull_mode: None,
        ..default()
    });

    commands.insert_resource(ExplosionAssets {
        normal_plus_texture,
        normal_minus_texture,
        smoke_texture,
        normal_plus_atlas,
        normal_minus_atlas,
        smoke_atlas,
        explosion_bright_material,
        explosion_dim_material,
        smoke_material,
    });
    
    info!("🎨 Real sprite sheet explosion assets loaded!");
}

// ===== MAIN EXPLOSION SPAWNING FUNCTIONS =====

// NEW: Real animated sprite explosion function
pub fn spawn_animated_sprite_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    explosion_assets: &ExplosionAssets,
    position: Vec3,
    radius: f32,
    intensity: f32,
    duration: f32,
) {
    info!("🎬 Spawning ANIMATED sprite explosion at {} with radius {} intensity {}", position, radius, intensity);
    
    // Create a quad mesh for sprite billboard
    let quad_mesh = meshes.add(Rectangle::new(radius * 0.3, radius * 0.3));
    
    // Choose explosion type based on intensity
    let explosion_type = if intensity > 2.5 {
        ExplosionType::Nuclear
    } else if intensity > 1.5 {
        ExplosionType::Fire
    } else {
        ExplosionType::Impact
    };
    
    // Create material with the actual sprite sheet texture
    let sprite_material = materials.add(StandardMaterial {
        base_color_texture: Some(explosion_assets.normal_plus_texture.clone()),
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgb(2.0, 1.5, 0.8).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: false,
        cull_mode: None,
        ..default()
    });
    
    // Spawn animated sprite explosion
    commands.spawn((
        PbrBundle {
            mesh: quad_mesh.clone(),
            material: sprite_material,
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(0.01)),
            ..default()
        },
        ExplosionTimer {
            timer: Timer::new(Duration::from_secs_f32(duration), TimerMode::Once),
        },
        SpriteExplosion {
            explosion_type: explosion_type.clone(),
            current_phase: ExplosionPhase::Initial,
            frame_count: 64, // 8x8 grid
            current_frame: 0,
            frame_duration: duration / 64.0,
            frame_timer: 0.0,
            scale: radius,
            fade_alpha: 1.0,
            phase_transition_timer: 0.0,
        },
        Name::new("AnimatedSpriteExplosion"),
    ));
    
    // Add smoke for large explosions
    if radius > 5.0 {
        let smoke_material = materials.add(StandardMaterial {
            base_color_texture: Some(explosion_assets.smoke_texture.clone()),
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: false,
            cull_mode: None,
            ..default()
        });
        
        commands.spawn((
            PbrBundle {
                mesh: quad_mesh,
                material: smoke_material,
                transform: Transform::from_translation(position + Vec3::Y * 1.5)
                    .with_scale(Vec3::splat(0.02)),
                ..default()
            },
            ExplosionTimer {
                timer: Timer::new(Duration::from_secs_f32(duration * 2.5), TimerMode::Once),
            },
            SpriteExplosion {
                explosion_type: ExplosionType::Smoke,
                current_phase: ExplosionPhase::Smoke,
                frame_count: 64,
                current_frame: 0,
                frame_duration: (duration * 2.5) / 64.0,
                frame_timer: 0.0,
                scale: radius * 1.2,
                fade_alpha: 0.6,
                phase_transition_timer: 0.0,
            },
            Name::new("AnimatedSmokeSprite"),
        ));
    }
}

// DEBUG: Colored quad explosion function (keeping for debugging)
pub fn spawn_debug_explosion_effect(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    radius: f32,
    intensity: f32,
    duration: f32,
) {
    info!("🔧 Spawning DEBUG colored explosion at {} with radius {} intensity {}", position, radius, intensity);
    
    // Create a quad mesh for debug billboard
    let quad_mesh = meshes.add(Rectangle::new(radius * 0.3, radius * 0.3));
    
    // Choose explosion type based on intensity
    let explosion_type = if intensity > 2.5 {
        ExplosionType::Nuclear
    } else if intensity > 1.5 {
        ExplosionType::Fire
    } else {
        ExplosionType::Impact
    };
    
    // Create debug colored material (NO texture)
    let debug_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgb(2.0, 1.5, 0.8).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: false,
        cull_mode: None,
        ..default()
    });
    
    // Spawn debug explosion with old component
    commands.spawn((
        PbrBundle {
            mesh: quad_mesh.clone(),
            material: debug_material,
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(0.01)),
            ..default()
        },
        ExplosionTimer {
            timer: Timer::new(Duration::from_secs_f32(duration), TimerMode::Once),
        },
        SpriteExplosion {
            explosion_type: explosion_type.clone(),
            current_phase: ExplosionPhase::Initial,
            frame_count: 64,
            current_frame: 0,
            frame_duration: duration / 64.0,
            frame_timer: 0.0,
            scale: radius,
            fade_alpha: 1.0,
            phase_transition_timer: 0.0,
        },
        Name::new("DebugColoredExplosion"),
    ));
}

// Backward compatibility function - now calls debug version for testing
pub fn spawn_explosion_effect(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    _images: &mut ResMut<Assets<Image>>,
    position: Vec3,
    radius: f32,
    intensity: f32,
    duration: f32,
) {
    info!("🔥 Spawning explosion effect at {} with radius {} intensity {}", position, radius, intensity);
    
    // For now, just call the debug version directly with proper component
    let quad_mesh = meshes.add(Rectangle::new(radius * 0.3, radius * 0.3));
    
    let explosion_type = if intensity > 2.5 {
        ExplosionType::Nuclear
    } else if intensity > 1.5 {
        ExplosionType::Fire
    } else {
        ExplosionType::Impact
    };
    
    let debug_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgb(2.0, 1.5, 0.8).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: false,
        cull_mode: None,
        ..default()
    });
    
    commands.spawn((
        PbrBundle {
            mesh: quad_mesh,
            material: debug_material,
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(0.01)),
            ..default()
        },
        ExplosionTimer {
            timer: Timer::new(Duration::from_secs_f32(duration), TimerMode::Once),
        },
        SpriteExplosion {
            explosion_type,
            current_phase: ExplosionPhase::Initial,
            frame_count: 64,
            current_frame: 0,
            frame_duration: duration / 64.0,
            frame_timer: 0.0,
            scale: radius,
            fade_alpha: 1.0,
            phase_transition_timer: 0.0,
        },
        Name::new("CompatibilityExplosion"),
    ));
}

pub fn spawn_explosion(
    commands: &mut Commands,
    explosion_assets: &ExplosionAssets,
    position: Vec3,
    radius: f32,
    intensity: f32,
) {
    info!("🔥 Real sprite explosion with TextureAtlas at {} with radius {} intensity {}", position, radius, intensity);
    // TODO: Implement TextureAtlas-based spawning here
    // This would create entities with TextureAtlas components for proper frame animation
}

// ===== DEBUG TEST EXPLOSIONS =====

fn debug_test_explosions(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // T key = debug colored explosions (for testing)
    if keyboard.just_pressed(KeyCode::KeyT) {
        info!("🧪 DEBUG: T key pressed - spawning DEBUG colored test explosions!");
        
        let test_positions = vec![
            (Vec3::new(0.0, 80.0, 120.0), 6.0, 1.0),     // Standard explosion
            (Vec3::new(-30.0, 70.0, 100.0), 8.0, 2.0),   // Large fire explosion
            (Vec3::new(30.0, 70.0, 100.0), 10.0, 3.0),   // Nuclear explosion
            (Vec3::new(0.0, 60.0, 90.0), 12.0, 2.5),     // Massive explosion
        ];
        
        for (i, (pos, radius, intensity)) in test_positions.iter().enumerate() {
            spawn_debug_explosion_effect(
                &mut commands,
                &mut meshes,
                &mut materials,
                *pos,
                *radius,
                *intensity,
                2.5 + i as f32 * 0.3,
            );
            info!("🧪 Debug colored explosion {} at {}", i, pos);
        }
    }
    
    // Y key = animated sprite explosions (real ones)
    if keyboard.just_pressed(KeyCode::KeyY) {
        if let Some(assets) = explosion_assets.as_ref() {
            info!("🎬 DEBUG: Y key pressed - spawning ANIMATED sprite test explosions!");
            
            let test_positions = vec![
                (Vec3::new(0.0, 80.0, 120.0), 6.0, 1.0),     // Standard explosion
                (Vec3::new(-30.0, 70.0, 100.0), 8.0, 2.0),   // Large fire explosion
                (Vec3::new(30.0, 70.0, 100.0), 10.0, 3.0),   // Nuclear explosion
                (Vec3::new(0.0, 60.0, 90.0), 12.0, 2.5),     // Massive explosion
            ];
            
            for (i, (pos, radius, intensity)) in test_positions.iter().enumerate() {
                spawn_animated_sprite_explosion(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &assets,
                    *pos,
                    *radius,
                    *intensity,
                    2.5 + i as f32 * 0.3,
                );
                info!("🎬 Debug animated sprite explosion {} at {}", i, pos);
            }
        } else {
            info!("⚠️ Explosion assets not loaded yet - cannot spawn animated explosions");
        }
    }
}

// ===== ANIMATION SYSTEMS =====

// Original colored quad animation system (for debugging)
fn animate_sprite_explosions(
    mut query: Query<(&mut Transform, &mut Handle<StandardMaterial>, &mut SpriteExplosion, &ExplosionTimer)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    camera_query: Query<&Transform, (With<Camera>, Without<SpriteExplosion>)>,
    time: Res<Time>,
) {
    // Get camera position for billboard effect
    let camera_position = if let Ok(camera_transform) = camera_query.get_single() {
        camera_transform.translation
    } else {
        Vec3::ZERO // Fallback if no camera found
    };
    
    for (mut transform, mut material_handle, mut sprite_explosion, timer) in query.iter_mut() {
        let progress = timer.timer.elapsed_secs() / timer.timer.duration().as_secs_f32();
        let progress = progress.clamp(0.0, 1.0);
        
        // Update frame animation for real sprite sheets
        sprite_explosion.frame_timer += time.delta_seconds();
        if sprite_explosion.frame_timer >= sprite_explosion.frame_duration {
            sprite_explosion.frame_timer = 0.0;
            sprite_explosion.current_frame += 1;
            
            // Keep frame within bounds
            if sprite_explosion.current_frame >= sprite_explosion.frame_count {
                sprite_explosion.current_frame = sprite_explosion.frame_count - 1; // Hold on last frame
            }
        }
        
        // Handle phase transitions for multi-phase explosions (debug colored quads)
        sprite_explosion.phase_transition_timer += time.delta_seconds();
        
        // Transition between phases based on time  
        if sprite_explosion.explosion_type != ExplosionType::Smoke {
            if progress < 0.3 && sprite_explosion.current_phase != ExplosionPhase::Initial {
                sprite_explosion.current_phase = ExplosionPhase::Initial;
            } else if progress >= 0.3 && progress < 0.7 && sprite_explosion.current_phase != ExplosionPhase::Secondary {
                sprite_explosion.current_phase = ExplosionPhase::Secondary;
            } else if progress >= 0.7 && sprite_explosion.current_phase != ExplosionPhase::Smoke {
                sprite_explosion.current_phase = ExplosionPhase::Smoke;
            }
        }
        
        // Animate scale based on explosion type and phase
        let scale_progress = match (&sprite_explosion.explosion_type, &sprite_explosion.current_phase) {
            (ExplosionType::Fire | ExplosionType::Nuclear, ExplosionPhase::Initial) => {
                // Rapid expansion in initial phase
                if progress < 0.3 {
                    (progress / 0.3).powf(0.5) // Fast expansion with ease-out
                } else {
                    1.0
                }
            },
            (ExplosionType::Fire | ExplosionType::Nuclear, ExplosionPhase::Secondary) => {
                // Continued expansion in secondary phase
                1.0 + (progress - 0.3) * 0.5
            },
            (_, ExplosionPhase::Smoke) => {
                // Gradual expansion for smoke
                1.0 + (progress - 0.7) * 2.0
            },
            _ => progress,
        };
        
        let current_scale = 0.01 + (sprite_explosion.scale * 0.15) * scale_progress;
        transform.scale = Vec3::splat(current_scale.min(sprite_explosion.scale * 0.25));
        
        // TRUE BILLBOARD EFFECT - Always face camera
        let explosion_position = transform.translation;
        let to_camera = (camera_position - explosion_position).normalize();
        
        // Simple billboard rotation - face camera directly
        if to_camera.length() > 0.001 {
            // Calculate rotation to face camera using simple approach
            let forward = to_camera;
            let right = Vec3::Y.cross(forward).normalize();
            let up = forward.cross(right).normalize();
            
            // Create rotation from basis vectors (right-handed coordinate system)
            transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
        }
        
        // Animate alpha and emissive based on phase and progress
        if let Some(material) = materials.get_mut(&*material_handle) {
            let (alpha_fade, emissive_strength) = match (&sprite_explosion.current_phase, progress) {
                (ExplosionPhase::Initial, p) => {
                    // Bright initial phase
                    (sprite_explosion.fade_alpha, 3.0 * (1.0 - p * 0.3))
                },
                (ExplosionPhase::Secondary, p) => {
                    // Dimmer secondary phase
                    (sprite_explosion.fade_alpha * 0.9, 2.0 * (1.0 - p * 0.5))
                },
                (ExplosionPhase::Smoke, p) => {
                    // Gradual fade for smoke
                    (sprite_explosion.fade_alpha * (1.0 - p * 0.7), 0.5 * (1.0 - p))
                },
            };
            
            // Update material alpha
            let mut color = material.base_color;
            color.set_alpha(alpha_fade.max(0.0));
            material.base_color = color;
            
            // Update emissive intensity (keep original color ratios)
            let current_emissive = material.emissive;
            let boosted_emissive = LinearRgba::new(
                current_emissive.red * emissive_strength,
                current_emissive.green * emissive_strength,
                current_emissive.blue * emissive_strength,
                current_emissive.alpha
            );
            material.emissive = boosted_emissive;
        }
    }
}

fn update_explosion_timers(
    mut query: Query<&mut ExplosionTimer>,
    time: Res<Time>,
) {
    for mut explosion_timer in query.iter_mut() {
        explosion_timer.timer.tick(time.delta());
    }
}

fn cleanup_finished_explosions(
    mut commands: Commands,
    query: Query<(Entity, &ExplosionTimer)>,
) {
    for (entity, timer) in query.iter() {
        if timer.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}

// UV-based sprite animation system for real animated explosions
fn animate_uv_sprite_explosions(
    mut query: Query<(&mut Transform, &mut Handle<StandardMaterial>, &mut AnimatedSpriteExplosion, &ExplosionTimer)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    camera_query: Query<&Transform, (With<Camera>, Without<AnimatedSpriteExplosion>)>,
    time: Res<Time>,
) {
    // Get camera position for billboard effect
    let camera_position = if let Ok(camera_transform) = camera_query.get_single() {
        camera_transform.translation
    } else {
        Vec3::ZERO
    };
    
    for (mut transform, mut material_handle, mut sprite_explosion, timer) in query.iter_mut() {
        let progress = timer.timer.elapsed_secs() / timer.timer.duration().as_secs_f32();
        let progress = progress.clamp(0.0, 1.0);
        
        // Update frame animation
        sprite_explosion.frame_timer += time.delta_seconds();
        if sprite_explosion.frame_timer >= sprite_explosion.frame_duration {
            sprite_explosion.frame_timer = 0.0;
            sprite_explosion.current_frame += 1;
            
            // Keep frame within bounds
            if sprite_explosion.current_frame >= sprite_explosion.frame_count {
                sprite_explosion.current_frame = sprite_explosion.frame_count - 1; // Hold on last frame
            }
        }
        
        // Handle phase transitions and material switching
        if let Some(assets) = explosion_assets.as_ref() {
            let old_phase = sprite_explosion.current_phase.clone();
            
            // Transition between phases based on time
            if sprite_explosion.explosion_type != ExplosionType::Smoke {
                if progress < 0.3 && sprite_explosion.current_phase != ExplosionPhase::Initial {
                    sprite_explosion.current_phase = ExplosionPhase::Initial;
                } else if progress >= 0.3 && progress < 0.7 && sprite_explosion.current_phase != ExplosionPhase::Secondary {
                    sprite_explosion.current_phase = ExplosionPhase::Secondary;
                } else if progress >= 0.7 && sprite_explosion.current_phase != ExplosionPhase::Smoke {
                    sprite_explosion.current_phase = ExplosionPhase::Smoke;
                }
                
                // Switch texture when phase changes
                if old_phase != sprite_explosion.current_phase {
                    let new_texture = match sprite_explosion.current_phase {
                        ExplosionPhase::Initial => assets.normal_plus_texture.clone(),
                        ExplosionPhase::Secondary => assets.normal_minus_texture.clone(),
                        ExplosionPhase::Smoke => assets.smoke_texture.clone(),
                    };
                    
                    // Update material with new texture
                    if let Some(material) = materials.get_mut(&*material_handle) {
                        material.base_color_texture = Some(new_texture);
                    }
                    
                    // Reset frame animation for new phase
                    sprite_explosion.current_frame = 0;
                    sprite_explosion.frame_timer = 0.0;
                }
            }
        }
        
        // Calculate UV coordinates for current frame (8x8 grid)
        let frame = sprite_explosion.current_frame;
        let grid_size = 8; // 8x8 grid
        let frame_x = frame % grid_size;
        let frame_y = frame / grid_size;
        
        // Calculate UV offsets for this frame
        let uv_scale = 1.0 / grid_size as f32;
        let uv_offset_x = frame_x as f32 * uv_scale;
        let uv_offset_y = frame_y as f32 * uv_scale;
        
        // TODO: Update mesh UV coordinates here
        // Currently showing full sprite sheet texture - need to implement UV coordinate animation
        // This would require either:
        // 1. Dynamically creating new meshes with updated UV coordinates per frame
        // 2. Using a custom shader that takes UV offset uniforms
        // 3. Using Bevy's Sprite2D system instead of 3D materials
        // For now, this switches textures between phases but shows full sprite sheet
        
        // Animate scale
        let scale_progress = match (&sprite_explosion.explosion_type, &sprite_explosion.current_phase) {
            (ExplosionType::Fire | ExplosionType::Nuclear, ExplosionPhase::Initial) => {
                if progress < 0.3 {
                    (progress / 0.3).powf(0.5)
                } else {
                    1.0
                }
            },
            (ExplosionType::Fire | ExplosionType::Nuclear, ExplosionPhase::Secondary) => {
                1.0 + (progress - 0.3) * 0.5
            },
            (_, ExplosionPhase::Smoke) => {
                1.0 + (progress - 0.7) * 2.0
            },
            _ => progress,
        };
        
        let current_scale = 0.01 + (sprite_explosion.scale * 0.15) * scale_progress;
        transform.scale = Vec3::splat(current_scale.min(sprite_explosion.scale * 0.25));
        
        // Billboard effect - always face camera
        let explosion_position = transform.translation;
        let to_camera = (camera_position - explosion_position).normalize();
        
        if to_camera.length() > 0.001 {
            let forward = to_camera;
            let right = Vec3::Y.cross(forward).normalize();
            let up = forward.cross(right).normalize();
            transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
        }
        
        // Animate alpha and emissive
        if let Some(material) = materials.get_mut(&*material_handle) {
            let (alpha_fade, emissive_strength) = match (&sprite_explosion.current_phase, progress) {
                (ExplosionPhase::Initial, p) => {
                    (sprite_explosion.fade_alpha, 3.0 * (1.0 - p * 0.3))
                },
                (ExplosionPhase::Secondary, p) => {
                    (sprite_explosion.fade_alpha * 0.9, 2.0 * (1.0 - p * 0.5))
                },
                (ExplosionPhase::Smoke, p) => {
                    (sprite_explosion.fade_alpha * (1.0 - p * 0.7), 0.5 * (1.0 - p))
                },
            };
            
            let mut color = material.base_color;
            color.set_alpha(alpha_fade.max(0.0));
            material.base_color = color;
            
            let current_emissive = material.emissive;
            let boosted_emissive = LinearRgba::new(
                current_emissive.red * emissive_strength,
                current_emissive.green * emissive_strength,
                current_emissive.blue * emissive_strength,
                current_emissive.alpha
            );
            material.emissive = boosted_emissive;
        }
    }
} 