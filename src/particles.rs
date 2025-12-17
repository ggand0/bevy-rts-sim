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
    // Ground explosion GPU parts debris (replaces CPU mesh entities)
    pub ground_parts_effect: Handle<EffectAsset>,
    pub ground_parts_texture: Handle<Image>,
    // Ground explosion GPU dirt debris (replaces CPU dirt entities)
    pub ground_dirt_effect: Handle<EffectAsset>,
    pub ground_vdirt_effect: Handle<EffectAsset>,
    pub ground_dirt_texture: Handle<Image>,
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

    // Size gradient for velocity-aligned particles (elongated)
    let mut vdirt_size_gradient = bevy_hanabi::Gradient::new();
    vdirt_size_gradient.add_key(0.0, Vec3::splat(1.0));
    vdirt_size_gradient.add_key(1.0, Vec3::splat(0.3));

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
    // Non-uniform size: base 1.0-2.0m, X: 0.5-1.0, Y: 0.6-1.2
    // For AlongVelocity: X = along velocity (elongated streak), Y = perpendicular width
    // Scale up 2x to match CPU visual appearance (velocity-aligned billboards render smaller)
    let vdirt_base_size = writer_vdirt.lit(2.0) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(2.0);
    let vdirt_scale_x = writer_vdirt.lit(0.5) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(0.5);
    let vdirt_scale_y = writer_vdirt.lit(0.6) + writer_vdirt.rand(ScalarType::Float) * writer_vdirt.lit(0.6);
    let vdirt_size = (vdirt_base_size.clone() * vdirt_scale_x).vec3(vdirt_base_size.clone() * vdirt_scale_y, vdirt_base_size);
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
