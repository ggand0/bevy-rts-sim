// Squad Details UI - Debug panel showing selected squad information

use bevy::prelude::*;

use crate::types::{BattleDroid, CombatUnit, MovementMode, MovementTracker, SquadManager, SquadMember, Team};
use super::state::SelectionState;

/// Marker component for the squad details UI panel
#[derive(Component)]
pub struct SquadDetailsUI;

/// Spawn the squad details UI panel (bottom-left corner)
pub fn spawn_squad_details_ui(commands: &mut Commands) {
    // Container for squad details
    commands.spawn((
        Text::new("No squad selected"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(0.9, 0.9, 0.9, 0.9)),
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
    selection_state: Res<SelectionState>,
    squad_manager: Res<SquadManager>,
    mut ui_query: Query<&mut Text, With<SquadDetailsUI>>,
    droid_query: Query<(&SquadMember, &BattleDroid, &Transform, &MovementMode, &CombatUnit, &MovementTracker)>,
) {
    let Ok(mut text) = ui_query.single_mut() else { return };

    if selection_state.selected_squads.is_empty() {
        **text = "No squad selected".to_string();
        return;
    }

    let mut info_lines: Vec<String> = Vec::new();
    info_lines.push(format!("=== Selected: {} squad(s) ===", selection_state.selected_squads.len()));

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

        for (sm, _droid, transform, mode, combat, tracker) in droid_query.iter() {
            if sm.squad_id == squad_id {
                alive_count += 1;
                avg_pos += transform.translation;

                match mode {
                    MovementMode::Hold => hold_count += 1,
                    MovementMode::AttackMove => attack_move_count += 1,
                    MovementMode::Move => move_count += 1,
                }

                if combat.current_target.is_some() {
                    engaged_count += 1;
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

        info_lines.push(format!(""));
        info_lines.push(format!("Squad #{}", squad_id));
        info_lines.push(format!("  Team: {}", team_str));
        info_lines.push(format!("  Units: {}/50 alive", alive_count));
        info_lines.push(format!("  Mode: {} ({}/{})", mode_str,
            if mode_str == "Hold" { hold_count }
            else if mode_str == "AttackMove" { attack_move_count }
            else { move_count },
            alive_count
        ));
        info_lines.push(format!("  Engaged: {}", engaged_count));
        info_lines.push(format!("  Stationary: {}", stationary_count));
        info_lines.push(format!("  Pos: ({:.0}, {:.0})", avg_pos.x, avg_pos.z));
        info_lines.push(format!("  Target: ({:.0}, {:.0})", squad.target_position.x, squad.target_position.z));

        // Show group info if part of a group
        if let Some(&group_id) = selection_state.squad_to_group.get(&squad_id) {
            info_lines.push(format!("  Group: #{}", group_id));
        }
    }

    **text = info_lines.join("\n");
}
