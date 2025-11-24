// War FX explosion spawner with UV-scrolling billboards
use bevy::prelude::*;
use crate::wfx_materials::SmokeScrollMaterial;

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
            max_lifetime: 5.0,
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
    let smoke_material = smoke_materials.add(SmokeScrollMaterial {
        tint_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        scroll_speed: 2.0,
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
            ..Default::default()
        },
        WarFXExplosion {
            lifetime: 0.0,
            max_lifetime: 5.0,
        },
        Name::new("WarFX_Explosion"),
    ));

    info!("✅ WAR FX: Spawned scrolling smoke explosion at {:?}", position);
}

/// Component to track War FX explosion lifetime
#[derive(Component)]
pub struct WarFXExplosion {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// System to update War FX explosions (billboard rotation + lifetime)
pub fn update_warfx_explosions(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut WarFXExplosion)>,
    camera_query: Query<&Transform, (With<Camera>, Without<WarFXExplosion>)>,
    time: Res<Time>,
) {
    // Get camera position for billboarding
    let camera_position = if let Ok(camera_transform) = camera_query.get_single() {
        camera_transform.translation
    } else {
        return; // No camera, can't billboard
    };

    for (entity, mut transform, mut explosion) in query.iter_mut() {
        // Update lifetime
        explosion.lifetime += time.delta_seconds();

        // Billboard effect - always face camera
        // Use look_at to orient the quad toward the camera
        transform.look_at(camera_position, Vec3::Y);

        // Despawn when finished
        if explosion.lifetime >= explosion.max_lifetime {
            commands.entity(entity).despawn_recursive();
        }
    }
}
