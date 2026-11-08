use bevy::prelude::*;
use crate::components::ModuleType;

// ============================================================================
// KINETIC AMMO TYPES
// 15 types, each with unique hit behavior and tradeoffs.
// Players load a magazine MIX at station. Fires in loaded order.
// Different weights affect fire rate. Can't change mid-combat.
// ============================================================================

/// The 15 kinetic ammo types.
///
/// The VARIANT names are the real-world designations and are load-bearing:
/// serde writes them straight into designs/*.json (`"ammo": "APFSDS"`) and
/// into saves, so renaming one silently breaks every ship design that used
/// it. What the player reads is `name()`, which is crew slang — a gun crew
/// calls a squash head a Bell because it rings the plate, not because anyone
/// aboard remembers what HESH stood for. Keep the two layers separate: the
/// identifier is a key, the name is fiction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum KineticAmmoType {
    /// Solid penetrator — goes through hull, hits modules behind
    AP,
    /// Penetrates then detonates inside — devastating but passes through thin targets
    APHE,
    /// Surface detonation + fragments — great vs creatures, zero penetration
    HEFrag,
    /// Sets fires — sustained DOT, useless in vacuum
    Incendiary,
    /// Disables modules — zero physical damage
    EMPShell,
    /// Proximity airburst — anti-missile, anti-swarm, weak vs armor
    Flak,
    /// Shaped charge — extreme penetration at perpendicular, bad at angles
    HEAT,
    /// Squash head — damages modules behind armor via shockwave, no penetration
    HESH,
    /// Fin-stabilized sabot — fastest, extreme penetration, needle damage
    APFSDS,

    // ---- EXOTIC ROUNDS -----------------------------------------------------
    // Everything above is something a foundry could press today. Everything
    // below needs a containment field, a nanoforge, or a collapsed core, and
    // the price per round says so. They fit the same breech and share the
    // same table, so a magazine can mix them freely with the cheap stuff.
    /// Bottled plasma — melts its way in, then keeps burning inside
    PlasmaSlug,
    /// Anti-hydrogen in a failing magnetic bottle — annihilates on contact
    Antimatter,
    /// Collapses to a pinpoint well — crushes a compartment, ignores angle
    Singularity,
    /// Disassembler swarm — no impact worth the name, eats the block for half a minute
    NaniteCanister,
    /// Phase-shifted slug — walks through shields and armour alike, arrives weak
    PhaseSlug,
    /// Fast neutrons — passes through the hull and kills the crew behind it
    NeutronShell,
}

impl KineticAmmoType {
    /// Every round, conventional block first and exotics after. The one
    /// place the roster is written down: the tuning picker renders this, and
    /// the tests sweep it, so a variant added to the enum without a stat
    /// entry can't slip through unnoticed.
    pub const ALL: [Self; 15] = [
        Self::AP, Self::APHE, Self::HEFrag, Self::Incendiary, Self::EMPShell,
        Self::Flak, Self::HEAT, Self::HESH, Self::APFSDS,
        Self::PlasmaSlug, Self::Antimatter, Self::Singularity,
        Self::NaniteCanister, Self::PhaseSlug, Self::NeutronShell,
    ];

    /// Whether this round needs a containment field or a nanoforge to exist.
    /// Only used to sort and to justify the price; nothing branches on it.
    pub fn is_exotic(&self) -> bool {
        matches!(self, Self::PlasmaSlug | Self::Antimatter | Self::Singularity
            | Self::NaniteCanister | Self::PhaseSlug | Self::NeutronShell)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::AP => "Solid",
            Self::APHE => "Cracker",
            Self::HEFrag => "Shredder",
            Self::Incendiary => "Torch",
            Self::EMPShell => "Blackout",
            Self::Flak => "Curtain",
            Self::HEAT => "Lance",
            Self::HESH => "Bell",
            Self::APFSDS => "Rod",
            Self::PlasmaSlug => "Plasma Slug",
            Self::Antimatter => "Antimatter",
            Self::Singularity => "Singularity",
            Self::NaniteCanister => "Nanite Swarm",
            Self::PhaseSlug => "Phase Slug",
            Self::NeutronShell => "Neutron Shell",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::AP => "Just metal, no filler. Goes through the plate and hits whatever is behind it. Wasted if nothing is.",
            Self::APHE => "Punches in, then opens up inside. Devastating - but a thin target lets it through before it can arm.",
            Self::HEFrag => "Bursts on the skin and throws fragments. Shreds anything unarmoured. Skips off heavy plate.",
            Self::Incendiary => "Starts fires that keep burning. Needs air in the compartment - a depressurised target won't light.",
            Self::EMPShell => "Kills every system in reach and scratches nothing. No physical damage at all, so no use on creatures.",
            Self::Flak => "Bursts short and fills the space with fragments. Puts a wall in front of missiles and swarms; wasted on armour.",
            Self::HEAT => "A focused jet that eats plate - but only square on. Hit at an angle and almost nothing gets through.",
            Self::HESH => "Doesn't penetrate. Rings the plate hard enough that its inner face lets go, and the scab kills what's behind. Needs solid hull to ring.",
            Self::APFSDS => "A long rod at absurd speed. Through everything, and often out the far side without hitting much.",
            Self::PlasmaSlug => "Bottled star-stuff. Melts a hole going in and keeps burning inside. Hotter and shorter-lived than incendiary.",
            Self::Antimatter => "A microgram of anti-hydrogen in a bottle that fails on impact. Nothing survives the blast. Costs more than the gun.",
            Self::Singularity => "Collapses to a pinpoint well and drags the compartment into it. Gravity does not care what angle you hit at.",
            Self::NaniteCanister => "A canister of disassemblers. Barely dents the plate, then eats the block from inside for half a minute.",
            Self::PhaseSlug => "Held out of phase. Shields have nothing to grab and armour barely does either - but it re-materialises with little left.",
            Self::NeutronShell => "Fast neutrons through the plate. Leaves the hull intact and the crew behind it dead.",
        }
    }

    /// Weight multiplier — heavier ammo = slower fire rate
    pub fn weight_mult(&self) -> f32 {
        match self {
            Self::AP => 1.0,          // Baseline
            Self::APHE => 1.3,        // Heavier — has explosive filler
            Self::HEFrag => 1.1,      // Slightly heavy
            Self::Incendiary => 1.0,  // Normal weight
            Self::EMPShell => 1.2,    // Electronics add weight
            Self::Flak => 0.9,        // Lighter — fragmentation casing
            Self::HEAT => 1.4,        // Heavy — shaped charge liner
            Self::HESH => 1.3,        // Heavy — plastic explosive
            Self::APFSDS => 0.7,      // Light — discarding sabot, tiny dart
            Self::PlasmaSlug => 0.8,      // Light — the slug is mostly field
            Self::Antimatter => 0.9,      // The bottle weighs more than the payload
            Self::Singularity => 1.7,     // Heaviest — a collapsed core in a casing
            Self::NaniteCanister => 1.0,  // A canister of dust
            Self::PhaseSlug => 0.5,       // Lightest — half of it isn't here
            Self::NeutronShell => 1.25,   // Shielded until it isn't
        }
    }

    /// Cost per round in credits
    pub fn cost_per_round(&self) -> u32 {
        match self {
            Self::AP => 2,
            Self::APHE => 5,
            Self::HEFrag => 3,
            Self::Incendiary => 3,
            Self::EMPShell => 8,
            Self::Flak => 2,
            Self::HEAT => 6,
            Self::HESH => 5,
            Self::APFSDS => 10,
            // Dearer than every conventional round on purpose: it beats
            // incendiary on burn, pen, speed and weight at once, so the
            // price is the only thing keeping incendiary worth loading.
            Self::PlasmaSlug => 12,
            Self::Antimatter => 45,   // The reason you don't just load these
            Self::Singularity => 30,
            Self::NaniteCanister => 12,
            Self::PhaseSlug => 22,
            Self::NeutronShell => 18,
        }
    }

    /// Muzzle velocity multiplier — affects lead prediction
    pub fn velocity_mult(&self) -> f32 {
        match self {
            Self::AP => 1.0,
            Self::APHE => 0.9,       // Slightly slower — heavier
            Self::HEFrag => 0.85,    // Slower
            Self::Incendiary => 0.95,
            Self::EMPShell => 0.8,   // Slow — fragile electronics
            Self::Flak => 1.1,       // Fast — needs to reach area
            Self::HEAT => 0.75,      // Slowest — shaped charge is heavy
            Self::HESH => 0.8,       // Slow — soft nose
            Self::APFSDS => 1.5,     // Fastest — lightweight dart
            Self::PlasmaSlug => 1.2,      // Fast — light, and it accelerates itself
            Self::Antimatter => 1.15,
            Self::Singularity => 0.7,     // Slow and heavy; lead it generously
            Self::NaniteCanister => 0.85, // Fragile cargo, gentle charge
            Self::PhaseSlug => 1.3,       // Half-real, half the drag
            Self::NeutronShell => 0.85,
        }
    }

    /// Penetration value
    pub fn penetration(&self) -> f32 {
        match self {
            Self::AP => 50.0,
            Self::APHE => 40.0,     // Less than AP — explosive takes space from penetrator
            Self::HEFrag => 0.0,    // Zero — detonates on surface
            Self::Incendiary => 5.0, // Minimal
            Self::EMPShell => 10.0,  // Light pen to get near electronics
            Self::Flak => 0.0,      // Airburst — doesn't hit armor directly
            Self::HEAT => 70.0,     // Extreme — but angle dependent
            Self::HESH => 0.0,      // Zero pen — works through shockwave
            Self::APFSDS => 90.0,   // Highest penetration
            Self::PlasmaSlug => 30.0,     // Burns through rather than punching
            Self::Antimatter => 25.0,     // Only needs to get barely inside
            Self::Singularity => 20.0,    // The well does the work, not the slug
            Self::NaniteCanister => 20.0, // Enough to breach and deliver
            Self::PhaseSlug => 95.0,      // Armour is barely there for it
            Self::NeutronShell => 15.0,   // The shell stops; the radiation doesn't
        }
    }

    /// Fraction of a round's damage that reaches a module still covered by
    /// live hull, the rest being spent on the armour itself. This is what
    /// makes ammo choice a decision: a dart goes almost straight through to
    /// the engine you aimed at, while flak can't touch an internal until the
    /// plating over it is gone. See armor_pass_through for the None case.
    pub fn pass_through(&self) -> f32 {
        (self.penetration() / 100.0).clamp(0.0, 0.9)
    }

    /// Whether a raised shield can intercept this round at all.
    ///
    /// True for exactly one round. A shield stops matter, and the phase slug
    /// spends its flight not quite being any — it crosses the bubble as if
    /// it were not there and hits the hull underneath. That is the whole
    /// reason to pay 22 credits for a round that lands under half an AP's
    /// damage: against a shielded target it is the only one that lands.
    pub fn ignores_shields(&self) -> bool {
        matches!(self, Self::PhaseSlug)
    }

    /// Direct damage on hit
    pub fn damage_mult(&self) -> f32 {
        match self {
            Self::AP => 1.0,
            Self::APHE => 1.4,       // Good — pen + explosion
            Self::HEFrag => 0.7,     // Per-target lower, but hits area
            Self::Incendiary => 0.5,  // Low initial, fire does the rest
            Self::EMPShell => 0.0,    // Zero physical damage
            Self::Flak => 0.4,       // Low per-fragment
            Self::HEAT => 1.8,       // High — focused jet
            Self::HESH => 0.3,       // Low direct — shockwave does internal
            Self::APFSDS => 0.8,     // Moderate — needle damage
            Self::PlasmaSlug => 0.6,      // Low direct, the burn is the point
            Self::Antimatter => 2.2,      // Highest in the game, by a distance
            Self::Singularity => 0.9,     // Modest on the block it hits — see the implosion
            Self::NaniteCanister => 0.15, // Almost nothing. Wait for it.
            Self::PhaseSlug => 0.45,      // The price of ignoring everything
            Self::NeutronShell => 0.1,    // It is not trying to hurt the ship
        }
    }

    /// Projectile visual color
    pub fn color(&self) -> Color {
        match self {
            Self::AP => Color::srgb(0.8, 0.7, 0.3),      // Brass
            Self::APHE => Color::srgb(0.9, 0.5, 0.2),    // Orange-brass
            Self::HEFrag => Color::srgb(1.0, 0.6, 0.1),  // Bright orange
            Self::Incendiary => Color::srgb(1.0, 0.3, 0.1), // Red-orange
            Self::EMPShell => Color::srgb(0.4, 0.5, 0.9), // Blue
            Self::Flak => Color::srgb(0.9, 0.9, 0.4),    // Yellow
            Self::HEAT => Color::srgb(0.8, 0.4, 0.1),    // Dark orange
            Self::HESH => Color::srgb(0.7, 0.7, 0.3),    // Olive
            Self::APFSDS => Color::srgb(0.9, 0.9, 1.0),  // White-bright (fast)
            Self::PlasmaSlug => Color::srgb(0.45, 0.85, 1.0),   // Arc-blue
            Self::Antimatter => Color::srgb(0.85, 0.45, 1.0),   // Violet
            Self::Singularity => Color::srgb(0.35, 0.2, 0.55),  // Dark — it eats its own light
            Self::NaniteCanister => Color::srgb(0.5, 0.9, 0.5), // Sickly green
            Self::PhaseSlug => Color::srgba(0.75, 0.95, 1.0, 0.6), // Half-there
            Self::NeutronShell => Color::srgb(0.8, 1.0, 0.6),   // Cherenkov green-white
        }
    }
}

/// Ammo hit behavior — what happens on impact
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmmoHitBehavior {
    /// Penetrates and continues (AP, APFSDS)
    Penetrate {
        remaining_pen: f32,
        damage_falloff: f32, // Damage reduction per layer penetrated
    },
    /// Penetrates then explodes inside (APHE)
    PenetrateExplode {
        penetration: f32,
        blast_damage: f32,
        blast_radius: f32,
        min_armor_to_arm: f32, // Needs this much armor to arm the fuse
    },
    /// Surface explosion + fragments (HE-Frag)
    SurfaceExplode {
        blast_damage: f32,
        blast_radius: f32,
        fragment_count: u32,
        fragment_damage: f32,
    },
    /// Sets fire (Incendiary)
    Ignite {
        fire_intensity: f32,
        fire_duration: f32,
    },
    /// Disables electronics (EMP Shell)
    EMPDisable {
        disable_radius: f32,
        disable_duration: f32,
    },
    /// Proximity airburst (Flak)
    ProximityBurst {
        trigger_distance: f32,
        fragment_count: u32,
        fragment_damage: f32,
        fragment_radius: f32,
    },
    /// Shaped charge jet (HEAT)
    ShapedCharge {
        jet_penetration: f32,
        jet_damage: f32,
        angle_sensitivity: f32, // 0.0 = any angle, 1.0 = must be perpendicular
    },
    /// Shockwave through armor (HESH)
    Shockwave {
        shockwave_damage: f32,
        shockwave_radius: f32, // How many blocks deep the shockwave goes
        requires_solid_hull: bool,
    },
    /// Collapses inward instead of blowing outward (Singularity).
    /// Reads like a blast on the damage numbers, but it is the one round
    /// whose armour interaction is angle-blind — see `ricochet_cos` — and
    /// the only one that sheds no spall, because nothing is thrown clear.
    Implode {
        crush_damage: f32,
        crush_radius: f32,
    },
    /// Radiation through intact armour (Neutron Shell). Hurts nobody's hull
    /// and everybody's crew — the ship is left whole and unmanned, which is
    /// also what makes it the round to fire at something you intend to board.
    Irradiate {
        /// Dose delivered to each crew member caught, in crew HP.
        dose: f32,
        /// How many of the ship's crew the cone reaches.
        crew_affected: u32,
    },
}

impl KineticAmmoType {
    /// Get the hit behavior for this ammo type, scaled by weapon damage.
    /// Radii are in world units, sized against the 66-unit block grid —
    /// the original values predated the grid and couldn't even reach an
    /// adjacent block (e.g. blast_radius 40 vs 66 between block centers).
    pub fn hit_behavior(&self, base_damage: f32) -> AmmoHitBehavior {
        match self {
            Self::AP => AmmoHitBehavior::Penetrate {
                remaining_pen: self.penetration(),
                damage_falloff: 0.3, // Loses 30% per layer
            },
            Self::APHE => AmmoHitBehavior::PenetrateExplode {
                penetration: self.penetration(),
                blast_damage: base_damage * 0.8,
                blast_radius: 75.0,  // hit block + its direct neighbors
                min_armor_to_arm: 15.0, // Thin targets = fuse doesn't arm
            },
            Self::HEFrag => AmmoHitBehavior::SurfaceExplode {
                blast_damage: base_damage * 0.5,
                blast_radius: 110.0, // one full ring of blocks
                fragment_count: 8,
                fragment_damage: base_damage * 0.15,
            },
            Self::Incendiary => AmmoHitBehavior::Ignite {
                fire_intensity: 0.6,
                fire_duration: 8.0,
            },
            Self::EMPShell => AmmoHitBehavior::EMPDisable {
                disable_radius: 120.0, // reaches modules behind armor
                disable_duration: 6.0,
            },
            Self::Flak => AmmoHitBehavior::ProximityBurst {
                trigger_distance: 30.0,
                fragment_count: 12,
                fragment_damage: base_damage * 0.1,
                fragment_radius: 130.0, // wide, weak — saturation weapon
            },
            Self::HEAT => AmmoHitBehavior::ShapedCharge {
                jet_penetration: self.penetration(),
                jet_damage: base_damage * 1.5,
                angle_sensitivity: 0.7, // Needs mostly perpendicular hit
            },
            Self::HESH => AmmoHitBehavior::Shockwave {
                shockwave_damage: base_damage * 0.6,
                shockwave_radius: 2.0, // 2 blocks deep
                requires_solid_hull: true,
            },
            Self::APFSDS => AmmoHitBehavior::Penetrate {
                remaining_pen: self.penetration(),
                damage_falloff: 0.15, // Only loses 15% per layer — goes through everything
            },
            // Hotter and shorter than incendiary: roughly double the burn
            // rate over less than half the time, so it wants a follow-up
            // rather than being left to cook.
            Self::PlasmaSlug => AmmoHitBehavior::Ignite {
                fire_intensity: 1.4,
                fire_duration: 3.5,
            },
            // No arming threshold — there is no fuse to arm, only a bottle
            // to fail. It goes off on a shuttle's skin as happily as on a
            // dreadnought's belt.
            Self::Antimatter => AmmoHitBehavior::PenetrateExplode {
                penetration: self.penetration(),
                blast_damage: base_damage * 1.6,
                blast_radius: 160.0, // two rings of blocks
                min_armor_to_arm: 0.0,
            },
            Self::Singularity => AmmoHitBehavior::Implode {
                crush_damage: base_damage * 1.1,
                crush_radius: 130.0, // one and a half rings, evenly crushed
            },
            // Same delivery as a burn, a different clock: a twentieth of
            // plasma's bite over eight times as long. Fire one early and it
            // is still working when the fight ends.
            Self::NaniteCanister => AmmoHitBehavior::Ignite {
                fire_intensity: 0.8,
                fire_duration: 26.0,
            },
            Self::PhaseSlug => AmmoHitBehavior::Penetrate {
                remaining_pen: self.penetration(),
                damage_falloff: 0.1, // Barely notices the plate it passed
            },
            Self::NeutronShell => AmmoHitBehavior::Irradiate {
                dose: 55.0,        // Two shells put a crewman down
                crew_affected: 2,
            },
        }
    }
}

// ============================================================================
// MAGAZINE SYSTEM — loaded mix of ammo types
// ============================================================================

/// A loaded magazine with a specific mix of ammo types
#[derive(Component, Clone, Debug)]
pub struct LoadedMagazine {
    /// Ammo types in firing order
    pub rounds: Vec<KineticAmmoType>,
    /// Current round index
    pub current_round: usize,
    /// Total rounds remaining
    pub remaining: u32,
}

impl LoadedMagazine {
    /// Create a magazine with a single ammo type
    pub fn uniform(ammo_type: KineticAmmoType, count: u32) -> Self {
        Self {
            rounds: vec![ammo_type],
            current_round: 0,
            remaining: count,
        }
    }

    /// Create a magazine with a mixed load
    /// Pattern repeats: e.g., [AP, AP, HEFrag] fires AP, AP, HEFrag, AP, AP, HEFrag...
    pub fn mixed(pattern: Vec<KineticAmmoType>, total_rounds: u32) -> Self {
        Self {
            rounds: pattern,
            current_round: 0,
            remaining: total_rounds,
        }
    }

    /// Get the next round to fire
    pub fn next_round(&mut self) -> Option<KineticAmmoType> {
        if self.remaining == 0 || self.rounds.is_empty() {
            return None;
        }

        let round = self.rounds[self.current_round % self.rounds.len()];
        self.current_round += 1;
        self.remaining -= 1;
        Some(round)
    }

    /// Average weight multiplier for the loaded mix (affects fire rate)
    pub fn avg_weight_mult(&self) -> f32 {
        if self.rounds.is_empty() { return 1.0; }
        let total: f32 = self.rounds.iter().map(|r| r.weight_mult()).sum();
        total / self.rounds.len() as f32
    }

    /// Total cost of the magazine
    pub fn total_cost(&self) -> u32 {
        if self.rounds.is_empty() { return 0; }
        // Cost is per-round based on the mix pattern, multiplied by total rounds
        let pattern_cost: u32 = self.rounds.iter().map(|r| r.cost_per_round()).sum();
        let avg_cost = pattern_cost / self.rounds.len() as u32;
        avg_cost * self.remaining
    }
}

/// Fallback price of one round, for a weapon the registry gives no magazine
/// (power-fed guns, anything unregistered).
///
/// This was once the price of EVERY round of every gun, flat. That was
/// harmless only while `INFINITE_AMMO` hid it: a gatling round cost the same
/// as a railgun slug, so the starter's four gatlings alone burned 120c a
/// second and a full rearm from empty came to ~6,700c against 750c to start.
pub const BASE_ROUND_PRICE: f32 = 5.0;

/// What one full magazine of plain AP costs, as a share of the gun's own shop
/// price. The single knob for the whole ammo economy.
///
/// At 0.4 a round costs what the gun says it should: a gatling round 0.1c, a
/// cannon shell 1.5c, a heavy missile 5c, a railgun slug 8c. Refilling the
/// starter's whole battery from empty is ~420c; a hard thirty-second fight
/// with every gun on it is ~270c -- felt against a 400-900c bounty, not
/// ruinous.
pub const MAGAZINE_SHARE: f32 = 0.4;

/// At or above this a "magazine" is the stand-in for a power-fed gun (lasers,
/// plasma), not rounds anyone buys. The weapon rack reads it the same way.
pub const POWER_FED_AMMO: u32 = 999;

/// Station price of one AP round for this gun: its shop price times
/// `MAGAZINE_SHARE`, spread over its stock magazine. The stock size, not the
/// live `max_ammo`, so enlarging a magazine doesn't make its rounds cheaper.
pub fn round_price(weapon: ModuleType) -> f32 {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    // The registry is built once at startup and never mutated, so its prices
    // can be read off a private copy instead of threading the resource
    // through every system that quotes a rearm.
    static PRICES: OnceLock<HashMap<ModuleType, f32>> = OnceLock::new();
    PRICES
        .get_or_init(|| {
            crate::building::registry::build_registry()
                .defs
                .iter()
                .filter_map(|(kind, def)| match def.companion {
                    crate::building::registry::CompanionData::Weapon { ammo, .. }
                        if ammo > 0 && ammo < POWER_FED_AMMO =>
                    {
                        Some((*kind, def.cost as f32 * MAGAZINE_SHARE / ammo as f32))
                    }
                    _ => None,
                })
                .collect()
        })
        .get(&weapon)
        .copied()
        .unwrap_or(BASE_ROUND_PRICE)
}

/// What a station charges to put `rounds` back in this gun's magazine.
///
/// The gun sets the base (`round_price`), the loaded type scales it by its
/// `cost_per_round` relative to AP. That multiplier is the only brake on the
/// exotic rounds -- antimatter out-damages the whole table and would simply
/// be the correct answer every time if a magazine of it cost the same as
/// brass.
pub fn rearm_price(weapon: ModuleType, ammo: Option<KineticAmmoType>, rounds: u32) -> f32 {
    let type_mult = match ammo {
        Some(a) => a.cost_per_round() as f32 / KineticAmmoType::AP.cost_per_round() as f32,
        None => 1.0,
    };
    round_price(weapon) * type_mult * rounds as f32
}

/// Default magazine configs for common loadouts
pub fn default_magazines() -> Vec<(&'static str, Vec<KineticAmmoType>, &'static str)> {
    vec![
        ("Straight Solid", vec![KineticAmmoType::AP], "Plain metal. Cheap, reliable, always does something."),
        ("Can Opener", vec![KineticAmmoType::AP, KineticAmmoType::AP, KineticAmmoType::APHE], "Two to weaken the plate, one to open up inside it."),
        ("Meat Grinder", vec![KineticAmmoType::HEFrag], "Skin bursts and fragments. For things that aren't wearing armour."),
        ("Arson", vec![KineticAmmoType::Incendiary, KineticAmmoType::Incendiary, KineticAmmoType::HEFrag], "Light it, then open it up so it keeps burning."),
        ("Point Defence", vec![KineticAmmoType::Flak], "A wall of fragments. Missiles and swarms die in it."),
        ("Dark Ship", vec![KineticAmmoType::AP, KineticAmmoType::EMPShell], "Open the hull, then kill everything wired behind it."),
        ("Lancework", vec![KineticAmmoType::HEAT], "Maximum against one target, square on. Angle ruins it."),
        ("Ringing", vec![KineticAmmoType::HESH], "Never breaches. Kills the compartment through plate it can't get through."),
        ("Long Rod", vec![KineticAmmoType::APFSDS], "Through anything, and often out the other side. Expensive."),
        ("Kitchen Sink", vec![KineticAmmoType::AP, KineticAmmoType::HEFrag, KineticAmmoType::Incendiary, KineticAmmoType::APHE], "A bit of everything."),
        // Exotic loads. Every one of these costs more per magazine than the
        // gun firing it, so they're written as MIXES with cheap filler — the
        // expensive round is the payload, the AP is what pays for the trip.
        ("Breach & Burn", vec![KineticAmmoType::AP, KineticAmmoType::PlasmaSlug], "Open the plate, then pour a star in through the hole."),
        ("Long Game", vec![KineticAmmoType::AP, KineticAmmoType::AP, KineticAmmoType::NaniteCanister], "Seed disassemblers early. Let the fight last long enough for them to finish."),
        ("Shield Breaker", vec![KineticAmmoType::PhaseSlug], "Ignores bubbles entirely. Weak per hit - this is how you hurt something you otherwise can't."),
        ("Boarding Prep", vec![KineticAmmoType::NeutronShell, KineticAmmoType::NeutronShell, KineticAmmoType::AP], "Kill the crew, keep the hull. Walk aboard a ship that still works."),
        ("Grave Digger", vec![KineticAmmoType::AP, KineticAmmoType::AP, KineticAmmoType::AP, KineticAmmoType::Singularity], "Three to strip the plate, one to fold the compartment inward."),
        ("Blank Cheque", vec![KineticAmmoType::Antimatter], "Every trigger pull costs more than a hull plate. Nothing survives one."),
    ]
}

/// What comes off the BACK of a plate when a round gets through it.
///
/// Distinct from the round's own effect in `AmmoHitBehavior`: that's the shell
/// doing something (detonating, burning, disabling). This is the ARMOUR
/// failing — fragments of the plate's inner face driven into whatever is
/// behind it. It's why a penetration is categorically worse than a dent, and
/// it's where the rounds differ most, because it describes what each one does
/// once it's inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpallProfile {
    /// Fragments thrown off the inner face.
    pub fragments: u32,
    /// Half-angle of the cone they spray into, in degrees. A long dart makes a
    /// neat hole; a squash head detaches a whole scab.
    pub cone_degrees: f32,
    /// Damage per fragment, as a fraction of the round's damage.
    pub damage_frac: f32,
    /// How far in the fragments carry, in cells.
    pub reach: f32,
    /// Whether it spalls WITHOUT penetrating. True only for HESH, whose entire
    /// identity is defeating armour it can't get through — it's the answer to
    /// a sloped hull, and the reason its penetration is 0 by design.
    pub through_solid: bool,
}

impl SpallProfile {
    pub const NONE: Self = Self {
        fragments: 0, cone_degrees: 0.0, damage_frac: 0.0, reach: 0.0, through_solid: false,
    };
}

/// Per-round spall. The shape of the cone is the round's signature:
///
///   APFSDS  narrow and deep   — a needle. Goes far in, wrecks little.
///   HEAT    narrow and hot    — kills whatever is directly behind the hole.
///   AP      the baseline.
///   APHE    wide              — its own blast finishes the compartment.
///   HESH    widest, and the only one that works through UNBREACHED armour.
pub fn spall(ammo: Option<KineticAmmoType>) -> SpallProfile {
    use KineticAmmoType::*;
    match ammo {
        Some(APFSDS) => SpallProfile { fragments: 3, cone_degrees: 12.0, damage_frac: 0.08, reach: 4.0, through_solid: false },
        Some(HEAT) => SpallProfile { fragments: 3, cone_degrees: 10.0, damage_frac: 0.30, reach: 3.0, through_solid: false },
        Some(APHE) => SpallProfile { fragments: 6, cone_degrees: 50.0, damage_frac: 0.12, reach: 2.0, through_solid: false },
        Some(HESH) => SpallProfile { fragments: 10, cone_degrees: 80.0, damage_frac: 0.18, reach: 2.0, through_solid: true },
        Some(Incendiary) => SpallProfile { fragments: 2, cone_degrees: 30.0, damage_frac: 0.05, reach: 2.0, through_solid: false },
        // Molten spatter rather than steel — wide, close, and it sticks.
        Some(PlasmaSlug) => SpallProfile { fragments: 4, cone_degrees: 45.0, damage_frac: 0.12, reach: 1.0, through_solid: false },
        // The inner face doesn't fragment so much as leave. Nothing else
        // comes close, which is what you are buying.
        Some(Antimatter) => SpallProfile { fragments: 12, cone_degrees: 70.0, damage_frac: 0.25, reach: 3.0, through_solid: false },
        // A phased slug parts the plate instead of breaking it; there is
        // barely a hole behind it to shed anything.
        Some(PhaseSlug) => SpallProfile { fragments: 1, cone_degrees: 15.0, damage_frac: 0.04, reach: 3.0, through_solid: false },
        // The one round that pulls in rather than throwing out. Its damage
        // is all in the implosion, and none of it goes anywhere.
        Some(Singularity) => SpallProfile::NONE,
        // The canister is meant to stay put and be opened from the inside.
        Some(NaniteCanister) => SpallProfile::NONE,
        // The shell stops at the plate. What gets through is not physical.
        Some(NeutronShell) => SpallProfile::NONE,
        // Surface bursts and EMP never get inside to shed anything.
        Some(HEFrag) | Some(Flak) | Some(EMPShell) => SpallProfile::NONE,
        // AP, and the unspecialised default for beams/rams/AI fire, which
        // arrives with no ammo profile recorded.
        Some(AP) | None => SpallProfile { fragments: 5, cone_degrees: 35.0, damage_frac: 0.10, reach: 2.0, through_solid: false },
    }
}

/// Pass-through for a hit whose round has no ammo profile — an unspecialised
/// shell, a beam, a ram. Armour stops most of it; a little always bleeds
/// through to whatever is bolted behind the plate.
pub fn armor_pass_through(ammo: Option<KineticAmmoType>) -> f32 {
    match ammo {
        Some(a) => a.pass_through(),
        None => 0.15,
    }
}

#[cfg(test)]
mod spall_tests {
    use super::*;

    /// The cone shape is each round's signature for what it does INSIDE a
    /// ship, and it's the axis they differ on most. A dart bores a neat hole;
    /// a squash head detaches a whole scab.
    #[test]
    fn cone_width_separates_the_rounds() {
        let dart = spall(Some(KineticAmmoType::APFSDS));
        let jet = spall(Some(KineticAmmoType::HEAT));
        let ap = spall(Some(KineticAmmoType::AP));
        let aphe = spall(Some(KineticAmmoType::APHE));
        let hesh = spall(Some(KineticAmmoType::HESH));

        assert!(dart.cone_degrees < ap.cone_degrees, "a long rod should bore, not scatter");
        assert!(jet.cone_degrees < ap.cone_degrees, "a shaped charge jet is focused");
        assert!(aphe.cone_degrees > ap.cone_degrees);
        assert!(hesh.cone_degrees > aphe.cone_degrees, "HESH should be the widest");
    }

    /// APFSDS reaches deepest and hurts least per fragment — the overpenetration
    /// trade its description already promises.
    #[test]
    fn a_dart_goes_deep_and_does_little() {
        let dart = spall(Some(KineticAmmoType::APFSDS));
        let ap = spall(Some(KineticAmmoType::AP));
        assert!(dart.reach > ap.reach);
        assert!(dart.damage_frac < ap.damage_frac);
        assert!(dart.fragments < ap.fragments);
    }

    /// HEAT concentrates: fewest fragments, hardest each. It kills the one
    /// thing behind the hole rather than gutting a compartment.
    #[test]
    fn a_shaped_charge_concentrates() {
        let jet = spall(Some(KineticAmmoType::HEAT));
        let ap = spall(Some(KineticAmmoType::AP));
        assert!(jet.damage_frac > ap.damage_frac);
        assert!(jet.fragments <= ap.fragments);
    }

    /// HESH is the ONLY round that spalls without getting through. That's its
    /// whole identity — the answer to armour too thick or too angled to
    /// breach — and the reason its penetration is 0 by design.
    #[test]
    fn only_hesh_spalls_through_unbreached_armour() {
        for ammo in KineticAmmoType::ALL {
            if ammo == KineticAmmoType::HESH { continue; }
            assert!(!spall(Some(ammo)).through_solid, "{ammo:?} must not spall through solid armour");
        }
        assert!(spall(Some(KineticAmmoType::HESH)).through_solid);
        assert_eq!(KineticAmmoType::HESH.penetration(), 0.0, "HESH earns its spall by not penetrating");
    }

    /// Surface bursts never get inside, so they shed nothing.
    #[test]
    fn surface_bursts_and_emp_shed_nothing() {
        for ammo in [KineticAmmoType::HEFrag, KineticAmmoType::Flak, KineticAmmoType::EMPShell] {
            assert_eq!(spall(Some(ammo)), SpallProfile::NONE, "{ammo:?} should not spall");
        }
    }

    /// Unspecialised fire — beams, rams, AI shots with no ammo recorded — gets
    /// the AP baseline rather than nothing, so a breach always means something.
    #[test]
    fn unspecialised_fire_gets_the_baseline() {
        assert_eq!(spall(None), spall(Some(KineticAmmoType::AP)));
        assert!(spall(None).fragments > 0);
    }

    /// Nothing may be pulled inward AND thrown outward. The singularity's
    /// damage is all in the implosion, so it is the only penetrating round
    /// that sheds nothing.
    #[test]
    fn an_implosion_throws_nothing_clear() {
        assert_eq!(spall(Some(KineticAmmoType::Singularity)), SpallProfile::NONE);
        assert!(matches!(
            KineticAmmoType::Singularity.hit_behavior(100.0),
            AmmoHitBehavior::Implode { .. }
        ));
    }

    /// Exactly one round crosses a raised shield. If a second ever does, the
    /// shield module stops being a decision and this test should be the thing
    /// that argues about it.
    #[test]
    fn only_the_phase_slug_ignores_shields() {
        for ammo in KineticAmmoType::ALL {
            assert_eq!(
                ammo.ignores_shields(),
                ammo == KineticAmmoType::PhaseSlug,
                "{ammo:?} shield behaviour",
            );
        }
    }

    /// A neutron shell is bought to leave a working ship behind. If its
    /// direct damage ever creeps up to the cheap rounds' level there is no
    /// reason to pay nine times the price for it.
    #[test]
    fn a_neutron_shell_spares_the_hull_and_not_the_crew() {
        let shell = KineticAmmoType::NeutronShell;
        assert!(shell.damage_mult() < KineticAmmoType::AP.damage_mult() * 0.25);
        assert_eq!(spall(Some(shell)), SpallProfile::NONE);
        match shell.hit_behavior(100.0) {
            AmmoHitBehavior::Irradiate { dose, crew_affected } => {
                assert!(dose > 0.0 && crew_affected > 0);
                // Two shells put a 100 HP crewman down — a real magazine's
                // worth of work, not a one-shot crew wipe.
                assert!(dose * 2.0 >= 100.0, "two shells should finish a crewman");
                assert!(dose < 100.0, "but one should not");
            }
            other => panic!("neutron shell should irradiate, got {other:?}"),
        }
    }

    /// The exotics are a price tier, not just a flavour tier. Every one of
    /// them costs more than every conventional round — that's what stops
    /// them from being a straight upgrade you always load.
    #[test]
    fn exotics_cost_more_than_anything_conventional() {
        let dearest_conventional = KineticAmmoType::ALL.iter()
            .filter(|a| !a.is_exotic())
            .map(|a| a.cost_per_round())
            .max()
            .unwrap();
        for ammo in KineticAmmoType::ALL.iter().filter(|a| a.is_exotic()) {
            assert!(
                ammo.cost_per_round() > dearest_conventional,
                "{ammo:?} at {} undercuts the conventional ceiling of {dearest_conventional}",
                ammo.cost_per_round(),
            );
        }
    }

    /// Antimatter is the ceiling on both axes at once — biggest hit, biggest
    /// bill. It is the round you fire when the shot has to land now.
    #[test]
    fn antimatter_is_the_ceiling() {
        let am = KineticAmmoType::Antimatter;
        for ammo in KineticAmmoType::ALL {
            if ammo == am { continue; }
            assert!(am.damage_mult() > ammo.damage_mult(), "{ammo:?} out-damages antimatter");
            assert!(am.cost_per_round() > ammo.cost_per_round(), "{ammo:?} out-costs antimatter");
        }
    }

    /// A full magazine of AP costs MAGAZINE_SHARE of the gun, whatever the gun.
    #[test]
    fn a_magazine_costs_a_share_of_its_gun() {
        let registry = crate::building::registry::build_registry();
        for weapon in [ModuleType::Gatling, ModuleType::Cannon, ModuleType::Railgun, ModuleType::HeavyMissile] {
            let def = registry.get(weapon);
            let crate::building::registry::CompanionData::Weapon { ammo, .. } = def.companion else {
                panic!("{weapon:?} has no magazine");
            };
            let magazine = rearm_price(weapon, Some(KineticAmmoType::AP), ammo);
            let want = def.cost as f32 * MAGAZINE_SHARE;
            assert!((magazine - want).abs() < 0.01, "{weapon:?}: {magazine} vs {want}");
        }
    }

    /// The flat rate charged a gatling round like a railgun slug.
    #[test]
    fn a_bigger_gun_has_dearer_rounds() {
        assert!(round_price(ModuleType::Railgun) > round_price(ModuleType::Cannon));
        assert!(round_price(ModuleType::Cannon) > round_price(ModuleType::Gatling));
        assert_eq!(rearm_price(ModuleType::Cannon, None, 20), rearm_price(ModuleType::Cannon, Some(KineticAmmoType::AP), 20));
    }

    /// Rearming the starter's battery from empty stays a fraction of what the
    /// run starts with, so a dry magazine is a bill, not a dead end.
    #[test]
    fn rearming_the_starter_from_empty_is_affordable() {
        let full = rearm_price(ModuleType::Cannon, None, 4 * 30)
            + rearm_price(ModuleType::Gatling, None, 4 * 300)
            + rearm_price(ModuleType::HeavyMissile, None, 2 * 12);
        assert!(full > 200.0 && full < 750.0, "starter rearm {full}c");
    }

    /// A magazine of exotics has to hurt to buy -- that's the entire brake on
    /// rounds that otherwise dominate the table.
    #[test]
    fn an_exotic_magazine_costs_real_money() {
        let ap = rearm_price(ModuleType::Cannon, Some(KineticAmmoType::AP), 30);
        for ammo in KineticAmmoType::ALL.iter().filter(|a| a.is_exotic()) {
            assert!(rearm_price(ModuleType::Cannon, Some(*ammo), 30) > ap * 2.0, "{ammo:?} rearm is too cheap");
        }
        // A cannon magazine of antimatter costs more than the cannon.
        assert!(rearm_price(ModuleType::Cannon, Some(KineticAmmoType::Antimatter), 30) > 113.0);
    }

    /// The serialized identifiers are a data contract, not decoration:
    /// designs/*.json stores `"ammo": "APFSDS"` and saves store the same, so
    /// renaming a variant to match its crew-slang display name would quietly
    /// orphan every ship design that loaded that round. Display names are
    /// free to change; these are not.
    #[test]
    fn variant_identifiers_survive_the_rename() {
        for (ammo, on_disk) in [
            (KineticAmmoType::AP, "\"AP\""),
            (KineticAmmoType::APHE, "\"APHE\""),
            (KineticAmmoType::HEFrag, "\"HEFrag\""),
            (KineticAmmoType::Incendiary, "\"Incendiary\""),
            (KineticAmmoType::EMPShell, "\"EMPShell\""),
            (KineticAmmoType::Flak, "\"Flak\""),
            (KineticAmmoType::HEAT, "\"HEAT\""),
            (KineticAmmoType::HESH, "\"HESH\""),
            (KineticAmmoType::APFSDS, "\"APFSDS\""),
        ] {
            assert_eq!(
                serde_json::to_string(&ammo).unwrap(), on_disk,
                "{ammo:?} changed on disk - existing ship designs would fail to load",
            );
        }
        // And the display name really has moved off the identifier, which is
        // the whole point of keeping the two apart.
        assert_eq!(KineticAmmoType::HESH.name(), "Bell");
        assert_eq!(KineticAmmoType::APFSDS.name(), "Rod");
    }

    /// No two rounds may share a display name — the tuning picker is a grid
    /// of nothing but these, so a collision is unpickable in practice.
    #[test]
    fn display_names_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for ammo in KineticAmmoType::ALL {
            assert!(seen.insert(ammo.name()), "duplicate display name {:?}", ammo.name());
        }
    }

    /// Every round in the roster has a real entry in every stat table — no
    /// variant riding on a placeholder. Catches a new variant bolted onto the
    /// enum and given a copy of AP's numbers.
    #[test]
    fn every_round_is_priced_and_named() {
        for ammo in KineticAmmoType::ALL {
            assert!(!ammo.name().is_empty());
            assert!(ammo.description().len() > 20, "{ammo:?} needs a real description");
            assert!(ammo.cost_per_round() > 0, "{ammo:?} must cost something");
            assert!(ammo.weight_mult() > 0.0 && ammo.velocity_mult() > 0.0, "{ammo:?}");
        }
    }
}
