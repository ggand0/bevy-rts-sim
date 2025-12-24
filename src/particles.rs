// Particle effects system using Bevy Hanabi
// Provides debris, sparks, and smoke particles for explosions
use bevy::prelude::*;
use bevy_hanabi::prelude::*;

pub struct ParticleEffectsPlugin;

impl Plugin for ParticleEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_systems(Startup, (setup_particle_effects, debug_orientation_math))
            .add_systems(Update, (cleanup_finished_particle_effects, debug_hanabi_entities));
    }
}

/// Debug system to verify orientation math by computing values for test velocities
fn debug_orientation_math() {
    info!("═══════════════════════════════════════════════════════════════");
    info!("  GPU ORIENTATION MATH DEBUG");
    info!("═══════════════════════════════════════════════════════════════");

    // Test velocities - opposite directions
    let test_velocities = [
        Vec3::new(1.0, 0.0, 0.0),   // +X
        Vec3::new(-1.0, 0.0, 0.0),  // -X
        Vec3::new(0.0, 1.0, 0.0),   // +Y
        Vec3::new(0.0, -1.0, 0.0),  // -Y
        Vec3::new(0.7, 0.7, 0.0),   // +X+Y diagonal
        Vec3::new(-0.7, 0.7, 0.0),  // -X+Y diagonal
    ];

    // Simulate camera at origin looking down -Z
    let camera_pos = Vec3::new(0.0, 5.0, 10.0);
    let particle_pos = Vec3::ZERO;

    info!("Camera position: {:?}", camera_pos);
    info!("Particle position: {:?}", particle_pos);
    info!("");

    // Test 1: Original X=velocity (bevy_hanabi AlongVelocity)
    info!("--- ORIGINAL X=velocity (no mirroring expected) ---");
    for vel in &test_velocities {
        let velocity = vel.normalize();
        let dir = (particle_pos - camera_pos).normalize(); // dir away from camera

        let axis_x = velocity;
        let axis_y = dir.cross(axis_x);
        let axis_z = axis_x.cross(axis_y);

        info!("vel={:+.2?} → axis_x={:+.3?}, axis_y={:+.3?}, axis_z={:+.3?}",
              velocity, axis_x, axis_y, axis_z);
    }
    info!("");

    // Test 2: Quaternion rotation from (0,1,0) to velocity
    info!("--- QUATERNION Y→velocity (mirroring expected) ---");
    for vel in &test_velocities {
        let velocity = vel.normalize();
        let from = Vec3::Y;

        // Half-angle quaternion formula
        let d = from.dot(velocity);
        let c = from.cross(velocity);

        let quat = if d < -0.999 {
            // Anti-parallel case
            Quat::from_xyzw(1.0, 0.0, 0.0, 0.0)
        } else {
            Quat::from_xyzw(c.x, c.y, c.z, 1.0 + d).normalize()
        };

        // Apply quaternion to default basis
        let axis_x = quat * Vec3::X;
        let axis_y = quat * Vec3::Y;
        let axis_z = quat * Vec3::Z;

        info!("vel={:+.2?} → axis_x={:+.3?}, axis_y={:+.3?}, axis_z={:+.3?}",
              velocity, axis_x, axis_y, axis_z);
    }
    info!("");

    // Test 3: X=velocity with -90° rotation (should give Y=velocity)
    info!("--- X=velocity + (-90° rotation) ---");
    for vel in &test_velocities {
        let velocity = vel.normalize();
        let dir = (particle_pos - camera_pos).normalize();

        // Original X=velocity
        let axis_x0 = velocity;
        let axis_y0 = dir.cross(axis_x0);
        let axis_z0 = axis_x0.cross(axis_y0);

        // Apply -90° rotation in X-Y plane
        let rot = -std::f32::consts::FRAC_PI_2;
        let cos_rot = rot.cos();
        let sin_rot = rot.sin();

        let axis_x = axis_x0 * cos_rot + axis_y0 * sin_rot;
        let axis_y = axis_y0 * cos_rot - axis_x0 * sin_rot;
        let axis_z = axis_z0;

        info!("vel={:+.2?} → axis_x={:+.3?}, axis_y={:+.3?}, axis_z={:+.3?}",
              velocity, axis_x, axis_y, axis_z);
    }

    info!("═══════════════════════════════════════════════════════════════");
}

// Component to mark particle effects for despawn after a fixed duration
// Uses simple spawn time tracking instead of per-frame timer ticking
#[derive(Component)]
pub struct ParticleEffectLifetime {
    pub spawn_time: f64,
    pub duration: f32,
}

// Resource to store particle effect templates
#[derive(Resource)]
pub struct ExplosionParticleEffects {
    pub debris_effect: Handle<EffectAsset>,
    pub sparks_effect: Handle<EffectAsset>,
    pub smoke_effect: Handle<EffectAsset>,
    #[allow(dead_code)]
    pub shield_impact_effect: Handle<EffectAsset>,
    pub mass_explosion_effect: Handle<EffectAsset>,
    #[allow(dead_code)]
    pub unit_death_flash: Handle<EffectAsset>,
    // Ground explosion GPU effects (replaces CPU spark entities)
    pub ground_sparks_effect: Handle<EffectAsset>,
    pub ground_flash_sparks_effect: Handle<EffectAsset>,
    pub ground_sparks_texture: Handle<Image>,
    // Ground explosion GPU parts debris (replaces CPU mesh entities)
    pub ground_parts_effect: Handle<EffectAsset>,
    pub ground_parts_texture: Handle<Image>,
    // Ground explosion GPU dirt debris (replaces CPU dirt entities)
    pub ground_dirt_effect: Handle<EffectAsset>,
    pub ground_vdirt_effect: Handle<EffectAsset>,
    pub ground_dirt_texture: Handle<Image>,
    // Ground explosion GPU fireballs (replaces CPU fireball entities)
    pub ground_fireball_effect: Handle<EffectAsset>,
    pub ground_fireball_texture: Handle<Image>,
    pub ground_fireball_secondary_texture: Handle<Image>,
    // Debug: simple colored quads to visualize velocity direction
    pub debug_fireball_effect: Handle<EffectAsset>,
    // Ground explosion GPU dust ring (replaces CPU dust entities)
    pub ground_dust_effect: Handle<EffectAsset>,
    pub ground_dust_texture: Handle<Image>,
    // Ground explosion GPU smoke cloud (replaces CPU smoke entities)
    pub ground_smoke_effect: Handle<EffectAsset>,
    pub ground_smoke_texture: Handle<Image>,
    // Ground explosion GPU wisp puffs (replaces CPU wisp entities)
    pub ground_wisp_effect: Handle<EffectAsset>,
    pub ground_wisp_texture: Handle<Image>,
}

fn setup_particle_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    asset_server: Res<AssetServer>,
) {
    info!("🎆 Setting up particle effects...");

    // Load flare texture for ground explosion sparks
    let ground_sparks_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/flare.png");
    let ground_dust_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/dust_4x1.png");
    let ground_smoke_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/smoke_8x8.png");
    let ground_wisp_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/wisp_8x8.png");

    // === DEBRIS PARTICLES ===
    // Physical debris chunks that fly outward
    let mut color_gradient1 = bevy_hanabi::Gradient::new();
    color_gradient1.add_key(0.0, Vec4::new(1.0, 0.5, 0.2, 1.0)); // Bright orange
    color_gradient1.add_key(0.3, Vec4::new(0.8, 0.3, 0.1, 1.0)); // Dark orange
    color_gradient1.add_key(0.6, Vec4::new(0.3, 0.3, 0.3, 0.8)); // Gray
    color_gradient1.add_key(1.0, Vec4::new(0.1, 0.1, 0.1, 0.0)); // Fade to black

    let mut size_gradient1 = bevy_hanabi::Gradient::new();
    size_gradient1.add_key(0.0, Vec3::splat(0.5));
    size_gradient1.add_key(1.0, Vec3::splat(0.3));

    let writer = ExprWriter::new();

    // Debris: burst of chunks flying outward
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(1.0).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(20.0).uniform(writer.lit(30.0)).expr(),
    };

    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer.lit(2.5).expr());
    let init_size = SetAttributeModifier::new(Attribute::SIZE, writer.lit(0.5).expr());

    let update_accel = AccelModifier::new(writer.lit(Vec3::new(0.0, -15.0, 0.0)).expr());
    let update_drag = LinearDragModifier::new(writer.lit(2.0).expr());

    let debris_module = writer.finish();

    let debris_effect = effects.add(
        EffectAsset::new(64, SpawnerSettings::once(5.0.into()), debris_module)
            .with_name("explosion_debris")
            .init(init_pos)
            .init(init_vel)
            .init(init_age)
            .init(init_lifetime)
            .init(init_size)
            .update(update_accel)
            .update(update_drag)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))
            .render(ColorOverLifetimeModifier::new(color_gradient1))
            .render(SizeOverLifetimeModifier { gradient: size_gradient1, screen_space_size: false })
    );

    // === SPARK PARTICLES ===
    // Bright, fast-moving sparks
    let mut color_gradient2 = bevy_hanabi::Gradient::new();
    color_gradient2.add_key(0.0, Vec4::new(1.0, 1.0, 0.8, 1.0)); // Bright yellow-white
    color_gradient2.add_key(0.1, Vec4::new(1.0, 0.8, 0.3, 1.0)); // Yellow
    color_gradient2.add_key(0.3, Vec4::new(1.0, 0.4, 0.1, 0.8)); // Orange
    color_gradient2.add_key(1.0, Vec4::new(0.5, 0.1, 0.0, 0.0)); // Fade out

    let mut size_gradient2 = bevy_hanabi::Gradient::new();
    size_gradient2.add_key(0.0, Vec3::splat(0.2));
    size_gradient2.add_key(1.0, Vec3::splat(0.05));

    let writer2 = ExprWriter::new();

    let init_pos2 = SetPositionSphereModifier {
        center: writer2.lit(Vec3::ZERO).expr(),
        radius: writer2.lit(0.5).expr(),
        dimension: ShapeDimension::Surface,
    };

    let init_vel2 = SetVelocitySphereModifier {
        center: writer2.lit(Vec3::ZERO).expr(),
        speed: writer2.lit(35.0).uniform(writer2.lit(50.0)).expr(),
    };

    let init_age2 = SetAttributeModifier::new(Attribute::AGE, writer2.lit(0.0).expr());
    let init_lifetime2 = SetAttributeModifier::new(Attribute::LIFETIME, writer2.lit(1.5).expr());
    let init_size2 = SetAttributeModifier::new(Attribute::SIZE, writer2.lit(0.2).expr());

    let update_accel2 = AccelModifier::new(writer2.lit(Vec3::new(0.0, -20.0, 0.0)).expr());
    let update_drag2 = LinearDragModifier::new(writer2.lit(3.0).expr());

    let sparks_module = writer2.finish();

    let sparks_effect = effects.add(
        EffectAsset::new(64, SpawnerSettings::once(5.0.into()), sparks_module)
            .with_name("explosion_sparks")
            .init(init_pos2)
            .init(init_vel2)
            .init(init_age2)
            .init(init_lifetime2)
            .init(init_size2)
            .update(update_accel2)
            .update(update_drag2)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))
            .render(ColorOverLifetimeModifier::new(color_gradient2))
            .render(SizeOverLifetimeModifier { gradient: size_gradient2, screen_space_size: false })
    );

    // === SMOKE PARTICLES ===
    // Rising smoke plumes
    let mut color_gradient3 = bevy_hanabi::Gradient::new();
    color_gradient3.add_key(0.0, Vec4::new(0.3, 0.3, 0.3, 0.0)); // Start transparent
    color_gradient3.add_key(0.2, Vec4::new(0.4, 0.4, 0.4, 0.6)); // Fade in
    color_gradient3.add_key(0.5, Vec4::new(0.35, 0.35, 0.35, 0.5)); // Peak
    color_gradient3.add_key(1.0, Vec4::new(0.2, 0.2, 0.2, 0.0)); // Fade out

    let mut size_gradient3 = bevy_hanabi::Gradient::new();
    size_gradient3.add_key(0.0, Vec3::splat(2.0));
    size_gradient3.add_key(1.0, Vec3::splat(4.0)); // Expand over lifetime

    let writer3 = ExprWriter::new();

    let init_pos3 = SetPositionSphereModifier {
        center: writer3.lit(Vec3::ZERO).expr(),
        radius: writer3.lit(2.0).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel3 = SetVelocitySphereModifier {
        center: writer3.lit(Vec3::ZERO).expr(),
        speed: writer3.lit(5.0).uniform(writer3.lit(10.0)).expr(),
    };

    let init_age3 = SetAttributeModifier::new(Attribute::AGE, writer3.lit(0.0).expr());
    let init_lifetime3 = SetAttributeModifier::new(Attribute::LIFETIME, writer3.lit(3.5).expr());
    let init_size3 = SetAttributeModifier::new(Attribute::SIZE, writer3.lit(2.0).expr());

    let update_accel3 = AccelModifier::new(writer3.lit(Vec3::new(0.0, 3.0, 0.0)).expr());
    let update_drag3 = LinearDragModifier::new(writer3.lit(1.0).expr());

    let smoke_module = writer3.finish();

    let smoke_effect = effects.add(
        EffectAsset::new(128, SpawnerSettings::once(50.0.into()), smoke_module)
            .with_name("explosion_smoke")
            .init(init_pos3)
            .init(init_vel3)
            .init(init_age3)
            .init(init_lifetime3)
            .init(init_size3)
            .update(update_accel3)
            .update(update_drag3)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))
            .render(ColorOverLifetimeModifier::new(color_gradient3))
            .render(SizeOverLifetimeModifier { gradient: size_gradient3, screen_space_size: false })
    );

    // === SHIELD IMPACT PARTICLES ===
    // Small, fast burst of energy particles for shield impacts
    let mut color_gradient4 = bevy_hanabi::Gradient::new();
    color_gradient4.add_key(0.0, Vec4::new(0.8, 1.0, 1.0, 1.0)); // Bright white-cyan
    color_gradient4.add_key(0.2, Vec4::new(0.5, 0.8, 1.0, 0.9)); // Bright cyan
    color_gradient4.add_key(0.5, Vec4::new(0.3, 0.6, 1.0, 0.6)); // Cyan
    color_gradient4.add_key(1.0, Vec4::new(0.2, 0.4, 0.8, 0.0)); // Fade to blue

    let mut size_gradient4 = bevy_hanabi::Gradient::new();
    size_gradient4.add_key(0.0, Vec3::splat(0.6));
    size_gradient4.add_key(0.3, Vec3::splat(0.8));
    size_gradient4.add_key(1.0, Vec3::splat(0.2));

    let writer4 = ExprWriter::new();

    let init_pos4 = SetPositionSphereModifier {
        center: writer4.lit(Vec3::ZERO).expr(),
        radius: writer4.lit(0.5).expr(),
        dimension: ShapeDimension::Surface,
    };

    let init_vel4 = SetVelocitySphereModifier {
        center: writer4.lit(Vec3::ZERO).expr(),
        speed: writer4.lit(12.0).uniform(writer4.lit(20.0)).expr(),
    };

    let init_age4 = SetAttributeModifier::new(Attribute::AGE, writer4.lit(0.0).expr());
    let init_lifetime4 = SetAttributeModifier::new(Attribute::LIFETIME, writer4.lit(0.8).expr());
    let init_size4 = SetAttributeModifier::new(Attribute::SIZE, writer4.lit(0.6).expr());

    let update_drag4 = LinearDragModifier::new(writer4.lit(3.0).expr());

    let shield_impact_module = writer4.finish();

    let shield_impact_effect = effects.add(
        EffectAsset::new(64, SpawnerSettings::once(40.0.into()), shield_impact_module)
            .with_name("shield_impact")
            .init(init_pos4)
            .init(init_vel4)
            .init(init_age4)
            .init(init_lifetime4)
            .init(init_size4)
            .update(update_drag4)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))
            .render(ColorOverLifetimeModifier::new(color_gradient4))
            .render(SizeOverLifetimeModifier { gradient: size_gradient4, screen_space_size: false })
    );

    // === MASS EXPLOSION EFFECT ===
    // Single effect that spawns particles across large radius (for tower destruction)
    // Replaces 1000+ individual explosion entities with ONE effect
    let mut color_gradient5 = bevy_hanabi::Gradient::new();
    color_gradient5.add_key(0.0, Vec4::new(1.0, 1.0, 0.9, 1.0)); // Bright white-yellow
    color_gradient5.add_key(0.1, Vec4::new(1.0, 0.8, 0.3, 1.0)); // Yellow
    color_gradient5.add_key(0.3, Vec4::new(1.0, 0.5, 0.1, 0.9)); // Orange
    color_gradient5.add_key(0.6, Vec4::new(0.8, 0.2, 0.0, 0.6)); // Red-orange
    color_gradient5.add_key(1.0, Vec4::new(0.3, 0.1, 0.0, 0.0)); // Fade out

    let mut size_gradient5 = bevy_hanabi::Gradient::new();
    size_gradient5.add_key(0.0, Vec3::splat(0.8));
    size_gradient5.add_key(0.2, Vec3::splat(1.5));
    size_gradient5.add_key(1.0, Vec3::splat(0.3));

    let writer5 = ExprWriter::new();

    // Spawn particles across TOWER_DESTRUCTION_RADIUS (80 units)
    let init_pos5 = SetPositionSphereModifier {
        center: writer5.lit(Vec3::ZERO).expr(),
        radius: writer5.lit(crate::constants::TOWER_DESTRUCTION_RADIUS).expr(),
        dimension: ShapeDimension::Volume,
    };

    // Particles fly upward and outward
    let init_vel5 = SetVelocitySphereModifier {
        center: writer5.lit(Vec3::ZERO).expr(),
        speed: writer5.lit(15.0).uniform(writer5.lit(35.0)).expr(),
    };

    let init_age5 = SetAttributeModifier::new(Attribute::AGE, writer5.lit(0.0).expr());
    // Random lifetime for staggered fade-out
    let init_lifetime5 = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer5.lit(1.5).uniform(writer5.lit(3.0)).expr()
    );
    let init_size5 = SetAttributeModifier::new(
        Attribute::SIZE,
        writer5.lit(0.5).uniform(writer5.lit(1.5)).expr()
    );

    let update_accel5 = AccelModifier::new(writer5.lit(Vec3::new(0.0, -8.0, 0.0)).expr());
    let update_drag5 = LinearDragModifier::new(writer5.lit(2.0).expr());

    let mass_explosion_module = writer5.finish();

    // Spawn ~500 particles over 2 seconds (250/sec rate)
    // SpawnerSettings::new(count, spawn_duration, period, cycle_count)
    // - count: particles per cycle
    // - spawn_duration: time to spawn that count
    // - period: total cycle time (spawn + pause)
    // - cycle_count: how many cycles (0 = infinite)
    let mass_explosion_effect = effects.add(
        EffectAsset::new(
            1024, // capacity
            SpawnerSettings::new(
                500.0.into(),  // 500 particles total
                2.0.into(),    // spawn over 2 seconds
                2.0.into(),    // no pause (period = spawn_duration)
                1,             // single cycle
            ),
            mass_explosion_module
        )
            .with_name("mass_explosion")
            .init(init_pos5)
            .init(init_vel5)
            .init(init_age5)
            .init(init_lifetime5)
            .init(init_size5)
            .update(update_accel5)
            .update(update_drag5)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))
            .render(ColorOverLifetimeModifier::new(color_gradient5))
            .render(SizeOverLifetimeModifier { gradient: size_gradient5, screen_space_size: false })
    );

    // === UNIT DEATH FLASH ===
    // Quick, LOW ground-level flash - stays near unit position, doesn't fly high
    // 10 particles, 0.4s lifetime, small-medium size
    let mut color_gradient6 = bevy_hanabi::Gradient::new();
    color_gradient6.add_key(0.0, Vec4::new(5.0, 4.0, 2.0, 1.0));  // Bright white-yellow HDR
    color_gradient6.add_key(0.1, Vec4::new(4.0, 2.0, 0.3, 1.0));  // Orange
    color_gradient6.add_key(0.3, Vec4::new(2.0, 0.5, 0.0, 0.8));  // Dark orange
    color_gradient6.add_key(1.0, Vec4::new(0.3, 0.05, 0.0, 0.0)); // Fade out

    let mut size_gradient6 = bevy_hanabi::Gradient::new();
    size_gradient6.add_key(0.0, Vec3::splat(1.0));  // Start
    size_gradient6.add_key(0.1, Vec3::splat(1.8));  // Quick flash
    size_gradient6.add_key(0.5, Vec3::splat(1.0));  // Shrink
    size_gradient6.add_key(1.0, Vec3::splat(0.2));  // End small

    let writer6 = ExprWriter::new();

    // Small spawn radius around unit center
    let init_pos6 = SetPositionSphereModifier {
        center: writer6.lit(Vec3::ZERO).expr(),
        radius: writer6.lit(0.3).expr(),
        dimension: ShapeDimension::Volume,
    };

    // VERY low velocity - particles barely move, stay at ground level
    let init_vel6 = SetVelocitySphereModifier {
        center: writer6.lit(Vec3::ZERO).expr(),
        speed: writer6.lit(1.0).uniform(writer6.lit(3.0)).expr(),
    };

    let init_age6 = SetAttributeModifier::new(Attribute::AGE, writer6.lit(0.0).expr());
    let init_lifetime6 = SetAttributeModifier::new(Attribute::LIFETIME, writer6.lit(0.4).expr());
    let init_size6 = SetAttributeModifier::new(Attribute::SIZE, writer6.lit(1.0).expr());

    // HEAVY drag + gravity to keep particles grounded
    let update_drag6 = LinearDragModifier::new(writer6.lit(8.0).expr());
    let update_accel6 = AccelModifier::new(writer6.lit(Vec3::new(0.0, -10.0, 0.0)).expr());

    let death_flash_module = writer6.finish();

    let unit_death_flash = effects.add(
        EffectAsset::new(32, SpawnerSettings::once(10.0.into()), death_flash_module)
            .with_name("unit_death_flash")
            .init(init_pos6)
            .init(init_vel6)
            .init(init_age6)
            .init(init_lifetime6)
            .init(init_size6)
            .update(update_drag6)
            .update(update_accel6)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))
            .render(ColorOverLifetimeModifier::new(color_gradient6))
            .render(SizeOverLifetimeModifier { gradient: size_gradient6, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU SPARKS ===
    // Replaces CPU spark entities (30-60 per explosion) with single GPU effect
    // UE5 spec: 90° upward cone, gravity 9.8 m/s², HDR color curve cooling
    //
    // CPU color curve (from update_spark_color):
    //   t=0.0:  (12.5, 6.75, 1.9, 1.0)   - Hot HDR orange-yellow
    //   t=0.55: (0.25, 0.0125, 0.0, 0.5) - Cooled to dim red, half alpha
    //   t=1.0:  (0.125, 0.005, 0.0, 0.0) - Fully faded
    let mut spark_color_gradient = bevy_hanabi::Gradient::new();
    // Match CPU's linear interpolation from hot orange-yellow to cooled red
    // CPU shader applies 4x brightness multiplier, so multiply by 4 here:
    // CPU values: (12.5, 6.75, 1.9) * 4 = (50, 27, 7.6) final HDR
    spark_color_gradient.add_key(0.0, Vec4::new(50.0, 27.0, 7.6, 1.0));    // Hot HDR orange-yellow
    spark_color_gradient.add_key(0.2, Vec4::new(32.0, 17.2, 4.8, 0.9));    // Still hot
    spark_color_gradient.add_key(0.4, Vec4::new(14.0, 7.2, 2.0, 0.75));    // Cooling
    spark_color_gradient.add_key(0.55, Vec4::new(1.0, 0.05, 0.0, 0.5));    // Cooled to dim red
    spark_color_gradient.add_key(0.75, Vec4::new(0.72, 0.032, 0.0, 0.25)); // Fading
    spark_color_gradient.add_key(1.0, Vec4::new(0.5, 0.02, 0.0, 0.0));     // Gone

    // CPU sparks maintain constant size (no size animation in update_spark_color)
    let mut spark_size_gradient = bevy_hanabi::Gradient::new();
    spark_size_gradient.add_key(0.0, Vec3::splat(1.0));
    spark_size_gradient.add_key(1.0, Vec3::splat(1.0));

    let writer_spark = ExprWriter::new();

    // Spawn at explosion center (CPU spawns at position, not offset)
    let spark_init_pos = SetPositionSphereModifier {
        center: writer_spark.lit(Vec3::ZERO).expr(),
        radius: writer_spark.lit(0.1).expr(),  // Very small radius - CPU spawns at center
        dimension: ShapeDimension::Volume,
    };

    // === VELOCITY: 90° upward hemisphere ===
    // CPU uses spherical coordinates: phi in [0, PI/2], theta in [0, TAU]
    // velocity = (sin(phi)*cos(theta), cos(phi), sin(phi)*sin(theta)) * speed
    // This creates an upward hemisphere cone with proper distribution.
    //
    // GPU: Use same spherical coordinate approach for identical distribution
    let theta = writer_spark.rand(ScalarType::Float) * writer_spark.lit(std::f32::consts::TAU);
    let phi = writer_spark.rand(ScalarType::Float) * writer_spark.lit(std::f32::consts::FRAC_PI_2);
    // Direction from spherical coords: (sin(phi)*cos(theta), cos(phi), sin(phi)*sin(theta))
    let sin_phi = phi.clone().sin();
    let cos_phi = phi.clone().cos();
    let cos_theta = theta.clone().cos();
    let sin_theta = theta.sin();
    let dir_x = sin_phi.clone() * cos_theta;
    let dir_y = cos_phi.clone();  // Y is up
    let dir_z = sin_phi * sin_theta;
    let hemisphere_dir = dir_x.vec3(dir_y, dir_z);
    // Random speed 15-37.5 m/s (matching CPU's rng.gen_range(15.0..37.5))
    // CPU also applies falloff: speed * (1.0 - (phi/PI_2) * 0.5)
    // falloff ranges from 1.0 (phi=0, straight up) to 0.5 (phi=PI/2, horizontal)
    let falloff = writer_spark.lit(1.0) - phi / writer_spark.lit(std::f32::consts::FRAC_PI_2) * writer_spark.lit(0.5);
    let spark_speed = (writer_spark.lit(15.0) + writer_spark.rand(ScalarType::Float) * writer_spark.lit(22.5)) * falloff;
    let spark_velocity = hemisphere_dir * spark_speed;
    let spark_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, spark_velocity.expr());

    let spark_init_age = SetAttributeModifier::new(Attribute::AGE, writer_spark.lit(0.0).expr());
    let spark_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_spark.lit(0.5) + writer_spark.rand(ScalarType::Float) * writer_spark.lit(1.5)).expr()
    );
    // CPU size: 0.8..1.8 * scale (scale applied via Transform, so just use 0.8..1.8)
    let spark_init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        (writer_spark.lit(0.8) + writer_spark.rand(ScalarType::Float) * writer_spark.lit(1.0)).expr()
    );

    // Gravity: -9.8 m/s² (matching CPU's VelocityAligned { gravity: 9.8 })
    let spark_update_accel = AccelModifier::new(writer_spark.lit(Vec3::new(0.0, -9.8, 0.0)).expr());

    // Texture slot for flare.png
    let spark_texture_slot = writer_spark.lit(0u32).expr();

    let mut spark_module = writer_spark.finish();
    spark_module.add_texture_slot("spark_texture");

    let ground_sparks_effect = effects.add(
        EffectAsset::new(512, SpawnerSettings::once(45.0.into()), spark_module)
            .with_name("ground_explosion_sparks")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
            .init(spark_init_pos)
            .init(spark_init_vel)
            .init(spark_init_age)
            .init(spark_init_lifetime)
            .init(spark_init_size)
            .update(spark_update_accel)
            .render(OrientModifier::new(OrientMode::AlongVelocity))
            .render(ParticleTextureModifier {
                texture_slot: spark_texture_slot,
                sample_mapping: ImageSampleMapping::ModulateOpacityFromR,  // Texture R channel controls alpha/shape
            })
            .render(ColorOverLifetimeModifier::new(spark_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: spark_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU FLASH SPARKS ===
    // Replaces CPU flash spark entities (20-50 per explosion) with single GPU effect
    // UE5 spec: Ring spawn, 100° cone, deceleration physics, "shooting star" elongation
    //
    // CPU behavior (from update_spark_l_physics):
    //   Color: Constant HDR orange (2.5, 1.625, 0.975), only alpha fades 1→0 LINEAR
    //   Velocity: 100° cone (wider than 90° hemisphere), ring spawn at equator
    //   Physics: CONSTANT DECELERATION (-2.5, -10, -5) m/s² - NOT drag!
    //   Speed: 4-55 m/s * scale (scale typically 1.0)
    let mut flash_color_gradient = bevy_hanabi::Gradient::new();
    // Constant HDR orange, alpha fades LINEARLY (1.0 - t)
    // CPU shader applies 4x brightness: (2.5, 1.625, 0.975) * 4 = (10, 6.5, 3.9)
    flash_color_gradient.add_key(0.0, Vec4::new(10.0, 6.5, 3.9, 1.0));
    flash_color_gradient.add_key(0.5, Vec4::new(10.0, 6.5, 3.9, 0.5));
    flash_color_gradient.add_key(1.0, Vec4::new(10.0, 6.5, 3.9, 0.0));

    // "Shooting star" effect with AlongVelocity: X is along velocity, Y is perpendicular
    // CPU: t=0: tiny → t=0.05: elongated (Y=0.3, X=50) → t=0.5: normalized (Y=5, X=3)
    // For AlongVelocity: X = along velocity (elongated), Y = perpendicular (thin)
    let mut flash_size_gradient = bevy_hanabi::Gradient::new();
    flash_size_gradient.add_key(0.0, Vec3::new(10.0, 0.1, 1.0));   // Very elongated along velocity
    flash_size_gradient.add_key(0.05, Vec3::new(10.0, 0.1, 1.0));  // Hold elongation briefly
    flash_size_gradient.add_key(0.2, Vec3::new(2.0, 0.3, 1.0));    // Shrinking
    flash_size_gradient.add_key(0.5, Vec3::new(0.6, 1.0, 1.0));    // Normalized
    flash_size_gradient.add_key(1.0, Vec3::new(0.6, 1.0, 1.0));    // Hold

    let writer_flash = ExprWriter::new();

    // Ring spawn on XZ plane (equator) - matches CPU's spawn_offset calculation
    let flash_init_pos = SetPositionCircleModifier {
        center: writer_flash.lit(Vec3::ZERO).expr(),
        axis: writer_flash.lit(Vec3::Y).expr(),
        radius: writer_flash.lit(0.5).expr(),
        dimension: ShapeDimension::Surface,
    };

    // === VELOCITY: 100° cone (wider than hemisphere) ===
    // CPU uses phi in [0, 100°] where 90° = horizontal, 100° = slightly below
    // velocity = (sin(phi)*cos(theta), cos(phi), sin(phi)*sin(theta)) * speed
    // CPU also applies velocity falloff: falloff = 1.0 - (phi / max_phi) * 0.5
    // This reduces speed for more horizontal particles (phi near max).
    //
    // GPU: Use same spherical coordinate approach for identical distribution
    let flash_theta = writer_flash.rand(ScalarType::Float) * writer_flash.lit(std::f32::consts::TAU);
    let max_phi_f = writer_flash.lit(100.0_f32.to_radians()); // 100° cone
    let flash_phi = writer_flash.rand(ScalarType::Float) * max_phi_f.clone();
    // Direction from spherical coords: (sin(phi)*cos(theta), cos(phi), sin(phi)*sin(theta))
    let flash_sin_phi = flash_phi.clone().sin();
    let flash_cos_phi = flash_phi.clone().cos();
    let flash_cos_theta = flash_theta.clone().cos();
    let flash_sin_theta = flash_theta.sin();
    let flash_dir_x = flash_sin_phi.clone() * flash_cos_theta;
    let flash_dir_y = flash_cos_phi.clone();  // Y is up
    let flash_dir_z = flash_sin_phi * flash_sin_theta;
    let cone_dir = flash_dir_x.vec3(flash_dir_y, flash_dir_z);
    // Speed 4-55 m/s (matching CPU's rng.gen_range(4.0..55.0))
    // CPU applies falloff: speed * (1.0 - (phi / max_phi) * 0.5)
    // Falloff ranges from 1.0 (phi=0, straight up) to 0.5 (phi=100°, slightly down)
    let flash_falloff = writer_flash.lit(1.0) - flash_phi / max_phi_f * writer_flash.lit(0.5);
    let flash_speed = (writer_flash.lit(4.0) + writer_flash.rand(ScalarType::Float) * writer_flash.lit(51.0)) * flash_falloff;
    let flash_velocity = cone_dir * flash_speed;
    let flash_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, flash_velocity.expr());

    let flash_init_age = SetAttributeModifier::new(Attribute::AGE, writer_flash.lit(0.0).expr());
    let flash_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_flash.lit(0.3) + writer_flash.rand(ScalarType::Float) * writer_flash.lit(0.7)).expr()
    );
    let flash_init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        (writer_flash.lit(0.4) + writer_flash.rand(ScalarType::Float) * writer_flash.lit(0.6)).expr()
    );

    // CPU flash sparks: CONSTANT DECELERATION, not gravity or drag!
    // CPU update_spark_l_physics: velocity += deceleration * dt * 10.0
    // deceleration = Vec3::new(-0.25, -1.0, -0.5) * 10 = (-2.5, -10, -5) m/s²
    // This is constant acceleration (negative = deceleration), not velocity-proportional drag.
    // AccelModifier applies: velocity += accel * dt, matching CPU behavior.
    let flash_update_accel = AccelModifier::new(writer_flash.lit(Vec3::new(-2.5, -10.0, -5.0)).expr());

    // Texture slot (same flare.png)
    let flash_texture_slot = writer_flash.lit(0u32).expr();

    let mut flash_module = writer_flash.finish();
    flash_module.add_texture_slot("flash_spark_texture");

    let ground_flash_sparks_effect = effects.add(
        EffectAsset::new(256, SpawnerSettings::once(35.0.into()), flash_module)
            .with_name("ground_explosion_flash_sparks")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
            .init(flash_init_pos)
            .init(flash_init_vel)
            .init(flash_init_age)
            .init(flash_init_lifetime)
            .init(flash_init_size)
            .update(flash_update_accel)
            // CPU uses constant deceleration (-2.5, -10, -5) m/s², not drag
            // AlongVelocity: X-axis along velocity, Y perpendicular, faces camera
            .render(OrientModifier::new(OrientMode::AlongVelocity))
            .render(ParticleTextureModifier {
                texture_slot: flash_texture_slot,
                sample_mapping: ImageSampleMapping::ModulateOpacityFromR,  // Texture R channel controls alpha/shape
            })
            .render(ColorOverLifetimeModifier::new(flash_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: flash_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU PARTS DEBRIS ===
    // Replaces CPU parts entities (50-75 per explosion) with single GPU effect
    // Uses baked sprite sheet of 3D debris meshes from multiple angles
    // Sprite sheet: 8 columns (angles) × 3 rows (variants) = 24 frames
    //
    // CPU behavior (from spawn_parts):
    //   Count: 50-75 particles
    //   Size: 0.3-0.5m * scale
    //   Velocity: X/Z: ±8m/s, Y: 5-25m/s (strong upward launch)
    //   Lifetime: 0.5-1.5s
    //   Gravity: 9.8 m/s²
    //   Scale curve: grow-in (0-10%), hold (10-90%), shrink-out (90-100%)
    let ground_parts_texture: Handle<Image> = asset_server.load("textures/generated/debris_sprites.png");

    // Color gradient: white (texture provides color), alpha for fade in/out
    let mut parts_color_gradient = bevy_hanabi::Gradient::new();
    parts_color_gradient.add_key(0.0, Vec4::new(1.0, 1.0, 1.0, 0.0));   // Start invisible
    parts_color_gradient.add_key(0.1, Vec4::new(1.0, 1.0, 1.0, 1.0));   // Fade in by 10%
    parts_color_gradient.add_key(0.9, Vec4::new(1.0, 1.0, 1.0, 1.0));   // Hold visible
    parts_color_gradient.add_key(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0));   // Fade out at end

    // Size gradient: grow-in, hold, shrink-out matching CPU scale curve
    let mut parts_size_gradient = bevy_hanabi::Gradient::new();
    parts_size_gradient.add_key(0.0, Vec3::splat(0.0));   // Start at 0
    parts_size_gradient.add_key(0.1, Vec3::splat(1.0));   // Grow to full by 10%
    parts_size_gradient.add_key(0.9, Vec3::splat(1.0));   // Hold at full
    parts_size_gradient.add_key(1.0, Vec3::splat(0.0));   // Shrink to 0 at end

    let writer_parts = ExprWriter::new();

    // Spawn at explosion center
    let parts_init_pos = SetPositionSphereModifier {
        center: writer_parts.lit(Vec3::ZERO).expr(),
        radius: writer_parts.lit(0.5).expr(),  // Small spawn radius
        dimension: ShapeDimension::Volume,
    };

    // Box velocity: X/Z: ±8, Y: 5-25 (CPU's UniformRangedVector)
    let parts_vel_x = writer_parts.rand(ScalarType::Float) * writer_parts.lit(16.0) - writer_parts.lit(8.0);
    let parts_vel_y = writer_parts.lit(5.0) + writer_parts.rand(ScalarType::Float) * writer_parts.lit(20.0);
    let parts_vel_z = writer_parts.rand(ScalarType::Float) * writer_parts.lit(16.0) - writer_parts.lit(8.0);
    let parts_velocity = parts_vel_x.vec3(parts_vel_y, parts_vel_z);
    let parts_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, parts_velocity.expr());

    let parts_init_age = SetAttributeModifier::new(Attribute::AGE, writer_parts.lit(0.0).expr());
    // Lifetime: 0.5-1.5s (matching CPU)
    let parts_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_parts.lit(0.5) + writer_parts.rand(ScalarType::Float) * writer_parts.lit(1.0)).expr()
    );
    // Size: 0.3-0.5m (matching CPU's rng.gen_range(0.3..0.5))
    let parts_init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        (writer_parts.lit(0.3) + writer_parts.rand(ScalarType::Float) * writer_parts.lit(0.2)).expr()
    );

    // Random sprite index [0, 23] - picks one of 24 frames (3 variants × 8 angles)
    // Each particle gets a fixed random frame at spawn (no animation)
    let parts_init_sprite = SetAttributeModifier::new(
        Attribute::SPRITE_INDEX,
        (writer_parts.rand(ScalarType::Float) * writer_parts.lit(24.0))
            .cast(ScalarType::Int)
            .expr()
    );

    // Gravity: -9.8 m/s² (matching CPU)
    let parts_update_accel = AccelModifier::new(writer_parts.lit(Vec3::new(0.0, -9.8, 0.0)).expr());

    // Texture slot for the sprite sheet
    let parts_texture_slot = writer_parts.lit(0u32).expr();

    let mut parts_module = writer_parts.finish();
    parts_module.add_texture_slot("debris_sprites");

    let ground_parts_effect = effects.add(
        EffectAsset::new(128, SpawnerSettings::once(60.0.into()), parts_module)
            .with_name("ground_explosion_parts")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(parts_init_pos)
            .init(parts_init_vel)
            .init(parts_init_age)
            .init(parts_init_lifetime)
            .init(parts_init_size)
            .init(parts_init_sprite)
            .update(parts_update_accel)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))  // Billboard facing camera
            .render(ParticleTextureModifier {
                texture_slot: parts_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,  // Texture provides both color and alpha
            })
            .render(FlipbookModifier { sprite_grid_size: UVec2::new(8, 3) })  // 8 columns × 3 rows
            .render(ColorOverLifetimeModifier::new(parts_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: parts_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU DIRT DEBRIS ===
    // Replaces CPU dirt entities (35 per explosion) with single GPU effect
    // CPU behavior (from spawn_dirt_debris):
    //   Count: 35 particles
    //   Size: 1.0-2.0m with non-uniform XY (X: 0.3-1.0, Y: 0.4-1.0)
    //   Velocity: box X/Z: ±5, Y: 15-25
    //   Lifetime: 1.0-4.0s
    //   Color: dark brown (0.082, 0.063, 0.050), fade-in then fade-out
    //   Physics: gravity 9.8, drag 2.0
    //   Orientation: CameraFacing
    let ground_dirt_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/dirt.png");

    // Color gradient: dark brown with alpha fade-in/out
    let mut dirt_color_gradient = bevy_hanabi::Gradient::new();
    dirt_color_gradient.add_key(0.0, Vec4::new(0.082, 0.063, 0.050, 0.0));   // Start invisible
    dirt_color_gradient.add_key(0.1, Vec4::new(0.082, 0.063, 0.050, 1.0));   // Fade in
    dirt_color_gradient.add_key(0.7, Vec4::new(0.082, 0.063, 0.050, 1.0));   // Hold
    dirt_color_gradient.add_key(1.0, Vec4::new(0.082, 0.063, 0.050, 0.0));   // Fade out

    // Size gradient: shrink over lifetime (CPU uses scale curve)
    // Non-uniform size handled by initial SIZE3 attribute
    let mut dirt_size_gradient = bevy_hanabi::Gradient::new();
    dirt_size_gradient.add_key(0.0, Vec3::splat(1.0));   // Full size
    dirt_size_gradient.add_key(1.0, Vec3::splat(0.3));   // Shrink to 30%

    let writer_dirt = ExprWriter::new();

    // Spawn at explosion center
    let dirt_init_pos = SetPositionSphereModifier {
        center: writer_dirt.lit(Vec3::ZERO).expr(),
        radius: writer_dirt.lit(0.5).expr(),
        dimension: ShapeDimension::Volume,
    };

    // Box velocity: X/Z: ±5, Y: 15-25 (matching CPU)
    let dirt_vel_x = writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(10.0) - writer_dirt.lit(5.0);
    let dirt_vel_y = writer_dirt.lit(15.0) + writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(10.0);
    let dirt_vel_z = writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(10.0) - writer_dirt.lit(5.0);
    let dirt_velocity = dirt_vel_x.vec3(dirt_vel_y, dirt_vel_z);
    let dirt_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, dirt_velocity.expr());

    let dirt_init_age = SetAttributeModifier::new(Attribute::AGE, writer_dirt.lit(0.0).expr());
    // Lifetime: 1.0-4.0s (matching CPU)
    let dirt_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_dirt.lit(1.0) + writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(3.0)).expr()
    );
    // Non-uniform size: base 1.0-2.0m, X: 0.3-1.0, Y: 0.4-1.0
    // SIZE3 allows Vec3 with independent X, Y, Z
    let dirt_base_size = writer_dirt.lit(1.0) + writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(1.0);
    let dirt_scale_x = writer_dirt.lit(0.3) + writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(0.7);
    let dirt_scale_y = writer_dirt.lit(0.4) + writer_dirt.rand(ScalarType::Float) * writer_dirt.lit(0.6);
    let dirt_size = (dirt_base_size.clone() * dirt_scale_x).vec3(dirt_base_size.clone() * dirt_scale_y, dirt_base_size);
    let dirt_init_size = SetAttributeModifier::new(Attribute::SIZE3, dirt_size.expr());

    // Gravity + drag approximation: use AccelModifier for gravity, LinearDragModifier for drag
    let dirt_update_accel = AccelModifier::new(writer_dirt.lit(Vec3::new(0.0, -9.8, 0.0)).expr());
    let dirt_update_drag = LinearDragModifier::new(writer_dirt.lit(2.0).expr());

    let dirt_texture_slot = writer_dirt.lit(0u32).expr();
    let mut dirt_module = writer_dirt.finish();
    dirt_module.add_texture_slot("dirt_texture");

    let ground_dirt_effect = effects.add(
        EffectAsset::new(64, SpawnerSettings::once(35.0.into()), dirt_module)
            .with_name("ground_explosion_dirt")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(dirt_init_pos)
            .init(dirt_init_vel)
            .init(dirt_init_age)
            .init(dirt_init_lifetime)
            .init(dirt_init_size)
            .update(dirt_update_accel)
            .update(dirt_update_drag)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))  // Billboard
            .render(ParticleTextureModifier {
                texture_slot: dirt_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,
            })
            .render(ColorOverLifetimeModifier::new(dirt_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: dirt_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU VELOCITY DIRT ===
    // Replaces CPU velocity dirt entities (10-15 per explosion) with single GPU effect
    // CPU behavior (from spawn_velocity_dirt):
    //   Count: 10-15 particles
    //   Size: 1.0-2.0m with non-uniform XY (X: 0.5-1.0, Y: 0.6-1.2)
    //   Velocity: hemisphere cone, speed 2.5-10m/s
    //   Lifetime: 0.8-1.7s
    //   Color: dark brown (same as dirt)
    //   Physics: NO gravity, drag 2.0
    //   Orientation: VelocityAligned

    // Color gradient: same dark brown with alpha fade
    let mut vdirt_color_gradient = bevy_hanabi::Gradient::new();
    vdirt_color_gradient.add_key(0.0, Vec4::new(0.082, 0.063, 0.050, 0.0));
    vdirt_color_gradient.add_key(0.1, Vec4::new(0.082, 0.063, 0.050, 1.0));
    vdirt_color_gradient.add_key(0.7, Vec4::new(0.082, 0.063, 0.050, 1.0));
    vdirt_color_gradient.add_key(1.0, Vec4::new(0.082, 0.063, 0.050, 0.0));

    // Size gradient for velocity-aligned particles
    // CPU Dirt001ScaleOverLife: Linear GROWTH from 1.0 to 2.0 (opposite of dirt!)
    let mut vdirt_size_gradient = bevy_hanabi::Gradient::new();
    vdirt_size_gradient.add_key(0.0, Vec3::splat(1.0));  // Start at 1.0×
    vdirt_size_gradient.add_key(1.0, Vec3::splat(2.0)); // Grow to 2.0×

    let writer_vdirt = ExprWriter::new();

    let vdirt_init_pos = SetPositionSphereModifier {
        center: writer_vdirt.lit(Vec3::ZERO).expr(),
        radius: writer_vdirt.lit(0.5).expr(),
        dimension: ShapeDimension::Volume,
    };

    // Hemisphere cone velocity: spherical coords with phi in [0, 90°]
    // CPU has falloff: faster at center (phi=0), slower at edges (phi=90°)
    // falloff = (1.0 - phi / (PI/2))^2, adjusted_speed = speed * (0.5 + 0.5 * falloff)
    let vdirt_theta = writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(std::f32::consts::TAU);
    let vdirt_phi = writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(std::f32::consts::FRAC_PI_2);
    let vdirt_sin_phi = vdirt_phi.clone().sin();
    let vdirt_cos_phi = vdirt_phi.clone().cos();
    let vdirt_cos_theta = vdirt_theta.clone().cos();
    let vdirt_sin_theta = vdirt_theta.sin();
    let vdirt_dir_x = vdirt_sin_phi.clone() * vdirt_cos_theta;
    let vdirt_dir_y = vdirt_cos_phi;
    let vdirt_dir_z = vdirt_sin_phi * vdirt_sin_theta;
    let vdirt_dir = vdirt_dir_x.vec3(vdirt_dir_y, vdirt_dir_z);
    // Speed: 2.5-10m/s with falloff (faster at center)
    // falloff = (1 - phi / (PI/2))^2, adjusted = speed * (0.5 + 0.5 * falloff)
    // No powf in expr API, so compute x^2 = x * x
    let vdirt_base_speed = writer_vdirt.lit(2.5) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(7.5);
    let vdirt_phi_normalized = vdirt_phi / writer_vdirt.lit(std::f32::consts::FRAC_PI_2);
    let vdirt_falloff_base = writer_vdirt.lit(1.0) - vdirt_phi_normalized;
    let vdirt_falloff = vdirt_falloff_base.clone() * vdirt_falloff_base; // x^2
    let vdirt_speed = vdirt_base_speed * (writer_vdirt.lit(0.5) + writer_vdirt.lit(0.5) * vdirt_falloff);
    let vdirt_velocity = vdirt_dir * vdirt_speed;
    let vdirt_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, vdirt_velocity.expr());

    let vdirt_init_age = SetAttributeModifier::new(Attribute::AGE, writer_vdirt.lit(0.0).expr());
    // Lifetime: 0.8-1.7s
    let vdirt_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_vdirt.lit(0.8) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(0.9)).expr()
    );
    // Non-uniform size: base 1.0-2.0m
    // CPU VelocityAligned: Y = along velocity (height), X = perpendicular (width)
    // CPU scales: base_scale_x = 0.5-1.0 (width), base_scale_y = 0.6-1.2 (height along velocity)
    // bevy_hanabi AlongVelocity: X = along velocity, Y = perpendicular
    // So we need to SWAP: GPU_X = CPU_Y (along velocity), GPU_Y = CPU_X (perpendicular)
    let vdirt_base_size = writer_vdirt.lit(1.0) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(1.0); // 1.0-2.0m
    let vdirt_cpu_scale_x = writer_vdirt.lit(0.5) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(0.5); // 0.5-1.0 (width)
    let vdirt_cpu_scale_y = writer_vdirt.lit(0.6) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(0.6); // 0.6-1.2 (height)
    // Swap axes: GPU_X = CPU_Y (height along velocity), GPU_Y = CPU_X (width perpendicular)
    let vdirt_size = (vdirt_base_size.clone() * vdirt_cpu_scale_y).vec3(vdirt_base_size.clone() * vdirt_cpu_scale_x, vdirt_base_size);
    let vdirt_init_size = SetAttributeModifier::new(Attribute::SIZE3, vdirt_size.expr());

    // No gravity, only drag
    let vdirt_update_drag = LinearDragModifier::new(writer_vdirt.lit(2.0).expr());

    let vdirt_texture_slot = writer_vdirt.lit(0u32).expr();
    let mut vdirt_module = writer_vdirt.finish();
    vdirt_module.add_texture_slot("vdirt_texture");

    let ground_vdirt_effect = effects.add(
        EffectAsset::new(32, SpawnerSettings::once(12.0.into()), vdirt_module)
            .with_name("ground_explosion_velocity_dirt")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(vdirt_init_pos)
            .init(vdirt_init_vel)
            .init(vdirt_init_age)
            .init(vdirt_init_lifetime)
            .init(vdirt_init_size)
            .update(vdirt_update_drag)
            .render(OrientModifier::new(OrientMode::AlongVelocity))  // Velocity aligned
            .render(ParticleTextureModifier {
                texture_slot: vdirt_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,
            })
            .render(ColorOverLifetimeModifier::new(vdirt_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: vdirt_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU FIREBALL ===
    // Replaces CPU fireball entities (main: 9-17, secondary: 7-13 per explosion)
    // Combined into single effect with ~25 particles total
    // CPU behavior (from spawn_main_fireball/spawn_secondary_fireball):
    //   Count: 9-17 (main) + 7-13 (secondary) = ~16-30 total
    //   Size: 14-18m base × 0.5→1.3 scale curve over lifetime
    //   Velocity: 90° hemisphere cone, 3-5 m/s
    //   Lifetime: 1.5s
    //   8x8 flipbook (64 frames)
    //   Orientation: VelocityAligned with bottom pivot
    //   Color: orange HSV variation (hue 0.08 ± 0.1)
    //   Alpha: S-curve fade
    // Original texture with code rotation
    let ground_fireball_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/main_9x9.png");
    let ground_fireball_secondary_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/secondary_8x8.png");

    // Color gradient: orange with S-curve alpha fade
    // CPU uses HSV variation around hue 0.08 (orange), saturation 0.8-1.0, value 0.8-1.0
    // S-curve alpha: t=0→1.0, t=0.2→0.77, t=0.4→0.56, t=0.6→0.33, t=0.8→0.14, t=1.0→0.0
    let mut fireball_color_gradient = bevy_hanabi::Gradient::new();
    fireball_color_gradient.add_key(0.0, Vec4::new(1.0, 0.5, 0.1, 1.0));   // Bright orange
    fireball_color_gradient.add_key(0.2, Vec4::new(1.0, 0.4, 0.1, 0.77));  // S-curve fade
    fireball_color_gradient.add_key(0.4, Vec4::new(0.9, 0.35, 0.1, 0.56));
    fireball_color_gradient.add_key(0.6, Vec4::new(0.8, 0.3, 0.1, 0.33));
    fireball_color_gradient.add_key(0.8, Vec4::new(0.7, 0.25, 0.1, 0.14));
    fireball_color_gradient.add_key(1.0, Vec4::new(0.5, 0.2, 0.1, 0.0));   // Fade out

    // Size gradient: CPU uses cubic ease-out (1-(1-t)³) from 0.5→1.3 scale
    // Base 16m * scale: 8m start → 21m end, fast expansion early
    let mut fireball_size_gradient = bevy_hanabi::Gradient::new();
    fireball_size_gradient.add_key(0.0, Vec3::splat(8.0));    // t=0: 0.5x
    fireball_size_gradient.add_key(0.2, Vec3::splat(14.2));   // t=0.2: 0.89x (fast jump)
    fireball_size_gradient.add_key(0.4, Vec3::splat(18.0));   // t=0.4: 1.127x
    fireball_size_gradient.add_key(0.6, Vec3::splat(20.0));   // t=0.6: 1.249x
    fireball_size_gradient.add_key(0.8, Vec3::splat(20.7));   // t=0.8: 1.294x
    fireball_size_gradient.add_key(1.0, Vec3::splat(21.0));   // t=1.0: 1.3x

    let writer_fireball = ExprWriter::new();

    // Position: hemisphere (Y >= 0) surface, radius 7.5 (15m diameter)
    let rx = writer_fireball.rand(ScalarType::Float) * writer_fireball.lit(2.0) - writer_fireball.lit(1.0);
    let ry = writer_fireball.rand(ScalarType::Float); // [0,1] for Y >= 0
    let rz = writer_fireball.rand(ScalarType::Float) * writer_fireball.lit(2.0) - writer_fireball.lit(1.0);
    let fb_pos = rx.vec3(ry, rz).normalized() * writer_fireball.lit(7.5);
    let fireball_init_pos = SetAttributeModifier::new(Attribute::POSITION, fb_pos.expr());

    // Velocity: outward from spawn position
    // velocity = normalize(position) * speed
    let fb_pos_read = writer_fireball.attr(Attribute::POSITION);
    let fb_outward_dir = fb_pos_read.normalized();
    let fb_speed = writer_fireball.lit(5.0) + writer_fireball.rand(ScalarType::Float) * writer_fireball.lit(3.0); // 5-8 m/s
    let fb_velocity = fb_outward_dir * fb_speed;
    let fireball_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, fb_velocity.expr());

    let fireball_init_age = SetAttributeModifier::new(Attribute::AGE, writer_fireball.lit(0.0).expr());
    // Lifetime 1.5s (matches CPU)
    let fireball_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer_fireball.lit(1.5).expr()
    );

    // Flipbook animation: 8x8 grid, 64 frames over 1.5s lifetime
    // Frame index driven by age/lifetime ratio
    let fb_age = writer_fireball.attr(Attribute::AGE);
    let fb_lifetime = writer_fireball.attr(Attribute::LIFETIME);
    let fb_progress = fb_age / fb_lifetime;
    let fb_frame = (fb_progress * writer_fireball.lit(64.0))
        .cast(ScalarType::Int)
        .min(writer_fireball.lit(63i32));
    let fireball_init_sprite = SetAttributeModifier::new(Attribute::SPRITE_INDEX, fb_frame.expr());

    let fireball_texture_slot = writer_fireball.lit(0u32).expr();
    // -90° rotation to align Y along velocity
    let fireball_rotation = writer_fireball.lit(-std::f32::consts::FRAC_PI_2).expr();
    // Per-particle random axis rotation (spin around velocity axis, 0 to TAU)
    // CPU: rotation_angle = rng.gen_range(0.0..TAU), applied as Quat::from_rotation_y
    let fb_random_spin = writer_fireball.rand(ScalarType::Float) * writer_fireball.lit(std::f32::consts::TAU);
    let fireball_init_spin = SetAttributeModifier::new(Attribute::F32_0, fb_random_spin.expr());
    let fireball_axis_rotation = writer_fireball.attr(Attribute::F32_0).expr();
    let mut fireball_module = writer_fireball.finish();
    fireball_module.add_texture_slot("fireball_texture");

    // TEST: Simplified version - no OrientModifier, no UV scaling
    // SimulationSpace::Local makes transform scale apply to particle positions and velocities
    let ground_fireball_effect = effects.add(
        EffectAsset::new(64, SpawnerSettings::once(25.0.into()), fireball_module)
            .with_name("ground_explosion_fireball")
            .with_simulation_space(SimulationSpace::Local)
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(fireball_init_pos)
            .init(fireball_init_vel)
            .init(fireball_init_age)
            .init(fireball_init_lifetime)
            .init(fireball_init_spin)
            .update(fireball_init_sprite)
            .render(OrientModifier::new(OrientMode::AlongVelocity)
                .with_rotation(fireball_rotation)
                .with_axis_rotation(fireball_axis_rotation))
            .render(ParticleTextureModifier {
                texture_slot: fireball_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,
            })
            // UV zoom MUST come before FlipbookModifier (CPU shader zooms before frame offset)
            .render(UVScaleOverLifetimeModifier {
                gradient: {
                    let mut g = bevy_hanabi::Gradient::new();
                    g.add_key(0.0, Vec2::splat(500.0));
                    g.add_key(0.2, Vec2::splat(466.0));
                    g.add_key(0.4, Vec2::splat(350.0));
                    g.add_key(0.6, Vec2::splat(224.0));
                    g.add_key(0.8, Vec2::splat(100.0));
                    g.add_key(1.0, Vec2::splat(1.0));
                    g
                },
            })
            .render(FlipbookModifier { sprite_grid_size: UVec2::new(8, 8) })
            .render(ColorOverLifetimeModifier::new(fireball_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: fireball_size_gradient, screen_space_size: false })
    );

    // ========== DEBUG FIREBALL EFFECT ==========
    // Simple colored quads to visualize velocity direction
    // Small particles, no texture, color shows velocity direction
    let writer_debug = ExprWriter::new();

    // Position: hemisphere (Y >= 0) surface, radius 2.5 (5m diameter)
    let dbg_rx = writer_debug.rand(ScalarType::Float) * writer_debug.lit(2.0) - writer_debug.lit(1.0);
    let dbg_ry = writer_debug.rand(ScalarType::Float); // [0,1] for Y >= 0
    let dbg_rz = writer_debug.rand(ScalarType::Float) * writer_debug.lit(2.0) - writer_debug.lit(1.0);
    let dbg_pos = dbg_rx.vec3(dbg_ry, dbg_rz).normalized() * writer_debug.lit(2.5);
    let debug_init_pos = SetAttributeModifier::new(Attribute::POSITION, dbg_pos.expr());

    // Velocity: outward from spawn position
    let dbg_pos_read = writer_debug.attr(Attribute::POSITION);
    let dbg_outward_dir = dbg_pos_read.normalized();
    let dbg_speed = writer_debug.lit(5.0);
    let dbg_velocity = dbg_outward_dir * dbg_speed;
    let debug_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, dbg_velocity.expr());

    let debug_init_age = SetAttributeModifier::new(Attribute::AGE, writer_debug.lit(0.0).expr());
    let debug_init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer_debug.lit(3.0).expr());

    // Color based on velocity direction (R=+X, G=+Y, B=+Z)
    let dbg_vel_read = writer_debug.attr(Attribute::VELOCITY).normalized();
    let dbg_vel_x = dbg_vel_read.clone().x();
    let dbg_vel_y = dbg_vel_read.clone().y();
    let dbg_vel_z = dbg_vel_read.z();
    // Map [-1,1] to [0,1] for color
    let dbg_r = (dbg_vel_x + writer_debug.lit(1.0)) * writer_debug.lit(0.5);
    let dbg_g = (dbg_vel_y + writer_debug.lit(1.0)) * writer_debug.lit(0.5);
    let dbg_b = (dbg_vel_z + writer_debug.lit(1.0)) * writer_debug.lit(0.5);
    let dbg_rgb = dbg_r.vec3(dbg_g, dbg_b);
    let dbg_color = dbg_rgb.vec4_xyz_w(writer_debug.lit(1.0)).pack4x8unorm();
    let debug_init_color = SetAttributeModifier::new(Attribute::COLOR, dbg_color.expr());

    let debug_module = writer_debug.finish();

    let debug_fireball_effect = effects.add(
        EffectAsset::new(100, SpawnerSettings::once(50.0.into()), debug_module)
            .with_name("debug_fireball")
            .with_simulation_space(SimulationSpace::Local)
            .init(debug_init_pos)
            .init(debug_init_vel)
            .init(debug_init_age)
            .init(debug_init_lifetime)
            .init(debug_init_color)
            .render(SizeOverLifetimeModifier {
                gradient: {
                    let mut g = bevy_hanabi::Gradient::new();
                    g.add_key(0.0, Vec3::splat(0.3));  // Small 0.3m particles
                    g.add_key(1.0, Vec3::splat(0.3));
                    g
                },
                screen_space_size: false,
            })
    );

    // === GROUND EXPLOSION GPU DUST RING ===
    // Replaces CPU dust entities (2-3 per explosion) with single GPU effect
    // CPU behavior (from spawn_dust_ring):
    //   Count: 2-3 particles
    //   Size: 3-5m base, scales from ZERO to (3×, 2×) - X grows faster than Y
    //   Velocity: 35° cone upward, 5-10 m/s
    //   Lifetime: 0.1-0.5s (very short)
    //   Color: dark brown (0.147, 0.114, 0.070), alpha 3.0→0 S-curve fade
    //   Texture: 4×1 flipbook, random FIXED frame (not animated)
    //   Orientation: VelocityAligned

    // Color: constant dark brown with HDR alpha (3.0→0) S-curve fade
    // UE5 uses alpha > 1.0 as brightness multiplier
    let mut dust_color_gradient = bevy_hanabi::Gradient::new();
    // S-curve fade: 3.0 * (1 - smoothstep(t))
    // t=0: 3.0, t=0.2: 2.59, t=0.5: 1.5, t=0.8: 0.34, t=1.0: 0
    dust_color_gradient.add_key(0.0, Vec4::new(0.147, 0.114, 0.070, 3.0));
    dust_color_gradient.add_key(0.2, Vec4::new(0.147, 0.114, 0.070, 2.59));
    dust_color_gradient.add_key(0.5, Vec4::new(0.147, 0.114, 0.070, 1.5));
    dust_color_gradient.add_key(0.8, Vec4::new(0.147, 0.114, 0.070, 0.34));
    dust_color_gradient.add_key(1.0, Vec4::new(0.147, 0.114, 0.070, 0.0));

    // Size gradient: linear growth from 0 to (3×, 2×) base size
    // Base size 4m (midpoint of 3-5m), so final X=12m, Y=8m
    // bevy_hanabi AlongVelocity: X = along velocity, Y = perpendicular
    // CPU VelocityAligned: Y = along velocity (height), X = perpendicular (width)
    // CPU scales: X grows to 3× (width), Y grows to 2× (height)
    // Swap for GPU: GPU_X = 2× (along velocity), GPU_Y = 3× (perpendicular width)
    let mut dust_size_gradient = bevy_hanabi::Gradient::new();
    dust_size_gradient.add_key(0.0, Vec3::new(0.0, 0.0, 1.0));           // Start at zero
    dust_size_gradient.add_key(1.0, Vec3::new(8.0, 12.0, 1.0));          // End at 2×, 3× (4m base)

    let writer_dust = ExprWriter::new();

    // Position: spawn at origin
    let dust_init_pos = SetAttributeModifier::new(
        Attribute::POSITION,
        writer_dust.lit(Vec3::ZERO).expr()
    );

    // Velocity: 35° cone pointing up, 5-10 m/s
    let dust_theta = writer_dust.rand(ScalarType::Float) * writer_dust.lit(std::f32::consts::TAU);
    let dust_cone_angle = writer_dust.lit(35.0_f32.to_radians());
    let dust_phi = writer_dust.rand(ScalarType::Float) * dust_cone_angle; // 0-35° from vertical
    let dust_sin_phi = dust_phi.clone().sin();
    let dust_cos_phi = dust_phi.cos();
    let dust_cos_theta = dust_theta.clone().cos();
    let dust_sin_theta = dust_theta.sin();
    let dust_dir_x = dust_sin_phi.clone() * dust_cos_theta;
    let dust_dir_y = dust_cos_phi;  // Mostly upward
    let dust_dir_z = dust_sin_phi * dust_sin_theta;
    let dust_dir = dust_dir_x.vec3(dust_dir_y, dust_dir_z);
    let dust_speed = writer_dust.lit(5.0) + writer_dust.rand(ScalarType::Float) * writer_dust.lit(5.0); // 5-10 m/s
    let dust_velocity = dust_dir * dust_speed;
    let dust_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, dust_velocity.expr());

    let dust_init_age = SetAttributeModifier::new(Attribute::AGE, writer_dust.lit(0.0).expr());
    // Lifetime: 0.1-0.5s
    let dust_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_dust.lit(0.1) + writer_dust.rand(ScalarType::Float) * writer_dust.lit(0.4)).expr()
    );

    // Sprite index: random fixed frame 0-3 (4 frames in 4×1 grid)
    // Cast rand float to int: floor(rand * 4)
    let dust_random_frame = (writer_dust.rand(ScalarType::Float) * writer_dust.lit(4.0)).cast(ScalarType::Int);
    let dust_init_sprite = SetAttributeModifier::new(Attribute::SPRITE_INDEX, dust_random_frame.expr());

    let dust_texture_slot = writer_dust.lit(0u32).expr();
    let mut dust_module = writer_dust.finish();
    dust_module.add_texture_slot("dust_texture");

    let ground_dust_effect = effects.add(
        EffectAsset::new(8, SpawnerSettings::once(3.0.into()), dust_module)
            .with_name("ground_explosion_dust_ring")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(dust_init_pos)
            .init(dust_init_vel)
            .init(dust_init_age)
            .init(dust_init_lifetime)
            .init(dust_init_sprite)
            .render(OrientModifier::new(OrientMode::AlongVelocity))  // Velocity aligned
            .render(ParticleTextureModifier {
                texture_slot: dust_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,
            })
            .render(FlipbookModifier { sprite_grid_size: UVec2::new(4, 1) })
            .render(ColorOverLifetimeModifier::new(dust_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: dust_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU SMOKE CLOUD ===
    // Replaces CPU smoke entities (10-15 per explosion) with single GPU effect
    // CPU behavior (from update_smoke_color, update_smoke_scale):
    //   Count: 10-15 particles
    //   Size: 0.5-1.0m base, grows 1x→3x using ease-out curve: 1-(1-t)²
    //   Velocity: screen-local spread ±12m/s on XY (we use world XZ)
    //   Lifetime: 0.8-2.5s
    //   Color: dark brown (0.147, 0.117, 0.089) → tan (0.328, 0.235, 0.156)
    //   Alpha: smoothstep bell curve 0→0.5→0
    //   Texture: 8×8 flipbook, 35 frames animated
    //   Orientation: Camera facing
    //   Drag: 2.0, slight upward acceleration

    // Color: dark brown → tan with smoothstep bell alpha curve (0→0.5→0)
    // CPU uses: color lerp over t, alpha = smoothstep bell peaking at 0.5
    let mut smoke_color_gradient = bevy_hanabi::Gradient::new();
    smoke_color_gradient.add_key(0.0, Vec4::new(0.147, 0.117, 0.089, 0.0));   // Dark brown, alpha=0
    smoke_color_gradient.add_key(0.25, Vec4::new(0.192, 0.147, 0.106, 0.25)); // 25% lerp, alpha rising
    smoke_color_gradient.add_key(0.5, Vec4::new(0.237, 0.176, 0.123, 0.5));   // 50% lerp, peak alpha
    smoke_color_gradient.add_key(0.75, Vec4::new(0.283, 0.206, 0.139, 0.25)); // 75% lerp, alpha falling
    smoke_color_gradient.add_key(1.0, Vec4::new(0.328, 0.235, 0.156, 0.0));   // Tan, alpha=0

    // Size: grow from base 0.75m using ease-out curve (1-(1-t)²), 1x→3x
    // ease_out(t) = 1 - (1-t)², scale = initial * (1 + 2*ease_out)
    // t=0.0: ease=0.0, scale=0.75 | t=0.25: ease=0.44, scale=1.41
    // t=0.5: ease=0.75, scale=1.88 | t=0.75: ease=0.94, scale=2.16 | t=1.0: ease=1.0, scale=2.25
    let mut smoke_size_gradient = bevy_hanabi::Gradient::new();
    smoke_size_gradient.add_key(0.0, Vec3::splat(0.75));   // 1x
    smoke_size_gradient.add_key(0.25, Vec3::splat(1.41));  // ease-out at 25%
    smoke_size_gradient.add_key(0.5, Vec3::splat(1.88));   // ease-out at 50%
    smoke_size_gradient.add_key(0.75, Vec3::splat(2.16));  // ease-out at 75%
    smoke_size_gradient.add_key(1.0, Vec3::splat(2.25));   // 3x

    let writer_smoke = ExprWriter::new();

    // Position: spawn at origin with slight Y offset
    let smoke_init_pos = SetAttributeModifier::new(
        Attribute::POSITION,
        writer_smoke.lit(Vec3::new(0.0, 0.1, 0.0)).expr()
    );

    // Velocity: spread on XZ plane (world-space equivalent of screen-local)
    // ±12 m/s horizontal, minimal Y
    let smoke_vel_x = (writer_smoke.rand(ScalarType::Float) * writer_smoke.lit(24.0)) - writer_smoke.lit(12.0);
    let smoke_vel_y = writer_smoke.rand(ScalarType::Float) * writer_smoke.lit(2.0);  // Slight upward
    let smoke_vel_z = (writer_smoke.rand(ScalarType::Float) * writer_smoke.lit(24.0)) - writer_smoke.lit(12.0);
    let smoke_velocity = smoke_vel_x.vec3(smoke_vel_y, smoke_vel_z);
    let smoke_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, smoke_velocity.expr());

    let smoke_init_age = SetAttributeModifier::new(Attribute::AGE, writer_smoke.lit(0.0).expr());
    // Lifetime: 0.8-2.5s
    let smoke_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_smoke.lit(0.8) + writer_smoke.rand(ScalarType::Float) * writer_smoke.lit(1.7)).expr()
    );

    // Drag and upward acceleration (UE5: drag 2.0, accel Y=0.5)
    let smoke_drag = LinearDragModifier::new(writer_smoke.lit(2.0).expr());
    let smoke_accel = AccelModifier::new(writer_smoke.lit(Vec3::new(0.0, 0.5, 0.0)).expr());

    let smoke_texture_slot = writer_smoke.lit(0u32).expr();
    let mut smoke_module = writer_smoke.finish();
    smoke_module.add_texture_slot("smoke_texture");

    let ground_smoke_effect = effects.add(
        EffectAsset::new(32, SpawnerSettings::once(12.0.into()), smoke_module)
            .with_name("ground_explosion_smoke")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(smoke_init_pos)
            .init(smoke_init_vel)
            .init(smoke_init_age)
            .init(smoke_init_lifetime)
            .update(smoke_drag)
            .update(smoke_accel)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))  // Camera facing
            .render(ParticleTextureModifier {
                texture_slot: smoke_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,
            })
            .render(FlipbookModifier { sprite_grid_size: UVec2::new(8, 8) })
            .render(ColorOverLifetimeModifier::new(smoke_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: smoke_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU WISP PUFFS ===
    // Replaces CPU wisp entities (3 per explosion) with single GPU effect
    // CPU behavior (from update_wisp_scale, update_wisp_alpha):
    //   Count: 3 particles
    //   Size: 0→5× using SMOOTHSTEP curve: t²(3-2t), base ~2m (0.8-1.8m × 1.5)
    //   Velocity: upward (3-6 m/s × 1.5), horizontal spread (±1 m/s × 1.5)
    //   Gravity: 9.8 m/s²
    //   Lifetime: 1.0-2.0s
    //   Color: dark grey (0.15, 0.12, 0.10)
    //   Alpha: TWO-PHASE: 4→1 fast (0-20%), then 1→0 linear (20-100%)
    //   Texture: 8×8 flipbook, 64 frames animated
    //   Orientation: Camera facing

    // Color: dark grey with two-phase alpha (4→1 fast, then 1→0 linear)
    // CPU: alpha = if t < 0.2 { 4.0 - (t/0.2)*3.0 } else { 1.0 - (t-0.2)/0.8 }
    let mut wisp_color_gradient = bevy_hanabi::Gradient::new();
    wisp_color_gradient.add_key(0.0, Vec4::new(0.15, 0.12, 0.10, 4.0));   // Start bright (HDR)
    wisp_color_gradient.add_key(0.2, Vec4::new(0.15, 0.12, 0.10, 1.0));   // Fast drop to 1.0
    wisp_color_gradient.add_key(1.0, Vec4::new(0.15, 0.12, 0.10, 0.0));   // Linear fade to 0

    // Size: 0→5× using SMOOTHSTEP curve (NOT cubic ease-in!)
    // smoothstep(t) = t² * (3 - 2t), final_size = base * smoothstep(t) * 5
    // Base ~2m (midpoint of 0.8-1.8m × 1.5), final = 10m
    // t=0.0: ss=0.0, size=0 | t=0.2: ss=0.104, size=1.04
    // t=0.4: ss=0.352, size=3.52 | t=0.6: ss=0.648, size=6.48
    // t=0.8: ss=0.896, size=8.96 | t=1.0: ss=1.0, size=10
    let mut wisp_size_gradient = bevy_hanabi::Gradient::new();
    wisp_size_gradient.add_key(0.0, Vec3::splat(0.0));     // Start at zero
    wisp_size_gradient.add_key(0.2, Vec3::splat(1.04));    // smoothstep(0.2) * 10
    wisp_size_gradient.add_key(0.4, Vec3::splat(3.52));    // smoothstep(0.4) * 10
    wisp_size_gradient.add_key(0.6, Vec3::splat(6.48));    // smoothstep(0.6) * 10
    wisp_size_gradient.add_key(0.8, Vec3::splat(8.96));    // smoothstep(0.8) * 10
    wisp_size_gradient.add_key(1.0, Vec3::splat(10.0));    // 5× final (2m base × 5)

    let writer_wisp = ExprWriter::new();

    // Position: spawn slightly above ground
    let wisp_init_pos = SetAttributeModifier::new(
        Attribute::POSITION,
        writer_wisp.lit(Vec3::new(0.0, 0.5, 0.0)).expr()
    );

    // Velocity: gentle upward (3-6 m/s), slight horizontal spread (±1 m/s)
    // Apply 1.5× scale modifier
    let wisp_vel_x = (writer_wisp.rand(ScalarType::Float) * writer_wisp.lit(3.0)) - writer_wisp.lit(1.5);  // ±1.5
    let wisp_vel_y = writer_wisp.lit(4.5) + writer_wisp.rand(ScalarType::Float) * writer_wisp.lit(4.5);    // 4.5-9 (3-6 × 1.5)
    let wisp_vel_z = (writer_wisp.rand(ScalarType::Float) * writer_wisp.lit(3.0)) - writer_wisp.lit(1.5);  // ±1.5
    let wisp_velocity = wisp_vel_x.vec3(wisp_vel_y, wisp_vel_z);
    let wisp_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, wisp_velocity.expr());

    let wisp_init_age = SetAttributeModifier::new(Attribute::AGE, writer_wisp.lit(0.0).expr());
    // Lifetime: 1.0-2.0s
    let wisp_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_wisp.lit(1.0) + writer_wisp.rand(ScalarType::Float) * writer_wisp.lit(1.0)).expr()
    );

    // Gravity: 9.8 m/s² downward
    let wisp_gravity = AccelModifier::new(writer_wisp.lit(Vec3::new(0.0, -9.8, 0.0)).expr());

    let wisp_texture_slot = writer_wisp.lit(0u32).expr();
    let mut wisp_module = writer_wisp.finish();
    wisp_module.add_texture_slot("wisp_texture");

    let ground_wisp_effect = effects.add(
        EffectAsset::new(8, SpawnerSettings::once(3.0.into()), wisp_module)
            .with_name("ground_explosion_wisp")
            .with_alpha_mode(bevy_hanabi::AlphaMode::Blend)
            .init(wisp_init_pos)
            .init(wisp_init_vel)
            .init(wisp_init_age)
            .init(wisp_init_lifetime)
            .update(wisp_gravity)
            .render(OrientModifier::new(OrientMode::FaceCameraPosition))  // Camera facing
            .render(ParticleTextureModifier {
                texture_slot: wisp_texture_slot,
                sample_mapping: ImageSampleMapping::Modulate,
            })
            .render(FlipbookModifier { sprite_grid_size: UVec2::new(8, 8) })
            .render(ColorOverLifetimeModifier::new(wisp_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: wisp_size_gradient, screen_space_size: false })
    );

    commands.insert_resource(ExplosionParticleEffects {
        debris_effect: debris_effect.clone(),
        sparks_effect: sparks_effect.clone(),
        smoke_effect: smoke_effect.clone(),
        shield_impact_effect,
        mass_explosion_effect,
        unit_death_flash,
        ground_sparks_effect: ground_sparks_effect.clone(),
        ground_flash_sparks_effect: ground_flash_sparks_effect.clone(),
        ground_sparks_texture: ground_sparks_texture.clone(),
        ground_parts_effect: ground_parts_effect.clone(),
        ground_parts_texture: ground_parts_texture.clone(),
        ground_dirt_effect: ground_dirt_effect.clone(),
        ground_vdirt_effect: ground_vdirt_effect.clone(),
        ground_dirt_texture: ground_dirt_texture.clone(),
        ground_fireball_effect: ground_fireball_effect.clone(),
        ground_fireball_texture: ground_fireball_texture.clone(),
        ground_fireball_secondary_texture: ground_fireball_secondary_texture.clone(),
        debug_fireball_effect: debug_fireball_effect.clone(),
        ground_dust_effect: ground_dust_effect.clone(),
        ground_dust_texture: ground_dust_texture.clone(),
        ground_smoke_effect: ground_smoke_effect.clone(),
        ground_smoke_texture: ground_smoke_texture.clone(),
        ground_wisp_effect: ground_wisp_effect.clone(),
        ground_wisp_texture: ground_wisp_texture.clone(),
    });

    // Warmup: Spawn particles far below the map to prime the GPU pipeline
    // MUST use Visibility::Visible for GPU to compile the effect shader
    // Position is far below map (-1000 Y) so they're not seen
    let warmup_pos = Vec3::new(0.0, -1000.0, 0.0);
    commands.spawn((
        ParticleEffect::new(debris_effect),
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,  // Must be Visible for GPU compilation
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },  // Longer duration to ensure compilation
        Name::new("WarmupDebris"),
    ));
    commands.spawn((
        ParticleEffect::new(sparks_effect),
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,  // Must be Visible for GPU compilation
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupSparks"),
    ));
    commands.spawn((
        ParticleEffect::new(smoke_effect),
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,  // Must be Visible for GPU compilation
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupSmoke"),
    ));
    // Warmup for ground explosion GPU sparks (with texture binding)
    commands.spawn((
        ParticleEffect::new(ground_sparks_effect),
        EffectMaterial {
            images: vec![ground_sparks_texture.clone()],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundSparks"),
    ));
    commands.spawn((
        ParticleEffect::new(ground_flash_sparks_effect),
        EffectMaterial {
            images: vec![ground_sparks_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundFlashSparks"),
    ));
    commands.spawn((
        ParticleEffect::new(ground_parts_effect),
        EffectMaterial {
            images: vec![ground_parts_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundParts"),
    ));
    // Warmup for ground explosion GPU dirt debris
    commands.spawn((
        ParticleEffect::new(ground_dirt_effect),
        EffectMaterial {
            images: vec![ground_dirt_texture.clone()],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundDirt"),
    ));
    commands.spawn((
        ParticleEffect::new(ground_vdirt_effect),
        EffectMaterial {
            images: vec![ground_dirt_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundVDirt"),
    ));
    // Warmup for ground explosion GPU fireballs
    commands.spawn((
        ParticleEffect::new(ground_fireball_effect),
        EffectMaterial {
            images: vec![ground_fireball_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundFireball"),
    ));
    // Warmup for ground explosion GPU dust ring
    commands.spawn((
        ParticleEffect::new(ground_dust_effect),
        EffectMaterial {
            images: vec![ground_dust_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundDust"),
    ));
    // Warmup for ground explosion GPU smoke cloud
    commands.spawn((
        ParticleEffect::new(ground_smoke_effect),
        EffectMaterial {
            images: vec![ground_smoke_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundSmoke"),
    ));
    // Warmup for ground explosion GPU wisp puffs
    commands.spawn((
        ParticleEffect::new(ground_wisp_effect),
        EffectMaterial {
            images: vec![ground_wisp_texture],
        },
        Transform::from_translation(warmup_pos).with_scale(Vec3::splat(0.001)),
        Visibility::Visible,
        ParticleEffectLifetime { spawn_time: 0.0, duration: 0.5 },
        Name::new("WarmupGroundWisp"),
    ));

    info!("✅ Particle effects ready (with warmup)");
}

/// Spawns a complete particle explosion effect at the given location
/// This combines debris, sparks, and smoke for a full effect
pub fn spawn_explosion_particles(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32, // Scale multiplier for the effect
    current_time: f64, // Current elapsed time from Time resource
) {
    trace!("💥 PARTICLES: Spawning explosion particles at {:?} with scale {}", position, scale);

    // Spawn debris particles
    commands.spawn((
        ParticleEffect::new(particle_effects.debris_effect.clone()),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 5.0,
        },
        Name::new("ExplosionDebris"),
    ));

    // Spawn sparks particles
    commands.spawn((
        ParticleEffect::new(particle_effects.sparks_effect.clone()),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 3.0,
        },
        Name::new("ExplosionSparks"),
    ));

    // Spawn smoke particles
    commands.spawn((
        ParticleEffect::new(particle_effects.smoke_effect.clone()),
        Transform::from_translation(position + Vec3::new(0.0, 2.0 * scale, 0.0))
            .with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 6.0,
        },
        Name::new("ExplosionSmoke"),
    ));
}

/// Spawns particles for turret explosions
/// More sparks/flames than standard explosion for visual impact
pub fn spawn_turret_explosion_particles(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    trace!("💥 TURRET PARTICLES: Spawning turret explosion at {:?}", position);

    // Spawn debris particles
    commands.spawn((
        ParticleEffect::new(particle_effects.debris_effect.clone()),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 5.0,
        },
        Name::new("TurretExplosionDebris"),
    ));

    // Spawn multiple spark effects for more flames
    for i in 0..3 {
        let offset = Vec3::new(
            (i as f32 - 1.0) * 0.5,
            i as f32 * 0.3,
            (i as f32 - 1.0) * 0.3,
        );
        commands.spawn((
            ParticleEffect::new(particle_effects.sparks_effect.clone()),
            Transform::from_translation(position + offset)
                .with_scale(Vec3::splat(scale * (1.0 + i as f32 * 0.2))),
            Visibility::Visible,
            ParticleEffectLifetime {
                spawn_time: current_time,
                duration: 3.0,
            },
            Name::new("TurretExplosionSparks"),
        ));
    }

    // Spawn smoke particles
    commands.spawn((
        ParticleEffect::new(particle_effects.smoke_effect.clone()),
        Transform::from_translation(position + Vec3::new(0.0, 2.0 * scale, 0.0))
            .with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 6.0,
        },
        Name::new("TurretExplosionSmoke"),
    ));
}

/// Spawns particles for smaller unit explosions
/// Uses fewer particles and smaller scale for better performance
#[allow(dead_code)]
pub fn spawn_unit_explosion_particles(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    current_time: f64, // Current elapsed time from Time resource
) {
    trace!("💥 UNIT PARTICLES: Spawning unit explosion particles at {:?}", position);

    // PERFORMANCE: Only spawn 1 effect per unit (sparks only) to reduce entity count
    // Each ParticleEffect entity has significant per-entity GPU overhead in Hanabi
    commands.spawn((
        ParticleEffect::new(particle_effects.sparks_effect.clone()),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(0.25)),
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 2.0,
        },
        Name::new("UnitSparks"),
    ));
}

/// Spawns particles for tower explosions
/// Uses full effect with maximum intensity
pub fn spawn_tower_explosion_particles(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    current_time: f64, // Current elapsed time from Time resource
) {
    spawn_explosion_particles(commands, particle_effects, position, 4.0, current_time); // Large scale for towers
}

/// Spawns a single mass explosion effect covering the tower destruction radius
/// Replaces 1000+ individual unit explosion entities with ONE Hanabi effect
pub fn spawn_mass_explosion(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    current_time: f64,
) {
    info!("💥 MASS EXPLOSION: Spawning at {:?} (radius={})", position, crate::constants::TOWER_DESTRUCTION_RADIUS);

    commands.spawn((
        ParticleEffect::new(particle_effects.mass_explosion_effect.clone()),
        Transform::from_translation(position),
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 5.0, // Cleanup after all particles fade (max lifetime 3s + buffer)
        },
        Name::new("MassExplosion"),
    ));
}

/// Spawns a death flash effect at unit position
/// Small, quick burst effect for individual unit deaths
#[allow(dead_code)]
pub fn spawn_unit_death_flash(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    current_time: f64,
) {
    // Use sparks_effect which WORKS in pending_explosion_system
    // Scale 0.4 for small unit-sized explosion
    commands.spawn((
        ParticleEffect::new(particle_effects.sparks_effect.clone()),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(0.4)),
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 2.0,
        },
        Name::new("UnitDeathFlash"),
    ));
}

/// Spawns particles for shield impacts
/// Small burst effect when lasers hit the shield
#[allow(dead_code)]
pub fn spawn_shield_impact_particles(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    current_time: f64,
) {
    // Use the dedicated shield impact effect (cyan burst)
    commands.spawn((
        ParticleEffect::new(particle_effects.shield_impact_effect.clone()),
        Transform::from_translation(position)
            .with_scale(Vec3::splat(2.0)), // Larger scale to make it more visible
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 2.0,
        },
        Name::new("ShieldImpact"),
    ));
}

/// DEBUG: System to inspect Hanabi entity components
pub fn debug_hanabi_entities(
    query: Query<(
        Entity,
        &Name,
        &Transform,
        &ParticleEffect,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&ViewVisibility>,
        Option<&CompiledParticleEffect>,
    )>,
) {
    for (entity, name, transform, _effect, vis, inherited_vis, view_vis, compiled) in query.iter() {
        let name_str = name.as_str();
        if name_str.contains("DeathFlash") || name_str.contains("MassExplosion")
            || name_str.contains("ExplosionDebris") || name_str.contains("ExplosionSparks") || name_str.contains("ExplosionSmoke") {
            // Uncomment for debugging hanabi particle visibility issues
            // info!(
            //     "🔍 HANABI {:?} '{}': pos={:?} Vis={:?} InheritedVis={:?} ViewVis={:?} Compiled={}",
            //     entity,
            //     name_str,
            //     transform.translation,
            //     vis.map(|v| format!("{:?}", v)),
            //     inherited_vis.map(|_| "Some"),
            //     view_vis.map(|_| "Some"),
            //     compiled.is_some()
            // );
            let _ = (entity, name_str, transform, vis, inherited_vis, view_vis, compiled); // suppress warnings
        }
    }
}

/// System to cleanup particle effects after their lifetime expires
/// Uses spawn time comparison instead of per-frame timer ticking for better performance
fn cleanup_finished_particle_effects(
    mut commands: Commands,
    query: Query<(Entity, &ParticleEffectLifetime)>,
    time: Res<Time>,
) {
    let start = std::time::Instant::now();
    let current_time = time.elapsed_secs_f64();
    let entity_count = query.iter().count();

    // let mut despawned = 0;
    for (entity, lifetime) in query.iter() {
        let elapsed = (current_time - lifetime.spawn_time) as f32;

        if elapsed >= lifetime.duration {
            commands.entity(entity).despawn();
            // despawned += 1;
        }
    }

    // Uncomment for debugging hanabi performance
    // let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    // let frame_time_ms = time.delta_secs() * 1000.0;
    // if entity_count > 0 {
    //     info!("📊 HANABI STATS: {} entities, {} despawned, {:.2}ms CPU, {:.2}ms frame_time ({:.0} FPS)",
    //           entity_count, despawned, elapsed_ms, frame_time_ms, 1000.0 / frame_time_ms);
    // }
    let _ = (start, entity_count); // suppress warnings
}

/// Spawns GPU-based particles for ground explosions
/// Replaces CPU spark and parts entities with 3 GPU particle effects:
/// - Sparks: 30-60 CPU entities → 1 GPU effect
/// - Flash Sparks: 20-50 CPU entities → 1 GPU effect
/// - Parts Debris: 50-75 CPU entities → 1 GPU effect
/// Total reduction: ~100-185 entities → 3 entities per explosion
pub fn spawn_ground_explosion_gpu_sparks(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // Generate unique seeds from current time to ensure randomization per spawn
    let seed = (current_time * 1000000.0) as u32;

    // GPU Sparks (replaces spawn_sparks - 30-60 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_sparks_effect.clone(),
            prng_seed: Some(seed),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_sparks_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 3.0,
        },
        Name::new("GE_GPU_Sparks"),
    ));

    // GPU Flash Sparks (replaces spawn_flash_sparks - 20-50 entities → 1 GPU effect)
    // Use different seed to avoid correlation
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_flash_sparks_effect.clone(),
            prng_seed: Some(seed.wrapping_add(12345)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_sparks_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 2.0,
        },
        Name::new("GE_GPU_FlashSparks"),
    ));

    // GPU Parts Debris (replaces spawn_parts - 50-75 entities → 1 GPU effect)
    // Uses sprite sheet of baked 3D debris meshes
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_parts_effect.clone(),
            prng_seed: Some(seed.wrapping_add(67890)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_parts_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 2.0,
        },
        Name::new("GE_GPU_Parts"),
    ));
}

/// Spawns GPU-based dirt particles for ground explosions
/// Replaces CPU dirt debris entities with 2 GPU particle effects:
/// - Dirt Debris: 35 CPU entities → 1 GPU effect (camera-facing, gravity)
/// - Velocity Dirt: 10-15 CPU entities → 1 GPU effect (velocity-aligned, no gravity)
/// Total reduction: ~45-50 entities → 2 entities per explosion
pub fn spawn_ground_explosion_gpu_dirt(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // Generate unique seeds from current time to ensure randomization per spawn
    let seed = (current_time * 1000000.0) as u32;

    // GPU Dirt Debris (replaces spawn_dirt_debris - 35 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_dirt_effect.clone(),
            prng_seed: Some(seed.wrapping_add(111111)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_dirt_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 5.0, // Longer lifetime for dirt (1-4s particles)
        },
        Name::new("GE_GPU_Dirt"),
    ));

    // GPU Velocity Dirt (replaces spawn_velocity_dirt - 10-15 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_vdirt_effect.clone(),
            prng_seed: Some(seed.wrapping_add(222222)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_dirt_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 3.0, // Shorter lifetime for velocity dirt (0.8-1.7s particles)
        },
        Name::new("GE_GPU_VDirt"),
    ));
}

/// Spawns GPU fireball particles for ground explosions
/// Replaces CPU main_fireball (9-17) + secondary_fireball (7-13) → 1 GPU effect with 25 particles
pub fn spawn_ground_explosion_gpu_fireballs(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // Generate unique seed from current time
    let seed = (current_time * 1000000.0) as u32;

    // GPU Fireballs (replaces CPU main + secondary fireball - ~16-30 entities → 1 GPU effect)
    // Note: Position offset to simulate bottom pivot (fireball expands from bottom)
    // CPU bottom pivot moved transform down by half height, we approximate with Y offset
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_fireball_effect.clone(),
            prng_seed: Some(seed.wrapping_add(333333)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_fireball_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 2.0, // 1.5s particles + buffer
        },
        Name::new("GE_GPU_Fireball"),
    ));
}

/// Spawn GPU dust ring effect (replaces CPU dust_ring - 2-3 entities → 1 GPU effect)
/// Short-lived dust particles that shoot upward in a cone, velocity-aligned
pub fn spawn_ground_explosion_gpu_dust(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // Generate unique seed from current time
    let seed = (current_time * 1000000.0) as u32;

    // GPU Dust Ring (replaces CPU dust_ring - 2-3 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_dust_effect.clone(),
            prng_seed: Some(seed.wrapping_add(444444)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_dust_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 1.0, // 0.1-0.5s particles + buffer
        },
        Name::new("GE_GPU_Dust"),
    ));
}

/// Spawn GPU smoke cloud for ground explosion
/// Replaces CPU smoke entities (10-15) with 1 GPU entity
pub fn spawn_ground_explosion_gpu_smoke(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // Generate unique seed from current time
    let seed = (current_time * 1000000.0) as u32;

    // GPU Smoke Cloud (replaces CPU smoke_cloud - 10-15 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_smoke_effect.clone(),
            prng_seed: Some(seed.wrapping_add(555555)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_smoke_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 4.0, // 0.8-2.5s particles + buffer
        },
        Name::new("GE_GPU_Smoke"),
    ));
}

/// Spawn GPU wisp puffs for ground explosion
/// Replaces CPU wisp entities (3) with 1 GPU entity
pub fn spawn_ground_explosion_gpu_wisp(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // Generate unique seed from current time
    let seed = (current_time * 1000000.0) as u32;

    // GPU Wisp Puffs (replaces CPU wisps - 3 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect {
            handle: particle_effects.ground_wisp_effect.clone(),
            prng_seed: Some(seed.wrapping_add(666666)),
        },
        EffectMaterial {
            images: vec![particle_effects.ground_wisp_texture.clone()],
        },
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Visible,
        ParticleEffectLifetime {
            spawn_time: current_time,
            duration: 3.0, // 1.0-2.0s particles + buffer
        },
        Name::new("GE_GPU_Wisp"),
    ));
}
