// Scenario UI systems

use bevy::prelude::*;

use super::{
    ScenarioState, WaveManager, WaveState,
    WaveCounterUI, EnemyCountUI, PreparationInstructionsUI, ScenarioUI,
    TURRET_BUDGET, INTER_WAVE_DELAY, STRATEGIC_WAVE_DELAY,
};

/// Spawn scenario UI elements
pub fn spawn_scenario_ui(commands: &mut Commands) {
    // Wave counter UI
    commands.spawn((
        Text::new("Assault: 0/0 | Wave: 0/0"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(55.0),
            left: Val::Px(10.0),
            ..default()
        },
        WaveCounterUI,
        ScenarioUI,
    ));

    // Enemy count UI
    commands.spawn((
        Text::new("Enemies: 0"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.3, 0.3)), // Red
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(80.0),
            left: Val::Px(10.0),
            ..default()
        },
        EnemyCountUI,
        ScenarioUI,
    ));

    // Preparation phase instructions
    commands.spawn((
        Text::new(format!("PREPARATION - Turrets: {}/{} | T: toggle type | Click: place | SPACE: start",
            TURRET_BUDGET, TURRET_BUDGET)),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.3, 1.0, 0.3)), // Green
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(105.0),
            left: Val::Px(10.0),
            ..default()
        },
        PreparationInstructionsUI,
        ScenarioUI,
    ));
}

/// Update wave counter UI
pub fn update_wave_counter_ui(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut query: Query<&mut Text, With<WaveCounterUI>>,
) {
    if !scenario_state.active {
        return;
    }

    for mut text in query.iter_mut() {
        // Show strategic wave and tactical wave progress
        *text = Text::new(format!("Assault: {}/{} | Wave: {}/{}",
            wave_manager.strategic_wave, wave_manager.total_strategic_waves,
            wave_manager.tactical_wave, wave_manager.total_tactical_waves));
    }
}

/// Update enemy count UI
pub fn update_enemy_count_ui(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut query: Query<&mut Text, With<EnemyCountUI>>,
) {
    if !scenario_state.active {
        return;
    }

    for mut text in query.iter_mut() {
        *text = Text::new(format!("Enemies: {}", wave_manager.enemies_remaining));
    }
}

/// Update preparation phase instructions UI
pub fn update_preparation_ui(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut query: Query<(&mut Text, &mut TextColor), With<PreparationInstructionsUI>>,
) {
    if !scenario_state.active {
        return;
    }

    for (mut text, mut color) in query.iter_mut() {
        match wave_manager.wave_state {
            WaveState::Preparation => {
                let turret_type = if wave_manager.place_mg_turret { "MG" } else { "Heavy" };
                *text = Text::new(format!(
                    "PREPARATION - Turrets: {}/{} | Type: {} | T: toggle | LMB: place | RMB: undo | SPACE: start",
                    wave_manager.turrets_remaining, TURRET_BUDGET, turret_type
                ));
                *color = TextColor(Color::srgb(0.3, 1.0, 0.3)); // Green
            }
            WaveState::Combat => {
                let status_text = if wave_manager.spawning_active {
                    format!("COMBAT - Assault {} Wave {} spawning...",
                        wave_manager.strategic_wave, wave_manager.tactical_wave)
                } else if wave_manager.tactical_wave < wave_manager.total_tactical_waves {
                    let remaining = INTER_WAVE_DELAY - wave_manager.next_wave_timer.elapsed_secs();
                    format!("COMBAT - Next wave in {:.0}s", remaining.max(0.0))
                } else {
                    format!("COMBAT - Final wave of Assault {}!", wave_manager.strategic_wave)
                };
                *text = Text::new(status_text);
                *color = TextColor(Color::srgb(1.0, 0.5, 0.3)); // Orange
            }
            WaveState::StrategicCooldown => {
                let remaining = STRATEGIC_WAVE_DELAY - wave_manager.strategic_cooldown_timer.elapsed_secs();
                *text = Text::new(format!(
                    "ASSAULT {} CLEARED! Next assault in {:.0}s",
                    wave_manager.strategic_wave, remaining.max(0.0)
                ));
                *color = TextColor(Color::srgb(0.3, 0.8, 1.0)); // Cyan
            }
            WaveState::Complete => {
                *text = Text::new("VICTORY! All assaults repelled!");
                *color = TextColor(Color::srgb(0.3, 1.0, 0.3)); // Green
            }
            WaveState::Idle => {
                *text = Text::new("");
            }
        }
    }
}
