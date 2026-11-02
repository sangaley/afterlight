//! Decoys and chaff against designed missiles (combat::missile_design).
//!
//! A ship with a live Decoy Launcher throws heat decoys, and one with a Chaff
//! Dispenser throws chaff, every few seconds while an enemy designed missile
//! is inbound. Each is thrown clear of the hull, off to the side the threat
//! is coming from, so a missile that takes it flies past the ship rather than
//! into it. Every locked seeker of the matching kind -- heat for a decoy,
//! radar for chaff -- that comes near one rolls once against it
//! (`MissileStats::fool_chance`); fooled, it chases the decoy and is lost
//! when the decoy burns out. A command link or a dumb nose has no seeker to
//! fool. Both blocks did nothing at all before this.
//!
//! Nothing here sees a missile on its own: a ship's Missile Warners spot
//! enemy missiles (`spot_missiles`), and its decoys and point defence guns
//! act only on what they've spotted, the way From the Depths needs munition
//! warners before its CIWS will engage.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::missile_design::MissileHead;
use super::missiles::BuiltMissile;
use crate::components::{DestroyedModule, Module, ModuleType, Ship, Velocity};
use crate::combat::new_projectiles::MissileProjectile;

/// Inbound missiles inside this range set the launchers off, and a decoy
/// fools a seeker inside this range of it.
pub const DECOY_RANGE: f32 = 3500.0 * super::gun_assembly::RANGE_SCALE;
/// Seconds between throws, per launcher.
pub const DECOY_INTERVAL: f32 = 4.0;
/// Seconds a decoy lasts.
pub const DECOY_LIFE: f32 = 4.0;

/// How far a Missile Warner sees an incoming missile: far enough to call one
/// fired from a sniper's range (5-8 km), so point defence turns onto it
/// and decoys go out in time. Was 4,500, inside the range ships now fight at.
pub const WARNER_RANGE: f32 = 8000.0 * super::gun_assembly::RANGE_SCALE;

/// On every ship: the enemy missiles its warners can see this frame, and
/// how many warners it has left to see them with.
#[derive(Component, Debug, Default, Clone)]
pub struct MissileWarning {
    pub spotted: Vec<Entity>,
    pub warners: u32,
}

impl MissileWarning {
    pub fn sees(&self, missile: Entity) -> bool {
        self.spotted.contains(&missile)
    }
}

/// Which enemy missiles each ship's warners see: those within
/// `WARNER_RANGE` of a live Missile Warner. A ship with none sees nothing.
#[allow(clippy::type_complexity)]
/// Which ship is on which side, for telling a threat from a friend's salvo.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Sides<'w, 's> {
    factions: Query<'w, 's, &'static crate::ai_ship::components::AiShipType>,
    escorts: Query<'w, 's, (), With<crate::ai_ship::standing_spawns::PlayerEscort>>,
    players: Query<'w, 's, (), With<Ship>>,
}

impl Sides<'_, '_> {
    /// Is a missile fired by `owner` something `me` should shoot down? Not its
    /// own, and not its side's: this only ever excluded the ship itself, so a
    /// fleet's point defence shot down the fleet's own salvo on the way out.
    pub fn threat(&self, me: Entity, owner: Option<Entity>) -> bool {
        let Some(owner) = owner else { return true };
        if owner == me {
            return false;
        }
        let side = |e: Entity| -> MissileSide {
            if self.players.contains(e) {
                MissileSide::You
            } else if self.escorts.contains(e) {
                MissileSide::YourEscort
            } else {
                self.factions.get(e).map_or(MissileSide::Unknown, |f| MissileSide::Faction(*f))
            }
        };
        side_threat(side(me), side(owner))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MissileSide {
    You,
    YourEscort,
    Faction(crate::ai_ship::components::AiShipType),
    /// Its ship has gone since it fired.
    Unknown,
}

/// Would a ship on side `me` shoot down a missile from side `owner`?
pub fn side_threat(me: MissileSide, owner: MissileSide) -> bool {
    use MissileSide::*;
    match (me, owner) {
        (You | YourEscort, You | YourEscort) => false,
        (Faction(a), Faction(b)) => crate::ai_ship::components::factions_hostile(a, b),
        _ => true,
    }
}

pub fn spot_missiles(
    mut commands: Commands,
    mut ships: Query<(Entity, Option<&Children>, Option<&mut MissileWarning>), Or<(With<Ship>, With<crate::ai_ship::components::AiShip>)>>,
    warners: Query<(&Module, &GlobalTransform), Without<DestroyedModule>>,
    missiles: Query<(Entity, &Transform, &MissileProjectile)>,
    sides: Sides,
) {
    for (ship, children, warning) in ships.iter_mut() {
        let eyes: Vec<Vec2> = children
            .into_iter()
            .flatten()
            .filter_map(|c| warners.get(*c).ok())
            .filter(|(m, _)| m.module_type == ModuleType::MissileWarner && m.health > 0.0)
            .map(|(_, gt)| gt.translation().truncate())
            .collect();
        let spotted = spotted_by(
            &eyes,
            missiles.iter().filter(|(_, _, m)| sides.threat(ship, m.owner_ship)).map(|(e, t, _)| (e, t.translation.truncate())),
        );
        let now = MissileWarning { spotted, warners: eyes.len() as u32 };
        match warning {
            Some(mut w) => *w = now,
            None => {
                commands.entity(ship).try_insert(now);
            }
        }
    }
}

/// Of these missiles, the ones within `WARNER_RANGE` of a warner at `eyes`.
pub fn spotted_by(eyes: &[Vec2], missiles: impl Iterator<Item = (Entity, Vec2)>) -> Vec<Entity> {
    if eyes.is_empty() {
        return Vec::new();
    }
    missiles
        .filter(|(_, p)| eyes.iter().any(|e| e.distance_squared(*p) < WARNER_RANGE * WARNER_RANGE))
        .map(|(e, _)| e)
        .collect()
}

#[derive(Component, Debug)]
pub struct MissileDecoy {
    /// A heat decoy (fools heat seekers); otherwise chaff (fools radar).
    pub heat: bool,
    pub life: f32,
    /// The ship that threw it: its own missiles ignore it.
    pub ship: Entity,
    /// Missiles that have already rolled against this one.
    pub rolled: Vec<Entity>,
}

fn fools(heat: bool, head: MissileHead) -> bool {
    (heat && head == MissileHead::Heat) || (!heat && head == MissileHead::Radar)
}

/// Throw decoys and chaff while enemy designed missiles the ship's warners
/// have spotted are inbound.
#[allow(clippy::type_complexity)]
pub fn throw_decoys(
    time: Res<Time>,
    mut commands: Commands,
    ships: Query<(Entity, &GlobalTransform, &Children, Option<&Velocity>, Option<&MissileWarning>), Or<(With<Ship>, With<crate::ai_ship::components::AiShip>)>>,
    launchers: Query<(Entity, &Module, &GlobalTransform), Without<DestroyedModule>>,
    missiles: Query<(Entity, &Transform, &MissileProjectile, &BuiltMissile)>,
    sides: Sides,
    mut ready_at: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    for (ship, ship_gt, children, velocity, warning) in ships.iter() {
        let Some(warning) = warning.filter(|w| !w.spotted.is_empty()) else { continue };
        let here = ship_gt.translation().truncate();
        for (launcher, module, gt) in children.iter().filter_map(|c| launchers.get(c).ok()) {
            let heat = match module.module_type {
                ModuleType::DecoyLauncher => true,
                ModuleType::ChaffDispenser => false,
                _ => continue,
            };
            if module.health <= 0.0 || ready_at.get(&launcher).is_some_and(|t| *t > now) {
                continue;
            }
            let inbound: Vec<Vec2> = missiles
                .iter()
                .filter(|(e, _, m, b)| sides.threat(ship, m.owner_ship) && fools(heat, b.head) && warning.sees(*e))
                .map(|(_, t, _, _)| t.translation.truncate())
                .filter(|p| p.distance(here) < DECOY_RANGE)
                .collect();
            if inbound.is_empty() {
                continue;
            }
            ready_at.insert(launcher, now + DECOY_INTERVAL);
            if super::gun_assembly::trace() {
                info!("DECOY thrown by {:?}: {}, {} inbound", ship, if heat { "heat decoy" } else { "chaff" }, inbound.len());
            }
            // Off to one side of the line the missiles are coming in on.
            let toward = (inbound.iter().sum::<Vec2>() / inbound.len() as f32 - here).normalize_or(Vec2::X);
            let side = if rand::random::<bool>() { 1.0 } else { -1.0 };
            let across = toward.perp() * side;
            let at = gt.translation().truncate() + toward * 400.0 + across * 900.0;
            let drift = velocity.map(|v| v.0).unwrap_or(Vec2::ZERO) + across * 180.0 - toward * 100.0;
            commands.spawn((
                Sprite {
                    color: if heat { Color::srgba(1.0, 0.75, 0.35, 0.95) } else { Color::srgba(0.7, 0.76, 0.86, 0.35) },
                    custom_size: Some(if heat { Vec2::splat(18.0) } else { Vec2::splat(160.0) }),
                    ..default()
                },
                Transform::from_translation(at.extend(0.4)),
                Velocity(drift),
                MissileDecoy { heat, life: DECOY_LIFE, ship, rolled: Vec::new() },
            ));
        }
    }
}

/// Every locked seeker of the matching kind near a decoy rolls once against it.
pub fn seduce_seekers(
    mut decoys: Query<(Entity, &Transform, &mut MissileDecoy)>,
    mut missiles: Query<(Entity, &Transform, &MissileProjectile, &mut BuiltMissile)>,
    sides: Sides,
) {
    for (decoy, decoy_tf, mut d) in decoys.iter_mut() {
        let at = decoy_tf.translation.truncate();
        for (m, tf, projectile, mut b) in missiles.iter_mut() {
            if b.decoy.is_some() || !b.locked || !fools(d.heat, b.head) || !sides.threat(d.ship, projectile.owner_ship) {
                continue;
            }
            if d.rolled.contains(&m) || tf.translation.truncate().distance(at) > DECOY_RANGE {
                continue;
            }
            d.rolled.push(m);
            if rand::random::<f32>() < b.stats.fool_chance {
                b.decoy = Some(decoy);
                if super::gun_assembly::trace() {
                    info!("MISSILE fooled by {}", if d.heat { "a heat decoy" } else { "chaff" });
                }
            }
        }
    }
}

/// Decoys drift, slow and burn out.
pub fn age_decoys(time: Res<Time>, mut commands: Commands, mut decoys: Query<(Entity, &mut Transform, &mut Velocity, &mut MissileDecoy, &mut Sprite)>) {
    let dt = time.delta_secs();
    for (e, mut tf, mut v, mut d, mut sprite) in decoys.iter_mut() {
        d.life -= dt;
        if d.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        tf.translation += (v.0 * dt).extend(0.0);
        v.0 *= (-0.6 * dt).exp();
        let a = (d.life / DECOY_LIFE).clamp(0.0, 1.0);
        sprite.color.set_alpha(if d.heat { 0.95 * a } else { 0.35 * a });
    }
}

/// How far out a point defence gun reaches for missiles.
pub const PD_RANGE: f32 = 4000.0 * super::gun_assembly::RANGE_SCALE;

/// How long a point defence round flies, at `speed`: to the edge of what it
/// engages. A miss used to fly half as far again, across the whole fight.
pub fn pd_lifetime(speed: f32) -> f32 {
    PD_RANGE / speed.max(1.0)
}

/// Where a round at `speed` from `from` meets something at `at` moving at
/// `vel` (relative to the shooter). None if it can't catch it.
pub fn intercept(from: Vec2, speed: f32, at: Vec2, vel: Vec2) -> Option<Vec2> {
    let d = at - from;
    let a = vel.length_squared() - speed * speed;
    let b = 2.0 * d.dot(vel);
    let c = d.length_squared();
    let t = if a.abs() < 1e-3 {
        if b >= 0.0 { return None; }
        -c / b
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let r = disc.sqrt();
        let (t1, t2) = ((-b - r) / (2.0 * a), (-b + r) / (2.0 * a));
        let t = [t1, t2].into_iter().filter(|t| *t > 0.0).fold(f32::INFINITY, f32::min);
        if !t.is_finite() {
            return None;
        }
        t
    };
    Some(at + vel * t)
}

/// What a point defence gun should shoot at: of the enemy missiles closing on
/// `ship_pos` within `reach` of the gun at `from`, the one that will get
/// there first; and where to aim (rounds at `speed`, the shooter moving at
/// `shooter_vel`) to meet it. `missiles`: (position, velocity).
pub fn pd_solution(
    ship_pos: Vec2,
    from: Vec2,
    speed: f32,
    shooter_vel: Vec2,
    reach: f32,
    missiles: impl Iterator<Item = (Vec2, Vec2)>,
) -> Option<Vec2> {
    missiles
        .filter(|(p, _)| p.distance(from) < reach)
        .filter_map(|(p, v)| {
            let to_ship = ship_pos - p;
            let closing = v.dot(to_ship.normalize_or_zero());
            (closing > 50.0).then(|| (to_ship.length() / closing, p, v))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .and_then(|(_, p, v)| intercept(from, speed, p, v - shooter_vel))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_fleet_does_not_shoot_down_its_own_salvo() {
        use crate::ai_ship::components::AiShipType::*;
        use MissileSide::*;
        assert!(!side_threat(Faction(TerranHegemony), Faction(TerranHegemony)), "same faction");
        assert!(side_threat(Faction(TerranHegemony), Faction(RecursiveKingdom)), "at war");
        assert!(side_threat(Faction(TerranHegemony), You));
        assert!(side_threat(You, Faction(TerranHegemony)));
        assert!(!side_threat(You, YourEscort), "your escort's missiles are yours");
        assert!(!side_threat(YourEscort, You));
        assert!(side_threat(Faction(TerranHegemony), Unknown), "fired by a ship that's gone");
    }

    use super::*;

    #[test]
    fn a_round_leads_a_crossing_missile() {
        // A missile 1,000 u off, crossing at 500 u/s; rounds at 2,500 u/s.
        let p = intercept(Vec2::ZERO, 2500.0, Vec2::new(1000.0, 0.0), Vec2::new(0.0, 500.0)).unwrap();
        assert!(p.y > 100.0 && p.y < 300.0, "{p:?}");
        let t = p.length() / 2500.0;
        assert!((p - (Vec2::new(1000.0, 0.0) + Vec2::new(0.0, 500.0) * t)).length() < 1.0);
    }

    #[test]
    fn point_defence_takes_the_missile_that_arrives_first() {
        let ship = Vec2::ZERO;
        // A slow near one and a fast farther one: the fast one lands first.
        let slow = (Vec2::new(800.0, 0.0), Vec2::new(-200.0, 0.0));
        let fast = (Vec2::new(0.0, 1500.0), Vec2::new(0.0, -1500.0));
        let going_away = (Vec2::new(-500.0, 0.0), Vec2::new(-900.0, 0.0));
        let aim = pd_solution(ship, ship, 3000.0, Vec2::ZERO, PD_RANGE, [slow, fast, going_away].into_iter()).unwrap();
        assert!(aim.x.abs() < 1.0 && aim.y > 0.0, "{aim:?}");
        assert!(pd_solution(ship, ship, 3000.0, Vec2::ZERO, PD_RANGE, [going_away].into_iter()).is_none());
    }

    #[test]
    fn only_a_warner_sees_a_missile_and_only_in_range() {
        let near = (Entity::from_raw_u32(1).unwrap(), Vec2::new(1000.0, 0.0));
        let far = (Entity::from_raw_u32(2).unwrap(), Vec2::new(WARNER_RANGE + 1500.0, 0.0));
        // No warner, nothing seen, however close.
        assert!(spotted_by(&[], [near, far].into_iter()).is_empty());
        let one = [Vec2::ZERO];
        assert_eq!(spotted_by(&one, [near, far].into_iter()), vec![near.0]);
        // A warner at the far end of the ship sees past the near one's reach.
        let two = [Vec2::ZERO, Vec2::new(2000.0, 0.0)];
        assert_eq!(spotted_by(&two, [near, far].into_iter()), vec![near.0, far.0]);
    }

    #[test]
    fn decoys_fool_heat_seekers_and_chaff_fools_radar() {
        assert!(fools(true, MissileHead::Heat));
        assert!(!fools(true, MissileHead::Radar));
        assert!(fools(false, MissileHead::Radar));
        assert!(!fools(false, MissileHead::Heat));
        // Nothing to fool on a command link or a dumb nose.
        for heat in [true, false] {
            assert!(!fools(heat, MissileHead::CommandLink));
            assert!(!fools(heat, MissileHead::DumbNose));
        }
    }
}
