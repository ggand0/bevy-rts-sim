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
}

fn setup_particle_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    info!("🎆 Setting up particle effects...");

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
            .render(ColorOverLifetimeModifier::new(color_gradient6))
            .render(SizeOverLifetimeModifier { gradient: size_gradient6, screen_space_size: false })
    );

    commands.insert_resource(ExplosionParticleEffects {
        debris_effect,
        sparks_effect,
        smoke_effect,
        shield_impact_effect,
        mass_explosion_effect,
        unit_death_flash,
    });

    info!("✅ Particle effects ready!");
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
        EffectSpawner::new(&SpawnerSettings::once(5.0.into())),
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
        EffectSpawner::new(&SpawnerSettings::once(5.0.into())),
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
        EffectSpawner::new(&SpawnerSettings::once(50.0.into())),
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
            info!(
                "🔍 HANABI {:?} '{}': pos={:?} Vis={:?} InheritedVis={:?} ViewVis={:?} Compiled={}",
                entity,
                name_str,
                transform.translation,
                vis.map(|v| format!("{:?}", v)),
                inherited_vis.map(|_| "Some"),
                view_vis.map(|_| "Some"),
                compiled.is_some()
            );
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

    let mut despawned = 0;
    for (entity, lifetime) in query.iter() {
        let elapsed = (current_time - lifetime.spawn_time) as f32;

        if elapsed >= lifetime.duration {
            commands.entity(entity).despawn();
            despawned += 1;
        }
    }

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let frame_time_ms = time.delta_secs() * 1000.0;
    if entity_count > 0 {
        info!("📊 HANABI STATS: {} entities, {} despawned, {:.2}ms CPU, {:.2}ms frame_time ({:.0} FPS)",
              entity_count, despawned, elapsed_ms, frame_time_ms, 1000.0 / frame_time_ms);
    }
}
