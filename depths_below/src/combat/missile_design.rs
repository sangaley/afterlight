//! Missiles you design: a strip of parts, back to front.
//!
//! ```text
//! [motor][body][body]...[head]
//! ```
//!
//! - **The motor** is the back: a booster (big kick, short burn), a sustainer
//!   (gentle, long burn) or a vectoring motor (steers with its exhaust).
//! - **Body parts** go between: fuel, fins, warheads (HE, frag, shaped
//!   charge, EMP), a proximity fuse, signal processors, armour sheaths.
//! - **The head** is the front: a heat seeker, a radar seeker, a command link
//!   that flies where you aim, or a dumb nose.
//!
//! The launcher sets the size: a Launch Pad takes 120 mm missiles of up to 4
//! parts, a Rail Launcher 250 mm / 6, a VLS Silo 400 mm / 9. Mass, thrust,
//! warhead and toughness scale with `k = (gauge / 250 mm)²`.
//!
//! Every number here is the Missile Bay preview's (tools/missile-bay), so a
//! design behaves in the game the way it did on the range. Each launcher
//! carries its design (`MissileLoadout`); one nobody has designed for flies
//! the launcher's default.

use super::gun_assembly::RANGE_SCALE;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::ModuleType;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissileMotor {
    #[default]
    Booster,
    Sustainer,
    Vectoring,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissileBody {
    Fuel,
    Fins,
    He,
    Frag,
    ShapedCharge,
    Emp,
    ProximityFuse,
    SignalProcessor,
    ArmourSheath,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissileHead {
    #[default]
    Radar,
    Heat,
    CommandLink,
    DumbNose,
}

impl MissileMotor {
    pub const ALL: [MissileMotor; 3] = [Self::Booster, Self::Sustainer, Self::Vectoring];
    pub fn name(self) -> &'static str {
        match self {
            Self::Booster => "Booster",
            Self::Sustainer => "Sustainer",
            Self::Vectoring => "Vectoring motor",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Booster => "BO",
            Self::Sustainer => "SU",
            Self::Vectoring => "VM",
        }
    }
    pub fn describe(self) -> &'static str {
        match self {
            Self::Booster => "Big kick, short burn. Fast and short-ranged.",
            Self::Sustainer => "Gentle, long burn. Slower off the rail, keeps going.",
            Self::Vectoring => "Steers with its exhaust: +2,600 sideways pull while it burns.",
        }
    }
    /// Tonnes at 250 mm.
    fn mass(self) -> f32 {
        match self {
            Self::Booster => 0.06,
            Self::Sustainer | Self::Vectoring => 0.07,
        }
    }
    /// u/s² × tonnes at 250 mm.
    fn thrust(self) -> f32 {
        match self {
            Self::Booster => 300.0,
            Self::Sustainer => 140.0,
            Self::Vectoring => 230.0,
        }
    }
    /// Seconds of burn with no fuel tank, and per tank.
    fn burn(self) -> (f32, f32) {
        match self {
            Self::Booster | Self::Vectoring => (0.6, 0.7),
            Self::Sustainer => (1.6, 2.2),
        }
    }
    fn cost(self) -> f32 {
        match self {
            Self::Booster => 25.0,
            Self::Sustainer => 30.0,
            Self::Vectoring => 45.0,
        }
    }
}

impl MissileBody {
    pub const ALL: [MissileBody; 9] = [
        Self::Fuel, Self::Fins, Self::He, Self::Frag, Self::ShapedCharge, Self::Emp,
        Self::ProximityFuse, Self::SignalProcessor, Self::ArmourSheath,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Fuel => "Fuel tank",
            Self::Fins => "Fins",
            Self::He => "HE warhead",
            Self::Frag => "Frag warhead",
            Self::ShapedCharge => "Shaped charge",
            Self::Emp => "EMP warhead",
            Self::ProximityFuse => "Proximity fuse",
            Self::SignalProcessor => "Signal processor",
            Self::ArmourSheath => "Armour sheath",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Fuel => "FU",
            Self::Fins => "FN",
            Self::He => "HE",
            Self::Frag => "FR",
            Self::ShapedCharge => "SC",
            Self::Emp => "EM",
            Self::ProximityFuse => "PX",
            Self::SignalProcessor => "SP",
            Self::ArmourSheath => "AS",
        }
    }
    pub fn describe(self) -> &'static str {
        match self {
            Self::Fuel => "Burns longer: +0.7 s on a booster, +2.2 s on a sustainer.",
            Self::Fins => "+1,700 sideways pull. Each costs 5% top speed and a little drag.",
            Self::He => "Blast. 325k damage, falling off to the rim. Armour takes 70%.",
            Self::Frag => "Wide and weak against plate, but shreds exposed weapon mounts.",
            Self::ShapedCharge => "A jet that punches through block after block. Needs to touch the hull.",
            Self::Emp => "No damage. Everything in the burst is switched off for 6 s.",
            Self::ProximityFuse => "Bursts near the hull instead of on it: no near misses.",
            Self::SignalProcessor => "Each one cuts the chance of being fooled to 45%.",
            Self::ArmourSheath => "+80% toughness against point defence.",
        }
    }
    fn mass(self) -> f32 {
        match self {
            Self::Fuel => 0.06,
            Self::Fins => 0.02,
            Self::He => 0.08,
            Self::Frag | Self::Emp | Self::ArmourSheath => 0.07,
            Self::ShapedCharge => 0.09,
            Self::ProximityFuse | Self::SignalProcessor => 0.02,
        }
    }
    fn cost(self) -> f32 {
        match self {
            Self::Fuel => 10.0,
            Self::Fins => 15.0,
            Self::ProximityFuse => 20.0,
            Self::He | Self::Frag => 30.0,
            Self::ShapedCharge => 45.0,
            Self::Emp => 55.0,
            Self::SignalProcessor => 40.0,
            Self::ArmourSheath => 25.0,
        }
    }
}

impl MissileHead {
    pub const ALL: [MissileHead; 4] = [Self::Heat, Self::Radar, Self::CommandLink, Self::DumbNose];
    pub fn name(self) -> &'static str {
        match self {
            Self::Heat => "Heat seeker",
            Self::Radar => "Radar seeker",
            Self::CommandLink => "Command link",
            Self::DumbNose => "Dumb nose",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Heat => "IR",
            Self::Radar => "RD",
            Self::CommandLink => "CL",
            Self::DumbNose => "DN",
        }
    }
    pub fn describe(self) -> &'static str {
        match self {
            Self::Heat => "Homes on engines. 3,200 u, ±35°. Fooled by heat decoys.",
            Self::Radar => "Homes on the middle of the ship. 5,200 u, ±50°. Fooled by chaff.",
            Self::CommandLink => "Flies where you aim, out to 7,000 u. Can't be fooled; less precise far out.",
            Self::DumbNose => "No seeker. Aimed once at launch, then flies straight. Carries a small HE charge.",
        }
    }
    fn mass(self) -> f32 {
        match self {
            Self::Heat => 0.04,
            Self::Radar | Self::DumbNose => 0.05,
            Self::CommandLink => 0.03,
        }
    }
    fn cost(self) -> f32 {
        match self {
            Self::Heat => 60.0,
            Self::Radar => 80.0,
            Self::CommandLink => 50.0,
            Self::DumbNose => 15.0,
        }
    }
}

/// What a launcher is: the size of missile it takes and how it puts one out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LauncherSpec {
    pub name: &'static str,
    pub gauge_mm: f32,
    pub max_parts: usize,
    /// Cold-gas speed out of the tube, u/s.
    pub eject: f32,
    /// Seconds before it can turn.
    pub popup: f32,
    pub reload_mult: f32,
}

pub fn launcher_spec(t: ModuleType) -> Option<LauncherSpec> {
    match t {
        ModuleType::LaunchPad => Some(LauncherSpec { name: "Launch Pad", gauge_mm: 120.0, max_parts: 4, eject: 260.0, popup: 0.2, reload_mult: 1.0 }),
        ModuleType::RailLauncher => Some(LauncherSpec { name: "Rail Launcher", gauge_mm: 250.0, max_parts: 6, eject: 220.0, popup: 0.25, reload_mult: 1.0 }),
        ModuleType::VlsSilo => Some(LauncherSpec { name: "VLS Silo", gauge_mm: 400.0, max_parts: 9, eject: 160.0, popup: 0.6, reload_mult: 1.15 }),
        _ => None,
    }
}

pub fn is_built_launcher(t: ModuleType) -> bool {
    launcher_spec(t).is_some()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MissileDesign {
    pub motor: MissileMotor,
    pub body: Vec<MissileBody>,
    pub head: MissileHead,
}

impl MissileDesign {
    /// What a launcher flies until someone designs it a missile: the Missile
    /// Bay's Hornet, Lancer and Sledge.
    pub fn default_for(t: ModuleType) -> Self {
        use MissileBody::*;
        match t {
            ModuleType::LaunchPad => Self { motor: MissileMotor::Booster, body: vec![Fins, He], head: MissileHead::Heat },
            ModuleType::VlsSilo => Self {
                motor: MissileMotor::Sustainer,
                body: vec![Fuel, Fins, SignalProcessor, He, He, He, ArmourSheath],
                head: MissileHead::Radar,
            },
            _ => Self { motor: MissileMotor::Booster, body: vec![Fuel, Fins, Fins, He], head: MissileHead::Radar },
        }
    }

    pub fn parts(&self) -> usize {
        self.body.len() + 2
    }

    fn count(&self, b: MissileBody) -> usize {
        self.body.iter().filter(|x| **x == b).count()
    }

    /// The strip as text, back to front: `[BO][FU][FN][HE][RD]`.
    pub fn strip(&self) -> String {
        let mut s = format!("[{}]", self.motor.code());
        for b in &self.body {
            s.push_str(&format!("[{}]", b.code()));
        }
        s.push_str(&format!("[{}]", self.head.code()));
        s
    }
}

/// A built launcher's missile design, on the launcher.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct MissileLoadout(pub MissileDesign);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seeker {
    Heat { range: f32, cone_deg: f32 },
    Radar { range: f32, cone_deg: f32 },
    CommandLink { range: f32 },
    None,
}

/// Everything a design adds up to, on a given launcher.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileStats {
    pub launcher: LauncherSpec,
    pub k: f32,
    pub mass_t: f32,
    /// u/s² while the motor burns (fin drag included).
    pub accel: f32,
    pub burn_s: f32,
    pub top_speed: f32,
    pub reach: f32,
    /// Per second, after burnout.
    pub drag: f32,
    /// Sideways pull, u/s²; more while a vectoring motor burns.
    pub lateral: f32,
    pub lateral_burning: f32,
    /// Seconds from launch to self-destruct.
    pub life_s: f32,
    pub turn_circle: f32,
    pub toughness: f32,
    pub seeker: Seeker,
    /// Chance a matching decoy fools it, per roll.
    pub fool_chance: f32,
    pub reload_s: f32,
    pub he_damage: f32,
    pub he_radius: f32,
    pub frag_damage: f32,
    pub frag_radius: f32,
    pub shaped_damage: f32,
    pub emp_radius: f32,
    pub ram_damage: f32,
    pub proximity: bool,
    pub fuse_radius: f32,
    pub cost: f32,
    pub parts: usize,
    pub fins: usize,
}

/// Coast after burnout before it self-destructs.
pub const COAST_LIFE: f32 = 6.0;
/// Below this after burnout a missile has stalled.
pub const STALL_SPEED: f32 = 250.0;
pub const MAX_SPEED: f32 = 2600.0 * RANGE_SCALE;
pub const EMP_SECONDS: f32 = 6.0;
/// On every warhead's damage (HE, frag, shaped charge), on top of the
/// Missile Bay's figures. Played 2026-11-03: missiles felt a little light
/// beside the slower, harder guns. Enemy missiles too, as with guns.
pub const WARHEAD_SCALE: f32 = 1.25;

pub fn stats(d: &MissileDesign, launcher: LauncherSpec) -> MissileStats {
    use MissileBody::*;
    let k = (launcher.gauge_mm / 250.0).powi(2);
    let mass_t = (d.motor.mass() + d.head.mass() + d.body.iter().map(|b| b.mass()).sum::<f32>()) * k;
    let fins = d.count(Fins);
    let tanks = d.count(Fuel) as f32;
    let procs = d.count(SignalProcessor) as i32;
    let sheaths = d.count(ArmourSheath) as f32;
    let n_he = d.count(He) as f32 + if d.head == MissileHead::DumbNose { 0.6 } else { 0.0 };
    let n_frag = d.count(Frag) as f32;
    let n_shaped = d.count(ShapedCharge) as f32;
    let n_emp = d.count(Emp) as f32;
    let (burn0, per_tank) = d.motor.burn();
    let burn_s = burn0 + tanks * per_tank;
    let fin_loss = (1.0 - 0.05 * fins as f32).max(0.6);
    let accel = d.motor.thrust() * k / mass_t * fin_loss * RANGE_SCALE;
    let drag = 0.10 + 0.02 * fins as f32;
    // Turning as tight relative to the fight as it always did.
    let lateral = (1400.0 + 1700.0 * fins as f32) / k.powf(0.25) * RANGE_SCALE;
    let lateral_burning = lateral + if d.motor == MissileMotor::Vectoring { 2600.0 * RANGE_SCALE } else { 0.0 };
    let life_s = launcher.popup + burn_s + COAST_LIFE;
    // Fly it in one dimension for top speed and reach, the way the range did.
    // Time is counted in whole steps: summed in f32, 0.01 s steps drift enough
    // to give the motor an extra one.
    let (mut v, mut x, mut top) = (launcher.eject as f64, 0.0f64, launcher.eject);
    let dt = 0.01f64;
    let (popup, burn_end) = (launcher.popup as f64, (launcher.popup + burn_s) as f64);
    for i in 0.. {
        let t = i as f64 * dt;
        if t >= life_s as f64 {
            break;
        }
        if t >= popup && t < burn_end {
            v = (v + accel as f64 * dt).min(MAX_SPEED as f64);
        } else if t >= burn_end {
            v *= (-(drag as f64) * dt).exp();
            if v < STALL_SPEED as f64 {
                break;
            }
        }
        x += v * dt;
        top = top.max(v as f32);
    }
    let x = x as f32;
    let seeker = match d.head {
        MissileHead::Heat => Seeker::Heat { range: 3200.0 * RANGE_SCALE, cone_deg: 35.0 },
        MissileHead::Radar => Seeker::Radar { range: 5200.0 * RANGE_SCALE, cone_deg: 50.0 },
        MissileHead::CommandLink => Seeker::CommandLink { range: 7000.0 * RANGE_SCALE },
        MissileHead::DumbNose => Seeker::None,
    };
    let fool_base = match d.head {
        MissileHead::Heat => 0.4,
        MissileHead::Radar => 0.35,
        _ => 0.0,
    };
    let parts = d.parts();
    MissileStats {
        launcher,
        k,
        mass_t,
        accel,
        burn_s,
        top_speed: top,
        reach: x,
        drag,
        lateral,
        lateral_burning,
        life_s,
        turn_circle: top * top / lateral,
        toughness: 50.0 * k.powf(0.7) * (1.0 + 0.8 * sheaths),
        seeker,
        fool_chance: fool_base * 0.45f32.powi(procs),
        reload_s: (2.0 + 0.8 * parts as f32) * (launcher.gauge_mm / 250.0).powf(0.6) * launcher.reload_mult,
        he_damage: 260.0 * WARHEAD_SCALE * k * n_he,
        he_radius: if n_he > 0.0 { 90.0 + 70.0 * (n_he * k).sqrt() } else { 0.0 },
        frag_damage: 110.0 * WARHEAD_SCALE * k * n_frag,
        frag_radius: if n_frag > 0.0 { 160.0 + 110.0 * (n_frag * k).sqrt() } else { 0.0 },
        shaped_damage: 380.0 * WARHEAD_SCALE * k * n_shaped,
        emp_radius: if n_emp > 0.0 { 150.0 + 90.0 * (n_emp * k).sqrt() } else { 0.0 },
        ram_damage: 40.0 * k,
        proximity: d.count(ProximityFuse) > 0,
        fuse_radius: 120.0 + 40.0 * k.sqrt(),
        cost: (d.motor.cost() + d.head.cost() + d.body.iter().map(|b| b.cost()).sum::<f32>()) * k.powf(0.6),
        parts,
        fins,
    }
}

/// A launcher's design, or the one it flies by default.
pub fn design_of(t: ModuleType, loadout: Option<&MissileLoadout>) -> MissileDesign {
    loadout.map(|l| l.0.clone()).unwrap_or_else(|| MissileDesign::default_for(t))
}

/// A launcher's weapon numbers come from its missile: reach is its range,
/// reload sets its rate of fire, and the warheads add up to its damage.
/// Runs when a launcher appears or its design changes.
pub fn fit_missile_loadouts(
    mut launchers: Query<
        (&crate::components::Module, Option<&MissileLoadout>, &mut crate::building::multiblock::components::BaseWeaponStats),
        Or<(Added<crate::building::multiblock::components::BaseWeaponStats>, Changed<MissileLoadout>)>,
    >,
) {
    for (module, loadout, mut base) in launchers.iter_mut() {
        let Some(spec) = launcher_spec(module.module_type) else { continue };
        let s = stats(&design_of(module.module_type, loadout), spec);
        base.damage = s.he_damage + s.frag_damage + s.shaped_damage;
        base.range = s.reach;
        base.fire_rate = 1.0 / s.reload_s;
        if super::gun_assembly::trace() {
            info!("MISSILE fitted {:?}: {}, reach {:.0}, top {:.0} u/s, reload {:.1} s", module.module_type, design_of(module.module_type, loadout).strip(), s.reach, s.top_speed, s.reload_s);
        }
    }
}

/// Point defence's chance of hitting a missile of size `k` going `speed`
/// with one shot (the Missile Bay's rule).
pub fn pd_hit_chance(speed: f32, k: f32) -> f32 {
    // (Its speed as it was tuned: point defence reaches further too, so a
    // faster missile doesn't give it any less time.)
    (0.22 * (1000.0 / (speed / RANGE_SCALE).max(300.0)).powf(0.8) * k.sqrt()).clamp(0.04, 0.9)
}

/// What's wrong with a design on this launcher, for the designer to show.
pub fn problems(d: &MissileDesign, s: &MissileStats) -> Vec<String> {
    let mut out = Vec::new();
    if s.parts > s.launcher.max_parts {
        out.push(format!("Too long for the {}: {} parts, it takes {}.", s.launcher.name, s.parts, s.launcher.max_parts));
    }
    if s.he_damage + s.frag_damage + s.shaped_damage + s.emp_radius == 0.0 {
        out.push(format!("No warhead. It only does damage by ramming: {:.0} to the block it hits.", s.ram_damage * s.top_speed / 1000.0));
    }
    if d.head != MissileHead::DumbNose && s.turn_circle > 900.0 {
        out.push(format!("Turn circle {:.0} blocks at top speed: it will overshoot a dodging target.", s.turn_circle / 66.0));
    }
    if s.shaped_damage > 0.0 && s.proximity {
        out.push("A shaped charge wants to touch the hull. With a proximity fuse it bursts early and wastes most of its jet.".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use MissileBody::*;

    fn rail() -> LauncherSpec { launcher_spec(ModuleType::RailLauncher).unwrap() }

    /// The Missile Bay's Lancer: reach 6,942, top speed 1,430, turn circle
    /// 6.5 blocks. Same rules, same answers.
    #[test]
    fn the_lancer_flies_as_it_did_on_the_range() {
        let d = MissileDesign::default_for(ModuleType::RailLauncher);
        assert_eq!(d.strip(), "[BO][FU][FN][FN][HE][RD]");
        let s = stats(&d, rail());
        // The Missile Bay's figures (1430 u/s, 6942 u), with the fight's scale.
        assert!((2.5..3.1).contains(&(s.top_speed / 1430.0)), "{}", s.top_speed);
        assert!((2.4..3.1).contains(&(s.reach / 6942.0)), "{}", s.reach);
        assert!((s.he_damage - 260.0 * WARHEAD_SCALE).abs() < 1e-3);
    }

    #[test]
    fn the_sledge_and_the_hornet_too() {
        let sledge = MissileDesign {
            motor: MissileMotor::Sustainer,
            body: vec![Fuel, Fins, SignalProcessor, ShapedCharge, ShapedCharge, ShapedCharge, ArmourSheath],
            head: MissileHead::CommandLink,
        };
        let s = stats(&sledge, launcher_spec(ModuleType::VlsSilo).unwrap());
        assert!((2.4..3.1).contains(&(s.reach / 7193.0)), "{}", s.reach);
        assert!((2.4..3.1).contains(&(s.top_speed / 1098.0)), "{}", s.top_speed);
        assert!((s.toughness - 174.0).abs() < 1.0);
        assert!((s.reload_s - 14.0).abs() < 0.1);
        let hornet = stats(&MissileDesign::default_for(ModuleType::LaunchPad), launcher_spec(ModuleType::LaunchPad).unwrap());
        assert!((2.4..3.1).contains(&(hornet.reach / 5240.0)), "{}", hornet.reach);
    }

    #[test]
    fn processors_make_a_seeker_harder_to_fool() {
        let mut d = MissileDesign::default_for(ModuleType::RailLauncher);
        let plain = stats(&d, rail()).fool_chance;
        d.body[1] = SignalProcessor;
        assert!((stats(&d, rail()).fool_chance - plain * 0.45).abs() < 1e-5);
    }

    #[test]
    fn too_many_parts_is_a_problem() {
        let mut d = MissileDesign::default_for(ModuleType::RailLauncher);
        d.body.push(He);
        let s = stats(&d, rail());
        assert!(problems(&d, &s).iter().any(|p| p.starts_with("Too long")));
    }

    #[test]
    fn point_defence_finds_small_fast_missiles_hard_to_hit() {
        assert!(pd_hit_chance(1400.0, 0.23) < pd_hit_chance(1400.0, 1.0));
        assert!(pd_hit_chance(600.0, 1.0) > pd_hit_chance(1400.0, 1.0));
    }
}
