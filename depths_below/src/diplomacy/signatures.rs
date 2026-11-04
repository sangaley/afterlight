//! Signature modules: a faction's own version of a module you already know.
//!
//! Every module is in the build menu from the start, and no faction's ships
//! carry parts of their own, so a signature isn't a new block. It is a plain
//! block built the faction's way: same behaviour, same art in the faction's
//! colours, its own name, one edge and one cost. They're sold at each
//! capital's Military tab once you're Friendly (dock_tabs), bought once per
//! run, and then offered in the build menu beside the plain block (V switches
//! between them).
//!
//! The list is the "Military markets draft" in the Capitals and Story Quests
//! Plan doc, with five entries swapped where the edge turned out to touch a
//! number nothing in the game reads (noise, manoeuvre thrusters, jammers,
//! targeting computers, stealth coating). No guns: combat balance was settled just before this, so the
//! edges are on armour, shields, engines, reactors, sensors and the ship's
//! insides.
//!
//! A signature is applied at spawn (`ship::spawn_module_signed`), never
//! patched on afterwards, so a loaded module's saved health lands on the
//! signature's maximum rather than the plain block's.

use bevy::prelude::*;

use crate::ai_ship::components::AiShipType;
use crate::components::ModuleType;

/// One faction's version of a module.
#[derive(Debug)]
pub struct SignatureDef {
    pub id: &'static str,
    pub faction: AiShipType,
    pub name: &'static str,
    pub base: ModuleType,
    /// The edge and the cost, as the Military tab says them.
    pub edge: &'static str,
    pub cost: &'static str,
    /// Credits to unlock it at the Military tab.
    pub price: u32,
    /// Multiplies the plain block's build price.
    pub build: f32,
    /// Multiplies the block's health.
    pub health: f32,
    /// Multiplies the block's main number (thrust, output, range, capacity,
    /// repair or healing rate, pull, shield).
    pub output: f32,
    /// Multiplies the power it draws.
    pub power: f32,
    /// Multiplies its blast when it goes up.
    pub blast: f32,
    /// Health it grows back, per second.
    pub regen: f32,
}

const PLAIN: SignatureDef = SignatureDef {
    id: "",
    faction: AiShipType::TheSilence,
    name: "",
    base: ModuleType::ArmorPlate,
    edge: "",
    cost: "",
    price: 0,
    build: 1.0,
    health: 1.0,
    output: 1.0,
    power: 1.0,
    blast: 1.0,
    regen: 0.0,
};

use AiShipType::*;
use ModuleType as M;

/// Every signature module, by faction.
pub const SIGNATURES: &[SignatureDef] = &[
    // ---- The Silence: no weapons, they listen ----
    SignatureDef { id: "pilgrim_berths", faction: TheSilence, name: "Pilgrim Berths", base: M::BasicQuarters, edge: "50% more berths", cost: "Build cost x2", price: 2000, output: 1.5, build: 2.0, ..PLAIN },
    SignatureDef { id: "listening_array", faction: TheSilence, name: "Listening Array", base: M::PassiveRadar, edge: "Twice the range", cost: "More power", price: 2500, output: 2.0, power: 1.5, ..PLAIN },
    SignatureDef { id: "cloister_bulkhead", faction: TheSilence, name: "Cloister Bulkhead", base: M::EmergencyBulkhead, edge: "Twice the health", cost: "Build cost x3", price: 2000, health: 2.0, build: 3.0, ..PLAIN },
    SignatureDef { id: "stillwater_scrubber", faction: TheSilence, name: "Stillwater Scrubber", base: M::OxygenScrubber, edge: "50% more oxygen", cost: "Build cost x2", price: 1500, output: 1.5, build: 2.0, ..PLAIN },
    // ---- Merchant Princes: luxury, deep stock ----
    SignatureDef { id: "deep_hold", faction: MerchantPrinces, name: "Deep Hold", base: M::BulkCargoHold, edge: "60% more cargo", cost: "Build cost x3", price: 4000, output: 1.6, build: 3.0, ..PLAIN },
    SignatureDef { id: "courier_engine", faction: MerchantPrinces, name: "Courier Engine", base: M::StandardEngine, edge: "30% more thrust", cost: "40% more power", price: 4500, output: 1.3, power: 1.4, ..PLAIN },
    SignatureDef { id: "bonded_tank", faction: MerchantPrinces, name: "Bonded Tank", base: M::FuelTank, edge: "+250 fuel capacity", cost: "Build cost x3", price: 2500, output: 1.5, build: 3.0, ..PLAIN },
    // ---- Free Haulers' Compact: cheap, plain, patched ----
    SignatureDef { id: "scrap_welded_plate", faction: FreeHaulersCompact, name: "Scrap-Welded Plate", base: M::ArmorPlate, edge: "Build cost x0.4", cost: "15% less health", price: 1500, build: 0.4, health: 0.85, ..PLAIN },
    SignatureDef { id: "tug_tractor", faction: FreeHaulersCompact, name: "Tug Tractor", base: M::TractorBeam, edge: "50% longer reach", cost: "More power", price: 3000, output: 1.5, power: 1.5, ..PLAIN },
    SignatureDef { id: "long_haul_tank", faction: FreeHaulersCompact, name: "Long-Haul Tank", base: M::FuelTank, edge: "+500 fuel capacity", cost: "Half the health", price: 2000, output: 2.0, health: 0.5, ..PLAIN },
    SignatureDef { id: "union_repair_bay", faction: FreeHaulersCompact, name: "Union Repair Bay", base: M::RepairBay, edge: "40% faster repairs", cost: "Build cost x2", price: 3500, output: 1.4, build: 2.0, ..PLAIN },
    // ---- Recursive Kingdom: one person, many hulls ----
    SignatureDef { id: "mirror_berths", faction: RecursiveKingdom, name: "Mirror Berths", base: M::BasicQuarters, edge: "Twice the berths", cost: "Half the health", price: 2000, output: 2.0, health: 0.5, ..PLAIN },
    SignatureDef { id: "twin_core_reactor", faction: RecursiveKingdom, name: "Twin Core Reactor", base: M::SmallReactor, edge: "60% more output", cost: "Explodes harder", price: 3500, output: 1.6, blast: 2.0, ..PLAIN },
    SignatureDef { id: "ram_plate", faction: RecursiveKingdom, name: "Ram Plate", base: M::ArmorPlate, edge: "40% more health", cost: "Build cost x2", price: 3000, health: 1.4, build: 2.0, ..PLAIN },
    // ---- Broken Choir: ghost ships, voices ----
    SignatureDef { id: "choir_relay", faction: BrokenChoir, name: "Choir Relay", base: M::RadarArray, edge: "60% more range", cost: "More power", price: 3000, output: 1.6, power: 1.5, ..PLAIN },
    SignatureDef { id: "echo_repair_bay", faction: BrokenChoir, name: "Echo Repair Bay", base: M::RepairBay, edge: "50% faster repairs", cost: "Half the health", price: 3500, output: 1.5, health: 0.5, ..PLAIN },
    SignatureDef { id: "hollow_plate", faction: BrokenChoir, name: "Hollow Plate", base: M::ArmorPlate, edge: "Build cost x0.5", cost: "30% less health", price: 1500, build: 0.5, health: 0.7, ..PLAIN },
    // ---- Stellar Preserve: keepers, vaults, they flee ----
    SignatureDef { id: "ark_vault", faction: StellarPreserve, name: "Ark Vault", base: M::CreatureContainment, edge: "Twice the hold space", cost: "Build cost x2", price: 2500, output: 2.0, build: 2.0, ..PLAIN },
    SignatureDef { id: "sanctuary_medbay", faction: StellarPreserve, name: "Sanctuary Medbay", base: M::MedBay, edge: "50% faster healing", cost: "Build cost x2", price: 3000, output: 1.5, build: 2.0, ..PLAIN },
    SignatureDef { id: "flight_engine", faction: StellarPreserve, name: "Flight Engine", base: M::StandardEngine, edge: "25% more thrust", cost: "30% less health", price: 3500, output: 1.25, health: 0.7, ..PLAIN },
    SignatureDef { id: "wardens_eye", faction: StellarPreserve, name: "Warden's Eye", base: M::AdvancedRadar, edge: "40% more range", cost: "Build cost x2", price: 3500, output: 1.4, build: 2.0, ..PLAIN },
    // ---- Synthesis Collective: grown, self-healing ----
    SignatureDef { id: "grown_plate", faction: SynthesisCollective, name: "Grown Plate", base: M::ArmorPlate, edge: "Heals itself slowly", cost: "20% less health", price: 5000, health: 0.8, regen: 1.0, ..PLAIN },
    SignatureDef { id: "graft_bay", faction: SynthesisCollective, name: "Graft Bay", base: M::SurgicalBay, edge: "50% faster surgery", cost: "Build cost x2", price: 4000, output: 1.5, build: 2.0, ..PLAIN },
    SignatureDef { id: "flesh_reactor", faction: SynthesisCollective, name: "Flesh Reactor", base: M::StandardReactor, edge: "40% more output", cost: "40% less health", price: 5000, output: 1.4, health: 0.6, ..PLAIN },
    // ---- Gilded Throne: elite mercenaries ----
    SignatureDef { id: "throne_emitter", faction: GildedThrone, name: "Throne Emitter", base: M::ShieldEmitter, edge: "50% more shield", cost: "50% more power", price: 6000, output: 1.5, power: 1.5, ..PLAIN },
    SignatureDef { id: "gilded_plate", faction: GildedThrone, name: "Gilded Plate", base: M::ArmorPlate, edge: "60% more health", cost: "Build cost x4", price: 5000, health: 1.6, build: 4.0, ..PLAIN },
    SignatureDef { id: "mercenary_engine", faction: GildedThrone, name: "Mercenary Engine", base: M::StandardEngine, edge: "40% more thrust", cost: "Build cost x3", price: 5500, output: 1.4, build: 3.0, ..PLAIN },
    SignatureDef { id: "retainer_quarters", faction: GildedThrone, name: "Retainer Quarters", base: M::BasicQuarters, edge: "60% more berths", cost: "Build cost x4", price: 4500, output: 1.6, build: 4.0, ..PLAIN },
    // ---- Corpse Stars: entropy cult, heavy armour ----
    SignatureDef { id: "tomb_plate", faction: CorpseStars, name: "Tomb Plate", base: M::ArmorPlate, edge: "Twice the health", cost: "Build cost x3", price: 6000, health: 2.0, build: 3.0, ..PLAIN },
    SignatureDef { id: "pyre_reactor", faction: CorpseStars, name: "Pyre Reactor", base: M::StandardReactor, edge: "60% more output", cost: "Explodes much harder", price: 6500, output: 1.6, blast: 3.0, ..PLAIN },
    SignatureDef { id: "dirge_pulse", faction: CorpseStars, name: "Dirge Pulse", base: M::EMPPulse, edge: "50% wider", cost: "More power", price: 5500, output: 1.5, power: 1.5, ..PLAIN },
    // ---- Terran Hegemony: battleships, discipline ----
    SignatureDef { id: "bastion_plate", faction: TerranHegemony, name: "Bastion Plate", base: M::ArmorPlate, edge: "70% more health", cost: "Build cost x3", price: 6000, health: 1.7, build: 3.0, ..PLAIN },
    SignatureDef { id: "fleet_reactor", faction: TerranHegemony, name: "Fleet Reactor", base: M::StandardReactor, edge: "30% more output", cost: "Build cost x2", price: 5000, output: 1.3, build: 2.0, ..PLAIN },
    SignatureDef { id: "line_shield_emitter", faction: TerranHegemony, name: "Line Shield Emitter", base: M::ShieldEmitter, edge: "30% more shield", cost: "Build cost x2", price: 5500, output: 1.3, build: 2.0, ..PLAIN },
    SignatureDef { id: "fleet_repair_bay", faction: TerranHegemony, name: "Fleet Repair Bay", base: M::RepairBay, edge: "30% faster repairs", cost: "More power", price: 4000, output: 1.3, power: 1.5, ..PLAIN },
];

/// Where a faction's signatures come from, one line each, said in the
/// Military tab: each delegation brought its own work to the summit.
pub fn summit_line(faction: AiShipType) -> &'static str {
    match faction {
        TheSilence => "Their delegation sat at the back of the summit hall and wrote everything down. These were built from their notes.",
        MerchantPrinces => "The Princes sold copies of these to every delegation at the summit. They kept the originals.",
        FreeHaulersCompact => "Patterns the Compact's riggers drew on the summit station's loading docks, the week before it ended.",
        RecursiveKingdom => "Their delegate came to the summit as a hundred copies of himself, and each copy brought a drawing.",
        BrokenChoir => "Built to plans one of their voices remembers from the summit. They are not sure which voice.",
        StellarPreserve => "The Preserve brought these to the summit to keep the delegates' specimens alive. Nobody asked which specimens.",
        SynthesisCollective => "Grown from samples taken at the summit. With consent, they say.",
        GildedThrone => "The Throne's bodyguards wore these patterns into the summit hall. Their clients walked out again.",
        CorpseStars => "Laid down by the Corpse Stars' delegation, who came to the summit already in mourning.",
        TerranHegemony => "Hegemony Command's patterns, filed at the summit as a gesture of trust and never withdrawn.",
        _ => "",
    }
}

/// A signature by id.
pub fn get(id: &str) -> Option<&'static SignatureDef> {
    SIGNATURES.iter().find(|s| s.id == id)
}

/// A faction's signatures, in Military-tab order.
pub fn of(faction: AiShipType) -> impl Iterator<Item = &'static SignatureDef> {
    SIGNATURES.iter().filter(move |s| s.faction == faction)
}

/// The faction's colour laid over the plain block's art: enough to tell a
/// Bastion Plate from a plain one at a glance, not enough to repaint it.
pub fn tint(def: &SignatureDef, base: Color) -> Color {
    let f = crate::ai_ship::components::faction_map_color(def.faction).to_srgba();
    let b = base.to_srgba();
    let mix = |x: f32, y: f32| x * (1.0 - TINT) + x * y * TINT;
    Color::srgba(mix(b.red, f.red), mix(b.green, f.green), mix(b.blue, f.blue), b.alpha)
}

/// How far the faction's colour goes into the block.
const TINT: f32 = 0.55;

/// A block built to a faction's signature. Saved with the module.
#[derive(Component, Clone, Copy, Debug)]
pub struct Signature(pub &'static SignatureDef);

/// Fuel the ship holds with no signature tanks: FuelState's own default.
pub const BASE_FUEL: f32 = 1500.0;
/// Extra fuel a tank's `output` share above one is worth.
const FUEL_PER_TANK_SHARE: f32 = 500.0;

/// Plain fuel tanks hold nothing the fuel gauge counts (the ship's tank is
/// one fixed figure); a signature tank adds to it while it stands.
pub fn fuel_capacity(
    ship: Query<Entity, With<crate::components::Ship>>,
    tanks: Query<(&Signature, &crate::components::Module, &ChildOf), Without<crate::components::DestroyedModule>>,
    mut fuel: ResMut<crate::resources::FuelState>,
) {
    let Ok(ship) = ship.single() else { return };
    let extra: f32 = tanks
        .iter()
        .filter(|(s, m, p)| p.parent() == ship && s.0.base == ModuleType::FuelTank && m.health > 0.0)
        .map(|(s, _, _)| (s.0.output - 1.0) * FUEL_PER_TANK_SHARE)
        .sum();
    let want = BASE_FUEL + extra;
    if (fuel.max_fuel - want).abs() > 0.5 {
        fuel.max_fuel = want;
    }
}

/// Debug override, in the house style (`DEPTHS_SUMMIT`):
/// `DEPTHS_SIGNATURES=all`, or a comma list of ids, starts the run with them
/// bought. Applied once per launch.
pub fn grant_from_env(mut standing: ResMut<crate::diplomacy::FactionStanding>, mut done: Local<bool>) {
    if *done {
        return;
    }
    *done = true;
    let Ok(want) = std::env::var("DEPTHS_SIGNATURES") else { return };
    for sig in SIGNATURES.iter().filter(|s| want == "all" || want.split(',').any(|w| w.trim() == s.id)) {
        if !standing.signatures.iter().any(|id| id == sig.id) {
            standing.signatures.push(sig.id.to_string());
        }
    }
    info!("SIGNATURE granted by DEPTHS_SIGNATURES: {}", standing.signatures.len());
}

/// Blocks that grow back (the Collective's Grown Plate).
pub fn regrow(time: Res<Time>, mut blocks: Query<(&Signature, &mut crate::components::Module), Without<crate::components::DestroyedModule>>) {
    let dt = time.delta_secs();
    for (sig, mut m) in &mut blocks {
        if sig.0.regen > 0.0 && m.health > 0.0 && m.health < m.max_health {
            m.health = (m.health + sig.0.regen * dt).min(m.max_health);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids are unique and the list matches the draft: 3-4 per faction, ten
    /// factions, no guns.
    #[test]
    fn three_or_four_each_and_no_guns() {
        let mut ids: Vec<_> = SIGNATURES.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SIGNATURES.len(), "duplicate id");
        for f in crate::celestial::territory::TERRITORIAL {
            let n = of(f).count();
            assert!((3..=4).contains(&n), "{f:?} has {n}");
            assert!(summit_line(f).is_ascii() && !summit_line(f).is_empty(), "{f:?} has no summit line");
        }
        for s in SIGNATURES {
            let cat = s.base.category();
            // Filed with the weapons, but neither hurts anyone: the beam tows
            // and the pulse switches things off (Dirge Pulse was on the list
            // as approved).
            if matches!(s.base, ModuleType::TractorBeam | ModuleType::EMPPulse) {
                continue;
            }
            assert!(
                !matches!(cat, crate::components::ModuleCategory::Weapons | crate::components::ModuleCategory::Guns | crate::components::ModuleCategory::Lasers),
                "{} is a gun",
                s.name
            );
        }
    }

    /// Every edge works through something the game reads (the bases whose
    /// numbers were checked, see the doc on the swapped entries). A new
    /// signature on any other base has to be checked the same way first.
    #[test]
    fn every_base_is_one_whose_edge_is_read() {
        use ModuleType as M;
        const READ: &[ModuleType] = &[
            M::ArmorPlate, M::EmergencyBulkhead, M::StandardEngine, M::PassiveRadar, M::AdvancedRadar, M::RadarArray,
            M::OxygenScrubber, M::BulkCargoHold, M::FuelTank, M::TractorBeam, M::RepairBay, M::BasicQuarters,
            M::SmallReactor, M::StandardReactor, M::CreatureContainment, M::MedBay, M::SurgicalBay, M::ShieldEmitter, M::EMPPulse,
        ];
        for s in SIGNATURES {
            assert!(READ.contains(&s.base), "{} is built on {:?}, whose edge nothing reads", s.name, s.base);
        }
        assert_eq!(BASE_FUEL, crate::resources::FuelState::default().max_fuel);
    }

    /// Every signature has an edge: something is better than the plain block.
    #[test]
    fn every_one_has_an_edge() {
        for s in SIGNATURES {
            let better = s.build < 1.0 || s.health > 1.0 || s.output > 1.0 || s.regen > 0.0;
            assert!(better, "{} is no better than plain", s.name);
            assert!(s.price > 0 && s.edge.is_ascii() && s.cost.is_ascii() && s.name.is_ascii());
        }
    }
}
