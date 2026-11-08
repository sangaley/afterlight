use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::combat::ammo_types::KineticAmmoType;
use crate::components::{Module, ModuleType, Weapon, WeaponCooldown};
use crate::building::registry::{CompanionData, ModuleRegistry};

// ============================================================================
// WEAPON TUNING — stat-only customization, power as the currency
// Each weapon carries three multipliers (0.5×–2.0×). Pushing a stat above
// baseline raises the weapon's power draw quadratically; undertuning refunds
// power. The reactor budget is the only cap: the firing systems already
// refuse to fire while the power grid is in deficit.
// ============================================================================

pub const TUNING_MIN: f32 = 0.5;
pub const TUNING_MAX: f32 = 2.0;

/// Which stat a tuning slider drives — also the UI's handle into the struct.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TuningField {
    Velocity,
    FireRate,
    Damage,
    /// Turret traverse (aim) speed multiplier — only meaningful for turret guns.
    Traverse,
}

fn one() -> f32 { 1.0 }

/// Per-weapon stat multipliers set from the tuning window.
/// AI ships get the identity default via the shared spawn path — unaffected.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct WeaponTuning {
    pub velocity: f32,
    pub fire_rate: f32,
    pub damage: f32,
    /// Turret aim-speed multiplier (applied to sprite_map::turret_turn_speed).
    #[serde(default = "one")]
    pub traverse: f32,
}

impl Default for WeaponTuning {
    fn default() -> Self {
        Self { velocity: 1.0, fire_rate: 1.0, damage: 1.0, traverse: 1.0 }
    }
}

impl WeaponTuning {
    pub fn get(&self, field: TuningField) -> f32 {
        match field {
            TuningField::Velocity => self.velocity,
            TuningField::FireRate => self.fire_rate,
            TuningField::Damage => self.damage,
            TuningField::Traverse => self.traverse,
        }
    }

    pub fn set(&mut self, field: TuningField, value: f32) {
        let v = value.clamp(TUNING_MIN, TUNING_MAX);
        match field {
            TuningField::Velocity => self.velocity = v,
            TuningField::FireRate => self.fire_rate = v,
            TuningField::Damage => self.damage = v,
            TuningField::Traverse => self.traverse = v,
        }
    }

    /// Power multiplier: mean of squared multipliers. All sliders at 1.0× →
    /// 1.0 (base draw); all at 2.0× → 4× draw; all at 0.5× → 0.25× draw.
    /// Quadratic makes maxing everything brutally expensive.
    pub fn power_factor(&self) -> f32 {
        (self.velocity.powi(2) + self.fire_rate.powi(2) + self.damage.powi(2)) / 3.0
    }
}

/// Currently loaded ammo type for kinetic weapons. The projectile spawn reads
/// this for velocity/damage/penetration modifiers, color, and damage type.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SelectedAmmo(pub KineticAmmoType);

impl Default for SelectedAmmo {
    fn default() -> Self {
        Self(KineticAmmoType::AP)
    }
}

/// Weapon families that get the tuning window (utility weapons excluded).
pub fn is_tunable_weapon(module_type: ModuleType) -> bool {
    matches!(module_type,
        ModuleType::Cannon | ModuleType::Railgun | ModuleType::Coilgun | ModuleType::Gatling
        | ModuleType::Laser | ModuleType::PlasmaCaster | ModuleType::IonDisruptor
        | ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket
        // A gun built from parts: its sliders sit on top of its parts and shell.
        | ModuleType::FiringPiece
        // A launcher: its window is where its missile is designed.
        | ModuleType::LaunchPad | ModuleType::RailLauncher | ModuleType::VlsSilo
        // A laser emitter: its window holds its point defence switch.
        | ModuleType::LaserEmitter
    )
}

pub fn is_kinetic_weapon(module_type: ModuleType) -> bool {
    matches!(module_type,
        ModuleType::Cannon | ModuleType::Railgun | ModuleType::Coilgun | ModuleType::Gatling
    )
}

pub fn is_missile_weapon(module_type: ModuleType) -> bool {
    matches!(module_type,
        ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket
    )
}

/// Single source of truth for kinetic projectile muzzle speed — the firing
/// system and the tuning window's live stat readout both use this.
pub fn base_projectile_speed(module_type: ModuleType) -> f32 {
    let tuned: f32 = match module_type {
        ModuleType::Railgun => 9000.0,
        ModuleType::Coilgun => 8000.0,
        ModuleType::Cannon => 6000.0,
        ModuleType::Gatling => 5000.0,
        _ => 6000.0,
    };
    tuned * crate::combat::gun_assembly::RANGE_SCALE
}

/// UI label for the velocity slider — it means different things per family.
pub fn velocity_label(module_type: ModuleType) -> &'static str {
    if is_missile_weapon(module_type) {
        "THRUST"
    } else if is_kinetic_weapon(module_type) {
        "VELOCITY"
    } else {
        "INTENSITY"
    }
}

/// Heat a weapon dumps into its tile per second of sustained fire, as a
/// function of tuning. Calibrated against the 5.0/s ambient cooling in
/// ship/heat.rs: stock (factor 1.0) = 3.0/s, sustainable forever; maxed
/// (factor 4.0) = 12.0/s, overheats in ~10s of sustained fire, then the
/// gun thermally throttles (fire gate at 95% max temp) and cooks itself
/// if pushed past max. THIS is the real cost of maxed sliders — power
/// draw alone barely registers against a mid-game reactor.
pub fn weapon_heat_per_second(power_factor: f32) -> f32 {
    3.0 * power_factor
}

/// Sustained fire is heat-sustainable below this (ambient cooling rate).
pub const AMBIENT_COOLING_RATE: f32 = 5.0;

/// Weapons with zero registry draw still need tuning to cost something.
const MIN_TUNING_BASE_POWER: f32 = 5.0;

/// Applies tuning to the module's power draw. Damage/fire-rate multipliers
/// are NOT applied here — apply_weapon_enhancers resets every Weapon from
/// its base/machine stats each frame and composes tuning inside that reset
/// (see its doc comment); writing Weapon from a second system just races it.
/// Power draw always derives from the registry base, never the live value —
/// deriving from current values is how the old customization system ended up
/// with a feedback loop that decayed weapon range every frame.
pub fn apply_weapon_tuning(
    registry: Res<ModuleRegistry>,
    mut query: Query<(&mut Module, &WeaponTuning), Changed<WeaponTuning>>,
) {
    for (mut module, tuning) in query.iter_mut() {
        let Some(def) = registry.defs.get(&module.module_type) else { continue };
        if !matches!(def.companion, CompanionData::Weapon { .. }) { continue }

        module.power_consumption =
            def.power_consumption.max(MIN_TUNING_BASE_POWER) * tuning.power_factor();
    }
}

/// Keeps each weapon's cooldown timer in step with its live fire_rate.
/// Pre-existing gap: the cooldown duration was set once at spawn, so every
/// later fire_rate change (enhancer adjacency bonuses, and now tuning) was
/// silently inert for actual fire cadence.
pub fn sync_weapon_cooldowns(
    mut query: Query<(&Weapon, &mut WeaponCooldown), Changed<Weapon>>,
) {
    for (weapon, mut cooldown) in query.iter_mut() {
        let duration = std::time::Duration::from_secs_f32(1.0 / weapon.fire_rate.max(0.05));
        if cooldown.timer.duration() != duration {
            cooldown.timer.set_duration(duration);
            // set_duration leaves `elapsed` alone, so a fire rate going UP
            // shortens the duration out from under a timer that may already be
            // past the new end. Timer::remaining() is `duration - elapsed` and
            // panics outright on that — "overflow when subtracting durations",
            // which is a hard crash, not a warning. ship::subsystems' torpedo
            // autoloader reads remaining_secs() every frame, so it lands there.
            if cooldown.timer.elapsed() > duration {
                cooldown.timer.set_elapsed(duration);
            }
        }
    }
}
