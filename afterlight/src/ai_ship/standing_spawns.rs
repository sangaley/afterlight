//! Your reputation follows you around.
//!
//! What a faction thinks of you now decides who comes looking:
//!
//! - **Hunters.** A faction that counts you Hostile, and holds or borders the
//!   system you're in, sends a hunter wing after you every so often: two
//!   ships at the edge of hostility, five and a capital when it truly hates
//!   you. They come in from beyond sight, straight for you, and give up after
//!   a while.
//! - **An escort.** In the space of a faction you're allied with (or of a
//!   trading faction you're allied with, whose ally then flies it), a few of
//!   its line ships form up on you and fight whoever fights you. Its patrols
//!   nearby answer when you're attacked, too.
//! - **A war fleet.** A big blow to a faction (a station, a capital ship: a
//!   drop of 20 standing or more at once) brings a battle group to the
//!   system you're in.
//!
//! All three are ordinary fleets in the world simulation with a mission on
//! them (`fleet::Mission`), so the block budget, saving and off-screen
//! fighting treat them like any other.

use bevy::prelude::*;
use rand::Rng;

use super::components::*;
use super::fleet::{self, FleetKind, FleetMember, Mission};
use crate::components::Ship;
use crate::diplomacy::{FactionStanding, StandingTier};

/// How far out a hunt or war fleet starts: past where ships materialise, so
/// it arrives rather than appears.
const ARRIVAL_RANGE: f32 = 54_000.0;
/// First hunt after this long in a system with a faction that hates you.
const FIRST_HUNT: f32 = 240.0;
/// Then one every this long, give or take.
const HUNT_EVERY: std::ops::Range<f32> = 360.0..600.0;
/// A hunt or war fleet that hasn't found you by then goes home.
const HUNT_LASTS: f32 = 900.0;
/// A standing drop this big in one go is a provocation.
pub const PROVOCATION: f32 = 20.0;
/// No second war fleet from the same faction within this long.
const WAR_FLEET_COOLDOWN: f32 = 600.0;
/// How fast a fleet sent after (or with) you travels off-screen.
const MISSION_CRUISE: f32 = 150.0;
/// An escort holds station this far behind you.
const ESCORT_BEHIND: f32 = 2_500.0;
/// Your distress reaches an ally's ships this far off.
const ALLY_HEARS_YOU: f32 = 12_000.0;

/// An escort ship flying with you.
#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerEscort;

/// A ship of a hunter wing or war fleet: it came for you, so it goes for you
/// whatever its faction's own habits (the Corpse Stars only ram what enters
/// their deep zone, say).
#[derive(Component, Clone, Copy, Debug)]
pub struct Hunter;

#[derive(Resource, Default)]
pub struct StandingSpawns {
    /// Seconds until each faction may hunt you again.
    hunt_clock: Vec<(AiShipType, f32)>,
    war_clock: Vec<(AiShipType, f32)>,
    /// The system an escort was last sent in (one per visit).
    escorted_in: Option<u32>,
    /// The system the clocks were counting for.
    system: Option<u32>,
}

/// AFTERLIGHT_HUNT_FIRST=<seconds>: the first hunt comes this soon (test switch).
fn first_hunt() -> f32 {
    std::env::var("AFTERLIGHT_HUNT_FIRST").ok().and_then(|v| v.parse().ok()).unwrap_or(FIRST_HUNT)
}

fn clock(clocks: &mut Vec<(AiShipType, f32)>, f: AiShipType, start: f32) -> &mut f32 {
    if let Some(i) = clocks.iter().position(|(g, _)| *g == f) {
        return &mut clocks[i].1;
    }
    clocks.push((f, start));
    &mut clocks.last_mut().unwrap().1
}

/// How many hunters a faction sends, and whether a capital leads them:
/// more the more it hates you.
pub fn hunt_size(standing: f32) -> (usize, bool) {
    let hate = ((StandingTier::HOSTILE_BELOW - standing) / 60.0).clamp(0.0, 1.0);
    (2 + (hate * 3.0).round() as usize, hate > 0.7)
}

/// The hulls of a hunter wing.
fn hunter_hulls(faction: AiShipType, standing: f32, rng: &mut impl Rng) -> Option<Vec<fleet::Hull>> {
    let (size, capital) = hunt_size(standing);
    let mut held = fleet::Held::new();
    let mut hulls = if capital {
        fleet::compose(faction, FleetKind::BattleGroup, rng, &mut held).unwrap_or_default()
    } else {
        Vec::new()
    };
    while hulls.len() < size {
        let more = fleet::compose(faction, FleetKind::Raiders, rng, &mut held)
            .or_else(|| fleet::compose(faction, FleetKind::Patrol, rng, &mut held))?;
        hulls.extend(more);
    }
    hulls.truncate(size.max(if capital { 3 } else { 0 }));
    Some(hulls)
}

/// Put a mission fleet into the simulation: `hulls` coming in from
/// `range` out on a random bearing, headed for `target`.
pub fn send_fleet(
    sim: &mut WorldSimulation,
    faction: AiShipType,
    kind: FleetKind,
    hulls: &[fleet::Hull],
    system_id: u32,
    target: Vec2,
    range: f32,
    mission: Mission,
    rng: &mut impl Rng,
) -> u32 {
    let at = target + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * range;
    let (mut f, mut members) = fleet::new_fleet(faction, kind, hulls, system_id, at, target, range * 2.0, rng);
    f.heading = (target - at).to_angle();
    f.destination = target;
    f.mission = mission;
    // Sent for you: it comes at speed, not at its patrol pace.
    f.cruise = MISSION_CRUISE;
    if matches!(mission, Mission::Escort) {
        // Behind you, not on top of you: the whole formation sits back.
        for m in &mut members {
            m.slot.x -= ESCORT_BEHIND;
        }
    }
    sim.add_fleet(f, members)
}

/// Hunters and escorts, by standing, in the system you're in.
#[allow(clippy::too_many_arguments)]
pub fn standing_spawns(
    time: Res<Time>,
    mut state: ResMut<StandingSpawns>,
    mut sim: ResMut<WorldSimulation>,
    standing: Res<FactionStanding>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    player: Query<&Transform, With<Ship>>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    let Ok(player) = player.single() else { return };
    let player_pos = player.translation.truncate();
    let Some(system_id) = streaming.loaded_system else { return };
    let Some(system) = galaxy.systems.iter().find(|s| s.id == system_id) else { return };
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    if state.system != Some(system_id) {
        // A new system: the clocks start over, and an escort can be sent.
        state.system = Some(system_id);
        state.hunt_clock.clear();
    }

    // Who's around: the owner and its neighbours. The bosses haunt rather
    // than hunt, and a trading faction hires its hunting out to its ally.
    let mut present: Vec<AiShipType> = system.faction.into_iter().collect();
    present.extend(crate::celestial::territory::border_factions(&galaxy, system_id));
    let hunters: Vec<AiShipType> = present
        .iter()
        .map(|f| f.ally().filter(|_| f.is_trading()).unwrap_or(*f))
        .filter(|f| standing.attacks_player(*f))
        .collect();
    for faction in hunters {
        let t = clock(&mut state.hunt_clock, faction, first_hunt());
        *t -= dt;
        if *t > 0.0 {
            continue;
        }
        *t = rng.gen_range(HUNT_EVERY);
        // One hunt at a time from a faction.
        let hunting = sim.fleets.iter().any(|f| f.faction == faction && matches!(f.mission, Mission::Hunt { .. }));
        if hunting {
            continue;
        }
        let Some(hulls) = hunter_hulls(faction, standing.value(faction), &mut rng) else { continue };
        let id = send_fleet(&mut sim, faction, FleetKind::Raiders, &hulls, system_id, player_pos, ARRIVAL_RANGE, Mission::Hunt { left: HUNT_LASTS }, &mut rng);
        info!("STANDING hunt: {:?} sends fleet {id} ({} ships) at standing {:.0}", faction, hulls.len(), standing.value(faction));
        notes.write(crate::events::ShowNotification {
            message: format!(
                "A {} hunter wing is on your trail ({} ships)",
                faction_display_name(faction),
                hulls.len()
            ),
            notification_type: crate::events::NotificationType::Danger,
            duration: 5.0,
        });
    }

    // An escort, in allied space, once a visit.
    if state.escorted_in != Some(system_id) {
        if let Some(owner) = system.faction.filter(|f| standing.tier(*f) == StandingTier::Allied) {
            let flier = owner.ally().filter(|_| owner.is_trading()).unwrap_or(owner);
            let mut held = fleet::Held::new();
            if let Some(hulls) = fleet::compose(flier, FleetKind::StationGuard, &mut rng, &mut held) {
                let hulls: Vec<_> = hulls.into_iter().take(3).collect();
                state.escorted_in = Some(system_id);
                let id = send_fleet(&mut sim, flier, FleetKind::Patrol, &hulls, system_id, player_pos, 12_000.0, Mission::Escort, &mut rng);
                info!("STANDING escort: {:?} sends fleet {id} ({} ships)", flier, hulls.len());
                notes.write(crate::events::ShowNotification {
                    message: format!("{} ships form up on you: an escort through their space", faction_display_name(flier)),
                    notification_type: crate::events::NotificationType::Success,
                    duration: 5.0,
                });
            }
        }
    }
}

/// A big blow to a faction brings a war fleet to the system you're in.
pub fn war_fleets(
    time: Res<Time>,
    mut state: ResMut<StandingSpawns>,
    mut sim: ResMut<WorldSimulation>,
    mut standing: ResMut<FactionStanding>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    player: Query<&Transform, With<Ship>>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    let dt = time.delta_secs();
    for (_, t) in &mut state.war_clock {
        *t -= dt;
    }
    // Only touch standing when there is something to take: a mutable borrow
    // marks it changed, and every frame of that made the diplomacy window
    // rebuild itself every frame.
    if standing.provocations.is_empty() {
        return;
    }
    let provoked: Vec<AiShipType> = std::mem::take(&mut standing.provocations);
    let (Ok(player), Some(system_id)) = (player.single(), streaming.loaded_system) else { return };
    let player_pos = player.translation.truncate();
    let mut rng = rand::thread_rng();
    for faction in provoked {
        // A trading faction sends its ally; the bosses don't send anyone.
        let sender = faction.ally().filter(|_| faction.is_trading()).unwrap_or(faction);
        if matches!(sender, AiShipType::EternalHegemony | AiShipType::Shepherd | AiShipType::TheSilence) {
            continue;
        }
        let t = clock(&mut state.war_clock, sender, 0.0);
        if *t > 0.0 {
            continue;
        }
        *t = WAR_FLEET_COOLDOWN;
        let mut held = fleet::Held::new();
        let hulls = fleet::compose(sender, FleetKind::BattleGroup, &mut rng, &mut held).or_else(|| {
            let mut a = fleet::compose(sender, FleetKind::Patrol, &mut rng, &mut held)?;
            a.extend(fleet::compose(sender, FleetKind::Patrol, &mut rng, &mut held)?);
            Some(a)
        });
        let Some(hulls) = hulls else { continue };
        let id = send_fleet(&mut sim, sender, FleetKind::BattleGroup, &hulls, system_id, player_pos, ARRIVAL_RANGE, Mission::Hunt { left: HUNT_LASTS }, &mut rng);
        info!("STANDING war fleet: {:?} sends fleet {id} ({} ships)", sender, hulls.len());
        notes.write(crate::events::ShowNotification {
            message: format!("The {} has sent a war fleet after you", faction_display_name(sender)),
            notification_type: crate::events::NotificationType::Danger,
            duration: 6.0,
        });
    }
}

/// Your escort fights whoever is fighting you, and so do your allies'
/// patrols nearby: any ship engaging you is everyone's target.
pub fn allies_defend_player(
    player: Query<(Entity, &Transform), With<Ship>>,
    mut ships: Query<(Entity, &Transform, &AiShipType, &AiShipBehavior, &AiShipTarget, &mut AiShipState, Has<PlayerEscort>), With<AiShip>>,
    standing: Res<FactionStanding>,
) {
    let Ok((player, player_tf)) = player.single() else { return };
    let player_pos = player_tf.translation.truncate();
    let attacker = ships
        .iter()
        .filter(|(_, _, _, b, t, s, _)| **b == AiShipBehavior::Engaging && t.entity == Some(player) && !s.is_destroyed)
        .min_by(|a, b| a.1.translation.truncate().distance(player_pos).total_cmp(&b.1.translation.truncate().distance(player_pos)))
        .map(|(e, ..)| e);
    let Some(attacker) = attacker else { return };
    for (e, tf, faction, _, _, mut state, escort) in ships.iter_mut() {
        if e == attacker || state.is_destroyed || !faction_fights(*faction) {
            continue;
        }
        let ally = standing.tier(*faction) == StandingTier::Allied;
        let near = tf.translation.truncate().distance(player_pos) < ALLY_HEARS_YOU;
        if escort || (ally && near) {
            state.alert_target = Some(attacker);
            state.alert_timer = state.alert_timer.max(4.0);
        }
    }
}

/// Mission fleets off-screen: hunters and war fleets steer for you and give
/// up in time; an escort keeps with you, and leaves when you leave its space
/// or its faction stops being your ally.
pub fn tick_missions(
    time: Res<Time>,
    mut sim: ResMut<WorldSimulation>,
    standing: Res<FactionStanding>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    player: Query<&Transform, With<Ship>>,
    mut commands: Commands,
    escorts: Query<(Entity, &FleetMember), Or<(With<PlayerEscort>, With<Hunter>)>>,
) {
    let dt = time.delta_secs();
    let player_pos = player.single().ok().map(|t| t.translation.truncate());
    let sim: &mut WorldSimulation = &mut sim;
    let live: std::collections::HashSet<u32> =
        sim.ships.iter().filter(|s| s.spawned && s.fleet != 0).map(|s| s.fleet).collect();
    let mut done: Vec<u32> = Vec::new();
    for f in sim.fleets.iter_mut() {
        match &mut f.mission {
            Mission::None => {}
            Mission::Hunt { left } => {
                *left -= dt;
                if *left <= 0.0 || streaming.loaded_system != Some(f.system_id) {
                    done.push(f.id);
                } else if let Some(p) = player_pos {
                    f.destination = p;
                    f.home_zone = p;
                }
            }
            Mission::Escort => {
                let owner = galaxy.systems.iter().find(|s| s.id == f.system_id).and_then(|s| s.faction);
                let still = owner.is_some_and(|o| standing.tier(o) == StandingTier::Allied);
                if streaming.loaded_system != Some(f.system_id) || !still {
                    done.push(f.id);
                } else if let Some(p) = player_pos {
                    f.destination = p;
                    f.home_zone = p;
                }
            }
        }
    }
    for id in done {
        if live.contains(&id) {
            // On screen: it stops being a mission and is just a fleet here,
            // and an escort stops flying with you.
            if let Some(f) = sim.fleets.iter_mut().find(|f| f.id == id) {
                f.mission = Mission::None;
            }
            for (e, m) in escorts.iter() {
                if m.fleet == id {
                    commands.entity(e).try_remove::<(PlayerEscort, Hunter)>();
                }
            }
        } else {
            sim.ships.retain(|s| s.fleet != id);
            sim.fleets.retain(|f| f.id != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunts_grow_with_hate() {
        let (small, cap_small) = hunt_size(-41.0);
        let (big, cap_big) = hunt_size(-100.0);
        assert_eq!((small, cap_small), (2, false));
        assert_eq!((big, cap_big), (5, true));
    }

    #[test]
    fn a_hunter_wing_is_the_size_its_hate_says() {
        let mut rng = rand::thread_rng();
        for standing in [-45.0, -70.0, -100.0] {
            let hulls = hunter_hulls(AiShipType::TerranHegemony, standing, &mut rng).unwrap();
            let (size, capital) = hunt_size(standing);
            assert!(hulls.len() >= size.min(3) && hulls.len() <= size.max(3), "{standing}: {} hulls", hulls.len());
            if capital {
                assert!(hulls.iter().any(|&(f, c)| fleet::role_of(f, c) == crate::ai_ship::roster::ClassRole::Capital));
            }
        }
    }

    #[test]
    fn an_escort_sits_behind_you() {
        let mut sim = WorldSimulation::default();
        let mut rng = rand::thread_rng();
        let hulls = fleet::hulls_of(AiShipType::TerranHegemony, &[crate::ai_ship::roster::intern("warden")]);
        let id = send_fleet(&mut sim, AiShipType::TerranHegemony, FleetKind::Patrol, &hulls, 1, Vec2::ZERO, 12_000.0, Mission::Escort, &mut rng);
        let ship = sim.ships.iter().find(|s| s.fleet == id).unwrap();
        assert!(ship.slot.x <= -ESCORT_BEHIND);
        assert_eq!(sim.fleet(id).unwrap().destination, Vec2::ZERO);
    }
}
