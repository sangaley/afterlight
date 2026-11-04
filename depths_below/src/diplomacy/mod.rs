//! Your standing with each faction.
//!
//! Every combat faction used to attack you on sight. Now each one holds an
//! opinion of you, from -100 to 100, and the opinion decides who shoots:
//! only a faction that counts you as Hostile attacks, and a ship you fire on
//! fires back whatever its faction thinks. The same number sets prices at the
//! faction's own station, and treaties (the diplomacy window) move it.
//!
//! Where it starts comes from the lore: the predators (Corpse Stars, the
//! Collective, the Choir, the Recursive Kingdom, the Eternal Hegemony and the
//! Shepherd) start hostile, the Hegemony and the Gilded Throne start neutral,
//! and the Preserve and the Silence start friendly-ish.
//!
//! What moves it: killing a faction's ships (more for a capital or a
//! station), hitting them at all, killing their enemies, and trading at their
//! station.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ai_ship::components::{factions_hostile, AiShipClass, AiShipState, AiShipType};
use crate::ai_ship::roster::{self, ClassRole};
use crate::components::Ship;
use crate::events::{AiShipDamaged, AiShipDestroyed};
use crate::states::GameState;

pub mod contacts;
pub mod dock_tabs;
pub mod jobs;
pub mod signatures;
pub mod stations;
pub mod ui;

/// Every faction with an opinion of you, in the order the window lists them.
pub const FACTIONS: [AiShipType; 12] = [
    AiShipType::RecursiveKingdom,
    AiShipType::TheSilence,
    AiShipType::StellarPreserve,
    AiShipType::BrokenChoir,
    AiShipType::SynthesisCollective,
    AiShipType::GildedThrone,
    AiShipType::CorpseStars,
    AiShipType::TerranHegemony,
    AiShipType::EternalHegemony,
    AiShipType::Shepherd,
    AiShipType::MerchantPrinces,
    AiShipType::FreeHaulersCompact,
];

/// Where standing starts, from the lore.
pub fn starting_standing(faction: AiShipType) -> f32 {
    use AiShipType::*;
    match faction {
        StellarPreserve | TheSilence => 30.0,
        TerranHegemony | GildedThrone => 0.0,
        RecursiveKingdom | BrokenChoir | SynthesisCollective | CorpseStars => -60.0,
        EternalHegemony => -70.0,
        Shepherd => -85.0,
        // Traders deal with anyone who pays.
        MerchantPrinces | FreeHaulersCompact => 10.0,
    }
}

/// Standing bands. Hostile is the only one that shoots on sight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StandingTier {
    Hostile,
    Unfriendly,
    Neutral,
    Friendly,
    Allied,
}

impl StandingTier {
    pub const HOSTILE_BELOW: f32 = -40.0;
    pub const UNFRIENDLY_BELOW: f32 = -10.0;
    pub const FRIENDLY_FROM: f32 = 25.0;
    /// Standing needed to propose an alliance, and the floor that ends one.
    pub const ALLIANCE_FROM: f32 = 60.0;

    pub fn label(self) -> &'static str {
        match self {
            StandingTier::Hostile => "Hostile",
            StandingTier::Unfriendly => "Unfriendly",
            StandingTier::Neutral => "Neutral",
            StandingTier::Friendly => "Friendly",
            StandingTier::Allied => "Allied",
        }
    }

    /// What their station charges you, as a multiplier on its buy price and
    /// on what it pays when you sell. None = they won't let you dock.
    pub fn station_prices(self) -> Option<(f32, f32)> {
        match self {
            StandingTier::Hostile => None,
            StandingTier::Unfriendly => Some((1.5, 0.7)),
            StandingTier::Neutral => Some((1.2, 0.85)),
            StandingTier::Friendly => Some((1.0, 1.0)),
            StandingTier::Allied => Some((0.85, 1.1)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FactionEntry {
    pub faction: AiShipType,
    pub value: f32,
    #[serde(default)]
    pub allied: bool,
}

/// One line of "why did this change", newest last.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StandingEvent {
    pub faction: AiShipType,
    pub delta: f32,
    pub reason: String,
}

const LOG_LEN: usize = 40;

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct FactionStanding {
    pub entries: Vec<FactionEntry>,
    #[serde(default)]
    pub log: Vec<StandingEvent>,
    /// Factions you just dealt a big blow (a drop of PROVOCATION or more at
    /// once), for ai_ship::standing_spawns to answer with a war fleet.
    #[serde(skip)]
    pub provocations: Vec<AiShipType>,
    /// Signature modules bought at military markets this run (ids, see
    /// signatures.rs). They ride here because the market is the faction's,
    /// and this saves and resets with the run.
    #[serde(default)]
    pub signatures: Vec<String>,
}

impl Default for FactionStanding {
    fn default() -> Self {
        Self {
            entries: FACTIONS
                .iter()
                .map(|&f| FactionEntry { faction: f, value: starting_standing(f), allied: false })
                .collect(),
            log: Vec::new(),
            provocations: Vec::new(),
            signatures: Vec::new(),
        }
    }
}

impl FactionStanding {
    fn entry(&self, faction: AiShipType) -> Option<&FactionEntry> {
        self.entries.iter().find(|e| e.faction == faction)
    }

    fn entry_mut(&mut self, faction: AiShipType) -> &mut FactionEntry {
        if let Some(i) = self.entries.iter().position(|e| e.faction == faction) {
            return &mut self.entries[i];
        }
        self.entries.push(FactionEntry { faction, value: starting_standing(faction), allied: false });
        self.entries.last_mut().unwrap()
    }

    pub fn value(&self, faction: AiShipType) -> f32 {
        self.entry(faction).map_or(starting_standing(faction), |e| e.value)
    }

    pub fn is_allied(&self, faction: AiShipType) -> bool {
        self.entry(faction).is_some_and(|e| e.allied)
    }

    pub fn tier(&self, faction: AiShipType) -> StandingTier {
        let v = self.value(faction);
        if self.is_allied(faction) {
            StandingTier::Allied
        } else if v < StandingTier::HOSTILE_BELOW {
            StandingTier::Hostile
        } else if v < StandingTier::UNFRIENDLY_BELOW {
            StandingTier::Unfriendly
        } else if v < StandingTier::FRIENDLY_FROM {
            StandingTier::Neutral
        } else {
            StandingTier::Friendly
        }
    }

    /// Does this faction shoot at you on sight?
    pub fn attacks_player(&self, faction: AiShipType) -> bool {
        self.tier(faction) == StandingTier::Hostile
    }

    /// Would you be shot at for being in this system? Its owner decides, and
    /// so does a boss haunting it, whoever holds the place.
    pub fn system_hostile(&self, system: &crate::celestial::resources::StarSystemDef) -> bool {
        system.faction.into_iter().chain(system.haunt).any(|f| self.attacks_player(f))
    }

    /// Move standing and say why. An alliance that falls below its floor ends.
    pub fn change(&mut self, faction: AiShipType, delta: f32, reason: impl Into<String>) {
        if delta == 0.0 {
            return;
        }
        let e = self.entry_mut(faction);
        let before = e.value;
        e.value = (e.value + delta).clamp(-100.0, 100.0);
        let applied = e.value - before;
        let mut reason = reason.into();
        if e.allied && e.value < StandingTier::ALLIANCE_FROM - 20.0 {
            e.allied = false;
            reason.push_str(" (the alliance is over)");
        }
        if applied <= -crate::ai_ship::standing_spawns::PROVOCATION && !self.provocations.contains(&faction) {
            self.provocations.push(faction);
        }
        if applied != 0.0 {
            self.log.push(StandingEvent { faction, delta: applied, reason });
            if self.log.len() > LOG_LEN {
                let extra = self.log.len() - LOG_LEN;
                self.log.drain(..extra);
            }
        }
    }

    pub fn set_allied(&mut self, faction: AiShipType, allied: bool, reason: impl Into<String>) {
        self.entry_mut(faction).allied = allied;
        self.log.push(StandingEvent { faction, delta: 0.0, reason: reason.into() });
    }
}

/// What a kill costs with the victim's faction, by what kind of hull it was.
pub(crate) fn kill_penalty(role: Option<ClassRole>, station: bool, platform: bool) -> f32 {
    if platform {
        return 15.0;
    }
    if station {
        return 50.0;
    }
    match role {
        Some(ClassRole::Boss) => 40.0,
        Some(ClassRole::Capital) => 25.0,
        Some(ClassRole::Civilian) => 18.0,
        Some(ClassRole::Experimental) => 15.0,
        Some(ClassRole::Line) => 12.0,
        Some(ClassRole::Scout) | None => 8.0,
    }
}

/// Marks a faction station (and its gun platforms): killing one costs more.
#[derive(Component, Clone, Copy, Debug)]
pub struct FactionStation;

/// Kills you made move standing: down with the victim's faction, a little up
/// with every faction at war with it.
fn standing_from_kills(
    mut deaths: MessageReader<AiShipDestroyed>,
    victims: Query<(&AiShipState, Option<&AiShipClass>, Has<FactionStation>, Has<stations::StationPlatform>)>,
    player: Query<Entity, With<Ship>>,
    mut standing: ResMut<FactionStanding>,
) {
    let Ok(player) = player.single() else {
        deaths.clear();
        return;
    };
    for death in deaths.read() {
        let Ok((state, class, station, platform)) = victims.get(death.entity) else { continue };
        if state.last_attacker != Some(player) {
            continue;
        }
        let victim = death.ship_type;
        let role = class.and_then(|c| roster::find(victim, c.0)).map(|c| c.role);
        let what = if platform {
            "gun platform"
        } else if station {
            stations::station_plan(victim).map_or("station", |p| p.name)
        } else {
            class.and_then(|c| roster::find(victim, c.0)).map_or("ship", |c| c.name)
        };
        let name = crate::ai_ship::components::faction_display_name(victim);
        standing.change(victim, -kill_penalty(role, station, platform), format!("You destroyed their {what}"));
        for &other in FACTIONS.iter() {
            if other != victim && factions_hostile(other, victim) {
                standing.change(other, 3.0, format!("You destroyed a {name} {what}"));
            }
        }
    }
}

/// Hitting a faction's ship at all costs a little, at most once a second per
/// faction so a gatling burst isn't a landslide.
fn standing_from_hits(
    time: Res<Time>,
    mut hits: MessageReader<AiShipDamaged>,
    ships: Query<&AiShipType>,
    player: Query<Entity, With<Ship>>,
    mut standing: ResMut<FactionStanding>,
    mut cooldown: Local<Vec<(AiShipType, f32)>>,
) {
    let now = time.elapsed_secs();
    let Ok(player) = player.single() else {
        hits.clear();
        return;
    };
    for hit in hits.read() {
        if hit.attacker != Some(player) {
            continue;
        }
        let Ok(&faction) = ships.get(hit.target) else { continue };
        if let Some((_, until)) = cooldown.iter_mut().find(|(f, _)| *f == faction) {
            if now < *until {
                continue;
            }
            *until = now + 1.0;
        } else {
            cooldown.push((faction, now + 1.0));
        }
        let reason = if matches!(hit.source, crate::events::DamageSource::Collision) {
            "You rammed their ship"
        } else {
            "You fired on their ship"
        };
        standing.change(faction, -1.0, reason);
    }
}

/// DEPTHS_STANDING=<faction>:<value>[,<faction>:<value>...] sets standing at
/// the start of a run, for testing (e.g. "TerranHegemony:-80" to be shot at).
fn standing_from_env(mut standing: ResMut<FactionStanding>, mut done: Local<bool>) {
    if *done {
        return;
    }
    *done = true;
    let Ok(spec) = std::env::var("DEPTHS_STANDING") else { return };
    for part in spec.split(',') {
        let Some((name, value)) = part.split_once(':') else { continue };
        let Some(&faction) = FACTIONS.iter().find(|f| format!("{f:?}") == name.trim()) else {
            warn!("DEPTHS_STANDING: no faction called {name:?}");
            continue;
        };
        // "allied": an alliance (at 80); otherwise a number.
        let (v, allied) = match value.trim() {
            "allied" => (80.0, true),
            v => match v.parse::<f32>() {
                Ok(v) => (v, false),
                Err(_) => continue,
            },
        };
        let now = standing.value(faction);
        standing.change(faction, v - now, "DEPTHS_STANDING");
        if allied {
            standing.set_allied(faction, true, "DEPTHS_STANDING");
        }
    }
    // Setting it is not a blow you dealt them.
    standing.provocations.clear();
    // DEPTHS_PROVOKE=<Faction>: as if you'd just destroyed their station.
    if let Ok(name) = std::env::var("DEPTHS_PROVOKE") {
        if let Some(&faction) = FACTIONS.iter().find(|f| format!("{f:?}") == name.trim()) {
            standing.provocations.push(faction);
        }
    }
}

pub struct DiplomacyPlugin;

impl Plugin for DiplomacyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FactionStanding>()
            .add_systems(
                Update,
                (standing_from_env, standing_from_kills, standing_from_hits).run_if(in_state(GameState::Exploring)),
            )
            .add_plugins((ui::DiplomacyUiPlugin, stations::FactionStationPlugin))
            .init_resource::<contacts::Contacts>()
            .add_systems(Update, contacts::contacts_flee_fallen_capitals)
            .add_systems(
                Update,
                (dock_tabs::dock_tab_keys, dock_tabs::military_keys, dock_tabs::refresh_standing_chip, dock_tabs::refresh_dock_tabs, jobs::deliver_cargo, dock_tabs::refresh_dock_hold)
                    .chain()
                    .run_if(in_state(GameState::Docked)),
            )
            .init_resource::<jobs::Jobs>()
            .add_message::<jobs::SummitStepWon>()
            .add_systems(Update, (signatures::grant_from_env, signatures::regrow, signatures::fuel_capacity))
            .add_systems(OnExit(GameState::Docked), dock_tabs::lapse_offers)
            .add_systems(Update, (jobs::track_jobs, jobs::standing_from_contracts));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lore_decides_who_shoots_first() {
        let s = FactionStanding::default();
        assert!(s.attacks_player(AiShipType::CorpseStars));
        assert!(s.attacks_player(AiShipType::Shepherd));
        assert!(!s.attacks_player(AiShipType::TerranHegemony));
        assert!(!s.attacks_player(AiShipType::StellarPreserve));
        assert_eq!(s.tier(AiShipType::TheSilence), StandingTier::Friendly);
    }

    #[test]
    fn standing_is_clamped_and_logged() {
        let mut s = FactionStanding::default();
        s.change(AiShipType::TerranHegemony, -500.0, "test");
        assert_eq!(s.value(AiShipType::TerranHegemony), -100.0);
        assert_eq!(s.tier(AiShipType::TerranHegemony), StandingTier::Hostile);
        assert_eq!(s.log.last().unwrap().delta, -100.0);
    }

    #[test]
    fn an_alliance_ends_when_standing_falls() {
        let mut s = FactionStanding::default();
        s.change(AiShipType::GildedThrone, 70.0, "test");
        s.set_allied(AiShipType::GildedThrone, true, "test");
        assert_eq!(s.tier(AiShipType::GildedThrone), StandingTier::Allied);
        s.change(AiShipType::GildedThrone, -40.0, "betrayal");
        assert!(!s.is_allied(AiShipType::GildedThrone));
    }

    #[test]
    fn a_save_without_standing_still_loads() {
        let s: FactionStanding = serde_json::from_str(r#"{"entries":[]}"#).unwrap();
        assert_eq!(s.value(AiShipType::CorpseStars), starting_standing(AiShipType::CorpseStars));
    }

    /// A big blow (a station, a capital ship) is a provocation; the cost of
    /// a hit or a scout is not.
    #[test]
    fn a_big_blow_provokes() {
        let mut s = FactionStanding::default();
        s.change(AiShipType::TerranHegemony, -8.0, "a scout");
        assert!(s.provocations.is_empty());
        s.change(AiShipType::TerranHegemony, -50.0, "their station");
        assert_eq!(s.provocations, vec![AiShipType::TerranHegemony]);
        // Not saved: a load doesn't replay old grudges.
        let back: FactionStanding = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(back.provocations.is_empty());
    }
}
