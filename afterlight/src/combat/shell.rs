//! Shells you design: a strip of modules, back to front.
//!
//! ```text
//! [base][casing][casing][body][body][head]
//! ```
//!
//! - **Casings** are propellant, half a gauge long each. More of them push
//!   the shell faster, but need more barrel to burn and kick harder.
//! - **Bodies** are a gauge long: solid steel for weight and penetration, or
//!   a payload -- explosive, fragmentation, EMP, incendiary.
//! - **The head** is what meets the target: an AP cap punches through, a
//!   HEAT cone burns through whatever the angle, the payload heads add one
//!   more unit of their payload.
//! - **The base** is optional: a tracer steadies the shot, a base bleeder
//!   carries it further.
//!
//! The whole strip has to fit the gun's loaders (combat::gun_math). Each
//! built gun carries its own design on its firing piece (`GunShell`); the
//! default -- AP head, one solid body, two casings -- is the shell the Gun
//! Deck preview and Phase 2's numbers assumed, so a gun nobody has designed a
//! shell for fires exactly as it did before.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::ammo_types::{AmmoHitBehavior, KineticAmmoType};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellHead {
    #[default]
    Ap,
    Heat,
    He,
    Frag,
    Emp,
    Incendiary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellBody {
    Solid,
    He,
    Frag,
    Emp,
    Incendiary,
    /// Not a payload: it bursts the shell beside an enemy missile instead of
    /// waiting to hit something. Flak (combat::missile_defence).
    ProximityFuse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellBase {
    #[default]
    None,
    Tracer,
    Bleeder,
}

impl ShellHead {
    pub const ALL: [ShellHead; 6] = [Self::Ap, Self::Heat, Self::He, Self::Frag, Self::Emp, Self::Incendiary];

    pub fn name(self) -> &'static str {
        match self {
            Self::Ap => "AP",
            Self::Heat => "HEAT",
            Self::He => "HE",
            Self::Frag => "Frag",
            Self::Emp => "EMP",
            Self::Incendiary => "Incendiary",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Ap => "Armour-piercing cap. Punches through plate; the faster and heavier the shell, the deeper.",
            Self::Heat => "Shaped charge. Burns through a fixed depth of armour at any speed, but carries little weight.",
            Self::He => "Explosive nose. Bursts on the surface: one more unit of blast.",
            Self::Frag => "Fragmentation nose. Shreds crew and leaves the plate standing: one more unit of frag.",
            Self::Emp => "EMP nose. Knocks out the systems around the hit: one more unit of EMP.",
            Self::Incendiary => "Incendiary nose. Starts a fire where it lands: one more unit of fire.",
        }
    }
}

impl ShellBody {
    pub const ALL: [ShellBody; 6] = [Self::Solid, Self::He, Self::Frag, Self::Emp, Self::Incendiary, Self::ProximityFuse];

    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::He => "HE",
            Self::Frag => "Frag",
            Self::Emp => "EMP",
            Self::Incendiary => "Incendiary",
            Self::ProximityFuse => "Prox fuse",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Solid => "S",
            Self::He => "HE",
            Self::Frag => "FR",
            Self::Emp => "EM",
            Self::Incendiary => "IN",
            Self::ProximityFuse => "PX",
        }
    }
}

impl ShellBase {
    pub const ALL: [ShellBase; 3] = [Self::None, Self::Tracer, Self::Bleeder];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "No base",
            Self::Tracer => "Tracer",
            Self::Bleeder => "Base bleeder",
        }
    }
}

/// Most modules a strip may have, casings included (FtD's limit).
pub const MAX_SHELL_MODULES: usize = 20;

/// Payload units, by kind: one per body of that kind, plus the head's own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Payload {
    pub he: f32,
    pub frag: f32,
    pub emp: f32,
    pub incendiary: f32,
}

impl Payload {
    pub fn total(&self) -> f32 {
        self.he + self.frag + self.emp + self.incendiary
    }
}

/// One shell design: the strip, back to front.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShellDesign {
    #[serde(default)]
    pub base: ShellBase,
    pub casings: u8,
    pub bodies: Vec<ShellBody>,
    pub head: ShellHead,
}

impl Default for ShellDesign {
    fn default() -> Self {
        Self { base: ShellBase::None, casings: 2, bodies: vec![ShellBody::Solid], head: ShellHead::Ap }
    }
}

/// A built gun's shell design. On its Firing Piece; none means the default.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct GunShell(pub ShellDesign);

impl ShellDesign {
    /// Modules in the strip, casings included.
    pub fn modules(&self) -> usize {
        usize::from(self.base != ShellBase::None) + self.casings as usize + self.bodies.len() + 1
    }

    /// Length in gauges: a gauge per module, half a gauge per casing.
    pub fn length_gauges(&self) -> f32 {
        f32::from(u8::from(self.base != ShellBase::None)) + 0.5 * self.casings as f32 + self.bodies.len() as f32 + 1.0
    }

    /// What the propellant has to push, in modules: head and bodies, and half
    /// a base. The default shell's is 2.
    pub fn projectile_modules(&self) -> f32 {
        1.0 + self.bodies.len() as f32 + if self.base == ShellBase::None { 0.0 } else { 0.5 }
    }

    /// Muzzle velocity with the propellant fully burnt, before rails: 850 m/s
    /// for the default shell, growing with the square root of propellant per
    /// module pushed.
    pub fn full_burn_velocity(&self) -> f32 {
        (super::gun_math::FULL_BURN_VELOCITY * (self.casings as f32 / self.projectile_modules()).sqrt()).min(MAX_PROPELLANT_VELOCITY)
    }

    /// How much of the shell's mass is hard metal, against the default
    /// shell's 2 (AP head + solid body): what kinetic damage scales by.
    pub fn kinetic_weight(&self) -> f32 {
        let head = match self.head {
            ShellHead::Ap => 1.0,
            ShellHead::Heat => 0.4,
            _ => 0.5,
        };
        head + self.bodies.iter().map(|b| if *b == ShellBody::Solid { 1.0 } else { 0.3 }).sum::<f32>()
    }

    /// Bursts beside an enemy missile (flak).
    pub fn has_proximity_fuse(&self) -> bool {
        self.bodies.contains(&ShellBody::ProximityFuse)
    }

    /// How close an enemy missile has to come for a fused shell to burst,
    /// and how far the burst reaches, in u. None without a fuse.
    pub fn flak_radii(&self, gauge_mm: f32) -> Option<(f32, f32)> {
        self.has_proximity_fuse().then(|| {
            let fuse = 40.0 + 0.35 * gauge_mm;
            (fuse, fuse * 1.6)
        })
    }

    /// What a flak burst does to each missile it reaches, before the shell's
    /// own kinetic share: its frag and HE payload, scaled by gauge.
    pub fn flak_damage(&self, gauge_mm: f32) -> f32 {
        let g = gauge_mm / 100.0;
        let p = self.payload();
        // Payload goes with the shell's volume, but a missile's thin skin
        // doesn't need much: a 20 mm burst still does a share.
        ((FLAK_PER_FRAG * p.frag + HE_DAMAGE_PER_UNIT * 2.0 * p.he) * g * g).max(FLAK_MIN_PER_100MM * g)
    }

    pub fn solid_bodies(&self) -> usize {
        self.bodies.iter().filter(|b| **b == ShellBody::Solid).count()
    }

    pub fn payload(&self) -> Payload {
        let mut p = Payload::default();
        let mut add = |kind: ShellBody| match kind {
            ShellBody::He => p.he += 1.0,
            ShellBody::Frag => p.frag += 1.0,
            ShellBody::Emp => p.emp += 1.0,
            ShellBody::Incendiary => p.incendiary += 1.0,
            ShellBody::Solid | ShellBody::ProximityFuse => {}
        };
        for b in &self.bodies {
            add(*b);
        }
        match self.head {
            ShellHead::He => add(ShellBody::He),
            ShellHead::Frag => add(ShellBody::Frag),
            ShellHead::Emp => add(ShellBody::Emp),
            ShellHead::Incendiary => add(ShellBody::Incendiary),
            ShellHead::Ap | ShellHead::Heat => {}
        }
        p
    }

    /// The payload that decides what the hit does: the most units, ties to
    /// the more destructive.
    pub fn dominant_payload(&self) -> Option<ShellBody> {
        let p = self.payload();
        [
            (ShellBody::He, p.he),
            (ShellBody::Incendiary, p.incendiary),
            (ShellBody::Emp, p.emp),
            (ShellBody::Frag, p.frag),
        ]
        .into_iter()
        .filter(|(_, n)| *n > 0.0)
        .fold(None, |best: Option<(ShellBody, f32)>, (k, n)| match best {
            Some((_, m)) if m >= n => best,
            _ => Some((k, n)),
        })
        .map(|(k, _)| k)
    }

    /// Penetration as a multiple of the default AP shell's, at the same
    /// speed. HEAT's is separate: it doesn't care about speed.
    pub fn penetration_factor(&self) -> f32 {
        match self.head {
            ShellHead::Ap => 0.9 + 0.1 * self.solid_bodies() as f32,
            ShellHead::Heat => 0.0,
            _ => 0.25,
        }
    }

    /// The fixed-list round whose armour behaviour (spall, angle, what gets
    /// through plate) this shell shares. Its on-hit effect is its own.
    pub fn analogue(&self) -> KineticAmmoType {
        match (self.head, self.dominant_payload()) {
            (ShellHead::Heat, _) => KineticAmmoType::HEAT,
            (ShellHead::Ap, Some(ShellBody::He)) => KineticAmmoType::APHE,
            (ShellHead::Ap, _) => KineticAmmoType::AP,
            (_, Some(ShellBody::Emp)) => KineticAmmoType::EMPShell,
            (_, Some(ShellBody::Incendiary)) => KineticAmmoType::Incendiary,
            _ => KineticAmmoType::HEFrag,
        }
    }

    /// What the shell does when it hits, for a gun of this gauge.
    ///
    /// Radii and durations are per 150 mm of calibre: the hit resolver
    /// scales them by the round's calibre, as it does every round's.
    pub fn hit_behavior(&self, gauge_mm: f32, penetration: f32) -> AmmoHitBehavior {
        let g = gauge_mm / 100.0;
        let p = self.payload();
        match self.dominant_payload() {
            Some(ShellBody::He) => {
                let blast_damage = HE_DAMAGE_PER_UNIT * g * g * p.he;
                let blast_radius = 50.0 + 20.0 * p.he;
                if self.head == ShellHead::Ap {
                    AmmoHitBehavior::PenetrateExplode { penetration, blast_damage, blast_radius, min_armor_to_arm: 10.0 }
                } else {
                    AmmoHitBehavior::SurfaceExplode {
                        blast_damage,
                        blast_radius,
                        fragment_count: 6,
                        fragment_damage: blast_damage * 0.25,
                    }
                }
            }
            Some(ShellBody::Frag) => AmmoHitBehavior::Irradiate {
                dose: FRAG_DOSE_PER_UNIT * g * p.frag,
                crew_affected: 1 + p.frag as u32,
            },
            Some(ShellBody::Emp) => AmmoHitBehavior::EMPDisable {
                disable_radius: 50.0 + 25.0 * p.emp,
                disable_duration: 1.5 + 1.5 * p.emp,
            },
            Some(ShellBody::Incendiary) => AmmoHitBehavior::Ignite {
                fire_intensity: 0.25 * p.incendiary,
                fire_duration: 3.0 + 2.0 * p.incendiary,
            },
            _ if self.head == ShellHead::Heat => AmmoHitBehavior::ShapedCharge {
                jet_penetration: penetration,
                jet_damage: 0.0,
                angle_sensitivity: 0.0,
            },
            _ => AmmoHitBehavior::Penetrate {
                remaining_pen: penetration,
                damage_falloff: (0.3 - 0.05 * self.solid_bodies() as f32).max(0.1),
            },
        }
    }

    /// The strip as text, back to front: `[C][C][S][AP]`.
    pub fn strip(&self) -> String {
        let mut s = String::new();
        match self.base {
            ShellBase::None => {}
            ShellBase::Tracer => s.push_str("[TR]"),
            ShellBase::Bleeder => s.push_str("[BB]"),
        }
        for _ in 0..self.casings {
            s.push_str("[c]");
        }
        for b in &self.bodies {
            s.push_str(&format!("[{}]", b.short()));
        }
        s.push_str(&format!("[{}]", self.head.name()));
        s
    }

    /// Room for one more module?
    pub fn has_room(&self) -> bool {
        self.modules() < MAX_SHELL_MODULES
    }
}

/// A shell can't leave faster than this on propellant alone.
pub const MAX_PROPELLANT_VELOCITY: f32 = 1800.0;
/// Blast per HE unit per block caught, times (gauge / 100 mm) squared.
pub const HE_DAMAGE_PER_UNIT: f32 = 6.0;
/// Flak damage per unit of frag at 100 mm, against a missile.
pub const FLAK_PER_FRAG: f32 = 40.0;
/// The least a flak burst does to a missile, times gauge / 100 mm.
pub const FLAK_MIN_PER_100MM: f32 = 40.0;
/// Crew harm per frag unit, times gauge / 100 mm.
pub const FRAG_DOSE_PER_UNIT: f32 = 12.0;
/// Extra damage a HEAT jet does, times (gauge / 100 mm) squared, whatever
/// the shell's speed.
pub const HEAT_JET_DAMAGE: f32 = 22.0;
/// Penetration (mm) a HEAT jet burns through, per mm of gauge.
pub const HEAT_PEN_PER_MM: f32 = 1.6;

#[cfg(test)]
mod shell_tests {
    use super::*;

    #[test]
    fn the_default_shell_is_the_one_phase_two_assumed() {
        let s = ShellDesign::default();
        assert_eq!(s.length_gauges(), 3.0);
        assert_eq!(s.projectile_modules(), 2.0);
        assert_eq!(s.full_burn_velocity(), 850.0);
        assert_eq!(s.kinetic_weight(), 2.0);
        assert_eq!(s.penetration_factor(), 1.0);
        assert_eq!(s.analogue(), KineticAmmoType::AP);
        assert_eq!(s.strip(), "[c][c][S][AP]");
    }

    #[test]
    fn more_propellant_more_speed_more_payload_less() {
        let mut fast = ShellDesign::default();
        fast.casings = 4;
        let mut heavy = ShellDesign::default();
        heavy.bodies.push(ShellBody::Solid);
        assert!(fast.full_burn_velocity() > 850.0);
        assert!(heavy.full_burn_velocity() < 850.0);
        let mut none = ShellDesign::default();
        none.casings = 0;
        assert_eq!(none.full_burn_velocity(), 0.0);
    }

    #[test]
    fn the_biggest_payload_decides_the_hit() {
        let mut s = ShellDesign { head: ShellHead::Ap, bodies: vec![ShellBody::He, ShellBody::He], ..default() };
        assert!(matches!(s.hit_behavior(200.0, 50.0), AmmoHitBehavior::PenetrateExplode { .. }), "AP with HE behind it: APHE");
        assert_eq!(s.analogue(), KineticAmmoType::APHE);
        s.head = ShellHead::He;
        assert!(matches!(s.hit_behavior(200.0, 50.0), AmmoHitBehavior::SurfaceExplode { .. }));
        s.bodies = vec![ShellBody::Emp, ShellBody::Emp];
        s.head = ShellHead::Emp;
        assert!(matches!(s.hit_behavior(200.0, 50.0), AmmoHitBehavior::EMPDisable { .. }));
        s.head = ShellHead::Frag;
        s.bodies = vec![ShellBody::Frag];
        assert!(matches!(s.hit_behavior(200.0, 50.0), AmmoHitBehavior::Irradiate { crew_affected: 3, .. }));
    }

    #[test]
    fn more_explosive_means_a_bigger_blast() {
        let one = ShellDesign { head: ShellHead::He, bodies: vec![], ..default() };
        let three = ShellDesign { head: ShellHead::He, bodies: vec![ShellBody::He, ShellBody::He], ..default() };
        let (AmmoHitBehavior::SurfaceExplode { blast_damage: d1, blast_radius: r1, .. },
             AmmoHitBehavior::SurfaceExplode { blast_damage: d3, blast_radius: r3, .. }) =
            (one.hit_behavior(200.0, 0.0), three.hit_behavior(200.0, 0.0)) else { panic!() };
        assert!(d3 > d1 && r3 > r1);
    }

    #[test]
    fn a_strip_holds_twenty_modules() {
        let mut s = ShellDesign::default();
        while s.has_room() {
            s.bodies.push(ShellBody::Solid);
        }
        assert_eq!(s.modules(), MAX_SHELL_MODULES);
    }

    #[test]
    fn a_design_round_trips_through_json() {
        let s = ShellDesign { base: ShellBase::Tracer, casings: 3, bodies: vec![ShellBody::He, ShellBody::Solid], head: ShellHead::Heat };
        let back: ShellDesign = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
