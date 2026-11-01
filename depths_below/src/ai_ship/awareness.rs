//! What an enemy ship knows about the fight it's in, and what it does about it.
//!
//! Four times a second every fighting hull is surveyed: its reach, its live
//! firepower, what it has lost, its shields, who it's shooting. Each AI ship
//! reads that picture against its own state -- who is hurting it, how fast it
//! is dying, whether its side is winning, whether missiles are coming -- and
//! settles on a `Tactic`, which the brain (who to shoot, whether to run), the
//! helm (what range to hold, how to weave) and the guns (what to aim at, when
//! to launch) all act on.
//!
//! Faction character stays: `temperament` is how much of this each one uses.
//! The Choir are ghosts and the Eternal never retreat. The weights in `Mind`
//! are what tools/combat-lab/train.py tunes against real fights.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::components::*;
use crate::combat::severance::SeveredBlock;
use crate::components::*;
use crate::events::AiShipDamaged;

const SURVEY_EVERY_S: f32 = 0.25;
/// How far a ship looks when it weighs up its side against theirs.
const FORCE_RADIUS: f32 = 48_000.0;
/// How long a hit stays on its mind, in seconds (the rate halves in about 0.7 of this).
const MEMORY_S: f32 = 8.0;
/// Seconds a ship sticks with a tactic before it will change its mind, so it
/// doesn't dither on the line between two.
const SETTLE_S: f32 = 3.0;
/// Reach this much longer than theirs is worth playing the range game for.
const REACH_EDGE: f32 = 1.12;
/// Seconds of hull left, at the rate it's going, before it will pull back to
/// recover.
const RECOVER_WHEN_DYING_IN: f32 = 120.0;
/// What's left of a ship, outgunned, before it decides to run.
const HURT_ENOUGH_TO_RUN: f32 = 0.75;
/// Guns left below which a hull counts as disarmed.
const DISARMED: f32 = 0.15;

/// One fighting hull, as anyone looking at it can tell.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hull {
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: Vec2,
    pub radius: f32,
    /// None for the player.
    pub faction: Option<AiShipType>,
    /// Its longest gun or beam.
    pub reach: f32,
    /// The range at which half its gun firepower can bear (`fighting_reach`).
    pub firing_reach: f32,
    pub missile_reach: f32,
    /// Damage a second its live guns and launchers can put out.
    pub firepower: f32,
    pub guns: u32,
    pub peak_guns: u32,
    pub blocks: u32,
    pub peak_blocks: u32,
    /// 0..1.
    pub shield: f32,
    /// Who it's shooting: the AI's chosen target, or whoever the player hit last.
    pub target: Option<Entity>,
    /// Blocks a second it's been losing.
    pub lost_rate: f32,
    pub fleet: Option<u32>,
}

impl Hull {
    pub fn guns_left(&self) -> f32 {
        if self.peak_guns == 0 { 1.0 } else { self.guns as f32 / self.peak_guns as f32 }
    }
    pub fn blocks_left(&self) -> f32 {
        if self.peak_blocks == 0 { 1.0 } else { self.blocks as f32 / self.peak_blocks as f32 }
    }
    /// Seconds until it's gone, at the rate it's going.
    pub fn time_to_die(&self) -> f32 {
        if self.lost_rate > 0.01 { self.blocks as f32 / self.lost_rate } else { f32::INFINITY }
    }
    /// Where it fights from: its main armament's reach, not its longest gun's.
    pub fn fighting(&self) -> f32 {
        if self.firing_reach > 0.0 { self.firing_reach } else { self.reach }
    }
    /// Gun range or, for a ship with nothing else, missile range.
    pub fn any_reach(&self) -> f32 {
        self.reach.max(self.missile_reach)
    }
    /// Whatever it can hit at all from where it is.
    pub fn disarmed(&self) -> bool {
        self.peak_guns > 0 && self.guns_left() <= DISARMED
    }
}

/// Every fighting hull, surveyed four times a second.
#[derive(Resource, Default)]
pub struct Battlefield {
    pub hulls: HashMap<Entity, Hull>,
    pub player: Option<Entity>,
    /// The target each fleet has called.
    pub fleet_calls: HashMap<u32, Entity>,
}

/// What a ship has settled on doing about the fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tactic {
    /// Not in a fight.
    #[default]
    None,
    /// An even match: hold its fighting range.
    Hold,
    /// They reach further: get inside their guns' advantage, fast.
    Close,
    /// It reaches further: stay where it can hit and they can't.
    Kite,
    /// Work round to their stern while a friend holds their eye.
    Flank,
    /// They're disarmed, broken or running: go in and finish it.
    Press,
    /// Shields gone and under the gun: open the range till they're back.
    Recover,
    /// Losing: get out, shooting as it goes.
    Withdraw,
}

/// One ship's reading of the fight.
#[derive(Component, Default, Debug)]
pub struct Awareness {
    pub tactic: Tactic,
    since: f32,
    pub why: &'static str,
    /// Who has been hurting it, with how much lately.
    pub hurt_by: Vec<(Entity, f32)>,
    /// Damage a second lately.
    pub hurt_rate: f32,
    /// Missiles flying at it, and seconds until the nearest arrives.
    pub inbound: u32,
    pub inbound_eta: f32,
    /// Its side's strength over theirs: 1 is even.
    pub balance: f32,
    /// Friends in the fight.
    pub peers: u32,
    /// Aim for the target's drive rather than its guns: it's running.
    pub aim_engines: bool,
}

/// How much of all this a faction's crews use.
#[derive(Clone, Copy, Debug)]
pub struct Temperament {
    /// The strength ratio under which it pulls out. 0 never does.
    pub withdraw: f32,
    /// 0 fights by reflex. 0.5 plays range and pulls back to recover. 0.6 flanks.
    pub cunning: f32,
    /// How hard it weaves under fire, 0..1.
    pub jink: f32,
    /// Will back off to outrange someone.
    pub kites: bool,
}

pub fn temperament(faction: AiShipType) -> Temperament {
    use AiShipType::*;
    let t = |withdraw, cunning, jink, kites| Temperament { withdraw, cunning, jink, kites };
    match faction {
        // Mercenaries: clever, quick, and not paid to die.
        GildedThrone => t(0.65, 1.0, 1.0, true),
        // A navy: disciplined, slow to give ground.
        TerranHegemony => t(0.3, 0.8, 0.5, true),
        // Never retreats. Too big to dance, not too proud to out-range you.
        EternalHegemony => t(0.0, 0.6, 0.2, true),
        Shepherd => t(0.0, 0.8, 0.1, false),
        // Ghosts. They come straight on.
        BrokenChoir => t(0.0, 0.0, 0.3, false),
        // Scrap crews: they rush, they swerve, they don't think about tomorrow.
        RecursiveKingdom => t(0.0, 0.3, 0.7, false),
        CorpseStars => t(0.3, 0.5, 0.4, false),
        // Fanatics.
        SynthesisCollective => t(0.0, 0.6, 0.5, false),
        // Not soldiers: they run first and think after.
        _ => t(1.0, 0.5, 0.6, false),
    }
}

/// The dials on all of it, tuned by tools/combat-lab/train.py. DEPTHS_AI_WEIGHTS
/// sets any of them for a run, "jink=1.2,focus=2".
#[derive(Resource, Clone, Copy, Debug)]
pub struct Mind {
    /// Scales how hard ships weave.
    pub jink: f32,
    /// What a fleet's called target is worth over any other.
    pub focus: f32,
    /// Scales when a ship decides it's beaten.
    pub withdraw: f32,
    /// Range kept beyond their reach when kiting, in units.
    pub kite_margin: f32,
    /// How far inside its reach a ship goes to close, as a fraction of reach.
    pub close_to: f32,
    /// Shield fraction under which a ship under the gun pulls back.
    pub recover_shield: f32,
    /// Seconds between the salvos every launcher holds for. 0 fires as ready.
    pub salvo_beat: f32,
    /// How hard it flanks, as a fraction of top speed.
    pub flank: f32,
}

impl Default for Mind {
    fn default() -> Self {
        let mut m = Self {
            jink: 0.6,
            focus: 2.0,
            withdraw: 1.0,
            kite_margin: 1_800.0,
            close_to: 0.55,
            recover_shield: 0.15,
            salvo_beat: 8.0,
            flank: 0.6,
        };
        if let Ok(spec) = std::env::var("DEPTHS_AI_WEIGHTS") {
            for pair in spec.split(',') {
                let Some((k, v)) = pair.split_once('=') else { continue };
                let Ok(v) = v.trim().parse::<f32>() else { continue };
                match k.trim() {
                    "jink" => m.jink = v,
                    "focus" => m.focus = v,
                    "withdraw" => m.withdraw = v,
                    "kite_margin" => m.kite_margin = v,
                    "close_to" => m.close_to = v,
                    "recover_shield" => m.recover_shield = v,
                    "salvo_beat" => m.salvo_beat = v,
                    "flank" => m.flank = v,
                    other => warn!("DEPTHS_AI_WEIGHTS: no dial called {other}"),
                }
            }
        }
        m
    }
}

/// The range at which half of a ship's firepower can bear, from `(range,
/// damage a second)` for each gun. A ship with one long gun and a battery of
/// short ones has to go in close to fight at all: Synthesis ships hold 18 km
/// disruptors and 7 km plasma casters, and held off at 13 km doing nothing.
pub fn fighting_reach(guns: &mut [(f32, f32)]) -> f32 {
    let total: f32 = guns.iter().map(|g| g.1).sum();
    guns.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut bearing = 0.0;
    for (range, power) in guns.iter() {
        bearing += power;
        if bearing >= total * 0.5 {
            return *range;
        }
    }
    0.0
}

/// How far most of its fire reaches: where half its guns bear or, for a ship
/// with no guns, its launchers.
fn main_battery(me: &Hull) -> f32 {
    if me.fighting() > 0.0 { me.fighting() } else { me.any_reach() }
}

/// What a target is worth shooting: the firepower killing it takes off the
/// board for the damage that costs. Whatever's in reach, hurting this ship,
/// or called by its fleet counts for more; a hull already disarmed counts for
/// little while something else is still shooting.
#[allow(clippy::too_many_arguments)]
pub fn target_worth(me: &Hull, them: &Hull, shooting_me: bool, called: bool, allies_on_it: u32, held: bool, mind: &Mind) -> f32 {
    let toughness = them.blocks as f32 * (1.0 + them.shield) + 30.0;
    let mut worth = (them.firepower + 1.0) / toughness;
    if them.disarmed() {
        worth *= 0.2;
    }
    // Measured against its main battery, not its longest gun: a ship with one
    // railgun and nine cannons locked onto a capital only the railgun reached,
    // and sat nearly silent with an escort in cannon range.
    let gap = (me.pos.distance(them.pos) - them.radius).max(1.0);
    let reach = main_battery(me).max(1.0);
    if gap > reach {
        worth *= (reach / gap).powi(2);
    }
    if shooting_me {
        worth *= 1.5;
    }
    if called {
        worth *= mind.focus;
    }
    worth *= 1.0 + 0.15 * allies_on_it as f32;
    if held {
        worth *= 1.4;
    }
    worth
}

/// Everything `choose_tactic` weighs that isn't on the two hulls.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub balance: f32,
    pub peers: u32,
    /// Seconds since anything hit it.
    pub quiet_for: f32,
}

/// What a ship does about the fight in front of it, and why.
pub fn choose_tactic(me: &Hull, them: &Hull, t: &Temperament, mind: &Mind, r: &Reading, current: Tactic) -> (Tactic, &'static str) {
    let withdraw_under = t.withdraw * mind.withdraw;
    if withdraw_under > 0.0 {
        if me.disarmed() {
            return (Tactic::Withdraw, "its guns are gone");
        }
        let losing_exchange = me.time_to_die() < 25.0 && me.time_to_die() < them.time_to_die() * 0.5;
        if losing_exchange {
            return (Tactic::Withdraw, "losing the exchange");
        }
        // Outgunned is a reason to leave once it has cost something real: a
        // ship that ran from a scratch never fired a shot, just shuttled in and
        // out of range.
        if r.balance < withdraw_under && me.blocks_left() < HURT_ENOUGH_TO_RUN {
            return (Tactic::Withdraw, "outgunned and hurt");
        }
        // Stays gone until the fight has really changed.
        if current == Tactic::Withdraw && (r.balance < withdraw_under * 1.5 || r.quiet_for < 8.0) {
            return (Tactic::Withdraw, "still getting clear");
        }
    }
    // Reflexes only: come straight on.
    if t.cunning <= 0.0 {
        return (Tactic::Hold, "doesn't think about it");
    }
    let broken = them.disarmed() || (them.blocks_left() < 0.35 && r.balance >= 1.0);
    if broken {
        return (Tactic::Press, "they're finished");
    }
    // Moving away isn't running: they have to be hurt and going.
    let running = r.balance >= 0.8
        && them.vel.dot((them.pos - me.pos).normalize_or_zero()) > 120.0
        && (them.guns_left() < 0.6 || them.blocks_left() < 0.7);
    if running {
        return (Tactic::Press, "they're running");
    }
    if t.cunning >= 0.5 {
        // Under the gun with nothing left to soak it: back off till the
        // shields are up, and let a friend take the heat if there is one.
        let under_the_gun = r.quiet_for < 3.0;
        let recovering = current == Tactic::Recover && me.shield < 0.6 && r.quiet_for < 12.0;
        // Pulling back costs its own guns, so only when it matters: a ship with
        // hours of hull left at this rate stays and shoots.
        let dying = me.time_to_die() < RECOVER_WHEN_DYING_IN;
        if dying && (recovering || (me.shield < mind.recover_shield && under_the_gun)) {
            return (Tactic::Recover, "shields down, pulling back");
        }
    }
    if them.fighting() > me.fighting() * REACH_EDGE {
        return (Tactic::Close, "they outrange it");
    }
    if t.kites && t.cunning >= 0.5 && me.fighting() > them.fighting() * REACH_EDGE {
        return (Tactic::Kite, "it outranges them");
    }
    // A friend holds their eye: go round the back.
    let their_eye_elsewhere = them.target.is_some() && r.peers > 0;
    if t.cunning >= 0.6 && their_eye_elsewhere {
        return (Tactic::Flank, "their eye is elsewhere");
    }
    (Tactic::Hold, "an even match")
}

/// The range a ship wants, centre to centre, on this tactic: `(hold, speed
/// closing, speed backing)`, the speeds as fractions of its top speed. `band`
/// is the range its faction would hold in an even fight, `fire_limit` the
/// furthest its guns still connect from.
pub fn plan_range(tactic: Tactic, band: f32, reach: f32, their_reach: f32, own_radius: f32, target_radius: f32, floor: f32, fire_limit: f32, mind: &Mind) -> (f32, f32, f32) {
    match tactic {
        Tactic::Kite => {
            let out_of_their_reach = their_reach + target_radius + own_radius * 0.5 + mind.kite_margin;
            (out_of_their_reach.min(fire_limit).max(floor), 0.5, 1.0)
        }
        Tactic::Close => (((reach * mind.close_to) + target_radius).max(floor), 1.0, 0.4),
        Tactic::Press => (((reach * mind.close_to * 0.7) + target_radius).max(floor), 1.0, 0.4),
        Tactic::Recover => {
            let out = their_reach + target_radius + own_radius + mind.kite_margin;
            (out.max(fire_limit).max(floor), 0.5, 1.0)
        }
        _ => (band.max(floor).min(fire_limit.max(floor)), 0.5, 0.4),
    }
}

/// The ship's sideways weave: full over/back on a slow square wave, its period
/// long enough for the helm to take it and shorter than a shell's flight at
/// range, so what was aimed where it was going misses. Harder under fire, and
/// harder still with missiles close.
pub fn jink(entity_bits: u64, now: f32, t: &Temperament, mind: &Mind, hurt_rate: f32, inbound_eta: f32) -> f32 {
    let period = 2.5 + (entity_bits % 17) as f32 / 17.0 * 2.5;
    let phase = (entity_bits % 101) as f32 / 101.0;
    let side = if ((now / period + phase).floor() as i64) & 1 == 0 { 1.0 } else { -1.0 };
    let urgency = 0.5 + if hurt_rate > 0.0 { 0.25 } else { 0.0 } + if inbound_eta < 8.0 { 0.5 } else { 0.0 };
    side * (t.jink * mind.jink * urgency).clamp(0.0, 0.9)
}

/// Which way round the target to work: the sign of the tangent that takes this
/// ship toward its stern quarter, 0 once it's there.
pub fn flank_side(me: &Hull, them: &Hull) -> f32 {
    let from_them = (me.pos - them.pos).normalize_or_zero();
    // How far off their nose it stands, signed: to their stern is ±150 deg.
    let off_nose = them.facing.angle_to(from_them);
    if off_nose.abs() > 150_f32.to_radians() {
        return 0.0;
    }
    // Keep going the way it's already off their nose: anticlockwise round
    // them for a positive angle.
    if off_nose >= 0.0 { 1.0 } else { -1.0 }
}

/// Where a ship running from a fight runs to: the nearest friend well out of
/// it, or just away.
pub fn retreat_point(me: &Hull, them: &Hull, friends: impl Iterator<Item = Vec2>) -> Vec2 {
    let away = (me.pos - them.pos).normalize_or(Vec2::X);
    let shelter = friends
        .filter(|f| (*f - me.pos).dot(away) > 0.0)
        .min_by(|a, b| a.distance(me.pos).total_cmp(&b.distance(me.pos)));
    match shelter {
        Some(friend) => friend + away * (me.radius * 2.0),
        None => me.pos + away * 24_000.0,
    }
}

/// Should this launcher let go now? Launchers across a fleet wait for the same
/// beat, so a salvo arrives together and swamps point defence instead of
/// feeding it one missile at a time.
pub fn salvo_open(now: f32, mind: &Mind) -> bool {
    mind.salvo_beat <= 0.0 || (now / mind.salvo_beat).fract() * mind.salvo_beat < 1.0
}

/// Survey every fighting hull and let each AI ship read the fight.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn survey_battlefield(
    time: Res<Time>,
    mut next: Local<f32>,
    mut field: ResMut<Battlefield>,
    mind: Res<Mind>,
    standing: Res<crate::diplomacy::FactionStanding>,
    mut hits: MessageReader<AiShipDamaged>,
    mut player_hit: Local<HashMap<Entity, f32>>,
    mut ai: Query<
        (
            Entity,
            &Transform,
            &Velocity,
            &AiShipType,
            &AiShipState,
            &AiShipBehavior,
            &AiShipTarget,
            &Children,
            Option<&crate::ship::collision::Collider>,
            Option<&crate::combat::shields::ShipShield>,
            Option<&super::fleet::FleetMember>,
            &mut Awareness,
        ),
        With<AiShip>,
    >,
    player: Query<
        (Entity, &Transform, &Velocity, &Children, Option<&crate::ship::collision::Collider>, Option<&crate::combat::shields::ShipShield>),
        With<Ship>,
    >,
    weapons: Query<
        (&Weapon, &Module, Option<&OwnedByAiShip>, Has<crate::combat::targeting::fire_groups::InterceptMode>),
        (Without<Engine>, Without<SeveredBlock>),
    >,
    missiles: Query<(&Transform, &crate::combat::new_projectiles::MissileProjectile)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();

    // Who's hurting whom, every frame so no hit is missed.
    let mut fresh: HashMap<Entity, Vec<(Option<Entity>, f32)>> = HashMap::default();
    let player_entity = player.single().ok().map(|p| p.0);
    for hit in hits.read() {
        fresh.entry(hit.target).or_default().push((hit.attacker, hit.amount));
        if hit.attacker.is_some() && hit.attacker == player_entity {
            *player_hit.entry(hit.target).or_insert(0.0) += hit.amount;
        }
    }
    for (.., mut aware) in ai.iter_mut() {
        let decay = (-dt / MEMORY_S).exp();
        aware.hurt_rate *= decay;
        for (_, amount) in aware.hurt_by.iter_mut() {
            *amount *= decay;
        }
        aware.hurt_by.retain(|(_, a)| *a > 1.0);
    }
    for (entity, .., mut aware) in ai.iter_mut() {
        let Some(hits) = fresh.get(&entity) else { continue };
        for (attacker, amount) in hits {
            aware.hurt_rate += amount / MEMORY_S;
            if let Some(attacker) = attacker {
                match aware.hurt_by.iter_mut().find(|(e, _)| e == attacker) {
                    Some(held) => held.1 += amount,
                    None => aware.hurt_by.push((*attacker, *amount)),
                }
            }
        }
    }
    for amount in player_hit.values_mut() {
        *amount *= (-dt / MEMORY_S).exp();
    }
    player_hit.retain(|_, a| *a > 1.0);

    if now < *next {
        return;
    }
    let step = SURVEY_EVERY_S.max(now - *next + SURVEY_EVERY_S);
    *next = now + SURVEY_EVERY_S;

    // Tally what every ship still has that shoots.
    #[derive(Default)]
    struct Guns {
        reach: f32,
        missile_reach: f32,
        firepower: f32,
        guns: u32,
        ranges: Vec<(f32, f32)>,
    }
    let mut tally: HashMap<Entity, Guns> = HashMap::default();
    for (weapon, module, owner, point_defence) in weapons.iter() {
        if point_defence || !module.is_active || module.health <= 0.0 {
            continue;
        }
        let Some(ship) = owner.map(|o| o.root).or(player_entity) else { continue };
        let g = tally.entry(ship).or_default();
        let missile = crate::building::is_launcher(module.module_type)
            || module.module_type == ModuleType::VlsSilo
            || crate::combat::missile_design::is_built_launcher(module.module_type);
        if missile {
            g.missile_reach = g.missile_reach.max(weapon.range);
        } else {
            g.reach = g.reach.max(weapon.range);
            g.ranges.push((weapon.range, weapon.damage * weapon.fire_rate.max(0.05)));
        }
        g.firepower += weapon.damage * weapon.fire_rate.max(0.05);
        g.guns += 1;
    }

    let previous = std::mem::take(&mut field.hulls);
    let mut take = |entity: Entity, mut hull: Hull, field: &mut Battlefield| {
        let mut g = tally.remove(&entity).unwrap_or_default();
        hull.reach = g.reach;
        hull.firing_reach = fighting_reach(&mut g.ranges);
        hull.missile_reach = g.missile_reach;
        hull.firepower = g.firepower;
        hull.guns = g.guns;
        let before = previous.get(&entity);
        hull.peak_guns = before.map_or(g.guns, |b| b.peak_guns.max(g.guns));
        hull.peak_blocks = before.map_or(hull.blocks, |b| b.peak_blocks.max(hull.blocks));
        let lost = before.map_or(0.0, |b| b.blocks.saturating_sub(hull.blocks) as f32) / step;
        let fade = (-step / MEMORY_S).exp();
        hull.lost_rate = before.map_or(0.0, |b| b.lost_rate) * fade + lost * (1.0 - fade);
        if hull.lost_rate < 0.02 {
            hull.lost_rate = 0.0;
        }
        field.hulls.insert(entity, hull);
    };
    let field = &mut *field;
    field.player = player_entity;
    if let Ok((entity, tf, vel, children, collider, shield)) = player.single() {
        let hull = Hull {
            pos: shield.map_or(tf.translation.truncate(), |s| s.world_center(tf)),
            vel: vel.0,
            facing: (tf.rotation * Vec3::X).truncate(),
            radius: collider.map_or(300.0, |c| c.bound_radius),
            faction: None,
            blocks: children.len() as u32,
            shield: shield.filter(|s| s.enabled && s.max > 0.0).map_or(0.0, |s| (s.current / s.max).clamp(0.0, 1.0)),
            target: player_hit.iter().max_by(|a, b| a.1.total_cmp(b.1)).map(|(e, _)| *e),
            ..default()
        };
        take(entity, hull, field);
    }
    for (entity, tf, vel, faction, state, _, target, children, collider, shield, member, _) in ai.iter() {
        if state.is_destroyed {
            continue;
        }
        let hull = Hull {
            pos: shield.map_or(tf.translation.truncate(), |s| s.world_center(tf)),
            vel: vel.0,
            facing: (tf.rotation * Vec3::X).truncate(),
            radius: collider.map_or(300.0, |c| c.bound_radius),
            faction: Some(*faction),
            blocks: children.len() as u32,
            shield: shield.filter(|s| s.max > 0.0).map_or(0.0, |s| (s.current / s.max).clamp(0.0, 1.0)),
            target: target.entity,
            fleet: member.map(|m| m.fleet),
            ..default()
        };
        take(entity, hull, field);
    }

    // Missiles in the air, by who they're after.
    let mut inbound: HashMap<Entity, (u32, f32)> = HashMap::default();
    for (tf, missile) in missiles.iter() {
        let Some(hull) = missile.target.and_then(|t| field.hulls.get(&t).map(|h| (t, h))) else { continue };
        let eta = (tf.translation.truncate().distance(hull.1.pos) - hull.1.radius).max(0.0) / 1_500.0;
        let held = inbound.entry(hull.0).or_insert((0, f32::INFINITY));
        held.0 += 1;
        held.1 = held.1.min(eta);
    }

    // Each fleet calls one target: the best one from where its centre stands.
    field.fleet_calls.clear();
    // (Who last shot it, and who it was called to fight.)
    type Grudge = (Option<Entity>, Option<Entity>);
    let grudge = |state: &AiShipState| -> Grudge {
        (state.last_attacker, state.alert_target.filter(|_| state.alert_timer > 0.0))
    };
    let is_foe = |faction: AiShipType, other: &Hull, entity: Entity, grudge: &Grudge| match other.faction {
        Some(theirs) => factions_hostile(faction, theirs),
        None => standing.attacks_player(faction) || grudge.0 == Some(entity) || grudge.1 == Some(entity),
    };
    let mut fleets: HashMap<u32, (Hull, AiShipType, Grudge)> = HashMap::default();
    for (entity, _, _, faction, state, ..) in ai.iter() {
        let Some(hull) = field.hulls.get(&entity).filter(|h| h.fleet.is_some() && !state.is_destroyed) else { continue };
        let lead = fleets.entry(hull.fleet.unwrap()).or_insert((*hull, *faction, grudge(state)));
        // Whoever has been shot at knows who's fighting.
        if state.last_attacker.is_some() && lead.2 .0.is_none() {
            lead.2 = grudge(state);
        }
    }
    for (fleet, (lead, faction, state)) in &fleets {
        let call = field
            .hulls
            .iter()
            .filter(|(e, h)| is_foe(*faction, h, **e, state) && h.pos.distance(lead.pos) < FORCE_RADIUS)
            .map(|(e, h)| (*e, target_worth(lead, h, false, false, 0, false, &mind)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((target, _)) = call {
            field.fleet_calls.insert(*fleet, target);
        }
    }

    // Now each ship reads the fight.
    for (entity, _, _, faction, state, behavior, target, _, _, _, _, mut aware) in ai.iter_mut() {
        let (count, eta) = inbound.get(&entity).copied().unwrap_or((0, f32::INFINITY));
        aware.inbound = count;
        aware.inbound_eta = eta;
        let Some(me) = field.hulls.get(&entity).copied() else {
            aware.tactic = Tactic::None;
            continue;
        };
        let mut ours = (me.firepower, me.blocks as f32 * (1.0 + me.shield));
        let mut theirs = (0.0, 0.0);
        let mut peers = 0;
        for (e, h) in field.hulls.iter() {
            if *e == entity || h.pos.distance(me.pos) > FORCE_RADIUS {
                continue;
            }
            if is_foe(*faction, h, *e, &grudge(state)) {
                theirs.0 += h.firepower;
                theirs.1 += h.blocks as f32 * (1.0 + h.shield);
            } else if h.faction.is_some() && h.firepower > 0.0 {
                ours.0 += h.firepower;
                ours.1 += h.blocks as f32 * (1.0 + h.shield);
                peers += 1;
            }
        }
        aware.peers = peers;
        aware.balance = if theirs.0 * theirs.1 > 0.0 {
            ((ours.0 * ours.1) / (theirs.0 * theirs.1)).sqrt()
        } else {
            f32::INFINITY
        };

        let fighting = *behavior == AiShipBehavior::Engaging
            || (*behavior == AiShipBehavior::Fleeing && aware.tactic == Tactic::Withdraw);
        let Some(them) = target.entity.and_then(|t| field.hulls.get(&t)).filter(|_| fighting) else {
            aware.tactic = Tactic::None;
            aware.aim_engines = false;
            continue;
        };

        let temper = temperament(*faction);
        let reading = Reading { balance: aware.balance, peers, quiet_for: state.last_hit_timer };
        let (choice, why) = choose_tactic(&me, them, &temper, &mind, &reading, aware.tactic);
        // Settled tactics stay a moment; running for its life doesn't wait.
        let changing = choice != aware.tactic;
        if changing && (now - aware.since >= SETTLE_S || aware.tactic == Tactic::None || choice == Tactic::Withdraw) {
            if crate::combat::gun_assembly::trace() {
                info!(
                    "AI MIND {:?} {:?}: {:?} -> {:?} ({}), balance {:.2}, reach {:.0} vs {:.0}, shields {:.2}, {:.1} blocks/s, {} missiles inbound",
                    entity, faction, aware.tactic, choice, why, aware.balance, me.reach, them.reach, me.shield, me.lost_rate, aware.inbound,
                );
            }
            aware.tactic = choice;
            aware.why = why;
            aware.since = now;
        }
        aware.aim_engines = aware.tactic == Tactic::Press
            && them.vel.dot((them.pos - me.pos).normalize_or_zero()) > 120.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hull(reach: f32) -> Hull {
        Hull {
            pos: Vec2::ZERO,
            facing: Vec2::X,
            radius: 500.0,
            faction: Some(AiShipType::GildedThrone),
            reach,
            firepower: 100.0,
            guns: 10,
            peak_guns: 10,
            blocks: 500,
            peak_blocks: 500,
            shield: 1.0,
            ..default()
        }
    }
    fn reading() -> Reading {
        Reading { balance: 1.0, peers: 0, quiet_for: 99.0 }
    }
    fn think(me: &Hull, them: &Hull, r: Reading) -> Tactic {
        choose_tactic(me, them, &temperament(AiShipType::GildedThrone), &Mind::default(), &r, Tactic::Hold).0
    }

    #[test]
    fn outranged_it_closes_and_outranging_it_kites() {
        let them = Hull { pos: Vec2::new(6000.0, 0.0), ..hull(7000.0) };
        assert_eq!(think(&hull(4000.0), &them, reading()), Tactic::Close);
        assert_eq!(think(&hull(10000.0), &them, reading()), Tactic::Kite);
        assert_eq!(think(&hull(7000.0), &them, reading()), Tactic::Hold);
    }

    #[test]
    fn a_disarmed_ship_runs_and_a_disarmed_enemy_gets_finished() {
        let me = Hull { guns: 1, ..hull(7000.0) };
        let them = Hull { pos: Vec2::new(6000.0, 0.0), ..hull(7000.0) };
        assert_eq!(think(&me, &them, reading()), Tactic::Withdraw);
        let helpless = Hull { guns: 0, ..them };
        assert_eq!(think(&hull(7000.0), &helpless, reading()), Tactic::Press);
    }

    #[test]
    fn it_runs_when_it_is_losing_the_exchange_not_just_when_hurt() {
        let them = Hull { pos: Vec2::new(6000.0, 0.0), ..hull(7000.0) };
        let dying = Hull { lost_rate: 40.0, ..hull(7000.0) };
        assert_eq!(think(&dying, &them, reading()), Tactic::Withdraw);
        // Dying as fast as what it's killing: it stays and trades.
        let trading = Hull { lost_rate: 40.0, ..them };
        assert_ne!(think(&dying, &trading, reading()), Tactic::Withdraw);
    }

    #[test]
    fn being_outgunned_only_counts_once_it_has_cost_something() {
        let them = Hull { pos: Vec2::new(6000.0, 0.0), ..hull(7000.0) };
        let r = Reading { balance: 0.3, ..reading() };
        assert_ne!(think(&hull(7000.0), &them, r), Tactic::Withdraw, "not hurt yet");
        let scratched = Hull { blocks: 495, lost_rate: 0.5, ..hull(7000.0) };
        assert_ne!(think(&scratched, &them, r), Tactic::Withdraw, "not from a scratch");
        let hurt = Hull { blocks: 300, ..hull(7000.0) };
        assert_eq!(think(&hurt, &them, r), Tactic::Withdraw);
    }

    #[test]
    fn shields_down_under_fire_it_pulls_back_until_they_are_up() {
        let them = Hull { pos: Vec2::new(6000.0, 0.0), ..hull(7000.0) };
        let exposed = Hull { shield: 0.05, lost_rate: 8.0, ..hull(7000.0) };
        let r = Reading { quiet_for: 1.0, ..reading() };
        assert_eq!(think(&exposed, &them, r), Tactic::Recover);
        let t = temperament(AiShipType::GildedThrone);
        let mid = Hull { shield: 0.4, lost_rate: 8.0, ..hull(7000.0) };
        assert_eq!(choose_tactic(&mid, &them, &t, &Mind::default(), &r, Tactic::Recover).0, Tactic::Recover, "not till they're back up");
        let up = Hull { shield: 0.8, lost_rate: 8.0, ..hull(7000.0) };
        // A big hull losing little stays and shoots.
        let barely = Hull { shield: 0.05, lost_rate: 0.5, ..hull(7000.0) };
        assert_ne!(think(&barely, &them, r), Tactic::Recover);
        assert_ne!(choose_tactic(&up, &them, &t, &Mind::default(), &r, Tactic::Recover).0, Tactic::Recover);
    }

    #[test]
    fn with_a_friend_holding_their_eye_it_flanks() {
        let them = Hull { pos: Vec2::new(6000.0, 0.0), target: Some(Entity::PLACEHOLDER), ..hull(7000.0) };
        assert_eq!(think(&hull(7000.0), &them, Reading { peers: 1, ..reading() }), Tactic::Flank);
        assert_eq!(think(&hull(7000.0), &them, Reading { peers: 0, ..reading() }), Tactic::Hold);
    }

    #[test]
    fn the_choir_do_not_think_and_the_eternal_do_not_run() {
        let them = Hull { pos: Vec2::new(6000.0, 0.0), ..hull(12000.0) };
        let me = Hull { guns: 0, lost_rate: 50.0, shield: 0.0, ..hull(4000.0) };
        let r = Reading { balance: 0.1, ..reading() };
        let choir = temperament(AiShipType::BrokenChoir);
        assert_eq!(choose_tactic(&me, &them, &choir, &Mind::default(), &r, Tactic::None).0, Tactic::Hold);
        let eternal = temperament(AiShipType::EternalHegemony);
        assert_ne!(choose_tactic(&me, &them, &eternal, &Mind::default(), &r, Tactic::None).0, Tactic::Withdraw);
    }

    #[test]
    fn range_plans_put_it_where_the_tactic_says() {
        let mind = Mind::default();
        // Kiting: past their reach, inside its own.
        let (kite, _, back) = plan_range(Tactic::Kite, 6000.0, 10000.0, 5000.0, 500.0, 500.0, 1600.0, 10000.0, &mind);
        assert!(kite > 5500.0 && kite < 10000.0, "{kite}");
        assert_eq!(back, 1.0);
        // Kiting never gives up its own shot.
        let (limited, ..) = plan_range(Tactic::Kite, 6000.0, 6000.0, 5900.0, 500.0, 500.0, 1600.0, 6200.0, &mind);
        assert!(limited <= 6200.0);
        let (close, come, _) = plan_range(Tactic::Close, 6000.0, 6000.0, 9000.0, 500.0, 500.0, 1600.0, 6200.0, &mind);
        assert!(close < 6000.0);
        assert_eq!(come, 1.0);
        let (press, ..) = plan_range(Tactic::Press, 6000.0, 6000.0, 6000.0, 500.0, 500.0, 1600.0, 6200.0, &mind);
        assert!(press < close);
        // Recovering goes out past their guns, even out of its own.
        let (recover, ..) = plan_range(Tactic::Recover, 6000.0, 6000.0, 8000.0, 500.0, 500.0, 1600.0, 6200.0, &mind);
        assert!(recover > 8500.0);
    }

    #[test]
    fn the_target_worth_shooting_is_the_one_that_hurts_for_the_least_killing() {
        let mind = Mind::default();
        let me = hull(7000.0);
        let at = Vec2::new(5000.0, 0.0);
        let glass_cannon = Hull { pos: at, firepower: 300.0, blocks: 200, peak_blocks: 200, ..hull(7000.0) };
        let brick = Hull { pos: at, firepower: 100.0, blocks: 2000, peak_blocks: 2000, ..hull(7000.0) };
        assert!(target_worth(&me, &glass_cannon, false, false, 0, false, &mind) > target_worth(&me, &brick, false, false, 0, false, &mind));
        // A hull with no guns left stops mattering.
        let toothless = Hull { guns: 0, ..glass_cannon };
        assert!(target_worth(&me, &toothless, false, false, 0, false, &mind) < target_worth(&me, &glass_cannon, false, false, 0, false, &mind));
        // The fleet's call outweighs a near tie.
        assert!(target_worth(&me, &brick, false, true, 0, false, &mind) > target_worth(&me, &brick, false, false, 0, false, &mind) * 1.5);
        // Out of reach is worth much less than in it.
        let far = Hull { pos: Vec2::new(20000.0, 0.0), ..glass_cannon };
        assert!(target_worth(&me, &far, false, false, 0, false, &mind) < target_worth(&me, &glass_cannon, false, false, 0, false, &mind) * 0.3);
    }

    #[test]
    fn it_shoots_what_its_main_battery_reaches() {
        let mind = Mind::default();
        // One long gun, a main battery a third as long.
        let me = Hull { firing_reach: 6000.0, ..hull(18000.0) };
        let near = Hull { pos: Vec2::new(5000.0, 0.0), firepower: 100.0, blocks: 500, peak_blocks: 500, ..hull(7000.0) };
        // Three times the prize, but only the long gun gets there.
        let far = Hull { pos: Vec2::new(15000.0, 0.0), firepower: 300.0, ..near };
        assert!(target_worth(&me, &near, false, false, 0, false, &mind) > target_worth(&me, &far, false, false, 0, false, &mind));
        // With nothing in battery range it still takes the best of what's there.
        let further = Hull { pos: Vec2::new(17000.0, 0.0), ..near };
        assert!(target_worth(&me, &far, false, false, 0, false, &mind) > target_worth(&me, &further, false, false, 0, false, &mind));
        // A missile boat counts its launchers: no guns doesn't mean no reach.
        let boat = Hull { reach: 0.0, firing_reach: 0.0, missile_reach: 20000.0, ..hull(0.0) };
        assert_eq!(main_battery(&boat), 20000.0);
        let in_reach = target_worth(&boat, &near, false, false, 0, false, &mind);
        assert!(in_reach > target_worth(&boat, &Hull { pos: Vec2::new(40000.0, 0.0), ..near }, false, false, 0, false, &mind));
    }

    #[test]
    fn the_weave_swaps_sides_and_is_harder_under_missiles() {
        let t = temperament(AiShipType::GildedThrone);
        let mind = Mind::default();
        let calm = jink(7, 0.0, &t, &mind, 0.0, f32::INFINITY);
        let hunted = jink(7, 0.0, &t, &mind, 10.0, 2.0);
        assert!(hunted.abs() > calm.abs());
        let mut sides = std::collections::HashSet::new();
        for s in 0..40 {
            sides.insert(jink(7, s as f32 * 0.5, &t, &mind, 0.0, f32::INFINITY) > 0.0);
        }
        assert_eq!(sides.len(), 2);
    }

    #[test]
    fn flanking_goes_the_short_way_to_their_stern_and_stops_there() {
        let them = hull(7000.0);
        let on_the_beam = Hull { pos: Vec2::new(0.0, 5000.0), ..hull(7000.0) };
        assert_eq!(flank_side(&on_the_beam, &them), 1.0);
        let other_beam = Hull { pos: Vec2::new(0.0, -5000.0), ..hull(7000.0) };
        assert_eq!(flank_side(&other_beam, &them), -1.0);
        let astern = Hull { pos: Vec2::new(-5000.0, 100.0), ..hull(7000.0) };
        assert_eq!(flank_side(&astern, &them), 0.0);
    }

    #[test]
    fn it_retreats_to_a_friend_on_the_way_out_never_past_the_enemy() {
        let them = Hull { pos: Vec2::new(5000.0, 0.0), ..hull(7000.0) };
        let me = hull(7000.0);
        let behind_me = Vec2::new(-3000.0, 0.0);
        let behind_them = Vec2::new(8000.0, 0.0);
        let to = retreat_point(&me, &them, [behind_them, behind_me].into_iter());
        assert!(to.x < -3000.0, "{to}");
        let alone = retreat_point(&me, &them, std::iter::empty());
        assert!(alone.x < -5000.0, "{alone}");
    }

    #[test]
    fn a_ship_fights_from_where_its_main_guns_reach() {
        // Mostly short plasma, a little long ion: it has to close.
        assert_eq!(fighting_reach(&mut [(18_000.0, 10.0), (7_500.0, 90.0)]), 7_500.0);
        // Mostly long guns, a few short: it can stand off.
        assert_eq!(fighting_reach(&mut [(23_000.0, 80.0), (7_500.0, 20.0)]), 23_000.0);
        assert_eq!(fighting_reach(&mut []), 0.0);
    }

    #[test]
    fn launchers_hold_for_the_beat() {
        let mind = Mind::default();
        assert!(salvo_open(16.2, &mind));
        assert!(!salvo_open(20.0, &mind));
        assert!(salvo_open(20.0, &Mind { salvo_beat: 0.0, ..mind }));
    }
}
