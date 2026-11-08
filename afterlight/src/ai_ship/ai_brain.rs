use bevy::prelude::*;

use crate::components::*;
use crate::ai_ship::components::*;
use crate::spatial::CreatureGrid;

/// Priority-scorer AI decision system for AI ships.
/// Each faction has its own unique decision tree.
/// Runs on a 0.25s tick.
///
/// TARGET SELECTION: alongside each faction's own trigger logic (unchanged),
/// every ship also gets a faction-agnostic `best_target` computed once per
/// tick — the highest (size+firepower)/distance score among the player and
/// every other living AI ship within this ship's own engage_range. This is
/// what lets a ship pull off a nearby tiny raider onto a much bigger,
/// farther threat instead of just fixating on whatever's closest.
///
/// Only the "attack anything/anyone in range" arms — the ones already
/// worded that way in each faction's own flavor text (Broken Choir, Terran Hegemony's
/// generic engage, Gilded Throne's tactical engage, Recursive Kingdom, Eternal Hegemony,
/// The Shepherd) — actually USE best_target for their fire-at-this position.
/// Player-specific narrative behaviors (Synthesis Collective protecting creatures
/// FROM the player, Corpse Stars ramming intruders out of their depth
/// zone, The Silence's player-shadowing, Stellar Preserve's flee-only) keep
/// targeting the player exactly as before — generalizing those would blur
/// what makes each faction distinct.
///
/// "Under fire → retaliate" arms (every faction that has one) use
/// `attacker_target` — resolved from AiShipState.last_attacker, which
/// process_ai_ship_damage_system fills in from AiShipDamaged.attacker (the
/// actual ship, player or AI, that fired the shot). A ship caught in
/// another AI ship's crossfire now fights back against THAT ship instead
/// of reflexively assuming it was the player. Falls back to best_target if
/// the real attacker died, left range, or was never recorded.
/// How near something hostile comes before a civilian hull runs from it.
const CIVILIAN_ALARM: f32 = 15_000.0;

/// The way a civilian hull runs: away from every threat at once, the nearer
/// ones counting for more, and -- once it's running -- mostly the way it's
/// already going, so the next threat to edge closer doesn't swing it round.
/// None with nothing to run from.
pub(crate) fn flee_heading(pos: Vec2, threats: &[Vec2], running_to: Option<Vec2>) -> Option<Vec2> {
    if threats.is_empty() {
        return None;
    }
    let away: Vec2 = threats
        .iter()
        .map(|t| {
            let d = pos - *t;
            d.normalize_or(Vec2::X) / d.length().max(300.0)
        })
        .sum();
    let away = away.normalize_or(Vec2::X);
    Some(match running_to.map(|d| (d - pos).normalize_or_zero()) {
        Some(current) if current != Vec2::ZERO => (current * 0.75 + away * 0.25).normalize_or(away),
        _ => away,
    })
}

pub fn ai_brain_system(
    time: Res<Time>,
    mut ai_ships: Query<(
        Entity,
        &Transform,
        &AiShipType,
        &mut AiShipState,
        &mut AiShipBehavior,
        &mut AiShipNav,
        &mut AiShipDecisionTimer,
        &mut AiShipTarget,
        &Children,
        Option<&AiLoadout>,
        Option<&super::fleet::FleetMember>,
        Has<super::fleet::CivilianHull>,
        Has<super::standing_spawns::PlayerEscort>,
        Has<super::standing_spawns::Hunter>,
    )>,
    player_ship: Query<(Entity, &Transform, &Children), With<Ship>>,
    player_weapon_query: Query<(&Weapon, &Module), Without<OwnedByAiShip>>,
    creature_query: Query<(Entity, &Transform, &Creature), Without<Ship>>,
    wreck_query: Query<(Entity, &Transform, &AiShipWreck)>,
    weapon_query: Query<(&Weapon, &Module, &OwnedByAiShip), Without<Engine>>,
    creature_grid: Res<CreatureGrid>,
    // Ships riding out a reactor breach — see the BERSERK arm below.
    melting_down: Query<Entity, With<ReactorMeltdown>>,
    standing: Res<crate::diplomacy::FactionStanding>,
    // Faction stations hold fire for their own rule, and reach: a hull
    // kilometres across has guns in range long before its centre is.
    stations: Query<(), With<crate::diplomacy::FactionStation>>,
    reach: Query<&crate::combat::shields::ShipShield>,
    // What each ship makes of the fight (ai_ship::awareness).
    field: Res<super::awareness::Battlefield>,
    mind: Res<super::awareness::Mind>,
    minds: Query<&super::awareness::Awareness>,
) {
    // --- Pre-pass: snapshot every living ship's position + "threat value"
    // (block count + 2x active weapon damage — cheap proxies for size and
    // firepower) BEFORE the mutable per-ship loop below. Two sequential
    // .iter() calls on the same query is fine in Bevy; it's the same
    // pattern as any read-then-mutate pass, just split across two loops.
    fn threat_value(children: &Children, weapon_dmg: f32) -> f32 {
        children.iter().count() as f32 + weapon_dmg * 2.0
    }

    let mut ai_snapshot: Vec<(Entity, Vec2, f32)> = Vec::new();
    let mut ai_factions: Vec<(Entity, AiShipType)> = Vec::new();
    // Who each fleet forms up on: its leader, or if the leader is gone, the
    // living member nearest the formation's centre. (fleet, ship, position,
    // facing, slot)
    let mut fleet_anchors: Vec<(u32, Entity, Vec2, Quat, Vec2)> = Vec::new();
    for (entity, transform, ship_type, state, _, _, _, _, children, _, member, _, escort, _) in ai_ships.iter() {
        if !state.is_destroyed {
            ai_factions.push((entity, *ship_type));
            if let Some(m) = member.filter(|_| !escort) {
                let candidate = (m.fleet, entity, transform.translation.truncate(), transform.rotation, m.slot);
                match fleet_anchors.iter_mut().find(|a| a.0 == m.fleet) {
                    None => fleet_anchors.push(candidate),
                    Some(held) => {
                        if m.leader || (held.4 != Vec2::ZERO && m.slot.length() < held.4.length()) {
                            *held = candidate;
                        }
                    }
                }
            }
        }
        if state.is_destroyed { continue; }
        let weapon_dmg: f32 = children.iter()
            .filter_map(|c| weapon_query.get(c).ok())
            .filter(|(_, module, _)| module.is_active && module.health > 0.0)
            .map(|(w, _, _)| w.damage)
            .sum();
        ai_snapshot.push((entity, transform.translation.truncate(), threat_value(children, weapon_dmg)));
    }

    let player_snapshot: Option<(Entity, Vec2, f32)> = player_ship.iter().next().map(|(e, t, children)| {
        let weapon_dmg: f32 = children.iter()
            .filter_map(|c| player_weapon_query.get(c).ok())
            .filter(|(_, module)| module.is_active && module.health > 0.0)
            .map(|(w, _)| w.damage)
            .sum();
        (e, t.translation.truncate(), threat_value(children, weapon_dmg))
    });

    // An escort forms up on you: where you are and which way you face.
    let player_frame: Option<(Vec2, Quat)> =
        player_ship.iter().next().map(|(_, t, _)| (t.translation.truncate(), t.rotation));

    for (entity, transform, ship_type, mut state, mut behavior, mut nav, mut timer, mut ai_target, children, _, member, civilian, escort, hunter) in ai_ships.iter_mut() {
        timer.timer.tick(time.delta());
        if !timer.timer.just_finished() {
            continue;
        }

        state.last_hit_timer += 0.25;

        if state.is_destroyed {
            *behavior = AiShipBehavior::Dead;
            continue;
        }

        // Your standing decides who comes for you: only a faction that counts
        // you Hostile treats you as a target. A ship you've fired on fires
        // back whatever its faction thinks of you.
        let player_snapshot = player_snapshot.filter(|(player, _, _)| {
            standing.attacks_player(*ship_type)
                || state.last_attacker == Some(*player)
                // Answering a fleet-mate (or a faction-mate's distress call)
                // that you're fighting.
                || (state.alert_timer > 0.0 && state.alert_target == Some(*player))
        });

        let pos = transform.translation.truncate();

        // BERSERK: a ship with a breached core has seconds to live and knows
        // it. It stops evaluating anything — no fleeing, no patrol — and
        // spends the countdown on whoever cracked it. Cracking a reactor buys
        // a kill, but the last eight seconds are the dangerous ones.
        if melting_down.contains(entity) {
            *behavior = AiShipBehavior::Engaging;
            if ai_target.entity.is_none() {
                ai_target.entity = state.last_attacker.or(player_snapshot.map(|(e, _, _)| e));
            }
            continue;
        }

        let under_fire = state.last_hit_timer < 5.0;
        let me = field.hulls.get(&entity).copied();
        // What's left of the whole ship: hull_integrity only counts its floor,
        // and read a ship shot to pieces as nearly whole.
        let hull_pct = me.map_or(state.hull_integrity, |m| m.blocks_left());
        let fuel_pct = state.fuel / state.max_fuel.max(1.0);
        let depth = (-pos.y / 10.0).max(0.0);

        // Detection/engagement range for the "should I start fighting" checks
        // below. It must stay above movement.rs's standoff distance (~0.55x
        // max weapon range, capped at 3000) so a ship engages from farther out
        // than it wants to hold, then closes in — otherwise it would never
        // start the fight from its own weapon range. 1.05x max range clears the
        // standoff comfortably, so the two systems don't oscillate between
        // engage and disengage.
        // From the survey: built guns stand on turrets, a level below the
        // ship, and reading its own children saw none of them, so a ship with
        // guns reaching 8 km didn't start a fight until 4.
        let max_weapon_range = me.map_or(0.0, |m| m.any_reach());
        let engage_range = if max_weapon_range > 0.0 { max_weapon_range * 1.05 } else { 4400.0 };
        // Guns ride the hull, so a big hull reaches further than its centre.
        let engage_range = engage_range + reach.get(entity).map_or(0.0, |s| s.radius * 0.8);

        // A faction station doesn't patrol, flee or ram: it fires on whatever
        // its faction is at war with, and on you if its faction counts you
        // Hostile or you've fired on it (player_snapshot is already gated).
        if stations.contains(entity) {
            let hostile = |e: Entity| {
                ai_factions.iter().find(|(f, _)| *f == e).is_some_and(|(_, t)| {
                    crate::ai_ship::components::factions_hostile(*ship_type, *t)
                })
            };
            let target = player_snapshot
                .iter()
                .map(|&(e, p, _)| (e, p))
                .chain(ai_snapshot.iter().filter(|(e, _, _)| *e != entity && hostile(*e)).map(|&(e, p, _)| (e, p)))
                .filter(|(_, p)| pos.distance(*p) < engage_range)
                .min_by(|(_, a), (_, b)| pos.distance(*a).total_cmp(&pos.distance(*b)));
            match target {
                Some((e, p)) => {
                    *behavior = AiShipBehavior::Engaging;
                    ai_target.entity = Some(e);
                    ai_target.position = p;
                }
                None => {
                    *behavior = AiShipBehavior::Idle;
                    ai_target.entity = None;
                }
            }
            continue;
        }

        // How much of the battery is still shooting. A ship that has been
        // disarmed doesn't heroically hold station waiting to be finished —
        // it runs, which is what keeps a won fight from turning into two
        // minutes of chasing a toothless hull around. Catch it or let it go.
        //
        // Out of the guns it was BUILT with (AiLoadout): dead modules are
        // despawned, so counting the survivors read a ship down to its last
        // gun as fully armed and the withdraw below never tripped. No guns
        // at all is 0, not "all of them".
        //
        // From the survey, which sees turret guns: counting the deck alone
        // read a fully armed ship as disarmed, and the Corpse Stars ran from
        // every fight they were ever in.
        let guns_left = me.map_or(1.0, |m| if m.peak_guns == 0 { 0.0 } else { m.guns_left() });

        // Perception
        let player_info = player_snapshot.map(|(e, p, _)| (e, p, pos.distance(p)));

        // Faction-agnostic target pick: highest value/distance among the
        // player + every OTHER living AI ship within THIS ship's own
        // engage_range. Only consulted by the "attack anything in range"
        // arms below — see the function doc comment for which ones.
        //
        // STICKY: the target this ship already has (ai_target.entity, from
        // last tick) gets its score boosted before comparing. Recomputing
        // a fresh "best" from scratch every 0.25s meant that in any cluster
        // of 3+ ships, near-tied scores flipped the winner tick to tick —
        // nav.destination jumped to a totally different ship's position
        // each time, and the standoff-orbit logic in movement.rs whipped
        // the heading around chasing it (the "moving all over the place"
        // playtest report). A held target now stays locked until something
        // clearly outclasses it (40%+ better score) or it dies/leaves range.
        const TARGET_STICKINESS: f32 = 1.4;
        let held_target = ai_target.entity;
        // Only ships its faction is at war with: this once counted every
        // ship in range, its own fleet-mates included, so a battle group
        // opened fire on itself the moment it materialised.
        let at_war = |e: Entity| {
            ai_factions.iter().any(|(f, t)| *f == e && crate::ai_ship::components::factions_hostile(*ship_type, *t))
        };
        let awareness = minds.get(entity).ok();
        let fleet_call = member.and_then(|m| field.fleet_calls.get(&m.fleet)).copied();
        // What each contact is worth shooting (awareness::target_worth): what
        // it can do, what killing it takes, who it's hurting, what the fleet
        // has called, and who else is already on it.
        let worth = |e: Entity, hull: &super::awareness::Hull, v: f32| -> f32 {
            let held = Some(e) == held_target;
            let Some(me) = me else {
                return if held { v * TARGET_STICKINESS } else { v };
            };
            let shooting_me = hull.target == Some(entity) || awareness.is_some_and(|a| a.hurt_by.iter().any(|(by, _)| *by == e));
            let allies = field.hulls.iter().filter(|(a, h)| **a != entity && h.target == Some(e) && h.faction == Some(*ship_type)).count() as u32;
            super::awareness::target_worth(&me, hull, shooting_me, fleet_call == Some(e), allies, held, &mind)
        };
        let best_target: Option<(Entity, Vec2)> = player_snapshot.iter()
            .map(|&(e, p, v)| (e, p, v))
            .chain(ai_snapshot.iter().filter(|(e, _, _)| *e != entity && at_war(*e)).map(|&(e, p, v)| (e, p, v)))
            .filter(|(_, p, _)| pos.distance(*p) < engage_range)
            .map(|(e, p, v)| {
                let score = field.hulls.get(&e).map_or(v / pos.distance(p).max(200.0), |hull| worth(e, hull, v));
                (e, p, score)
            })
            .max_by(|(_, _, a), (_, _, b)| a.total_cmp(b))
            .map(|(e, p, _)| (e, p));

        // Real attacker resolution for "under fire" retaliation — replaces
        // guessing "it was probably the player" or "whatever's biggest"
        // with "whoever ACTUALLY shot me" (state.last_attacker, set by
        // process_ai_ship_damage_system from AiShipDamaged.attacker).
        // Falls back to best_target if the attacker died, left range, or
        // was never recorded (fresh spawn, only non-attributable damage
        // like fire DoT so far).
        let attacker_target: Option<(Entity, Vec2)> = state.last_attacker
            .and_then(|a| {
                player_snapshot.filter(|(e, _, _)| *e == a).map(|(e, p, _)| (e, p))
                    .or_else(|| ai_snapshot.iter().find(|(e, _, _)| *e == a).map(|&(e, p, _)| (e, p)))
            })
            .or(best_target);

        // Spatial-grid narrowed: only the creature(s) in nearby cells are distance-checked,
        // instead of every creature in the world, every 0.25s, for every AI ship.
        let nearest_creature = creature_grid.0.nearest(pos, 600.0, None)
            .and_then(|(e, d)| creature_query.get(e).ok().map(|(_, t, c)| {
                (e, d, t.translation.truncate(), c.creature_type)
            }));

        // Only the Recursive Kingdom scorer consumes this — wide range so
        // scavenger waves spawned at the edge of the area actually smell
        // the carcass and burn inward; picked-clean wrecks don't count.
        let nearest_wreck = wreck_query.iter()
            .filter(|(_, _, aw)| aw.loot_remaining > 0)
            .map(|(e, t, _)| {
                let d = pos.distance(t.translation.truncate());
                (e, d, t.translation.truncate())
            })
            .filter(|(_, d, _)| *d < 3000.0)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        struct ScoredAction {
            score: f32,
            behavior: AiShipBehavior,
            destination: Option<Vec2>,
            /// Who/where to actually FIRE at — distinct from `destination`,
            /// which is sometimes an offset (Gilded Throne's flank point,
            /// Corpse Stars's ram-from-above point), not the target itself.
            target: Option<(Entity, Vec2)>,
        }

        let mut actions: Vec<ScoredAction> = Vec::with_capacity(12);

        // A fleet escort's place: its slot on whoever the fleet forms up on,
        // turned with that ship. None for a leader or a ship alone.
        let formation_facing: Option<f32> = member.and_then(|m| {
            if escort {
                return player_frame.map(|(_, facing)| facing.to_euler(EulerRot::ZYX).0);
            }
            fleet_anchors
                .iter()
                .find(|a| a.0 == m.fleet && a.1 != entity)
                .map(|a| a.3.to_euler(EulerRot::ZYX).0)
        });
        let formation_point: Option<Vec2> = member.and_then(|m| {
            if escort {
                return player_frame.map(|(at, facing)| at + (facing * m.slot.extend(0.0)).truncate());
            }
            fleet_anchors
                .iter()
                .find(|a| a.0 == m.fleet && a.1 != entity)
                .map(|&(_, _, at, facing, anchor_slot)| at + (facing * (m.slot - anchor_slot).extend(0.0)).truncate())
        });

        // ====================================================================
        // DISTRESS RESPONSE — faction-agnostic, evaluated before the per-
        // faction trees. A same-faction ally within earshot called for backup
        // (ai_distress_system set alert_target/alert_timer); converge on and
        // engage whoever they're fighting. This is what makes a whole patrol
        // pile onto the player once one of them is scrapping, without spawning
        // any ships out of thin air. Non-combatants (The Silence, Stellar Preserve)
        // never answer. Scored at 93 — above a faction's ordinary engage
        // (78-90) but below its own critical-flee/kamikaze (95-100), so
        // self-preservation and each faction's signature move still win.
        // Target position is resolved from the same snapshots best_target
        // uses; if the caller's target has died or left range, the alert
        // simply can't be acted on this tick and the ship falls through to
        // its normal behavior.
        // ====================================================================
        // hull_pct gate: a ship that's already hurt keeps its own self-
        // preservation (flee/kamikaze) instead of charging back in to help —
        // you don't answer a call for backup when you're the one about to die.
        if state.alert_timer > 0.0 && faction_fights(*ship_type) && hull_pct > 0.35 {
            let resolved = state.alert_target.and_then(|at| {
                player_snapshot
                    .filter(|(e, _, _)| *e == at)
                    .map(|(_, p, _)| (at, p))
                    .or_else(|| {
                        ai_snapshot.iter().find(|(e, _, _)| *e == at).map(|&(_, p, _)| (at, p))
                    })
            });
            if let Some((at, apos)) = resolved {
                actions.push(ScoredAction {
                    score: 93.0,
                    behavior: AiShipBehavior::Engaging,
                    destination: Some(apos),
                    target: Some((at, apos)),
                });
            }
        }

        // ====================================================================
        // Per-faction unique decision trees
        // ====================================================================
        match *ship_type {
            // ----------------------------------------------------------------
            // TRADERS (Merchant Princes, Free Haulers' Compact): no warfleet.
            // Shot at, they run from whoever did it and let their escort and
            // their ally do the fighting (ai_distress_system); otherwise they
            // keep their course. Their civilian hulls run from any threat
            // anyway (CivilianHull, below); this covers the rest, the Arms
            // Bazaar among them.
            // ----------------------------------------------------------------
            AiShipType::MerchantPrinces | AiShipType::FreeHaulersCompact => {
                if under_fire {
                    if let Some((_, t_pos)) = attacker_target {
                        let away = (pos - t_pos).normalize_or(Vec2::X);
                        actions.push(ScoredAction {
                            score: 90.0,
                            behavior: AiShipBehavior::Fleeing,
                            destination: Some(pos + away * 4_000.0),
                            target: None,
                        });
                    }
                }
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 40.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // LEVIATHAN RIDERS: Hunt creatures, avoid combat, flee when hurt
            // Prioritize: find creatures > capture > flee > patrol
            // ----------------------------------------------------------------
            AiShipType::StellarPreserve => {
                // Critical flee
                if hull_pct < 0.25 || fuel_pct < 0.1 {
                    actions.push(ScoredAction {
                        score: 100.0,
                        behavior: AiShipBehavior::Fleeing,
                        destination: Some(pos + Vec2::new(0.0, 600.0)),
                        target: None,
                    });
                }

                // Flee from player if under fire (not a fighter)
                if under_fire {
                    if let Some((_, p_pos, _)) = player_info {
                        let away = (pos - p_pos).normalize_or_zero();
                        actions.push(ScoredAction {
                            score: 85.0,
                            behavior: AiShipBehavior::Fleeing,
                            destination: Some(pos + away * 500.0),
                            target: None,
                        });
                    }
                }

                // Chase creatures to capture them (primary purpose)
                if let Some((_, dist, c_pos, _)) = nearest_creature {
                    if dist < 500.0 {
                        actions.push(ScoredAction {
                            score: 75.0,
                            behavior: AiShipBehavior::Salvaging, // "salvaging" = capturing
                            destination: Some(c_pos),
                            target: None,
                        });
                    }
                }

                // Patrol hunting grounds
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 40.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // ABYSSAL CULT: Protect creatures, attack creature-killers,
            // kamikaze ram when below 20% HP, patrol sacred waters
            // ----------------------------------------------------------------
            AiShipType::SynthesisCollective => {
                // KAMIKAZE when critically damaged - ram nearest target
                // Detection range must exceed the standoff (movement.rs) or
                // the ship engages, immediately backs off past the trigger
                // distance, drops out of Engaging, drifts back in, and
                // repeats — a fast oscillation that looks like glitching.
                // engage_range is weapon-based now (1.05x max weapon range)
                // so this holds regardless of loadout.
                if hull_pct < 0.20 {
                    if let Some((_, p_pos, dist)) = player_info {
                        if dist < engage_range {
                            actions.push(ScoredAction {
                                score: 100.0,
                                behavior: AiShipBehavior::Engaging,
                                destination: Some(p_pos), // ram into player
                                target: player_info.map(|(pe, pp, _)| (pe, pp)),
                            });
                        }
                    }
                }

                // Attack player if they're near creatures (protecting sea life)
                if let Some((_, c_dist, _c_pos, _)) = nearest_creature {
                    if let Some((_, p_pos, p_dist)) = player_info {
                        // Player near a creature = threat to sacred life
                        if p_dist < engage_range && c_dist < 400.0 {
                            actions.push(ScoredAction {
                                score: 88.0,
                                behavior: AiShipBehavior::Engaging,
                                destination: Some(p_pos),
                                target: player_info.map(|(pe, pp, _)| (pe, pp)),
                            });
                        }
                    }
                }

                // And anyone its faction has declared against, in range.
                if let Some((_, t_pos)) = best_target {
                    actions.push(ScoredAction {
                        score: 78.0,
                        behavior: AiShipBehavior::Engaging,
                        destination: Some(t_pos),
                        target: best_target,
                    });
                }

                // Under fire → fight back (fanatics don't flee easily),
                // against whoever actually shot them.
                if under_fire {
                    if let Some((_, t_pos)) = attacker_target {
                        actions.push(ScoredAction {
                            score: 82.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: attacker_target,
                        });
                    }
                }

                // Escort creatures (follow them around)
                if let Some((_, _, c_pos, _)) = nearest_creature {
                    actions.push(ScoredAction {
                        score: 60.0,
                        behavior: AiShipBehavior::FollowingTradeRoute,
                        destination: Some(c_pos + Vec2::new(100.0, 0.0)),
                        target: None,
                    });
                }

                // Patrol sacred waters
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 35.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // THE DROWNED: Mindless ghost ships. Attack everything in range.
            // No fleeing, no self-preservation. Erratic movement.
            // ----------------------------------------------------------------
            AiShipType::BrokenChoir => {
                // Never flee - already dead, can't die again (narratively)

                // Attack anything nearby (detection must exceed the
                // standoff distance — see Synthesis Collective kamikaze comment).
                // "Anything" is now literal: best_target, not just player.
                if let Some((_, t_pos)) = best_target {
                    if pos.distance(t_pos) < engage_range {
                        actions.push(ScoredAction {
                            score: 80.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: best_target,
                        });
                    }
                }

                // Attack creatures too (mindless aggression). No ship-vs-
                // creature targeting system exists yet, so this still
                // fires at the player if one's around — same as before.
                if let Some((_, dist, c_pos, _)) = nearest_creature {
                    if dist < 300.0 {
                        actions.push(ScoredAction {
                            score: 70.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(c_pos),
                            target: player_info.map(|(pe, pp, _)| (pe, pp)),
                        });
                    }
                }

                // Wander aimlessly - repeat old patrol routes (ghost behavior)
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 50.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }

                // Just drift if nothing else
                actions.push(ScoredAction {
                    score: 20.0,
                    behavior: AiShipBehavior::Idle,
                    destination: Some(pos + Vec2::new(50.0, -30.0)),
                    target: None,
                });
            }

            // ----------------------------------------------------------------
            // PRESSURE KINGS: Deep-zone gatekeepers.
            // Attack anyone above 800m in their territory.
            // Ram intruders upward. Don't flee. Ignore shallow threats.
            // ----------------------------------------------------------------
            AiShipType::CorpseStars => {
                // Shot at, it answers wherever it is: above its own depth it
                // used to turn its back and sink while you shelled it.
                if depth < 600.0 && under_fire {
                    if let Some((_, t_pos)) = attacker_target {
                        actions.push(ScoredAction {
                            score: 95.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: attacker_target,
                        });
                    }
                }
                // Only active in deep void - idle near station
                if depth < 600.0 {
                    actions.push(ScoredAction {
                        score: 90.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: Some(pos + Vec2::new(0.0, -500.0)), // go deeper
                        target: None,
                    });
                } else {
                    // RAM anyone upward, out of the deep territory — their
                    // own flavor text says "anyone," not "the player"
                    // specifically. Mis-scoped player-only in an earlier
                    // pass; best_target fixes it (detection must exceed the
                    // standoff — see Synthesis Collective's kamikaze comment).
                    if let Some((t_entity, t_pos)) = best_target {
                        if pos.distance(t_pos) < engage_range {
                            // Position above the target to push it up
                            let ram_pos = Vec2::new(t_pos.x, t_pos.y + 150.0);
                            actions.push(ScoredAction {
                                score: 90.0,
                                behavior: AiShipBehavior::Engaging,
                                destination: Some(ram_pos),
                                target: Some((t_entity, t_pos)),
                            });
                        }
                    }

                    // Attack whoever's under fire — retaliate against the
                    // actual attacker, not just whoever's player-shaped.
                    if under_fire {
                        if let Some((_, t_pos)) = attacker_target {
                            actions.push(ScoredAction {
                                score: 95.0,
                                behavior: AiShipBehavior::Engaging,
                                destination: Some(t_pos),
                                target: attacker_target,
                            });
                        }
                    }

                    // Retreat when nearly dead OR shot toothless — a fighter
                    // with a third of its guns left has nothing to win by
                    // staying, and hull_pct alone almost never trips now that
                    // fights are decided by subsystems rather than by grinding
                    // plating (see combat::check_ai_cripple).
                    if hull_pct < 0.10 || guns_left <= 0.35 {
                        actions.push(ScoredAction {
                            score: 98.0,
                            behavior: AiShipBehavior::Fleeing,
                            destination: Some(pos + (pos - player_info.map(|(_, p, _)| p).unwrap_or(pos)).normalize_or_zero() * 900.0),
                            target: None,
                        });
                    }
                }

                // Deep patrol
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 40.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // GLASS EYE: Silent watchers. NEVER attack. Always flee.
            // Follow player from safe distance. Fastest flee speed.
            // ----------------------------------------------------------------
            AiShipType::TheSilence => {
                // If under fire, flee at maximum speed
                if under_fire {
                    let flee_dest = if let Some((_, p_pos, _)) = player_info {
                        let away = (pos - p_pos).normalize_or_zero();
                        pos + away * 1000.0 // flee very far
                    } else {
                        pos + Vec2::new(0.0, 800.0)
                    };
                    actions.push(ScoredAction {
                        score: 100.0,
                        behavior: AiShipBehavior::Fleeing,
                        destination: Some(flee_dest),
                        target: None,
                    });
                }

                // Shadow the player from 500-800u distance (surveillance)
                if let Some((_, p_pos, dist)) = player_info {
                    if dist < 400.0 {
                        // Too close - back off
                        let away = (pos - p_pos).normalize_or_zero();
                        actions.push(ScoredAction {
                            score: 80.0,
                            behavior: AiShipBehavior::Fleeing,
                            destination: Some(pos + away * 300.0),
                            target: None,
                        });
                    } else if dist < 900.0 {
                        // Good shadowing distance - maintain
                        let toward = (p_pos - pos).normalize_or_zero();
                        let shadow_pos = p_pos - toward * 650.0;
                        actions.push(ScoredAction {
                            score: 70.0,
                            behavior: AiShipBehavior::FollowingTradeRoute,
                            destination: Some(shadow_pos),
                            target: None,
                        });
                    }
                }

                // Roam scanning routes
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 45.0,
                        behavior: AiShipBehavior::FollowingTradeRoute,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // IRON TIDE: Heavy battleship. Engage everything hostile in range.
            // Slow to maneuver. Never flees unless nearly destroyed.
            // Maximum aggression at close range.
            // ----------------------------------------------------------------
            AiShipType::TerranHegemony => {
                // Only retreat at extreme damage
                if hull_pct < 0.10 && fuel_pct < 0.15 {
                    actions.push(ScoredAction {
                        score: 100.0,
                        behavior: AiShipBehavior::Fleeing,
                        destination: Some(pos + Vec2::new(0.0, 500.0)),
                        target: None,
                    });
                }

                // Under fire → immediately engage (battle-hardened),
                // against whoever actually shot them.
                if under_fire {
                    if let Some((_, t_pos)) = attacker_target {
                        actions.push(ScoredAction {
                            score: 95.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: attacker_target,
                        });
                    }
                }

                // Engage ANY target in weapon range (long range weapons) —
                // best_target makes "any" literal: a farther battleship can
                // now outweigh a closer but trivial target.
                if let Some((_, t_pos)) = best_target {
                    if pos.distance(t_pos) < engage_range {
                        actions.push(ScoredAction {
                            score: 85.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: best_target,
                        });
                    }
                }

                // Engage large creatures too (shows dominance). No ship-vs-
                // creature targeting system exists yet, so this still fires
                // at the player if one's around — same as before.
                if let Some((_, dist, c_pos, c_type)) = nearest_creature {
                    let is_large = matches!(c_type, CreatureType::Leviathan);
                    if dist < 400.0 && is_large {
                        actions.push(ScoredAction {
                            score: 75.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(c_pos),
                            target: player_info.map(|(pe, pp, _)| (pe, pp)),
                        });
                    }
                }

                // Slow patrol - battleship doesn't rush
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 40.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // BLACKWATER PMC: Tactical mercs. Hunt bounties (player if hostile).
            // Flank targets. Coordinate. Disengage when outmatched.
            // ----------------------------------------------------------------
            AiShipType::GildedThrone => {
                // Tactical retreat when damaged (live to fight another day)
                if hull_pct < 0.30 {
                    let flee_dest = if let Some((_, p_pos, _)) = player_info {
                        let away = (pos - p_pos).normalize_or_zero();
                        pos + away * 600.0
                    } else {
                        pos + Vec2::new(0.0, 400.0)
                    };
                    actions.push(ScoredAction {
                        score: 90.0,
                        behavior: AiShipBehavior::Fleeing,
                        destination: Some(flee_dest),
                        target: None,
                    });
                }

                // Under fire → fight back, flanking whoever actually shot
                // them.
                if under_fire {
                    if let Some((t_entity, t_pos)) = attacker_target {
                        // FLANK - don't go straight at target, offset to the side
                        let to_target = (t_pos - pos).normalize_or_zero();
                        let flank = Vec2::new(-to_target.y, to_target.x); // perpendicular
                        let flank_pos = t_pos + flank * 200.0;
                        actions.push(ScoredAction {
                            score: 85.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(flank_pos),
                            target: Some((t_entity, t_pos)),
                        });
                    }
                }

                // Engage the best target at medium range (tactical
                // distance) — mercs hunt whatever bounty is worth the
                // most, not just whoever's player-shaped and nearby.
                // Flank math is unchanged, just parameterized on t_pos.
                if let Some((t_entity, t_pos)) = best_target {
                    if pos.distance(t_pos) < engage_range {
                        let to_target = (t_pos - pos).normalize_or_zero();
                        let flank = Vec2::new(-to_target.y, to_target.x);
                        let offset = if pos.x > t_pos.x { 1.0 } else { -1.0 };
                        let flank_pos = t_pos + flank * 180.0 * offset;
                        actions.push(ScoredAction {
                            score: 78.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(flank_pos),
                            target: Some((t_entity, t_pos)),
                        });
                    }
                }

                // Patrol routes
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 45.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // RUST SWARM: Aggressive junk. Attack anything nearby.
            // No self-preservation. Kamikaze when critical. Mine everything.
            // ----------------------------------------------------------------
            AiShipType::RecursiveKingdom => {
                // KAMIKAZE when critical - charge at nearest/biggest target
                // (best_target — genuinely "nearest target" now, not just
                // whichever one happens to be the player)
                if hull_pct < 0.25 {
                    if let Some((_, t_pos)) = best_target {
                        if pos.distance(t_pos) < 2_500.0 {
                            actions.push(ScoredAction {
                                score: 100.0,
                                behavior: AiShipBehavior::Engaging,
                                destination: Some(t_pos), // death charge
                                target: best_target,
                            });
                        }
                    }
                }

                // Attack aggressively — swarm mentality, whatever's closest
                // and worth swarming, not specifically the player.
                // (Anything it can reach: at 500 u it never started a fight at
                // all, on hulls wider than that.)
                if let Some((_, t_pos)) = best_target {
                    if pos.distance(t_pos) < engage_range {
                        actions.push(ScoredAction {
                            score: 80.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: best_target,
                        });
                    }
                }

                // Also chase wrecks for scrap
                if let Some((_, _, w_pos)) = nearest_wreck {
                    actions.push(ScoredAction {
                        score: 65.0,
                        behavior: AiShipBehavior::Salvaging,
                        destination: Some(w_pos),
                        target: None,
                    });
                }

                // Erratic patrol (random, twitchy movement)
                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 35.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }

                // Idle drift
                actions.push(ScoredAction {
                    score: 15.0,
                    behavior: AiShipBehavior::Idle,
                    destination: Some(pos + Vec2::new(30.0, -20.0)),
                    target: None,
                });
            }

            // ----------------------------------------------------------------
            // DREADNOUGHT: Mega-battleship. Never retreats — no flee action
            // at all. Engages anything in an enormous detection range and
            // grinds it down with sheer weapon coverage.
            // ----------------------------------------------------------------
            AiShipType::EternalHegemony => {
                // Under fire → retaliate against whoever actually shot them.
                if under_fire {
                    if let Some((_, t_pos)) = attacker_target {
                        actions.push(ScoredAction {
                            score: 95.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: attacker_target,
                        });
                    }
                }

                // "Engages anything in an enormous detection range" — now
                // actually anything, via best_target.
                if let Some((_, t_pos)) = best_target {
                    if pos.distance(t_pos) < 27_000.0 {
                        actions.push(ScoredAction {
                            score: 90.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: best_target,
                        });
                    }
                }

                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 40.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }

            // ----------------------------------------------------------------
            // VOID TITAN: The apex threat. Never flees, never hesitates — if
            // you're in range, it's already coming for you.
            // ----------------------------------------------------------------
            AiShipType::Shepherd => {
                // "If you're in range, it's already coming for you" —
                // whoever "you" is, via best_target.
                if let Some((_, t_pos)) = best_target {
                    if pos.distance(t_pos) < 36_000.0 {
                        actions.push(ScoredAction {
                            score: 100.0,
                            behavior: AiShipBehavior::Engaging,
                            destination: Some(t_pos),
                            target: best_target,
                        });
                    }
                }

                if !nav.waypoints.is_empty() {
                    actions.push(ScoredAction {
                        score: 40.0,
                        behavior: AiShipBehavior::Patrolling,
                        destination: nav.waypoints.get(nav.current_waypoint).copied(),
                        target: None,
                    });
                }
            }
        }

        // A hunter came for you: it closes and engages wherever you are in
        // the system, whatever its faction's habits, unless its own
        // survival or its signature move scores higher.
        if hunter {
            if let Some((pe, pp, _)) = player_info {
                actions.push(ScoredAction {
                    score: 92.0,
                    behavior: AiShipBehavior::Engaging,
                    destination: Some(pp),
                    target: Some((pe, pp)),
                });
            }
        }

        // A civilian hull runs instead of fighting: from whoever is shooting
        // at it, or from anything hostile that comes near. Its escort is
        // what answers (ai_distress_system).
        if civilian {
            actions.retain(|a| a.behavior != AiShipBehavior::Engaging);
            let hostile = |e: Entity| {
                ai_factions.iter().any(|(f, t)| *f == e && crate::ai_ship::components::factions_hostile(*ship_type, *t))
            };
            // Already running: it stays scared until well clear, not the
            // moment it crosses the alarm line -- that turned it straight back
            // toward the threat, and it ran, turned, ran, its nose swinging.
            let fleeing = *behavior == AiShipBehavior::Fleeing;
            let alarm = if fleeing { CIVILIAN_ALARM * 1.8 } else { CIVILIAN_ALARM };
            let shooter = under_fire.then(|| attacker_target.map(|(_, p)| p)).flatten();
            let threats: Vec<Vec2> = shooter
                .into_iter()
                .chain(player_snapshot.map(|(_, p, _)| p))
                .chain(ai_snapshot.iter().filter(|(e, _, _)| *e != entity && hostile(*e)).map(|&(_, p, _)| p))
                .filter(|p| pos.distance(*p) < alarm)
                .collect();
            if let Some(away) = flee_heading(pos, &threats, fleeing.then_some(nav.destination).flatten()) {
                actions.push(ScoredAction {
                    score: 99.0,
                    behavior: AiShipBehavior::Fleeing,
                    destination: Some(pos + away * 4_000.0),
                    target: None,
                });
            }
        }

        // Default fallback for all factions
        actions.push(ScoredAction {
            score: 5.0,
            behavior: AiShipBehavior::Patrolling,
            destination: nav.waypoints.get(nav.current_waypoint).copied(),
            target: None,
        });

        // Pick highest score
        if let Some(best) = actions.iter().max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal)) {
            // TEMP [AI_VS_AI_DIAGNOSTIC]
            if std::env::var("AFTERLIGHT_AI_VS_AI_TEST").is_ok() {
                info!("[AI_VS_AI brain] {:?} ({:?}) score={:.0} behavior={:?} engage_range={:.0} best_target={:?} attacker_target={:?}",
                    entity, ship_type, best.score, best.behavior, engage_range, best_target, attacker_target);
            }
            *behavior = best.behavior;
            if let Some(dest) = best.destination {
                nav.destination = Some(dest);
            }
            // It has decided this fight is lost: it runs for a friend or for
            // open space, with its guns still on whoever it was fighting.
            let withdrawing = *behavior == AiShipBehavior::Engaging
                && !hunter
                && awareness.is_some_and(|a| a.tactic == super::awareness::Tactic::Withdraw);
            if withdrawing {
                if let (Some(me), Some(them)) = (me, best.target.and_then(|(e, _)| field.hulls.get(&e))) {
                    *behavior = AiShipBehavior::Fleeing;
                    let friends = field.hulls.iter()
                        .filter(|(e, h)| **e != entity && h.faction == Some(*ship_type) && h.lost_rate < 1.0 && h.firepower > 0.0)
                        .map(|(_, h)| h.pos);
                    nav.destination = Some(super::awareness::retreat_point(&me, them, friends));
                }
            }
            // An escort with nothing better to do keeps its place in the
            // formation instead of flying a patrol of its own.
            nav.face = None;
            if *behavior == AiShipBehavior::Patrolling {
                if let Some(slot) = formation_point {
                    nav.destination = Some(slot);
                    nav.face = formation_facing;
                }
            }
            // Combat target for ai_weapon_fire_system — separate from
            // nav.destination (see AiShipTarget's doc comment for why).
            // Non-Engaging actions carry target: None, which correctly
            // clears any stale target once the ship disengages.
            let new_target = best.target.map(|(e, _)| e);
            // A changed target invalidates the aimed subsystem — force
            // ai_weapon_fire_system to re-pick one on the new ship rather than
            // keep aiming at a module that belonged to the old target.
            if ai_target.entity != new_target {
                ai_target.subsystem = None;
                ai_target.aim_at = None;
            }
            ai_target.entity = new_target;
            ai_target.position = best.target.map(|(_, p)| p).unwrap_or(pos);
        }

        // Advance waypoints when near current one
        if let Some(dest) = nav.destination.filter(|_| formation_point.is_none()) {
            if pos.distance(dest) < 80.0 && !nav.waypoints.is_empty() {
                nav.current_waypoint = (nav.current_waypoint + 1) % nav.waypoints.len();
                nav.destination = nav.waypoints.get(nav.current_waypoint).copied();
            }
        }
    }
}


#[cfg(test)]
mod flee_tests {
    use super::*;

    /// Between two threats it runs out to the side, not toward either.
    #[test]
    fn runs_from_both_threats() {
        let h = flee_heading(Vec2::ZERO, &[Vec2::new(1_000.0, 0.0), Vec2::new(-1_000.0, 200.0)], None).unwrap();
        assert!(h.y < -0.9, "{h}");
    }

    /// Once running, a new threat edging in only bends the course.
    #[test]
    fn keeps_its_course_once_running() {
        let pos = Vec2::ZERO;
        let running_to = Some(Vec2::new(0.0, 4_000.0));
        let h = flee_heading(pos, &[Vec2::new(0.0, -2_000.0), Vec2::new(-3_000.0, 1_000.0)], running_to).unwrap();
        assert!(h.y > 0.8, "{h}");
    }

    #[test]
    fn nothing_to_run_from() {
        assert!(flee_heading(Vec2::ZERO, &[], None).is_none());
    }
}
