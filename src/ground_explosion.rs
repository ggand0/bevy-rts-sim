// UE5 Niagara-style ground explosion with flipbook billboards
// Ported from NS_Explosion_Sand_5

use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use rand::Rng;

use crate::explosion_shader::ExplosionMaterial;
use crate::wfx_materials::AdditiveMaterial;

// ===== COMPONENTS =====

/// Flipbook sprite animation component
#[derive(Component)]
pub struct FlipbookSprite {
    pub columns: u32,
    pub rows: u32,
    pub total_frames: u32,
    pub frame_duration: f32,
    pub elapsed: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Velocity-aligned billboard - sprite up-axis follows velocity direction
#[derive(Component)]
pub struct VelocityAligned {
    pub velocity: Vec3,
    pub gravity: f32,
}

/// Standard camera-facing billboard (unaligned mode)
#[derive(Component)]
pub struct CameraFacing;

/// Marker for bottom-pivot billboards (fireballs grow upward)
#[derive(Component)]
pub struct BottomPivot;

/// Ground explosion parent entity for lifetime tracking
#[derive(Component)]
pub struct GroundExplosion {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Marker for ground explosion child entities
#[derive(Component)]
pub struct GroundExplosionChild;

// ===== PRELOADED ASSETS =====

#[derive(Resource)]
pub struct GroundExplosionAssets {
    // Flipbook textures
    pub main_texture: Handle<Image>,        // 9x9 (81 frames)
    pub secondary_texture: Handle<Image>,   // 8x8 (64 frames)
    pub smoke_texture: Handle<Image>,       // 8x8 (64 frames)
    pub wisp_texture: Handle<Image>,        // 8x8 (64 frames)
    pub dust_texture: Handle<Image>,        // 4x1 (4 frames)
    // Single-frame textures
    pub dirt_texture: Handle<Image>,
    pub flare_texture: Handle<Image>,
    pub impact_texture: Handle<Image>,
    // Shared meshes
    pub centered_quad: Handle<Mesh>,
    pub bottom_pivot_quad: Handle<Mesh>,
}

// ===== MESH CREATION =====

/// Create a standard centered quad mesh
fn create_centered_quad(size: f32) -> Mesh {
    let half = size / 2.0;

    let vertices = vec![
        [-half, -half, 0.0],
        [ half, -half, 0.0],
        [ half,  half, 0.0],
        [-half,  half, 0.0],
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    let uvs = vec![
        [0.0, 1.0],
        [1.0, 1.0],
        [1.0, 0.0],
        [0.0, 0.0],
    ];

    let indices = Indices::U32(vec![0, 1, 2, 0, 2, 3]);

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(indices)
}

/// Create a bottom-pivot quad mesh (origin at bottom-center, grows upward)
fn create_bottom_pivot_quad(size: f32) -> Mesh {
    let half = size / 2.0;

    // Vertices offset so origin is at bottom-center
    let vertices = vec![
        [-half, 0.0, 0.0],      // bottom-left
        [ half, 0.0, 0.0],      // bottom-right
        [ half, size, 0.0],     // top-right
        [-half, size, 0.0],     // top-left
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    let uvs = vec![
        [0.0, 1.0],
        [1.0, 1.0],
        [1.0, 0.0],
        [0.0, 0.0],
    ];

    let indices = Indices::U32(vec![0, 1, 2, 0, 2, 3]);

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(indices)
}

// ===== ASSET LOADING =====

pub fn setup_ground_explosion_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("🌋 Loading ground explosion assets...");

    let assets = GroundExplosionAssets {
        // Premium assets - CC-BY licensed, not included in repo
        // See assets/textures/premium/ground_explosion/
        main_texture: asset_server.load("textures/premium/ground_explosion/main_9x9.png"),
        secondary_texture: asset_server.load("textures/premium/ground_explosion/secondary_8x8.png"),
        smoke_texture: asset_server.load("textures/premium/ground_explosion/smoke_8x8.png"),
        wisp_texture: asset_server.load("textures/premium/ground_explosion/wisp_8x8.png"),
        dust_texture: asset_server.load("textures/premium/ground_explosion/dust_4x1.png"),
        dirt_texture: asset_server.load("textures/premium/ground_explosion/dirt.png"),
        flare_texture: asset_server.load("textures/premium/ground_explosion/flare.png"),
        impact_texture: asset_server.load("textures/premium/ground_explosion/impact.png"),
        centered_quad: meshes.add(create_centered_quad(1.0)),
        bottom_pivot_quad: meshes.add(create_bottom_pivot_quad(1.0)),
    };

    commands.insert_resource(assets);
    info!("✅ Ground explosion assets loaded");
}

// ===== MAIN SPAWN FUNCTION =====

/// Spawn a complete UE5-style ground explosion
pub fn spawn_ground_explosion(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    explosion_materials: &mut ResMut<Assets<ExplosionMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
) {
    info!("🌋 Spawning ground explosion at {:?} (scale: {})", position, scale);

    let mut rng = rand::thread_rng();

    // Main fireball (9x9 flipbook, velocity aligned, bottom pivot)
    spawn_main_fireball(commands, assets, explosion_materials, position, scale, &mut rng);

    // Secondary fireball (8x8 flipbook, velocity aligned, bottom pivot)
    spawn_secondary_fireball(commands, assets, explosion_materials, position, scale, &mut rng);

    // Smoke cloud (8x8 flipbook, camera facing)
    spawn_smoke_cloud(commands, assets, explosion_materials, position, scale, &mut rng);

    // Wisp smoke puffs (8x8 flipbook, camera facing, short duration)
    spawn_wisps(commands, assets, explosion_materials, position, scale, &mut rng);

    // Dust ring (4x1 flipbook, velocity aligned)
    spawn_dust_ring(commands, assets, explosion_materials, position, scale, &mut rng);

    // Sparks with gravity (single texture, velocity aligned)
    spawn_sparks(commands, assets, additive_materials, position, scale, &mut rng);

    // Bright flash sparks (single texture, velocity aligned)
    spawn_flash_sparks(commands, assets, additive_materials, position, scale, &mut rng);

    // Impact ground flash
    spawn_impact_flash(commands, assets, additive_materials, position, scale);

    // Dirt debris (single texture, camera facing)
    spawn_dirt_debris(commands, assets, explosion_materials, position, scale, &mut rng);

    // Velocity-stretched dirt (single texture, velocity aligned)
    spawn_velocity_dirt(commands, assets, explosion_materials, position, scale, &mut rng);

    info!("✅ Ground explosion spawned with 10 emitters");
}

// ===== EMITTER SPAWN FUNCTIONS =====

/// Main fireball - 9x9 flipbook (81 frames), 1s duration, bottom pivot, velocity aligned
fn spawn_main_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let size = 10.0 * scale;
    let lifetime = 1.0;
    let frame_duration = lifetime / 81.0;

    // Initial upward velocity for velocity alignment
    let velocity = Vec3::new(
        rng.gen_range(-0.5..0.5),
        rng.gen_range(3.0..5.0),
        rng.gen_range(-0.5..0.5),
    ) * scale;

    let material = materials.add(ExplosionMaterial {
        frame_data: Vec4::new(0.0, 0.0, 9.0, 1.0),
        color_data: Vec4::new(1.0, 1.0, 1.0, 2.0),
        sprite_texture: assets.main_texture.clone(),
    });

    commands.spawn((
        Mesh3d(assets.bottom_pivot_quad.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(Vec3::splat(size)),
        Visibility::Visible,
        NotShadowCaster,
        NotShadowReceiver,
        FlipbookSprite {
            columns: 9,
            rows: 9,
            total_frames: 81,
            frame_duration,
            elapsed: 0.0,
            lifetime: 0.0,
            max_lifetime: lifetime,
        },
        VelocityAligned { velocity, gravity: 0.0 },
        BottomPivot,
        GroundExplosionChild,
        Name::new("GE_MainFireball"),
    ));
}

/// Secondary fireball - 8x8 flipbook (64 frames), 1s duration
fn spawn_secondary_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let size = 8.0 * scale;
    let lifetime = 1.0;
    let frame_duration = lifetime / 64.0;

    // Offset slightly from main fireball
    let offset = Vec3::new(
        rng.gen_range(-1.0..1.0) * scale,
        rng.gen_range(0.5..1.5) * scale,
        rng.gen_range(-1.0..1.0) * scale,
    );

    let velocity = Vec3::new(
        rng.gen_range(-0.5..0.5),
        rng.gen_range(2.0..4.0),
        rng.gen_range(-0.5..0.5),
    ) * scale;

    let material = materials.add(ExplosionMaterial {
        frame_data: Vec4::new(0.0, 0.0, 8.0, 1.0),
        color_data: Vec4::new(1.0, 1.0, 1.0, 2.0),
        sprite_texture: assets.secondary_texture.clone(),
    });

    commands.spawn((
        Mesh3d(assets.bottom_pivot_quad.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position + offset).with_scale(Vec3::splat(size)),
        Visibility::Visible,
        NotShadowCaster,
        NotShadowReceiver,
        FlipbookSprite {
            columns: 8,
            rows: 8,
            total_frames: 64,
            frame_duration,
            elapsed: 0.0,
            lifetime: 0.0,
            max_lifetime: lifetime,
        },
        VelocityAligned { velocity, gravity: 0.0 },
        BottomPivot,
        GroundExplosionChild,
        Name::new("GE_SecondaryFireball"),
    ));
}

/// Smoke cloud - 8x8 flipbook (64 frames), 1s duration, camera facing
fn spawn_smoke_cloud(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // Spawn multiple smoke particles
    let count = 3;
    let lifetime = 1.0;
    let frame_duration = lifetime / 64.0;

    for i in 0..count {
        let size = rng.gen_range(6.0..10.0) * scale;
        let offset = Vec3::new(
            rng.gen_range(-2.0..2.0) * scale,
            rng.gen_range(1.0..3.0) * scale,
            rng.gen_range(-2.0..2.0) * scale,
        );

        // Random start frame for variety
        let start_frame = rng.gen_range(0..10);

        let material = materials.add(ExplosionMaterial {
            frame_data: Vec4::new(
                (start_frame % 8) as f32,
                (start_frame / 8) as f32,
                8.0,
                0.8, // Slightly transparent smoke
            ),
            color_data: Vec4::new(0.7, 0.7, 0.7, 1.0), // Gray smoke
            sprite_texture: assets.smoke_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + offset).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames: 64,
                frame_duration,
                elapsed: start_frame as f32 * frame_duration,
                lifetime: 0.0,
                max_lifetime: lifetime + rng.gen_range(0.0..0.5),
            },
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Smoke_{}", i)),
        ));
    }
}

/// Wisp smoke puffs - 8x8 flipbook, 0.5s duration, fast small puffs
fn spawn_wisps(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 5;
    let lifetime = 0.5;
    let frame_duration = lifetime / 64.0;

    for i in 0..count {
        let size = rng.gen_range(1.5..3.0) * scale;
        let offset = Vec3::new(
            rng.gen_range(-3.0..3.0) * scale,
            rng.gen_range(0.5..2.0) * scale,
            rng.gen_range(-3.0..3.0) * scale,
        );

        let material = materials.add(ExplosionMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 0.7),
            color_data: Vec4::new(0.8, 0.8, 0.8, 1.0),
            sprite_texture: assets.wisp_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + offset).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames: 64,
                frame_duration,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Wisp_{}", i)),
        ));
    }
}

/// Dust ring - 4x1 flipbook (4 frames), velocity aligned horizontal spray
fn spawn_dust_ring(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 8;
    let lifetime = 1.0;
    let frame_duration = lifetime / 4.0;

    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let size = rng.gen_range(2.0..4.0) * scale;

        // Radial outward velocity
        let velocity = Vec3::new(
            angle.cos() * rng.gen_range(3.0..6.0),
            rng.gen_range(0.5..1.5),
            angle.sin() * rng.gen_range(3.0..6.0),
        ) * scale;

        let offset = Vec3::new(
            angle.cos() * scale,
            0.2 * scale,
            angle.sin() * scale,
        );

        let material = materials.add(ExplosionMaterial {
            frame_data: Vec4::new(0.0, 0.0, 4.0, 0.6),
            color_data: Vec4::new(0.8, 0.7, 0.6, 1.0), // Sandy brown
            sprite_texture: assets.dust_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + offset).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 4,
                rows: 1,
                total_frames: 4,
                frame_duration,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            VelocityAligned { velocity, gravity: 2.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Dust_{}", i)),
        ));
    }
}

/// Sparks - single texture embers with gravity, velocity aligned
fn spawn_sparks(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 15;
    let lifetime = 2.0;

    for i in 0..count {
        let size = rng.gen_range(0.1..0.3) * scale;

        // Random outward velocity with upward bias
        let theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi: f32 = rng.gen_range(0.2..1.2);
        let speed: f32 = rng.gen_range(8.0..15.0) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed + rng.gen_range(2.0..5.0) * scale,
            phi.sin() * theta.sin() * speed,
        );

        let material = materials.add(AdditiveMaterial {
            tint_color: Vec4::new(1.0, 0.8, 0.3, 1.0), // Orange-yellow
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
            particle_texture: assets.flare_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            VelocityAligned { velocity, gravity: 8.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Spark_{}", i)),
        ));
    }
}

/// Flash sparks - bright quick sparks, 1s duration
fn spawn_flash_sparks(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 10;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.15..0.4) * scale;

        let theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi: f32 = rng.gen_range(0.3..1.0);
        let speed: f32 = rng.gen_range(10.0..20.0) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed,
            phi.sin() * theta.sin() * speed,
        );

        let material = materials.add(AdditiveMaterial {
            tint_color: Vec4::new(1.0, 1.0, 0.9, 1.0), // Bright white-yellow
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
            particle_texture: assets.flare_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            VelocityAligned { velocity, gravity: 5.0 },
            GroundExplosionChild,
            Name::new(format!("GE_FlashSpark_{}", i)),
        ));
    }
}

/// Impact flash - ground glow at explosion origin
fn spawn_impact_flash(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
) {
    let size = 8.0 * scale;
    let lifetime = 0.5;

    let material = materials.add(AdditiveMaterial {
        tint_color: Vec4::new(1.0, 0.9, 0.7, 1.0),
        soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
        particle_texture: assets.impact_texture.clone(),
    });

    // Position slightly above ground, facing up
    commands.spawn((
        Mesh3d(assets.centered_quad.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position + Vec3::Y * 0.1)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(size)),
        Visibility::Visible,
        NotShadowCaster,
        NotShadowReceiver,
        FlipbookSprite {
            columns: 1,
            rows: 1,
            total_frames: 1,
            frame_duration: lifetime,
            elapsed: 0.0,
            lifetime: 0.0,
            max_lifetime: lifetime,
        },
        GroundExplosionChild,
        Name::new("GE_ImpactFlash"),
    ));
}

/// Dirt debris - billboard dirt chunks, camera facing
fn spawn_dirt_debris(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 8;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.3..0.8) * scale;

        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(4.0..8.0) * scale;

        let velocity = Vec3::new(
            theta.cos() * speed,
            rng.gen_range(5.0..10.0) * scale,
            theta.sin() * speed,
        );

        let material = materials.add(ExplosionMaterial {
            frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
            color_data: Vec4::new(0.6, 0.5, 0.4, 1.0), // Brown dirt
            sprite_texture: assets.dirt_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * 0.5 * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            CameraFacing,
            VelocityAligned { velocity, gravity: 12.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Dirt_{}", i)),
        ));
    }
}

/// Velocity-stretched dirt - debris that stretches along velocity
fn spawn_velocity_dirt(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<ExplosionMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 6;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.4..0.9) * scale;

        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(6.0..12.0) * scale;

        let velocity = Vec3::new(
            theta.cos() * speed,
            rng.gen_range(8.0..15.0) * scale,
            theta.sin() * speed,
        );

        let material = materials.add(ExplosionMaterial {
            frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
            color_data: Vec4::new(0.5, 0.4, 0.3, 1.0),
            sprite_texture: assets.dirt_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * 0.5 * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            VelocityAligned { velocity, gravity: 10.0 },
            GroundExplosionChild,
            Name::new(format!("GE_VelDirt_{}", i)),
        ));
    }
}

// ===== ANIMATION SYSTEMS =====

/// Update flipbook sprite animations and lifetime
pub fn animate_flipbook_sprites(
    mut query: Query<(
        &mut FlipbookSprite,
        &MeshMaterial3d<ExplosionMaterial>,
    )>,
    mut materials: ResMut<Assets<ExplosionMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut sprite, material_handle) in query.iter_mut() {
        sprite.elapsed += dt;
        sprite.lifetime += dt;

        // Calculate current frame
        let frame = ((sprite.elapsed / sprite.frame_duration) as u32) % sprite.total_frames;
        let col = frame % sprite.columns;
        let row = frame / sprite.columns;

        // Calculate alpha fade (fade out in last 20% of lifetime)
        let progress = sprite.lifetime / sprite.max_lifetime;
        let alpha = if progress > 0.8 {
            1.0 - (progress - 0.8) * 5.0
        } else {
            1.0
        };

        // Update material
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.frame_data.x = col as f32;
            material.frame_data.y = row as f32;
            material.frame_data.w = alpha.max(0.0);
        }
    }
}

/// Update velocity-aligned billboards
pub fn update_velocity_aligned_billboards(
    mut query: Query<(
        &mut Transform,
        &mut VelocityAligned,
        Option<&BottomPivot>,
    )>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    let camera_pos = camera_query
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    for (mut transform, mut vel_aligned, bottom_pivot) in query.iter_mut() {
        // Apply gravity
        vel_aligned.velocity.y -= vel_aligned.gravity * dt;

        // Apply velocity to position
        transform.translation += vel_aligned.velocity * dt;

        // Calculate rotation to align with velocity
        let velocity_dir = vel_aligned.velocity.normalize_or_zero();

        if velocity_dir.length_squared() > 0.001 {
            // For velocity-aligned sprites, the up-axis points along velocity
            // and the sprite faces the camera
            let up = velocity_dir;
            let to_camera = (camera_pos - transform.translation).normalize_or_zero();

            // Project to_camera onto the plane perpendicular to up
            let forward = (to_camera - up * to_camera.dot(up)).normalize_or_zero();

            if forward.length_squared() > 0.001 {
                let right = up.cross(forward).normalize();
                let corrected_forward = right.cross(up);

                // For bottom-pivot, we want the quad to extend upward from origin
                if bottom_pivot.is_some() {
                    transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, corrected_forward));
                } else {
                    transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, corrected_forward));
                }
            }
        } else {
            // Fallback to camera-facing when velocity is near zero
            let to_camera = (camera_pos - transform.translation).normalize_or_zero();
            if to_camera.length_squared() > 0.001 {
                let forward = to_camera;
                let right = Vec3::Y.cross(forward).normalize_or_zero();
                if right.length_squared() > 0.001 {
                    let up = forward.cross(right);
                    transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
                }
            }
        }
    }
}

/// Update camera-facing billboards
pub fn update_camera_facing_billboards(
    mut query: Query<&mut Transform, (With<CameraFacing>, With<GroundExplosionChild>)>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
) {
    let camera_pos = camera_query
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    for mut transform in query.iter_mut() {
        let to_camera = (camera_pos - transform.translation).normalize_or_zero();

        if to_camera.length_squared() > 0.001 {
            let forward = to_camera;
            let right = Vec3::Y.cross(forward).normalize_or_zero();
            if right.length_squared() > 0.001 {
                let up = forward.cross(right);
                transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
            }
        }
    }
}

/// Cleanup expired ground explosion particles
pub fn cleanup_ground_explosions(
    mut commands: Commands,
    query: Query<(Entity, &FlipbookSprite), With<GroundExplosionChild>>,
) {
    for (entity, sprite) in query.iter() {
        if sprite.lifetime >= sprite.max_lifetime {
            commands.entity(entity).despawn();
        }
    }
}

/// Update additive material alpha for sparks
pub fn animate_additive_sprites(
    query: Query<(
        &FlipbookSprite,
        &MeshMaterial3d<AdditiveMaterial>,
    ), With<GroundExplosionChild>>,
    mut materials: ResMut<Assets<AdditiveMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        let progress = sprite.lifetime / sprite.max_lifetime;
        let alpha = if progress > 0.7 {
            1.0 - (progress - 0.7) * 3.33
        } else {
            1.0
        };

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.tint_color.w = alpha.max(0.0);
        }
    }
}
