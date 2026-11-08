use bevy::prelude::*;

use crate::components::*;
use crate::events::*;
use crate::combat::{spawn_floating_damage, spawn_hit_effect};
use crate::combat::ammo_types::KineticAmmoType;
use super::components::*;

/// AI ships in Engaging state fire weapons at their current AiShipTarget —
/// the player OR another AI ship, whichever ai_brain picked this tick (see
/// AiShipTarget's doc comment). WHO to target is only re-decided every
/// 0.25s (the brain tick); WHERE to aim is re-read from that target's live
/// Transform every single frame this system runs — AiShipTarget.position is
/// a snapshot from the moment it was picked, stale by up to 0.25s, which
/// was enough for an orbiting/strafing ship (standard combat maneuver, see
/// movement.rs's standoff-orbit) to be gone from that point by the time a
/// shot arrived. Live lookup fixes shots consistently whiffing at range.
///
/// Two upgrades layered on top of that live aim, both to make enemies an
/// actual threat: LEAD PREDICTION — shots aim where the target WILL be, via
/// the same calculate_lead the player's own guns use, so a moving ship no
/// longer simply outruns enemy fire; and SUBSYSTEM AIMING — per-faction
/// aim_priority sends rounds at a specific enemy module (your weapons /
/// engines / reactor) instead of the hull centre, cached on
/// AiShipTarget.subsystem and re-picked when it's destroyed. Damage is
/// resolved by impact geometry, so aiming a shot at your engine genuinely
/// knocks out your engine. Empty-doctrine factions keep aiming centre-of-mass.
/// Whether a ship should be shooting at all this tick.
///
/// Behaviour alone used to be the whole gate, and `ai_brain_system` skips
/// destroyed ships entirely -- so a dead ship's behaviour froze at whatever it
/// held when it died. Anything that went down fighting stayed `Engaging` and
/// kept firing out of a hull the player had just been told was "derelict
/// adrift, ripe for salvage". Salvage is the loop the demo is built around;
/// being shot by the thing you came to board is not a difficulty curve.
///
/// A reactor meltdown is deliberately still armed: `tick_reactor_meltdown`
/// only sets `is_destroyed` at detonation, so the eight seconds of berserk
/// fire between breach and blast survive this check.
fn ship_may_fire(behavior: &AiShipBehavior, state: &AiShipState) -> bool {
    !state.is_destroyed && *behavior == AiShipBehavior::Engaging
}

pub fn ai_weapon_fire_system(
    time: Res<Time>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ai_ships: Query<(
        Entity,
        &Transform,
        &AiShipType,
        &AiShipBehavior,
        &mut AiShipTarget,
        &Children,
        Option<&super::power::AiPowerState>,
        &AiShipState,
    ), With<AiShip>>,
    mut weapon_query: Query<(
        &mut Weapon,
        &mut WeaponCooldown,
        &Module,
        &AmmoStorage,
        &OwnedByAiShip,
        Option<&ModuleEfficiency>,
        // A weapon's OWN position and mount. AI guns fired from the ship root
        // through a 360-degree arc, so every gun on the ship was effectively
        // the same gun at the same point — a stern turret shot forward through
        // its own hull, and a dozen mounts all muzzle-flashed on top of each
        // other. Both of those the player has never been able to do.
        &GlobalTransform,
        &WeaponMount,
        // Faction loadouts have set this since layouts.rs was written
        // (apply_module_extras on spawn); nothing ever read it at fire time,
        // so every AI shot resolved as unspecialised regardless.
        Option<&crate::building::customization::tuning::SelectedAmmo>,
        // A launcher's missile design (combat::missile_design).
        Option<&crate::combat::missile_design::MissileLoadout>,
        // On point defence: shoots at incoming missiles, nothing else.
        Has<crate::combat::targeting::fire_groups::InterceptMode>,
    )>,
    player_query: Query<(Entity, &Transform), With<Ship>>,
    target_transform_query: Query<&Transform>,
    // Ship hit detection (combat/projectiles.rs) centers on the shield's
    // world_center — the blocks' centroid, not the root, since the root is
    // often at one end of the layout. Aiming at the raw root position was
    // consistent geometry mismatch on any ship with an off-center layout.
    target_shield_query: Query<&crate::combat::shields::ShipShield>,
    // Live world position + health of ANY module on ANY ship (player or AI) —
    // used to aim at, and lazily re-pick, a specific enemy subsystem.
    module_pos_query: Query<(&Module, &GlobalTransform)>,
    children_query: Query<&Children>,
    // Target velocity, for lead prediction. Player and AI ships both carry it.
    velocity_query: Query<&Velocity>,
    mut fired_events: MessageWriter<WeaponFired>,
    standing: Res<crate::diplomacy::FactionStanding>,
    // Guns built from parts, the hull's and those on its turrets' decks.
    built: (
        Query<&Children, With<crate::combat::turret_deck::TurretBody>>,
        Query<(&crate::combat::gun_assembly::GunAssembly, Option<&crate::combat::gun_assembly::GunMuzzle>)>,
        Query<&crate::combat::turret_deck::TurretBody>,
    ),
    // What it makes of the fight: how big the target is, whether it's
    // running, and when the salvo goes (ai_ship::awareness).
    read: (
        Res<super::awareness::Battlefield>,
        Res<super::awareness::Mind>,
        Query<&super::awareness::Awareness>,
    ),
) {
    let (turret_decks, assemblies, turret_bodies) = &built;
    let (field, mind, minds) = &read;
    // AFTERLIGHT_MOVETEST_ENEMY spawns a target dummy that's shot-free by
    // default (for testing movement/damage-model in isolation). Set
    // AFTERLIGHT_MOVETEST_ENEMY_SHOOTS=1 too to let it fire back.
    if crate::demo::skip_ai_ship_spawn()
        && std::env::var("AFTERLIGHT_MOVETEST_ENEMY_SHOOTS").ok().as_deref() != Some("1")
    {
        return;
    }

    let player = player_query.single().ok();

    for (ai_entity, ai_transform, ship_type, behavior, mut ai_target, children, ai_power, state) in ai_ships.iter_mut() {
        // A ship pulling out of a fight it has lost goes on shooting at whoever
        // it's running from.
        let covering_retreat = *behavior == AiShipBehavior::Fleeing && ai_target.entity.is_some() && !state.is_destroyed;
        if !ship_may_fire(behavior, state) && !covering_retreat {
            continue;
        }

        // Power-starved ships hold fire — same hard cutoff the player's own
        // kinetic/missile weapons already use (combat/new_projectiles.rs,
        // combat/missiles.rs). None (graph not computed yet this tick, e.g.
        // the ship just spawned) defaults to permissive so a fresh ship
        // isn't blocked before its first power tick ever runs.
        if ai_power.is_some_and(|p| p.power_balance < 0.0) {
            continue;
        }

        // Standing: a faction that isn't Hostile holds fire on you unless you
        // shot first (the brain applies the same rule when picking targets;
        // this also covers the no-target fallback below).
        let may_shoot_player = player.is_some_and(|(p, _)| {
            standing.attacks_player(*ship_type)
                || state.last_attacker == Some(p)
                // Its fleet (or a ship of its faction it answered) is
                // fighting you: it fights you too.
                || (state.alert_timer > 0.0 && state.alert_target == Some(p))
        });
        if !may_shoot_player && ai_target.entity.is_some() && ai_target.entity == player.map(|(p, _)| p) {
            continue;
        }
        let player_pos = player
            .filter(|_| may_shoot_player)
            .map(|(_, t)| t.translation.truncate());

        // Centre-of-mass of whoever the brain picked, re-read fresh every
        // frame. Falls back to the last-known snapshot (target despawned
        // mid-frame, say), then to the player, only if the live lookup fails.
        // This is the range reference and the aim fallback for factions with
        // no subsystem doctrine.
        let Some(centroid) = ai_target.entity
            .and_then(|e| target_transform_query.get(e).ok().map(|t| {
                target_shield_query.get(e).ok()
                    .map(|s| s.world_center(t))
                    .unwrap_or_else(|| t.translation.truncate())
            }))
            .or_else(|| Some(ai_target.position).filter(|_| ai_target.entity.is_some()))
            .or(player_pos)
        else { continue };

        let ai_pos = ai_transform.translation.truncate();

        // --- Subsystem aim point (faction doctrine) ---
        // Default to centre-of-mass; only override when this faction targets
        // subsystems AND a matching live module is found on the target.
        let mut aim_base = centroid;
        // A target that's running gets its drive shot out, whatever the doctrine.
        let aim_engines = minds.get(ai_entity).is_ok_and(|a| a.aim_engines);
        let priorities = if aim_engines {
            &[crate::components::ModuleCategory::Propulsion][..]
        } else {
            aim_priority(*ship_type)
        };
        if !priorities.is_empty() {
            if let Some(target_entity) = ai_target.entity {
                // Reuse the cached pick while it's still a live module.
                let cached = ai_target.subsystem
                    .and_then(|m| module_pos_query.get(m).ok())
                    .filter(|(module, _)| module.is_active && module.health > 0.0)
                    .filter(|(module, _)| priorities.iter().any(|cat| aim_matches(module.module_type, *cat)))
                    .map(|(_, gt)| gt.translation().truncate());
                if let Some(p) = cached {
                    aim_base = p;
                } else {
                    // Re-pick: walk the doctrine's category priority; within a
                    // category take the module nearest the shooter (near side
                    // first — the round has to punch through outer blocks to
                    // reach it anyway).
                    let mut chosen: Option<(Entity, Vec2)> = None;
                    if let Ok(kids) = children_query.get(target_entity) {
                        for cat in priorities {
                            let mut best: Option<(Entity, Vec2, f32)> = None;
                            for child in kids.iter() {
                                let Ok((module, gt)) = module_pos_query.get(child) else { continue };
                                if !module.is_active || module.health <= 0.0 { continue; }
                                if !aim_matches(module.module_type, *cat) { continue; }
                                let wp = gt.translation().truncate();
                                let d = wp.distance(ai_pos);
                                if best.map_or(true, |(_, _, bd)| d < bd) {
                                    best = Some((child, wp, d));
                                }
                            }
                            if let Some((e, wp, _)) = best {
                                chosen = Some((e, wp));
                                break;
                            }
                        }
                    }
                    match chosen {
                        Some((e, wp)) => { ai_target.subsystem = Some(e); aim_base = wp; }
                        None => { ai_target.subsystem = None; } // no live target module; aim centre-of-mass
                    }
                }
            }
        }

        ai_target.aim_at = ai_target.entity.map(|_| aim_base);

        // Target velocity, for lead prediction (player and AI both carry it).
        let target_vel = ai_target.entity
            .and_then(|e| velocity_query.get(e).ok())
            .map(|v| v.0)
            .unwrap_or(Vec2::ZERO);

        // Range is gated on the ship centre, so a subsystem sitting a little
        // farther out than the hull centroid doesn't push the target out of
        // "in range" on its own.
        let dist_to_target = ai_pos.distance(centroid);
        // A round only has to reach the near side of the hull, not its middle:
        // a ship holding at the edge of its reach measured to the far centre
        // of a big target, sat out of "range", and never fired.
        let target_radius = ai_target.entity.and_then(|e| field.hulls.get(&e)).map_or(0.0, |h| h.radius);
        let salvo = super::awareness::salvo_open(time.elapsed_secs(), mind);

        // The hull's own weapons, then whatever stands on its turrets.
        let guns: Vec<(Entity, Option<Entity>)> = children
            .iter()
            .map(|c| (c, None))
            .chain(
                children.iter()
                    .filter_map(|t| turret_decks.get(t).ok().map(|deck| (t, deck)))
                    .flat_map(|(t, deck)| deck.iter().map(move |g| (g, Some(t)))),
            )
            .collect();
        for (child, turret) in guns {
            let gun = assemblies.get(child).ok();
            let Ok((mut weapon, mut cooldown, module, ammo_storage, _owned, eff, weapon_gt, mount, loaded, missile, point_defence)) =
                weapon_query.get_mut(child)
            else {
                continue;
            };

            if !module.is_active || module.health <= 0.0
                || (!crate::combat::INFINITE_AMMO && weapon.ammo == 0) {
                continue;
            }

            // Unstaffed weapon stations produce nothing — same rule the
            // player's own ship runs under (compute_module_efficiency,
            // crew/mod.rs). Every weapon module is crew_station:true in the
            // registry, so this is a real gate for every AI faction, scaled
            // by crew_fill_fraction per faction (ai_ship::components).
            let efficiency = effective_efficiency(module, eff);
            if efficiency <= 0.0 {
                continue;
            }
            // A built gun with its barrels shot away has no muzzle speed.
            if gun.is_some_and(|(g, _)| g.stats.velocity_mps <= 0.0) {
                continue;
            }

            // Each gun shoots from where it actually is -- a built gun from
            // the end of its barrel...
            let muzzle = gun.and_then(|(_, m)| m.map(|m| m.world)).unwrap_or_else(|| weapon_gt.translation().truncate());

            // On point defence: the missile its turret has engaged, if any,
            // and never the ship (combat::missile_defence).
            if point_defence && module.module_type == ModuleType::FiringPiece {
                let Some(aim) = turret.and_then(|t| turret_bodies.get(t).ok()).filter(|b| b.pd_engaged).and_then(|b| b.aim) else { continue };
                let Some((g, m)) = gun else { continue };
                if m.is_some_and(|m| m.dir.dot((aim - muzzle).normalize_or_zero()) < (mount.firing_arc / 2.0).to_radians().cos()) {
                    continue;
                }
                cooldown.timer.tick(time.delta());
                if !cooldown.timer.is_finished() { continue; }
                cooldown.timer.reset();
                if !crate::combat::INFINITE_AMMO {
                    weapon.ammo = weapon.ammo.saturating_sub(1);
                }
                let round = crate::combat::new_projectiles::spawn_ai_round(
                    &mut commands, &asset_server, child, module.module_type, muzzle, aim,
                    weapon.damage * efficiency, g.stats.velocity_mps * crate::combat::gun_assembly::GAME_SPEED_PER_MPS,
                    // Stops where the engagement does, not on across the fight.
                    weapon.range.min(crate::combat::missile_defence::PD_RANGE), ammo_storage.ammo_type, loaded.map(|a| a.0), Some(g),
                );
                commands.entity(round).insert(crate::combat::point_defense::PointDefenseRound { ship: ai_entity });
                continue;
            }

            // ...and is in range from there. Measured from the ship's centre
            // this was the same thing on a small hull, but a capital or a
            // station is kilometres across, and its outer batteries sat
            // silent with the enemy well inside their reach.
            if dist_to_target.min(muzzle.distance(centroid)) - target_radius * 0.8 > weapon.range {
                continue;
            }

            // ...and only if its mount can bear. The player's guns have always
            // been held to this (is_in_firing_arc in new_projectiles); the AI
            // skipped it entirely, so a Fixed-mount railgun bolted facing aft
            // could fire dead ahead through the length of its own ship. Turrets
            // still cover 360, broadsides 180, fixed mounts 120 — so where a
            // weapon SITS on an enemy hull now decides what it can shoot at,
            // exactly as it does on yours.
            let to_target = aim_base - muzzle;
            // A built gun bears where its barrel points: a spinal gun along
            // the hull, a turret gun wherever its turret has turned.
            // A silo launches up out of the deck: it bears on anything. A
            // designed missile pops out of its tube and turns onto the target
            // itself, so no launcher of one needs to face it (the player's
            // don't either).
            let silo = module.module_type == ModuleType::VlsSilo;
            let bears = crate::combat::missile_design::is_built_launcher(module.module_type) || match gun.and_then(|(_, m)| m.copied()) {
                Some(m) => m.dir.dot(to_target.normalize_or_zero()) >= (mount.firing_arc / 2.0).to_radians().cos(),
                None => crate::combat::is_in_firing_arc(
                    ai_transform.rotation.to_euler(EulerRot::ZYX).0,
                    &module.rotation,
                    mount,
                    to_target.normalize_or_zero(),
                ),
            };
            if !bears {
                continue;
            }

            // Tick cooldown
            cooldown.timer.tick(time.delta());
            if !cooldown.timer.is_finished() {
                continue;
            }
            // Launchers wait for the salvo, loaded.
            let launcher = silo || crate::building::is_launcher(module.module_type)
                || crate::combat::missile_design::is_built_launcher(module.module_type);
            if launcher && !salvo {
                continue;
            }

            cooldown.timer.reset();
            if !crate::combat::INFINITE_AMMO {
                weapon.ammo = weapon.ammo.saturating_sub(1);
            }
            fired_events.write(WeaponFired {
                weapon_type: module.module_type,
                position: muzzle,
                from_player: false,
            });

            // Muzzle speed: the SAME per-weapon base the player's own guns
            // fire at (base_projectile_speed — Railgun 9000, Cannon 6000, …),
            // not the old flat 1800 the AI used for everything. At any real
            // range a 1800 shell took 3-5× as long to arrive as the player's
            // same weapon, so enemy fire crawled in and got walked out of —
            // most of "they don't hit me." Enemies fire at the untuned base
            // speed; the player can still tune theirs faster. (× the loaded
            // ammo's own velocity profile, same as the player.)
            let base_speed = match gun {
                Some((g, _)) => g.stats.velocity_mps * crate::combat::gun_assembly::GAME_SPEED_PER_MPS,
                None => crate::building::customization::tuning::base_projectile_speed(module.module_type),
            };
            let proj_speed = if gun.is_some() { base_speed } else { base_speed * ammo_storage.ammo_type.speed_mult() };

            // Lead the shot: aim where the subsystem/centroid WILL be by the
            // time the round gets there — turns "sprays where you were" into
            // "actually hits a moving ship."
            //
            // SHOOTER VELOCITY IS ZERO here, deliberately. calculate_lead's
            // relative-velocity math assumes the projectile inherits the
            // shooter's own velocity (the player's guns do — see
            // new_projectiles.rs). The legacy AI projectile does NOT: it flies
            // a plain straight line at a fixed speed from muzzle to aim point
            // (projectiles.rs projectile_movement). Feeding the enemy's own
            // velocity in made the aim over-compensate by shooter_vel ×
            // travel_time — a big miss for a ship strafing its standoff orbit.
            // Leading purely on the TARGET's velocity matches how the round
            // actually travels.
            let lead = crate::combat::targeting::lead_prediction::calculate_lead(
                muzzle,
                Vec2::ZERO,
                aim_base,
                target_vel,
                Vec2::ZERO,
                proj_speed,
                crate::combat::targeting::lead_prediction::PredictionTier::BasicLead,
                weapon.range,
            );
            // A built gun scatters as the player's does (new_projectiles): its
            // own spread, worse at range. Enemy rounds used to fly dead true,
            // so every battery in a fleet put every shell on one reactor from
            // 7 km the moment its guns could bear.
            let aim_point = match gun {
                Some((g, _)) => crate::combat::apply_accuracy_spread(muzzle, lead.aim_point, lead.distance_accuracy * 0.85, g.stats.spread_deg * 4.0),
                None => lead.aim_point,
            };

            // Launchers get real missiles, not a fast red bullet. Enemy
            // tubes routed through the legacy straight-line path, so none of
            // the eject, guidance, channel or collision behaviour applied to
            // anything an enemy fired.
            // A designed missile: the launcher's own design, led onto the
            // target the way the player's fire control does it.
            if let Some(spec) = crate::combat::missile_design::launcher_spec(module.module_type) {
                let design = crate::combat::missile_design::design_of(module.module_type, missile);
                let s = crate::combat::missile_design::stats(&design, spec);
                let tgo = muzzle.distance(aim_base) / (s.top_speed * 0.8).max(600.0);
                let launch_aim = aim_base + target_vel * tgo;
                let facing = Vec2::from_angle(ai_transform.rotation.to_euler(EulerRot::ZYX).0 + module.rotation.facing_angle());
                // A silo fires straight out from the ship, then turns.
                let launch_dir = if silo { crate::combat::missiles::silo_out(muzzle, ai_transform.translation.truncate(), facing) } else { facing };
                crate::combat::missiles::launch_built(&mut commands, &crate::combat::missiles::MissileLaunch {
                    weapon: child,
                    ship: ai_entity,
                    launch_cell: module.grid_position,
                    // A silo fires up out of the deck: never obstructed.
                    launch_axis: if silo { IVec2::ZERO } else { module.rotation.facing_offset() },
                    muzzle,
                    launch_dir,
                    ship_velocity: Vec2::ZERO,
                    target: ai_target.entity,
                    module_type: module.module_type,
                    damage: weapon.damage,
                    bays: 1,
                    thrust_mult: 1.0,
                }, &s, design.head, launch_aim, false);
                continue;
            }
            if crate::building::is_launcher(module.module_type) {
                let bays = 1;
                crate::combat::missiles::launch_missiles(&mut commands, &crate::combat::missiles::MissileLaunch {
                    weapon: child,
                    ship: ai_entity,
                    launch_cell: module.grid_position,
                    launch_axis: module.rotation.facing_offset(),
                    muzzle,
                    launch_dir: Vec2::from_angle(
                        ai_transform.rotation.to_euler(EulerRot::ZYX).0 + module.rotation.facing_angle(),
                    ),
                    ship_velocity: Vec2::ZERO,
                    target: ai_target.entity,
                    module_type: module.module_type,
                    damage: weapon.damage * efficiency,
                    bays,
                    thrust_mult: 1.0,
                });
                continue;
            }

            crate::combat::new_projectiles::spawn_ai_round(
                &mut commands,
                &asset_server,
                child,
                module.module_type,
                muzzle,
                aim_point,
                weapon.damage * efficiency,
                base_speed,
                weapon.range,
                ammo_storage.ammo_type,
                loaded.map(|a| a.0),
                gun.map(|(g, _)| g),
            );
            if let (Some((g, _)), true) = (gun, crate::combat::gun_assembly::trace()) {
                info!("AI GUN fired {:?} on {:?}: {:.0} mm at {:.0} u/s, {:.0} damage", child, ai_entity, g.stats.gauge_mm, proj_speed, weapon.damage);
            }
        }
    }
}

/// Does this block answer to that doctrine? "Silence the guns" has to mean a
/// built gun too: its parts are their own category, and a doctrine that only
/// looked for the old weapon blocks found none on a modern ship and went for
/// the next thing on its list. The ring is the part to hit: a turret with its
/// ring shot out is jammed for good.
fn aim_matches(module_type: ModuleType, category: crate::components::ModuleCategory) -> bool {
    use crate::components::ModuleCategory::*;
    module_type.category() == category
        || (category == Weapons && matches!(module_type, ModuleType::TurretRingSmall | ModuleType::TurretRing | ModuleType::TurretRingLarge))
}

/// Which enemy subsystems a faction tries to shoot out, in priority order. An
/// empty list means "no doctrine" — aim centre-of-mass, the historical
/// behaviour. This is what gives factions distinct combat personalities:
/// professionals defang you, gatekeepers strand you, zealots and bosses go for
/// the core, dumb swarms just hammer the middle.
fn aim_priority(faction: AiShipType) -> &'static [crate::components::ModuleCategory] {
    use crate::components::ModuleCategory::*;
    use AiShipType::*;
    match faction {
        // Elite mercs: disable your guns, then your drive — a clean takedown.
        GildedThrone => &[Weapons, Propulsion],
        // Battleship doctrine: silence the guns, then crack the reactor.
        TerranHegemony => &[Weapons, Power],
        // Deep-zone gatekeepers: kill your engines, strand you in the dark.
        CorpseStars => &[Propulsion, Weapons],
        // Bosses: methodical — guns first, then the reactor for the kill.
        EternalHegemony => &[Weapons, Power],
        Shepherd => &[Power, Weapons],
        // Zealots: fixate on the reactor, a holy execution.
        SynthesisCollective => &[Power],
        // Mindless ghosts / dumb swarm / everything else: centre of mass.
        _ => &[],
    }
}

// ============================================================================
// DISTRESS CALLS — reinforcement aggro WITHOUT spawning ships from nowhere
// (spawn_raider_waves was cut for exactly that reason — see ai_ship/mod.rs).
// A ship in a real fight broadcasts; nearby SAME-FACTION fighters already in
// the world converge on and engage whoever the caller is fighting.
// ============================================================================

/// How far a distress call reaches. Tuned to a patrol/nest cluster, not the
/// whole map — territories sit tens of thousands of units apart, so a call
/// only ever wakes a faction's own neighbours, never a distant faction.
const DISTRESS_RADIUS: f32 = 13_500.0;
/// How long a summoned ship stays aggro'd on the caller's target before it
/// drifts back to its own patrol.
const ALERT_DURATION: f32 = 25.0;
/// Minimum gap between one ship's distress broadcasts.
const DISTRESS_COOLDOWN: f32 = 12.0;

/// Ships mid-fight call for backup; nearby same-faction fighters answer.
///
/// Two passes because both the caller (to stamp its re-broadcast cooldown)
/// and every responder (to receive the alert) need `&mut AiShipState`, and a
/// single mutable iteration can't cross-reference other rows. Pass 1 reads out
/// the fresh broadcasts; pass 2 ticks every ship's timers and applies them.
/// The brain then acts on `alert_target`/`alert_timer` (see its distress-
/// response arm). Bevy serialises this against the brain/damage systems
/// automatically (all want `&mut AiShipState`), so there's no ordering to fix.
pub fn ai_distress_system(
    time: Res<Time>,
    mut ai_ships: Query<
        (
            Entity,
            &Transform,
            &AiShipType,
            &AiShipBehavior,
            &AiShipTarget,
            &mut AiShipState,
            Option<&super::fleet::FleetMember>,
            Has<super::fleet::CivilianHull>,
        ),
        With<AiShip>,
    >,
    escorts: Query<(), With<super::standing_spawns::PlayerEscort>>,
    player_query: Query<Entity, With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let dt = time.delta_secs();
    let player = player_query.single().ok();

    // A fleet fights as one: when any member is fighting, or is being shot
    // at, the rest of its fighting ships are told who, wherever they are in
    // the formation. No cooldown and no "in trouble" bar -- an escort doesn't
    // wait for its capital to be hurt. (fleet, target)
    let fleet_calls = fleet_calls(
        ai_ships.iter().map(|(e, _, _, behavior, target, state, member, _)| FleetVoice {
            fleet: member.map(|m| m.fleet),
            behavior: *behavior,
            target: target.entity,
            last_attacker: state.last_attacker,
            last_hit_timer: state.last_hit_timer,
            destroyed: state.is_destroyed,
            escort: escorts.contains(e),
        }),
        player,
    );

    // PASS 1 — collect fresh broadcasts: a fighting ship, off cooldown, that's
    // actually in trouble (recently hit or hull hurt) and has a target.
    let mut broadcasts: Vec<(Entity, AiShipType, Vec2, Entity)> = Vec::new();
    for (e, tf, ftype, behavior, target, state, _, civilian) in ai_ships.iter() {
        if state.is_destroyed { continue; }
        // A civilian hull doesn't fight, but it does call for help when
        // it's hit: its escort, its faction's ships, and a trader's ally.
        if civilian && state.distress_cooldown <= 0.0 && state.last_hit_timer < 5.0 {
            if let Some(attacker) = state.last_attacker {
                broadcasts.push((e, *ftype, tf.translation.truncate(), attacker));
                continue;
            }
        }
        if *behavior != AiShipBehavior::Engaging { continue; }
        if state.distress_cooldown > 0.0 { continue; }
        if !faction_fights(*ftype) { continue; }
        let Some(tgt) = target.entity else { continue };
        let in_trouble = state.last_hit_timer < 5.0 || state.hull_integrity < 0.9;
        if !in_trouble { continue; }
        broadcasts.push((e, *ftype, tf.translation.truncate(), tgt));
    }

    // PASS 2 — tick every ship's timers, stamp caller cooldowns, and alert
    // nearby same-faction fighters onto the caller's target.
    let mut player_backup_called = false;
    for (e, tf, ftype, _behavior, _target, mut state, member, civilian) in ai_ships.iter_mut() {
        state.alert_timer = (state.alert_timer - dt).max(0.0);
        state.distress_cooldown = (state.distress_cooldown - dt).max(0.0);
        if state.is_destroyed { continue; }

        if broadcasts.iter().any(|(be, _, _, _)| *be == e) {
            state.distress_cooldown = DISTRESS_COOLDOWN;
        }

        if !faction_fights(*ftype) || civilian { continue; }
        // A fleet's call reaches all of it, before any stranger's.
        if let Some(&(_, t)) = member.and_then(|m| fleet_calls.iter().find(|(f, _)| *f == m.fleet)) {
            if t != e {
                state.alert_target = Some(t);
                state.alert_timer = state.alert_timer.max(ALERT_DURATION);
                continue;
            }
        }
        let pos = tf.translation.truncate();
        // Whether this ship was already answering a call before this frame —
        // used so the "reinforcements!" warning fires once per NEW recruit,
        // not every time an already-committed ship's alert is refreshed.
        let was_alerted = state.alert_timer > 0.0;

        // Answer the nearest same-faction call within earshot (never one's own).
        let mut nearest: Option<(f32, Entity)> = None;
        for (be, bfaction, bpos, btgt) in &broadcasts {
            if *be == e || !ftype.defends(*bfaction) { continue; }
            let d = pos.distance(*bpos);
            // A trader's ally answers from twice as far: it's what the
            // trader pays it for.
            let reach = if *bfaction != *ftype { DISTRESS_RADIUS * 2.0 } else { DISTRESS_RADIUS };
            if d > reach { continue; }
            if nearest.map_or(true, |(nd, _)| d < nd) {
                nearest = Some((d, *btgt));
            }
        }
        if let Some((_, btgt)) = nearest {
            state.alert_target = Some(btgt);
            state.alert_timer = state.alert_timer.max(ALERT_DURATION);
            if !was_alerted && Some(btgt) == player {
                player_backup_called = true;
            }
        }
    }

    if player_backup_called {
        notifications.write(ShowNotification {
            message: "Enemy is calling in reinforcements!".into(),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    }
}

/// Who each fleet turns on: (fleet, target).
///
/// A member calls whoever it is fighting, or whoever just hit it. Two cases
/// used to go unanswered, and a squadron watched you pick off one of its own:
///
/// - Only the first member's call was heard. One already busy with some
///   other ship drowned out the one you were shooting. A call against you
///   now outranks any other.
/// - A member you killed outright called nobody: the dead were skipped. A
///   wreck now calls for as long as its last hit is fresh.
///
/// Your own escort wing is the exception: a stray round from you into one of
/// them doesn't turn the rest of it on you.
struct FleetVoice {
    fleet: Option<u32>,
    behavior: AiShipBehavior,
    target: Option<Entity>,
    last_attacker: Option<Entity>,
    last_hit_timer: f32,
    destroyed: bool,
    escort: bool,
}

fn fleet_calls(members: impl Iterator<Item = FleetVoice>, player: Option<Entity>) -> Vec<(u32, Entity)> {
    let mut calls: Vec<(u32, Entity)> = Vec::new();
    for FleetVoice { fleet, behavior, target, last_attacker, last_hit_timer, destroyed, escort } in members {
        let Some(fleet) = fleet else { continue };
        let hit_by = last_attacker.filter(|_| last_hit_timer < 5.0);
        let fighting = target.filter(|_| behavior == AiShipBehavior::Engaging && !destroyed);
        let call = if hit_by.is_some() && hit_by == player && !escort {
            hit_by
        } else if destroyed {
            None
        } else {
            fighting.or(hit_by)
        };
        let Some(t) = call else { continue };
        match calls.iter_mut().find(|(f, _)| *f == fleet) {
            None => calls.push((fleet, t)),
            Some(existing) if Some(t) == player && !escort => existing.1 = t,
            Some(_) => {}
        }
    }
    calls
}

/// Whether a hit from `attacker` names it as the ship's attacker: never the
/// ship itself or a friend, always the player (who has no faction here) or
/// anything else that isn't an AI ship.
fn counts_as_attacker(target: Entity, target_faction: Option<AiShipType>, attacker: Entity, attacker_faction: Option<AiShipType>) -> bool {
    attacker != target
        && match (target_faction, attacker_faction) {
            (Some(own), Some(theirs)) => factions_hostile(own, theirs),
            _ => true,
        }
}

/// Process damage to AI ships — per-module penetration
pub fn process_ai_ship_damage_system(
    mut damage_events: MessageReader<AiShipDamaged>,
    mut ai_ships: Query<(&mut AiShipState, &Children, Option<&AiLoadout>), With<AiShip>>,
    // GlobalTransform, not Transform: a block's Transform is ship-LOCAL and
    // `event.position` is world space, so "nearest to the impact" was
    // measured between two different frames and landed on blocks nowhere
    // near the shot (and the damage numbers floated up near the origin).
    mut hull_query: Query<(&mut HullSegment, &GlobalTransform, &OwnedByAiShip), Without<Module>>,
    mut module_query: Query<(&mut Module, &GlobalTransform, &OwnedByAiShip), Without<HullSegment>>,
    mut destroyed_events: MessageWriter<AiShipDestroyed>,
    ai_ship_query: Query<(&Transform, &AiShipType, Option<&BountyTarget>), With<AiShip>>,
    mut commands: Commands,
) {
    for event in damage_events.read() {
        let Ok((mut state, children, loadout)) = ai_ships.get_mut(event.target) else {
            continue;
        };

        state.last_hit_timer = 0.0;
        // Preserve the last known attributable attacker across non-
        // attributable damage ticks (fire DoT, self-detonation) rather
        // than clearing it — a ship mid-burn from an earlier shot should
        // still remember who fired it.
        //
        // Only an enemy counts. A fleet-mate's stray round, a blast that
        // caught its own side, or two hulls in formation scraping each
        // other named a friend as the attacker, and the fleet call that
        // follows turned the whole fleet on it -- the accused ship even
        // answered its neighbours' call and targeted itself. A guard fleet
        // at a starport tore itself apart at full speed, through whatever
        // was parked in the middle.
        let faction = |e: Entity| ai_ship_query.get(e).ok().map(|(_, &f, _)| f);
        if let Some(attacker) = event.attacker {
            if counts_as_attacker(event.target, faction(event.target), attacker, faction(attacker)) {
                state.last_attacker = Some(attacker);
            }
        }

        let impact_pos = event.position.unwrap_or(Vec2::ZERO);
        let mut remaining_damage = event.amount;

        // Collect child hull segments sorted by distance from impact
        let mut hull_hits: Vec<(Entity, f32)> = Vec::new();
        for child in children.iter() {
            if let Ok((_, hull_transform, owned)) = hull_query.get(child) {
                if owned.root == event.target {
                    let dist = hull_transform.translation().truncate().distance(impact_pos);
                    hull_hits.push((child, dist));
                }
            }
        }
        hull_hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply damage to nearest hull segments
        for (hull_entity, _dist) in &hull_hits {
            if remaining_damage <= 0.0 {
                break;
            }
            if let Ok((mut hull, hull_transform, _)) = hull_query.get_mut(*hull_entity) {
                let damage_to_apply = remaining_damage.min(hull.health);
                hull.health -= damage_to_apply;
                remaining_damage -= damage_to_apply;

                spawn_floating_damage(
                    &mut commands,
                    hull_transform.translation().truncate(),
                    damage_to_apply,
                    Color::srgb(1.0, 0.3, 0.3),
                );
                spawn_hit_effect(
                    &mut commands,
                    hull_transform.translation().truncate(),
                    Color::srgb(1.0, 0.5, 0.2),
                    16.0,
                );
            }
        }

        // If damage penetrates hull, hit nearest modules
        if remaining_damage > 0.0 {
            let mut module_hits: Vec<(Entity, f32)> = Vec::new();
            for child in children.iter() {
                if let Ok((_, mod_transform, owned)) = module_query.get(child) {
                    if owned.root == event.target {
                        let dist = mod_transform.translation().truncate().distance(impact_pos);
                        module_hits.push((child, dist));
                    }
                }
            }
            module_hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            for (mod_entity, _dist) in &module_hits {
                if remaining_damage <= 0.0 {
                    break;
                }
                if let Ok((mut module, mod_transform, _)) = module_query.get_mut(*mod_entity) {
                    let damage_to_apply = remaining_damage.min(module.health);
                    module.health -= damage_to_apply;
                    remaining_damage -= damage_to_apply;

                    if module.health <= 0.0 {
                        module.is_active = false;
                    }

                    spawn_floating_damage(
                        &mut commands,
                        mod_transform.translation().truncate(),
                        damage_to_apply,
                        Color::srgb(1.0, 0.6, 0.2),
                    );
                }
            }
        }

        // Recalculate hull integrity -- against the hull the ship was built
        // with. Summing max_health over the survivors made integrity climb
        // back up as dead plates were despawned, so a ship stripped to its
        // last few tiles read as barely scratched and Gutted could not fire.
        let mut total_hull_hp = 0.0_f32;
        let mut max_hull_hp = 0.0_f32;
        for child in children.iter() {
            if let Ok((hull, _, owned)) = hull_query.get(child) {
                if owned.root == event.target {
                    total_hull_hp += hull.health;
                    max_hull_hp += hull.max_health;
                }
            }
        }
        if let Some(loadout) = loadout {
            max_hull_hp = max_hull_hp.max(loadout.hull_max);
        }
        state.hull_integrity = if max_hull_hp > 0.0 {
            total_hull_hp / max_hull_hp
        } else {
            0.0
        };

        // Check destruction
        if state.hull_integrity <= 0.0 && !state.is_destroyed {
            state.is_destroyed = true;
            if let Ok((ai_transform, ai_ship_type, bounty)) = ai_ship_query.get(event.target) {
                destroyed_events.write(AiShipDestroyed {
                    entity: event.target,
                    ship_type: *ai_ship_type,
                    position: ai_transform.translation.truncate(),
                    bounty_id: bounty.map(|b| b.0),
                    cause: ShipDeathCause::Gutted,
                });
            }
        }
    }
}

/// Fraction of a subsystem class still alive below which it counts as shot
/// out. Not zero: the last gun on a battleship shouldn't keep a 160-tile
/// hulk "in the fight" for another three minutes of grinding.
const CRIPPLE_THRESHOLD: f32 = 0.25;

/// Seconds between a reactor breach and detonation. The ship is at its most
/// dangerous here — see tick_reactor_meltdown.
pub const MELTDOWN_SECONDS: f32 = 8.0;

/// Counts a class of modules on one AI ship as (alive, total). The total is
/// only what still EXISTS: a destroyed module is despawned half a second
/// after it dies (`ship::damage::tick_pending_removal`). Ratios divide by
/// the spawn-time count instead -- see `surviving_fraction`.
pub(crate) fn subsystem_tally(
    root: Entity,
    children: &Children,
    module_query: &Query<(&Module, &OwnedByAiShip)>,
    matches_class: impl Fn(&Module) -> bool,
) -> (u32, u32) {
    let mut alive = 0;
    let mut total = 0;
    for child in children.iter() {
        let Ok((module, owned)) = module_query.get(child) else { continue };
        if owned.root != root || !matches_class(module) { continue; }
        total += 1;
        if module.health > 0.0 {
            alive += 1;
        }
    }
    (alive, total)
}

/// Share of a subsystem class still alive, out of what the ship was BUILT
/// with (`AiLoadout`), not out of what is left of it. Dividing by the live
/// total read a ship with one gun left of four as fully armed, because the
/// three dead ones had already been despawned. A ship spawned without a
/// loadout falls back to the live total; a class it never had reads 0.
pub(crate) fn surviving_fraction(alive: u32, total_now: u32, original: Option<u32>) -> f32 {
    let total = original.map_or(total_now, |o| o.max(total_now));
    if total == 0 {
        0.0
    } else {
        alive as f32 / total as f32
    }
}

/// A gun: the old fixed weapons, and a gun built from parts (counted once,
/// by its firing piece).
pub(crate) fn is_weapon_type(module_type: ModuleType) -> bool {
    ModuleCategory::Weapons.module_types().contains(&module_type) || module_type == ModuleType::FiringPiece
}
/// Where a ship is commanded from: lose them all and nobody is fighting it.
pub(crate) fn is_command_type(module_type: ModuleType) -> bool {
    matches!(module_type, ModuleType::HelmStation | ModuleType::BridgeWing | ModuleType::AICombatCore)
}

pub(crate) fn is_engine_type(module_type: ModuleType) -> bool {
    ModuleCategory::Propulsion.module_types().contains(&module_type)
}

pub(crate) fn is_weapon_module(module: &Module) -> bool {
    is_weapon_type(module.module_type)
}

fn is_engine_module(module: &Module) -> bool {
    is_engine_type(module.module_type)
}

/// A ship is DEFEATED when it can no longer fight, not when its last hull
/// tile is ground off. Shoot out the guns and the engines and the crew
/// strikes colors: the ship stops fighting and drifts as an intact derelict.
///
/// This is the anti-grind valve. An Terran Hegemony is 160 tiles x 500 HP — eighty
/// thousand hull HP, minutes of held fire — so before this the only kill
/// anyone ever went for was sniping the reactor, and everything in between
/// was a slog. Now a fight ends when you've taken the ship apart in the
/// places that matter, and the wreck you're left with is the best salvage in
/// the game (wreck.rs scores loot by how intact the hull still is).
pub fn check_ai_cripple(
    mut ai_ships: Query<
        (Entity, &Transform, &AiShipType, &Children, &mut AiShipState, Option<&BountyTarget>, Option<&AiLoadout>, Has<crate::diplomacy::FactionStation>, Option<&super::air::AiCrewComplement>),
        (With<AiShip>, Without<ReactorMeltdown>),
    >,
    module_query: Query<(&Module, &OwnedByAiShip)>,
    crew_query: Query<&crate::components::CrewMember, With<OwnedByAiShip>>,
    mut destroyed_events: MessageWriter<AiShipDestroyed>,
    mut notifications: MessageWriter<ShowNotification>,
    decks: Query<&Children, With<crate::combat::turret_deck::TurretBody>>,
) {
    for (entity, transform, ship_type, children, mut state, bounty, loadout, station, complement) in ai_ships.iter_mut() {
        if state.is_destroyed { continue; }
        // Nothing has touched it. A big hull spawns over several frames, and
        // one still arriving has no guns or engines yet: it was read as a
        // derelict and struck its colours with its whole crew aboard.
        if state.last_hit_timer >= 999.0 { continue; }
        // A station has no engines by design, and an unarmed one (the
        // Silence's Monastery) no guns: neither has anything to strike.
        if station && loadout.is_none_or(|l| l.guns == 0) { continue; }

        // Its guns stand on its turrets as well as its deck. Counting the deck
        // alone found none on a modern ship, so it was "toothless" from the
        // moment it arrived, and losing its engines finished it.
        let (guns_alive, guns_total) = children.iter().filter_map(|c| decks.get(c).ok()).fold(
            subsystem_tally(entity, children, &module_query, is_weapon_module),
            |(alive, total), deck| {
                let (a, t) = subsystem_tally(entity, deck, &module_query, is_weapon_module);
                (alive + a, total + t)
            },
        );
        let (engines_alive, engines_total) = subsystem_tally(entity, children, &module_query, is_engine_module);

        // Against the spawn-time loadout, not the survivors: dead modules are
        // despawned, so the live total shrank with every loss and the ratio
        // never fell. An unarmed hull (The Silence) is toothless by
        // construction and an engineless one already adrift -- a class it
        // never had reads 0, never "still fine".
        let toothless = surviving_fraction(guns_alive, guns_total, loadout.map(|l| l.guns))
            <= CRIPPLE_THRESHOLD;
        let immobile = surviving_fraction(engines_alive, engines_total, loadout.map(|l| l.engines))
            <= CRIPPLE_THRESHOLD;
        // Every bridge, helm and AI core it was built with gone: nobody is
        // left to fight it. A ship built with none can't lose them.
        let (command_alive, _) = subsystem_tally(entity, children, &module_query, |m| is_command_type(m.module_type));
        let headless = loadout.is_some_and(|l| l.command > 0) && command_alive == 0;
        // Its reactors down to a quarter of what they made: the guns,
        // shields and drives it has left have nothing to run on. (Every
        // reactor gone is a meltdown instead -- check_ai_reactor_destruction.)
        let power_left: f32 = children
            .iter()
            .filter_map(|c| module_query.get(c).ok())
            .filter(|(m, owned)| owned.root == entity && m.health > 0.0)
            .map(|(m, _)| m.power_generation)
            .sum();
        let powerless = loadout.is_some_and(|l| l.power_mw > 0.0 && power_left <= l.power_mw * CRIPPLE_THRESHOLD);
        // Most of its crew dead -- suffocated, vented, burned: nobody left
        // to work the ship.
        let crew_alive = children.iter().filter_map(|c| crew_query.get(c).ok()).filter(|c| c.health > 0.0).count() as u32;
        let crewless = complement.is_some_and(|c| c.0 > 0 && crew_alive as f32 <= c.0 as f32 * CRIPPLE_THRESHOLD);
        // A class it was never built with is nothing it lost: an Oracle has no
        // guns and no engines to begin with, and read as a derelict from birth.
        let built_to_fight_or_fly = loadout.is_none_or(|l| l.guns > 0 || l.engines > 0);
        let why = if headless {
            "its bridge is gone"
        } else if powerless {
            "its reactors are failing"
        } else if crewless {
            "its crew are dead"
        } else if toothless && immobile && built_to_fight_or_fly {
            "derelict adrift"
        } else {
            continue;
        };

        state.is_destroyed = true;
        let pos = transform.translation.truncate();
        info!(
            "{:?} strikes: {why} (command {command_alive}, power {power_left:.0} MW of {:.0}, crew {crew_alive} of {}, guns {guns_alive}, engines {engines_alive})",
            ship_type, loadout.map_or(0.0, |l| l.power_mw), complement.map_or(0, |c| c.0)
        );
        notifications.write(ShowNotification {
            message: format!("{} strikes colors - {why}, ripe for salvage.", faction_display_name(*ship_type)),
            notification_type: NotificationType::Success,
            duration: 4.0,
        });
        destroyed_events.write(AiShipDestroyed {
            entity,
            ship_type: *ship_type,
            position: pos,
            bounty_id: bounty.map(|b| b.0),
            cause: ShipDeathCause::Struck,
        });
    }
}

/// A breached reactor no longer kills the ship on the spot. Popping the core
/// used to be an instant win, which made every fight a race to dig one hole
/// — now it starts a countdown, and the dying ship spends it berserk (see
/// tick_reactor_meltdown). Ships with a second live reactor just lose that
/// one: redundancy is why a Eternal Hegemony takes longer than a raider, instead
/// of raw hit points.
pub fn check_ai_reactor_destruction(
    mut commands: Commands,
    destroyed_reactors: Query<(&Module, &OwnedByAiShip), Added<DestroyedModule>>,
    ai_ships: Query<(&AiShipState, &Children, Option<&ReactorMeltdown>)>,
    module_query: Query<(&Module, &OwnedByAiShip)>,
    mut shield_query: Query<&mut crate::combat::shields::ShipShield>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for (module, owned) in destroyed_reactors.iter() {
        if !is_reactor(module.module_type) { continue; }

        let Ok((state, children, melting)) = ai_ships.get(owned.root) else { continue };
        if state.is_destroyed || melting.is_some() { continue; }

        // Any other reactor still running? Then the ship just browns out a
        // little and fights on.
        let (reactors_alive, _) = subsystem_tally(
            owned.root, children, &module_query, |m| is_reactor(m.module_type),
        );
        if reactors_alive > 0 { continue; }

        // Containment is gone: the bubble drops now, the ship goes later.
        if let Ok(mut shield) = shield_query.get_mut(owned.root) {
            shield.enabled = false;
            shield.current = 0.0;
        }
        commands.entity(owned.root).try_insert(ReactorMeltdown { remaining: MELTDOWN_SECONDS });
        notifications.write(ShowNotification {
            message: format!("REACTOR BREACH - detonation in {:.0}s. Get clear.", MELTDOWN_SECONDS),
            notification_type: NotificationType::Danger,
            duration: 4.0,
        });
    }
}

fn is_reactor(module_type: ModuleType) -> bool {
    matches!(module_type,
        ModuleType::SmallReactor | ModuleType::StandardReactor
        | ModuleType::LargeReactor | ModuleType::FusionReactor
    )
}

/// Runs the breach countdown. The ship keeps its shield down and burns what's
/// left in the capacitors: it fights to the last second rather than sitting
/// there waiting to be a kill notification, so the payoff for cracking a core
/// is a scramble to get clear, not a free win.
pub fn tick_reactor_meltdown(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    time: Res<Time>,
    mut ai_ships: Query<(
        Entity,
        &Transform,
        &AiShipType,
        &mut AiShipState,
        &mut ReactorMeltdown,
        Option<&BountyTarget>,
    )>,
    mut shield_query: Query<&mut crate::combat::shields::ShipShield>,
    mut destroyed_events: MessageWriter<AiShipDestroyed>,
) {
    let dt = time.delta_secs();
    for (entity, transform, ship_type, mut state, mut meltdown, bounty) in ai_ships.iter_mut() {
        if state.is_destroyed {
            commands.entity(entity).remove::<ReactorMeltdown>();
            continue;
        }

        // Hold the bubble down for the whole countdown — update_shields would
        // otherwise recharge it after a few quiet seconds.
        if let Ok(mut shield) = shield_query.get_mut(entity) {
            shield.enabled = false;
            shield.current = 0.0;
        }

        let was = meltdown.remaining;
        meltdown.remaining -= dt;
        let pos = transform.translation.truncate();

        // One flare per remaining second, brightening as it goes.
        if was.ceil() != meltdown.remaining.ceil() && meltdown.remaining > 0.0 {
            let heat = 1.0 - (meltdown.remaining / MELTDOWN_SECONDS);
            // The whole job of this phase is to say "get away from that ship".
            // A flat square once a second said it very quietly; a real blast
            // that grows and reddens each time reads as a countdown.
            crate::combat::spawn_explosion(
                &mut commands,
                &fx,
                pos,
                14.0 + 26.0 * heat,
                Color::srgb(1.0, 0.6 - 0.4 * heat, 0.1),
            );
        }

        if meltdown.remaining > 0.0 { continue; }

        state.is_destroyed = true;
        state.hull_integrity = 0.0;
        commands.entity(entity).remove::<ReactorMeltdown>();
        // Was two stacked opaque squares, 260 and 180 units, for 200ms. This
        // is the loudest thing that happens in a fight and it deserves the
        // composite: the fireball reaches ~250 units on its own.
        crate::combat::spawn_explosion(
            &mut commands,
            &fx,
            pos,
            115.0,
            Color::srgb(1.0, 0.6, 0.15),
        );
        destroyed_events.write(AiShipDestroyed {
            entity,
            ship_type: *ship_type,
            position: pos,
            bounty_id: bounty.map(|b| b.0),
            cause: ShipDeathCause::Meltdown,
        });
    }
}

/// Blast radius of a cooking-off magazine or fuel bunker, in world units
/// (blocks are 66 across, so this reaches the ring of blocks around it).
const COOKOFF_RADIUS: f32 = 100.0;

/// AMMO COOK-OFF / FUEL FIRE on enemy ships. Player ships have had this since
/// chain_reactions.rs, but AI ships never did — so the one shot that should
/// end a fight outright, straight into the magazine, did exactly as much as a
/// shot into a corridor. Now a destroyed ammo bay or fuel tank takes its
/// neighbours with it, which is what makes a lucky (or aimed) hit on a
/// magazine the fastest kill in the game.
pub fn ai_chain_reactions(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,

    // ParamSet because the "what just blew up" query and the "damage the
    // neighbours" query both touch Module — one shared, one mutable.
    mut modules: ParamSet<(
        Query<(&Module, &GlobalTransform, &OwnedByAiShip, Has<Explosive>), Added<DestroyedModule>>,
        Query<(&mut Module, &GlobalTransform, &OwnedByAiShip), Without<HullSegment>>,
    )>,
    mut hull_query: Query<(&mut HullSegment, &GlobalTransform, &OwnedByAiShip), Without<Module>>,
    mut damage_events: MessageWriter<AiShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let blasts: Vec<(Entity, Vec2, f32, bool)> = modules.p0().iter()
        .filter_map(|(module, gt, owned, fused)| {
            let blast = match module.module_type {
                ModuleType::WarheadBay => 90.0,
                ModuleType::AmmoFeedUnit | ModuleType::AmmoBay => 55.0,
                ModuleType::FuelTank | ModuleType::FuelProcessor => 40.0,
                _ => return None,
            };
            Some((owned.root, gt.translation().truncate(), blast, fused))
        })
        .collect();

    for (root, pos, blast, fused) in blasts {
        notifications.write(ShowNotification {
            message: "Magazine cook-off aboard the target!".into(),
            notification_type: NotificationType::Success,
            duration: 2.5,
        });
        // A block that carries `Explosive` (ammo bays and fuel tanks do) is
        // already blowing up through ship::damage::queue_ai_detonation, on
        // its own fuse with its own fires. Applying this blast as well
        // cooked every such magazine off twice.
        if fused { continue; }

        for (mut hull, hull_gt, hull_owned) in hull_query.iter_mut() {
            if hull_owned.root != root { continue; }
            if hull_gt.translation().truncate().distance(pos) > COOKOFF_RADIUS { continue; }
            hull.health = (hull.health - blast).max(0.0);
        }
        for (mut neighbour, mod_gt, mod_owned) in modules.p1().iter_mut() {
            if mod_owned.root != root { continue; }
            if neighbour.health <= 0.0 { continue; }
            if mod_gt.translation().truncate().distance(pos) > COOKOFF_RADIUS { continue; }
            neighbour.health = (neighbour.health - blast).max(0.0);
        }

        crate::combat::spawn_explosion(&mut commands, &fx, pos, 60.0, Color::srgb(1.0, 0.7, 0.2));

        // Bookkeeping only — the damage above is already applied; this makes
        // the ship recompute its hull integrity (and remember it was hit).
        damage_events.write(AiShipDamaged {
            target: root,
            source: DamageSource::Explosion,
            amount: 0.0,
            position: Some(pos),
            direction: None,
            attacker: None,
        });
    }
}

/// Seconds an AI ship must go without anything to shoot at before its
/// magazines are full again.
pub const AI_RESTOCK_QUIET: f32 = 30.0;

/// AI ships refill their magazines between fights.
///
/// They never dock, so with ammo actually burning a ship that emptied its
/// guns in one fight would sit silent for the rest of the run, and drawing
/// a patrol's fire until it went dry would be the safe way to beat every
/// ship in a system. Within a fight a magazine is still a magazine -- outlast
/// it and the guns stop -- but half a minute with nothing in range and the
/// ship has restocked from its own holds.
pub fn ai_restock_between_fights(
    time: Res<Time>,
    ships: Query<(Entity, &GlobalTransform, Option<&AiShipTarget>, &Children), (With<AiShip>, Without<AiShipWreck>)>,
    mut weapons: Query<&mut Weapon, Without<DestroyedModule>>,
    mut quiet: Local<bevy::platform::collections::HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    let mut alive = bevy::platform::collections::HashSet::new();
    for (ship, gt, target, children) in &ships {
        alive.insert(ship);
        let reach = children
            .iter()
            .filter_map(|c| weapons.get(c).ok())
            .map(|w| w.range)
            .fold(0.0f32, f32::max);
        let engaged = target.is_some_and(|t| {
            t.entity.is_some() && t.position.distance(gt.translation().truncate()) <= reach * 1.2
        });
        let timer = quiet.entry(ship).or_insert(0.0);
        if engaged {
            *timer = 0.0;
            continue;
        }
        *timer += dt;
        if *timer < AI_RESTOCK_QUIET {
            continue;
        }
        for child in children.iter() {
            if let Ok(mut weapon) = weapons.get_mut(child) {
                if weapon.ammo < weapon.max_ammo {
                    weapon.ammo = weapon.max_ammo;
                }
            }
        }
    }
    quiet.retain(|ship, _| alive.contains(ship));
}

/// Crews that keep watching their rounds skip off you eventually stop firing
/// the round that skips.
///
/// This is the counterplay to sloped armour. Without it, a player who learns
/// to angle becomes steadily harder to hurt with no answer from the other
/// side — fights get EASIER the better you understand the system, which is
/// backwards. An enemy that changes what it loads is also the most legible
/// form of AI thinking available here: you see the tracers change colour and
/// then start biting.
///
/// The replacement is picked to defeat geometry rather than to hit harder.
/// HESH spalls through armour it never breaches; the exotics ignore impact
/// angle outright. Which one a faction reaches for is a characterisation:
/// scrappers improvise a squash head, deep-zone lords have something worse.
pub fn ai_adapt_ammo(
    // Entity comes from the SAME query. Zipping a separate Query<Entity> against
    // this one would assume both iterate in the same archetype order, which
    // Bevy does not promise — and getting it wrong silently attributes one
    // ship's gunnery record to another.
    mut ships: Query<(Entity, &AiShipType, &AiShipTarget, &Children, &mut AiGunneryLog)>,
    mut weapons: Query<&mut crate::building::customization::tuning::SelectedAmmo, With<Weapon>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut last_target: Local<std::collections::HashMap<Entity, Option<Entity>>>,
) {
    for (entity, faction, target, children, mut log) in ships.iter_mut() {
        // Studying a new ship starts the lesson over — what worked against the
        // last one says nothing about this one's armour.
        let previous = last_target.entry(entity).or_insert(None);
        if *previous != target.entity {
            *previous = target.entity;
            log.ricochets = 0;
            log.switched = false;
        }

        if log.switched || log.ricochets < RICOCHETS_BEFORE_SWITCH {
            continue;
        }
        log.switched = true;

        let answer = angle_proof_round(*faction);
        let mut changed = 0;
        for child in children.iter() {
            if let Ok(mut loaded) = weapons.get_mut(child) {
                loaded.0 = answer;
                changed += 1;
            }
        }
        if changed > 0 {
            notifications.write(ShowNotification {
                message: format!("Enemy switching ammunition - {}", answer.name()),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
        }
    }
}

/// What a faction reaches for when angles are beating it. Characterisation as
/// much as balance: a scrap crew improvises, a deep-zone lord doesn't have to.
fn angle_proof_round(faction: AiShipType) -> KineticAmmoType {
    use AiShipType::*;
    match faction {
        // Nothing exotic aboard — but a squash head doesn't need to get
        // through, and that's the whole trick.
        RecursiveKingdom | BrokenChoir | StellarPreserve => KineticAmmoType::HESH,
        // Disciplined gunnery: a dart barely deflects at any angle.
        GildedThrone | TerranHegemony => KineticAmmoType::APFSDS,
        // Bio-organic and deep-zone: they have stranger things loaded.
        SynthesisCollective | CorpseStars | TheSilence => KineticAmmoType::HESH,
        // Gravity does not care what angle you hit at.
        EternalHegemony | Shepherd => KineticAmmoType::Singularity,
        // Freight: whatever the hold had in it.
        MerchantPrinces | FreeHaulersCompact => KineticAmmoType::HESH,
    }
}

#[cfg(test)]
mod cripple_tests {
    use super::*;

    fn module(module_type: ModuleType, health: f32) -> Module {
        Module {
            module_type,
            health,
            max_health: 100.0,
            power_consumption: 0.0,
            power_generation: 0.0,
            is_active: health > 0.0,
            grid_position: IVec2::ZERO,
            size: IVec2::ONE,
            rotation: Rotation::North,
        }
    }

    /// The ratio is out of what was built, not out of what is left.
    #[test]
    fn losses_count_against_the_original_loadout() {
        // One gun left of four; the three dead ones are already despawned,
        // so the live tally says 1 of 1.
        assert_eq!(surviving_fraction(1, 1, Some(4)), 0.25);
        // No loadout recorded: the live total is all there is.
        assert_eq!(surviving_fraction(1, 2, None), 0.5);
        // A class the ship never had, or has lost entirely, reads empty.
        assert_eq!(surviving_fraction(0, 0, Some(0)), 0.0);
        assert_eq!(surviving_fraction(0, 0, None), 0.0);
    }

    /// The real system, the way the game leaves it mid-fight: three of four
    /// guns and both engines shot away and despawned, one gun still alive.
    /// That ship is beaten and must strike. Before the loadout it read
    /// "1 of 1 guns" and fought on forever.
    #[test]
    fn a_ship_whose_dead_modules_were_despawned_still_strikes() {
        let mut app = App::new();
        app.add_message::<AiShipDestroyed>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_ai_cripple);

        let ship = app.world_mut().spawn((
            AiShip,
            Transform::default(),
            AiShipType::RecursiveKingdom,
            AiShipState { last_hit_timer: 0.0, ..default() },
            AiLoadout { guns: 4, engines: 2, hull_max: 1000.0, ..default() },
        )).id();
        app.world_mut().spawn((
            module(ModuleType::Gatling, 100.0),
            OwnedByAiShip { root: ship },
            ChildOf(ship),
        ));

        app.update();
        assert!(
            app.world().get::<AiShipState>(ship).unwrap().is_destroyed,
            "one gun of four and no engines left, and the ship did not strike"
        );
    }

    /// A big hull arriving over several frames: built with guns and engines,
    /// none of them spawned yet, and nothing has touched it.
    #[test]
    fn a_ship_still_spawning_in_does_not_strike() {
        let mut app = App::new();
        app.add_message::<AiShipDestroyed>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_ai_cripple);
        let ship = app.world_mut().spawn((
            AiShip, Transform::default(), AiShipType::TerranHegemony, AiShipState::default(),
            AiLoadout { guns: 6, engines: 20, command: 8, power_mw: 4720.0, ..default() },
        )).id();
        app.world_mut().spawn((module(ModuleType::HelmStation, 0.0), OwnedByAiShip { root: ship }, ChildOf(ship)));
        app.update();
        assert!(!app.world().get::<AiShipState>(ship).unwrap().is_destroyed);
    }

    /// Engines gone, but its guns are on its turrets, a level below the ship.
    #[test]
    fn guns_on_turrets_count_and_a_ship_with_them_fights_on_without_engines() {
        let mut app = App::new();
        app.add_message::<AiShipDestroyed>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_ai_cripple);
        let ship = app.world_mut().spawn((
            AiShip, Transform::default(), AiShipType::TerranHegemony, AiShipState { last_hit_timer: 0.0, ..default() },
            AiLoadout { guns: 2, engines: 4, ..default() },
        )).id();
        let ring = app.world_mut().spawn((module(ModuleType::TurretRing, 0.0), OwnedByAiShip { root: ship }, ChildOf(ship))).id();
        let turret = app.world_mut().spawn((crate::combat::turret_deck::TurretBody::new(ring, IVec2::ZERO, 3), ChildOf(ship))).id();
        for _ in 0..2 {
            app.world_mut().spawn((module(ModuleType::Gatling, 100.0), OwnedByAiShip { root: ship }, ChildOf(turret)));
        }
        app.update();
        assert!(!app.world().get::<AiShipState>(ship).unwrap().is_destroyed);
    }

    /// A fully armed, fully driven ship with these modules besides, built
    /// with `loadout`: does it strike?
    fn strikes(loadout: AiLoadout, alive: &[(ModuleType, f32)]) -> bool {
        let mut app = App::new();
        app.add_message::<AiShipDestroyed>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_ai_cripple);
        let ship = app.world_mut().spawn((
            AiShip, Transform::default(), AiShipType::RecursiveKingdom, AiShipState { last_hit_timer: 0.0, ..default() }, loadout,
        )).id();
        let mut kit = vec![(ModuleType::Gatling, 0.0), (ModuleType::StandardEngine, 0.0)];
        kit.extend_from_slice(alive);
        for (t, power) in kit {
            let mut m = module(t, 100.0);
            m.power_generation = power;
            app.world_mut().spawn((m, OwnedByAiShip { root: ship }, ChildOf(ship)));
        }
        app.update();
        app.world().get::<AiShipState>(ship).unwrap().is_destroyed
    }

    #[test]
    fn a_ship_with_its_bridge_shot_away_strikes_with_its_guns_still_on() {
        let built = AiLoadout { guns: 1, engines: 1, command: 2, power_mw: 500.0, ..default() };
        let reactor = (ModuleType::StandardReactor, 500.0);
        assert!(!strikes(built, &[reactor, (ModuleType::HelmStation, 0.0)]), "one helm left is enough to fight on");
        assert!(strikes(built, &[reactor]));
    }

    #[test]
    fn a_ship_down_to_a_quarter_of_its_power_strikes() {
        let built = AiLoadout { guns: 1, engines: 1, command: 1, power_mw: 2000.0, ..default() };
        let helm = (ModuleType::HelmStation, 0.0);
        assert!(!strikes(built, &[helm, (ModuleType::StandardReactor, 500.0), (ModuleType::StandardReactor, 500.0)]));
        assert!(strikes(built, &[helm, (ModuleType::StandardReactor, 500.0)]));
    }

    #[test]
    fn a_ship_that_has_lost_most_of_its_crew_strikes() {
        let crewed = |alive: u32| {
            let mut app = App::new();
            app.add_message::<AiShipDestroyed>();
            app.add_message::<ShowNotification>();
            app.add_systems(Update, check_ai_cripple);
            let ship = app.world_mut().spawn((
                AiShip, Transform::default(), AiShipType::RecursiveKingdom, AiShipState { last_hit_timer: 0.0, ..default() },
                AiLoadout { guns: 1, engines: 1, ..default() }, super::super::air::AiCrewComplement(8),
            )).id();
            for t in [ModuleType::Gatling, ModuleType::StandardEngine] {
                app.world_mut().spawn((module(t, 100.0), OwnedByAiShip { root: ship }, ChildOf(ship)));
            }
            for i in 0..8 {
                let health = if i < alive { 100.0 } else { 0.0 };
                app.world_mut().spawn((
                    crate::components::CrewMember { name: "Kade".into(), health, max_health: 100.0, oxygen: 100.0, morale: 100.0, state: crate::components::CrewState::Idle },
                    OwnedByAiShip { root: ship }, ChildOf(ship),
                ));
            }
            app.update();
            app.world().get::<AiShipState>(ship).unwrap().is_destroyed
        };
        assert!(!crewed(4), "half the crew is still a crew");
        assert!(crewed(2));
    }

    #[test]
    fn a_ship_built_with_no_bridge_cant_lose_one() {
        let built = AiLoadout { guns: 1, engines: 1, command: 0, power_mw: 0.0, ..default() };
        assert!(!strikes(built, &[]));
    }
}

#[cfg(test)]
mod fleet_call_tests {
    use super::*;

    fn voice(fleet: u32, behavior: AiShipBehavior, target: Option<Entity>) -> FleetVoice {
        FleetVoice { fleet: Some(fleet), behavior, target, last_attacker: None, last_hit_timer: 99.0, destroyed: false, escort: false }
    }

    fn hit(mut v: FleetVoice, by: Entity, secs_ago: f32) -> FleetVoice {
        v.last_attacker = Some(by);
        v.last_hit_timer = secs_ago;
        v
    }

    /// One member already scrapping with some other ship must not drown out
    /// the one you are shooting: the whole squadron turns on you.
    #[test]
    fn a_squadron_answers_you_over_its_other_fights() {
        let player = Entity::from_raw_u32(1).unwrap();
        let other = Entity::from_raw_u32(2).unwrap();
        let calls = fleet_calls(
            [
                voice(7, AiShipBehavior::Engaging, Some(other)),
                hit(voice(7, AiShipBehavior::Patrolling, None), player, 0.5),
                voice(7, AiShipBehavior::Patrolling, None),
            ]
            .into_iter(),
            Some(player),
        );
        assert_eq!(calls, vec![(7, player)]);
    }

    /// Killing one outright still brings the rest -- for a few seconds.
    #[test]
    fn a_member_killed_outright_still_calls() {
        let player = Entity::from_raw_u32(1).unwrap();
        let wreck = |secs_ago| {
            let mut v = hit(voice(3, AiShipBehavior::Dead, None), player, secs_ago);
            v.destroyed = true;
            v
        };
        assert_eq!(fleet_calls([wreck(0.25)].into_iter(), Some(player)), vec![(3, player)]);
        let stale = fleet_calls([wreck(9.0)].into_iter(), Some(player));
        assert!(stale.is_empty(), "an old wreck keeps calling: {stale:?}");
    }

    /// Your own escort shrugs off a stray round from you mid-fight.
    #[test]
    fn your_escort_forgives_a_stray_round() {
        let player = Entity::from_raw_u32(1).unwrap();
        let enemy = Entity::from_raw_u32(2).unwrap();
        let mut v = hit(voice(4, AiShipBehavior::Engaging, Some(enemy)), player, 0.5);
        v.escort = true;
        assert_eq!(fleet_calls([v].into_iter(), Some(player)), vec![(4, enemy)]);
    }
}

#[cfg(test)]
mod gunnery_tests {
    use super::*;

    fn state(destroyed: bool) -> AiShipState {
        AiShipState { is_destroyed: destroyed, ..default() }
    }

    /// A ship that has struck its colors does not shoot.
    ///
    /// `check_ai_cripple` announces "derelict adrift, ripe for salvage" and
    /// the game then invites the player to board it with F. It was still
    /// firing the whole time, because the only gate was a behaviour field
    /// that nothing updates after death.
    #[test]
    fn a_derelict_does_not_shoot() {
        assert!(
            !ship_may_fire(&AiShipBehavior::Engaging, &state(true)),
            "a ship that struck its colors kept firing at the player"
        );
    }

    /// The living still do.
    #[test]
    fn an_engaging_ship_shoots() {
        assert!(ship_may_fire(&AiShipBehavior::Engaging, &state(false)));
    }

    /// And only the engaging ones. A patrolling ship is not firing at you.
    #[test]
    fn a_ship_not_engaging_holds_fire() {
        assert!(!ship_may_fire(&AiShipBehavior::Patrolling, &state(false)));
        assert!(!ship_may_fire(&AiShipBehavior::Dead, &state(false)));
    }
    use crate::combat::impact::{obliquity, Obliquity};
    use crate::building::Block;
    use crate::components::HullMaterial;
    use bevy::math::{IVec2, Vec2};

    const ALL_FACTIONS: [AiShipType; 10] = [
        AiShipType::StellarPreserve, AiShipType::SynthesisCollective, AiShipType::BrokenChoir,
        AiShipType::CorpseStars, AiShipType::TheSilence, AiShipType::TerranHegemony,
        AiShipType::GildedThrone, AiShipType::RecursiveKingdom, AiShipType::EternalHegemony,
        AiShipType::Shepherd,
    ];

    /// The point of switching is to stop skipping. Every faction's answer has
    /// to actually beat the geometry that beat it — otherwise the crew "adapts"
    /// into the same failure and the player still can't be touched.
    #[test]
    fn every_factions_answer_survives_the_angle_that_beat_it() {
        // The plate that caused the problem, hit at a glancing 78 degrees.
        let plate = Block::hull(IVec2::ZERO, HullMaterial::Composite);
        let face = Vec2::NEG_X;
        let incoming = Vec2::from_angle(78f32.to_radians());

        // Baseline: unspecialised fire skips off this, which is why the crew
        // is reconsidering in the first place.
        assert!(obliquity(face, incoming, &plate, None, 1.0).ricochet);

        for faction in ALL_FACTIONS {
            let answer = angle_proof_round(faction);
            let o = obliquity(face, incoming, &plate, Some(answer), 1.0);
            let spall = crate::combat::ammo_types::spall(Some(answer));
            // Either it doesn't deflect, or it doesn't NEED to get through.
            assert!(
                !o.ricochet || spall.through_solid,
                "{faction:?} switches to {answer:?}, which still skips and can't spall through"
            );
        }
    }

    /// A switch has to be a change. Reloading the round that was already
    /// bouncing is the crew learning nothing.
    #[test]
    fn the_answer_is_never_the_default_round() {
        for faction in ALL_FACTIONS {
            assert_ne!(angle_proof_round(faction), KineticAmmoType::AP);
        }
    }

    /// Head-on, the switch must not be a straight downgrade — the crew is
    /// solving an angle problem, not throwing damage away.
    #[test]
    fn the_answer_still_works_square_on() {
        let plate = Block::hull(IVec2::ZERO, HullMaterial::Composite);
        for faction in ALL_FACTIONS {
            let answer = angle_proof_round(faction);
            let o = obliquity(Vec2::NEG_X, Vec2::X, &plate, Some(answer), 1.0);
            assert!(!o.ricochet, "{faction:?}'s {answer:?} shouldn't bounce dead-on");
            assert_eq!(o, Obliquity::HEAD_ON.clone(), "{faction:?}: square-on is square-on");
        }
    }
}

// (Kills are announced once, by ai_ship::wreck when the wreck forms: its
// headline names the cause -- struck colours, core detonation, gutted -- and
// the wreck's condition. A second announcer here put up a line saying the
// same thing for every kill, which in a swarm fight was half the stack.)


#[cfg(test)]
mod attacker_tests {
    use super::*;

    /// Friendly fire hurts, but it doesn't make an enemy: a fleet that
    /// named a fleet-mate as its attacker turned on it, through anything
    /// parked nearby.
    #[test]
    fn only_an_enemy_is_remembered_as_the_attacker() {
        let [ship, mate, player] = [1, 2, 3].map(|n| Entity::from_raw_u32(n).unwrap());
        let gilded = Some(AiShipType::GildedThrone);
        assert!(!counts_as_attacker(ship, gilded, ship, gilded), "its own blast");
        assert!(!counts_as_attacker(ship, gilded, mate, gilded), "a fleet-mate");
        assert!(!counts_as_attacker(ship, Some(AiShipType::MerchantPrinces), mate, Some(AiShipType::MerchantPrinces)), "same guild");
        assert!(counts_as_attacker(ship, gilded, player, None), "the player");
        assert!(counts_as_attacker(ship, Some(AiShipType::MerchantPrinces), mate, Some(AiShipType::FreeHaulersCompact)), "the trade war");
    }
}
