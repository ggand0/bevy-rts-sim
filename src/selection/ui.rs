// Squad Details UI - Debug panel showing selected squad information

use bevy::prelude::*;
use std::collections::HashMap;

use crate::types::{BattleDroid, CombatUnit, MovementMode, MovementTracker, SquadManager, SquadMember, Team, TurretBase, MgTurret, Health};
use crate::constants::{
    SQUAD_SIZE, INFANTRY_BASE_ACCURACY, TURRET_BASE_ACCURACY, ACCURACY_STATIONARY_BONUS, ACCURACY_HIGH_GROUND_BONUS,
    ACCURACY_TARGET_MOVING_PENALTY, HIGH_GROUND_HEIGHT_THRESHOLD,
};
use crate::combat::{calculate_hit_chance, calculate_range_penalty};
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

/// Resource to cache last combat state per turret
#[derive(Resource, Default)]
pub struct TurretCombatCache {
    pub cache: HashMap<Entity, CachedCombatState>,
}

/// Timer for throttling UI updates (UI doesn't need 60fps updates)
#[derive(Resource)]
pub struct UiUpdateTimer(pub Timer);

impl Default for UiUpdateTimer {
    fn default() -> Self {
        // Update UI 10 times per second (every 100ms)
        Self(Timer::from_seconds(0.1, TimerMode::Repeating))
    }
}

/// Maximum squads to show detailed info for (performance optimization)
const MAX_DETAILED_SQUADS: usize = 1;

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

/// Build accuracy modifier segments (shared between squad and turret UI)
/// `suffix` is empty string for current state, " (last)" for cached state
fn build_accuracy_modifiers(
    segments: &mut Vec<ColoredSegment>,
    has_high_ground: bool,
    targets_moving: bool,
    avg_distance: f32,
    avg_height_diff: f32,
    final_acc: f32,
    suffix: &str,
) {
    // High ground - green if active, grey if not
    if has_high_ground {
        segments.push(ColoredSegment::new(format!("\n  +High Ground: +{:.0}%{}", ACCURACY_HIGH_GROUND_BONUS * 100.0, suffix), COLOR_GREEN));
    } else {
        segments.push(ColoredSegment::new(format!("\n  High Ground: --{}", suffix), COLOR_GREY));
    }

    // Target moving - red penalty if active, grey if not
    if targets_moving {
        segments.push(ColoredSegment::new(format!("\n  -Target Moving: -{:.0}%{}", ACCURACY_TARGET_MOVING_PENALTY * 100.0, suffix), COLOR_RED));
    } else {
        segments.push(ColoredSegment::new(format!("\n  Target Moving: --{}", suffix), COLOR_GREY));
    }

    // Range penalty - red if active, grey if not
    let range_penalty = calculate_range_penalty(avg_distance);
    if range_penalty > 0.0 {
        segments.push(ColoredSegment::new(format!("\n  -Range ({:.0}u): -{:.0}%{}", avg_distance, range_penalty * 100.0, suffix), COLOR_RED));
    } else {
        segments.push(ColoredSegment::new(format!("\n  Range ({:.0}u): --{}", avg_distance, suffix), COLOR_GREY));
    }

    segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}%{}", final_acc * 100.0, suffix)));

    // Height indicator with color
    let sign = if avg_height_diff >= 0.0 { "+" } else { "" };
    let height_color = if has_high_ground { COLOR_GREEN } else { COLOR_GREY };
    segments.push(ColoredSegment::new(format!("\n  Height: {}{}m{}", sign, avg_height_diff as i32, suffix), height_color));
}

/// Build accuracy modifier segments for engaged combat state (infantry)
fn build_accuracy_segments_engaged(
    segments: &mut Vec<ColoredSegment>,
    has_stationary_bonus: bool,
    has_high_ground: bool,
    targets_moving: bool,
    avg_distance: f32,
    avg_height_diff: f32,
) {
    let final_acc = ui_accuracy_estimate(has_stationary_bonus, has_high_ground, !targets_moving, avg_distance);
    build_accuracy_modifiers(segments, has_high_ground, targets_moving, avg_distance, avg_height_diff, final_acc, "");
}

/// Build accuracy modifier segments for cached (last known) combat state (infantry)
fn build_accuracy_segments_cached(
    segments: &mut Vec<ColoredSegment>,
    has_stationary_bonus: bool,
    cached: &CachedCombatState,
) {
    let final_acc = ui_accuracy_estimate(has_stationary_bonus, cached.has_high_ground, !cached.targets_moving, cached.avg_distance);
    build_accuracy_modifiers(segments, cached.has_high_ground, cached.targets_moving, cached.avg_distance, cached.avg_height_diff, final_acc, " (last)");
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
    time: Res<Time>,
    mut ui_timer: ResMut<UiUpdateTimer>,
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut combat_cache: ResMut<SquadCombatCache>,
    ui_query: Query<(Entity, Option<&Children>), With<SquadDetailsUI>>,
    mut text_query: Query<&mut Text, With<SquadDetailsUI>>,
    droid_query: Query<(&SquadMember, &BattleDroid, &Transform, &MovementMode, &CombatUnit, &MovementTracker)>,
    target_query: Query<(&Transform, &MovementTracker), With<BattleDroid>>,
) {
    // Throttle UI updates for performance
    ui_timer.0.tick(time.delta());
    if !ui_timer.0.just_finished() {
        return;
    }

    let Ok((ui_entity, children)) = ui_query.single() else { return };
    let Ok(mut root_text) = text_query.single_mut() else { return };

    // Despawn only children of this UI entity (not all spans globally)
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    if selection_state.selected_squads.is_empty() {
        **root_text = "No squad selected".to_string();
        return;
    }

    // Build colored segments
    let mut segments: Vec<ColoredSegment> = Vec::new();
    segments.push(ColoredSegment::default_color(format!("=== Selected: {} squad(s) ===", selection_state.selected_squads.len())));

    // PERFORMANCE: Aggregate all squad stats in a SINGLE pass over droids
    // This changes O(selected_squads * all_droids) to O(all_droids)
    struct SquadStats {
        alive_count: u32,
        hold_count: u32,
        attack_move_count: u32,
        move_count: u32,
        engaged_count: u32,
        stationary_count: u32,
        avg_pos: Vec3,
        total_distance: f32,
        target_moving_count: u32,
        high_ground_count: u32,
        targets_sampled: u32,
        total_height_diff: f32,
    }

    let mut squad_stats: HashMap<u32, SquadStats> = HashMap::new();

    // Initialize stats for selected squads only
    for &squad_id in &selection_state.selected_squads {
        squad_stats.insert(squad_id, SquadStats {
            alive_count: 0,
            hold_count: 0,
            attack_move_count: 0,
            move_count: 0,
            engaged_count: 0,
            stationary_count: 0,
            avg_pos: Vec3::ZERO,
            total_distance: 0.0,
            target_moving_count: 0,
            high_ground_count: 0,
            targets_sampled: 0,
            total_height_diff: 0.0,
        });
    }

    // Single pass over all droids
    for (sm, _droid, transform, mode, combat, tracker) in droid_query.iter() {
        let Some(stats) = squad_stats.get_mut(&sm.squad_id) else { continue };

        stats.alive_count += 1;
        stats.avg_pos += transform.translation;

        match mode {
            MovementMode::Hold => stats.hold_count += 1,
            MovementMode::AttackMove => stats.attack_move_count += 1,
            MovementMode::Move => stats.move_count += 1,
        }

        if let Some(target_entity) = combat.current_target {
            stats.engaged_count += 1;

            if let Ok((target_transform, target_tracker)) = target_query.get(target_entity) {
                stats.targets_sampled += 1;
                stats.total_distance += transform.translation.distance(target_transform.translation);
                stats.total_height_diff += transform.translation.y - target_transform.translation.y;
                if !target_tracker.is_stationary {
                    stats.target_moving_count += 1;
                }
                if transform.translation.y > target_transform.translation.y + HIGH_GROUND_HEIGHT_THRESHOLD {
                    stats.high_ground_count += 1;
                }
            }
        }

        if tracker.is_stationary {
            stats.stationary_count += 1;
        }
    }

    // Now build UI from precomputed stats
    for (idx, &squad_id) in selection_state.selected_squads.iter().enumerate() {
        let Some(squad) = squad_manager.get_squad(squad_id) else { continue };
        let Some(stats) = squad_stats.get(&squad_id) else { continue };

        let alive_count = stats.alive_count;
        let mut avg_pos = stats.avg_pos;
        if alive_count > 0 {
            avg_pos /= alive_count as f32;
        }

        // Determine dominant mode
        let mode_str = if stats.hold_count > stats.attack_move_count && stats.hold_count > stats.move_count {
            "Hold"
        } else if stats.attack_move_count > stats.move_count {
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
            if mode_str == "Hold" { stats.hold_count }
            else if mode_str == "AttackMove" { stats.attack_move_count }
            else { stats.move_count },
            alive_count
        )));

        // Only show detailed stats for first MAX_DETAILED_SQUADS (performance optimization)
        if idx >= MAX_DETAILED_SQUADS {
            segments.push(ColoredSegment::new("\n  (detailed stats hidden)".to_string(), COLOR_GREY));
            continue;
        }

        segments.push(ColoredSegment::default_color(format!("\n  Engaged: {}", stats.engaged_count)));
        segments.push(ColoredSegment::default_color(format!("\n  Stationary: {}/{}", stats.stationary_count, alive_count)));
        segments.push(ColoredSegment::default_color(format!("\n  Pos: ({:.0}, {:.0}, h={:.0})", avg_pos.x, avg_pos.z, avg_pos.y)));
        segments.push(ColoredSegment::default_color(format!("\n  Target: ({:.0}, {:.0})", squad.target_position.x, squad.target_position.z)));

        // Accuracy breakdown
        segments.push(ColoredSegment::default_color("\n  --- Accuracy ---".to_string()));
        segments.push(ColoredSegment::default_color(format!("\n  Base: {:.0}%", INFANTRY_BASE_ACCURACY * 100.0)));

        let stationary_ratio = if alive_count > 0 { stats.stationary_count as f32 / alive_count as f32 } else { 0.0 };
        let has_stationary_bonus = stationary_ratio > 0.5;

        // Stationary bonus - green if active, grey if not
        if has_stationary_bonus {
            segments.push(ColoredSegment::new(format!("\n  +Stationary: +{:.0}%", ACCURACY_STATIONARY_BONUS * 100.0), COLOR_GREEN));
        } else {
            segments.push(ColoredSegment::new("\n  Stationary: --".to_string(), COLOR_GREY));
        }

        // If engaged, show combat-specific accuracy modifiers
        if stats.targets_sampled > 0 {
            let avg_distance = stats.total_distance / stats.targets_sampled as f32;
            let avg_height_diff = stats.total_height_diff / stats.targets_sampled as f32;
            let high_ground_ratio = stats.high_ground_count as f32 / stats.targets_sampled as f32;
            let target_moving_ratio = stats.target_moving_count as f32 / stats.targets_sampled as f32;

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
    ui_timer: Res<UiUpdateTimer>,
    selection_state: Res<SelectionState>,
    mut turret_cache: ResMut<TurretCombatCache>,
    ui_query: Query<(Entity, Option<&Children>), With<TurretDetailsUI>>,
    mut text_query: Query<&mut Text, With<TurretDetailsUI>>,
    turret_base_query: Query<(&Transform, &TurretBase, &Health, &Children)>,
    turret_assembly_query: Query<(&CombatUnit, Option<&MgTurret>)>,
    target_query: Query<(&Transform, &MovementTracker), With<BattleDroid>>,
) {
    // Throttle: squad UI ticks the timer, we just check if it fired
    if !ui_timer.0.just_finished() {
        return;
    }

    let Ok((ui_entity, ui_children)) = ui_query.single() else { return };
    let Ok(mut root_text) = text_query.single_mut() else { return };

    // Despawn only children of this UI entity (not all spans globally)
    if let Some(children) = ui_children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
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
    if let Some((combat_unit, mg_turret)) = assembly_info {
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

                // Cache combat state
                turret_cache.cache.insert(turret_entity, CachedCombatState {
                    has_high_ground,
                    targets_moving: target_moving,
                    avg_distance: distance,
                    avg_height_diff: height_diff,
                });

                segments.push(ColoredSegment::default_color("\n  --- Accuracy ---".to_string()));
                segments.push(ColoredSegment::default_color(format!("\n  Base: {:.0}%", TURRET_BASE_ACCURACY * 100.0)));

                // Stationary - turrets are always stationary (built into base accuracy)
                segments.push(ColoredSegment::new("\n  Stationary: (built-in)".to_string(), COLOR_GREY));

                // High ground
                // Use shared helper for accuracy modifiers
                let final_acc = turret_accuracy_estimate(has_high_ground, !target_moving, distance);
                build_accuracy_modifiers(&mut segments, has_high_ground, target_moving, distance, height_diff, final_acc, "");
            } else {
                segments.push(ColoredSegment::new("\n  Target: INVALID".to_string(), COLOR_GREY));
            }
        } else {
            segments.push(ColoredSegment::new("\n  Target: None".to_string(), COLOR_GREY));

            // Show cached combat state if available, otherwise idle
            segments.push(ColoredSegment::default_color("\n  --- Accuracy ---".to_string()));
            segments.push(ColoredSegment::default_color(format!("\n  Base: {:.0}%", TURRET_BASE_ACCURACY * 100.0)));
            segments.push(ColoredSegment::new("\n  Stationary: (built-in)".to_string(), COLOR_GREY));

            if let Some(cached) = turret_cache.cache.get(&turret_entity) {
                // Use shared helper for cached accuracy modifiers
                let final_acc = turret_accuracy_estimate(cached.has_high_ground, !cached.targets_moving, cached.avg_distance);
                build_accuracy_modifiers(&mut segments, cached.has_high_ground, cached.targets_moving, cached.avg_distance, cached.avg_height_diff, final_acc, " (last)");
            } else {
                // No cached state - show idle
                segments.push(ColoredSegment::new("\n  High Ground: --".to_string(), COLOR_GREY));
                segments.push(ColoredSegment::new("\n  Target Moving: --".to_string(), COLOR_GREY));
                segments.push(ColoredSegment::new("\n  Range: --".to_string(), COLOR_GREY));
                segments.push(ColoredSegment::default_color(format!("\n  = Hit Chance: {:.0}% (idle)", TURRET_BASE_ACCURACY * 100.0)));
            }
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
