//! Docking and casting off, shown rather than cut.
//!
//! F near a station used to put the ship straight on its berth. Now (unless
//! Settings > Docking says Instant) it is flown in the way a real ship comes
//! alongside -- the way crewed spacecraft dock, scaled to the game:
//!
//! 1. Brake. Traffic control calls a berth and the ship burns off whatever
//!    speed it had, turning toward its way in. It doesn't stop dead.
//! 2. Transit to the gate, a hold point out along the berth's lane: turn
//!    onto the course first, burn, coast, slow down (round the station if
//!    it was on the far side, never through it).
//! 3. Hold at the gate, stopped, and turn about: the berth is backed into,
//!    so the ship can leave nose first.
//! 4. Final approach on thrusters: backing in down the lane, slower and
//!    slower the nearer it gets, to a creep for the last stretch.
//! 5. Contact: the soft capture gives a little and settles.
//! 6. Hard dock: the clamps close, the walkway swings out, the pressure is
//!    evened out, and the hatch opens. Then the old docking step runs:
//!    berth, top-ups, the station's services.
//!
//! Launching runs it backwards: walkway in, clamps off, a gentle push away on
//! thrusters, then the main engines, and the ship is handed back to you
//! moving -- not dropped dead in space. F skips either at any point.
//!
//! The motion is simulated, not keyframed: a speed and a turn rate that
//! change only as fast as the ship could change them, so nothing jumps. The
//! ship is moved by setting its transform after physics, so nothing else has
//! to know a sequence is running.

use bevy::prelude::*;

use super::home_base::{berth_position, SystemStations};
use crate::components::{Ship, ShipPhysics, Velocity};
use crate::states::GameState;

/// How docking is shown (Settings > Docking): 0 Full, 1 Short, 2 Instant.
pub fn style(settings: &crate::ui::menu_buttons::GameSettings) -> u8 {
    settings.docking % 3
}

/// The hold point, this far out along the lane from the berth.
pub const GATE: f32 = 9_000.0;

/// How the ship flies a docking (Full). Short flies it twice as briskly.
#[derive(Clone, Copy, Debug)]
pub struct Handling {
    /// Burning off the speed it had when docking was called.
    pub brake: f32,
    /// Transit: top speed, speeding up, slowing down.
    pub cruise: f32,
    pub accel: f32,
    pub decel: f32,
    /// Turning: angular acceleration and top turn rate.
    pub turn_accel: f32,
    pub turn_max: f32,
    /// Final approach: top speed, how fast it changes, the creep at the end,
    /// and how speed falls off with the range left (speed = range x this).
    pub final_max: f32,
    pub final_accel: f32,
    pub creep: f32,
    pub final_k: f32,
    /// Seconds: at least this long calling the berth; held at the gate once
    /// turned about; contact settling; clamps; walkway; pressure.
    pub call: f32,
    pub hold: f32,
    pub contact: f32,
    pub clamps: f32,
    pub walkway: f32,
    pub pressure: f32,
    /// Leaving: the thruster push, then the main engines.
    pub push: f32,
    pub push_accel: f32,
    pub depart: f32,
    pub depart_accel: f32,
}

impl Handling {
    pub const FULL: Handling = Handling {
        brake: 3_000.0,
        cruise: 4_200.0,
        accel: 2_000.0,
        decel: 1_600.0,
        turn_accel: 1.3,
        turn_max: 0.9,
        final_max: 1_400.0,
        final_accel: 600.0,
        creep: 200.0,
        final_k: 0.6,
        call: 1.5,
        hold: 1.2,
        contact: 1.2,
        clamps: 0.6,
        walkway: 1.8,
        pressure: 1.4,
        push: 450.0,
        push_accel: 300.0,
        depart: 3_600.0,
        depart_accel: 1_600.0,
    };

    pub fn of(short: bool) -> Handling {
        let h = Self::FULL;
        if !short {
            return h;
        }
        let (fast, quick, brief) = (2.0, 1.6, 0.5);
        Handling {
            brake: h.brake * fast,
            cruise: h.cruise * fast,
            accel: h.accel * fast,
            decel: h.decel * fast,
            turn_accel: h.turn_accel * quick * quick,
            turn_max: h.turn_max * quick,
            final_max: h.final_max * fast,
            final_accel: h.final_accel * fast,
            creep: h.creep * fast,
            final_k: h.final_k * fast,
            call: h.call * brief,
            hold: h.hold * brief,
            contact: h.contact * brief,
            clamps: h.clamps * brief,
            walkway: h.walkway * brief,
            pressure: h.pressure * brief,
            push: h.push * fast,
            push_accel: h.push_accel * fast,
            depart: h.depart,
            depart_accel: h.depart_accel * fast,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leg {
    /// Traffic control calls the berth; the ship burns off its speed.
    Brake,
    /// Flown to the gate.
    Transit,
    /// Stopped at the gate, turning about.
    Hold,
    /// Backing in down the lane.
    Final,
    /// Soft capture: it gives a little and settles.
    Contact,
    /// Clamps, walkway, pressure.
    Berth,
    /// Casting off: walkway in, clamps off.
    CastOff,
    /// Pushed off, then away on the main engines.
    Leave,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub station: usize,
    /// A station that moves (a faction starport, riding its planet): the
    /// whole run is flown relative to it. Everything below is where it was
    /// when the run began (`origin`); each frame it is carried along by how
    /// far the station has gone since.
    pub anchor: Option<Entity>,
    pub origin: Vec2,
    /// Its name, for the traffic notes, where it isn't one of the system's
    /// stations.
    pub name: String,
    pub leg: Leg,
    /// Seconds into this leg, and into the whole run.
    pub t: f32,
    pub clock: f32,
    /// The ship as the sequence flies it.
    pub pos: Vec2,
    pub vel: Vec2,
    pub angle: f32,
    pub omega: f32,
    /// Speed along the way (transit, final, leaving).
    pub speed: f32,
    /// Picked the ship's own motion up yet (the first frame).
    pub started: bool,
    pub berth: Vec2,
    pub station_pos: Vec2,
    pub extent: f32,
    /// The way to the gate (a cubic) and its length table, set at transit.
    pub path: [Vec2; 4],
    pub lut: Vec<f32>,
    pub s: f32,
    /// Seconds turned about and holding at the gate.
    pub held: f32,
    /// Where the walkway meets the station, and the ship's near side.
    pub tip: Vec2,
    pub ship_edge: f32,
    pub short: bool,
    /// How far the walkway is out, 0-1; the main engines' throttle shown.
    pub walk: f32,
    pub throttle: f32,
    /// A traffic note to put up, taken by the system.
    pub note: Option<(&'static str, &'static str)>,
}

/// What a step of the sequence came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Going,
    /// Docked: on the berth, walkway out.
    Docked,
    /// Clear of the station: the ship is the player's again.
    Clear,
}

impl Run {
    fn new(station: usize, leg: Leg, at: Vec2, angle: f32, berth: Vec2, station_pos: Vec2, extent: f32, tip: Vec2, ship_edge: f32, short: bool) -> Self {
        Self {
            station,
            anchor: None,
            origin: Vec2::ZERO,
            name: String::new(),
            leg,
            t: 0.0,
            clock: 0.0,
            pos: at,
            vel: Vec2::ZERO,
            angle,
            omega: 0.0,
            speed: 0.0,
            started: false,
            berth,
            station_pos,
            extent,
            path: [at; 4],
            lut: Vec::new(),
            s: 0.0,
            held: 0.0,
            tip,
            ship_edge,
            short,
            walk: 0.0,
            throttle: 0.0,
            note: None,
        }
    }

    pub fn gate(&self) -> Vec2 {
        self.berth + Vec2::X * GATE
    }

    pub fn docking_in(&self) -> bool {
        !matches!(self.leg, Leg::CastOff | Leg::Leave)
    }

    fn to(&mut self, leg: Leg) {
        self.leg = leg;
        self.t = 0.0;
    }

    /// One step of `dt` seconds.
    pub fn advance(&mut self, dt: f32) -> Step {
        let h = Handling::of(self.short);
        self.t += dt;
        self.clock += dt;
        self.throttle = 0.0;
        match self.leg {
            Leg::Brake => {
                // Burn off the speed it came in with, turning toward the way in.
                let v = self.vel.length();
                self.vel = if v <= h.brake * dt { Vec2::ZERO } else { self.vel * (1.0 - h.brake * dt / v) };
                self.pos += self.vel * dt;
                let toward = self.gate() - self.pos;
                if toward.length() > 1.0 {
                    turn(&mut self.angle, &mut self.omega, toward.to_angle(), dt, &h);
                }
                if self.vel == Vec2::ZERO && self.t >= h.call {
                    if self.pos.distance(self.gate()) < 600.0 {
                        self.pos = self.gate();
                        self.to(Leg::Hold);
                        self.note = Some(("holding at the gate  -  turning about", "hands off  -  F skips"));
                    } else {
                        self.path = transit_path(self.pos, self.gate(), self.station_pos, self.extent);
                        self.lut = arc_table(&self.path);
                        self.s = 0.0;
                        self.speed = 0.0;
                        self.to(Leg::Transit);
                        self.note = Some(("cleared to the gate", "hands off  -  F skips"));
                    }
                }
            }
            Leg::Transit => {
                let total = *self.lut.last().unwrap_or(&0.0);
                let u = t_at(&self.lut, self.s);
                let tangent = bezier_tangent(&self.path, u);
                let heading = if tangent.length() > 1e-3 { tangent.to_angle() } else { self.angle };
                let err = turn(&mut self.angle, &mut self.omega, heading, dt, &h);
                // Turned onto the course before it burns: no crabbing along.
                let align = if err > 1.0 { 0.0 } else { err.cos().max(0.0).powi(6) };
                let left = (total - self.s).max(0.0);
                let want = (h.cruise * align).min((2.0 * h.decel * left).sqrt());
                let before = self.speed;
                self.speed = toward(self.speed, want, if want > self.speed { h.accel } else { h.decel }, dt);
                if self.speed > before + 1e-3 {
                    self.throttle = 1.0;
                }
                self.s = (self.s + self.speed * dt).min(total);
                self.pos = bezier(&self.path, t_at(&self.lut, self.s));
                if left - self.speed * dt <= 1.0 && self.speed < 120.0 {
                    self.pos = self.gate();
                    self.speed = 0.0;
                    self.to(Leg::Hold);
                    self.note = Some(("holding at the gate  -  turning about", "hands off  -  F skips"));
                }
            }
            Leg::Hold => {
                // Turned about to face out along the lane: backed in, it
                // leaves nose first.
                let err = turn(&mut self.angle, &mut self.omega, 0.0, dt, &h);
                if err < 0.002 && self.omega.abs() < 0.01 {
                    self.angle = 0.0;
                    self.omega = 0.0;
                    self.held += dt;
                    if self.held >= h.hold {
                        self.speed = 0.0;
                        self.to(Leg::Final);
                        self.note = Some(("cleared to approach  -  backing in", "hands off  -  F skips"));
                    }
                }
            }
            Leg::Final => {
                let left = (self.pos.x - self.berth.x).max(0.0);
                let want = (left * h.final_k).clamp(h.creep, h.final_max);
                self.speed = toward(self.speed, want, h.final_accel, dt);
                self.pos.y = self.berth.y;
                if left <= self.speed * dt {
                    self.pos = self.berth;
                    self.to(Leg::Contact);
                    self.note = Some(("contact  -  soft capture", ""));
                } else {
                    self.pos.x -= self.speed * dt;
                }
            }
            Leg::Contact => {
                // The capture ring gives under the ship's last creep and
                // springs it back: a small, damped sway onto the berth.
                let w = std::f32::consts::TAU / 0.9;
                let a = self.speed / w;
                let give = a * (w * self.t).sin() * (-3.5 * self.t).exp();
                self.pos = self.berth - Vec2::X * give;
                if self.t >= h.contact {
                    self.pos = self.berth;
                    self.speed = 0.0;
                    self.to(Leg::Berth);
                    self.note = Some(("hard dock  -  clamps on, walkway out", ""));
                }
            }
            Leg::Berth => {
                self.pos = self.berth;
                self.walk = ease((self.t - h.clamps) / h.walkway);
                let out = h.clamps + h.walkway;
                if self.t >= out && self.t - dt < out {
                    self.note = Some(("pressure equal  -  hatch open", ""));
                }
                if self.t >= out + h.pressure {
                    self.walk = 1.0;
                    return Step::Docked;
                }
            }
            Leg::CastOff => {
                self.pos = self.berth;
                self.walk = 1.0 - ease(self.t / h.walkway);
                if self.t >= h.walkway + h.clamps {
                    self.walk = 0.0;
                    self.speed = 0.0;
                    self.to(Leg::Leave);
                    self.note = Some(("clamps off  -  pushing clear", ""));
                }
            }
            Leg::Leave => {
                // A gentle push on thrusters until clear of the berth, then
                // the main engines.
                let out = self.pos.x - self.berth.x;
                let clear = out > 1_500.0;
                let (want, acc) = if clear { (h.depart, h.depart_accel) } else { (h.push, h.push_accel) };
                self.speed = toward(self.speed, want, acc, dt);
                if clear {
                    self.throttle = 1.0;
                }
                self.pos.x += self.speed * dt;
                self.pos.y = self.berth.y;
                self.angle = 0.0;
                if out >= GATE + 3_000.0 {
                    return Step::Clear;
                }
            }
        }
        Step::Going
    }
}

/// The sequence under way, if any; and the station the ship is moored at.
#[derive(Resource, Default)]
pub struct Docking {
    pub run: Option<Run>,
    /// Arrived: home_base::station_docking finishes docking at this station.
    pub arrived: Option<usize>,
    /// Moored here (set on docking, taken on launch).
    pub moored_at: Option<usize>,
    /// Arrived at a faction starport: diplomacy::stations finishes docking.
    pub arrived_starport: bool,
}

impl Docking {
    pub fn busy(&self) -> bool {
        self.run.is_some()
    }
}

/// A point on a cubic.
pub fn bezier(p: &[Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    p[0] * u * u * u + p[1] * 3.0 * u * u * t + p[2] * 3.0 * u * t * t + p[3] * t * t * t
}

/// The cubic's direction at `t`.
fn bezier_tangent(p: &[Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    (p[1] - p[0]) * 3.0 * u * u + (p[2] - p[1]) * 6.0 * u * t + (p[3] - p[2]) * 3.0 * t * t
}

const ARC_STEPS: usize = 96;

/// The cubic's length from its start at each of ARC_STEPS + 1 even steps.
fn arc_table(p: &[Vec2; 4]) -> Vec<f32> {
    let mut out = Vec::with_capacity(ARC_STEPS + 1);
    let mut sum = 0.0;
    let mut last = p[0];
    out.push(0.0);
    for i in 1..=ARC_STEPS {
        let at = bezier(p, i as f32 / ARC_STEPS as f32);
        sum += at.distance(last);
        last = at;
        out.push(sum);
    }
    out
}

/// The cubic's parameter at distance `s` along it.
fn t_at(lut: &[f32], s: f32) -> f32 {
    if lut.len() < 2 {
        return 1.0;
    }
    let i = lut.partition_point(|l| *l < s).clamp(1, lut.len() - 1);
    let (a, b) = (lut[i - 1], lut[i]);
    let f = if b > a { ((s - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
    ((i - 1) as f32 + f) / (lut.len() - 1) as f32
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// `v` toward `want`, by no more than `rate * dt`.
fn toward(v: f32, want: f32, rate: f32, dt: f32) -> f32 {
    v + (want - v).clamp(-rate * dt, rate * dt)
}

/// Turn toward `target` the way a ship does: the turn rate builds up and
/// eases off again, never more than it could. Returns how far off it still is.
fn turn(angle: &mut f32, omega: &mut f32, target: f32, dt: f32, h: &Handling) -> f32 {
    let diff = wrap(target - *angle);
    if diff.abs() < 0.002 && omega.abs() < 0.02 {
        *angle = target;
        *omega = 0.0;
        return 0.0;
    }
    // The fastest turn rate it can still stop from in time.
    let want = diff.signum() * (2.0 * h.turn_accel * diff.abs()).sqrt().min(h.turn_max);
    *omega = toward(*omega, want, h.turn_accel, dt);
    let step = *omega * dt;
    if step.abs() > diff.abs() && step.signum() == diff.signum() {
        *angle = target;
        *omega = 0.0;
    } else {
        *angle = wrap(*angle + step);
    }
    wrap(target - *angle).abs()
}

/// The way to the gate at `gate` (out along the lane from the berth) for a
/// station at `station` reaching `extent` from its centre: it arrives
/// heading in along the lane, and goes round the north or south side first
/// if the ship starts west of the station's open face.
pub fn transit_path(from: Vec2, gate: Vec2, station: Vec2, extent: f32) -> [Vec2; 4] {
    let berth = gate - Vec2::X * GATE;
    let outside = gate + Vec2::X * 7_000.0;
    if from.x > berth.x - 1_000.0 {
        // Already on the open side: swing out and come in along the lane.
        [from, from.lerp(outside, 0.5), outside, gate]
    } else {
        let side = if from.y >= station.y { 1.0 } else { -1.0 };
        let round = Vec2::new(berth.x + 4_000.0, station.y + side * (extent + 6_000.0));
        [from, round, outside + Vec2::Y * side * 3_000.0, gate]
    }
}

/// Start docking at `station`'s berth. Returns false if it's Instant.
#[allow(clippy::too_many_arguments)]
/// Fly the ship in to the berth at the end of a faction starport's east arm.
/// The starport rides its planet, so the run is flown relative to `anchor`
/// (the starport), from where it is now (`station_pos`). `tip` is how far
/// east of its middle the berth arm ends; `extent` how far all of it, gun
/// platforms included, reaches. False if docking is set to Instant.
#[allow(clippy::too_many_arguments)]
pub fn begin_docking_at(
    docking: &mut Docking,
    settings: &crate::ui::menu_buttons::GameSettings,
    anchor: Entity,
    name: String,
    station_pos: Vec2,
    tip: f32,
    extent: f32,
    from: Vec2,
    from_angle: f32,
    ship_radius: f32,
) -> bool {
    if style(settings) == 2 || docking.busy() {
        return false;
    }
    let berth = berth_position(station_pos, tip, ship_radius);
    let mut run = Run::new(usize::MAX, Leg::Brake, from, from_angle, berth, station_pos, extent, station_pos + Vec2::X * tip, ship_radius * 0.8, style(settings) == 1);
    run.anchor = Some(anchor);
    run.origin = station_pos;
    run.name = name;
    run.note = Some(("berth called  -  east arm", "hands off  -  F skips"));
    docking.run = Some(run);
    true
}

/// Whether the way in to a starport's berth is clear of the planet it sits
/// by: the lane east from the berth, and the swing round to it from `from`.
/// The starport sits on its planet's night side, so for part of every orbit
/// the planet is across the lane, and the ship would be flown through it.
pub fn starport_lane_clear(from: Vec2, station_pos: Vec2, tip: f32, extent: f32, ship_radius: f32, planets: &[(Vec2, f32)]) -> bool {
    let berth = berth_position(station_pos, tip, ship_radius);
    let gate = berth + Vec2::X * GATE;
    let path = transit_path(from, gate, station_pos, extent);
    let mut points: Vec<Vec2> = (0..=48).map(|i| bezier(&path, i as f32 / 48.0)).collect();
    points.extend((0..=24).map(|i| berth.lerp(gate + Vec2::X * 7_000.0, i as f32 / 24.0)));
    planets.iter().all(|(c, r)| points.iter().all(|p| p.distance(*c) > r + ship_radius + 1_500.0))
}

pub fn begin_docking(
    docking: &mut Docking,
    settings: &crate::ui::menu_buttons::GameSettings,
    station: usize,
    station_pos: Vec2,
    station_radius: f32,
    extent: f32,
    from: Vec2,
    from_angle: f32,
    ship_radius: f32,
) -> bool {
    if style(settings) == 2 || docking.busy() {
        return false;
    }
    let berth = berth_position(station_pos, station_radius, ship_radius);
    let mut run = Run::new(
        station,
        Leg::Brake,
        from,
        from_angle,
        berth,
        station_pos,
        extent,
        station_pos + Vec2::X * station_radius,
        ship_radius * 0.8,
        style(settings) == 1,
    );
    run.note = Some(("berth called  -  east street", "hands off  -  F skips"));
    docking.run = Some(run);
    true
}

/// The lane's lights and the player's walkway.
#[derive(Component)]
pub struct LaneLight {
    pub order: usize,
    /// Which side of the lane: +1 north, -1 south.
    pub side: f32,
}

#[derive(Component)]
pub struct PlayerWalkway;

/// The traffic note on screen.
#[derive(Component)]
pub struct TrafficNote;

/// Fly the ship along the sequence, after physics so nothing undoes it.
#[allow(clippy::too_many_arguments)]
pub fn run_docking(
    mut commands: Commands,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut docking: ResMut<Docking>,
    stations: Res<SystemStations>,
    art: Option<Res<super::raft_art::RaftArt>>,
    mut ship: Query<(&mut Transform, &mut Velocity, &mut ShipPhysics), With<Ship>>,
    mut lane: Query<(Entity, &LaneLight, &mut Sprite, &mut Transform), (Without<PlayerWalkway>, Without<Ship>)>,
    mut walkway: Query<(Entity, &mut Sprite, &mut Transform), (With<PlayerWalkway>, Without<Ship>, Without<LaneLight>)>,
    notes: Query<Entity, With<TrafficNote>>,
    anchors: Query<&Transform, (With<crate::diplomacy::stations::StationAnchor>, Without<Ship>, Without<LaneLight>, Without<PlayerWalkway>)>,
) {
    let Some(run) = docking.run.as_mut() else { return };
    let Ok((mut tf, mut vel, mut physics)) = ship.single_mut() else { return };
    // A starport: where it is now, against where it was when the run began.
    let anchored = run.anchor.map(|a| anchors.get(a).ok().map(|t| t.translation.truncate()));
    let shift = match anchored {
        Some(Some(now)) => now - run.origin,
        _ => Vec2::ZERO,
    };
    let site_name = match anchored {
        Some(Some(_)) => Some(run.name.clone()),
        Some(None) => None,
        None => stations.sites.iter().find(|s| s.index == run.station).map(|s| s.name.clone()),
    };
    let Some(site_name) = site_name else {
        // The station's gone (a jump mid-sequence): drop the sequence and
        // everything it put up, so nothing is left hanging on screen.
        docking.run = None;
        for (e, ..) in &lane {
            commands.entity(e).despawn();
        }
        for (e, ..) in &walkway {
            commands.entity(e).despawn();
        }
        for e in &notes {
            commands.entity(e).despawn();
        }
        return;
    };
    // The ship's own motion, picked up where it was.
    if !run.started {
        run.started = true;
        run.pos = tf.translation.truncate() - shift;
        run.angle = physics.rotation;
        run.vel = if run.docking_in() { vel.0 } else { Vec2::ZERO };
    }
    // F skips the rest.
    let skip = keyboard.just_pressed(KeyCode::KeyF) && run.clock > 0.1;
    let step = if skip {
        if run.docking_in() { Step::Docked } else { Step::Clear }
    } else {
        run.advance(time.delta_secs().min(0.1))
    };

    // The lane lights, for the way in.
    if lane.is_empty() && matches!(run.leg, Leg::Brake | Leg::Transit | Leg::Hold | Leg::Final) && step == Step::Going {
        if let Some(art) = &art {
            for i in 0..8 {
                let x = run.berth.x + GATE * (1.0 - i as f32 / 8.0) + shift.x;
                for side in [-1.0f32, 1.0] {
                    commands.spawn((
                        Sprite { image: art.glow.clone(), color: Color::srgba(1.0, 0.85, 0.45, 0.0), custom_size: Some(Vec2::splat(700.0)), ..default() },
                        Transform::from_xyz(x, run.berth.y + side * 2_200.0 + shift.y, 0.2),
                        LaneLight { order: i, side },
                    ));
                }
            }
        }
    }
    if walkway.is_empty() {
        if let Some(art) = &art {
            commands.spawn((
                Sprite { image: art.tube.clone(), custom_size: Some(Vec2::new(1.0, 260.0)), color: Color::NONE, ..default() },
                Transform::from_xyz(run.tip.x + shift.x, run.tip.y + shift.y, 0.25),
                PlayerWalkway,
            ));
        }
    }
    if let Some((line, keys)) = run.note.take() {
        for e in &notes {
            commands.entity(e).despawn();
        }
        spawn_note(&mut commands, &site_name, line, keys);
    }

    tf.translation.x = run.pos.x + shift.x;
    tf.translation.y = run.pos.y + shift.y;
    tf.rotation = Quat::from_rotation_z(run.angle);
    physics.rotation = run.angle;
    physics.angular_velocity = 0.0;
    // The main engines' glow while it burns.
    physics.throttle = run.throttle;
    vel.0 = Vec2::ZERO;

    // The lane lights chase in toward the berth while it comes in.
    let lit = matches!(run.leg, Leg::Brake | Leg::Transit | Leg::Hold | Leg::Final);
    for (_, light, mut sprite, mut ltf) in &mut lane {
        let on = if lit { (((run.clock * 6.0 - light.order as f32) * 0.7).sin() * 0.5 + 0.5).powi(2) } else { 0.0 };
        sprite.color = Color::srgba(1.0, 0.85, 0.45, 0.25 + on * 0.75);
        // Carried along with a station that moves.
        ltf.translation.x = run.berth.x + GATE * (1.0 - light.order as f32 / 8.0) + shift.x;
        ltf.translation.y = run.berth.y + light.side * 2_200.0 + shift.y;
    }
    // The walkway reaches from the berth tip to the ship's near side.
    let berth = run.berth;
    let reach = (berth.x - run.ship_edge - run.tip.x).max(1.0);
    for (_, mut sprite, mut wtf) in &mut walkway {
        let l = (reach * run.walk).max(1.0);
        sprite.custom_size = Some(Vec2::new(l, 260.0));
        sprite.color = if run.walk > 0.0 { Color::WHITE } else { Color::NONE };
        wtf.translation.x = run.tip.x + l * 0.5 + shift.x;
        wtf.translation.y = run.tip.y + shift.y;
    }

    if step == Step::Going {
        return;
    }
    let station = run.station;
    let speed = run.speed;
    if step == Step::Docked {
        // Put it on the berth exactly, walkway out.
        tf.translation.x = berth.x + shift.x;
        tf.translation.y = berth.y + shift.y;
        tf.rotation = Quat::IDENTITY;
        physics.rotation = 0.0;
        physics.throttle = 0.0;
        for (_, mut sprite, mut wtf) in &mut walkway {
            sprite.custom_size = Some(Vec2::new(reach, 260.0));
            sprite.color = Color::WHITE;
            wtf.translation.x = run.tip.x + reach * 0.5 + shift.x;
        }
        if run.anchor.is_some() {
            docking.arrived_starport = true;
        } else {
            docking.arrived = Some(station);
        }
    } else {
        // Clear: the walkway's gone, and the ship is handed back moving.
        for (e, ..) in &walkway {
            commands.entity(e).despawn();
        }
        vel.0 = Vec2::X * if skip { 0.0 } else { speed };
        physics.throttle = 0.0;
    }
    for (e, ..) in &lane {
        commands.entity(e).despawn();
    }
    for e in &notes {
        commands.entity(e).despawn();
    }
    docking.run = None;
}

fn spawn_note(commands: &mut Commands, station: &str, line: &str, keys: &str) {
    use crate::ui::theme::{Sheet, ThemeFonts};
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(110.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-150.0)),
                width: Val::Px(300.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Sheet::BG),
            BorderColor::all(Sheet::EDGE),
            GlobalZIndex(60),
            TrafficNote,
        ))
        .with_children(|n| {
            n.spawn((Text::new(format!("{} TRAFFIC", station.to_uppercase())), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(Sheet::TITLE)));
            n.spawn((Text::new(line), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.86, 0.88, 0.92))));
            if !keys.is_empty() {
                n.spawn((Text::new(keys), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(Sheet::LABEL)));
            }
        });
}

/// Moored: remember where, so launching knows to cast off. Every frame
/// docked until it knows: a new game opens docked a frame or two before the
/// station list is filled.
pub fn note_moored(
    mut docking: ResMut<Docking>,
    stations: Res<SystemStations>,
    ship: Query<&Transform, With<Ship>>,
) {
    if docking.moored_at.is_some() {
        return;
    }
    let Ok(tf) = ship.single() else { return };
    docking.moored_at = stations.nearest_in_range(tf.translation.truncate()).map(|s| s.index);
}

/// Launching: cast off and be drawn out, unless docking is Instant.
pub fn begin_cast_off(
    mut commands: Commands,
    mut docking: ResMut<Docking>,
    settings: Res<crate::ui::menu_buttons::GameSettings>,
    stations: Res<SystemStations>,
    ship: Query<(&Transform, Option<&crate::ship::collision::Collider>), With<Ship>>,
    walkways: Query<Entity, With<PlayerWalkway>>,
) {
    let Some(station) = docking.moored_at.take() else { return };
    if style(&settings) == 2 || docking.busy() {
        for e in &walkways {
            commands.entity(e).despawn();
        }
        return;
    }
    let Ok((tf, collider)) = ship.single() else { return };
    let Some(site) = stations.sites.iter().find(|s| s.index == station) else { return };
    let ship_radius = collider.map(|c| c.bound_center.length() + c.bound_radius).unwrap_or(1_200.0);
    let at = tf.translation.truncate();
    let mut run = Run::new(
        station,
        Leg::CastOff,
        at,
        0.0,
        at,
        site.pos,
        super::home_base::station_extent(site.index),
        site.pos + Vec2::X * site.radius(),
        ship_radius * 0.8,
        style(&settings) == 1,
    );
    run.walk = 1.0;
    run.note = Some(("hatch shut  -  walkway in", "F skips"));
    docking.run = Some(run);
}

pub struct DockingPlugin;

impl Plugin for DockingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Docking>()
            .add_systems(Update, note_moored.run_if(in_state(GameState::StationDocked)))
            .add_systems(OnEnter(GameState::Exploring), begin_cast_off)
            .add_systems(
                PostUpdate,
                run_docking
                    .before(bevy::transform::TransformSystems::Propagate)
                    // A starport has been moved with its planet first.
                    .after(crate::diplomacy::stations::hold_anchored)
                    .run_if(in_state(GameState::Exploring)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run at a station that moves is flown in the station's own frame:
    /// stepped the same as one at a station that stands still, and carried
    /// along by how far the station has gone. The berth stays the berth.
    #[test]
    fn a_run_at_a_moving_starport_ends_on_its_berth() {
        let tip = 14_000.0;
        let station = Vec2::new(200_000.0, -40_000.0);
        let mut docking = Docking::default();
        let settings = crate::ui::menu_buttons::GameSettings::default();
        let from = station + Vec2::new(30_000.0, 8_000.0);
        assert!(begin_docking_at(&mut docking, &settings, Entity::PLACEHOLDER, "Port".into(), station, tip, 22_000.0, from, 0.0, 1_200.0));
        let mut run = docking.run.clone().unwrap();
        assert_eq!(run.origin, station);
        run.started = true;
        // The station goes 150 u/s north-east the whole while.
        let v = Vec2::new(106.0, 106.0);
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        let mut step = Step::Going;
        let mut last = run.pos;
        while step == Step::Going && t < 600.0 {
            step = run.advance(dt);
            t += dt;
            let shift = v * t;
            let on_screen = run.pos + shift;
            // Never through the station's middle.
            assert!(on_screen.distance(station + shift) > tip * 0.5, "flown through the starport at {t:.1} s");
            assert!(run.pos.distance(last) < 200.0, "jumped {} in one frame", run.pos.distance(last));
            last = run.pos;
        }
        assert_eq!(step, Step::Docked, "never docked in {t:.0} s");
        assert_eq!(run.berth, berth_position(station, tip, 1_200.0));
        // Where it ends up on screen is the berth, carried along.
        assert!((run.pos - run.berth).length() < 1.0);
    }

    /// The lane is shut when the planet the starport sits by lies across it.
    #[test]
    fn a_planet_across_the_lane_shuts_it() {
        let station = Vec2::ZERO;
        let (tip, extent) = (14_000.0, 22_000.0);
        let from = Vec2::new(60_000.0, 5_000.0);
        // The planet behind the station: the lane east is clear.
        assert!(starport_lane_clear(from, station, tip, extent, 1_200.0, &[(Vec2::new(-60_000.0, 0.0), 30_000.0)]));
        // The planet east of it: the lane runs into it.
        assert!(!starport_lane_clear(from, station, tip, extent, 1_200.0, &[(Vec2::new(55_000.0, 0.0), 25_000.0)]));
    }

    const DT: f32 = 1.0 / 60.0;

    fn docking_run(from: Vec2, angle: f32, vel: Vec2, short: bool) -> Run {
        let station = Vec2::ZERO;
        let berth = Vec2::new(20_000.0, 0.0);
        let mut run = Run::new(0, Leg::Brake, from, angle, berth, station, 15_000.0, Vec2::new(14_000.0, 0.0), 1_000.0, short);
        run.started = true;
        run.vel = vel;
        run
    }

    /// Fly a whole run, checking every frame that the ship only moves and
    /// turns as fast as a ship could. Returns the seconds it took.
    fn fly(run: &mut Run, done: Step) -> f32 {
        let h = Handling::of(run.short);
        let top = h.cruise.max(h.depart).max(h.brake) * 1.05 + run.vel.length();
        let mut last = run.pos;
        let mut last_angle = run.angle;
        for frame in 0..60 * 120 {
            let step = run.advance(DT);
            let moved = run.pos.distance(last);
            assert!(moved <= top * DT + 1.0, "jumped {moved} in a frame on {:?} at {}", run.leg, run.pos);
            let turned = wrap(run.angle - last_angle).abs();
            assert!(turned <= h.turn_max * DT * 1.01 + 1e-4, "spun {turned} rad in a frame on {:?}", run.leg);
            assert!(run.pos.distance(run.station_pos) > run.extent * 0.5, "flew over the station at {}", run.pos);
            last = run.pos;
            last_angle = run.angle;
            if step != Step::Going {
                assert_eq!(step, done);
                return frame as f32 * DT;
            }
        }
        panic!("never finished: stuck on {:?} at {}", run.leg, run.pos);
    }

    #[test]
    fn a_full_docking_is_flown_in_not_teleported() {
        let mut run = docking_run(Vec2::new(45_000.0, 14_000.0), std::f32::consts::FRAC_PI_2, Vec2::new(-1_800.0, 400.0), false);
        let secs = fly(&mut run, Step::Docked);
        assert_eq!(run.pos, run.berth);
        assert_eq!(run.angle, 0.0, "it backs in, facing out along the lane");
        assert!((20.0..60.0).contains(&secs), "a full docking took {secs}s");
    }

    #[test]
    fn short_docking_is_quicker_but_still_flown() {
        let mut full = docking_run(Vec2::new(45_000.0, 14_000.0), 1.0, Vec2::ZERO, false);
        let mut short = docking_run(Vec2::new(45_000.0, 14_000.0), 1.0, Vec2::ZERO, true);
        let (f, s) = (fly(&mut full, Step::Docked), fly(&mut short, Step::Docked));
        assert!(s < f * 0.7, "short {s}s vs full {f}s");
        assert!(s > 6.0, "short is still a flight, not a cut: {s}s");
    }

    #[test]
    fn it_creeps_onto_the_berth() {
        let mut run = docking_run(Vec2::new(40_000.0, -6_000.0), 3.0, Vec2::ZERO, false);
        let h = Handling::FULL;
        for _ in 0..60 * 120 {
            run.advance(DT);
            if run.leg == Leg::Contact {
                assert!(run.speed <= h.creep + 1.0, "touched at {}", run.speed);
                return;
            }
        }
        panic!("never touched");
    }

    #[test]
    fn from_the_far_side_it_goes_round_not_through() {
        let mut run = docking_run(Vec2::new(-45_000.0, 8_000.0), 0.0, Vec2::ZERO, false);
        fly(&mut run, Step::Docked);
        let mut run = docking_run(Vec2::new(-40_000.0, -12_000.0), 2.0, Vec2::new(0.0, 900.0), false);
        fly(&mut run, Step::Docked);
    }

    #[test]
    fn the_way_to_the_gate_arrives_along_the_lane() {
        let gate = Vec2::new(29_000.0, 0.0);
        let p = transit_path(Vec2::new(60_000.0, 9_000.0), gate, Vec2::ZERO, 15_000.0);
        assert_eq!(bezier(&p, 1.0), gate);
        // Heading in toward the station at the end.
        assert!(bezier_tangent(&p, 1.0).x < 0.0);
    }

    #[test]
    fn casting_off_hands_the_ship_back_moving() {
        let berth = Vec2::new(20_000.0, 0.0);
        let mut run = Run::new(0, Leg::CastOff, berth, 0.0, berth, Vec2::ZERO, 15_000.0, Vec2::new(14_000.0, 0.0), 1_000.0, false);
        run.started = true;
        run.walk = 1.0;
        let secs = fly(&mut run, Step::Clear);
        assert!(run.speed > 1_000.0, "dropped dead at {}", run.speed);
        assert!((6.0..30.0).contains(&secs), "casting off took {secs}s");
    }

    #[test]
    fn arc_lengths_find_their_place_on_the_curve() {
        let p = [Vec2::ZERO, Vec2::new(1_000.0, 0.0), Vec2::new(2_000.0, 0.0), Vec2::new(3_000.0, 0.0)];
        let lut = arc_table(&p);
        assert!((lut.last().unwrap() - 3_000.0).abs() < 1.0);
        let mid = bezier(&p, t_at(&lut, 1_500.0));
        assert!((mid.x - 1_500.0).abs() < 20.0, "{mid}");
    }
}
