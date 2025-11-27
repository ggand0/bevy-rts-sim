// War FX explosion spawner with UV-scrolling billboards
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use crate::wfx_materials::{SmokeScrollMaterial, AdditiveMaterial, SmokeOnlyMaterial};
use rand::Rng;

/// Create a quad mesh with proper UVs for texture sampling
fn create_quad_mesh(size: f32) -> Mesh {
    let half = size / 2.0;

    let vertices = vec![
        [-half, -half, 0.0], // bottom-left
        [ half, -half, 0.0], // bottom-right
        [ half,  half, 0.0], // top-right
        [-half,  half, 0.0], // top-left
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    let uvs = vec![
        [0.0, 1.0], // bottom-left (UV origin is top-left in most textures)
        [1.0, 1.0], // bottom-right
        [1.0, 0.0], // top-right
        [0.0, 0.0], // top-left
    ];

    let indices = Indices::U32(vec![0, 1, 2, 0, 2, 3]);

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(indices)
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

    // Match Unity WFX_ExplosiveSmoke_Big2 configuration exactly
    // Unity spawns exactly 2 particles: 1 at t=0, 1 at t=0.1s
    let glow_count = 2;

    for i in 0..glow_count {
        // No position offset - Unity spawns at exact position
        let offset = Vec3::ZERO;

        // White tint (Unity default)
        let glow_material = additive_materials.add(AdditiveMaterial {
            tint_color: Vec4::new(1.0, 1.0, 1.0, 1.0), // Full white, alpha controlled by curve
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),  // Unity default InvFade = 1.0
            particle_texture: glow_texture.clone(),
        });

        // Base quad size: Unity start size = 9 (much larger than other emitters)
        let quad_size = 9.0 * scale;
        let quad_mesh = meshes.add(Rectangle::new(quad_size, quad_size));

        // Unity WFX_ExplosiveSmoke_Big2 lifetime: 0.7s constant
        let lifetime = 0.7;

        // Glow stays centered at explosion origin (no drift)
        let velocity = Vec3::ZERO;

        // Unity: Start Rotation = 45° fixed (not spinning)
        let rotation_speed = 0.0;

        // Unity Size Over Lifetime curve - grow then shrink (fireball expansion/contraction)
        // Original has curved tangents; using more keyframes to approximate the shape:
        // - Fast initial growth (smooth acceleration)
        // - Peak at 60%
        // - Steep shrink at end (steep -3.32 tangent)
        let scale_curve = AnimationCurve {
            keyframes: vec![
                (0.0, 0.3),    // Start small (30%)
                (0.10, 0.5),   // Quick initial growth
                (0.20, 0.7),   // Continue growing
                (0.50, 0.95),  // Near peak
                (0.60, 1.0),   // Peak at full size
                (0.75, 0.8),   // Start shrinking
                (0.90, 0.6),   // Continue shrinking (steep)
                (1.0, 0.5),    // End at 50%
            ],
        };

        // Unity Alpha Over Lifetime curve (from EMBEDDED_GLOW_EMITTER_DETAILS.md):
        // 0% → 0.0, 10% → 1.0, 25% → 1.0, 100% → 0.0
        // Quick fade-in (0-10%), brief hold (10-25%), long gradual fade-out (25-100%)
        // End at 98% to ensure fade completes before despawn at 100%
        let alpha_curve = AnimationCurve {
            keyframes: vec![
                (0.0, 0.0),    // Start invisible
                (0.10, 1.0),   // Fade in by 10%
                (0.25, 1.0),   // Hold bright until 25%
                (0.98, 0.0),   // Fade out to 0 just before end
                (1.0, 0.0),    // Stay at 0
            ],
        };

        // Unity Color Over Lifetime curve (from EMBEDDED_GLOW_EMITTER_DETAILS.md):
        // t=0%: rgb(0.976, 0.753, 0.714) - Pink/coral
        // t=50%: rgb(1.0, 0.478, 0.478) - Salmon/red (holds to end)
        let color_curve = ColorCurve {
            keyframes: vec![
                (0.0, Vec3::new(0.976, 0.753, 0.714)),  // Pink/coral - warm heat
                (0.5, Vec3::new(1.0, 0.478, 0.478)),    // Salmon - saturated red
                (1.0, Vec3::new(1.0, 0.478, 0.478)),    // Hold salmon to end
            ],
        };

        commands.spawn((
            MaterialMeshBundle {
                mesh: quad_mesh,
                material: glow_material,
                transform: Transform::from_translation(position + offset)
                    .with_rotation(Quat::from_rotation_z(45.0_f32.to_radians())), // Unity: 45° rotation
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
                scale_curve,
                alpha_curve,
                color_curve,
                velocity,
                rotation_speed,
                base_rotation: 0.0,
            },
            Name::new(format!("WFX_Glow_{}", i)),
        ));
    }

    info!("✅ WAR FX: Spawned {} glow billboards", glow_count);
}

/// Spawns glow sparkles emitter - fast-moving ember particles with gravity
/// Unity emitter: "Glow sparkles" - bright sparks that shoot outward and fall
///
/// Key characteristics:
/// - High speed: 24 units/sec (vs 0.4 for smoke)
/// - Strong gravity: 4 units/sec² creates arcing trajectories
/// - Short lifetime: 0.25-0.35 seconds
/// - Fire color gradient: white → yellow → orange → red
/// - Additive blending for glow effect
/// - Front-loaded bursts: 20+10+5 particles in 0.1 seconds
pub fn spawn_glow_sparkles(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    let glow_texture = asset_server.load("textures/wfx/WFX_T_GlowCircle A8.png");

    info!("✨ WAR FX: Spawning glow sparkles at {:?} (35 particles)", position);

    let mut rng = rand::thread_rng();

    // Burst configuration from Unity: (delay_seconds, particle_count)
    // Front-loaded: 20+10+5 = 35 particles over 0.1 seconds
    let bursts = [
        (0.00_f32, 20_u32),
        (0.05_f32, 10_u32),
        (0.10_f32, 5_u32),
    ];

    // Tiny sparkle size (Unity: 0.05 to 0.1)
    let base_size = 0.15 * scale; // Slightly larger for visibility
    let quad_mesh = meshes.add(Rectangle::new(base_size, base_size));

    for (delay, count) in bursts {
        for _ in 0..count {
            // Random direction - wider spread (70°) for more horizontal spray
            // phi measured from vertical (Y axis), so 70° gives good horizontal coverage
            let theta = rng.gen_range(0.0..std::f32::consts::TAU); // Full circle around Y
            let phi = rng.gen_range(0.0..(70.0_f32).to_radians()); // 70° cone from vertical
            let dir = Vec3::new(
                phi.sin() * theta.cos(),
                phi.cos(), // Y component (upward bias decreases with larger phi)
                phi.sin() * theta.sin(),
            ).normalize();

            // Small random offset from center (Unity radius: 2)
            let spawn_offset = dir * rng.gen_range(0.0..2.0) * scale;

            // High initial velocity (Unity: speed 24)
            let speed = 24.0 * scale;
            let initial_velocity = dir * speed;

            // Random lifetime (Unity: 0.25 to 0.35 seconds)
            let lifetime = rng.gen_range(0.25..0.35);

            // Random size multiplier (Unity: 0.05 to 0.1, normalized)
            let size_mult = rng.gen_range(0.5..1.0);

            // Size curve: grow from 40% to full size
            // Include global scale factor so sparkles scale with explosion
            let scale_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 0.399 * size_mult * scale),
                    (0.347, 0.764 * size_mult * scale),
                    (1.0, 0.990 * size_mult * scale),
                ],
            };

            // Alpha curve: stay bright, quick fade at end
            let alpha_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 1.0),
                    (0.8, 1.0),
                    (1.0, 0.0),
                ],
            };

            // Color gradient: white → yellow → orange → dark red
            let color_curve = ColorCurve {
                keyframes: vec![
                    (0.0, Vec3::new(1.0, 1.0, 1.0)),           // White (hottest)
                    (0.076, Vec3::new(1.0, 0.973, 0.843)),     // Warm white
                    (0.25, Vec3::new(1.0, 0.945, 0.471)),      // Yellow
                    (0.515, Vec3::new(1.0, 0.796, 0.420)),     // Orange
                    (0.779, Vec3::new(0.718, 0.196, 0.0)),     // Dark red/ember
                ],
            };

            // Create additive material (same as embedded glow)
            let sparkle_material = additive_materials.add(AdditiveMaterial {
                tint_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
                particle_texture: glow_texture.clone(),
            });

            commands.spawn((
                MaterialMeshBundle {
                    mesh: quad_mesh.clone(),
                    material: sparkle_material,
                    transform: Transform::from_translation(position + spawn_offset)
                        .with_scale(Vec3::splat(0.399 * size_mult)), // Start at initial size
                    visibility: if delay == 0.0 { Visibility::Visible } else { Visibility::Hidden },
                    ..Default::default()
                },
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                WarFXExplosion {
                    lifetime: 0.0,
                    max_lifetime: lifetime,
                },
                WarFxFlame {
                    spawn_delay: delay,
                    active: delay == 0.0,
                },
                AnimatedSparkle {
                    scale_curve,
                    alpha_curve,
                    color_curve,
                    velocity: initial_velocity,
                    gravity: 4.0 * scale, // Unity gravity modifier: 4
                },
                Name::new("WFX_Sparkle"),
            ));
        }
    }

    info!("✅ WAR FX: Spawned 35 glow sparkles");
}

/// Spawns dot sparkles emitter - dense shower of bright sparks
/// Unity emitter: "Dot Sparkles" - fast-moving sparks that spray outward with gravity
///
/// Key characteristics:
/// - Random high speed: 12-24 units/sec (varied spread)
/// - Moderate gravity: 2 units/sec² (gentler arcs than glow sparkles)
/// - Short lifetime: 0.2-0.3 seconds
/// - 75 particles total in 3 equal bursts (25×3)
/// - Fire color gradient: white → yellow → orange → red
/// - Narrower spread: 25° cone (more focused than glow sparkles)
pub fn spawn_dot_sparkles(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    // Use GlowCircle instead of SmallDots - SmallDots is a multi-dot atlas that doesn't work well
    let dot_texture = asset_server.load("textures/wfx/WFX_T_GlowCircle A8.png");

    info!("✨ WAR FX: Spawning dot sparkles at {:?} (75 particles)", position);

    let mut rng = rand::thread_rng();

    // Burst configuration: 75 particles total in 3 equal bursts
    let bursts = [
        (0.00_f32, 25_u32),
        (0.12_f32, 25_u32),
        (0.25_f32, 25_u32),
    ];

    // Small dot size - slightly larger than Unity for visibility
    let base_size = 0.2 * scale;
    let quad_mesh = meshes.add(Rectangle::new(base_size, base_size));

    for (delay, count) in bursts {
        for _ in 0..count {
            // Random direction - 25° cone (narrower than glow sparkles' 70°)
            // But allow full hemisphere spread for more visible effect
            let theta = rng.gen_range(0.0..std::f32::consts::TAU);
            let phi = rng.gen_range(0.0..(50.0_f32).to_radians()); // Wider than Unity's 25° for visibility
            let dir = Vec3::new(
                phi.sin() * theta.cos(),
                phi.cos(),
                phi.sin() * theta.sin(),
            ).normalize();

            // Random offset from center (Unity radius: 1.6)
            let spawn_offset = dir * rng.gen_range(0.0..1.6) * scale;

            // Random speed between 12-24 (creates varied spread)
            let speed = rng.gen_range(12.0..24.0) * scale;
            let initial_velocity = dir * speed;

            // Random lifetime (Unity: 0.2 to 0.3 seconds)
            let lifetime = rng.gen_range(0.2..0.3);

            // Random size multiplier
            let size_mult = rng.gen_range(0.5..1.0);

            // Size curve: grow from 40% to full size
            let scale_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 0.399 * size_mult * scale),
                    (0.347, 0.764 * size_mult * scale),
                    (1.0, 0.990 * size_mult * scale),
                ],
            };

            // Alpha curve: stay bright for 60%, then fade
            let alpha_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 1.0),
                    (0.6, 1.0),
                    (1.0, 0.0),
                ],
            };

            // Fire color gradient (same as glow sparkles)
            let color_curve = ColorCurve {
                keyframes: vec![
                    (0.0, Vec3::new(1.0, 1.0, 1.0)),           // White
                    (0.20, Vec3::new(1.0, 0.984, 0.843)),      // Warm white
                    (0.40, Vec3::new(1.0, 0.945, 0.471)),      // Yellow
                    (0.50, Vec3::new(1.0, 0.796, 0.420)),      // Orange
                    (0.75, Vec3::new(0.718, 0.196, 0.0)),      // Dark red
                ],
            };

            let sparkle_material = additive_materials.add(AdditiveMaterial {
                tint_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
                particle_texture: dot_texture.clone(),
            });

            commands.spawn((
                MaterialMeshBundle {
                    mesh: quad_mesh.clone(),
                    material: sparkle_material,
                    transform: Transform::from_translation(position + spawn_offset)
                        .with_scale(Vec3::splat(0.399 * size_mult)),
                    visibility: if delay == 0.0 { Visibility::Visible } else { Visibility::Hidden },
                    ..Default::default()
                },
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                WarFXExplosion {
                    lifetime: 0.0,
                    max_lifetime: lifetime,
                },
                WarFxFlame {
                    spawn_delay: delay,
                    active: delay == 0.0,
                },
                AnimatedSparkle {
                    scale_curve,
                    alpha_curve,
                    color_curve,
                    velocity: initial_velocity,
                    gravity: 2.0 * scale, // Lower gravity than glow sparkles (2 vs 4)
                },
                Name::new("WFX_DotSparkle"),
            ));
        }
    }

    info!("✅ WAR FX: Spawned 75 dot sparkles");
}

/// Spawns vertical dot sparkles emitter - upward-floating sparks
/// Unity emitter: "Dot Sparkles Vertical" - sparks that rise without gravity
///
/// Key characteristics:
/// - Slower speed: 6-12 units/sec (half of regular dot sparkles)
/// - Zero gravity: sparks float upward, don't fall
/// - Very short lifetime: 0.1-0.3 seconds
/// - 15 particles total in 3 small bursts (5×3)
/// - Same fire color gradient
/// - Primarily upward direction
pub fn spawn_dot_sparkles_vertical(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    // Use GlowCircle instead of SmallDots - SmallDots is a multi-dot atlas that doesn't work well
    let dot_texture = asset_server.load("textures/wfx/WFX_T_GlowCircle A8.png");

    info!("⬆️ WAR FX: Spawning vertical dot sparkles at {:?} (15 particles)", position);

    let mut rng = rand::thread_rng();

    // Burst configuration: 15 particles total in 3 small bursts
    let bursts = [
        (0.00_f32, 5_u32),
        (0.15_f32, 5_u32),
        (0.30_f32, 5_u32),
    ];

    // Small dot size - slightly larger than Unity for visibility
    let base_size = 0.2 * scale;
    let quad_mesh = meshes.add(Rectangle::new(base_size, base_size));

    for (delay, count) in bursts {
        for _ in 0..count {
            // Primarily upward direction with some X/Z variation
            // Approximates mesh emission with narrow arc
            let y = rng.gen_range(0.7..1.0); // Mostly up but with more spread
            let x = rng.gen_range(-0.3..0.3);
            let z = rng.gen_range(-0.3..0.3);
            let dir = Vec3::new(x, y, z).normalize();

            // Small random offset from center
            let spawn_offset = Vec3::new(
                rng.gen_range(-0.5..0.5) * scale,
                rng.gen_range(0.0..0.5) * scale,
                rng.gen_range(-0.5..0.5) * scale,
            );

            // Slower speed than regular dot sparkles (6-12 vs 12-24)
            let speed = rng.gen_range(6.0..12.0) * scale;
            let initial_velocity = dir * speed;

            // Very short lifetime (Unity: 0.1 to 0.3 seconds)
            let lifetime = rng.gen_range(0.1..0.3);

            // Random size multiplier
            let size_mult = rng.gen_range(0.5..1.0);

            // Size curve: grow from 40% to full size
            let scale_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 0.399 * size_mult * scale),
                    (0.347, 0.764 * size_mult * scale),
                    (1.0, 0.990 * size_mult * scale),
                ],
            };

            // Alpha curve: stay bright for 60%, then fade
            let alpha_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 1.0),
                    (0.6, 1.0),
                    (1.0, 0.0),
                ],
            };

            // Fire color gradient (same as other sparkles)
            let color_curve = ColorCurve {
                keyframes: vec![
                    (0.0, Vec3::new(1.0, 1.0, 1.0)),           // White
                    (0.20, Vec3::new(1.0, 0.984, 0.843)),      // Warm white
                    (0.40, Vec3::new(1.0, 0.945, 0.471)),      // Yellow
                    (0.50, Vec3::new(1.0, 0.796, 0.420)),      // Orange
                    (0.75, Vec3::new(0.718, 0.196, 0.0)),      // Dark red
                ],
            };

            let sparkle_material = additive_materials.add(AdditiveMaterial {
                tint_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
                particle_texture: dot_texture.clone(),
            });

            commands.spawn((
                MaterialMeshBundle {
                    mesh: quad_mesh.clone(),
                    material: sparkle_material,
                    transform: Transform::from_translation(position + spawn_offset)
                        .with_scale(Vec3::splat(0.399 * size_mult)),
                    visibility: if delay == 0.0 { Visibility::Visible } else { Visibility::Hidden },
                    ..Default::default()
                },
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                WarFXExplosion {
                    lifetime: 0.0,
                    max_lifetime: lifetime,
                },
                WarFxFlame {
                    spawn_delay: delay,
                    active: delay == 0.0,
                },
                AnimatedSparkle {
                    scale_curve,
                    alpha_curve,
                    color_curve,
                    velocity: initial_velocity,
                    gravity: 0.0, // No gravity - sparks float upward
                },
                Name::new("WFX_DotSparkleVertical"),
            ));
        }
    }

    info!("✅ WAR FX: Spawned 15 vertical dot sparkles");
}

/// Component for sparkle animation with gravity
#[derive(Component, Clone)]
pub struct AnimatedSparkle {
    pub scale_curve: AnimationCurve,
    pub alpha_curve: AnimationCurve,
    pub color_curve: ColorCurve,
    pub velocity: Vec3,
    pub gravity: f32,
}

/// System to animate glow sparkles (scale, color, velocity with gravity)
pub fn animate_glow_sparkles(
    mut query: Query<
        (
            &mut Transform,
            &mut AnimatedSparkle,
            &WarFXExplosion,
            &Handle<AdditiveMaterial>,
        ),
        With<AnimatedSparkle>,
    >,
    mut additive_materials: ResMut<Assets<AdditiveMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();

    for (mut transform, mut sparkle, explosion, material_handle) in query.iter_mut() {
        // Calculate progress through lifetime (0.0 to 1.0)
        let progress = (explosion.lifetime / explosion.max_lifetime).clamp(0.0, 1.0);

        // Evaluate scale curve
        let current_scale = sparkle.scale_curve.evaluate(progress);
        transform.scale = Vec3::splat(current_scale);

        // Evaluate alpha curve
        let current_alpha = sparkle.alpha_curve.evaluate(progress);

        // Evaluate color curve
        let current_color = sparkle.color_curve.evaluate(progress);

        // Update material tint color
        if let Some(material) = additive_materials.get_mut(material_handle) {
            material.tint_color = Vec4::new(
                current_color.x,
                current_color.y,
                current_color.z,
                current_alpha,
            );
        }

        // Apply gravity to velocity (accumulates over time)
        sparkle.velocity.y -= sparkle.gravity * dt;

        // Apply velocity to position
        transform.translation += sparkle.velocity * dt;
    }
}

/// Spawns a complete explosion effect combining all 6 emitters:
/// 1. Embedded glow (central flash)
/// 2. Explosion flames (fire/smoke billboards)
/// 3. Smoke emitter (lingering smoke trail, delayed 0.5s)
/// 4. Glow sparkles (fast-moving embers with gravity)
/// 5. Dot sparkles (dense shower of bright sparks)
/// 6. Dot sparkles vertical (upward-floating sparks)
///
/// This creates the full Unity WFX_ExplosiveSmoke_Big effect.
pub fn spawn_combined_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    smoke_scroll_materials: &mut ResMut<Assets<SmokeScrollMaterial>>,
    smoke_only_materials: &mut ResMut<Assets<SmokeOnlyMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
) {
    info!("💥 WAR FX: Spawning COMBINED explosion at {:?} (scale: {})", position, scale);

    // 1. Central glow flash (instant, 0.7s lifetime)
    spawn_warfx_center_glow(
        commands,
        meshes,
        additive_materials,
        asset_server,
        position,
        scale,
    );

    // 2. Explosion flames/smoke billboards (57 particles in bursts)
    spawn_explosion_flames(
        commands,
        meshes,
        smoke_scroll_materials,
        asset_server,
        position,
        scale,
    );

    // 3. Smoke emitter (30 particles, delayed 0.5s start)
    spawn_smoke_emitter(
        commands,
        meshes,
        smoke_only_materials,
        asset_server,
        position,
        scale,
    );

    // 4. Glow sparkles (35 fast-moving embers with gravity)
    spawn_glow_sparkles(
        commands,
        meshes,
        additive_materials,
        asset_server,
        position,
        scale,
    );

    // 5. Dot sparkles (75 dense sparks with moderate gravity)
    spawn_dot_sparkles(
        commands,
        meshes,
        additive_materials,
        asset_server,
        position,
        scale,
    );

    // 6. Dot sparkles vertical (15 upward-floating sparks)
    spawn_dot_sparkles_vertical(
        commands,
        meshes,
        additive_materials,
        asset_server,
        position,
        scale,
    );

    info!("✅ WAR FX: Combined explosion complete - 6 emitters spawned");
}

/// Spawns smoke emitter billboards using UV-scrolling smoke texture
/// Unity emitter: "Smoke" - creates lingering smoke trail after initial explosion
///
/// Key characteristics:
/// - Delayed start: 0.5s after explosion
/// - Continuous emission: 20 particles/sec over 1.5s (~30 particles total)
/// - Gray color: constant (0.725, 0.725, 0.725) not gradient
/// - Fade-in alpha: particles fade in, hold, then fade out
/// - Slow rotation: 70-90°/sec for organic look
pub fn spawn_smoke_emitter(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    smoke_only_materials: &mut ResMut<Assets<SmokeOnlyMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    base_scale: f32,
) {
    // Use same smoke texture with UV scrolling
    let smoke_texture = asset_server.load("textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    info!("💨 WAR FX: Spawning smoke emitter at {:?} (30 particles over 1.5s)", position);

    let mut rng = rand::thread_rng();

    // Continuous emission: 20 particles/sec for 1.5s = 30 particles
    // Spacing: 1.5s / 30 = 0.05s between particles
    // Plus 0.5s global start delay
    let particle_count = 30;

    // Gray start color (constant, not gradient like Explosion emitter)
    let start_color = Vec3::new(0.725, 0.725, 0.725);
    let start_alpha: f32 = 0.694;

    // Create shared quad mesh (Unity: start size 4-6)
    let quad_size = 5.0 * base_scale;
    let quad_mesh = meshes.add(create_quad_mesh(quad_size));

    for i in 0..particle_count {
        // Staggered spawn delay: 0.5s base + 50ms per particle
        let spawn_delay = 0.5 + (i as f32 * 0.05);

        // Random position within sphere (Unity: radius 1.6, angle 5°)
        let radius = 1.6 * base_scale;
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let spread = rng.gen_range(0.0..0.087); // ~5 degrees in radians
        let vertical_spread = rng.gen_range(-0.5..0.5) * spread;
        let offset = Vec3::new(
            radius * spread * angle.cos(),
            vertical_spread * radius,
            radius * spread * angle.sin(),
        );

        // Random lifetime (Unity: 3s to 4s)
        let lifetime = rng.gen_range(3.0..4.0);

        // Random initial rotation (Unity: 0° to 360°)
        let initial_rotation = rng.gen_range(0.0..std::f32::consts::TAU);

        // Random size multiplier (Unity: 4 to 6, normalized)
        let size_mult = rng.gen_range(0.8..1.2);

        // Alpha curve: fade-in, hold, fade-out (different from Explosion!)
        // Unity: 0%→0, 15%→1, 33%→1, 100%→0
        let alpha_curve = AnimationCurve {
            keyframes: vec![
                (0.0, 0.0),    // Start transparent (fade in)
                (0.15, 1.0),   // Fully opaque
                (0.33, 1.0),   // Stay opaque
                (1.0, 0.0),    // Fade out
            ],
        };

        // Size curve: gradual growth (gentler than Explosion's rapid pop)
        // Unity: 0%→0.414, 100%→1.0
        let scale_curve = AnimationCurve {
            keyframes: vec![
                (0.0, 0.414 * size_mult),
                (1.0, 1.0 * size_mult),
            ],
        };

        // Color stays constant gray (Unity: Color Over Lifetime is white, no change)
        // The gray comes from startColor (0.725) being multiplied
        let color_curve = ColorCurve::constant(start_color);

        // Velocity: Y increases over time (Unity curve: 1.5→3.0 by 20%)
        // Simplified to constant approximation
        let velocity = Vec3::new(
            rng.gen_range(-0.5..0.5),   // X: minimal spread
            rng.gen_range(2.0..3.0),    // Y: upward drift
            rng.gen_range(-0.5..0.5),   // Z: minimal spread
        ) * base_scale;

        // Rotation: 70-90°/sec (1.22-1.57 rad/s) - slower than Explosion
        let rotation_speed = rng.gen_range(1.22..1.57);

        // Create smoke-only material with gray color (no flame blending)
        // scroll_speed = 10.0 (from Unity .mat file)
        // W component packs scroll_speed (integer) + alpha (decimal)
        let scroll_speed = 10.0_f32;
        let packed_w = scroll_speed + start_alpha.min(0.999);
        let smoke_material = smoke_only_materials.add(SmokeOnlyMaterial {
            tint_color_and_speed: Vec4::new(
                start_color.x,
                start_color.y,
                start_color.z,
                packed_w,
            ),
            smoke_texture: smoke_texture.clone(),
        });

        // All particles start inactive (spawn_delay > 0)
        commands.spawn((
            MaterialMeshBundle {
                mesh: quad_mesh.clone(),
                material: smoke_material,
                transform: Transform::from_translation(position + offset)
                    .with_rotation(Quat::from_rotation_z(initial_rotation))
                    .with_scale(Vec3::ZERO), // Start at zero scale (hidden)
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            WarFXExplosion {
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            WarFxFlame {
                spawn_delay,
                active: false, // All start inactive
            },
            AnimatedSmokeOnlyBillboard {
                scale_curve,
                alpha_curve,
                color_curve,
                velocity,
                rotation_speed,
                base_rotation: initial_rotation,
            },
            Name::new(format!("WFX_Smoke_{}", i)),
        ));
    }

    info!("✅ WAR FX: Spawned {} smoke billboards (delayed start)", particle_count);
}

/// Spawns explosion effect billboards using UV-scrolling smoke texture
/// Unity emitter: "Explosion" - creates both flame bursts AND smoke from single system
///
/// Key insight: Uses randomized start colors from gradient (orange→brown)
/// - Orange particles look like flames initially, then gray out
/// - Brown particles look like smoke from the start
///
/// Total: 38 particles in 4 staggered bursts (15+10+8+5)
pub fn spawn_explosion_flames(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    smoke_materials: &mut ResMut<Assets<SmokeScrollMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    base_scale: f32,
) {
    // Use smoke texture with UV scrolling (creates morphing appearance)
    let smoke_texture = asset_server.load("textures/wfx/WFX_T_SmokeLoopAlpha.tga");

    info!("🔥 WAR FX: Spawning explosion at {:?} (57 particles in 4 bursts)", position);

    let mut rng = rand::thread_rng();

    // Burst configuration: 57 particles total (1.5x Unity's 38)
    // Front-loaded to create initial impact
    let bursts = [
        (0.00_f32, 23_u32),  // 15 * 1.5
        (0.10_f32, 15_u32),  // 10 * 1.5
        (0.20_f32, 12_u32),  // 8 * 1.5
        (0.30_f32, 7_u32),   // 5 * 1.5
    ];

    // Start color gradient from Unity (particles spawn with random color from this gradient)
    // 0%: rgb(1.0, 0.922, 0.827) - Cream/pale orange
    // 9%: rgb(1.0, 0.522, 0.2)   - Bright orange (FLAME)
    // 42%: rgb(0.663, 0.235, 0.184) - Dark red/brown (SMOKE)
    // 100%: rgb(0.996, 0.741, 0.498) - Tan
    let start_colors = [
        Vec3::new(1.0, 0.922, 0.827),   // Cream
        Vec3::new(1.0, 0.522, 0.2),     // Bright orange
        Vec3::new(0.663, 0.235, 0.184), // Dark brown
        Vec3::new(0.996, 0.741, 0.498), // Tan
    ];

    // Create shared quad mesh with explicit UVs (Unity: start size 4-6)
    let quad_size = 5.0 * base_scale;
    let quad_mesh = meshes.add(create_quad_mesh(quad_size));

    let mut total_spawned = 0;

    for (delay, count) in bursts {
        for i in 0..count {
            // Random position on sphere surface - full spherical distribution
            // Use spherical coordinates to scatter particles around glow center
            let radius = rng.gen_range(1.5..3.5) * base_scale;
            let theta = rng.gen_range(0.0..std::f32::consts::TAU); // Horizontal angle (0 to 360°)
            let phi = rng.gen_range(0.3..std::f32::consts::PI); // Vertical angle (covers top to bottom)
            let offset = Vec3::new(
                radius * phi.sin() * theta.cos(),
                radius * phi.cos(), // Y from cos(phi): +1 at top, -1 at bottom
                radius * phi.sin() * theta.sin(),
            );

            // Random lifetime (Unity: 3s to 4s)
            let lifetime = rng.gen_range(3.0..4.0);

            // Random initial rotation (Unity: 0° to 360°)
            let initial_rotation = rng.gen_range(0.0..std::f32::consts::TAU);

            // Random start size multiplier (Unity: 4 to 6, normalized)
            let size_mult = rng.gen_range(0.8..1.2);

            // Random start color from gradient (simulating Unity's gradient sampling)
            // Weight toward orange (flame) for visual impact
            let color_index = rng.gen_range(0..4);
            let start_color = start_colors[color_index];

            // Create smoke scroll material with random start color
            // scroll_speed = 10.0 (from Unity .mat file)
            // W component packs scroll_speed (integer) + alpha (decimal): 10.0 + 0.999 = 10.999
            // Initial alpha = 0.999 (nearly 1.0, clamped to avoid floor collision)
            let scroll_speed = 10.0_f32;
            let initial_alpha = 0.999_f32; // Start fully opaque
            let packed_w = scroll_speed + initial_alpha;
            let smoke_material = smoke_materials.add(SmokeScrollMaterial {
                tint_color_and_speed: Vec4::new(
                    start_color.x,
                    start_color.y,
                    start_color.z,
                    packed_w,
                ),
                smoke_texture: smoke_texture.clone(),
            });

            // Unity Size Over Lifetime curve (rapid "pop" effect):
            // t=0.0: 0.396, t=0.046: 0.814 (rapid expand), t=1.0: 1.0 (slow grow)
            let scale_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 0.396 * size_mult),
                    (0.046, 0.814 * size_mult),  // Rapid pop in first 5%
                    (1.0, 1.0 * size_mult),
                ],
            };

            // Alpha curve from Unity: 100%→100%→57.6%→0%
            let alpha_curve = AnimationCurve {
                keyframes: vec![
                    (0.0, 1.0),    // Full opacity
                    (0.3, 1.0),    // Stay opaque
                    (0.6, 0.576),  // Start fading
                    (1.0, 0.0),    // Fully transparent
                ],
            };

            // Color over lifetime from Unity: grayscale multiplier applied to start color
            // Unity ColorOverLifetime multiplies the start color by these grayscale values:
            // t=0%: 1.0 (full brightness), t=20%: 0.694, t=41%: 0.404, t=100%: 0.596
            // This maintains the color hue while transitioning to a darker shade
            let color_curve = ColorCurve {
                keyframes: vec![
                    (0.0, start_color),                        // Full brightness
                    (0.2, start_color * 0.694),                // 69.4% brightness
                    (0.41, start_color * 0.404),               // 40.4% brightness
                    (1.0, start_color * 0.596),                // 59.6% brightness (keeps hue)
                ],
            };

            // Velocity (Unity: X:4, Y:8, Z:4 - upward bias)
            let velocity = Vec3::new(
                rng.gen_range(-4.0..4.0),   // X: random spread
                rng.gen_range(4.0..8.0),    // Y: upward (Unity: 8)
                rng.gen_range(-4.0..4.0),   // Z: random spread
            ) * base_scale * 0.5;  // Scale down velocity for longer lifetime

            // Rotation speed (Unity: 90°/sec = 1.5707963 rad/s)
            let rotation_speed = rng.gen_range(-1.57..1.57);

            // Determine if this particle is active immediately or delayed
            let is_active = delay == 0.0;

            commands.spawn((
                MaterialMeshBundle {
                    mesh: quad_mesh.clone(),
                    material: smoke_material,
                    transform: Transform::from_translation(position + offset)
                        .with_rotation(Quat::from_rotation_z(initial_rotation))
                        .with_scale(if is_active { Vec3::splat(0.396 * size_mult) } else { Vec3::ZERO }),
                    visibility: if is_active { Visibility::Visible } else { Visibility::Hidden },
                    ..Default::default()
                },
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                WarFXExplosion {
                    lifetime: 0.0,
                    max_lifetime: lifetime,
                },
                WarFxFlame {
                    spawn_delay: delay,
                    active: is_active,
                },
                AnimatedExplosionBillboard {
                    scale_curve,
                    alpha_curve,
                    color_curve,
                    velocity,
                    rotation_speed,
                    base_rotation: initial_rotation,
                },
                Name::new(format!("WFX_Explosion_{}_{}", delay as i32 * 100, i)),
            ));

            total_spawned += 1;
        }
    }

    info!("✅ WAR FX: Spawned {} explosion billboards", total_spawned);
}

/// Component for explosion billboard animation (for SmokeScrollMaterial)
/// Handles scale, alpha, and color curves plus velocity/rotation
#[derive(Component, Clone)]
pub struct AnimatedExplosionBillboard {
    pub scale_curve: AnimationCurve,
    pub alpha_curve: AnimationCurve,
    pub color_curve: ColorCurve,
    pub velocity: Vec3,
    pub rotation_speed: f32,
    pub base_rotation: f32,
}

/// Component for smoke-only billboard animation (for SmokeOnlyMaterial)
/// Same structure as AnimatedExplosionBillboard but uses different material type
#[derive(Component, Clone)]
pub struct AnimatedSmokeOnlyBillboard {
    pub scale_curve: AnimationCurve,
    pub alpha_curve: AnimationCurve,
    pub color_curve: ColorCurve,
    pub velocity: Vec3,
    pub rotation_speed: f32,
    pub base_rotation: f32,
}

/// System to animate smoke-only billboards (scale, alpha, velocity, rotation) over lifetime
/// Uses SmokeOnlyMaterial for pure gray smoke without flame blending
pub fn animate_smoke_only_billboards(
    mut query: Query<
        (
            &mut Transform,
            &mut AnimatedSmokeOnlyBillboard,
            &WarFXExplosion,
            &Handle<SmokeOnlyMaterial>,
        ),
        With<AnimatedSmokeOnlyBillboard>,
    >,
    mut smoke_materials: ResMut<Assets<SmokeOnlyMaterial>>,
    time: Res<Time>,
) {
    for (mut transform, mut billboard, explosion, material_handle) in query.iter_mut() {
        // Calculate progress through lifetime (0.0 to 1.0)
        let progress = (explosion.lifetime / explosion.max_lifetime).clamp(0.0, 1.0);

        // Evaluate scale curve (gradual growth for smoke)
        let current_scale = billboard.scale_curve.evaluate(progress);
        transform.scale = Vec3::splat(current_scale);

        // Evaluate alpha curve (fade-in, hold, fade-out)
        let current_alpha = billboard.alpha_curve.evaluate(progress);

        // Evaluate color curve (stays constant gray for smoke)
        let current_color = billboard.color_curve.evaluate(progress);

        // Update material tint color (RGB) and pack alpha with scroll_speed (W)
        if let Some(material) = smoke_materials.get_mut(material_handle) {
            // Extract scroll_speed from packed value (integer part)
            let scroll_speed = material.tint_color_and_speed.w.floor();
            // Pack scroll_speed (integer) + alpha (decimal) into w component
            let packed_w = scroll_speed + current_alpha.clamp(0.0, 0.999);
            material.tint_color_and_speed = Vec4::new(
                current_color.x,
                current_color.y,
                current_color.z,
                packed_w,
            );
        }

        // Apply velocity (smoke drifts upward)
        transform.translation += billboard.velocity * time.delta_seconds();

        // Apply rotation animation
        billboard.base_rotation += billboard.rotation_speed * time.delta_seconds();
    }
}

/// System to animate explosion billboards (scale, color, velocity, rotation) over lifetime
pub fn animate_explosion_billboards(
    mut query: Query<
        (
            &mut Transform,
            &mut AnimatedExplosionBillboard,
            &WarFXExplosion,
            &Handle<SmokeScrollMaterial>,
        ),
        With<AnimatedExplosionBillboard>,
    >,
    mut smoke_materials: ResMut<Assets<SmokeScrollMaterial>>,
    time: Res<Time>,
) {
    for (mut transform, mut billboard, explosion, material_handle) in query.iter_mut() {
        // Calculate progress through lifetime (0.0 to 1.0)
        let progress = (explosion.lifetime / explosion.max_lifetime).clamp(0.0, 1.0);

        // Evaluate scale curve (rapid "pop" effect)
        let current_scale = billboard.scale_curve.evaluate(progress);
        transform.scale = Vec3::splat(current_scale);

        // Evaluate alpha and color curves
        let current_alpha = billboard.alpha_curve.evaluate(progress);
        let current_color = billboard.color_curve.evaluate(progress);

        // Update material tint color (RGB) and pack alpha with scroll_speed (W)
        // SmokeScrollMaterial: (R, G, B, packed_scroll_alpha)
        // Packed format: scroll_speed.floor() + alpha (where alpha is 0.0-1.0)
        // This avoids premultiplying alpha into color which darkens flames prematurely
        if let Some(material) = smoke_materials.get_mut(material_handle) {
            // Extract scroll_speed from packed value (integer part)
            let scroll_speed = material.tint_color_and_speed.w.floor();
            // Pack scroll_speed (integer) + alpha (decimal) into w component
            let packed_w = scroll_speed + current_alpha.clamp(0.0, 0.999);
            material.tint_color_and_speed = Vec4::new(
                current_color.x,
                current_color.y,
                current_color.z,
                packed_w,
            );
        }

        // Apply velocity (particles drift upward and outward)
        transform.translation += billboard.velocity * time.delta_seconds();

        // Apply rotation animation
        billboard.base_rotation += billboard.rotation_speed * time.delta_seconds();
    }
}

/// Component to track War FX explosion lifetime
#[derive(Component)]
pub struct WarFXExplosion {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Component for flame particles with spawn delay support
/// Used for staggered burst spawning (Unity burst timing: t=0s, t=0.1s, t=0.2s, t=0.3s)
#[derive(Component)]
pub struct WarFxFlame {
    pub spawn_delay: f32,    // Seconds until this flame activates
    pub active: bool,        // Whether delay has elapsed
}

/// Animation curve with keyframes for non-linear interpolation
#[derive(Clone)]
pub struct AnimationCurve {
    pub keyframes: Vec<(f32, f32)>, // (time, value) pairs
}

impl AnimationCurve {
    /// Evaluate curve at given time (0.0 to 1.0)
    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        if self.keyframes.len() == 1 {
            return self.keyframes[0].1;
        }

        let t = t.clamp(0.0, 1.0);

        // Find the two keyframes to interpolate between
        for i in 0..self.keyframes.len() - 1 {
            let (t0, v0) = self.keyframes[i];
            let (t1, v1) = self.keyframes[i + 1];

            if t >= t0 && t <= t1 {
                // Linear interpolation between keyframes
                let local_t = (t - t0) / (t1 - t0);
                return v0 + (v1 - v0) * local_t;
            }
        }

        // If we're past the last keyframe, return last value
        self.keyframes.last().unwrap().1
    }

    /// Create a simple linear curve (for backward compatibility)
    pub fn linear(start: f32, end: f32) -> Self {
        Self {
            keyframes: vec![(0.0, start), (1.0, end)],
        }
    }
}

/// Color curve with RGB keyframes for color transitions over lifetime
#[derive(Clone)]
pub struct ColorCurve {
    pub keyframes: Vec<(f32, Vec3)>, // (time, RGB) pairs
}

impl ColorCurve {
    /// Evaluate curve at given time (0.0 to 1.0) and return RGB color
    pub fn evaluate(&self, t: f32) -> Vec3 {
        if self.keyframes.is_empty() {
            return Vec3::ONE; // Default to white
        }
        if self.keyframes.len() == 1 {
            return self.keyframes[0].1;
        }

        let t = t.clamp(0.0, 1.0);

        // Find the two keyframes to interpolate between
        for i in 0..self.keyframes.len() - 1 {
            let (t0, c0) = self.keyframes[i];
            let (t1, c1) = self.keyframes[i + 1];

            if t >= t0 && t <= t1 {
                // Linear interpolation between color keyframes
                let local_t = (t - t0) / (t1 - t0);
                return c0 + (c1 - c0) * local_t;
            }
        }

        // If we're past the last keyframe, return last color
        self.keyframes.last().unwrap().1
    }

    /// Create a constant color curve
    pub fn constant(color: Vec3) -> Self {
        Self {
            keyframes: vec![(0.0, color), (1.0, color)],
        }
    }
}

/// Component for billboard animation over lifetime (for AdditiveMaterial)
#[derive(Component, Clone)]
pub struct AnimatedBillboard {
    pub scale_curve: AnimationCurve,
    pub alpha_curve: AnimationCurve,
    pub color_curve: ColorCurve,
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
/// Note: Skips inactive flames (those with spawn_delay > 0) to prevent premature lifetime expiration
pub fn update_warfx_explosions(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut WarFXExplosion, &Name, Option<&WarFxFlame>)>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    time: Res<Time>,
) {
    // Get camera position for billboarding
    let camera_position = if let Ok(camera_transform) = camera_query.get_single() {
        camera_transform.translation()
    } else {
        return; // No camera, can't billboard
    };

    for (entity, mut transform, mut explosion, name, maybe_flame) in query.iter_mut() {
        // Skip inactive flames - they shouldn't update lifetime until activated
        // This prevents delayed flames from expiring before they even appear
        if let Some(flame) = maybe_flame {
            if !flame.active {
                continue;
            }
        }

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

        // Despawn after lifetime completes
        if explosion.lifetime >= explosion.max_lifetime {
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

        // Evaluate scale curve
        let current_scale = billboard.scale_curve.evaluate(progress);
        transform.scale = Vec3::splat(current_scale);

        // Evaluate alpha curve
        let current_alpha = billboard.alpha_curve.evaluate(progress);

        // Evaluate color curve
        let current_color = billboard.color_curve.evaluate(progress);

        // Update material tint color with new RGB and alpha
        if let Some(material) = additive_materials.get_mut(material_handle) {
            material.tint_color = Vec4::new(
                current_color.x,
                current_color.y,
                current_color.z,
                current_alpha,
            );
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

/// System to handle spawn delay for explosion flames
/// Flames with spawn_delay > 0 are hidden until their delay elapses
/// Once active, the animate_warfx_billboards system handles animation
pub fn animate_explosion_flames(
    mut query: Query<(
        &mut WarFxFlame,
        &mut WarFXExplosion,
        &mut Transform,
        &mut Visibility,
    )>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();

    for (mut flame, mut explosion, mut transform, mut visibility) in query.iter_mut() {
        // Handle spawn delay for inactive flames
        if !flame.active {
            flame.spawn_delay -= delta;

            if flame.spawn_delay <= 0.0 {
                // Activate this flame
                flame.active = true;
                *visibility = Visibility::Visible;
                // Reset explosion lifetime to start animation from beginning
                explosion.lifetime = 0.0;
            }
            // Keep hidden and don't update lifetime while inactive
            continue;
        }

        // Active flames: update lifetime (animation handled by animate_warfx_billboards)
        // The WarFXExplosion lifetime is updated by update_warfx_explosions system
    }
}
