use crate::resources::ItemType;

// ============================================================================
// STATION TYPES
// Haven is always the Shipyard (the only station with build access). Every
// outpost is deterministically assigned one of the other types — stable
// across a run (seeded by station index), giving each outpost an actual
// identity instead of being an interchangeable resupply blob with an
// arbitrary price hash. Type drives both what an outpost pays well for
// (sell prices) and what it's cheap to buy from it (service costs).
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StationType {
    /// Haven only. Balanced prices, the only place with build access.
    Shipyard,
    /// Pays poorly for raw ore/crystal/alloy (they mine their own), pays
    /// over for imported fuel/ammo. Cheap hull repair (scrap on hand).
    MiningColony,
    /// Generous sell prices across the board — the place to dump mixed
    /// cargo — but no service discounts.
    TradeHub,
    /// Cheap ammo resupply, pays well for salvaged artifacts (intel value).
    MilitaryOutpost,
    /// Pays well for crystal/artifacts/bio samples (study value), poorly
    /// for scrap.
    ResearchOutpost,
    /// Cheap fuel resupply, otherwise unremarkable prices.
    RefuelDepot,
}

/// station_idx: 0 = Haven, 1..=12 = outposts (see world::home_base).
pub fn station_type(station_idx: usize) -> StationType {
    if station_idx == 0 {
        return StationType::Shipyard;
    }
    // Deterministic per-station hash — stable across the run, not re-rolled
    // every time you dock.
    let hash = (station_idx as u32).wrapping_mul(2654435761);
    match hash % 5 {
        0 => StationType::MiningColony,
        1 => StationType::TradeHub,
        2 => StationType::MilitaryOutpost,
        3 => StationType::ResearchOutpost,
        _ => StationType::RefuelDepot,
    }
}

pub fn station_type_name(t: StationType) -> &'static str {
    match t {
        StationType::Shipyard => "Shipyard",
        StationType::MiningColony => "Mining Colony",
        StationType::TradeHub => "Trade Hub",
        StationType::MilitaryOutpost => "Military Outpost",
        StationType::ResearchOutpost => "Research Outpost",
        StationType::RefuelDepot => "Refuel Depot",
    }
}

/// Base sell-price multiplier for `item` at a station of type `t`, before
/// the small per-station random jitter (see resources::station_item_price).
pub fn type_price_multiplier(t: StationType, item: ItemType) -> f32 {
    use ItemType::*;
    match (t, item) {
        (StationType::MiningColony, ScrapMetal) => 0.55,
        (StationType::MiningColony, Crystal) => 0.65,
        (StationType::MiningColony, RareAlloy) => 0.70,
        (StationType::MiningColony, FuelCell) => 1.30,
        (StationType::MiningColony, AmmoCrate) => 1.20,

        (StationType::TradeHub, _) => 1.20,

        (StationType::MilitaryOutpost, AmmoCrate) => 0.70,
        (StationType::MilitaryOutpost, ScrapMetal) => 1.10,
        (StationType::MilitaryOutpost, AncientArtifact) => 1.30,

        (StationType::ResearchOutpost, Crystal) => 1.50,
        (StationType::ResearchOutpost, AncientArtifact) => 1.60,
        (StationType::ResearchOutpost, BioSample) => 1.40,
        (StationType::ResearchOutpost, ScrapMetal) => 0.75,

        (StationType::RefuelDepot, FuelCell) => 0.60,

        _ => 1.0,
    }
}

/// Service cost multiplier for a station of type `t` — applied to the
/// credit portion of a docking/resupply service (after any resource offset).
#[derive(Clone, Copy, Default)]
pub struct ServiceDiscounts {
    pub fuel: f32,
    pub ammo: f32,
    pub hull_repair: f32,
}

pub fn service_discounts(t: StationType) -> ServiceDiscounts {
    match t {
        StationType::RefuelDepot => ServiceDiscounts { fuel: 0.5, ammo: 1.0, hull_repair: 1.0 },
        StationType::MilitaryOutpost => ServiceDiscounts { fuel: 1.0, ammo: 0.5, hull_repair: 1.0 },
        StationType::MiningColony => ServiceDiscounts { fuel: 1.0, ammo: 1.0, hull_repair: 0.7 },
        _ => ServiceDiscounts { fuel: 1.0, ammo: 1.0, hull_repair: 1.0 },
    }
}

/// A station's service prices, with what the dark has made scarce: fuel and
/// hull repairs cost more near it (narrative::front::front_scarcity).
pub fn service_discounts_at(station_idx: usize, galaxy: &crate::celestial::resources::GalaxyMap) -> ServiceDiscounts {
    let mut d = service_discounts(station_type(station_idx));
    let scarce = scarcity_at(station_idx, galaxy);
    d.fuel *= scarce;
    d.hull_repair *= scarce;
    d
}

/// How scarce fuel and repairs are at this station because of the dark.
pub fn scarcity_at(station_idx: usize, galaxy: &crate::celestial::resources::GalaxyMap) -> f32 {
    let system_id = (station_idx / crate::world::home_base::STATIONS_PER_SYSTEM) as u32;
    crate::narrative::front::front_scarcity(galaxy, Some(system_id))
}

/// How much dearer everything is this far from Haven.
///
/// Going outward used to cost nothing worth noticing: refuel was free on
/// docking and there are two stations in every system, so a player could fly
/// to the edge of the galaxy and top up along the way. Distance was the
/// game's whole progression axis and it was free to traverse.
///
/// Prices scale with the system's distance from Haven in galaxy space, so the
/// further out you push the more every berth costs to use — and the further
/// back your last affordable one is.
pub fn distance_price_multiplier(system_galaxy_pos: bevy::prelude::Vec2) -> f32 {
    let t = (system_galaxy_pos.length() / crate::celestial::galaxy::GALAXY_RADIUS)
        .clamp(0.0, 1.0);
    // Haven is at the origin and stays at face value; the far edge is triple.
    1.0 + t * 2.0
}

#[cfg(test)]
mod distance_price_tests {
    use super::*;
    use bevy::prelude::Vec2;

    /// Haven is the baseline. If the home berth were ever dearer than face
    /// value the opening would start by punishing the player for docking.
    #[test]
    fn haven_is_face_value() {
        assert_eq!(distance_price_multiplier(Vec2::ZERO), 1.0);
    }

    /// And the edge has to cost enough to be a budget rather than a rounding
    /// error, because distance being free is the thing this exists to fix.
    #[test]
    fn the_edge_is_meaningfully_dearer() {
        let edge = distance_price_multiplier(Vec2::new(crate::celestial::galaxy::GALAXY_RADIUS, 0.0));
        assert!(edge >= 2.5, "the far edge is only {edge}x — not a real cost");
    }

    /// It must rise smoothly, or there is a cliff where one more jump suddenly
    /// doubles the bill.
    #[test]
    fn it_rises_smoothly() {
        let r = crate::celestial::galaxy::GALAXY_RADIUS;
        let steps: Vec<f32> = [0.0, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|t| distance_price_multiplier(Vec2::new(r * t, 0.0)))
            .collect();
        for w in steps.windows(2) {
            assert!(w[1] > w[0], "prices did not rise: {steps:?}");
            assert!(w[1] - w[0] < 1.0, "a single step more than doubles: {steps:?}");
        }
    }

    /// Beyond the edge must not keep climbing, or a blind warp into deep void
    /// produces an absurd bill.
    #[test]
    fn it_is_capped() {
        let r = crate::celestial::galaxy::GALAXY_RADIUS;
        let edge = distance_price_multiplier(Vec2::new(r, 0.0));
        let past = distance_price_multiplier(Vec2::new(r * 10.0, 0.0));
        assert_eq!(edge, past);
    }
}
