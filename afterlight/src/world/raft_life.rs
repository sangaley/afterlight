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

use super::raft::{collision_circles, Mooring, RaftPlan, Tube, CELL};
use super::raft_art::{spawn_tube, BakeKey, RaftArt, RaftBakes, RaftHull};

/// On a walkway: the berths it joins.
#[derive(Component, Clone, Copy)]
pub struct TubeOf(pub [Option<usize>; 2]);

/// On a moored ship's picture and its lights: its berth.
#[derive(Component, Clone, Copy)]
pub struct ShipSlot(pub usize);

/// On the Raft's root: who is moored where, and when the next comes or goes.
#[derive(Component)]
pub struct RaftLive {
    pub plan: RaftPlan,
    /// How many ships the Raft keeps moored.
    pub target: usize,
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
    pub fn new(plan: RaftPlan, seed: u64) -> Self {
        let target = plan.moorings.iter().filter(|m| m.slot.is_some()).count();
        let mut rng = StdRng::seed_from_u64(seed ^ 0x11FE);
        let next_leave = rng.gen_range(20.0..45.0);
        Self { plan, target, busy: HashSet::new(), departing: Vec::new(), arriving: Vec::new(), next_leave, next_arrive: f32::INFINITY, rng }
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
}

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
            e.insert(Breath { phase: h * 10.0, speed: 0.25 + (h * 0.5 + 0.5) * 0.5 });
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

/// Ships come and go.
#[allow(clippy::too_many_arguments)]
pub fn raft_roster(
    mut commands: Commands,
    time: Res<Time>,
    art: Option<Res<RaftArt>>,
    mut bakes: ResMut<RaftBakes>,
    mut rafts: Query<(Entity, &mut RaftLive)>,
    ships: Query<(Entity, &ShipSlot, &ChildOf)>,
    tubes: Query<(Entity, &TubeOf, &ChildOf)>,
) {
    if art.is_none() {
        return;
    }
    let dt = time.delta_secs();
    for (root, mut live) in &mut rafts {
        let live = &mut *live;
        // AFTERLIGHT_RAFT_BUSY=1: comings and goings every few seconds, for filming.
        let busy_k = if std::env::var("AFTERLIGHT_RAFT_BUSY").is_ok_and(|v| v == "1") { 10.0 } else { 1.0 };
        live.next_leave -= dt * busy_k;
        live.next_arrive -= dt * busy_k;

        // Someone casts off.
        if live.next_leave <= 0.0 {
            live.next_leave = live.rng.gen_range(25.0..60.0);
            let free: Vec<usize> = live
                .plan
                .moorings
                .iter()
                .filter_map(|m| m.slot)
                .filter(|s| !live.busy.contains(s))
                .collect();
            if !free.is_empty() {
                let slot = free[live.rng.gen_range(0..free.len())];
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
                    // Its berth is empty once it's clear.
                    info!("Raft: the {} at berth {slot} casts off", m.design);
                    live.departing.push(m);
                    live.next_arrive = live.next_arrive.min(live.rng.gen_range(8.0..20.0));
                    set_collider(&mut commands, root, &live.plan);
                }
            }
        }

        // Someone is brought in to an empty berth.
        let count = live.plan.moorings.iter().filter(|m| m.slot.is_some()).count() + live.arriving.len();
        if live.next_arrive <= 0.0 {
            live.next_arrive = if count + 1 < live.target { live.rng.gen_range(10.0..25.0) } else { f32::INFINITY };
            let ready: Vec<usize> = (0..live.plan.spares.len())
                .filter(|i| {
                    let m = &live.plan.spares[*i];
                    m.slot.is_some_and(|s| !live.busy.contains(&s)) && bakes.done_get(&(m.design, m.quarter)).is_some()
                })
                .collect();
            if count < live.target + 1 && !ready.is_empty() {
                let i = ready[live.rng.gen_range(0..ready.len())];
                let m = live.plan.spares.remove(i);
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
            }
        }
    }
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
    mut breath: Query<(&Breath, &mut Sprite), (Without<Departing>, Without<Arriving>)>,
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

    for (b, mut sprite) in &mut breath {
        if sprite.color != Color::NONE {
            let a = 0.8 + 0.2 * (now * b.speed + b.phase).sin();
            sprite.color = sprite.color.with_alpha(a);
        }
    }
}
