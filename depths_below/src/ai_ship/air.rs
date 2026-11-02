//! An enemy ship's air, tile by tile, and the crew breathing it.
//!
//! The same fluid as the player's (`ship::air`): every interior tile carries
//! its own pressure, neighbours equalise, and holes pull to vacuum. So a hole
//! at the bow empties the bow first while the stern feeds it, and a capital
//! with one hole in it loses its air slowly -- a ship is nearly always one
//! big compartment (its hallways join everything), so venting a whole room
//! at once would have suffocated every crew on the first hit. Inner hull and
//! doors divide it, as on the player's ship.
//!
//! AI crew have no bodies (see `ai_ship::crew`), so each stands where its job
//! is: at the station it is manning, or else at a berth handed out the first
//! time the ship's air is laid down. A crewman whose tile runs thin holds his
//! breath, then dies -- and a ship that loses most of its crew that way
//! strikes its colours (`combat::check_ai_cripple`).

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use super::components::{AiShip, OwnedByAiShip};
use super::interior::Breaches;
use crate::components::{CrewMember, CrewStation, HullLayer, HullSegment, Module};
use crate::events::{CrewDamageSource, CrewDamaged};

/// How fast a tile beside a hole empties, per second (as `ship::air`).
const VENT_CONDUCTANCE: f32 = 0.9;
/// How fast neighbouring tiles equalise (as `ship::air`).
const DIFFUSE_RATE: f32 = 2.5;
/// The air is stepped at this fixed interval: one 2.5-rate exchange with each
/// of four neighbours stays stable below ~0.1 s.
const STEP: f32 = 0.05;
/// Air below this and a crewman is gasping.
pub const THIN_AIR: f32 = 0.3;
/// Seconds of emergency air, from full. Your crew have 90-second suits and
/// nine seconds took a whole enemy crew: one hole at the bow and a capital with
/// forty guns struck its colours a minute later, barely scratched.
const BREATH_SECS: f32 = 30.0;
/// Seconds to die once the breath is gone.
const SUFFOCATE_SECS: f32 = 4.0;
/// Breath back per second in good air, as a share of a full one.
const RECOVER_PER_S: f32 = 0.2;
/// Holes one crewman seals in a second: a big crew keeps up under fire, a
/// handful of survivors doesn't.
const SEAL_PER_CREW_S: f32 = 1.0 / 60.0;
/// How fast life support brings a sealed ship's air back, per second.
const REFILL_PER_S: f32 = 0.05;

/// What an enemy's oxygen tank holds, as the player's does (`building::registry`).
const OXYGEN_TANK_AIR: f32 = 200.0;

/// One enemy ship's air, in its own grid cells.
#[derive(Component, Default)]
pub struct AiAir {
    /// 1.0 = full atmosphere, 0.0 = vacuum. Only tiles that hold air.
    pub pressure: HashMap<IVec2, f32>,
    /// Time not yet stepped.
    since: f32,
    /// Breaches its crew have patched. Still wounds, no longer leaks.
    pub sealed: HashSet<IVec2>,
    /// Work done toward the next patch.
    patching: f32,
    /// Oxygen tanks aboard: the block, the compartment it feeds, the air it holds.
    pub tanks: Vec<(Entity, Vec<IVec2>, f32)>,
}

/// Where an AI crewman is when he isn't manning anything.
#[derive(Component, Clone, Copy, Debug)]
pub struct AiCrewBerth(pub IVec2);

/// The crew a ship spawned with: what "most of its crew lost" is measured
/// against, since the dead stay aboard but count for nothing.
#[derive(Component, Clone, Copy, Debug)]
pub struct AiCrewComplement(pub u32);

/// Tiles that hold air: decking (hallways) and every block that encloses
/// breathable volume, less the doors that shut to divide it.
pub fn interior_cells(
    hull: impl Iterator<Item = (IVec2, HullLayer)>,
    modules: impl Iterator<Item = (bool, bool, Vec<IVec2>)>,
) -> HashSet<IVec2> {
    let mut inside: HashSet<IVec2> = HashSet::new();
    // Inner hull is what divides a ship, as on the player's (`building::rooms`):
    // without it every enemy was one room, and a single hole emptied it.
    let mut walls: HashSet<IVec2> = HashSet::new();
    for (cell, layer) in hull {
        match layer {
            HullLayer::Hallway => {
                inside.insert(cell);
            }
            HullLayer::Inner => {
                walls.insert(cell);
            }
            _ => {}
        }
    }
    let mut doors: HashSet<IVec2> = HashSet::new();
    for (holds_air, door, cells) in modules {
        if door {
            doors.extend(cells);
        } else if holds_air {
            inside.extend(cells);
        }
    }
    inside.retain(|c| !doors.contains(c) && !walls.contains(c));
    inside
}

/// The air connected to a block: from its own cell or the one beside it.
fn compartment_of(inside: &HashSet<IVec2>, cell: IVec2) -> Vec<IVec2> {
    const SIDES: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
    let Some(start) = std::iter::once(cell).chain(SIDES.iter().map(|s| cell + *s)).find(|c| inside.contains(c)) else {
        return Vec::new();
    };
    let mut seen: HashSet<IVec2> = [start].into_iter().collect();
    let mut stack = vec![start];
    while let Some(at) = stack.pop() {
        for side in SIDES {
            let next = at + side;
            if inside.contains(&next) && seen.insert(next) {
                stack.push(next);
            }
        }
    }
    seen.into_iter().collect()
}

/// One fixed step of the air: holes inside are vacuum, tiles beside a hole
/// vent through it, and neighbours equalise.
pub fn step_air(pressure: &mut HashMap<IVec2, f32>, holes: &HashSet<IVec2>, dt: f32) {
    const SIDES: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
    for (cell, p) in pressure.iter_mut() {
        if holes.contains(cell) {
            *p = 0.0;
            continue;
        }
        let open = SIDES.iter().filter(|s| holes.contains(&(*cell + **s))).count() as f32;
        if open > 0.0 {
            *p -= (*p * VENT_CONDUCTANCE * open * dt).min(*p);
        }
    }
    let prev = pressure.clone();
    for (cell, p) in prev.iter() {
        for side in SIDES {
            let Some(q) = prev.get(&(*cell + side)) else { continue };
            if p > q {
                let delta = (p - q) * DIFFUSE_RATE * dt;
                *pressure.get_mut(cell).unwrap() -= delta;
                *pressure.get_mut(&(*cell + side)).unwrap() += delta;
            }
        }
    }
    for p in pressure.values_mut() {
        *p = p.clamp(0.0, 1.0);
    }
}

/// Lays down a ship's air the first time it has a hull to hold it, and gives
/// each of its crew a berth somewhere inside.
pub fn fill_ai_air(
    mut commands: Commands,
    ships: Query<(Entity, &Children), (With<AiShip>, Without<AiAir>)>,
    hull: Query<&HullSegment>,
    modules: Query<&Module>,
    crew: Query<(), (With<CrewMember>, Without<AiCrewBerth>)>,
) {
    for (ship, children) in ships.iter() {
        let inside = interior_cells(
            children.iter().filter_map(|c| hull.get(c).ok()).map(|h| (h.grid_position, h.hull_layer)),
            children.iter().filter_map(|c| modules.get(c).ok()).map(|m| {
                let footprint = crate::building::footprints::footprint_override(m.module_type);
                (
                    m.module_type.holds_atmosphere(),
                    m.module_type.is_containment_door(),
                    crate::building::ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprint).to_vec(),
                )
            }),
        );
        if inside.is_empty() {
            continue; // its blocks aren't in yet
        }
        // Berths spread through the ship in a fixed order, so a ship is the
        // same from one run to the next.
        let mut tiles: Vec<IVec2> = inside.iter().copied().collect();
        tiles.sort_by_key(|c| (c.x, c.y));
        for (i, c) in children.iter().filter(|c| crew.contains(*c)).enumerate() {
            let at = tiles[(i * 7919) % tiles.len()];
            commands.entity(c).try_insert(AiCrewBerth(at));
        }
        let tanks = children
            .iter()
            .filter_map(|c| modules.get(c).ok().map(|m| (c, m)))
            .filter(|(_, m)| m.module_type == crate::components::ModuleType::OxygenTank)
            .map(|(c, m)| (c, compartment_of(&inside, m.grid_position), OXYGEN_TANK_AIR))
            .collect();
        commands.entity(ship).try_insert(AiAir { pressure: inside.into_iter().map(|c| (c, 1.0)).collect(), tanks, ..default() });
    }
}

/// Air out through the holes, and the crew breathing what is left.
pub fn breathe_ai_ships(
    time: Res<Time>,
    mut ships: Query<(Entity, &mut AiAir, &Breaches, &Children), With<AiShip>>,
    stations: Query<(&Module, &CrewStation)>,
    mut crew: Query<(Entity, &mut CrewMember, Option<&AiCrewBerth>), With<OwnedByAiShip>>,
    mut wounds: MessageWriter<CrewDamaged>,
    mut traced: Local<f32>,
    parts: Query<&Module>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // DEPTHS_AIR_TRACE=1: every 5 s, each holed ship's air and living crew.
    *traced += dt;
    let trace = *traced >= 5.0 && std::env::var("DEPTHS_AIR_TRACE").ok().as_deref() == Some("1");
    if *traced >= 5.0 {
        *traced = 0.0;
    }
    // Only ships with a hole in them: an intact one is full and stays full.
    for (ship, mut air, holes, children) in ships.iter_mut() {
        // Damage control: whoever is alive works through the holes, and once
        // the last is sealed life support brings the air back.
        let hands = children.iter().filter_map(|c| crew.get(c).ok()).filter(|(_, m, _)| m.health > 0.0).count();
        air.patching += hands as f32 * SEAL_PER_CREW_S * dt;
        let air = &mut *air;
        let unsealed: Vec<IVec2> = holes.0.difference(&air.sealed).copied().collect();
        for hole in unsealed.iter().take(air.patching as usize) {
            air.sealed.insert(*hole);
        }
        air.patching = if unsealed.is_empty() { 0.0 } else { air.patching.fract() };
        let open: HashSet<IVec2> = holes.0.difference(&air.sealed).copied().collect();

        air.since += dt;
        let mut steps = 0;
        while air.since >= STEP && steps < 10 {
            air.since -= STEP;
            steps += 1;
            if !open.is_empty() {
                step_air(&mut air.pressure, &open, STEP);
            }
        }
        air.since = air.since.min(STEP);
        let AiAir { pressure, tanks, .. } = air;
        for (block, tiles, stored) in tanks.iter_mut() {
            if parts.get(*block).is_ok_and(|m| m.health > 0.0) {
                crate::ship::air::feed_tiles(pressure, tiles, stored, dt);
            }
        }
        if open.is_empty() && hands > 0 {
            for p in air.pressure.values_mut() {
                *p = (*p + REFILL_PER_S * dt).min(1.0);
            }
        }

        // Who is where: on station if manning one, else in his berth.
        let posts: HashMap<Entity, IVec2> = children
            .iter()
            .filter_map(|c| stations.get(c).ok())
            .filter_map(|(m, s)| s.assigned_crew.map(|c| (c, m.grid_position)))
            .collect();
        for child in children.iter() {
            let Ok((entity, mut member, berth)) = crew.get_mut(child) else { continue };
            if member.health <= 0.0 {
                continue;
            }
            let Some(at) = posts.get(&entity).copied().or(berth.map(|b| b.0)) else { continue };
            let pressure = if open.contains(&at) { 0.0 } else { air.pressure.get(&at).copied().unwrap_or(1.0) };
            if pressure >= THIN_AIR {
                member.oxygen = (member.oxygen + 100.0 * RECOVER_PER_S * dt).min(100.0);
                continue;
            }
            if member.oxygen > 0.0 {
                member.oxygen = (member.oxygen - 100.0 / BREATH_SECS * dt).max(0.0);
                continue;
            }
            member.health = (member.health - member.max_health / SUFFOCATE_SECS * dt).max(0.0);
            if member.health <= 0.0 {
                wounds.write(CrewDamaged { crew: entity, amount: member.max_health, source: CrewDamageSource::Decompression });
            }
        }
        if trace {
            let mean = air.pressure.values().sum::<f32>() / air.pressure.len().max(1) as f32;
            let thin = air.pressure.values().filter(|p| **p < THIN_AIR).count();
            let alive = children.iter().filter_map(|c| crew.get(c).ok()).filter(|(_, m, _)| m.health > 0.0).count();
            info!("AIR {:?}: mean {:.2}, {} of {} tiles thin, {} holes, {} crew alive", ship, mean, thin, air.pressure.len(), holes.0.len(), alive);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corridor(n: i32) -> HashMap<IVec2, f32> {
        (0..n).map(|x| (IVec2::new(x, 0), 1.0)).collect()
    }

    fn run(pressure: &mut HashMap<IVec2, f32>, holes: &HashSet<IVec2>, secs: f32) {
        for _ in 0..(secs / STEP) as u32 {
            step_air(pressure, holes, STEP);
        }
    }

    /// `crew` hands aboard a corridor with one hole in it, after `secs`:
    /// was it sealed, and how much air is left?
    fn damage_control(crew: u32, secs: u32) -> (bool, f32) {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_message::<CrewDamaged>();
        app.add_systems(Update, breathe_ai_ships);
        let hole = IVec2::new(-1, 0);
        let ship = app.world_mut().spawn((
            AiShip,
            AiAir { pressure: corridor(30), ..default() },
            Breaches([hole].into_iter().collect()),
        )).id();
        for _ in 0..crew {
            app.world_mut().spawn((
                CrewMember { name: "Kade".into(), health: 100.0, max_health: 100.0, oxygen: 100.0, morale: 100.0, state: crate::components::CrewState::Idle },
                OwnedByAiShip { root: ship },
                AiCrewBerth(IVec2::new(29, 0)),
                ChildOf(ship),
            ));
        }
        for _ in 0..secs * 20 {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(50));
            app.update();
        }
        let air = app.world().get::<AiAir>(ship).unwrap();
        (air.sealed.contains(&hole), air.pressure.values().sum::<f32>() / 30.0)
    }

    #[test]
    fn a_crew_seals_the_hole_and_the_air_comes_back() {
        let (sealed, air) = damage_control(60, 25);
        assert!(sealed);
        assert!(air > 0.95, "life support brought it back: {air}");
    }

    #[test]
    fn a_handful_of_survivors_cannot_keep_up() {
        assert!(damage_control(60, 2).0, "sixty hands, a hole a second");
        assert!(!damage_control(2, 2).0, "two hands take half a minute over it");
    }

    #[test]
    fn a_hole_empties_the_near_end_first() {
        let mut p = corridor(30);
        let holes: HashSet<IVec2> = [IVec2::new(-1, 0)].into_iter().collect();
        run(&mut p, &holes, 5.0);
        assert!(p[&IVec2::new(0, 0)] < p[&IVec2::new(29, 0)]);
        // The rest of the corridor is still feeding it: one hole in a long
        // ship is slow.
        assert!(p.values().sum::<f32>() > 20.0);
    }

    #[test]
    fn a_hole_through_the_deck_drains_much_faster_than_one_in_the_side() {
        let side: HashSet<IVec2> = [IVec2::new(-1, 0)].into_iter().collect();
        let deck: HashSet<IVec2> = [IVec2::new(0, 0)].into_iter().collect();
        let (mut a, mut b) = (corridor(30), corridor(30));
        run(&mut a, &side, 10.0);
        run(&mut b, &deck, 10.0);
        assert!(b.values().sum::<f32>() < a.values().sum::<f32>());
    }

    #[test]
    fn a_tank_holds_the_air_up_until_it_runs_dry() {
        let holes: HashSet<IVec2> = [IVec2::new(-1, 0)].into_iter().collect();
        let mean = |p: &HashMap<IVec2, f32>| p.values().sum::<f32>() / p.len() as f32;
        let (mut plain, mut tanked) = (corridor(30), corridor(30));
        let tiles: Vec<IVec2> = tanked.keys().copied().collect();
        let mut stored = 200.0;
        for _ in 0..(60.0 / STEP) as u32 {
            step_air(&mut plain, &holes, STEP);
            step_air(&mut tanked, &holes, STEP);
            crate::ship::air::feed_tiles(&mut tanked, &tiles, &mut stored, STEP);
        }
        assert!(mean(&tanked) > mean(&plain) + 0.25, "{} against {}", mean(&tanked), mean(&plain));
        assert!(stored < 200.0);

        // Dry, it does nothing.
        let before = mean(&plain);
        let mut none = 0.0;
        crate::ship::air::feed_tiles(&mut plain, &tiles, &mut none, STEP);
        assert_eq!(mean(&plain), before);
    }

    #[test]
    fn a_ship_with_no_holes_keeps_its_air() {
        let mut p = corridor(10);
        run(&mut p, &HashSet::new(), 10.0);
        assert!(p.values().all(|v| (*v - 1.0).abs() < 1e-4));
    }

    #[test]
    fn a_closed_door_keeps_the_far_side() {
        // Decking 0..10 with a door at 5: the door cell holds no air, so the
        // two sides never exchange.
        let inside = interior_cells(
            (0..10).map(|x| (IVec2::new(x, 0), HullLayer::Hallway)),
            [(false, true, vec![IVec2::new(5, 0)])].into_iter(),
        );
        assert!(!inside.contains(&IVec2::new(5, 0)));
        let mut p: HashMap<IVec2, f32> = inside.into_iter().map(|c| (c, 1.0)).collect();
        let holes: HashSet<IVec2> = [IVec2::new(-1, 0)].into_iter().collect();
        run(&mut p, &holes, 60.0);
        assert!(p[&IVec2::new(0, 0)] < THIN_AIR);
        assert!((p[&IVec2::new(9, 0)] - 1.0).abs() < 1e-4);
    }
}
