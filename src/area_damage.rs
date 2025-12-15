// Area damage system - handles explosion damage zones and knockback physics
// Three zones: Core (instant death), Mid (RNG death), Rim (knockback only)

use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::explosion_shader::{spawn_custom_shader_explosion, ExplosionAssets, ExplosionMaterial};
use crate::terrain::TerrainHeightmap;
use crate::types::*;

/// Process area damage events - apply death/knockback to units in explosion radius
pub fn area_damage_system(
    mut commands: Commands,
    mut events: EventReader<AreaDamageEvent>,
    spatial_grid: Res<SpatialGrid>,
    mut squad_manager: ResMut<SquadManager>,
    heightmap: Option<Res<TerrainHeightmap>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut explosion_materials: ResMut<Assets<ExplosionMaterial>>,
    time: Res<Time>,
    droid_query: Query<(Entity, &Transform, &BattleDroid), (Without<KnockbackState>, Without<RagdollDeath>)>,
) {
    let mut rng = rand::thread_rng();

    for event in events.read() {
        let core_radius = AREA_DAMAGE_CORE_RADIUS * event.scale;
        let mid_radius = AREA_DAMAGE_MID_RADIUS * event.scale;
        let rim_radius = AREA_DAMAGE_RIM_RADIUS * event.scale;

        // Get nearby droids using spatial grid
        let nearby = spatial_grid.get_nearby_droids(event.position);

        for &entity in &nearby {
            let Ok((_, transform, _droid)) = droid_query.get(entity) else {
                continue;
            };

            let distance = transform.translation.distance(event.position);

            // Skip if outside all zones
            if distance > rim_radius {
                continue;
            }

            // Calculate direction away from explosion center (for knockback/ragdoll)
            let direction = if distance > 0.1 {
                (transform.translation - event.position).normalize()
            } else {
                // Random direction if at center
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                Vec3::new(angle.cos(), 0.0, angle.sin())
            };

            // Get ground height at unit position
            let ground_y = heightmap
                .as_ref()
                .map(|hm| hm.sample_height(transform.translation.x, transform.translation.z))
                .unwrap_or(0.0);

            if distance <= core_radius {
                // CORE ZONE: Instant death
                apply_death_effect(
                    &mut commands,
                    &mut squad_manager,
                    entity,
                    transform.translation,
                    direction,
                    ground_y,
                    explosion_assets.as_deref(),
                    &mut meshes,
                    &mut explosion_materials,
                    time.elapsed_secs_f64(),
                    &mut rng,
                );
            } else if distance <= mid_radius {
                // MID ZONE: RNG death (probability decreases with distance)
                // At core boundary: 80% death, at mid boundary: 20% death
                let t = (distance - core_radius) / (mid_radius - core_radius);
                let death_probability = 0.8 - (t * 0.6);

                if rng.gen::<f32>() < death_probability {
                    apply_death_effect(
                        &mut commands,
                        &mut squad_manager,
                        entity,
                        transform.translation,
                        direction,
                        ground_y,
                        explosion_assets.as_deref(),
                        &mut meshes,
                        &mut explosion_materials,
                        time.elapsed_secs_f64(),
                        &mut rng,
                    );
                } else {
                    // Survived mid zone - apply knockback instead
                    apply_knockback(
                        &mut commands,
                        entity,
                        direction,
                        ground_y,
                        event.scale,
                        &mut rng,
                    );
                }
            } else {
                // RIM ZONE: Knockback only
                apply_knockback(
                    &mut commands,
                    entity,
                    direction,
                    ground_y,
                    event.scale,
                    &mut rng,
                );
            }
        }
    }
}

/// Apply death effect to a unit (50% flipbook, 50% ragdoll)
fn apply_death_effect(
    commands: &mut Commands,
    squad_manager: &mut ResMut<SquadManager>,
    entity: Entity,
    position: Vec3,
    direction: Vec3,
    ground_y: f32,
    explosion_assets: Option<&ExplosionAssets>,
    meshes: &mut ResMut<Assets<Mesh>>,
    explosion_materials: &mut ResMut<Assets<ExplosionMaterial>>,
    current_time: f64,
    rng: &mut impl Rng,
) {
    // Remove from squad
    squad_manager.remove_unit_from_squad(entity);

    // 50/50 split between flipbook explosion and ragdoll death
    if rng.gen::<bool>() {
        // Flipbook explosion - despawn immediately, spawn visual
        commands.entity(entity).try_despawn();

        if let Some(assets) = explosion_assets {
            spawn_custom_shader_explosion(
                commands,
                meshes,
                explosion_materials,
                assets,
                None, // No particle effects for unit deaths
                position,
                2.0,  // radius
                1.0,  // intensity
                EXPLOSION_EFFECT_DURATION,
                false, // not a tower
                current_time,
            );
        }
    } else {
        // Ragdoll death - unit flies away
        let speed = rng.gen_range(RAGDOLL_MIN_SPEED..RAGDOLL_MAX_SPEED);

        // Add upward component for arc trajectory
        let up_component: f32 = rng.gen_range(0.5..0.8);
        let horizontal = (1.0_f32 - up_component * up_component).sqrt();
        let velocity = Vec3::new(
            direction.x * horizontal * speed,
            up_component * speed,
            direction.z * horizontal * speed,
        );

        // Random angular velocity for tumbling
        let angular_velocity = Vec3::new(
            rng.gen_range(-5.0..5.0),
            rng.gen_range(-5.0..5.0),
            rng.gen_range(-5.0..5.0),
        );

        commands.entity(entity).try_insert(RagdollDeath {
            velocity,
            angular_velocity,
            gravity: RAGDOLL_GRAVITY,
            ground_y,
        });
    }
}

/// Apply knockback to a unit (no death)
fn apply_knockback(
    commands: &mut Commands,
    entity: Entity,
    direction: Vec3,
    ground_y: f32,
    scale: f32,
    rng: &mut impl Rng,
) {
    let speed = KNOCKBACK_BASE_SPEED * scale * rng.gen_range(0.8..1.2);

    // Add upward component for arc trajectory
    let up_component: f32 = rng.gen_range(0.3..0.5);
    let horizontal = (1.0_f32 - up_component * up_component).sqrt();
    let velocity = Vec3::new(
        direction.x * horizontal * speed,
        up_component * speed,
        direction.z * horizontal * speed,
    );

    commands.entity(entity).try_insert(KnockbackState {
        velocity,
        gravity: KNOCKBACK_GRAVITY,
        ground_y,
        is_airborne: true,
        stun_timer: KNOCKBACK_STUN_DURATION,
    });
}

/// Update ragdoll death physics - units fly through air and despawn on ground contact
pub fn ragdoll_death_system(
    mut commands: Commands,
    time: Res<Time>,
    heightmap: Option<Res<TerrainHeightmap>>,
    mut query: Query<(Entity, &mut Transform, &mut RagdollDeath)>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, mut ragdoll) in query.iter_mut() {
        // Apply velocity
        transform.translation += ragdoll.velocity * dt;

        // Apply gravity
        ragdoll.velocity.y += ragdoll.gravity * dt;

        // Apply tumble rotation
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            ragdoll.angular_velocity.x * dt,
            ragdoll.angular_velocity.y * dt,
            ragdoll.angular_velocity.z * dt,
        );
        transform.rotation = rotation * transform.rotation;

        // Update ground height at current position
        let current_ground_y = heightmap
            .as_ref()
            .map(|hm| hm.sample_height(transform.translation.x, transform.translation.z))
            .unwrap_or(ragdoll.ground_y);

        // Check ground collision
        if transform.translation.y <= current_ground_y {
            // Hit ground - despawn
            commands.entity(entity).try_despawn();
        }
    }
}

/// Update knockback physics - units fly through air, land, then stun
pub fn knockback_physics_system(
    mut commands: Commands,
    time: Res<Time>,
    heightmap: Option<Res<TerrainHeightmap>>,
    mut query: Query<(Entity, &mut Transform, &mut KnockbackState)>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, mut knockback) in query.iter_mut() {
        if knockback.is_airborne {
            // Apply velocity
            transform.translation += knockback.velocity * dt;

            // Apply gravity
            knockback.velocity.y += knockback.gravity * dt;

            // Update ground height at current position
            let current_ground_y = heightmap
                .as_ref()
                .map(|hm| hm.sample_height(transform.translation.x, transform.translation.z))
                .unwrap_or(knockback.ground_y);

            // Check ground collision
            if transform.translation.y <= current_ground_y {
                // Landed - snap to ground, start stun timer
                transform.translation.y = current_ground_y;
                knockback.is_airborne = false;
                knockback.velocity = Vec3::ZERO;
            }
        } else {
            // On ground - count down stun timer
            knockback.stun_timer -= dt;

            if knockback.stun_timer <= 0.0 {
                // Stun over - remove component, unit resumes normal behavior
                commands.entity(entity).remove::<KnockbackState>();
            }
        }
    }
}
