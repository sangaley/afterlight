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
    overheard: Option<ResMut<crate::narrative::news::Overheard>>,
    preload: Option<ResMut<WarpPreload>>,
    site: Option<Res<crate::narrative::site::SitePlace>>,
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
                    // The drive finds out before you do: if you hadn't heard,
                    // this is how (narrative::news).
                    let sys = galaxy_map.systems.iter().find(|s| s.id == id);
                    let unheard = sys.is_some_and(|s| s.heard.taken_at.is_none());
                    let message = match sys {
                        Some(s) if unheard => format!("The drive won't take you. Something is wrong at {}.", s.name),
                        _ => "The warp drive won't jump into the dark. It needs a Dark Ward aboard.".to_string(),
                    };
                    if unheard {
                        if let Some(mut heard) = overheard {
                            heard.0.push(id);
                        }
                    }
                    notifications.write(ShowNotification {
                        message,
                        notification_type: NotificationType::Warning,
                        duration: 5.0,
                    });
                    return;
                }
            }

            // The summit site is in the dark as well, though it is open
            // space: the same rule (narrative::site).
            if !crate::narrative::site::may_jump(site.as_ref().and_then(|s| s.0), target, ward.as_ref().is_some_and(|w| w.aboard)) {
                notifications.write(ShowNotification {
                    message: "The warp drive won't jump into the tear. It needs a Dark Ward aboard.".to_string(),
                    notification_type: NotificationType::Warning,
                    duration: 5.0,
                });
                return;
            }

            let dist = streaming.current_galaxy_pos.distance(target_pos);
            // The drive goes so far and no further: a far place takes several
            // jumps, through other people's space. One tank used to reach
            // nine tenths of the galaxy, a hostile capital included.
            let reach = drive_reach(fuel_state.drive, &galaxy_map);
            if dist > reach {
                notifications.write(ShowNotification {
                    message: format!(
                        "Beyond the jump drive's reach ({:.1} of {:.1}). Go by way of a nearer system, or refit the drive at a shipyard.",
                        dist / 1_000_000.0,
                        reach / 1_000_000.0
                    ),
                    notification_type: NotificationType::Warning,
                    duration: 4.0,
                });
                return;
            }
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
            if let Some(mut preload) = preload {
                preload.jumping = true;
            }

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
    preload: Option<ResMut<WarpPreload>>,
) {
    let mut no_preload = WarpPreload::default();
    let mut preload_res = preload;
    let preload: &mut WarpPreload = match preload_res.as_deref_mut() {
        Some(p) => p,
        None => &mut no_preload,
    };
    let (mut pending, mut dash_target) = pending;
    let (textures, asset_server, looted) = spawn_assets;
    for event in jump_events.read() {
        let Ok((mut ship_transform, mut ship_velocity)) = ship_query.single_mut() else {
            continue;
        };

        let now = time.elapsed_secs_f64();

        // Loaded while the drive charged (preload_warp_destination): where
        // to come out, and the system already spawned. Otherwise worked out
        // and loaded now.
        let ready = (preload.target == Some(event.target) && preload.arrival.is_some()).then(|| std::mem::take(preload));
        *preload = WarpPreload::default();
        let (resolved_system_id, arrival_pos, ready_info) = match ready {
            Some(p) => (p.system, p.arrival.unwrap_or_default(), p.info),
            None => {
                let from = streaming.current_galaxy_pos;
                let (id, at) = plan_arrival(&galaxy_map, event.target, |d| d.faction.is_some_and(|f| standing.attacks_player(f)), from);
                (id, at, None)
            }
        };

        if let Some(old_id) = streaming.loaded_system.filter(|old| Some(*old) != resolved_system_id) {
            galaxy::unload_system(&mut commands, &celestial_query, &mut galaxy_map, old_id, now);
        }

        let (new_galaxy_pos, arrival_name) = if let Some(new_id) = resolved_system_id {
            let system_info = match ready_info {
                Some(info) => info,
                None => {
                    let Some(info) = galaxy::load_system(&mut commands, &asset_server, &textures, &mut galaxy_map, new_id, now, &looted) else {
                        continue;
                    };
                    info
                }
            };
            let Some(def) = galaxy_map.systems.iter().find(|s| s.id == new_id) else { continue };
            let galaxy_pos = def.galaxy_pos;
            let name = def.name.clone();
            ship_transform.translation.x = arrival_pos.x;
            ship_transform.translation.y = arrival_pos.y;

            streaming.loaded_system = Some(new_id);
            galaxy.systems.push(system_info);
            galaxy.current_system = new_id;

            (galaxy_pos, name)
        } else {
            // Genuinely empty space — nothing to spawn, nothing to load.
            let GalaxyWarpTarget::BlindPoint(pos) = event.target else { continue };
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

/// Where a jump to `target` comes out: the system it lands in (a blind point
/// close enough to a system snaps onto it) and the point. Decided when the
/// charge starts, so the destination can be loaded while the drive spools.
///
/// At a station, inside docking range, where the system has one -- every
/// system carries STATIONS_PER_SYSTEM full stations (world::home_base), so a
/// jump lands somewhere you can refit rather than somewhere you have to
/// survive. (A "land the player IN the fight" drop it replaced put you 5k
/// from a live faction ship on every arrival.) Otherwise in the faction's
/// population cluster, clear of the star, or clear of the star in a safe
/// system; a blind point with nothing there is empty space.
pub(crate) fn plan_arrival(
    galaxy_map: &GalaxyMap,
    target: GalaxyWarpTarget,
    hostile: impl Fn(&super::resources::StarSystemDef) -> bool,
    from: Vec2,
) -> (Option<u32>, Vec2) {
    let resolved = match target {
        GalaxyWarpTarget::System(id) => Some(id),
        GalaxyWarpTarget::BlindPoint(pos) => galaxy::system_within_snap_tolerance(galaxy_map, pos),
    };
    let a = rand::random::<f32>() * std::f32::consts::TAU;
    let dir = Vec2::new(a.cos(), a.sin());
    let Some(def) = resolved.and_then(|id| galaxy_map.systems.iter().find(|s| s.id == id)) else {
        let at = match target {
            GalaxyWarpTarget::BlindPoint(pos) => galaxy::blind_point_local_center(pos),
            GalaxyWarpTarget::System(_) => Vec2::ZERO,
        };
        return (resolved, at);
    };
    // Into the space of someone who shoots on sight: the drive puts you at
    // the edge, on the side you came from, not in their docking lane.
    if hostile(def) {
        return (resolved, hostile_arrival(def, from));
    }
    if let Some(site) = crate::world::home_base::station_sites(def).first() {
        // Out in its docking lane (east, kept clear for the berths), a little
        // to one side: a random bearing put the ship on top of a starport's
        // piers, for collision to shove it off. Still inside docking range.
        let across = (rand::random::<f32>() - 0.5) * 3_000.0;
        return (resolved, site.pos + Vec2::new(site.radius() + STATION_ARRIVAL_STANDOFF * 2.5, across));
    }
    let at = if def.faction.is_some() {
        let cluster_center = galaxy::faction_cluster_center(def);
        let territory_radius = def
            .faction
            .and_then(|f| crate::ai_ship::components::faction_territories().into_iter().find(|t| t.faction == f))
            .map(|t| t.radius)
            .unwrap_or(30_000.0);
        cluster_center + dir * territory_radius * 0.5
    } else {
        def.local_center + dir * 100_000.0
    };
    (resolved, at)
}

/// How far out from a hostile system's star the drive puts you: past the
/// jump points, short of its stations (180-420 km out).
const HOSTILE_EDGE: f32 = 140_000.0;

/// Where a jump from `from` (galaxy position) into the hostile system `def`
/// comes out: on the side you came from, clear of the star and the planets.
pub fn hostile_arrival(def: &super::resources::StarSystemDef, from: Vec2) -> Vec2 {
    let side = (from - def.galaxy_pos).normalize_or(Vec2::X);
    crate::ai_ship::traffic::clear_point(def, def.local_center + side * HOSTILE_EDGE, 8_000.0)
}

/// The jump drive's reach, by refit, before the galaxy has its say.
pub const DRIVE_REACH: [f32; 3] = [1_800_000.0, 2_800_000.0, 4_200_000.0];
/// What each refit costs, at a shipyard.
pub const DRIVE_REFIT_COST: [u32; 2] = [3_000, 7_000];

/// How far a jump drive with `refits` reaches in one jump. Never less than
/// the longest single jump needed to get everywhere by hops: no system may
/// be cut off by an unlucky galaxy.
pub fn drive_reach(refits: u8, galaxy: &GalaxyMap) -> f32 {
    DRIVE_REACH[(refits as usize).min(DRIVE_REACH.len() - 1)].max(longest_hop(galaxy) * 1.02)
}

/// The longest jump there is no way round: the hop every route to some
/// system needs at least (the widest link of a minimum spanning tree).
pub fn longest_hop(galaxy: &GalaxyMap) -> f32 {
    let n = galaxy.systems.len();
    if n < 2 {
        return 0.0;
    }
    let mut joined = vec![false; n];
    let mut best = vec![f32::MAX; n];
    best[0] = 0.0;
    let mut widest: f32 = 0.0;
    for _ in 0..n {
        let Some(i) = (0..n).filter(|&i| !joined[i]).min_by(|&a, &b| best[a].total_cmp(&best[b])) else { break };
        joined[i] = true;
        widest = widest.max(best[i]);
        for j in 0..n {
            if !joined[j] {
                let d = galaxy.systems[i].galaxy_pos.distance(galaxy.systems[j].galaxy_pos);
                best[j] = best[j].min(d);
            }
        }
    }
    widest
}

/// Load where a charging warp is going while the drive spools: a jump
/// between stars spawns the system it lands in (planets, star, its stations
/// via world::home_base, its population on the warm list) once the charge is
/// a little way in; any warp, a dash too, has the ships near where it comes
/// out brought in (ai_ship::simulation treats that point as somewhere you
/// are). Called off, it's all taken away again.
#[allow(clippy::too_many_arguments)]
pub fn preload_warp_destination(
    mut commands: Commands,
    mut preload: ResMut<WarpPreload>,
    mut galaxy_map: ResMut<GalaxyMap>,
    mut streaming: ResMut<SystemStreamingManager>,
    mut sim: ResMut<crate::ai_ship::components::WorldSimulation>,
    ship: Query<(Option<&WarpCharging>, Option<&crate::ui::MapWarpCharging>), With<Ship>>,
    members: Query<(Entity, &StarSystemMember)>,
    spawn_assets: (Res<crate::vfx::procedural_textures::CelestialTextures>, Res<AssetServer>, Res<LootedPois>),
    time: Res<Time>,
    standing: Res<crate::diplomacy::FactionStanding>,
) {
    let (stars, dash) = ship.single().map_or((None, None), |(s, d)| (s, d));
    let now = time.elapsed_secs_f64();
    if let Some(charge) = stars {
        if preload.target != Some(charge.target) {
            call_off(&mut commands, &mut preload, &mut galaxy_map, &mut streaming, &members, now);
            let from = streaming.current_galaxy_pos;
            let (system, arrival) = plan_arrival(&galaxy_map, charge.target, |d| d.faction.is_some_and(|f| standing.attacks_player(f)), from);
            *preload = WarpPreload { target: Some(charge.target), system, arrival: Some(arrival), ..default() };
        }
        // A little way in, so a jump called off at once costs nothing.
        let due = charge.charge_timer.fraction() >= 0.12 || charge.charge_timer.duration().as_secs_f32() - charge.charge_timer.elapsed_secs() < 3.0;
        if let (Some(id), None, true) = (preload.system, preload.info.as_ref(), due) {
            if streaming.loaded_system != Some(id) {
                let was = galaxy_map.systems.iter().find(|s| s.id == id).map(|s| s.discovery);
                let (textures, assets, looted) = (&spawn_assets.0, &spawn_assets.1, &spawn_assets.2);
                preload.info = galaxy::load_system(&mut commands, assets, textures, &mut galaxy_map, id, now, looted);
                preload.was = was;
                crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy_map, id);
                if !streaming.warm_systems.contains(&id) {
                    streaming.warm_systems.push(id);
                    preload.warmed = true;
                }
                info!("WARP preload: system {id} loaded ahead");
            }
        }
        return;
    }
    if let Some(dash) = dash {
        if preload.arrival != Some(dash.target_pos) {
            *preload = WarpPreload { arrival: Some(dash.target_pos), ..default() };
        }
        return;
    }
    // Nothing charging. Jumped: the jump takes it. Called off: put it back.
    if !preload.jumping && preload.arrival.is_some() {
        call_off(&mut commands, &mut preload, &mut galaxy_map, &mut streaming, &members, now);
    }
}

/// A preloaded destination taken away again: its contents despawned, the map
/// as it was, the warm list as it was.
fn call_off(
    commands: &mut Commands,
    preload: &mut WarpPreload,
    galaxy_map: &mut GalaxyMap,
    streaming: &mut SystemStreamingManager,
    members: &Query<(Entity, &StarSystemMember)>,
    now: f64,
) {
    if let (Some(id), Some(_)) = (preload.system, preload.info.as_ref()) {
        if streaming.loaded_system != Some(id) {
            galaxy::unload_system(commands, members, galaxy_map, id, now);
            if let (Some(def), Some(was)) = (galaxy_map.systems.iter_mut().find(|s| s.id == id), preload.was) {
                def.discovery = was;
            }
            if preload.warmed {
                streaming.warm_systems.retain(|w| *w != id);
            }
        }
    }
    *preload = WarpPreload::default();
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

    /// The first drive gets you everywhere, a hop at a time, in any galaxy;
    /// it doesn't get you across the galaxy in one; each refit goes further.
    #[test]
    fn the_drive_reaches_everywhere_by_hops_and_nowhere_in_one() {
        for seed in [42, 7, 1_234, 99_999, 5_148_093_764_381_399_055] {
            let map = galaxy::generate_galaxy_map(seed);
            let reach = drive_reach(0, &map);
            // Everyone reachable from Haven by hops of at most `reach`.
            let mut seen = vec![false; map.systems.len()];
            let mut todo = vec![0usize];
            seen[0] = true;
            while let Some(i) = todo.pop() {
                for j in 0..map.systems.len() {
                    if !seen[j] && map.systems[i].galaxy_pos.distance(map.systems[j].galaxy_pos) <= reach {
                        seen[j] = true;
                        todo.push(j);
                    }
                }
            }
            assert!(seen.iter().all(|s| *s), "seed {seed}: a system no hops reach");
            let furthest = map.systems.iter().map(|s| s.galaxy_pos.distance(map.systems[0].galaxy_pos)).fold(0.0, f32::max);
            assert!(reach < furthest * 0.6, "seed {seed}: one jump reaches {:.0}% of the way to the furthest", reach / furthest * 100.0);
            assert!(drive_reach(1, &map) >= reach && drive_reach(2, &map) > drive_reach(1, &map));
            assert_eq!(drive_reach(9, &map), drive_reach(2, &map));
        }
    }

    /// Into hostile space you come out at the edge, on your side, clear of
    /// the star, the planets and the stations; into friendly space, at a station.
    #[test]
    fn into_hostile_space_you_arrive_at_the_edge() {
        let map = galaxy::generate_galaxy_map(42);
        for def in map.systems.iter().filter(|s| s.faction.is_some()).take(12) {
            let from = def.galaxy_pos + Vec2::new(-900_000.0, 300_000.0);
            let (id, at) = plan_arrival(&map, GalaxyWarpTarget::System(def.id), |_| true, from);
            assert_eq!(id, Some(def.id));
            let roll = galaxy::system_roll(def);
            let planets = roll.planet_paths();
            let keep = crate::celestial::poi::KeepClear { center: def.local_center, star_radius: roll.star_radius(), planets: &planets };
            assert!(keep.is_clear(at), "{}: came out in something", def.name);
            for site in crate::world::home_base::station_sites(def) {
                assert!(at.distance(site.pos) > site.radius() * 1.9 + 20_000.0, "{}: came out by {}", def.name, site.name);
            }
            let side = (at - def.local_center).dot((from - def.galaxy_pos).normalize());
            assert!(side > 0.0, "{}: came out on the far side", def.name);
            let (_, friendly) = plan_arrival(&map, GalaxyWarpTarget::System(def.id), |_| false, from);
            let station = crate::world::home_base::station_sites(def)[0].pos;
            assert!(friendly.distance(station) < at.distance(station));
        }
    }

    /// Warping into a system has to leave you able to press F. These two
    /// constants live in different files, so nothing but this connects them.
    #[test]
    fn arrival_standoff_is_inside_docking_range() {
        // The farthest plan_arrival puts you: out along the lane, off to one side.
        assert!(
            (STATION_ARRIVAL_STANDOFF * 2.5).hypot(1_500.0) < crate::world::home_base::DOCK_MARGIN,
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
