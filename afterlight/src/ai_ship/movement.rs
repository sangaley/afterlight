use bevy::prelude::*;

use crate::components::*;
use super::components::*;

/// Top speed for harbour traffic inside the harbour.
const HARBOUR_SPEED: f32 = 350.0;
/// How far past a starport's hull the harbour reaches: the guard fleet
/// patrols within 8 km of it, convoys berth 4 km off it.
const HARBOUR_REACH: f32 = 12_000.0;
/// Past the harbour the limit lifts gradually (u/s per unit out), so a fleet
/// leaving it speeds up instead of lurching.
const HARBOUR_EASE: f32 = 0.25;

/// Speed limit for a ship `distance` from the centre of a starport whose hull
/// reaches `extent`. A Gilded Throne patrol at 2,000 u/s wheeled its formation
/// into itself and shoved hulls into whatever sat by the station.
pub fn harbour_speed_cap(distance: f32, extent: f32) -> f32 {
    HARBOUR_SPEED + (distance - extent - HARBOUR_REACH).max(0.0) * HARBOUR_EASE
}

/// Top speed per square root of thrust-per-block. A ship's speed comes from
/// how much engine it has for its size, like the player's thrust over mass.
/// It used to be the raw sum of its engines' thrust, which never looked at
/// the hull: the fleet classes carry 5-16 engines, and a 160-block raider
/// with five of them ran at 2,500 u/s -- 3,300 fleeing -- and a throne barge
/// at 5,800. The root squeezes the range: a heavy hull with little engine is
/// slow, a light one with lots is quick, and neither is absurd.
const AI_SPEED_PER_ROOT_TPB: f32 = 180.0;

/// No AI ship goes faster than this, whatever it's doing.
const AI_MAX_SPEED: f32 = 1_100.0;

/// Base speed for a ship with `thrust` from its live engines over `blocks`.
/// The faction's character is applied softened (its root): the old
/// multipliers were tuned against raw thrust and would stretch the range
/// back out.
pub fn ai_top_speed(thrust: f32, blocks: usize, faction_mult: f32) -> f32 {
    let per_block = thrust / blocks.max(1) as f32;
    AI_SPEED_PER_ROOT_TPB * per_block.max(0.0).sqrt() * faction_mult.max(0.0).sqrt()
}

/// How fast an AI ship can swing its nose round, in rad/s, by size. Small
/// ships turn quicker than big ones; nothing whips about. Turning used to
/// ease toward the target heading with no limit at all, so whenever the
/// heading it wanted changed -- a target circling, a formation slot moving,
/// a swerve round a rock -- the ship snapped across in a fraction of a
/// second, and a ship fighting at close range jittered left and right. The
/// player's own hull turns ~0.86 rad/s.
pub fn ai_turn_rate(blocks: usize) -> f32 {
    (19.0 / (blocks.max(1) as f32).sqrt()).clamp(0.35, 1.8)
}

/// Turn `rotation` toward `target_angle`, easing in but never faster than
/// `max_rate` rad/s.
fn turn_toward(rotation: Quat, target_angle: f32, blend: f32, max_rate: f32, dt: f32) -> Quat {
    let current = rotation.to_euler(EulerRot::ZYX).0;
    let diff = (target_angle - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let step = (diff * blend).clamp(-max_rate * dt, max_rate * dt);
    Quat::from_rotation_z(current + step)
}

/// AFTERLIGHT_STANDOFF=<u> (filming): the distance a fighting ship holds, before
/// hull sizes, in place of the one its guns choose (at most 3,000 in play).
/// For a wide shot of two lines fighting across open space.
/// The distances a faction likes to fight at (from its target's hull), or
/// None for one that rams.
pub fn fighting_band(faction: AiShipType) -> Option<(f32, f32)> {
    use AiShipType::*;
    match faction {
        RecursiveKingdom => None,
        // Snipers: big guns and the discipline to stay out of reach.
        EternalHegemony | TerranHegemony | GildedThrone => Some((18_000.0, 22_000.0)),
        // Brawlers: frag swarms, EMP and tractor beams want it close.
        BrokenChoir | CorpseStars | Shepherd => Some((5_000.0, 9_000.0)),
        // Everyone else holds the line between.
        _ => Some((12_000.0, 18_000.0)),
    }
}

use super::awareness::Tactic;

fn filming_standoff() -> Option<f32> {
    static V: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_STANDOFF").ok().and_then(|v| v.parse().ok()))
}

/// AFTERLIGHT_HOLD_LINES=1 (filming): ships in a fight hold where they are --
/// turning to face, swaying, firing -- instead of closing on a target out in
/// the enemy's second row. Keeps two placed battle lines apart for a shot.
fn hold_lines() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_HOLD_LINES").ok().as_deref() == Some("1"))
}

/// How far the target's hull reaches from its middle: a fighting distance is
/// held to its near side, or a big target is hugged.
fn target_reach(target: Option<&AiShipTarget>, hulls: &Query<&crate::ship::collision::Collider>) -> f32 {
    target.and_then(|t| t.entity).and_then(|e| hulls.get(e).ok()).map_or(0.0, |c| c.bound_radius)
}

/// Where a ship heading for a point `to_dest` away should point its nose.
///
/// Far off, at the point. Close in, the bearing to the point swings with every
/// few units of error, so an escort sitting on its formation slot -- the slot
/// moving under it as the leader turns -- nodded its nose left and right all
/// the time. Near the point the ship faces the way it is actually moving, and
/// holds its heading when it isn't moving, easing back onto the bearing as the
/// distance opens.
pub(crate) fn heading_for(to_dest: Vec2, velocity: Vec2, current: f32) -> f32 {
    let dist = to_dest.length();
    let bearing = to_dest.y.atan2(to_dest.x);
    if dist >= NEAR {
        return bearing;
    }
    // Moving briskly toward the point: face the way it's moving. Stopped, or
    // drifting past the point after overshooting it: keep the heading it has
    // rather than spinning round to face backwards.
    let toward = velocity.length() > 150.0 && velocity.dot(to_dest) > 0.0;
    let base = if toward { velocity.y.atan2(velocity.x) } else { current };
    let k = (dist / NEAR).clamp(0.0, 1.0);
    base + wrap(bearing - base) * k * k
}

/// The share of its top speed a ship wants `dist` from its point. It brakes
/// into the point: at full speed to within a few units of it, a ship
/// overshot, swung back, overshot again -- looping round its stop for good,
/// the "ships just doing circles" at a station. An escort eases into its slot
/// over a longer run. Not while fighting: a rammer means to hit.
fn arrive_factor(dist: f32, max_speed: f32, settle: bool, engaging: bool) -> f32 {
    if settle {
        (dist / 600.0).min(1.0)
    } else if engaging {
        1.0
    } else {
        (dist / (max_speed * 0.3).max(250.0)).min(1.0)
    }
}

/// Room a ship in a fight keeps between its hull and any other.
const KEEP_CLEAR: f32 = 700.0;

/// Which way, and how hard (0..1), a hull at `at` should ease off the others
/// crowding it: away from each one closer than both radii plus KEEP_CLEAR,
/// the harder the deeper it's in.
fn keep_clear(me: Entity, at: Vec2, radius: f32, hulls: &[(Entity, Vec2, f32)]) -> Vec2 {
    let mut push = Vec2::ZERO;
    for &(other, center, r) in hulls {
        if other == me {
            continue;
        }
        let off = at - center;
        let reach = radius + r + KEEP_CLEAR;
        let d = off.length();
        if d < reach {
            let away = if d > 1.0 { off / d } else { Vec2::X };
            push += away * (1.0 - d / reach);
        }
    }
    push.clamp_length_max(1.0)
}

/// Only the sideways part of a push, across the line to `toward` (the
/// target): ships behind shoved the front of a line into the enemy, and the
/// whole side crept in. How far off the target it holds is the standoff's
/// business.
fn along_the_line(push: Vec2, toward: Vec2) -> Vec2 {
    let side = Vec2::new(-toward.y, toward.x);
    side * push.dot(side)
}

/// Within this of its point, a ship stops aiming its nose at the point.
const NEAR: f32 = 1_200.0;

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Where an escort points: the formation's heading near its slot, the
/// bearing to the slot when it's far off and has ground to make up.
pub(crate) fn escort_heading(face: f32, to_dest: Vec2, velocity: Vec2, current: f32) -> f32 {
    const FAR: f32 = 3_000.0;
    let dist = to_dest.length();
    if dist >= FAR {
        return heading_for(to_dest, velocity, current);
    }
    let k = ((dist - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0);
    let far = to_dest.y.atan2(to_dest.x);
    face + wrap(far - face) * k * k
}

/// Ease the heading a ship steers for toward `raw`, so a target that flips
/// back and forth -- a slot jittering round the ship, avoidance picking a
/// side and dropping it -- averages out instead of wagging the nose.
pub(crate) fn smooth_heading(prev: Option<f32>, raw: f32, dt: f32) -> f32 {
    const SETTLE: f32 = 0.5; // seconds
    match prev {
        None => raw,
        Some(p) => p + wrap(raw - p) * (1.0 - (-dt / SETTLE).exp()),
    }
}

/// Moves AI ships toward their nav destination, reading child Engine components for thrust.
pub fn ai_ship_movement_system(
    time: Res<Time>,
    mut ai_ships: Query<(
        Entity,
        &mut Transform,
        &mut Velocity,
        &AiShipBehavior,
        &mut AiShipNav,
        &AiShipState,
        &AiShipType,
        &Children,
        Option<&crate::ship::collision::Collider>,
        Option<&super::fleet::FleetMember>,
        Has<super::standing_spawns::PlayerEscort>,
        Option<&AiShipTarget>,
    ), With<AiShip>>,
    // The target's own size, for AFTERLIGHT_STANDOFF_HULLS (see below).
    hulls: Query<&crate::ship::collision::Collider>,
    engine_query: Query<(&Engine, &Module, &OwnedByAiShip)>,
    weapon_query: Query<(&Weapon, &Module, &OwnedByAiShip), Without<Engine>>,
    // Terrain look-ahead so ships swing around rocks/stations on their way
    // instead of grinding along everything that's solid now.
    collider_grid: Res<crate::ship::collision::ColliderGrid>,
    terrain: Query<
        (&Transform, &crate::ship::collision::Collider),
        crate::ship::collision::TerrainFilter,
    >,
    // The player's hull is in the way too: a fleet crossing near you flowed
    // into it, and an escort could wedge itself between the Pincer's claws.
    player: Query<(&Transform, &crate::ship::collision::Collider), (With<Ship>, Without<AiShip>)>,
    faction_stations: Res<crate::diplomacy::stations::FactionStations>,
    // What the ship has decided to do about the fight (ai_ship::awareness).
    field: Res<super::awareness::Battlefield>,
    mind: Res<super::awareness::Mind>,
    minds: Query<&super::awareness::Awareness>,
    trading: Res<super::traffic::TradingFleets>,
) {
    let dt = time.delta_secs();
    let player_obstacle = player.single().ok().map(|(tf, c)| {
        (crate::ship::collision::world_point(tf, c.bound_center), c.bound_radius)
    });

    // Every live AI hull, so ships in a fight can keep clear of each other.
    let ai_hulls: Vec<(Entity, Vec2, f32)> = ai_ships
        .iter()
        .filter_map(|(e, tf, _, behavior, _, state, _, _, collider, ..)| {
            let c = collider.filter(|_| *behavior != AiShipBehavior::Dead && !state.is_destroyed)?;
            Some((e, crate::ship::collision::world_point(tf, c.bound_center), c.bound_radius))
        })
        .collect();

    // Each ship's reach: its longest gun or beam, wherever it stands --
    // built guns are on turrets, a level below the ship, and a ship that
    // only read its own children held its range for a reach of 0. Missiles
    // only count for a ship with nothing else: a silo reaches 12 km and
    // would hold a sniper past every gun it has.
    let reach: bevy::platform::collections::HashMap<Entity, f32> = {
        let mut guns: bevy::platform::collections::HashMap<Entity, f32> = Default::default();
        let mut missiles: bevy::platform::collections::HashMap<Entity, f32> = Default::default();
        for (weapon, module, owner) in weapon_query.iter() {
            if !module.is_active || module.health <= 0.0 {
                continue;
            }
            let missile = crate::building::is_launcher(module.module_type) || module.module_type == ModuleType::VlsSilo;
            let r = (if missile { &mut missiles } else { &mut guns }).entry(owner.root).or_insert(0.0);
            *r = r.max(weapon.range);
        }
        for (ship, r) in missiles {
            guns.entry(ship).or_insert(r);
        }
        guns
    };
    for (_entity, mut transform, mut velocity, behavior, mut nav, state, ship_type, children, collider, member, escort, target) in ai_ships.iter_mut() {
        if *behavior == AiShipBehavior::Dead || state.is_destroyed {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        // Sum thrust from child engines — tracking the undamaged total too,
        // so we know how crippled the drive section is.
        let mut total_thrust = 0.0_f32;
        let mut max_possible_thrust = 0.0_f32;
        for child in children.iter() {
            if let Ok((engine, module, _owned)) = engine_query.get(child) {
                max_possible_thrust += engine.thrust;
                if module.is_active && module.health > 0.0 {
                    let efficiency = module.health / module.max_health;
                    total_thrust += engine.thrust * efficiency;
                }
            }
        }

        // DEATH SPIRALS — a ship with mangled engines can't hold a heading.
        // Thrust asymmetry shows up as a slow sinusoidal weave injected into
        // its steering; the worse the drive damage, the wider and drunker
        // the wander. Phase from the entity index so a damaged squadron
        // doesn't wobble in lockstep.
        let engine_integrity = if max_possible_thrust > 0.0 {
            (total_thrust / max_possible_thrust).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let wobble_angle = if engine_integrity < 0.85 {
            let hurt = 1.0 - engine_integrity;
            let phase = (_entity.to_bits() % 97) as f32;
            (time.elapsed_secs() * 2.3 + phase).sin() * hurt * hurt * 1.1
        } else {
            0.0
        };

        // Speed multiplier per behavior
        let behavior_mult = match behavior {
            AiShipBehavior::Patrolling | AiShipBehavior::Idle => 0.6,
            AiShipBehavior::FollowingTradeRoute => 0.8,
            AiShipBehavior::Engaging => 1.0,
            AiShipBehavior::Fleeing | AiShipBehavior::EvadingCreature => 1.3,
            AiShipBehavior::Salvaging => 0.3,
            AiShipBehavior::Dead => 0.0,
        };

        // In formation, the leader holds back so its escorts can keep up,
        // and an escort out of place hurries to its slot.
        let to_slot = nav.destination.map_or(0.0, |d| d.distance(transform.translation.truncate()));
        let behavior_mult = match (member, behavior) {
            (Some(m), AiShipBehavior::Patrolling) if m.leader => behavior_mult * 0.75,
            // An escort keeps up with you: you fly at full speed, it doesn't
            // dawdle at patrol pace.
            (Some(_), AiShipBehavior::Patrolling) if escort => behavior_mult * 1.6,
            (Some(_), AiShipBehavior::Patrolling) if to_slot > 1_500.0 => behavior_mult * 1.35,
            _ => behavior_mult,
        };
        // An escort settles into its slot rather than overshooting it.
        let settle = (escort || member.is_some_and(|m| !m.leader)) && *behavior == AiShipBehavior::Patrolling;

        // Faction-specific speed characteristics
        let faction_mult = match ship_type {
            AiShipType::TheSilence => 1.6,      // fastest ship in the game
            AiShipType::RecursiveKingdom => 1.3,      // fast but erratic
            AiShipType::GildedThrone => 1.2,     // quick tactical ship
            AiShipType::StellarPreserve => 0.9,      // creature-towed, moderate
            AiShipType::SynthesisCollective => 1.0,    // average
            AiShipType::BrokenChoir => 0.7,        // sluggish, damaged engines
            AiShipType::CorpseStars => 0.8,   // heavy but powerful engines
            AiShipType::TerranHegemony => 0.6,       // slow battleship
            AiShipType::EternalHegemony => 0.4,    // colossal, lumbering
            AiShipType::Shepherd => 0.35,     // barely moves, but it doesn't need to
            AiShipType::MerchantPrinces => 0.9,   // laden freight
            AiShipType::FreeHaulersCompact => 0.8, // pushing a load
        };

        let blocks = children.len();
        let max_speed = (ai_top_speed(total_thrust, blocks, faction_mult) * behavior_mult).min(AI_MAX_SPEED);
        // A trader about its business gets on with it.
        let max_speed = match member.filter(|m| *behavior == AiShipBehavior::Patrolling && trading.0.contains(&m.fleet)) {
            Some(m) if m.leader => max_speed.max(super::traffic::SPEED_ON_SCREEN),
            Some(_) => max_speed.max(super::traffic::ESCORT_SPEED_ON_SCREEN),
            None => max_speed,
        };
        let turn_rate = ai_turn_rate(blocks);
        // Harbour traffic goes slow. A ship going about its business near a
        // faction starport keeps to harbour speed; one fighting or running
        // for its life doesn't.
        let harbour_bound = matches!(
            behavior,
            AiShipBehavior::Patrolling | AiShipBehavior::Idle | AiShipBehavior::FollowingTradeRoute | AiShipBehavior::Salvaging
        );
        let max_speed = match faction_stations.post {
            Some((_, port, extent)) if harbour_bound => {
                max_speed.min(harbour_speed_cap(transform.translation.truncate().distance(port), extent))
            }
            _ => max_speed,
        };

        // Frame-rate-independent smoothing fraction: closes this much of the
        // remaining gap (to a target velocity or a target angle) per second.
        // The old `(80.0 * dt).min(1.0)` saturated to a full 1.0 snap at any
        // normal framerate — velocity teleported to its new value every
        // single frame while rotation eased in smoothly, so the ship visibly
        // slid in a direction its nose wasn't pointed at. This converges
        // continuously instead, same shape at 30fps or 300fps.
        let vel_blend = 1.0 - (-6.0_f32 * dt).exp();
        let turn_blend = 1.0 - (-4.5_f32 * dt).exp();

        if let Some(destination) = nav.destination {
            let pos = transform.translation.truncate();
            let own_radius = collider.map(|c| c.bound_radius).unwrap_or(300.0);
            // Room kept round your hull, wider the faster this ship is going:
            // its velocity eases onto a new heading, so a hull at 1,500 u/s
            // carries on a few hundred units before a swerve takes.
            let player_obstacle = player_obstacle
                .map(|(center, radius)| (center, radius + (velocity.0.length() * 0.3).clamp(200.0, 800.0)));
            // A formation slot or waypoint that lands on you is taken just
            // outside your hull instead. A patrol sweeping past a parked
            // ship had escorts flying their slots straight through it.
            let destination = match player_obstacle {
                Some((center, radius)) if *behavior != AiShipBehavior::Engaging => {
                    let keep = radius + own_radius;
                    if destination.distance(center) < keep {
                        center + (destination - center).normalize_or((pos - center).normalize_or(Vec2::X)) * keep
                    } else {
                        destination
                    }
                }
                _ => destination,
            };
            // Drive damage skews the perceived heading — the ship weaves
            // toward where its broken engines are dragging it.
            let to_dest = Vec2::from_angle(wobble_angle).rotate(destination - pos);
            let dist = to_dest.length();

            // Combat standoff: while engaging, hold a firing distance from the
            // target instead of flying into (and through) it. Below the band:
            // back off. Inside the band: orbit sideways. Beyond it: approach.
            // Recursive Kingdom keeps its point-blank ramming — that IS their faction,
            // regardless of what it's carrying. Everyone else holds at their
            // own longest-range active weapon (85% of its range, so they
            // fight solidly inside their own reach instead of right at the
            // ragged edge) — a ship stripped down to short-range guns closes
            // in, one carrying a sniper weapon hangs back, instead of every
            // ship in a faction using the same fixed distance regardless of
            // loadout. Falls back to the old faction defaults only if a ship
            // somehow has no active weapons (e.g. mid-repair).
            // Its main armament's reach: its longest gun can be a long way
            // past where the rest of its guns do anything.
            let longest = reach.get(&_entity).copied().unwrap_or(0.0);
            let max_weapon_range = field.hulls.get(&_entity).map_or(longest, |h| h.fighting()).max(1.0).min(longest.max(1.0));
            let max_weapon_range = if longest > 0.0 { max_weapon_range } else { 0.0 };

            // Fights at range, FtD-style: a ship holds where its weapons
            // still connect -- 70% of its longest reach, so a sniper's fight
            // stays inside its target's guns too -- inside its
            // faction's band, and never past 90% of what it can hit. Snipers
            // hang back 18-22 km, line ships 12-18, brawlers close to 5-9 km,
            // and the Recursive Kingdom still rams (that IS their identity).
            // It used to be half its reach capped at 3 km, from when slow
            // shells on small ships never landed at range; ships are twice
            // the size now and the guns are built to reach.
            let standoff = if *behavior == AiShipBehavior::Engaging {
                match fighting_band(*ship_type) {
                    None => 0.0,
                    Some((lo, hi)) if max_weapon_range > 0.0 => {
                        (max_weapon_range * 0.7).clamp(lo, hi).min(max_weapon_range * 0.9)
                    }
                    Some((lo, _)) => lo,
                }
            } else {
                0.0
            };
            let standoff = filming_standoff().filter(|_| standoff > 0.0).unwrap_or(standoff);
            // The gap is the guns' business: from this ship's middle (where its
            // guns are, on the whole) to the target's near side, and never
            // past what they reach. The hulls only set a floor, so two big
            // ships never shove each other. Adding its own size on top parked
            // a big sniper 11-12 km off with guns that reach 7.
            let awareness = minds.get(_entity).ok();
            let tactic = awareness.map_or(Tactic::None, |a| a.tactic);
            let enemy = target.and_then(|t| t.entity).and_then(|e| field.hulls.get(&e));
            let (standoff, floor, ceiling, closing, backing) = if standoff > 0.0 {
                let target_r = target_reach(target, &hulls);
                let floor = own_radius + target_r + 600.0;
                let ceiling = if max_weapon_range > 0.0 { max_weapon_range * 0.95 + target_r } else { f32::MAX };
                let (hold, closing, backing) = super::awareness::plan_range(
                    tactic, standoff + target_r, max_weapon_range, enemy.map_or(0.0, |e| e.reach),
                    own_radius, target_r, floor, ceiling.max(floor), &mind,
                );
                // A ship recovering goes out past its own guns on purpose.
                let ceiling = if tactic == Tactic::Recover { hold } else { ceiling };
                (hold, floor, ceiling.max(floor), closing, backing)
            } else {
                (0.0, 0.0, 0.0, 0.5, 0.4)
            };
            let temper = super::awareness::temperament(*ship_type);
            // Weave, in fractions of top speed, once it's in a fight.
            let weave = if tactic == Tactic::None {
                0.0
            } else {
                super::awareness::jink(
                    _entity.to_bits(), time.elapsed_secs(), &temper, &mind,
                    awareness.map_or(0.0, |a| a.hurt_rate), awareness.map_or(f32::INFINITY, |a| a.inbound_eta),
                )
            };

            if standoff > 0.0 && (dist < standoff * 1.3 || hold_lines()) && dist > 1.0 {
                let direction = to_dest / dist; // toward the player
                // Hold and shoot instead of orbiting. Each ship parks at its own
                // range (jittered so they don't all sit at one perfect radius)
                // and STATION-KEEPS there: it only closes if pushed too far out,
                // eases back only if crowded too close — otherwise it holds and
                // fires. No perpetual circling.
                let bits = _entity.to_bits();
                let jitter = 0.8 + ((bits % 13) as f32 / 13.0) * 0.35; // 0.8..1.15
                // Only a settled line keeps its jitter: a tactic means the range.
                let settled = matches!(tactic, Tactic::None | Tactic::Hold | Tactic::Flank);
                let hold = (standoff * if settled { jitter } else { 1.0 }).clamp(floor, ceiling);
                if crate::combat::gun_assembly::trace() && (time.elapsed_secs() % 2.0) < dt {
                    info!("STANDOFF {:?} {:?}: {:.0} u off, holding {:.0} (reach {:.0})", _entity, ship_type, dist, hold, max_weapon_range);
                }
                let radial = if hold_lines() {
                    Vec2::ZERO
                // ...and comes back in before its guns stop reaching: the 10%
                // of slack took a ship holding near its limit out of range and
                // left it there, shooting nothing.
                } else if dist > (hold * 1.1).min(ceiling) {
                    direction * max_speed * closing // drift in
                } else if dist < hold * (if settled { 0.7 } else { 0.92 }) {
                    -direction * max_speed * backing // ease back
                } else {
                    Vec2::ZERO // in the pocket — hold position
                };
                // A faint sideways sway so it's not a frozen statue — a small
                // fraction of the old orbit, not a full circle.
                let phase = (bits % 211) as f32;
                let sway_sign = if bits & 1 == 0 { 1.0 } else { -1.0 };
                let tangent = Vec2::new(-direction.y, direction.x) * sway_sign;
                let sway = tangent * max_speed * 0.1 * (time.elapsed_secs() * 0.4 + phase).sin();
                // In a fight it weaves, to spoil anyone leading it; working round
                // a target, it goes for their stern.
                let side = Vec2::new(-direction.y, direction.x);
                let sway = sway + side * max_speed * weave;
                let sway = match (tactic, enemy, field.hulls.get(&_entity)) {
                    (Tactic::Flank, Some(them), Some(me)) => {
                        // `side` is anticlockwise round this ship looking at
                        // the target; round the target it's the other way.
                        sway - side * max_speed * mind.flank * super::awareness::flank_side(me, them)
                    }
                    _ => sway,
                };
                // And off any hull crowding its own: every ship holding on
                // one target parked in the same spot, and a side became one
                // heap of overlapping hulls. Now it spreads along its line.
                let own_center = collider.map_or(pos, |c| crate::ship::collision::world_point(&transform, c.bound_center));
                let spread = along_the_line(keep_clear(_entity, own_center, own_radius, &ai_hulls), direction) * max_speed * 0.5;
                velocity.0 = velocity.0.lerp(radial + sway + spread, vel_blend);

                // Face the player — you're fine with this, and it reads natural
                // for a ship holding station and trading fire.
                let target_angle = direction.y.atan2(direction.x);
                nav.steer = Some(target_angle);
                transform.rotation = turn_toward(transform.rotation, target_angle, turn_blend, turn_rate, dt);
            } else if dist > 5.0 {
                // Bend the heading around any terrain blocking the way.
                let direction = crate::ship::collision::steer_around(
                    &collider_grid,
                    &terrain,
                    pos,
                    to_dest / dist,
                    dist,
                    own_radius,
                );
                // Not while fighting you: a ship engaging holds its own
                // standoff, and a rammer means to hit.
                let direction = match player_obstacle {
                    Some((center, radius)) if *behavior != AiShipBehavior::Engaging => {
                        crate::ship::collision::steer_clear_of(pos, direction, dist, own_radius, center, radius)
                    }
                    _ => direction,
                };

                // Gradually accelerate toward desired velocity
                let arrive = arrive_factor(dist, max_speed, settle, *behavior == AiShipBehavior::Engaging);
                let side = Vec2::new(-direction.y, direction.x);
                let desired_vel = direction * max_speed * arrive + side * max_speed * weave * 0.7;
                velocity.0 = velocity.0.lerp(desired_vel, vel_blend);

                // Face where it's going -- or, close to the point, where it's
                // actually moving (heading_for); an escort near its slot
                // faces the way the formation does. Smoothed (smooth_heading).
                let current = transform.rotation.to_euler(EulerRot::ZYX).0;
                let raw = match nav.face.filter(|_| settle) {
                    Some(face) => escort_heading(face, direction * dist, velocity.0, current),
                    None => heading_for(direction * dist, velocity.0, current),
                };
                let target_angle = smooth_heading(nav.steer, raw, dt);
                nav.steer = Some(target_angle);
                transform.rotation = turn_toward(transform.rotation, target_angle, turn_blend, turn_rate, dt);
            } else {
                // Near destination, slow down
                velocity.0 *= (1.0 - 3.0 * dt).max(0.0);
            }
        } else {
            // No destination, drift to stop
            velocity.0 *= (1.0 - 2.0 * dt).max(0.0);
        }

        // Apply drag
        let drag = 0.5 * dt;
        velocity.0 *= 1.0 - drag;

        // Apply velocity to position
        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;
    }
}

/// Updates AI ship depth from Y position
pub fn ai_thruster_system(
    mut ai_ships: Query<(&Transform, &mut AiShipState), With<AiShip>>,
) {
    for (transform, mut state) in ai_ships.iter_mut() {
        // Depth = negative Y (deeper = more negative Y = higher depth value)
        state.depth = (-transform.translation.y / 10.0).max(0.0);
    }
}

/// Consumes fuel based on engine activity
pub fn ai_fuel_system(
    time: Res<Time>,
    mut ai_ships: Query<(&mut AiShipState, &AiShipBehavior, &Children), With<AiShip>>,
    engine_query: Query<(&Engine, &Module, &OwnedByAiShip)>,
) {
    let dt = time.delta_secs();

    for (mut state, behavior, children) in ai_ships.iter_mut() {
        if *behavior == AiShipBehavior::Dead {
            continue;
        }

        let mut fuel_consumption = 0.0_f32;
        for child in children.iter() {
            if let Ok((engine, module, _)) = engine_query.get(child) {
                if module.is_active && module.health > 0.0 {
                    fuel_consumption += engine.fuel_consumption;
                }
            }
        }

        // Reduce consumption when idle/patrolling
        let consumption_mult = match behavior {
            AiShipBehavior::Idle => 0.1,
            AiShipBehavior::Patrolling | AiShipBehavior::Salvaging => 0.5,
            AiShipBehavior::FollowingTradeRoute => 0.7,
            _ => 1.0,
        };

        state.fuel = (state.fuel - fuel_consumption * consumption_mult * dt).max(0.0);
    }
}

#[cfg(test)]
mod harbour_tests {
    use super::*;

    /// Inside the harbour everyone keeps to harbour speed; the limit lifts
    /// smoothly outside it, and is gone a system's width away.
    #[test]
    fn harbour_traffic_slows_near_the_starport() {
        let extent = 2_300.0;
        assert_eq!(harbour_speed_cap(0.0, extent), HARBOUR_SPEED);
        assert_eq!(harbour_speed_cap(extent + HARBOUR_REACH, extent), HARBOUR_SPEED);
        let mut last = HARBOUR_SPEED;
        for step in 1..100 {
            let cap = harbour_speed_cap(extent + HARBOUR_REACH + step as f32 * 200.0, extent);
            assert!(cap > last && cap - last <= 200.0 * HARBOUR_EASE + 0.01, "the limit jumps at {step}");
            last = cap;
        }
        assert!(harbour_speed_cap(100_000.0, extent) > 10_000.0);
    }
}

#[cfg(test)]
mod heading_tests {
    use super::*;

    /// An escort riding on its slot: the slot jitters a few dozen units round
    /// it every tick while the fleet cruises east. Its nose must stay east,
    /// not swing to each new bearing.
    #[test]
    fn an_escort_on_its_slot_does_not_nod() {
        let cruising = Vec2::new(300.0, 0.0);
        for (dx, dy) in [(30.0, 25.0), (-20.0, 30.0), (25.0, -30.0), (-30.0, -20.0)] {
            let h = heading_for(Vec2::new(dx, dy), cruising, 0.0);
            assert!(h.abs() < 0.05, "slot at ({dx},{dy}) swung the nose to {h}");
        }
    }

    /// Stopped near its point, a ship holds the heading it has.
    #[test]
    fn a_ship_arriving_at_speed_settles_on_its_point() {
        // The movement loop's own velocity rule, at 60 fps: a ship coming in
        // fast and off-line must end up parked on its point, not looping it.
        let max_speed = 1500.0;
        let dt = 1.0 / 60.0;
        let mut pos = Vec2::new(-4000.0, 300.0);
        let mut vel = Vec2::new(max_speed, 0.0);
        let mut closest = f32::MAX;
        for _ in 0..(20.0 / dt) as usize {
            let to = -pos;
            let dist = to.length();
            if dist > 5.0 {
                let want = to / dist * max_speed * arrive_factor(dist, max_speed, false, false);
                vel = vel.lerp(want, 1.0 - (-6.0_f32 * dt).exp());
            } else {
                vel *= (1.0 - 3.0 * dt).max(0.0);
            }
            vel *= 1.0 - 0.5 * dt;
            pos += vel * dt;
            closest = closest.min(pos.length());
        }
        // in the brain's waypoint radius (80), and stopped
        assert!(pos.length() < 80.0, "ended {:.0} from its point", pos.length());
        assert!(vel.length() < 60.0, "still moving at {:.0}", vel.length());
        assert!(closest < 80.0);
    }

    #[test]
    fn a_ship_eases_off_a_hull_crowding_it_and_ignores_far_ones() {
        let mut world = World::new();
        let (me, near, far) = (world.spawn_empty().id(), world.spawn_empty().id(), world.spawn_empty().id());
        let hulls = [(me, Vec2::ZERO, 800.0), (near, Vec2::new(1000.0, 0.0), 800.0), (far, Vec2::new(0.0, 9000.0), 800.0)];
        let push = keep_clear(me, Vec2::ZERO, 800.0, &hulls);
        assert!(push.x < -0.3 && push.y.abs() < 1e-4, "{push}");
        // alone, nothing to ease off from
        assert_eq!(keep_clear(me, Vec2::ZERO, 800.0, &hulls[..1]), Vec2::ZERO);
    }

    #[test]
    fn a_crowded_ship_slides_along_its_line_not_into_the_enemy() {
        // enemy to the east; a push from behind (west) must not move it east
        let toward = Vec2::X;
        assert!(along_the_line(Vec2::new(0.8, 0.0), toward).length() < 1e-6);
        let side = along_the_line(Vec2::new(0.5, 0.5), toward);
        assert!(side.x.abs() < 1e-6 && (side.y - 0.5).abs() < 1e-6, "{side}");
    }

    #[test]
    fn a_rammer_does_not_brake() {
        assert_eq!(arrive_factor(50.0, 1500.0, false, true), 1.0);
    }

    #[test]
    fn a_ship_at_rest_by_its_point_holds_heading() {
        let h = heading_for(Vec2::new(0.0, 40.0), Vec2::ZERO, 0.7);
        assert!((h - 0.7).abs() < 0.01, "{h}");
    }

    /// An escort sitting on its slot faces the formation's way, wherever the
    /// slot jitters to.
    #[test]
    fn an_escort_near_its_slot_faces_the_formation() {
        let face = 1.0;
        for to_slot in [Vec2::new(200.0, -150.0), Vec2::new(-300.0, 250.0), Vec2::new(0.0, 900.0)] {
            let h = escort_heading(face, to_slot, Vec2::new(-80.0, 30.0), 0.2);
            assert!((h - face).abs() < 0.05, "slot {to_slot} pointed it to {h}");
        }
        // Far behind, it heads for the slot.
        let far = escort_heading(face, Vec2::new(0.0, -6_000.0), Vec2::ZERO, 0.0);
        assert!((far + std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{far}");
    }

    /// A target flipping between two sides every frame averages out: the
    /// heading steered for barely moves.
    #[test]
    fn a_flipping_target_does_not_wag_the_steering() {
        let mut steer = Some(0.0);
        let mut lo = 0.0f32;
        let mut hi = 0.0f32;
        for k in 0..240 {
            let raw = if k % 2 == 0 { 0.6 } else { -0.6 };
            let s = smooth_heading(steer, raw, 1.0 / 60.0);
            steer = Some(s);
            if k > 60 {
                lo = lo.min(s);
                hi = hi.max(s);
            }
        }
        assert!(hi - lo < 0.05, "steering swung {lo}..{hi}");
    }

    /// Far from the point, it still faces the point.
    #[test]
    fn far_off_it_faces_the_point() {
        let h = heading_for(Vec2::new(0.0, 5_000.0), Vec2::new(300.0, 0.0), 0.0);
        assert!((h - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{h}");
    }
}

#[cfg(test)]
mod ai_speed_tests {
    use super::*;

    /// The Silence's five-engine Listener: 160 blocks, 2000 thrust, the
    /// fastest faction. It ran at 2,560 u/s; it stays the quickest thing out
    /// there but under the ceiling, and fleeing (x1.3) too.
    #[test]
    fn the_fastest_raider_stays_under_the_ceiling() {
        let engage = ai_top_speed(2000.0, 161, 1.6);
        assert!(engage > 600.0 && engage * 1.3 <= AI_MAX_SPEED * 1.2, "{engage}");
    }

    /// A barge of 1,444 blocks with fifteen engines is not a racer.
    #[test]
    fn a_big_hull_is_slower_than_a_light_one_with_the_same_engines() {
        let barge = ai_top_speed(6000.0, 1444, 1.2);
        let courier = ai_top_speed(2000.0, 164, 1.2);
        assert!(barge < courier, "{barge} vs {courier}");
        assert!(barge < 600.0, "barge {barge}");
    }

    #[test]
    fn small_ships_turn_quicker_but_nothing_whips_round() {
        assert!(ai_turn_rate(150) > ai_turn_rate(1500));
        assert!(ai_turn_rate(10) <= 1.8 && ai_turn_rate(100_000) >= 0.35);
    }

    /// A half-turn target is reached at the capped rate, not in one frame.
    #[test]
    fn a_turn_is_capped_per_second() {
        let dt = 1.0 / 60.0;
        let mut rot = Quat::IDENTITY;
        for _ in 0..30 {
            rot = turn_toward(rot, std::f32::consts::PI * 0.9, 1.0, 1.0, dt);
        }
        let angle = rot.to_euler(EulerRot::ZYX).0;
        assert!((angle - 0.5).abs() < 0.02, "half a second at 1 rad/s turns 0.5 rad, got {angle}");
    }
}

#[cfg(test)]
mod fighting_band_tests {
    use super::*;

    #[test]
    fn snipers_hold_far_out_brawlers_close_and_the_kingdom_rams() {
        let (lo, hi) = fighting_band(AiShipType::EternalHegemony).unwrap();
        assert!(lo >= 18_000.0 && hi <= 22_000.0);
        let (_, brawl) = fighting_band(AiShipType::BrokenChoir).unwrap();
        assert!(brawl <= 9_000.0);
        assert!(fighting_band(AiShipType::RecursiveKingdom).is_none());
    }
}
