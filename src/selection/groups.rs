// Squad grouping logic - Total War style formation preservation
use bevy::prelude::*;
use std::collections::HashMap;
use crate::types::*;

use super::state::SelectionState;

/// Squad group - Total War style formation preservation
#[derive(Clone)]
pub struct SquadGroup {
    #[allow(dead_code)]
    pub id: u32,
    pub squad_ids: Vec<u32>,
    pub squad_offsets: HashMap<u32, Vec3>,     // Relative offsets from group center (in original coordinate system)
    pub original_formation_facing: Vec3,        // Original facing direction when group was created (never changes)
    pub formation_facing: Vec3,                 // Current facing direction (updates with each move)
}

/// Check if the current selection is exactly one complete group (all living squads in a group are selected)
/// Uses squad_manager to filter out dead squads (those with no members)
pub fn check_is_complete_group(selection_state: &SelectionState, squad_manager: &SquadManager) -> Option<u32> {
    if selection_state.selected_squads.is_empty() {
        return None;
    }

    // Check if all selected squads belong to the same group
    let mut group_id: Option<u32> = None;
    for &squad_id in &selection_state.selected_squads {
        if let Some(&gid) = selection_state.squad_to_group.get(&squad_id) {
            if group_id.is_none() {
                group_id = Some(gid);
            } else if group_id != Some(gid) {
                // Squads belong to different groups
                return None;
            }
        } else {
            // At least one squad is not in a group
            return None;
        }
    }

    // Check if all LIVING squads in the group are selected
    if let Some(gid) = group_id {
        if let Some(group) = selection_state.groups.get(&gid) {
            // Only consider squads that are still alive (have members)
            let living_squad_ids: Vec<u32> = group.squad_ids.iter()
                .filter(|&&id| {
                    squad_manager.get_squad(id)
                        .map_or(false, |s| !s.members.is_empty())
                })
                .copied()
                .collect();

            // Group needs at least 1 living squad to be valid
            if living_squad_ids.is_empty() {
                return None;
            }

            let all_living_in_selection = living_squad_ids.iter()
                .all(|id| selection_state.selected_squads.contains(id));
            if all_living_in_selection {
                return Some(gid);
            }
        }
    }

    None
}

/// System: Handle group toggle with G key (group if ungrouped, ungroup if grouped)
pub fn group_command_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection_state: ResMut<SelectionState>,
    squad_manager: Res<SquadManager>,
) {
    // Toggle group with G (or U to ungroup)
    if keyboard.just_pressed(KeyCode::KeyG) || keyboard.just_pressed(KeyCode::KeyU) {
        if selection_state.selected_squads.is_empty() {
            return;
        }

        let selected_squads = selection_state.selected_squads.clone();

        // Check if all selected squads are already in the same group
        let all_in_same_group = if let Some(&first_group) = selection_state.squad_to_group.get(&selected_squads[0]) {
            selected_squads.iter().all(|&id| {
                selection_state.squad_to_group.get(&id) == Some(&first_group)
            })
        } else {
            false
        };

        // If U is pressed or all are already grouped together, ungroup
        if keyboard.just_pressed(KeyCode::KeyU) || all_in_same_group {
            let mut ungrouped_count = 0;

            for &squad_id in &selected_squads {
                if let Some(&group_id) = selection_state.squad_to_group.get(&squad_id) {
                    selection_state.squad_to_group.remove(&squad_id);
                    ungrouped_count += 1;

                    // Remove from group
                    if let Some(group) = selection_state.groups.get_mut(&group_id) {
                        group.squad_ids.retain(|&id| id != squad_id);
                        group.squad_offsets.remove(&squad_id);
                    }
                }
            }

            // Clean up empty groups
            selection_state.groups.retain(|_, group| !group.squad_ids.is_empty());

            if ungrouped_count > 0 {
                info!("Ungrouped {} squads", ungrouped_count);
            }
        } else if selected_squads.len() >= 2 {
            // Group the selected squads

            // Remove existing group memberships for selected squads
            for &squad_id in &selected_squads {
                if let Some(&old_group_id) = selection_state.squad_to_group.get(&squad_id) {
                    selection_state.squad_to_group.remove(&squad_id);

                    // Remove from old group's squad list
                    if let Some(old_group) = selection_state.groups.get_mut(&old_group_id) {
                        old_group.squad_ids.retain(|&id| id != squad_id);
                        old_group.squad_offsets.remove(&squad_id);
                    }
                }
            }

            // Clean up empty groups
            selection_state.groups.retain(|_, group| !group.squad_ids.is_empty());

            // Calculate group center from squad positions
            let mut group_center = Vec3::ZERO;
            let mut valid_squad_count = 0;
            let mut avg_facing = Vec3::ZERO;

            for &squad_id in &selected_squads {
                if let Some(squad) = squad_manager.get_squad(squad_id) {
                    group_center += squad.center_position;
                    avg_facing += squad.facing_direction;
                    valid_squad_count += 1;
                }
            }

            if valid_squad_count > 0 {
                group_center /= valid_squad_count as f32;
                avg_facing = avg_facing.normalize_or_zero();

                // Calculate offsets for each squad
                let mut squad_offsets = HashMap::new();
                let mut squad_ids = Vec::new();

                for &squad_id in &selected_squads {
                    if let Some(squad) = squad_manager.get_squad(squad_id) {
                        let offset = squad.center_position - group_center;
                        squad_offsets.insert(squad_id, offset);
                        squad_ids.push(squad_id);
                    }
                }

                // Create new group
                let group_id = selection_state.next_group_id;
                selection_state.next_group_id += 1;

                let group = SquadGroup {
                    id: group_id,
                    squad_ids: squad_ids.clone(),
                    squad_offsets,
                    original_formation_facing: avg_facing,  // Store original facing (never changes)
                    formation_facing: avg_facing,            // Current facing (will be updated on moves)
                };

                selection_state.groups.insert(group_id, group);

                // Update squad-to-group mapping
                for squad_id in squad_ids {
                    selection_state.squad_to_group.insert(squad_id, group_id);
                }

                info!("Created group {} with {} squads", group_id, valid_squad_count);
            }
        } else if selected_squads.len() == 1 {
            info!("Need at least 2 squads to create a group");
        }
    }
}
