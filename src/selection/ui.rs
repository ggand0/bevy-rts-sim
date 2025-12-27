// Squad Details UI - Debug panel showing selected squad information

use bevy::prelude::*;
use std::collections::HashMap;

use crate::types::{BattleDroid, CombatUnit, MovementMode, MovementTracker, SquadManager, SquadMember, Team, TurretBase, MgTurret, Health};
use crate::constants::{
    SQUAD_SIZE, INFANTRY_BASE_ACCURACY, TURRET_BASE_ACCURACY, ACCURACY_STATIONARY_BONUS, ACCURACY_HIGH_GROUND_BONUS,
    ACCURACY_TARGET_MOVING_PENALTY, ACCURACY_RANGE_FALLOFF_START, ACCURACY_RANGE_FALLOFF_PER_50U,
    HIGH_GROUND_HEIGHT_THRESHOLD,
};
use crate::combat::calculate_hit_chance;
use super::state::SelectionState;

/// Marker component for the squad details UI panel
#[derive(Component)]
pub struct SquadDetailsUI;

/// Marker for text span children (so we can despawn them on update)
#[derive(Component)]
pub struct SquadDetailsSpan;

/// Cached combat state for a squad (shown when idle)
#[derive(Clone, Default)]
pub struct CachedCombatState {
    pub has_high_ground: bool,
    pub targets_moving: bool,
    pub avg_distance: f32,
    pub avg_height_diff: f32,
}

/// Resource to cache last combat state per squad
#[derive(Resource, Default)]
pub struct SquadCombatCache {
    pub cache: HashMap<u32, CachedCombatState>,
}

// Colors for UI elements
const COLOR_DEFAULT: Color = Color::srgba(0.9, 0.9, 0.9, 0.9);
const COLOR_GREEN: Color = Color::srgba(0.4, 0.9, 0.4, 1.0);
const COLOR_GREY: Color = Color::srgba(0.5, 0.5, 0.5, 0.8);
const COLOR_RED: Color = Color::srgba(0.9, 0.4, 0.4, 1.0);

/// A text segment with color
struct ColoredSegment {
    text: String,
    color: Color,
}

impl ColoredSegment {
    fn new(text: impl Into<String>, color: Color) -> Self {
        Self { text: text.into(), color }
    }

    fn default_color(text: impl Into<String>) -> Self {
        Self { text: text.into(), color: COLOR_DEFAULT }
    }
}

/// Wrapper to call calculate_hit_chance with pre-computed boolean conditions.
/// Constructs synthetic positions that yield the same results as the booleans.
fn ui_accuracy_estimate(
    shooter_stationary: bool,
    has_high_ground: bool,
    target_stationary: bool,
    avg_distance: f32,
) -> f32 {
    // Construct positions that yield correct high_ground check in calculate_hit_chance
    let height = if has_high_ground { HIGH_GROUND_HEIGHT_THRESHOLD + 1.0 } else { 0.0 };
    let shooter_pos = Vec3::new(0.0, height, 0.0);
    // Adjust horizontal distance so 3D distance approximates avg_distance
    let horiz_dist = (avg_distance.powi(2) - height.powi(2)).max(0.0).sqrt();
    let target_pos = Vec3::new(horiz_dist, 0.0, 0.0);

    calculate_hit_chance(INFANTRY_BASE_ACCURACY, shooter_pos, target_pos, shooter_stationary, target_stationary)
}

/// Build accuracy modifier segments for engaged combat state
fn build_accuracy_segments_engaged(
    segments: &mut Vec<ColoredSegment>,
    has_stationary_bonus: bool,
    has_high_ground: bool,
    targets_moving: bool,
    avg_distance: f32,
    avg_height_diff: f32,
) {
    // High ground - green if active, grey if not
    if has_high_ground {
        segments.push(ColoredSegment::new(format!("\n  +High Ground: +{:.0}%", ACCURACY_HIGH_GROUND_BONUS * 100.0), COLOR_GREEN));
    } else {
        segments.push(ColoredSegment::new("\n  High Ground: --".to_string(), COLOR_GREY));
    }

    // Target moving - red penalty if active, grey if not
    if targets_moving {
        segments.push(ColoredSegment::new(format!("\n  -Target Moving: -{:.0}%", ACCURACY_TARGET_MOVING_PENALTY * 100.0), COLOR_RED));
    } else {
        segments.push(ColoredSegment::new("\n  Target Moving: --".to_string(), COLOR_GREY));
    }

    // Range penalty - red if active, grey if not
    if avg_distance > ACCURACY_RANGE_FALLOFF_START {
        let range_penalty = ((avg_distance - ACCURACY_RANGE_FALLOFF_START) / 50.0) * ACCURACY_RANGE_FALLOFF_PER_50U;
        segments.push(ColoredSegment::new(format!("\n  -Range ({:.0}u): -{:.0}%", avg_distance, range_penalty * 100.0), COLOR_RED));
    } else {
        segments.push(ColoredSegment::new(format!("\n  Range ({:.0}u): --", avg_distance), COLOR_GREY));
    }

    let final_acc = ui_accuracy_estimate(has_stationary_bonus, has_high_ground, !targets_moving, avg_distance);
    segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}%", final_acc * 100.0)));

    // Height indicator with color
    let sign = if avg_height_diff >= 0.0 { "+" } else { "" };
    let height_color = if has_high_ground { COLOR_GREEN } else { COLOR_GREY };
    segments.push(ColoredSegment::new(format!("\n  Height: {}{}m", sign, avg_height_diff as i32), height_color));
}

/// Build accuracy modifier segments for cached (last known) combat state
fn build_accuracy_segments_cached(
    segments: &mut Vec<ColoredSegment>,
    has_stationary_bonus: bool,
    cached: &CachedCombatState,
) {
    // High ground from cache
    if cached.has_high_ground {
        segments.push(ColoredSegment::new(format!("\n  +High Ground: +{:.0}% (last)", ACCURACY_HIGH_GROUND_BONUS * 100.0), COLOR_GREEN));
    } else {
        segments.push(ColoredSegment::new("\n  High Ground: -- (last)".to_string(), COLOR_GREY));
    }

    // Target moving from cache
    if cached.targets_moving {
        segments.push(ColoredSegment::new(format!("\n  -Target Moving: -{:.0}% (last)", ACCURACY_TARGET_MOVING_PENALTY * 100.0), COLOR_RED));
    } else {
        segments.push(ColoredSegment::new("\n  Target Moving: -- (last)".to_string(), COLOR_GREY));
    }

    // Range from cache
    if cached.avg_distance > ACCURACY_RANGE_FALLOFF_START {
        let range_penalty = ((cached.avg_distance - ACCURACY_RANGE_FALLOFF_START) / 50.0) * ACCURACY_RANGE_FALLOFF_PER_50U;
        segments.push(ColoredSegment::new(format!("\n  -Range ({:.0}u): -{:.0}% (last)", cached.avg_distance, range_penalty * 100.0), COLOR_RED));
    } else {
        segments.push(ColoredSegment::new(format!("\n  Range ({:.0}u): -- (last)", cached.avg_distance), COLOR_GREY));
    }

    let final_acc = ui_accuracy_estimate(has_stationary_bonus, cached.has_high_ground, !cached.targets_moving, cached.avg_distance);
    segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}% (last)", final_acc * 100.0)));

    // Height from cache with color
    let sign = if cached.avg_height_diff >= 0.0 { "+" } else { "" };
    let height_color = if cached.has_high_ground { COLOR_GREEN } else { COLOR_GREY };
    segments.push(ColoredSegment::new(format!("\n  Height: {}{}m (last)", sign, cached.avg_height_diff as i32), height_color));
}

/// Build accuracy modifier segments for idle state (no combat data)
fn build_accuracy_segments_idle(
    segments: &mut Vec<ColoredSegment>,
    has_stationary_bonus: bool,
    avg_pos: Vec3,
) {
    segments.push(ColoredSegment::new("\n  High Ground: --".to_string(), COLOR_GREY));
    segments.push(ColoredSegment::new("\n  Target Moving: --".to_string(), COLOR_GREY));
    segments.push(ColoredSegment::new("\n  Range: --".to_string(), COLOR_GREY));

    // Idle accuracy: base + stationary bonus if applicable, no combat modifiers
    let final_acc = ui_accuracy_estimate(has_stationary_bonus, false, true, 0.0);
    segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}% (idle)", final_acc * 100.0)));
    segments.push(ColoredSegment::new(format!("\n  Height: {}m", avg_pos.y as i32), COLOR_GREY));
}

/// Spawn the squad details UI panel (bottom-left corner)
pub fn spawn_squad_details_ui(commands: &mut Commands) {
    // Container for squad details - root text entity
    commands.spawn((
        Text::new("No squad selected"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(COLOR_DEFAULT),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        SquadDetailsUI,
    ));
}

/// Update the squad details UI with selected squad information
pub fn update_squad_details_ui(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut combat_cache: ResMut<SquadCombatCache>,
    ui_query: Query<Entity, With<SquadDetailsUI>>,
    span_query: Query<Entity, With<SquadDetailsSpan>>,
    mut text_query: Query<&mut Text, With<SquadDetailsUI>>,
    droid_query: Query<(Entity, &SquadMember, &BattleDroid, &Transform, &MovementMode, &CombatUnit, &MovementTracker)>,
    target_query: Query<(&Transform, &MovementTracker), With<BattleDroid>>,
) {
    let Ok(ui_entity) = ui_query.single() else { return };
    let Ok(mut root_text) = text_query.single_mut() else { return };

    // Despawn old span children
    for span_entity in span_query.iter() {
        commands.entity(span_entity).despawn();
    }

    if selection_state.selected_squads.is_empty() {
        **root_text = "No squad selected".to_string();
        return;
    }

    // Build colored segments
    let mut segments: Vec<ColoredSegment> = Vec::new();
    segments.push(ColoredSegment::default_color(format!("=== Selected: {} squad(s) ===", selection_state.selected_squads.len())));

    for &squad_id in &selection_state.selected_squads {
        let Some(squad) = squad_manager.get_squad(squad_id) else { continue };

        // Count alive units and aggregate stats
        let mut alive_count = 0;
        let mut hold_count = 0;
        let mut attack_move_count = 0;
        let mut move_count = 0;
        let mut engaged_count = 0;
        let mut stationary_count = 0;
        let mut avg_pos = Vec3::ZERO;

        // Aggregate accuracy data across all units
        let mut total_distance = 0.0f32;
        let mut target_moving_count = 0;
        let mut high_ground_count = 0;
        let mut targets_sampled = 0;
        let mut total_height_diff = 0.0f32;

        for (_entity, sm, _droid, transform, mode, combat, tracker) in droid_query.iter() {
            if sm.squad_id == squad_id {
                alive_count += 1;
                avg_pos += transform.translation;

                match mode {
                    MovementMode::Hold => hold_count += 1,
                    MovementMode::AttackMove => attack_move_count += 1,
                    MovementMode::Move => move_count += 1,
                }

                if let Some(target_entity) = combat.current_target {
                    engaged_count += 1;

                    if let Ok((target_transform, target_tracker)) = target_query.get(target_entity) {
                        targets_sampled += 1;
                        total_distance += transform.translation.distance(target_transform.translation);
                        total_height_diff += transform.translation.y - target_transform.translation.y;
                        if !target_tracker.is_stationary {
                            target_moving_count += 1;
                        }
                        if transform.translation.y > target_transform.translation.y + HIGH_GROUND_HEIGHT_THRESHOLD {
                            high_ground_count += 1;
                        }
                    }
                }

                if tracker.is_stationary {
                    stationary_count += 1;
                }
            }
        }

        if alive_count > 0 {
            avg_pos /= alive_count as f32;
        }

        // Determine dominant mode
        let mode_str = if hold_count > attack_move_count && hold_count > move_count {
            "Hold"
        } else if attack_move_count > move_count {
            "AttackMove"
        } else {
            "Move"
        };

        let team_str = match squad.team {
            Team::A => "A (Blue)",
            Team::B => "B (Red)",
        };

        segments.push(ColoredSegment::default_color(format!("\n\nSquad #{}", squad_id)));
        segments.push(ColoredSegment::default_color(format!("\n  Team: {}", team_str)));
        segments.push(ColoredSegment::default_color(format!("\n  Units: {}/{} alive", alive_count, SQUAD_SIZE)));
        segments.push(ColoredSegment::default_color(format!("\n  Mode: {} ({}/{})", mode_str,
            if mode_str == "Hold" { hold_count }
            else if mode_str == "AttackMove" { attack_move_count }
            else { move_count },
            alive_count
        )));
        segments.push(ColoredSegment::default_color(format!("\n  Engaged: {}", engaged_count)));
        segments.push(ColoredSegment::default_color(format!("\n  Stationary: {}/{}", stationary_count, alive_count)));
        segments.push(ColoredSegment::default_color(format!("\n  Pos: ({:.0}, {:.0}, h={:.0})", avg_pos.x, avg_pos.z, avg_pos.y)));
        segments.push(ColoredSegment::default_color(format!("\n  Target: ({:.0}, {:.0})", squad.target_position.x, squad.target_position.z)));

        // Accuracy breakdown
        segments.push(ColoredSegment::default_color("\n  --- Accuracy ---".to_string()));
        segments.push(ColoredSegment::default_color(format!("\n  Base: {:.0}%", INFANTRY_BASE_ACCURACY * 100.0)));

        let stationary_ratio = if alive_count > 0 { stationary_count as f32 / alive_count as f32 } else { 0.0 };
        let has_stationary_bonus = stationary_ratio > 0.5;

        // Stationary bonus - green if active, grey if not
        if has_stationary_bonus {
            segments.push(ColoredSegment::new(format!("\n  +Stationary: +{:.0}%", ACCURACY_STATIONARY_BONUS * 100.0), COLOR_GREEN));
        } else {
            segments.push(ColoredSegment::new(format!("\n  Stationary: --"), COLOR_GREY));
        }

        // If engaged, show combat-specific accuracy modifiers
        if targets_sampled > 0 {
            let avg_distance = total_distance / targets_sampled as f32;
            let avg_height_diff = total_height_diff / targets_sampled as f32;
            let high_ground_ratio = high_ground_count as f32 / targets_sampled as f32;
            let target_moving_ratio = target_moving_count as f32 / targets_sampled as f32;

            let has_high_ground = high_ground_ratio > 0.5;
            let targets_moving = target_moving_ratio > 0.5;

            // Cache combat state
            combat_cache.cache.insert(squad_id, CachedCombatState {
                has_high_ground,
                targets_moving,
                avg_distance,
                avg_height_diff,
            });

            build_accuracy_segments_engaged(
                &mut segments,
                has_stationary_bonus,
                has_high_ground,
                targets_moving,
                avg_distance,
                avg_height_diff,
            );
        } else {
            // Not engaged - show cached combat state if available
            if let Some(cached) = combat_cache.cache.get(&squad_id) {
                build_accuracy_segments_cached(&mut segments, has_stationary_bonus, cached);
            } else {
                build_accuracy_segments_idle(&mut segments, has_stationary_bonus, avg_pos);
            }
        }

        // Group info
        if let Some(&group_id) = selection_state.squad_to_group.get(&squad_id) {
            segments.push(ColoredSegment::default_color(format!("\n  Group: #{}", group_id)));
        }
    }

    // Set root text to first segment, spawn rest as TextSpan children
    if let Some(first) = segments.first() {
        **root_text = first.text.clone();
        commands.entity(ui_entity).insert(TextColor(first.color));
    }

    // Spawn remaining segments as TextSpan children
    for segment in segments.iter().skip(1) {
        let span = commands.spawn((
            TextSpan::new(segment.text.clone()),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(segment.color),
            SquadDetailsSpan,
        )).id();
        commands.entity(ui_entity).add_child(span);
    }
}

// ============================================================================
// TURRET DETAILS UI
// ============================================================================

/// Marker component for the turret details UI panel
#[derive(Component)]
pub struct TurretDetailsUI;

/// Marker for turret text span children
#[derive(Component)]
pub struct TurretDetailsSpan;

/// Spawn the turret details UI panel (bottom-right corner)
pub fn spawn_turret_details_ui(commands: &mut Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(COLOR_DEFAULT),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        TurretDetailsUI,
    ));
}

/// Update the turret details UI with selected turret information
pub fn update_turret_details_ui(
    mut commands: Commands,
    selection_state: Res<SelectionState>,
    ui_query: Query<Entity, With<TurretDetailsUI>>,
    span_query: Query<Entity, With<TurretDetailsSpan>>,
    mut text_query: Query<&mut Text, With<TurretDetailsUI>>,
    turret_base_query: Query<(&Transform, &TurretBase, &Health, &Children)>,
    turret_assembly_query: Query<(&CombatUnit, Option<&MgTurret>, &MovementTracker)>,
    target_query: Query<(&Transform, &MovementTracker), With<BattleDroid>>,
) {
    let Ok(ui_entity) = ui_query.single() else { return };
    let Ok(mut root_text) = text_query.single_mut() else { return };

    // Despawn old span children
    for span_entity in span_query.iter() {
        commands.entity(span_entity).despawn();
    }

    // Check if a turret is selected
    let Some(turret_entity) = selection_state.selected_turret else {
        **root_text = "".to_string();
        return;
    };

    // Get turret base info
    let Ok((transform, turret_base, health, children)) = turret_base_query.get(turret_entity) else {
        **root_text = "".to_string();
        return;
    };

    // Find the rotating assembly (child with CombatUnit)
    let mut assembly_info = None;
    for child in children.iter() {
        if let Ok(info) = turret_assembly_query.get(child) {
            assembly_info = Some(info);
            break;
        }
    }

    // Build colored segments
    let mut segments: Vec<ColoredSegment> = Vec::new();
    segments.push(ColoredSegment::default_color("=== Selected Turret ===".to_string()));

    let team_str = match turret_base.team {
        Team::A => "A (Blue)",
        Team::B => "B (Red)",
    };
    segments.push(ColoredSegment::default_color(format!("\n  Team: {}", team_str)));

    // Health with color
    let health_pct = (health.current / health.max * 100.0) as i32;
    let health_color = if health_pct > 50 {
        COLOR_GREEN
    } else if health_pct > 25 {
        Color::srgba(0.9, 0.7, 0.2, 1.0) // Yellow-orange
    } else {
        COLOR_RED
    };
    segments.push(ColoredSegment::new(format!("\n  Health: {:.0}/{:.0} ({}%)", health.current, health.max, health_pct), health_color));

    // Position
    segments.push(ColoredSegment::default_color(format!("\n  Pos: ({:.0}, {:.0}, h={:.0})", transform.translation.x, transform.translation.z, transform.translation.y)));

    // Combat info from assembly
    if let Some((combat_unit, mg_turret, _tracker)) = assembly_info {
        // Target info
        if let Some(target_entity) = combat_unit.current_target {
            if let Ok((target_transform, target_tracker)) = target_query.get(target_entity) {
                let distance = transform.translation.distance(target_transform.translation);
                let height_diff = transform.translation.y - target_transform.translation.y;

                segments.push(ColoredSegment::new("\n  Target: ENGAGED".to_string(), COLOR_GREEN));
                segments.push(ColoredSegment::default_color(format!("\n  Distance: {:.0}u", distance)));

                // Accuracy calculation for turrets
                let has_high_ground = height_diff > HIGH_GROUND_HEIGHT_THRESHOLD;
                let target_moving = !target_tracker.is_stationary;

                segments.push(ColoredSegment::default_color("\n  --- Accuracy ---".to_string()));
                segments.push(ColoredSegment::default_color(format!("\n  Base: {:.0}%", TURRET_BASE_ACCURACY * 100.0)));

                // Stationary - turrets are always stationary (built into base accuracy)
                segments.push(ColoredSegment::new("\n  Stationary: (built-in)".to_string(), COLOR_GREY));

                // High ground
                if has_high_ground {
                    segments.push(ColoredSegment::new(format!("\n  +High Ground: +{:.0}%", ACCURACY_HIGH_GROUND_BONUS * 100.0), COLOR_GREEN));
                } else {
                    segments.push(ColoredSegment::new("\n  High Ground: --".to_string(), COLOR_GREY));
                }

                // Target moving
                if target_moving {
                    segments.push(ColoredSegment::new(format!("\n  -Target Moving: -{:.0}%", ACCURACY_TARGET_MOVING_PENALTY * 100.0), COLOR_RED));
                } else {
                    segments.push(ColoredSegment::new("\n  Target Moving: --".to_string(), COLOR_GREY));
                }

                // Range
                if distance > ACCURACY_RANGE_FALLOFF_START {
                    let range_penalty = ((distance - ACCURACY_RANGE_FALLOFF_START) / 50.0) * ACCURACY_RANGE_FALLOFF_PER_50U;
                    segments.push(ColoredSegment::new(format!("\n  -Range ({:.0}u): -{:.0}%", distance, range_penalty * 100.0), COLOR_RED));
                } else {
                    segments.push(ColoredSegment::new(format!("\n  Range ({:.0}u): --", distance), COLOR_GREY));
                }

                // Final accuracy
                let final_acc = turret_accuracy_estimate(has_high_ground, !target_moving, distance);
                segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}%", final_acc * 100.0)));

                // Height indicator
                let sign = if height_diff >= 0.0 { "+" } else { "" };
                let height_color = if has_high_ground { COLOR_GREEN } else { COLOR_GREY };
                segments.push(ColoredSegment::new(format!("\n  Height: {}{}m", sign, height_diff as i32), height_color));
            } else {
                segments.push(ColoredSegment::new("\n  Target: INVALID".to_string(), COLOR_GREY));
            }
        } else {
            segments.push(ColoredSegment::new("\n  Target: None".to_string(), COLOR_GREY));

            // Show base accuracy when idle
            segments.push(ColoredSegment::default_color("\n  --- Accuracy ---".to_string()));
            segments.push(ColoredSegment::default_color(format!("\n  Base: {:.0}%", TURRET_BASE_ACCURACY * 100.0)));
            segments.push(ColoredSegment::new("\n  Stationary: (built-in)".to_string(), COLOR_GREY));
            segments.push(ColoredSegment::new("\n  High Ground: --".to_string(), COLOR_GREY));
            segments.push(ColoredSegment::new("\n  Target Moving: --".to_string(), COLOR_GREY));
            segments.push(ColoredSegment::new("\n  Range: --".to_string(), COLOR_GREY));
            segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}% (idle)", TURRET_BASE_ACCURACY * 100.0)));
        }

        // MG turret specific info
        if let Some(mg) = mg_turret {
            segments.push(ColoredSegment::default_color("\n  --- MG Status ---".to_string()));
            segments.push(ColoredSegment::default_color(format!("\n  Burst: {}/{}", mg.shots_in_burst, mg.max_burst_shots)));
            if mg.cooldown_timer > 0.0 {
                segments.push(ColoredSegment::new(format!("\n  Cooldown: {:.1}s", mg.cooldown_timer), COLOR_GREY));
            } else {
                segments.push(ColoredSegment::new("\n  Ready to fire".to_string(), COLOR_GREEN));
            }
        } else {
            segments.push(ColoredSegment::default_color("\n  Type: Heavy Turret".to_string()));
        }
    }

    // Set root text to first segment, spawn rest as TextSpan children
    if let Some(first) = segments.first() {
        **root_text = first.text.clone();
        commands.entity(ui_entity).insert(TextColor(first.color));
    }

    // Spawn remaining segments as TextSpan children
    for segment in segments.iter().skip(1) {
        let span = commands.spawn((
            TextSpan::new(segment.text.clone()),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(segment.color),
            TurretDetailsSpan,
        )).id();
        commands.entity(ui_entity).add_child(span);
    }
}

/// Calculate turret accuracy estimate (turrets are always stationary)
fn turret_accuracy_estimate(has_high_ground: bool, target_stationary: bool, distance: f32) -> f32 {
    let height = if has_high_ground { HIGH_GROUND_HEIGHT_THRESHOLD + 1.0 } else { 0.0 };
    let shooter_pos = Vec3::new(0.0, height, 0.0);
    let horiz_dist = (distance.powi(2) - height.powi(2)).max(0.0).sqrt();
    let target_pos = Vec3::new(horiz_dist, 0.0, 0.0);

    calculate_hit_chance(TURRET_BASE_ACCURACY, shooter_pos, target_pos, true, target_stationary)
}
