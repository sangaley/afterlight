//! The Raft alive: ships cast off and leave, and newcomers are brought in by
//! tug to the empty berths; traffic runs up and down the streets, shuttles
//! ply between the hulk and the moored ships, and every ship's windows dim
//! and brighten on a beat of their own.
//!
//! The roster lives on the Raft's root (`RaftLive`); its collision follows
//! it, so a berth whose ship has gone really is empty.

use std::collections::HashSet;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::raft::{collision_circles, faction_of, library, refit, Mooring, RaftPlan, Tube, CELL, MOORED};
#[allow(unused_imports)]
use super::raft::design;
use crate::ai_ship::components::AiShipType;
use super::raft_art::{spawn_tube, BakeKey, RaftArt, RaftBakes, RaftHull};

/// On a walkway: the berths it joins.
#[derive(Component, Clone, Copy)]
pub struct TubeOf(pub [Option<usize>; 2]);

/// On a moored ship's picture and its lights: its berth.
#[derive(Component, Clone, Copy)]
pub struct ShipSlot(pub usize);

/// Haven's roster as it stands: (berth, design) for every ship moored or on
/// its way in. Saved with the game; a loaded one rebuilds Haven as it was.
#[derive(Resource, Default)]
pub struct HavenRoster {
    pub berths: Option<Vec<(usize, String)>>,
    /// Haven must be rebuilt from `berths` (a save was loaded, or a new run).
    pub reload: bool,
}

/// Every faction, for reading a design's folder back to who flies it.
const FACTIONS: [AiShipType; 12] = [
    AiShipType::StellarPreserve,
    AiShipType::SynthesisCollective,
    AiShipType::BrokenChoir,
    AiShipType::CorpseStars,
    AiShipType::TheSilence,
    AiShipType::TerranHegemony,
    AiShipType::GildedThrone,
    AiShipType::RecursiveKingdom,
    AiShipType::EternalHegemony,
    AiShipType::Shepherd,
    AiShipType::MerchantPrinces,
    AiShipType::FreeHaulersCompact,
];

/// The faction whose designs live in this folder.
pub fn faction_named(slug: &str) -> Option<AiShipType> {
    FACTIONS.into_iter().find(|f| crate::ai_ship::design_slug(*f) == slug)
}

/// On the Raft's root: who is moored where, and when the next comes or goes.
#[derive(Component)]
pub struct RaftLive {
    pub plan: RaftPlan,
    /// How many ships it starts with; it grows from there with the Cascade.
    pub base: usize,
    /// How many ships the Raft keeps moored.
    pub target: usize,
    /// Streets gone dark (the dark scene round Haven), outermost first.
    pub dark_streets: Vec<usize>,
    /// Bumped on every change to who is moored, or which streets are dark.
    pub version: u32,
    /// Berths with a ship on its way in or out.
    pub busy: HashSet<usize>,
    /// On their way out: their berths come free once they're gone.
    pub departing: Vec<Mooring>,
    /// On their way in: moored once they arrive.
    pub arriving: Vec<Mooring>,
    pub next_leave: f32,
    pub next_arrive: f32,
    pub rng: StdRng,
}

impl RaftLive {
    /// A Raft from its plan, or as a saved roster left it.
    pub fn new(mut plan: RaftPlan, seed: u64, roster: Option<&[(usize, String)]>) -> Self {
        let base = plan.moorings.iter().filter(|m| m.slot.is_some()).count();
        if let Some(roster) = roster {
            let lib = library();
            let mut berths: Vec<Mooring> = plan.moorings.iter().chain(&plan.spares).filter(|m| m.slot.is_some()).cloned().collect();
            berths.sort_by_key(|m| m.slot);
            plan.moorings.retain(|m| m.slot.is_none());
            plan.spares.clear();
            for mut m in berths {
                let saved = roster.iter().find(|(s, _)| Some(*s) == m.slot);
                match saved.and_then(|(_, d)| lib.get_key_value(d.as_str())) {
                    Some((key, _)) => {
                        m.design = key;
                        plan.moorings.push(m);
                    }
                    None => plan.spares.push(m),
                }
            }
            // Walkways for who's actually there.
            let moored: HashSet<usize> = plan.moorings.iter().filter_map(|m| m.slot).collect();
            // The hulk's own stay; ship-to-ship ones if both are here; the
            // street ones are laid again below for whoever is moored.
            plan.tubes.retain(|t| match t.ends {
                [None, None] => true,
                [Some(a), Some(b)] => moored.contains(&a) && moored.contains(&b),
                _ => false,
            });
            for m in plan.moorings.iter().filter(|m| m.slot.is_some()) {
                plan.tubes.push(Tube { a: m.at, b: m.dock, width: CELL * 1.8, ends: [m.slot, None] });
            }
            plan.circles = collision_circles(&plan);
        }
        let mut rng = StdRng::seed_from_u64(seed ^ 0x11FE);
        let next_leave = rng.gen_range(20.0..45.0);
        let target = plan.moorings.iter().filter(|m| m.slot.is_some()).count().max(base);
        Self {
            plan,
            base,
            target,
            dark_streets: Vec::new(),
            version: 1,
            busy: HashSet::new(),
            departing: Vec::new(),
            arriving: Vec::new(),
            next_leave,
            next_arrive: 5.0,
            rng,
        }
    }

    /// Ships moored or on their way in, by berth and design.
    pub fn roster(&self) -> Vec<(usize, String)> {
        self.plan.moorings.iter().chain(&self.arriving).filter_map(|m| Some((m.slot?, m.design.to_string()))).collect()
    }

    /// Every berth there is, moored or empty.
    pub fn berths(&self) -> usize {
        self.plan.moorings.iter().chain(&self.plan.spares).chain(&self.departing).chain(&self.arriving).filter(|m| m.slot.is_some()).count()
    }
}

/// Leaving: backing out of the berth, then away, fading.
#[derive(Component)]
pub struct Departing {
    pub t: f32,
    pub start: Vec2,
    pub back: Vec2,
    pub out: Vec2,
    pub slot: Option<usize>,
}

/// Arriving: brought in from outside to a berth.
#[derive(Component)]
pub struct Arriving {
    pub t: f32,
    pub from: Vec2,
    pub to: Vec2,
    pub slot: usize,
    /// The tug leading it (it leaves once the ship is in).
    pub tug: bool,
}

/// A light running up and down a street.
#[derive(Component)]
pub struct StreetMover {
    pub a: Vec2,
    pub b: Vec2,
    pub speed: f32,
    pub phase: f32,
}

/// A shuttle between the hulk and a moored ship.
#[derive(Component)]
pub struct Shuttle {
    pub from: Vec2,
    pub to: Vec2,
    pub t: f32,
    pub wait: f32,
}

/// On a ship's lights: windows dimming and brightening.
#[derive(Component)]
pub struct Breath {
    pub phase: f32,
    pub speed: f32,
    /// Its street, to go dark with it.
    pub street: Option<usize>,
}

/// A red emergency light on a ship in a dark street.
#[derive(Component)]
pub struct EmergencyLight(pub usize);

/// A quarter's name over its street.
#[derive(Component)]
pub struct QuarterLabel(pub usize);

const LEAVE_BACK_S: f32 = 4.0;
const LEAVE_S: f32 = 22.0;
const ARRIVE_S: f32 = 18.0;
const TUG: &str = "free_haulers_compact/tug";

/// A moored ship's two sprites (picture and lights), under `root`.
pub fn spawn_ship(commands: &mut Commands, root: Entity, m: &Mooring, bakes: &RaftBakes, seed: u64) -> [Entity; 2] {
    let key: BakeKey = (m.design, m.quarter);
    let z = 0.01 + m.level * 0.03;
    let mut out = [Entity::PLACEHOLDER; 2];
    for (i, lights) in [false, true].into_iter().enumerate() {
        let mut sprite = Sprite { color: Color::NONE, ..default() };
        if let Some((body, glow, size)) = bakes.done_get(&key) {
            sprite.image = if lights { glow.clone() } else { body.clone() };
            sprite.custom_size = Some(*size);
            sprite.color = Color::WHITE;
        }
        let mut e = commands.spawn((sprite, Transform::from_translation(m.at.extend(z + if lights { 0.001 } else { 0.0 })), RaftHull { key, lights }));
        if let Some(slot) = m.slot {
            e.insert(ShipSlot(slot));
        }
        if lights {
            let h = (m.at.x * 0.013 + m.at.y * 0.007 + seed as f32 * 0.001).sin();
            e.insert(Breath { phase: h * 10.0, speed: 0.25 + (h * 0.5 + 0.5) * 0.5, street: m.street });
        }
        out[i] = e.id();
    }
    commands.entity(root).add_children(&out);
    out
}

/// The street traffic and shuttles a Raft starts with.
pub fn spawn_traffic(commands: &mut Commands, root: Entity, plan: &RaftPlan, art: &RaftArt, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed ^ 0x7A4F);
    let mut kids = Vec::new();
    for (a, b) in &plan.streets {
        for _ in 0..rng.gen_range(3..7) {
            let warm = rng.gen_bool(0.7);
            kids.push(
                commands
                    .spawn((
                        Sprite {
                            image: art.glow.clone(),
                            color: if warm { Color::srgba(1.0, 0.82, 0.5, 0.95) } else { Color::srgba(0.85, 0.92, 1.0, 0.95) },
                            custom_size: Some(Vec2::splat(CELL * 4.0)),
                            ..default()
                        },
                        Transform::from_translation(a.extend(0.0035)),
                        StreetMover { a: *a, b: *b, speed: rng.gen_range(250.0..600.0), phase: rng.gen_range(0.0..2.0) },
                    ))
                    .id(),
            );
        }
    }
    let docks: Vec<Vec2> = plan.moorings.iter().filter(|m| m.slot.is_some()).map(|m| m.dock).collect();
    for _ in 0..10 {
        let to = if docks.is_empty() { Vec2::X * 5_000.0 } else { docks[rng.gen_range(0..docks.len())] };
        kids.push(
            commands
                .spawn((
                    Sprite { image: art.shuttle.clone(), custom_size: Some(Vec2::new(CELL * 7.0, CELL * 3.5)), ..default() },
                    Transform::from_xyz(0.0, 0.0, 0.06),
                    Shuttle { from: Vec2::new(rng.gen_range(-2_500.0..2_500.0), rng.gen_range(-2_000.0..2_000.0)), to, t: rng.gen_range(0.0..1.0), wait: 0.0 },
                ))
                .id(),
        );
    }
    commands.entity(root).add_children(&kids);
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ships come and go; the Raft grows with the Cascade, takes in the
/// refugees of the factions losing systems, and goes dark when the dark comes.
#[allow(clippy::too_many_arguments)]
pub fn raft_roster(
    mut commands: Commands,
    time: Res<Time>,
    art: Option<Res<RaftArt>>,
    mut bakes: ResMut<RaftBakes>,
    mut rafts: Query<(Entity, &mut RaftLive)>,
    ships: Query<(Entity, &ShipSlot, &ChildOf)>,
    tubes: Query<(Entity, &TubeOf, &ChildOf)>,
    story: (Option<Res<crate::narrative::CascadeState>>, Option<Res<crate::narrative::dark_scene::DarkView>>, Res<crate::celestial::resources::GalaxyMap>),
    mut roster: ResMut<HavenRoster>,
) {
    if art.is_none() {
        return;
    }
    let (cascade, dark_view, galaxy) = story;
    let level = cascade.as_ref().map_or(0.0, |c| c.level).clamp(0.0, 1.0);
    // AFTERLIGHT_RAFT_DARK=<0..1>: pin the dark round Haven, for filming.
    let dark = std::env::var("AFTERLIGHT_RAFT_DARK").ok().and_then(|v| v.parse::<f32>().ok())
        .unwrap_or_else(|| dark_view.as_ref().map_or(0.0, |d| d.level))
        .clamp(0.0, 1.0);
    // Who is losing: systems each faction held that the dark has (or is taking).
    let mut loss: std::collections::HashMap<&'static str, f32> = std::collections::HashMap::new();
    for sys in &galaxy.systems {
        if let (Some(f), true) = (sys.faction, sys.dark >= 0.5) {
            *loss.entry(crate::ai_ship::design_slug(f)).or_default() += 1.0;
        }
    }
    let dt = time.delta_secs();
    for (root, mut live) in &mut rafts {
        let live = &mut *live;
        // AFTERLIGHT_RAFT_BUSY=1: comings and goings every few seconds, for filming.
        let busy_k = if std::env::var("AFTERLIGHT_RAFT_BUSY").is_ok_and(|v| v == "1") { 10.0 } else { 1.0 };
        live.next_leave -= dt * busy_k;
        live.next_arrive -= dt * busy_k;

        // Bigger as the Cascade spreads: the refugees have nowhere else.
        let berths = live.berths();
        live.target = live.base + ((berths.saturating_sub(live.base)) as f32 * level).round() as usize;

        // Streets go dark with the scene round Haven, outermost first.
        let n_streets = live.plan.streets.len();
        let n_dark = ((dark * n_streets as f32 * 0.7).floor() as usize).min(n_streets);
        let mut order: Vec<usize> = (1..n_streets).collect();
        order.sort_by(|a, b| {
            let mid = |i: usize| (live.plan.streets[i].0 + live.plan.streets[i].1).length();
            mid(*b).total_cmp(&mid(*a))
        });
        let dark_now: Vec<usize> = order.into_iter().take(n_dark).collect();
        if dark_now != live.dark_streets {
            live.dark_streets = dark_now;
            live.version += 1;
        }

        // Someone casts off: in the dark, whoever is in a dark street first.
        if live.next_leave <= 0.0 {
            live.next_leave = live.rng.gen_range(25.0..60.0) * (1.0 + level) / (1.0 + 3.0 * dark);
            let free: Vec<(usize, bool)> = live
                .plan
                .moorings
                .iter()
                .filter_map(|m| Some((m.slot?, m.street.is_some_and(|s| live.dark_streets.contains(&s)))))
                .filter(|(s, _)| !live.busy.contains(s))
                .collect();
            let cut_loose: Vec<usize> = free.iter().filter(|(_, d)| *d).map(|(s, _)| *s).collect();
            let pool: Vec<usize> = if dark > 0.2 && !cut_loose.is_empty() { cut_loose } else { free.iter().map(|(s, _)| *s).collect() };
            if !pool.is_empty() {
                let slot = pool[live.rng.gen_range(0..pool.len())];
                if let Some(i) = live.plan.moorings.iter().position(|m| m.slot == Some(slot)) {
                    let m = live.plan.moorings.remove(i);
                    live.busy.insert(slot);
                    for (e, of, parent) in &tubes {
                        if parent.parent() == root && of.0.contains(&Some(slot)) {
                            commands.entity(e).despawn();
                        }
                    }
                    let back = (m.at - m.dock).normalize_or(Vec2::Y);
                    let out = m.at.normalize_or(Vec2::Y);
                    for (e, s, parent) in &ships {
                        if parent.parent() == root && s.0 == slot {
                            commands.entity(e).insert(Departing { t: 0.0, start: m.at, back, out, slot: Some(slot) });
                        }
                    }
                    info!("Raft: the {} at berth {slot} casts off", m.design);
                    live.departing.push(m);
                    live.next_arrive = live.next_arrive.min(live.rng.gen_range(8.0..20.0));
                    live.version += 1;
                    set_collider(&mut commands, root, &live.plan);
                }
            }
        }

        // Someone is brought in to an empty berth -- nobody comes into the dark.
        let count = live.plan.moorings.iter().filter(|m| m.slot.is_some()).count() + live.arriving.len();
        if live.next_arrive <= 0.0 {
            live.next_arrive = if dark >= 0.6 || count >= live.target {
                live.rng.gen_range(20.0..40.0)
            } else if count + 5 < live.target {
                live.rng.gen_range(6.0..14.0)
            } else {
                live.rng.gen_range(10.0..25.0)
            };
            if dark < 0.6 && count < live.target {
                if let Some((i, m)) = pick_refugee(live, &loss, &mut bakes) {
                    live.plan.spares.remove(i);
                    let slot = m.slot.unwrap_or(0);
                    live.busy.insert(slot);
                    let out = m.at.normalize_or(Vec2::Y);
                    let from = m.at + out * (live.plan.extent * 0.9 + 25_000.0);
                    let [a, b] = spawn_ship(&mut commands, root, &m, &bakes, slot as u64);
                    for e in [a, b] {
                        commands.entity(e).insert((Arriving { t: 0.0, from, to: m.at, slot, tug: false }, Transform::from_translation(from.extend(0.05))));
                    }
                    // A tug leads it in, just ahead of it.
                    let q = if out.x.abs() > out.y.abs() { 0 } else { 1 };
                    bakes.ask([(TUG, q)]);
                    if let Some((body, _, size)) = bakes.done_get(&(TUG, q)) {
                        let tug = commands
                            .spawn((
                                Sprite { image: body.clone(), custom_size: Some(*size), ..default() },
                                Transform::from_translation(from.extend(0.06)),
                                Arriving { t: 0.0, from: from - out * 3_500.0, to: m.at - out * 3_500.0, slot, tug: true },
                            ))
                            .id();
                        commands.entity(root).add_child(tug);
                    }
                    info!("Raft: a {} is brought in to berth {slot}", m.design);
                    live.arriving.push(m);
                    live.version += 1;
                }
            }
        }

        // Haven's roster, for the save.
        roster.berths = Some(live.roster());
    }
}

/// An empty berth for the next refugee, and who's in it. Most come from the
/// factions losing systems, refitted into a berth their ship fits, and moored
/// where their own people already are, so quarters form. Only ships whose
/// pictures are painted are brought in (the rest are asked for).
fn pick_refugee(live: &mut RaftLive, loss: &std::collections::HashMap<&'static str, f32>, bakes: &mut RaftBakes) -> Option<(usize, Mooring)> {
    // Whose ship it is.
    let factions: Vec<&'static str> = {
        let mut f: Vec<&'static str> = MOORED.iter().map(|d| faction_of(d)).collect();
        f.sort();
        f.dedup();
        f
    };
    let weights: Vec<f32> = factions.iter().map(|f| 1.0 + 3.0 * loss.get(f).copied().unwrap_or(0.0)).collect();
    let total: f32 = weights.iter().sum();
    let mut roll = live.rng.gen_range(0.0..total);
    let mut who = factions[0];
    for (f, w) in factions.iter().zip(&weights) {
        if roll < *w {
            who = f;
            break;
        }
        roll -= w;
    }
    // Their quarter: the street with most of their ships.
    let mut per_street = std::collections::HashMap::new();
    for m in &live.plan.moorings {
        if let (Some(st), true) = (m.street, faction_of(m.design) == who) {
            *per_street.entry(st).or_insert(0usize) += 1;
        }
    }
    let quarter = per_street.into_iter().max_by_key(|(_, n)| *n).map(|(s, _)| s);
    let theirs: Vec<&'static str> = MOORED.iter().copied().filter(|d| faction_of(d) == who).collect();
    // Every empty berth they fit, their quarter first.
    let mut best: Vec<(f32, usize, Mooring)> = Vec::new();
    for (i, spare) in live.plan.spares.iter().enumerate() {
        if spare.slot.is_none_or(|s| live.busy.contains(&s)) {
            continue;
        }
        let fitted = theirs.iter().filter_map(|d| refit(spare, d)).next().unwrap_or_else(|| spare.clone());
        let score = if spare.street == quarter && quarter.is_some() { 5.0 } else { 1.0 } * if faction_of(fitted.design) == who { 2.0 } else { 1.0 };
        best.push((score * live.rng.gen_range(0.5..1.0), i, fitted));
    }
    best.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, i, m) in best {
        bakes.ask([(m.design, m.quarter)]);
        if bakes.done_get(&(m.design, m.quarter)).is_some() {
            return Some((i, m));
        }
    }
    None
}

fn set_collider(commands: &mut Commands, root: Entity, plan: &RaftPlan) {
    let circles = collision_circles(plan);
    commands
        .entity(root)
        .insert((crate::ship::collision::Collider::circles(circles.clone(), f32::INFINITY), super::home_base::StationCircles(circles)));
}

/// Move what's leaving and arriving, the street lights and the shuttles,
/// and let the windows breathe.
#[allow(clippy::too_many_arguments)]
pub fn raft_motion(
    mut commands: Commands,
    time: Res<Time>,
    art: Option<Res<RaftArt>>,
    mut rafts: Query<&mut RaftLive>,
    mut leaving: Query<(Entity, &mut Departing, &mut Transform, &mut Sprite, &ChildOf), Without<Arriving>>,
    mut coming: Query<(Entity, &mut Arriving, &mut Transform, &mut Sprite, &ChildOf), Without<Departing>>,
    mut movers: Query<(&StreetMover, &mut Transform), (Without<Departing>, Without<Arriving>, Without<Shuttle>)>,
    mut shuttles: Query<(&mut Shuttle, &mut Transform, &ChildOf), (Without<Departing>, Without<Arriving>, Without<StreetMover>)>,
    mut breath: Query<(&Breath, &mut Sprite, &ChildOf), (Without<Departing>, Without<Arriving>)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();

    for (e, mut d, mut tf, mut sprite, parent) in &mut leaving {
        d.t += dt;
        let backed = ease(d.t / LEAVE_BACK_S);
        let away = (d.t - LEAVE_BACK_S).max(0.0);
        let pos = d.start + d.back * backed * CELL * 30.0 + d.out * away * away * 90.0;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        let fade = 1.0 - ((d.t - LEAVE_S * 0.55) / (LEAVE_S * 0.45)).clamp(0.0, 1.0);
        if sprite.color != Color::NONE {
            sprite.color = sprite.color.with_alpha(fade);
        }
        if d.t >= LEAVE_S {
            commands.entity(e).despawn();
            // Gone: its berth is free for someone else.
            if let (Some(slot), Ok(mut live)) = (d.slot, rafts.get_mut(parent.parent())) {
                if let Some(i) = live.departing.iter().position(|m| m.slot == Some(slot)) {
                    let m = live.departing.remove(i);
                    live.plan.spares.push(m);
                    live.busy.remove(&slot);
                    live.version += 1;
                }
            }
        }
    }

    for (e, mut a, mut tf, mut sprite, parent) in &mut coming {
        a.t += dt;
        let f = (a.t / ARRIVE_S).clamp(0.0, 1.0);
        // Slowing all the way in, like something being eased onto a berth.
        let k = 1.0 - (1.0 - f).powi(3);
        let pos = a.from.lerp(a.to, k);
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        if sprite.color != Color::NONE {
            sprite.color = sprite.color.with_alpha((a.t / 3.0).clamp(0.0, 1.0));
        }
        if f >= 1.0 {
            if a.tug {
                // The tug's job is done: it heads back out.
                let out = (a.from - a.to).normalize_or(Vec2::Y);
                commands.entity(e).remove::<Arriving>().insert(Departing { t: LEAVE_BACK_S, start: a.to, back: Vec2::ZERO, out, slot: None });
                continue;
            }
            commands.entity(e).remove::<Arriving>();
            // Tied up: its walkway goes out, and it counts as moored.
            if let Ok(mut live) = rafts.get_mut(parent.parent()) {
                let slot = a.slot;
                if let Some(i) = live.arriving.iter().position(|m| m.slot == Some(slot)) {
                    let m = live.arriving.remove(i);
                    live.busy.remove(&slot);
                    let tube = Tube { a: m.at, b: m.dock, width: CELL * 1.8, ends: [Some(slot), None] };
                    if let Some(art) = &art {
                        let t = spawn_tube(&mut commands, art, &tube);
                        commands.entity(parent.parent()).add_child(t);
                    }
                    info!("Raft: the {} is moored at berth {slot}", m.design);
                    live.plan.moorings.push(m);
                    live.version += 1;
                    set_collider(&mut commands, parent.parent(), &live.plan);
                }
            }
        }
    }

    for (m, mut tf) in &mut movers {
        let len = m.a.distance(m.b).max(1.0);
        let u = ((now * m.speed / len + m.phase).rem_euclid(2.0) - 1.0).abs();
        let p = m.a.lerp(m.b, u);
        tf.translation.x = p.x;
        tf.translation.y = p.y;
    }

    for (mut s, mut tf, parent) in &mut shuttles {
        if s.wait > 0.0 {
            s.wait -= dt;
            continue;
        }
        let len = s.from.distance(s.to).max(1.0);
        s.t += dt * 900.0 / len;
        let p = s.from.lerp(s.to, ease(s.t));
        tf.translation.x = p.x;
        tf.translation.y = p.y;
        tf.rotation = Quat::from_rotation_z((s.to - s.from).to_angle());
        if s.t >= 1.0 {
            // Turn round, and sometimes pick another ship to run to.
            s.t = 0.0;
            s.wait = 2.5;
            let back = s.from;
            s.from = s.to;
            s.to = back;
            if s.from.length() < 3_000.0 {
                if let Ok(mut live) = rafts.get_mut(parent.parent()) {
                    let docks: Vec<Vec2> = live.plan.moorings.iter().filter(|m| m.slot.is_some()).map(|m| m.dock).collect();
                    if !docks.is_empty() {
                        let i = live.rng.gen_range(0..docks.len());
                        s.to = docks[i];
                    }
                }
            }
        }
    }

    for (b, mut sprite, parent) in &mut breath {
        if sprite.color != Color::NONE {
            let dark = b.street.is_some_and(|s| rafts.get(parent.parent()).is_ok_and(|live| live.dark_streets.contains(&s)));
            // A dark street's windows are out but for a few, guttering.
            let a = if dark { 0.06 + 0.04 * (now * 3.1 + b.phase).sin().max(0.0) } else { 0.8 + 0.2 * (now * b.speed + b.phase).sin() };
            sprite.color = sprite.color.with_alpha(a);
        }
    }
}

/// Quarters named over their streets, and red emergency lights in the dark
/// ones; redone when who's moored or which streets are dark changes.
#[allow(clippy::too_many_arguments)]
pub fn raft_districts(
    mut commands: Commands,
    art: Option<Res<RaftArt>>,
    rafts: Query<(Entity, &RaftLive)>,
    mut labels: Query<(Entity, &QuarterLabel, &ChildOf, &mut Text2d, &mut Visibility)>,
    lights: Query<(Entity, &EmergencyLight, &ChildOf)>,
    mut seen: Local<std::collections::HashMap<Entity, u32>>,
) {
    let Some(art) = art else { return };
    for (root, live) in &rafts {
        if seen.get(&root) == Some(&live.version) {
            continue;
        }
        seen.insert(root, live.version);
        // Names.
        for (si, (a, b)) in live.plan.streets.iter().enumerate() {
            let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            let mut total = 0;
            for m in live.plan.moorings.iter().filter(|m| m.street == Some(si) && m.slot.is_some()) {
                *counts.entry(faction_of(m.design)).or_default() += 1;
                total += 1;
            }
            let top = counts.into_iter().max_by_key(|(_, n)| *n);
            let words = match top {
                Some((slug, n)) if n >= 4 && n * 2 >= total => faction_named(slug)
                    .map(|f| format!("{} QUARTER\n{n} ships", crate::ai_ship::components::faction_display_name(f).to_uppercase())),
                _ => None,
            };
            let words = if live.dark_streets.contains(&si) { words.map(|w| format!("{w}  -  DARK")) } else { words };
            let existing = labels.iter_mut().find(|(_, l, p, ..)| l.0 == si && p.parent() == root);
            match (existing, words) {
                (Some((_, _, _, mut text, mut vis)), Some(w)) => {
                    text.0 = w;
                    *vis = Visibility::Inherited;
                }
                (Some((_, _, _, _, mut vis)), None) => *vis = Visibility::Hidden,
                (None, Some(w)) => {
                    let mid = (*a + *b) * 0.5;
                    let label = commands
                        .spawn((
                            Text2d::new(w),
                            TextFont { font_size: FontSize::Px(18.0), ..default() },
                            TextColor(Color::srgba(0.62, 0.72, 0.92, 0.75)),
                            Transform::from_translation((mid + Vec2::new(0.0, CELL * 6.0)).extend(0.2)),
                            crate::camera::ZoomInvariantText,
                            QuarterLabel(si),
                        ))
                        .id();
                    commands.entity(root).add_child(label);
                }
                (None, None) => {}
            }
        }
        // Emergency red on every ship in a dark street.
        let want: std::collections::HashMap<usize, Vec2> = live
            .plan
            .moorings
            .iter()
            .filter(|m| m.street.is_some_and(|s| live.dark_streets.contains(&s)))
            .filter_map(|m| Some((m.slot?, m.at)))
            .collect();
        let mut have = HashSet::new();
        for (e, l, p) in &lights {
            if p.parent() != root {
                continue;
            }
            if want.contains_key(&l.0) {
                have.insert(l.0);
            } else {
                commands.entity(e).despawn();
            }
        }
        for (slot, at) in want {
            if have.contains(&slot) {
                continue;
            }
            let e = commands
                .spawn((
                    Sprite { image: art.glow.clone(), color: Color::srgb(1.0, 0.18, 0.12), custom_size: Some(Vec2::splat(CELL * 10.0)), ..default() },
                    Transform::from_translation(at.extend(0.07)),
                    super::home_base::StationBeacon::new(slot as f32 * 0.7, Color::srgb(1.0, 0.18, 0.12)),
                    EmergencyLight(slot),
                ))
                .id();
            commands.entity(root).add_child(e);
        }
    }
}

/// A new run starts from a fresh Haven, not the last run's.
pub fn forget_roster_on_new_run(mut ev: MessageReader<crate::events::NewExpeditionRequest>, mut roster: ResMut<HavenRoster>) {
    if ev.read().count() > 0 {
        roster.berths = None;
        roster.reload = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::raft::{outline, plan_raft};

    #[test]
    fn a_saved_roster_rebuilds_the_same_raft() {
        let first = RaftLive::new(plan_raft(12, 30), 12, None);
        // Swap a few ships about, as a run would.
        let mut roster = first.roster();
        roster.truncate(24);
        let spare = first.plan.spares.last().cloned().expect("an empty berth");
        roster.push((spare.slot.unwrap(), spare.design.to_string()));
        let again = RaftLive::new(plan_raft(12, 30), 12, Some(&roster));
        let mut a = again.roster();
        let mut b = roster.clone();
        a.sort();
        b.sort();
        assert_eq!(a, b);
        // Every moored ship has its walkway, and nothing sits on anything.
        for m in again.plan.moorings.iter().filter(|m| m.slot.is_some()) {
            assert!(again.plan.tubes.iter().any(|t| t.a == m.at && t.b == m.dock));
        }
        let all: Vec<Rect> = again.plan.moorings.iter().filter_map(|m| outline(m, -CELL * 0.25)).collect();
        for m in again.plan.moorings.iter().filter(|m| m.slot.is_some()) {
            let r = outline(m, -CELL * 0.25).unwrap();
            assert_eq!(all.iter().filter(|o| !o.intersect(r).is_empty()).count(), 1, "{} overlaps something", m.design);
        }
        // The empty berths are the rest.
        assert_eq!(again.berths(), first.berths());
    }

    #[test]
    fn every_moored_faction_folder_is_a_faction() {
        for d in MOORED {
            assert!(faction_named(faction_of(d)).is_some(), "{d}");
        }
    }
}
