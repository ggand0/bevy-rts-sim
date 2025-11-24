// War FX explosion spawner with UV-scrolling billboards
use bevy::prelude::*;
use crate::wfx_materials::{SmokeScrollMaterial, AdditiveMaterial};
use rand::Rng;

// Temporary test function using StandardMaterial
pub fn spawn_warfx_tower_explosion_test(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    let smoke_texture = asset_server.load("textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    info!("🎨 TEST: Loading smoke texture: textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    let test_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(smoke_texture),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    let quad_size = 20.0 * scale;
    let quad_mesh = meshes.add(Rectangle::new(quad_size, quad_size));

    info!("📐 TEST: Creating standard material quad: {}x{} at position {:?}", quad_size, quad_size, position);

    commands.spawn((
        PbrBundle {
            mesh: quad_mesh,
            material: test_material,
            transform: Transform::from_translation(position),
            visibility: Visibility::Visible,
            ..Default::default()
        },
        bevy::pbr::NotShadowCaster,
        bevy::pbr::NotShadowReceiver,
        WarFXExplosion {
            lifetime: 0.0,
            max_lifetime: 30.0, // Longer duration for testing
        },
        Name::new("WarFX_Test_Explosion"),
    ));

    info!("✅ TEST: Spawned standard material test explosion at {:?}", position);
}

/// Spawns a War FX explosion with scrolling smoke billboards
/// This is a simpler approach using custom materials instead of particle systems
pub fn spawn_warfx_tower_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    smoke_materials: &mut ResMut<Assets<SmokeScrollMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    // Load smoke texture
    let smoke_texture = asset_server.load("textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    info!("🎨 Loading smoke texture: textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    // Create scrolling smoke material
    // RGB = white tint (1,1,1), A = scroll speed (2.0)
    let smoke_material = smoke_materials.add(SmokeScrollMaterial {
        tint_color_and_speed: Vec4::new(1.0, 1.0, 1.0, 2.0),
        smoke_texture: smoke_texture.clone(),
    });

    // Create billboard quad mesh
    let quad_size = 20.0 * scale;
    let quad_mesh = meshes.add(Rectangle::new(quad_size, quad_size));

    info!("📐 Creating quad: {}x{} at position {:?}", quad_size, quad_size, position);

    // Spawn billboard quad at explosion position
    commands.spawn((
        MaterialMeshBundle {
            mesh: quad_mesh,
            material: smoke_material,
            transform: Transform::from_translation(position),
            visibility: Visibility::Visible,
            ..Default::default()
        },
        bevy::pbr::NotShadowCaster,
        bevy::pbr::NotShadowReceiver,
        WarFXExplosion {
            lifetime: 0.0,
            max_lifetime: 30.0, // Longer duration for testing
        },
        Name::new("WarFX_Explosion"),
    ));

    info!("✅ WAR FX: Spawned scrolling smoke explosion at {:?}", position);
}

/// Spawns War FX flame burst using billboard quads with AdditiveMaterial
/// Matches the original Unity War FX implementation: manually spawned billboards
pub fn spawn_warfx_flame_burst(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    let flame_texture = asset_server.load("textures/wfx/WFX_T_DoubleFlames A8.tga");

    info!("🔥 WAR FX: Spawning flame burst billboards at {:?}", position);

    let mut rng = rand::thread_rng();

    // Spawn 3-5 flame billboards with random variations
    let flame_count = rng.gen_range(3..=5);

    for i in 0..flame_count {
        // Random position offset (slight spread)
        let offset = Vec3::new(
            rng.gen_range(-1.0..1.0) * scale,
            rng.gen_range(-0.5..0.5) * scale,
            rng.gen_range(-1.0..1.0) * scale,
        );

        // Orange tint color (bright to dark orange)
        let tint_r = rng.gen_range(0.9..1.0);
        let tint_g = rng.gen_range(0.5..0.7);
        let tint_b = rng.gen_range(0.1..0.3);

        let flame_material = additive_materials.add(AdditiveMaterial {
            tint_color: Vec4::new(tint_r, tint_g, tint_b, 1.0),
            particle_texture: flame_texture.clone(),
        });

        // Varied sizes
        let quad_size = rng.gen_range(3.0..6.0) * scale;
        let quad_mesh = meshes.add(Rectangle::new(quad_size, quad_size));

        // Varied lifetimes (0.2-0.5 seconds)
        let lifetime = rng.gen_range(0.2..0.5);

        // Random outward velocity (burst pattern)
        let velocity = Vec3::new(
            rng.gen_range(-3.0..3.0),
            rng.gen_range(1.0..3.0),
            rng.gen_range(-3.0..3.0),
        ) * scale;

        // Random rotation speed
        let rotation_speed = rng.gen_range(-1.0..1.0); // radians per second

        commands.spawn((
            MaterialMeshBundle {
                mesh: quad_mesh,
                material: flame_material,
                transform: Transform::from_translation(position + offset),
                visibility: Visibility::Visible,
                ..Default::default()
            },
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            WarFXExplosion {
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            AnimatedBillboard {
                initial_scale: 1.0,
                target_scale: 2.0,
                initial_alpha: 1.0,
                target_alpha: 0.0,
                velocity,
                rotation_speed,
                base_rotation: 0.0,
            },
            Name::new(format!("WFX_Flame_{}", i)),
        ));
    }

    info!("✅ WAR FX: Spawned {} flame billboards", flame_count);
}

/// Spawns War FX center glow using large billboard quads with AdditiveMaterial
/// Creates a bright, persistent glow at the explosion center
pub fn spawn_warfx_center_glow(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    let glow_texture = asset_server.load("textures/wfx/WFX_T_GlowCircle A8.png");

    info!("💡 WAR FX: Spawning center glow billboards at {:?}", position);

    let mut rng = rand::thread_rng();

    // Spawn 2-3 large glow billboards
    let glow_count = rng.gen_range(2..=3);

    for i in 0..glow_count {
        // Small position offset (tighter than flames)
        let offset = Vec3::new(
            rng.gen_range(-0.5..0.5) * scale,
            rng.gen_range(-0.2..0.2) * scale,
            rng.gen_range(-0.5..0.5) * scale,
        );

        // White-orange tint
        let tint_r = 1.0;
        let tint_g = rng.gen_range(0.8..1.0);
        let tint_b = rng.gen_range(0.5..0.7);

        let glow_material = additive_materials.add(AdditiveMaterial {
            tint_color: Vec4::new(tint_r, tint_g, tint_b, 0.7), // Slightly transparent
            particle_texture: glow_texture.clone(),
        });

        // Larger sizes than flames
        let quad_size = rng.gen_range(5.0..8.0) * scale;
        let quad_mesh = meshes.add(Rectangle::new(quad_size, quad_size));

        // Longer lifetimes (0.5-1.0 seconds)
        let lifetime = rng.gen_range(0.5..1.0);

        // Minimal velocity (glow stays at center)
        let velocity = Vec3::new(
            rng.gen_range(-0.5..0.5),
            rng.gen_range(0.0..0.5),
            rng.gen_range(-0.5..0.5),
        ) * scale;

        // Slower rotation than flames
        let rotation_speed = rng.gen_range(-0.5..0.5); // radians per second

        commands.spawn((
            MaterialMeshBundle {
                mesh: quad_mesh,
                material: glow_material,
                transform: Transform::from_translation(position + offset),
                visibility: Visibility::Visible,
                ..Default::default()
            },
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            WarFXExplosion {
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            AnimatedBillboard {
                initial_scale: 4.0,
                target_scale: 0.5,
                initial_alpha: 0.7,
                target_alpha: 0.0,
                velocity,
                rotation_speed,
                base_rotation: 0.0,
            },
            Name::new(format!("WFX_Glow_{}", i)),
        ));
    }

    info!("✅ WAR FX: Spawned {} glow billboards", glow_count);
}

/// Spawns War FX smoke column using scrolling smoke billboards with SmokeScrollMaterial
/// Creates rising smoke with UV scrolling animation
pub fn spawn_warfx_smoke_column(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    smoke_materials: &mut ResMut<Assets<SmokeScrollMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    let smoke_texture = asset_server.load("textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    info!("💨 WAR FX: Spawning smoke column billboards at {:?}", position);

    let mut rng = rand::thread_rng();

    // Spawn 3-5 smoke billboards
    let smoke_count = rng.gen_range(3..=5);

    for i in 0..smoke_count {
        // Random position offset
        let offset = Vec3::new(
            rng.gen_range(-1.5..1.5) * scale,
            rng.gen_range(0.0..1.0) * scale,
            rng.gen_range(-1.5..1.5) * scale,
        );

        // Create scrolling smoke material
        // RGB = white/gray tint, A = scroll speed (2.0)
        let tint_brightness = rng.gen_range(0.7..1.0);
        let smoke_material = smoke_materials.add(SmokeScrollMaterial {
            tint_color_and_speed: Vec4::new(
                tint_brightness,
                tint_brightness,
                tint_brightness,
                2.0, // scroll speed
            ),
            smoke_texture: smoke_texture.clone(),
        });

        // Varied sizes
        let quad_size = rng.gen_range(8.0..12.0) * scale;
        let quad_mesh = meshes.add(Rectangle::new(quad_size, quad_size));

        // Longer lifetimes for smoke (3-5 seconds)
        let lifetime = rng.gen_range(3.0..5.0);

        // Upward velocity (smoke rises)
        let velocity = Vec3::new(
            rng.gen_range(-0.3..0.3),
            rng.gen_range(1.5..2.5), // Mostly upward
            rng.gen_range(-0.3..0.3),
        ) * scale;

        // Rotation animation
        let rotation_speed = rng.gen_range(-0.3..0.3); // Slow spin

        commands.spawn((
            MaterialMeshBundle {
                mesh: quad_mesh,
                material: smoke_material,
                transform: Transform::from_translation(position + offset),
                visibility: Visibility::Visible,
                ..Default::default()
            },
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            WarFXExplosion {
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            AnimatedSmokeBillboard {
                initial_scale: 1.0,
                target_scale: 2.5,
                initial_alpha: 0.8,
                target_alpha: 0.0,
                velocity,
                rotation_speed,
                base_rotation: 0.0,
            },
            Name::new(format!("WFX_Smoke_{}", i)),
        ));
    }

    info!("✅ WAR FX: Spawned {} smoke billboards", smoke_count);
}

/// Component to track War FX explosion lifetime
#[derive(Component)]
pub struct WarFXExplosion {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Component for billboard animation over lifetime (for AdditiveMaterial)
#[derive(Component)]
pub struct AnimatedBillboard {
    pub initial_scale: f32,
    pub target_scale: f32,
    pub initial_alpha: f32,
    pub target_alpha: f32,
    pub velocity: Vec3,
    pub rotation_speed: f32, // Radians per second
    pub base_rotation: f32,  // Current rotation accumulator
}

/// Component for smoke billboard animation over lifetime (for SmokeScrollMaterial)
#[derive(Component)]
pub struct AnimatedSmokeBillboard {
    pub initial_scale: f32,
    pub target_scale: f32,
    pub initial_alpha: f32,
    pub target_alpha: f32,
    pub velocity: Vec3,
    pub rotation_speed: f32, // Radians per second
    pub base_rotation: f32,  // Current rotation accumulator
}

/// System to update War FX explosions (billboard rotation + lifetime)
pub fn update_warfx_explosions(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut WarFXExplosion, &Name)>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    time: Res<Time>,
) {
    // Get camera position for billboarding
    let camera_position = if let Ok(camera_transform) = camera_query.get_single() {
        camera_transform.translation()
    } else {
        return; // No camera, can't billboard
    };

    for (entity, mut transform, mut explosion, name) in query.iter_mut() {
        // Update lifetime
        explosion.lifetime += time.delta_seconds();

        // Billboard effect - rotate to face camera
        // Keep the quad upright (parallel to Y axis) while facing the camera
        let billboard_pos = transform.translation;
        let to_camera = camera_position - billboard_pos;

        // Project direction onto XZ plane (keep billboard upright)
        let direction_xz = Vec3::new(to_camera.x, 0.0, to_camera.z).normalize();

        // Rotate around Y axis to face camera
        let angle = direction_xz.x.atan2(direction_xz.z);
        transform.rotation = Quat::from_rotation_y(angle);

        // Despawn when finished
        if explosion.lifetime >= explosion.max_lifetime {
            info!("🧹 Despawning War FX explosion '{}' after {:.1}s", name.as_str(), explosion.lifetime);
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// System to animate billboards (scale, alpha, velocity, rotation) over their lifetime
pub fn animate_warfx_billboards(
    mut query: Query<
        (
            &mut Transform,
            &mut AnimatedBillboard,
            &WarFXExplosion,
            &Handle<AdditiveMaterial>,
        ),
        With<AnimatedBillboard>,
    >,
    mut additive_materials: ResMut<Assets<AdditiveMaterial>>,
    time: Res<Time>,
) {
    for (mut transform, mut billboard, explosion, material_handle) in query.iter_mut() {
        // Calculate progress through lifetime (0.0 to 1.0)
        let progress = (explosion.lifetime / explosion.max_lifetime).clamp(0.0, 1.0);

        // Interpolate scale
        let current_scale =
            billboard.initial_scale + (billboard.target_scale - billboard.initial_scale) * progress;
        transform.scale = Vec3::splat(current_scale);

        // Interpolate alpha
        let current_alpha =
            billboard.initial_alpha + (billboard.target_alpha - billboard.initial_alpha) * progress;

        // Update material tint color with new alpha
        if let Some(material) = additive_materials.get_mut(material_handle) {
            material.tint_color.w = current_alpha;
        }

        // Apply velocity (smoke rises, flames burst outward)
        transform.translation += billboard.velocity * time.delta_seconds();

        // Apply rotation animation
        billboard.base_rotation += billboard.rotation_speed * time.delta_seconds();
    }
}

/// System to animate smoke billboards (scale, alpha, velocity, rotation) over their lifetime
pub fn animate_warfx_smoke_billboards(
    mut query: Query<
        (
            &mut Transform,
            &mut AnimatedSmokeBillboard,
            &WarFXExplosion,
            &Handle<SmokeScrollMaterial>,
        ),
        With<AnimatedSmokeBillboard>,
    >,
    mut smoke_materials: ResMut<Assets<SmokeScrollMaterial>>,
    time: Res<Time>,
) {
    for (mut transform, mut billboard, explosion, material_handle) in query.iter_mut() {
        // Calculate progress through lifetime (0.0 to 1.0)
        let progress = (explosion.lifetime / explosion.max_lifetime).clamp(0.0, 1.0);

        // Interpolate scale
        let current_scale =
            billboard.initial_scale + (billboard.target_scale - billboard.initial_scale) * progress;
        transform.scale = Vec3::splat(current_scale);

        // Interpolate alpha
        let current_alpha =
            billboard.initial_alpha + (billboard.target_alpha - billboard.initial_alpha) * progress;

        // Update material tint color with new alpha
        // SmokeScrollMaterial stores (R, G, B, scroll_speed), we need to modify RGB for alpha
        if let Some(material) = smoke_materials.get_mut(material_handle) {
            // Fade the tint color to black as alpha decreases
            let base_color = material.tint_color_and_speed.truncate().normalize_or_zero();
            material.tint_color_and_speed.x = base_color.x * current_alpha;
            material.tint_color_and_speed.y = base_color.y * current_alpha;
            material.tint_color_and_speed.z = base_color.z * current_alpha;
            // Keep scroll speed (w component) unchanged
        }

        // Apply velocity (smoke rises)
        transform.translation += billboard.velocity * time.delta_seconds();

        // Apply rotation animation
        billboard.base_rotation += billboard.rotation_speed * time.delta_seconds();
    }
}
