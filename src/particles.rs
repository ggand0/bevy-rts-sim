// Particle effects system using Bevy Hanabi
// Provides debris, sparks, and smoke particles for explosions
use bevy::prelude::*;
use bevy_hanabi::prelude::*;

pub struct ParticleEffectsPlugin;

impl Plugin for ParticleEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_systems(Startup, setup_particle_effects)
            .add_systems(Update, (cleanup_finished_particle_effects, debug_hanabi_entities));
    }
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
}

fn setup_particle_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    asset_server: Res<AssetServer>,
) {
    info!("🎆 Setting up particle effects...");

    // Load flare texture for ground explosion sparks
    let ground_sparks_texture: Handle<Image> = asset_server.load("textures/premium/ground_explosion/flare.png");

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
    spark_color_gradient.add_key(0.0, Vec4::new(12.5, 6.75, 1.9, 1.0));    // Hot HDR orange-yellow
    spark_color_gradient.add_key(0.2, Vec4::new(8.0, 4.3, 1.2, 0.9));      // Still hot
    spark_color_gradient.add_key(0.4, Vec4::new(3.5, 1.8, 0.5, 0.75));     // Cooling
    spark_color_gradient.add_key(0.55, Vec4::new(0.25, 0.0125, 0.0, 0.5)); // Cooled to dim red
    spark_color_gradient.add_key(0.75, Vec4::new(0.18, 0.008, 0.0, 0.25)); // Fading
    spark_color_gradient.add_key(1.0, Vec4::new(0.125, 0.005, 0.0, 0.0));  // Gone

    let mut spark_size_gradient = bevy_hanabi::Gradient::new();
    spark_size_gradient.add_key(0.0, Vec3::splat(1.0));
    spark_size_gradient.add_key(1.0, Vec3::splat(0.3));

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
    // This creates an upward hemisphere cone.
    //
    // GPU approach: Generate random direction in full sphere, then make Y positive
    // to restrict to upper hemisphere.
    let rand_dir = writer_spark.rand(VectorType::VEC3F) * writer_spark.lit(2.0) - writer_spark.lit(1.0);
    let rand_dir_normalized = rand_dir.normalized();
    // Force Y component positive (upward hemisphere)
    let dir_x = rand_dir_normalized.clone().x();
    let dir_y = rand_dir_normalized.clone().y().abs();  // abs() = upward only
    let dir_z = rand_dir_normalized.z();
    let hemisphere_dir = dir_x.vec3(dir_y, dir_z).normalized();
    // Random speed 15-37.5 m/s (matching CPU's rng.gen_range(15.0..37.5))
    let spark_speed = writer_spark.lit(15.0) + writer_spark.rand(ScalarType::Float) * writer_spark.lit(22.5);
    let spark_velocity = hemisphere_dir * spark_speed;
    let spark_init_vel = SetAttributeModifier::new(Attribute::VELOCITY, spark_velocity.expr());

    let spark_init_age = SetAttributeModifier::new(Attribute::AGE, writer_spark.lit(0.0).expr());
    let spark_init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        (writer_spark.lit(0.5) + writer_spark.rand(ScalarType::Float) * writer_spark.lit(1.5)).expr()
    );
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
                sample_mapping: ImageSampleMapping::ModulateOpacityFromR,
            })
            .render(ColorOverLifetimeModifier::new(spark_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: spark_size_gradient, screen_space_size: false })
    );

    // === GROUND EXPLOSION GPU FLASH SPARKS ===
    // Replaces CPU flash spark entities (20-50 per explosion) with single GPU effect
    // UE5 spec: Ring spawn, 100° cone, deceleration physics, "shooting star" elongation
    //
    // CPU behavior (from update_spark_l_color):
    //   Color: Constant HDR orange (2.5, 1.625, 0.975), only alpha fades 1→0
    //   Velocity: 100° cone (wider than 90° hemisphere), ring spawn at equator
    //   Deceleration: (-0.25, -1.0, -0.5) * 10 = (-2.5, -10, -5) m/s²
    let mut flash_color_gradient = bevy_hanabi::Gradient::new();
    // Constant HDR orange, alpha fades linearly
    flash_color_gradient.add_key(0.0, Vec4::new(2.5, 1.625, 0.975, 1.0));
    flash_color_gradient.add_key(0.3, Vec4::new(2.5, 1.625, 0.975, 0.7));
    flash_color_gradient.add_key(0.6, Vec4::new(2.5, 1.625, 0.975, 0.4));
    flash_color_gradient.add_key(1.0, Vec4::new(2.5, 1.625, 0.975, 0.0));

    // "Shooting star" effect: very elongated at start, normalizes over time
    // CPU: t=0: tiny → t=0.05: 0.3×50 → t=0.5: 5×3 → t=1.0: 5×3
    // In bevy_hanabi with AlongVelocity, X is perpendicular, Y is along velocity
    let mut flash_size_gradient = bevy_hanabi::Gradient::new();
    flash_size_gradient.add_key(0.0, Vec3::new(0.06, 10.0, 1.0));  // Very elongated (shooting star)
    flash_size_gradient.add_key(0.05, Vec3::new(0.06, 10.0, 1.0)); // Hold elongation briefly
    flash_size_gradient.add_key(0.2, Vec3::new(0.3, 2.0, 1.0));    // Shrinking
    flash_size_gradient.add_key(0.5, Vec3::new(1.0, 0.6, 1.0));    // Normalized
    flash_size_gradient.add_key(1.0, Vec3::new(1.0, 0.6, 1.0));    // Hold

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
    // cos(100°) ≈ -0.17, so Y can be slightly negative
    //
    // GPU approach: Generate hemisphere direction, but allow slight downward bias
    let rand_dir_f = writer_flash.rand(VectorType::VEC3F) * writer_flash.lit(2.0) - writer_flash.lit(1.0);
    let rand_dir_norm_f = rand_dir_f.normalized();
    // For 100° cone: Y can range from -0.17 to 1.0, so we clamp Y to be >= -0.2
    // Simpler: just use abs(Y) * 0.9 + Y * 0.1 to bias upward but allow slight down
    let dir_x_f = rand_dir_norm_f.clone().x();
    let dir_y_raw = rand_dir_norm_f.clone().y();
    // Mix abs(y) and raw y to allow ~100° cone: 80% upward bias, 20% raw
    let dir_y_f = dir_y_raw.clone().abs() * writer_flash.lit(0.8) + dir_y_raw * writer_flash.lit(0.2);
    let dir_z_f = rand_dir_norm_f.z();
    let cone_dir = dir_x_f.vec3(dir_y_f, dir_z_f).normalized();
    // Speed 4-55 m/s (matching CPU's rng.gen_range(4.0..55.0))
    let flash_speed = writer_flash.lit(4.0) + writer_flash.rand(ScalarType::Float) * writer_flash.lit(51.0);
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

    // CPU flash sparks: NO gravity (gravity: 0.0), only deceleration via update_spark_l_physics
    // The deceleration gradually slows particles down, not constant downward acceleration.
    // Use LINEAR DRAG to approximate the velocity-dependent slowdown behavior.
    // Higher drag = faster slowdown. CPU multiplies velocity by decel * dt * 10 = small reduction each frame.
    let flash_update_drag = LinearDragModifier::new(writer_flash.lit(4.0).expr());

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
            .update(flash_update_drag)
            // No AccelModifier - CPU flash sparks have no gravity, only drag/deceleration
            .render(OrientModifier::new(OrientMode::AlongVelocity))
            .render(ParticleTextureModifier {
                texture_slot: flash_texture_slot,
                sample_mapping: ImageSampleMapping::ModulateOpacityFromR,
            })
            .render(ColorOverLifetimeModifier::new(flash_color_gradient))
            .render(SizeOverLifetimeModifier { gradient: flash_size_gradient, screen_space_size: false })
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

/// Spawns GPU-based sparks for ground explosions
/// Replaces 30-60 CPU spark entities with 2 GPU particle effects
pub fn spawn_ground_explosion_gpu_sparks(
    commands: &mut Commands,
    particle_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    // GPU Sparks (replaces spawn_sparks - 30-60 entities → 1 GPU effect)
    commands.spawn((
        ParticleEffect::new(particle_effects.ground_sparks_effect.clone()),
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
    commands.spawn((
        ParticleEffect::new(particle_effects.ground_flash_sparks_effect.clone()),
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
}
