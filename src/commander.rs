use bevy::prelude::*;
use crate::types::{BattleDroid, SquadMember, Team, SquadManager};

// Commander promotion and visual update system
pub fn commander_promotion_system(
    mut squad_manager: ResMut<SquadManager>,
    mut unit_query: Query<(Entity, &mut SquadMember), With<BattleDroid>>,
) {
    // Collect squads that need commander updates to avoid multiple borrows
    let mut squads_needing_update: Vec<(u32, Entity, Team)> = Vec::new();
    
    // Only check squads where the commander entity doesn't exist anymore
    for (squad_id, squad) in squad_manager.squads.iter() {
        if let Some(commander_entity) = squad.commander {
            // Check if this commander entity still exists
            let commander_exists = unit_query.iter()
                .any(|(entity, squad_member)| 
                    entity == commander_entity && squad_member.squad_id == *squad_id);
            
            if !commander_exists {
                // Commander is dead/missing, we need to promote someone
                squads_needing_update.push((*squad_id, commander_entity, squad.team));
            }
        }
    }
    
    // Update squads that need new commanders
    for (squad_id, _old_commander, _team) in squads_needing_update {
        // Find a new commander from this squad
        let mut potential_commander = None;
        for (entity, squad_member) in unit_query.iter() {
            if squad_member.squad_id == squad_id && !squad_member.is_commander {
                potential_commander = Some(entity);
                break; // Take the first available unit
            }
        }
        
        if let Some(new_commander) = potential_commander {
            // Update squad manager
            if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
                squad.commander = Some(new_commander);
            }
            
            // Update the unit's commander status
            for (entity, mut squad_member) in unit_query.iter_mut() {
                if entity == new_commander && squad_member.squad_id == squad_id {
                    squad_member.is_commander = true;
                    
                    info!("Promoted new commander for squad {} (replaced dead commander)", squad_id);
                    break;
                }
            }
        } else {
            // No units left in squad, clear commander
            if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
                squad.commander = None;
                info!("Squad {} has no remaining units, cleared commander", squad_id);
            }
        }
    }
}

// Commander visual update system - safely updates materials for promoted commanders
pub fn commander_visual_update_system(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut unit_query: Query<(Entity, &mut SquadMember, &Handle<StandardMaterial>, &Children, &BattleDroid), With<BattleDroid>>,
    head_query: Query<(Entity, &Handle<StandardMaterial>), Without<BattleDroid>>,
    squad_manager: Res<SquadManager>,
) {
    // Collect entities that need visual updates to avoid borrowing conflicts
    let mut updates_needed = Vec::new();
    
    for (entity, squad_member, _current_material_handle, children, droid) in unit_query.iter() {
        if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
            let should_be_commander = squad.commander == Some(entity);
            
            // Check if this unit's commander status matches its visual appearance
            if should_be_commander && !squad_member.is_commander {
                // This unit was just promoted to commander
                updates_needed.push((entity, droid.team, true, children.iter().copied().collect::<Vec<_>>()));
            } else if !should_be_commander && squad_member.is_commander {
                // This unit was demoted from commander (shouldn't happen often)
                updates_needed.push((entity, droid.team, false, children.iter().copied().collect::<Vec<_>>()));
            }
        }
    }
    
    // Apply visual updates with unique materials - with robust entity existence checks
    for (entity, team, make_commander, children) in updates_needed {
        // CRITICAL: Double-check entity still exists before applying any changes
        if unit_query.get(entity).is_err() {
            // Entity was destroyed between collection and application, skip it
            continue;
        }
        
        // Update SquadMember component first
        if let Ok((_, mut squad_member, _, _, _)) = unit_query.get_mut(entity) {
            squad_member.is_commander = make_commander;
        } else {
            // Entity no longer exists, skip material updates
            continue;
        }
        
        if make_commander {
            // Create unique commander materials for this promoted unit
            let new_commander_body = materials.add(StandardMaterial {
                base_color: match team {
                    Team::A => Color::srgb(0.9, 0.8, 0.4), // Golden yellow
                    Team::B => Color::srgb(0.9, 0.5, 0.3), // Orange-red
                },
                metallic: 0.5,
                perceptual_roughness: 0.3,
                alpha_mode: AlphaMode::Opaque,
                unlit: false,
                fog_enabled: true,
                ..default()
            });
            
            // Triple check: verify entity still exists AND can be commanded
            if unit_query.get(entity).is_ok() {
                if let Some(mut entity_cmd) = commands.get_entity(entity) {
                    entity_cmd.try_insert(new_commander_body);
                }
            }
            
            // Update head material with same safety checks
            for &child_entity in children.iter() {
                if head_query.get(child_entity).is_ok() {
                    let new_commander_head = materials.add(StandardMaterial {
                        base_color: match team {
                            Team::A => Color::srgb(1.0, 0.9, 0.5), // Bright gold
                            Team::B => Color::srgb(1.0, 0.6, 0.4), // Bright orange
                        },
                        metallic: 0.4,
                        perceptual_roughness: 0.4,
                        ..default()
                    });
                    
                    // Check child still exists before commanding
                    if head_query.get(child_entity).is_ok() {
                        if let Some(mut child_cmd) = commands.get_entity(child_entity) {
                            child_cmd.try_insert(new_commander_head);
                        }
                    }
                }
            }
        } else {
            // Revert to regular materials with safety checks
            let new_regular_body = materials.add(StandardMaterial {
                base_color: match team {
                    Team::A => Color::srgb(0.7, 0.7, 0.8), // Regular blue-gray
                    Team::B => Color::srgb(0.9, 0.9, 0.95), // Regular white
                },
                metallic: match team {
                    Team::A => 0.3,
                    Team::B => 0.4,
                },
                perceptual_roughness: 0.5,
                ..default()
            });
            
            // Triple check before applying demotion materials
            if unit_query.get(entity).is_ok() {
                if let Some(mut entity_cmd) = commands.get_entity(entity) {
                    entity_cmd.try_insert(new_regular_body);
                }
            }
            
            // Update head material with safety checks
            for &child_entity in children.iter() {
                if head_query.get(child_entity).is_ok() {
                    let new_regular_head = materials.add(StandardMaterial {
                        base_color: match team {
                            Team::A => Color::srgb(0.8, 0.6, 0.4), // Regular head
                            Team::B => Color::srgb(0.95, 0.95, 1.0), // Regular head
                        },
                        metallic: match team {
                            Team::A => 0.2,
                            Team::B => 0.3,
                        },
                        perceptual_roughness: 0.6,
                        ..default()
                    });
                    
                    // Check child still exists before commanding
                    if head_query.get(child_entity).is_ok() {
                        if let Some(mut child_cmd) = commands.get_entity(child_entity) {
                            child_cmd.try_insert(new_regular_head);
                        }
                    }
                }
            }
        }
    }
} 