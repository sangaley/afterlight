use bevy::prelude::*;
use crate::components::{Ship, Velocity};
use crate::events::{ShowNotification, NotificationType};
use crate::resources::FuelState;
use super::components::*;
use super::resources::*;
use super::events::*;
use super::galaxy;

// Interstellar warp cost/time, scaled by galaxy-map distance (t = dist /
// GALAXY_RADIUS, 0..1) — deliberately separate from the G-key local warp
// dash (ui/mod.rs's WARP_DASH_* constants): that's a same-system reposition
// bounded by a ~600k-unit local map, this is a jump across the whole
// galaxy, orders of magnitude larger. Strawman numbers, tune by feel.
// Cost rises with the SQUARE of distance, and a full-radius jump deliberately
// costs more than a full tank.
//
// It used to be linear and capped at 500 of a 1500 tank, which meant a
// minute-one player could click the far edge of the galaxy map, wait four
// seconds, and arrive parked beside a station in the hardest territory in the
// game. Crossing the galaxy was not a cost, it was a menu.
//
// Squared keeps neighbours cheap and makes the edge unreachable in one go:
//   t=0.2  ->  144 fuel      a hop to a neighbour, trivial
//   t=0.5  ->  480 fuel      a real commitment, a third of a tank
//   t=0.8  -> 1104 fuel      most of everything you have
//   t=1.0  -> 1680 fuel      more than a tank: impossible in one jump
// So reaching the far systems means staging through the middle ones, which
// are exactly the ones that get progressively more dangerous.

/// Spin-up for a jump between stars. Charge follows the same squared curve as
/// fuel (see `interstellar_charge_time`), so a hop to a neighbour stays quick
/// and the far edge is a real wait: 4s, about 7.5s mid-galaxy, 18s at the rim.
///
/// Tuned in opposite directions more than once, so the history is kept. It
/// began at 6-60s, dead time at the keyboard; cut to 1-4s, the weight vanished
/// and crossing the galaxy cost the same as crossing a room. Two branches then
/// diverged -- cascade-story to 1-8s, galaxy-map-look to 4-18s -- and 4-18s
/// is kept for a reason that is not taste. The local warp dash is capped at 3s
/// (`ui::WARP_DASH_MAX_CHARGE`), so an interstellar jump starting at 1s would
/// be QUICKER than crossing a system and the two drives would invert.
/// `ui::warp_pacing_tests` pins that ordering.
const INTERSTELLAR_BASE_CHARGE: f32 = 4.0; // seconds
const INTERSTELLAR_MAX_EXTRA_CHARGE: f32 = 14.0;
pub(crate) const INTERSTELLAR_BASE_FUEL: f32 = 80.0;
pub(crate) const INTERSTELLAR_MAX_EXTRA_FUEL: f32 = 1600.0;

// pub(crate): reused by ui/mod.rs for the galaxy map's cost/charge preview
// so the sidebar shows the exact numbers the jump will actually use.
pub(crate) fn interstellar_charge_time(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    INTERSTELLAR_BASE_CHARGE + t * t * INTERSTELLAR_MAX_EXTRA_CHARGE
}

pub(crate) fn interstellar_fuel_cost(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    INTERSTELLAR_BASE_FUEL + t * t * INTERSTELLAR_MAX_EXTRA_FUEL
}

pub(crate) fn target_galaxy_pos(galaxy_map: &GalaxyMap, target: GalaxyWarpTarget) -> Option<Vec2> {
    match target {
        GalaxyWarpTarget::System(id) => galaxy_map.systems.iter().find(|s| s.id == id).map(|s| s.galaxy_pos),
        GalaxyWarpTarget::BlindPoint(pos) => Some(pos),
    }
}

/// The warp key (G, or V -- `ui::warp_key_pressed`) charges a jump toward
/// whatever the galaxy map has set as the pending target
/// (PendingGalaxyWarpTarget) — a known system, or a blind point in space
/// with nothing confirmed there (the galaxy map is clickable anywhere, not
/// just discovered pips — see ui/mod.rs). With no galaxy target the same key
/// dashes within the system instead (ui::warp_dash_input). Press once and
/// the charge runs on its own — no need to hold the key through an
/// 18-second charge; press again to cancel early. Charge time and fuel
/// cost both scale with galaxy-map distance to the target.
/// How far past a station's structure a warp drops your ship's root.
///
/// Must stay inside `home_base::DOCK_MARGIN` or arriving "at a station" means
/// arriving next to one you still have to fly to — which is the whole point
/// of landing here. Enough standoff that the hull doesn't materialise inside
/// the structure: the starter's hull reaches ~1,100 from its root.
pub const STATION_ARRIVAL_STANDOFF: f32 = 1_400.0;

pub fn warp_input_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    galaxy_map: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    pending: Res<PendingGalaxyWarpTarget>,
    mut fuel_state: ResMut<FuelState>,
    ship_query: Query<(Entity, Has<crate::ui::MapWarpCharging>), (With<Ship>, Without<WarpCharging>)>,
    mut charging_query: Query<(Entity, &mut WarpCharging), With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut jump_events: MessageWriter<WarpJumpStarted>,
    ward: Option<Res<crate::narrative::ward::WardState>>,
) {
    // The key starts the charge and it runs on its own from there — no need
    // to hold it through the whole (up to 18s) charge. A second press while
    // already charging cancels instead.
    if crate::ui::warp_key_pressed(&keyboard) {
        if let Ok((entity, _)) = charging_query.single() {
            commands.entity(entity).remove::<WarpCharging>();
            notifications.write(ShowNotification {
                message: "Warp cancelled.".into(),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        } else if let Ok((ship_entity, dash_charging)) = ship_query.single() {
            // No star picked, or a dash already spinning up: the key is the
            // local dash's (ui::warp_dash_input), and so is "no destination".
            let Some(target) = pending.0.filter(|_| !dash_charging) else { return };
            let Some(target_pos) = target_galaxy_pos(&galaxy_map, target) else { return };
            // Into the dark only with a working Dark Ward aboard.
            if let GalaxyWarpTarget::System(id) = target {
                if !crate::narrative::ward::may_enter(&galaxy_map, id, ward.as_ref().is_some_and(|w| w.aboard)) {
                    notifications.write(ShowNotification {
                        message: "The warp drive won't jump into the dark. It needs a Dark Ward aboard.".into(),
                        notification_type: NotificationType::Warning,
                        duration: 5.0,
                    });
                    return;
                }
            }

            let dist = streaming.current_galaxy_pos.distance(target_pos);
            if dist < 1.0 {
                notifications.write(ShowNotification {
                    message: "Already there.".into(),
                    notification_type: NotificationType::Info,
                    duration: 2.5,
                });
                return;
            }

            let t = dist / galaxy::GALAXY_RADIUS;
            let fuel_cost = interstellar_fuel_cost(t);
            if fuel_state.current_fuel < fuel_cost {
                notifications.write(ShowNotification {
                    message: format!("Not enough fuel for the jump ({:.0} needed, {:.0} available).", fuel_cost, fuel_state.current_fuel),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
                return;
            }
            let charge_time = interstellar_charge_time(t);
            let target_desc = match target {
                GalaxyWarpTarget::System(id) => galaxy_map.systems.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default(),
                GalaxyWarpTarget::BlindPoint(_) => "uncharted space".to_string(),
            };

            commands.entity(ship_entity).insert(WarpCharging {
                target,
                fuel_cost,
                charge_timer: Timer::from_seconds(charge_time, TimerMode::Once),
            });

            notifications.write(ShowNotification {
                message: format!("Warp drive charging toward {}... ({:.0}s, G again to cancel)", target_desc, charge_time),
                notification_type: NotificationType::Info,
                duration: charge_time + 1.0,
            });
        }
    }

    // Tick charge timer
    if let Ok((entity, mut charging)) = charging_query.single_mut() {
        let before = charging.charge_timer.fraction();
        charging.charge_timer.tick(time.delta());

        // Progress notification, once: on the frame the charge crosses half.
        // It was a 50-55% window, which every frame inside it re-announced --
        // a dozen "Warp drive at 50%..." per jump stacked up the notices.
        let pct = charging.charge_timer.fraction();
        if crossed_halfway(before, pct) {
            notifications.write(ShowNotification {
                message: "Warp drive at 50%...".into(),
                notification_type: NotificationType::Warning,
                duration: 1.5,
            });
        }

        // Jump when charge completes
        if charging.charge_timer.is_finished() {
            let target = charging.target;
            fuel_state.current_fuel = (fuel_state.current_fuel - charging.fuel_cost).max(0.0);
            commands.entity(entity).remove::<WarpCharging>();

            jump_events.write(WarpJumpStarted { target });
        }
    }
}

/// Execute the warp jump. A System target loads that system deterministically
/// (as before). A BlindPoint target either snaps to arriving at a real
/// system if one is within SNAP_TOLERANCE of it, or lands in genuinely
/// empty space — nothing spawns, loaded_system becomes None, but the
/// player still has a real local position and Warm neighbors keep ticking
/// based on proximity to that point. Either way, unloads the old system
/// first and refreshes Warm neighbors around the new position.
pub fn execute_warp_jump(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut jump_events: MessageReader<WarpJumpStarted>,
    mut galaxy: ResMut<GalaxyState>,
    mut galaxy_map: ResMut<GalaxyMap>,
    mut streaming: ResMut<SystemStreamingManager>,
    // The galaxy-map target, and the local G-dash target: a point in the
    // system being left, meaningless in the one arrived at.
    pending: (ResMut<PendingGalaxyWarpTarget>, ResMut<crate::ui::PendingWarpTarget>),
    celestial_query: Query<(Entity, &StarSystemMember)>,
    mut ship_query: Query<(&mut Transform, &mut Velocity), With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut completed_events: MessageWriter<WarpJumpCompleted>,
    spawn_assets: (Res<crate::vfx::procedural_textures::CelestialTextures>, Res<AssetServer>, Res<LootedPois>),
    time: Res<Time>,
    mut sim: ResMut<crate::ai_ship::components::WorldSimulation>,
    standing: Res<crate::diplomacy::FactionStanding>,
) {
    let (mut pending, mut dash_target) = pending;
    let (textures, asset_server, looted) = spawn_assets;
    for event in jump_events.read() {
        let Ok((mut ship_transform, mut ship_velocity)) = ship_query.single_mut() else {
            continue;
        };

        let now = time.elapsed_secs_f64();

        // Resolve the target into either a real system id (snapping a
        // close-enough blind point onto it) or a raw empty-space position.
        let resolved_system_id = match event.target {
            GalaxyWarpTarget::System(id) => Some(id),
            GalaxyWarpTarget::BlindPoint(pos) => galaxy::system_within_snap_tolerance(&galaxy_map, pos),
        };

        if let Some(old_id) = streaming.loaded_system {
            galaxy::unload_system(&mut commands, &celestial_query, &mut galaxy_map, old_id, now);
        }

        let (new_galaxy_pos, arrival_name) = if let Some(new_id) = resolved_system_id {
            let Some(system_info) = galaxy::load_system(&mut commands, &asset_server, &textures, &mut galaxy_map, new_id, now, &looted) else {
                continue;
            };
            let Some(def) = galaxy_map.systems.iter().find(|s| s.id == new_id) else { continue };
            let center = def.local_center;
            let galaxy_pos = def.galaxy_pos;
            let name = def.name.clone();

            // Land IN the faction's population cluster, not just "somewhere
            // safe near the star" — that flat 50k-100k distance used to put
            // the player nowhere near a weak faction's tight cluster (e.g.
            // Recursive Kingdom's whole territory is only 15k radius), landing them
            // in what looked like empty space every time. The cluster is
            // offset clear of the star itself (see
            // celestial::galaxy::faction_cluster_center), so arriving near
            // it can't accidentally drop the player inside/on top of the
            // star regardless of faction or star size.
            let arrival_pos = if def.faction.is_some() {
                let cluster_center = galaxy::faction_cluster_center(def);
                let territory_radius = def.faction
                    .and_then(|f| crate::ai_ship::components::faction_territories().into_iter().find(|t| t.faction == f))
                    .map(|t| t.radius)
                    .unwrap_or(30_000.0);
                let arrival_angle = rand::random::<f32>() * std::f32::consts::TAU;
                cluster_center + Vec2::new(arrival_angle.cos(), arrival_angle.sin()) * territory_radius * 0.5
            } else {
                // Safe system (Haven) — no faction cluster, just clear the star.
                let safe_distance = system_info.star_entity.map(|_| 100_000.0).unwrap_or(50_000.0);
                let arrival_angle = rand::random::<f32>() * std::f32::consts::TAU;
                center + Vec2::new(arrival_angle.cos(), arrival_angle.sin()) * safe_distance
            };
            ship_transform.translation.x = arrival_pos.x;
            ship_transform.translation.y = arrival_pos.y;

            streaming.loaded_system = Some(new_id);
            galaxy.systems.push(system_info);
            galaxy.current_system = new_id;

            (galaxy_pos, name)
        } else {
            // Genuinely empty space — nothing to spawn, nothing to load.
            let GalaxyWarpTarget::BlindPoint(pos) = event.target else { continue };
            let arrival_pos = galaxy::blind_point_local_center(pos);
            ship_transform.translation.x = arrival_pos.x;
            ship_transform.translation.y = arrival_pos.y;

            streaming.loaded_system = None;

            (pos, "uncharted space".to_string())
        };

        ship_velocity.0 = Vec2::ZERO; // Kill momentum on arrival
        streaming.current_galaxy_pos = new_galaxy_pos;
        streaming.warm_systems = galaxy::nearest_neighbors_to_pos(&galaxy_map, new_galaxy_pos, streaming.loaded_system, galaxy::WARM_NEIGHBOR_COUNT);
        pending.0 = None;
        // Left set, the next G press dashed toward a point that only meant
        // something in the old system -- across the new one, or into its sun.
        dash_target.0 = None;

        // First-ever visit populates a system's faction ships; a repeat
        // visit leaves whatever's left (however depleted by earlier combat)
        // exactly as it is — see ensure_system_population's doc comment.
        if let Some(new_id) = streaming.loaded_system {
            crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy_map, new_id);
        }
        for warm_id in streaming.warm_systems.clone() {
            crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy_map, warm_id);
        }

        // Arrive at a station, inside docking range.
        //
        // Every system carries STATIONS_PER_SYSTEM full stations (world::
        // home_base) — buy, sell, build and hire at all of them — so a jump
        // now lands somewhere you can refit rather than somewhere you have to
        // survive. The cluster-relative arrival computed above is a fallback
        // for systems we can't place a station for.
        //
        // This replaces a "land the player IN the fight" re-drop that put you
        // 5k from a live faction ship the moment you arrived. That solved a
        // real problem — factions field 1-5 ships across a 15-80k territory,
        // so a neutral arrival is usually empty space — but it meant every
        // jump opened in an engagement you didn't choose, with no chance to
        // sell a hold of salvage or repair first. Finding the fight is a
        // short flight; being dropped into one isn't a decision.
        let arrived_in = streaming
            .loaded_system
            .and_then(|id| galaxy_map.systems.iter().find(|s| s.id == id));
        if let Some(def) = arrived_in {
            if let Some(site) = crate::world::home_base::station_sites(def).first() {
                // Off to one side rather than on top of the structure.
                let a = rand::random::<f32>() * std::f32::consts::TAU;
                let arrival = site.pos + Vec2::new(a.cos(), a.sin()) * (site.radius() + STATION_ARRIVAL_STANDOFF);
                ship_transform.translation.x = arrival.x;
                ship_transform.translation.y = arrival.y;
            }
        }

        // Arrival burst. The departure end is in a system that is about to be
        // unloaded, so only this one is ever actually seen -- the jump itself
        // is a translation write, and without this the whole manoeuvre reads
        // as the starfield suddenly being different.
        let at = ship_transform.translation.truncate();
        crate::vfx::particles::spawn_warp_flash(
            &mut commands, &fx, at, 260.0, Color::srgb(0.45, 0.70, 1.0),
        );

        completed_events.write(WarpJumpCompleted { system_id: streaming.loaded_system });

        let hostile = streaming.loaded_system
            .and_then(|id| galaxy_map.systems.iter().find(|s| s.id == id))
            .is_some_and(|s| standing.system_hostile(s));
        notifications.write(if hostile {
            ShowNotification {
                message: format!("Arrived at {arrival_name} - HOSTILE SPACE, they attack on sight"),
                notification_type: NotificationType::Danger,
                duration: 6.0,
            }
        } else {
            ShowNotification {
                message: format!("Warp complete! Arrived at {}", arrival_name),
                notification_type: NotificationType::Success,
                duration: 4.0,
            }
        });
    }
}

/// When warp completes, notify the player about the new system (skipped
/// for a blind landing in empty space — nothing to scan).
pub fn on_warp_complete(
    mut completed_events: MessageReader<WarpJumpCompleted>,
    galaxy: Res<GalaxyState>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for event in completed_events.read() {
        let Some(system_id) = event.system_id else { continue };
        if let Some(system) = galaxy.systems.iter().find(|s| s.id == system_id) {
            let planet_count = system.planet_entities.len();
            notifications.write(ShowNotification {
                message: format!("System scan: {} planets detected. Proceed with caution.", planet_count),
                notification_type: NotificationType::Info,
                duration: 5.0,
            });
        }
    }
}

#[cfg(test)]
mod arrival_tests {
    use super::*;

    /// Warping into a system has to leave you able to press F. These two
    /// constants live in different files, so nothing but this connects them.
    #[test]
    fn arrival_standoff_is_inside_docking_range() {
        assert!(
            STATION_ARRIVAL_STANDOFF < crate::world::home_base::DOCK_MARGIN,
            "arriving {STATION_ARRIVAL_STANDOFF} past a {} docking margin means you land next to a station you still have to fly to",
            crate::world::home_base::DOCK_MARGIN
        );
    }

    /// ...and every system really does have a station to arrive at, so the
    /// fallback path is genuinely only for uncharted space.
    #[test]
    fn every_system_has_somewhere_to_arrive() {
        for seed in [1u64, 42, 7_777] {
            for def in &galaxy::generate_galaxy_map(seed).systems {
                let sites = crate::world::home_base::station_sites(def);
                assert!(!sites.is_empty(), "seed {seed}: {} has no station to warp to", def.name);
            }
        }
    }
}

#[cfg(test)]
mod cost_tests {
    use super::*;

    /// A neighbour must stay cheap, or ordinary play becomes a fuel chore.
    #[test]
    fn short_hops_are_affordable() {
        assert!(interstellar_fuel_cost(0.2) < 200.0);
        // Was an absolute `< 2.0`. That bound and the request to make the
        // interstellar jump LONGER cannot both hold: the local warp dash caps
        // at 3s (`ui::WARP_DASH_MAX_CHARGE`), and any interstellar jump under
        // that would be quicker than crossing a system -- the two drives
        // invert. So the absolute floor moved to 4s, and what this keeps is
        // the part of the original intent that survives it: a hop to a
        // neighbour is a small fraction of the wait to cross the galaxy.
        let short = interstellar_charge_time(0.2);
        let far = interstellar_charge_time(1.0);
        assert!(
            short < far * 0.35,
            "a short hop charges {short}s against {far}s for the far edge -- \
             neighbours are not meaningfully quicker to reach"
        );
    }

    /// And the far edge must not be reachable in one jump from a full tank.
    /// That is the whole point: staging through the middle systems is what
    /// turns distance into a difficulty ramp instead of a menu choice.
    #[test]
    fn the_far_edge_cannot_be_reached_in_one_jump() {
        const TANK: f32 = 1500.0; // FuelState::default max_fuel
        assert!(
            interstellar_fuel_cost(1.0) > TANK,
            "a full-radius jump costs {} of a {TANK} tank — the player can \
             still skip straight to the hardest content",
            interstellar_fuel_cost(1.0)
        );
        // Half way must still be possible, or the galaxy is unreachable.
        assert!(interstellar_fuel_cost(0.5) < TANK);
    }

    /// Cost must rise faster than distance, or halving the range halves the
    /// price and staging buys the player nothing.
    #[test]
    fn cost_is_superlinear() {
        let half = interstellar_fuel_cost(0.5) - INTERSTELLAR_BASE_FUEL;
        let full = interstellar_fuel_cost(1.0) - INTERSTELLAR_BASE_FUEL;
        assert!(full > half * 2.0, "half={half} full={full}");
    }
}

/// True only on the tick that carries a charge from below half to half or more.
fn crossed_halfway(before: f32, after: f32) -> bool {
    before < 0.5 && after >= 0.5
}

#[cfg(test)]
mod halfway_tests {
    use super::*;

    /// A four-second charge at 60fps passes through 50-55% over a dozen
    /// frames; it is announced on exactly one of them.
    #[test]
    fn halfway_is_announced_once() {
        let frames: Vec<f32> = (0..=240).map(|i| i as f32 / 240.0).collect();
        let hits = frames.windows(2).filter(|w| crossed_halfway(w[0], w[1])).count();
        assert_eq!(hits, 1);
    }
}
