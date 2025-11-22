// Enhanced sprite-based explosion effects with real sprite sheets and custom shader
use bevy::prelude::*;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
};
use std::time::Duration;
use bevy::render::alpha::AlphaMode;
use crate::types::RtsCamera;

// ===== CUSTOM EXPLOSION MATERIAL =====

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ExplosionMaterial {
    #[uniform(0)]
    pub frame_data: Vec4, // x: frame_x, y: frame_y, z: grid_size, w: alpha
    #[uniform(1)]
    pub color_data: Vec4, // RGB: base color, A: emissive strength
    #[texture(2, dimension = "2d")]
    #[sampler(3)]
    pub sprite_texture: Handle<Image>,
}

impl Material for ExplosionMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/explosion.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ===== PLUGIN =====

pub struct ExplosionShaderPlugin;

impl Plugin for ExplosionShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ExplosionMaterial>::default())
            .add_systems(Startup, setup_explosion_assets)
            .add_systems(Update, (
                update_explosion_timers,
                animate_sprite_explosions,
                animate_custom_shader_explosions,
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

#[derive(Component)]
pub struct CustomShaderExplosion {
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
    // New 5x5 flipbook explosion texture
    pub explosion_flipbook_texture: Handle<Image>,
    
    // TextureAtlas layout for 5x5 grid (25 frames)
    pub explosion_atlas: Handle<TextureAtlasLayout>,
    
    // Materials for different phases (using same texture but different settings)
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
    // Load the new 5x5 flipbook texture
    let explosion_flipbook_texture = asset_server.load("textures/Explosion02HD_5x5.tga");
    info!("🎨 Loading explosion texture: textures/Explosion02HD_5x5.tga");
    
    // Create TextureAtlas layout for 5x5 sprite grid (25 frames total)
    let atlas_layout = TextureAtlasLayout::from_grid(UVec2::splat(5), 5, 5, None, None);
    let explosion_atlas = texture_atlas_layouts.add(atlas_layout);
    
    // Create materials that use the flipbook texture
    let explosion_bright_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgb(2.0, 1.5, 0.8).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true, // Disable lighting for shadows
        cull_mode: None, // Disable backface culling for billboards
        ..default()
    });
    
    let explosion_dim_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.85),
        emissive: Color::srgb(1.0, 0.8, 0.4).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    
    let smoke_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.7),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    commands.insert_resource(ExplosionAssets {
        explosion_flipbook_texture,
        explosion_atlas,
        explosion_bright_material,
        explosion_dim_material,
        smoke_material,
    });
    
    info!("🎨 New 5x5 flipbook explosion assets loaded!");
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
    let quad_mesh = meshes.add(Rectangle::new(radius * 2.0, radius * 2.0));
    
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
        base_color_texture: Some(explosion_assets.explosion_flipbook_texture.clone()),
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgb(2.0, 1.5, 0.8).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true, // Disable lighting to remove shadows
        cull_mode: None,
        ..default()
    });
    
    // Spawn animated sprite explosion
    commands.spawn((
        PbrBundle {
            mesh: quad_mesh.clone(),
            material: sprite_material,
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(1.0)),
            ..default()
        },
        ExplosionTimer {
            timer: Timer::new(Duration::from_secs_f32(duration * 0.8), TimerMode::Once), // Faster animation
        },
        SpriteExplosion {
            explosion_type: explosion_type.clone(),
            current_phase: ExplosionPhase::Initial,
            frame_count: 25, // 5x5 grid
            current_frame: 0,
            frame_duration: (duration * 0.8) / 25.0, // Faster frame duration
            frame_timer: 0.0,
            scale: radius,
            fade_alpha: 1.0,
            phase_transition_timer: 0.0,
        },
        NotShadowCaster, // Prevent this entity from casting shadows
        NotShadowReceiver, // Prevent this entity from receiving shadows
        Name::new("AnimatedSpriteExplosion"),
    ));
    
    // Removed automatic smoke spawning to prevent overlapping explosions
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
        unlit: true, // Disable lighting to remove shadows
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
            frame_count: 25,
            current_frame: 0,
            frame_duration: duration / 25.0,
            frame_timer: 0.0,
            scale: radius,
            fade_alpha: 1.0,
            phase_transition_timer: 0.0,
        },
        NotShadowCaster, // Prevent this entity from casting shadows
        NotShadowReceiver, // Prevent this entity from receiving shadows
        Name::new("DebugColoredExplosion"),
    ));
}

// Backward compatibility function - needs ExplosionAssets to work properly
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
    // This is a stub that creates invisible explosions
    // The actual tower explosion system should use spawn_animated_sprite_explosion instead
    info!("⚠️ spawn_explosion_effect called without ExplosionAssets - creating placeholder");
    info!("   Position: {}, Radius: {}, Intensity: {}", position, radius, intensity);
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

// NEW: Custom shader animated sprite exdddddddplosion function
pub fn spawn_custom_shader_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    explosion_materials: &mut ResMut<Assets<ExplosionMaterial>>,
    explosion_assets: &ExplosionAssets,
    particle_effects: Option<&crate::particles::ExplosionParticleEffects>,
    position: Vec3,
    radius: f32,
    intensity: f32,
    duration: f32,
    is_tower: bool,
    current_time: f64,
) {
    trace!("🎭 Spawning CUSTOM SHADER explosion at {} with radius {} intensity {}", position, radius, intensity);

    // Create a quad mesh for sprite billboard
    let quad_mesh = meshes.add(Rectangle::new(radius * 2.0, radius * 2.0));

    // Choose explosion type based on intensity
    let explosion_type = if intensity > 2.5 {
        ExplosionType::Nuclear
    } else if intensity > 1.5 {
        ExplosionType::Fire
    } else {
        ExplosionType::Impact
    };

    // Create custom explosion material with frame 0
    let explosion_material = explosion_materials.add(ExplosionMaterial {
        frame_data: Vec4::new(0.0, 0.0, 5.0, 1.0), // frame_x=0, frame_y=0, grid_size=5, alpha=1
        color_data: Vec4::new(1.0, 1.0, 1.0, 2.0), // white tint, emissive=2.0
        sprite_texture: explosion_assets.explosion_flipbook_texture.clone(),
    });

    // Spawn custom shader explosion
    commands.spawn((
        MaterialMeshBundle {
            mesh: quad_mesh.clone(),
            material: explosion_material,
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(1.0)),
            ..default()
        },
        ExplosionTimer {
            timer: Timer::new(Duration::from_secs_f32(duration * 0.8), TimerMode::Once), // Faster animation
        },
        CustomShaderExplosion {
            explosion_type: explosion_type.clone(),
            current_phase: ExplosionPhase::Initial,
            frame_count: 25, // 5x5 grid
            current_frame: 0,
            frame_duration: (duration * 0.8) / 25.0, // Faster frame duration
            frame_timer: 0.0,
            scale: radius,
            fade_alpha: 1.0,
        },
        NotShadowCaster, // Prevent this entity from casting shadows
        NotShadowReceiver, // Prevent this entity from receiving shadows
        Name::new("CustomShaderExplosion"),
    ));

    // Spawn particle effects bundled with billboard explosion (with probability for performance)
    if let Some(particles) = particle_effects {
        // Towers always spawn particles, units spawn based on probability
        let should_spawn = is_tower || rand::random::<f32>() < crate::constants::PARTICLE_SPAWN_PROBABILITY;

        if should_spawn {
            trace!("🔥 BUNDLING: Spawning particles for explosion at {:?}, is_tower={}", position, is_tower);
            if is_tower {
                crate::particles::spawn_tower_explosion_particles(commands, particles, position, current_time);
            } else {
                crate::particles::spawn_unit_explosion_particles(commands, particles, position, current_time);
            }
        } else {
            trace!("🎲 SKIP PARTICLES: Probability check failed for unit explosion at {:?}", position);
        }
    } else {
        debug!("⚠️ NO PARTICLES: particle_effects is None at {:?}, is_tower={}", position, is_tower);
    }
}

// ===== DEBUG TEST EXPLOSIONS =====

fn debug_test_explosions(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut explosion_materials: ResMut<Assets<ExplosionMaterial>>,
    mut images: ResMut<Assets<Image>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // T key functionality removed - no longer needed since U key works perfectly
    
    // Y key = animated sprite explosions (single explosion for easy observation)
    if keyboard.just_pressed(KeyCode::KeyY) {
        if let Some(assets) = explosion_assets.as_ref() {
            info!("🎬 DEBUG: Y key pressed - spawning single ANIMATED sprite test explosion!");
            
            spawn_animated_sprite_explosion(
                &mut commands,
                &mut meshes,
                &mut materials,
                &assets,
                Vec3::new(0.0, 8.0, 0.0), // Single explosion at battlefield center, elevated
                8.0,
                2.0,
                3.0,
            );
            info!("🎬 Single animated sprite explosion spawned at center");
        } else {
            info!("⚠️ Explosion assets not loaded yet - cannot spawn animated explosions");
        }
    }
    
    // U key = custom shader explosions (single explosion for easy observation)
    if keyboard.just_pressed(KeyCode::KeyU) {
        if let Some(assets) = explosion_assets.as_ref() {
            info!("🎭 DEBUG: U key pressed - spawning single CUSTOM SHADER test explosion!");
            
            spawn_custom_shader_explosion(
                &mut commands,
                &mut meshes,
                &mut explosion_materials,
                &assets,
                None, // No particles for debug test
                Vec3::new(0.0, 8.0, 0.0), // Single explosion at battlefield center, elevated
                8.0,
                2.0,
                3.0,
                false, // Not a tower
                time.elapsed_seconds_f64(),
            );
            info!("🎭 Single custom shader explosion spawned at center");
        } else {
            info!("⚠️ Explosion assets not loaded yet - cannot spawn custom shader explosions");
        }
    }

    // I key = simple solid color test explosions (single explosion for easy observation)
    if keyboard.just_pressed(KeyCode::KeyI) {
        info!("🟡 DEBUG: I key pressed - spawning single SIMPLE SOLID COLOR test explosion!");
        
        // Create a simple colored quad with no texture
        let quad_mesh = meshes.add(Rectangle::new(16.0, 16.0));
        
        let test_material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.5, 0.0, 0.8), // Bright orange, semi-transparent
            emissive: Color::srgb(2.0, 1.0, 0.0).into(),  // Bright orange emissive
            alpha_mode: AlphaMode::Blend,
            unlit: true, // No lighting calculations for simple test
            cull_mode: None,
            ..default()
        });
        
        commands.spawn((
            PbrBundle {
                mesh: quad_mesh,
                material: test_material,
                transform: Transform::from_translation(Vec3::new(0.0, 8.0, 0.0))
                    .with_scale(Vec3::splat(1.0)),
                ..default()
            },
            ExplosionTimer {
                timer: Timer::new(Duration::from_secs_f32(5.0), TimerMode::Once), // Long duration for testing
            },
            NotShadowCaster, // Prevent this entity from casting shadows
            NotShadowReceiver, // Prevent this entity from receiving shadows
            Name::new("SimpleTestExplosion"),
        ));
        
        info!("🟡 Single simple test explosion spawned at center");
    }
}

// ===== ANIMATION SYSTEMS =====

// Original colored quad animation system (for debugging)
fn animate_sprite_explosions(
    mut query: Query<(&mut Transform, &mut Handle<StandardMaterial>, &mut SpriteExplosion, &ExplosionTimer)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<SpriteExplosion>)>,
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
        
        // DISABLED: Phase transitions to prevent "three explosions" effect
        // The self-contained sprite sheet already has all phases baked in
        // sprite_explosion.phase_transition_timer += time.delta_seconds();
        
        // Keep constant scale since explosion animation is baked into the texture
        transform.scale = Vec3::splat(1.0);
        
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
        
        // Simplified: Keep full intensity throughout since the sprite sheet contains all phases
        if let Some(material) = materials.get_mut(&*material_handle) {
            // Maintain full intensity and only fade near the end
            let alpha_fade = if progress > 0.9 {
                sprite_explosion.fade_alpha * (1.0 - (progress - 0.9) * 10.0) // Fade only in last 10%
            } else {
                sprite_explosion.fade_alpha // Full alpha for 90% of animation
            };
            
            let emissive_strength = if progress > 0.9 {
                2.0 * (1.0 - (progress - 0.9) * 5.0) // Reduce emissive only near end
            } else {
                2.0 // Full emissive strength for most of animation
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

// Custom shader sprite animation system for real animated explosions
fn animate_custom_shader_explosions(
    mut query: Query<(&mut Transform, &mut Handle<ExplosionMaterial>, &mut CustomShaderExplosion, &ExplosionTimer)>,
    mut explosion_materials: ResMut<Assets<ExplosionMaterial>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<CustomShaderExplosion>)>,
    time: Res<Time>,
) {
    // Get camera position for billboard effect
    let camera_position = if let Ok(camera_transform) = camera_query.get_single() {
        camera_transform.translation
    } else {
        Vec3::ZERO
    };
    
    for (mut transform, material_handle, mut sprite_explosion, timer) in query.iter_mut() {
        let progress = timer.timer.elapsed_secs() / timer.timer.duration().as_secs_f32();
        let progress = progress.clamp(0.0, 1.0);
        
        // Update frame animation
        sprite_explosion.frame_timer += time.delta_seconds();
        if sprite_explosion.frame_timer >= sprite_explosion.frame_duration {
            sprite_explosion.frame_timer = 0.0;
            let old_frame = sprite_explosion.current_frame;
            sprite_explosion.current_frame += 1;
            
            // Keep frame within bounds
            if sprite_explosion.current_frame >= sprite_explosion.frame_count {
                sprite_explosion.current_frame = sprite_explosion.frame_count - 1; // Hold on last frame
            }
            
            // Debug log first few frames
            if old_frame < 5 {
                trace!("🎞️ Explosion frame update: {} → {}", old_frame, sprite_explosion.current_frame);
            }
        }
        
        // DISABLED: Phase transitions to prevent "three explosions" effect
        // The self-contained sprite sheet already has all phases baked in
        
        // Update material uniforms with current frame and animation data
        if let Some(material) = explosion_materials.get_mut(&*material_handle) {
            // Calculate frame coordinates in 5x5 grid
            let frame = sprite_explosion.current_frame;
            let grid_size = 5;
            let frame_x = frame % grid_size;
            let frame_y = frame / grid_size;
            
            // Simplified: Keep full intensity throughout since the sprite sheet contains all phases
            let alpha_fade = if progress > 0.9 {
                sprite_explosion.fade_alpha * (1.0 - (progress - 0.9) * 10.0) // Fade only in last 10%
            } else {
                sprite_explosion.fade_alpha // Full alpha for 90% of animation
            };
            
            let emissive_strength = if progress > 0.9 {
                2.0 * (1.0 - (progress - 0.9) * 5.0) // Reduce emissive only near end
            } else {
                2.0 // Full emissive strength for most of animation
            };
            
            // Update frame data uniform
            material.frame_data = Vec4::new(
                frame_x as f32,
                frame_y as f32,
                grid_size as f32,
                alpha_fade.max(0.0)
            );
            
            // Update color data uniform
            material.color_data = Vec4::new(1.0, 1.0, 1.0, emissive_strength);
        }
        
        // Keep constant scale since explosion animation is baked into the texture
        transform.scale = Vec3::splat(1.0);
        
        // Billboard effect - always face camera
        let explosion_position = transform.translation;
        let to_camera = (camera_position - explosion_position).normalize();
        
        if to_camera.length() > 0.001 {
            let forward = to_camera;
            let right = Vec3::Y.cross(forward).normalize();
            let up = forward.cross(right).normalize();
            transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
        }
    }
} 