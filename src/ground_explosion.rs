// UE5 Niagara-style ground explosion with flipbook billboards
// Ported from NS_Explosion_Sand_5

use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError};
use bevy::asset::RenderAssetUsages;
use rand::Rng;

use crate::wfx_materials::AdditiveMaterial;

// ===== FLIPBOOK MATERIAL =====

/// Custom material for flipbook sprite sheets with non-square grid support
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FlipbookMaterial {
    #[uniform(0)]
    pub frame_data: Vec4, // x: frame_col, y: frame_row, z: columns, w: rows
    #[uniform(1)]
    pub color_data: Vec4, // RGB: tint color, A: alpha
    #[texture(2, dimension = "2d")]
    #[sampler(3)]
    pub sprite_texture: Handle<Image>,
}

impl Material for FlipbookMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/flipbook.wgsl".into()
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
    pub base_alpha: f32, // Original alpha for fade calculations
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

// ===== DEBUG MENU =====

/// Debug menu state for ground explosion emitter testing
#[derive(Resource, Default)]
pub struct GroundExplosionDebugMenu {
    pub active: bool,
}

/// Emitter types for individual testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterType {
    MainFireball,      // 1
    SecondaryFireball, // 2
    Smoke,             // 3
    Wisp,              // 4
    Dust,              // 5
    Spark,             // 6
    FlashSpark,        // 7
    Impact,            // 8
    Dirt,              // 9
    // VelocityDirt not mapped - only 9 keys
}

// ===== PRELOADED ASSETS =====

#[derive(Resource)]
pub struct GroundExplosionAssets {
    // Flipbook textures
    pub main_texture: Handle<Image>,        // 8x8 (64 frames)
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

    // Vertices in counter-clockwise order (facing +Z)
    let vertices = vec![
        [-half, -half, 0.0],  // bottom-left  (vertex 0)
        [ half, -half, 0.0],  // bottom-right (vertex 1)
        [ half,  half, 0.0],  // top-right    (vertex 2)
        [-half,  half, 0.0],  // top-left     (vertex 3)
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    // Bevy UV convention: [0,0] is TOP-LEFT, [1,1] is BOTTOM-RIGHT
    // This matches DirectX/Vulkan/Metal/WebGPU and image file formats
    let uvs = vec![
        [0.0, 1.0],  // bottom-left  -> UV bottom-left
        [1.0, 1.0],  // bottom-right -> UV bottom-right
        [1.0, 0.0],  // top-right    -> UV top-right
        [0.0, 0.0],  // top-left     -> UV top-left
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
        [-half, 0.0, 0.0],      // bottom-left  (vertex 0)
        [ half, 0.0, 0.0],      // bottom-right (vertex 1)
        [ half, size, 0.0],     // top-right    (vertex 2)
        [-half, size, 0.0],     // top-left     (vertex 3)
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    // Bevy UV convention: [0,0] is TOP-LEFT, [1,1] is BOTTOM-RIGHT
    let uvs = vec![
        [0.0, 1.0],  // bottom-left  -> UV bottom-left
        [1.0, 1.0],  // bottom-right -> UV bottom-right
        [1.0, 0.0],  // top-right    -> UV top-right
        [0.0, 0.0],  // top-left     -> UV top-left
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
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
) {
    info!("🌋 Spawning ground explosion at {:?} (scale: {})", position, scale);

    let mut rng = rand::thread_rng();

    // Main fireball (9x9 flipbook, velocity aligned, bottom pivot)
    spawn_main_fireball(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Secondary fireball (8x8 flipbook, velocity aligned, bottom pivot)
    spawn_secondary_fireball(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Smoke cloud (8x8 flipbook, camera facing)
    spawn_smoke_cloud(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Wisp smoke puffs (8x8 flipbook, camera facing, short duration)
    spawn_wisps(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Dust ring (4x1 flipbook, velocity aligned)
    spawn_dust_ring(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Sparks with gravity (single texture, velocity aligned)
    spawn_sparks(commands, assets, additive_materials, position, scale, &mut rng);

    // Bright flash sparks (single texture, velocity aligned)
    spawn_flash_sparks(commands, assets, additive_materials, position, scale, &mut rng);

    // Impact ground flash
    spawn_impact_flash(commands, assets, flipbook_materials, position, scale);

    // Dirt debris (single texture, camera facing)
    spawn_dirt_debris(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Velocity-stretched dirt (single texture, velocity aligned)
    spawn_velocity_dirt(commands, assets, flipbook_materials, position, scale, &mut rng);

    info!("✅ Ground explosion spawned with 10 emitters");
}

// ===== EMITTER SPAWN FUNCTIONS =====

/// Spawn a single emitter by type (for debug testing)
pub fn spawn_single_emitter(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    emitter_type: EmitterType,
    position: Vec3,
    scale: f32,
) {
    let mut rng = rand::thread_rng();
    match emitter_type {
        EmitterType::MainFireball => spawn_main_fireball(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::SecondaryFireball => spawn_secondary_fireball(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Smoke => spawn_smoke_cloud(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Wisp => spawn_wisps(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Dust => spawn_dust_ring(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Spark => spawn_sparks(commands, assets, additive_materials, position, scale, &mut rng),
        EmitterType::FlashSpark => spawn_flash_sparks(commands, assets, additive_materials, position, scale, &mut rng),
        EmitterType::Impact => spawn_impact_flash(commands, assets, flipbook_materials, position, scale),
        EmitterType::Dirt => spawn_dirt_debris(commands, assets, flipbook_materials, position, scale, &mut rng),
    }
}

/// Main fireball - 8x8 flipbook (64 frames), 1s duration, bottom pivot, velocity aligned
pub fn spawn_main_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let size = 15.0 * scale;  // 1.5x size
    let lifetime = 1.0;
    let frame_duration = lifetime / 64.0;  // 64 frames for 8x8 grid

    // Initial upward velocity for velocity alignment
    let velocity = Vec3::new(
        rng.gen_range(-0.5..0.5),
        rng.gen_range(3.0..5.0),
        rng.gen_range(-0.5..0.5),
    ) * scale;

    let material = materials.add(FlipbookMaterial {
        frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0), // col, row, columns, rows - 8x8 grid
        color_data: Vec4::new(1.0, 1.0, 1.0, 1.0), // RGB tint, alpha
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
            columns: 8,
            rows: 8,
            total_frames: 64,
            frame_duration,
            elapsed: 0.0,
            lifetime: 0.0,
            max_lifetime: lifetime,
            base_alpha: 1.0,
        },
        VelocityAligned { velocity, gravity: 0.0 },
        BottomPivot,
        GroundExplosionChild,
        Name::new("GE_MainFireball"),
    ));
}

/// Secondary fireball - 8x8 flipbook (64 frames), 1s duration
pub fn spawn_secondary_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let size = 12.0 * scale;  // 1.5x size
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

    let material = materials.add(FlipbookMaterial {
        frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0), // col, row, columns, rows
        color_data: Vec4::new(1.0, 1.0, 1.0, 1.0), // RGB tint, alpha
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
            base_alpha: 1.0,
        },
        VelocityAligned { velocity, gravity: 0.0 },
        BottomPivot,
        GroundExplosionChild,
        Name::new("GE_SecondaryFireball"),
    ));
}

/// Smoke cloud - 8x8 flipbook (64 frames), 1s duration, camera facing
pub fn spawn_smoke_cloud(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // Spawn multiple smoke particles
    let count = 3;
    let lifetime = 1.0;
    let frame_duration = lifetime / 64.0;

    for i in 0..count {
        let size = rng.gen_range(9.0..15.0) * scale;  // 1.5x size
        let offset = Vec3::new(
            rng.gen_range(-2.0..2.0) * scale,
            rng.gen_range(1.0..3.0) * scale,
            rng.gen_range(-2.0..2.0) * scale,
        );

        // Random start frame for variety
        let start_frame = rng.gen_range(0..10);

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(
                (start_frame % 8) as f32,
                (start_frame / 8) as f32,
                8.0,
                8.0, // columns, rows
            ),
            color_data: Vec4::new(0.7, 0.7, 0.7, 0.8), // Gray smoke, slightly transparent
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
                base_alpha: 0.8,
            },
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Smoke_{}", i)),
        ));
    }
}

/// Wisp smoke puffs - 8x8 flipbook, 0.5s duration, fast small puffs
pub fn spawn_wisps(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 5;
    let lifetime = 0.5;
    let frame_duration = lifetime / 64.0;

    for i in 0..count {
        let size = rng.gen_range(2.25..4.5) * scale;  // 1.5x size
        let offset = Vec3::new(
            rng.gen_range(-3.0..3.0) * scale,
            rng.gen_range(0.5..2.0) * scale,
            rng.gen_range(-3.0..3.0) * scale,
        );

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0), // col, row, columns, rows
            color_data: Vec4::new(0.8, 0.8, 0.8, 0.7), // alpha in color_data.w
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
                base_alpha: 0.7,
            },
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Wisp_{}", i)),
        ));
    }
}

/// Dust ring - 4x1 flipbook (4 frames), velocity aligned horizontal spray
pub fn spawn_dust_ring(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 8;
    let lifetime = 1.0;
    let frame_duration = lifetime / 4.0;

    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let size = rng.gen_range(3.0..6.0) * scale;  // 1.5x size

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

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 4.0, 1.0), // col, row, columns (4), rows (1)
            color_data: Vec4::new(0.8, 0.7, 0.6, 0.6), // Sandy brown, alpha
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
                base_alpha: 0.6,
            },
            VelocityAligned { velocity, gravity: 2.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Dust_{}", i)),
        ));
    }
}

/// Sparks - single texture embers with gravity, velocity aligned
pub fn spawn_sparks(
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
        let size = rng.gen_range(0.15..0.45) * scale;  // 1.5x size

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
                base_alpha: 1.0,
            },
            VelocityAligned { velocity, gravity: 8.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Spark_{}", i)),
        ));
    }
}

/// Flash sparks - bright quick sparks, 1s duration
pub fn spawn_flash_sparks(
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
        let size = rng.gen_range(0.225..0.6) * scale;  // 1.5x size

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
                base_alpha: 1.0,
            },
            VelocityAligned { velocity, gravity: 5.0 },
            GroundExplosionChild,
            Name::new(format!("GE_FlashSpark_{}", i)),
        ));
    }
}

/// Impact flash - ground glow at explosion origin
/// Uses FlipbookMaterial with alpha blending for proper texture alpha support
pub fn spawn_impact_flash(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
) {
    let size = 15.0 * scale;  // 1.5x size
    let lifetime = 0.3;  // Quick flash

    let material = materials.add(FlipbookMaterial {
        frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),  // Single frame
        color_data: Vec4::new(1.0, 0.9, 0.7, 1.0),  // Warm yellow-orange tint
        sprite_texture: assets.impact_texture.clone(),
    });

    // Position slightly above ground, facing up (laying flat on the ground)
    commands.spawn((
        Mesh3d(assets.centered_quad.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position + Vec3::Y * 0.15)
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
            base_alpha: 1.0,
        },
        GroundExplosionChild,
        Name::new("GE_ImpactFlash"),
    ));
}

/// Dirt debris - billboard dirt chunks, camera facing
pub fn spawn_dirt_debris(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 8;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.45..1.2) * scale;  // 1.5x size

        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(4.0..8.0) * scale;

        let velocity = Vec3::new(
            theta.cos() * speed,
            rng.gen_range(5.0..10.0) * scale,
            theta.sin() * speed,
        );

        let material = materials.add(FlipbookMaterial {
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
                base_alpha: 1.0,
            },
            CameraFacing,
            VelocityAligned { velocity, gravity: 12.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Dirt_{}", i)),
        ));
    }
}

/// Velocity-stretched dirt - debris that stretches along velocity
pub fn spawn_velocity_dirt(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 6;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.6..1.35) * scale;  // 1.5x size

        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(6.0..12.0) * scale;

        let velocity = Vec3::new(
            theta.cos() * speed,
            rng.gen_range(8.0..15.0) * scale,
            theta.sin() * speed,
        );

        let material = materials.add(FlipbookMaterial {
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
                base_alpha: 1.0,
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
        &MeshMaterial3d<FlipbookMaterial>,
        Option<&Name>,
    )>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut sprite, material_handle, name) in query.iter_mut() {
        sprite.elapsed += dt;
        sprite.lifetime += dt;

        // Calculate current frame
        let frame = ((sprite.elapsed / sprite.frame_duration) as u32) % sprite.total_frames;
        let col = frame % sprite.columns;
        let row = frame / sprite.columns;

        // Debug: log frame progression for main fireball
        if let Some(n) = name {
            if n.as_str().contains("MainFireball") && frame % 10 == 0 {
                trace!("🎞️ {} frame={} (col={}, row={}) elapsed={:.2}s", n, frame, col, row, sprite.elapsed);
            }
        }

        // Calculate alpha fade (fade out in last 20% of lifetime)
        let progress = sprite.lifetime / sprite.max_lifetime;
        let alpha = if progress > 0.8 {
            1.0 - (progress - 0.8) * 5.0
        } else {
            1.0
        };

        // Update material - frame_data.xy = current frame, color_data.w = alpha
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.frame_data.x = col as f32;
            material.frame_data.y = row as f32;
            // Use base_alpha from component, multiply by fade factor
            material.color_data.w = (sprite.base_alpha * alpha).max(0.0);
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

// ===== DEBUG MENU SYSTEM =====

/// Debug menu system - P to toggle, 1-9 to spawn individual emitters
pub fn ground_explosion_debug_menu_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut debug_menu: ResMut<GroundExplosionDebugMenu>,
    mut commands: Commands,
    ground_assets: Option<Res<GroundExplosionAssets>>,
    mut flipbook_materials: ResMut<Assets<FlipbookMaterial>>,
    mut additive_materials: ResMut<Assets<AdditiveMaterial>>,
) {
    // P key toggles the debug menu
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        debug_menu.active = !debug_menu.active;
        if debug_menu.active {
            info!("🔧 Ground Explosion Debug Menu ACTIVE");
            info!("   F1: Main Fireball (8x8 flipbook)");
            info!("   F2: Secondary Fireball (8x8 flipbook)");
            info!("   F3: Smoke Cloud (8x8 flipbook)");
            info!("   F4: Wisps (8x8 flipbook)");
            info!("   F5: Dust Ring (4x1 flipbook)");
            info!("   F6: Sparks (additive)");
            info!("   F7: Flash Sparks (additive)");
            info!("   F8: Impact Flash (additive)");
            info!("   F9: Dirt Debris");
            info!("   F10: FULL EXPLOSION (all emitters)");
            info!("   P: Close menu");
        } else {
            info!("🔧 Ground Explosion Debug Menu CLOSED");
        }
        return;
    }

    // Only process F keys when menu is active
    if !debug_menu.active {
        return;
    }

    let Some(assets) = ground_assets else {
        if keyboard_input.any_just_pressed([
            KeyCode::F1, KeyCode::F2, KeyCode::F3,
            KeyCode::F4, KeyCode::F5, KeyCode::F6,
            KeyCode::F7, KeyCode::F8, KeyCode::F9,
        ]) {
            warn!("Ground explosion assets not loaded yet!");
        }
        return;
    };

    let position = Vec3::new(0.0, 0.0, 0.0);
    let scale = 1.0;

    let emitter = if keyboard_input.just_pressed(KeyCode::F1) {
        Some((EmitterType::MainFireball, "Main Fireball"))
    } else if keyboard_input.just_pressed(KeyCode::F2) {
        Some((EmitterType::SecondaryFireball, "Secondary Fireball"))
    } else if keyboard_input.just_pressed(KeyCode::F3) {
        Some((EmitterType::Smoke, "Smoke Cloud"))
    } else if keyboard_input.just_pressed(KeyCode::F4) {
        Some((EmitterType::Wisp, "Wisps"))
    } else if keyboard_input.just_pressed(KeyCode::F5) {
        Some((EmitterType::Dust, "Dust Ring"))
    } else if keyboard_input.just_pressed(KeyCode::F6) {
        Some((EmitterType::Spark, "Sparks"))
    } else if keyboard_input.just_pressed(KeyCode::F7) {
        Some((EmitterType::FlashSpark, "Flash Sparks"))
    } else if keyboard_input.just_pressed(KeyCode::F8) {
        Some((EmitterType::Impact, "Impact Flash"))
    } else if keyboard_input.just_pressed(KeyCode::F9) {
        Some((EmitterType::Dirt, "Dirt Debris"))
    } else {
        None
    };

    if let Some((emitter_type, name)) = emitter {
        spawn_single_emitter(
            &mut commands,
            &assets,
            &mut flipbook_materials,
            &mut additive_materials,
            emitter_type,
            position,
            scale,
        );
        info!("🌋 Spawned: {} at (0, 0, 0)", name);
    }

    // F10 spawns the combined full explosion effect
    if keyboard_input.just_pressed(KeyCode::F10) {
        spawn_ground_explosion(
            &mut commands,
            &assets,
            &mut flipbook_materials,
            &mut additive_materials,
            position,
            1.5,  // Use 1.5x scale for combined effect
        );
        info!("🌋 Spawned: FULL GROUND EXPLOSION at (0, 0, 0)");
    }
}
