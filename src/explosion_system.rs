// Explosion orchestration system - handles delayed explosions and visual effects
use bevy::prelude::*;
use crate::constants::*;
use crate::explosion_shader::{spawn_custom_shader_explosion, ExplosionAssets};
use crate::particles::ExplosionParticleEffects;
use crate::types::AudioAssets;
use crate::types::UplinkTower;

/// Component for entities waiting to explode after a delay
#[derive(Component)]
pub struct PendingExplosion {
    pub delay_timer: f32,
    #[allow(dead_code)]
    pub explosion_power: f32,
}

/// Component for active explosion visual effects
#[derive(Component)]
pub struct ExplosionEffect {
    pub timer: f32,
    pub max_time: f32,
    #[allow(dead_code)]
    pub radius: f32,
    #[allow(dead_code)]
    pub intensity: f32,
}

/// Maximum explosions to process per frame (prevents lag spikes)
const MAX_EXPLOSIONS_PER_FRAME: usize = 20;

/// System that processes pending explosions after their delay timers expire
pub fn pending_explosion_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut explosion_materials: ResMut<Assets<crate::explosion_shader::ExplosionMaterial>>,
    mut smoke_materials: ResMut<Assets<crate::wfx_materials::SmokeScrollMaterial>>,
    mut additive_materials: ResMut<Assets<crate::wfx_materials::AdditiveMaterial>>,
    mut smoke_only_materials: ResMut<Assets<crate::wfx_materials::SmokeOnlyMaterial>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    particle_effects: Option<Res<ExplosionParticleEffects>>,
    audio_assets: Res<AudioAssets>,
    asset_server: Res<AssetServer>,
    mut explosion_query: Query<(Entity, &mut PendingExplosion, &Transform, Option<&UplinkTower>), With<PendingExplosion>>,
    time: Res<Time>,
) {
    let mut ready_to_explode: Vec<(Entity, Vec3, f32, bool, f32)> = Vec::new();
    let mut towers_ready: Vec<(Entity, Vec3, f32)> = Vec::new();
    let mut total_pending = 0;

    // Update timers and collect ready entities
    for (entity, mut pending, transform, tower_component) in explosion_query.iter_mut() {
        total_pending += 1;
        let old_timer = pending.delay_timer;
        pending.delay_timer -= time.delta_secs();

        if pending.delay_timer <= 0.0 {
            let is_tower = tower_component.is_some();
            let explosion_radius = if is_tower {
                tower_component.unwrap().destruction_radius * 0.5
            } else {
                8.0
            };

            debug!("⏰ Entity {:?} ready to explode: timer {:.3}s → {:.3}s",
                   entity.index(), old_timer, pending.delay_timer);

            if is_tower {
                towers_ready.push((entity, transform.translation, explosion_radius));
            } else {
                ready_to_explode.push((entity, transform.translation, explosion_radius, is_tower, old_timer));
            }
        }
    }

    if total_pending > 0 || !ready_to_explode.is_empty() || !towers_ready.is_empty() {
        info!("📊 EXPLOSION FRAME: {} total pending, {} units ready, {} towers ready",
              total_pending, ready_to_explode.len(), towers_ready.len());
    }

    // Shuffle unit explosions for visual variety
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    ready_to_explode.shuffle(&mut rng);

    // Process towers first (high priority, always immediate)
    for (entity, position, _explosion_radius) in towers_ready {
        info!("🏰 Processing TOWER explosion at {:?}", position);

        commands.spawn((
            AudioPlayer::new(audio_assets.explosion_sound.clone()),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::new(crate::constants::VOLUME_EXPLOSION)),
        ));

        crate::wfx_spawn::spawn_combined_explosion(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &mut smoke_materials,
            &mut smoke_only_materials,
            &asset_server,
            position,
            4.0,
        );

        commands.entity(entity).despawn_recursive();
    }

    // Process unit explosions with frame limit
    let num_to_process = ready_to_explode.len().min(MAX_EXPLOSIONS_PER_FRAME);
    if num_to_process > 0 {
        info!("💥 Processing {} unit explosions this frame (limit: {})", num_to_process, MAX_EXPLOSIONS_PER_FRAME);
    }

    for (entity, position, explosion_radius, is_tower, _old_timer) in ready_to_explode.iter().take(MAX_EXPLOSIONS_PER_FRAME) {
        if let Some(assets) = explosion_assets.as_ref() {
            spawn_custom_shader_explosion(
                &mut commands,
                &mut meshes,
                &mut explosion_materials,
                &assets,
                particle_effects.as_ref().map(|p| p.as_ref()),
                *position,
                explosion_radius * 0.1,
                1.0,
                EXPLOSION_EFFECT_DURATION,
                *is_tower,
                time.elapsed_secs_f64(),
            );
        } else {
            warn!("Cannot spawn unit explosion - ExplosionAssets not loaded");
        }

        commands.entity(*entity).despawn_recursive();
    }

    if ready_to_explode.len() > MAX_EXPLOSIONS_PER_FRAME {
        warn!("⚠️ Skipped {} explosions this frame (over limit)", ready_to_explode.len() - MAX_EXPLOSIONS_PER_FRAME);
    }
}

/// System that updates active explosion visual effects
pub fn explosion_effect_system(
    mut commands: Commands,
    mut explosion_query: Query<(Entity, &mut ExplosionEffect, &Transform), With<ExplosionEffect>>,
    time: Res<Time>,
) {
    for (entity, mut effect, _transform) in explosion_query.iter_mut() {
        effect.timer += time.delta_secs();

        if effect.timer >= effect.max_time {
            commands.entity(entity).despawn();
        }
    }
}
