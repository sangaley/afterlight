pub mod bounty_nav;
pub mod generation;
pub mod tracking;
pub mod ui;

use bevy::prelude::*;
use serde::{Serialize, Deserialize};

use crate::ai_ship::components::AiShipType;
use crate::components::{CreatureType, PoiType, ZoneType};
use crate::resources::ItemType;
use crate::states::GameState;

// ============================================================================
// CORE DATA TYPES
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum Faction {
    ResearchInstitute,
    Navy,
    SalvageGuild,
}

impl Faction {
    pub fn name(&self) -> &'static str {
        match self {
            Faction::ResearchInstitute => "Stellar Preserve",
            Faction::Navy => "Terran Hegemony",
            Faction::SalvageGuild => "Free Haulers' Compact",
        }
    }

    /// The real faction behind the board's name: the Navy is the
    /// Hegemony's, the Salvage Guild the Compact's, the Research Institute
    /// the Preserve's. Work for them counts with that faction
    /// (diplomacy::jobs::standing_from_contracts).
    pub fn real(&self) -> crate::ai_ship::components::AiShipType {
        use crate::ai_ship::components::AiShipType;
        match self {
            Faction::ResearchInstitute => AiShipType::StellarPreserve,
            Faction::Navy => AiShipType::TerranHegemony,
            Faction::SalvageGuild => AiShipType::FreeHaulersCompact,
        }
    }

    pub fn all() -> &'static [Faction] {
        &[Faction::ResearchInstitute, Faction::Navy, Faction::SalvageGuild]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum ContractType {
    Kill,
    ExplorePoi,
    ReachDepth,
    RetrieveSalvage,
    CaptureLive,
    SurveyZone,
    DestroyShip,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ContractObjective {
    Kill { creature_type: CreatureType, target_count: u32, current_count: u32 },
    ExplorePoi { poi_type: PoiType, discovered: bool },
    ReachDepth { target_depth: f32, reached: bool },
    RetrieveSalvage { item_type: ItemType, target_count: u32, current_count: u32 },
    CaptureLive { creature_type: CreatureType, captured: bool },
    SurveyZone { zone: ZoneType, required_seconds: f32, elapsed_seconds: f32 },
    /// Bounty on one specific tagged hostile ship (see
    /// ai_ship::components::BountyTarget) — reward scales with how far that
    /// ship actually is from Haven Station and how tough its faction is
    /// (see contracts::generation::destroy_ship_reward). `target_id` matches
    /// AiShipDestroyed::bounty_id when that exact ship dies.
    DestroyShip { ship_type: AiShipType, target_id: u32, destroyed: bool },
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum ContractStatus {
    Available,
    Active,
    Completed,
    TurnedIn,
    Failed,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Contract {
    pub id: u32,
    pub faction: Faction,
    pub contract_type: ContractType,
    pub title: String,
    pub description: String,
    pub objective: ContractObjective,
    pub reward: u32,
    pub deposit: u32,
    pub star_rating: u8,
    pub depth_zone: ZoneType,
    pub status: ContractStatus,
}

impl Contract {
    /// Returns a short progress string for HUD display.
    /// Not-yet-done objectives read "..." -- they used to read "X", which on
    /// a tracker next to "DONE" looks like a failed objective, not an open one.
    pub fn progress_text(&self) -> String {
        match &self.objective {
            ContractObjective::Kill { current_count, target_count, .. } => {
                format!("{}/{}", current_count, target_count)
            }
            ContractObjective::ExplorePoi { discovered, .. } => {
                if *discovered { "Done".into() } else { "...".into() }
            }
            ContractObjective::ReachDepth { reached, .. } => {
                if *reached { "Done".into() } else { "...".into() }
            }
            ContractObjective::RetrieveSalvage { current_count, target_count, .. } => {
                format!("{}/{}", current_count, target_count)
            }
            ContractObjective::CaptureLive { captured, .. } => {
                if *captured { "Done".into() } else { "...".into() }
            }
            ContractObjective::SurveyZone { elapsed_seconds, required_seconds, .. } => {
                format!("{:.0}/{:.0}s", elapsed_seconds, required_seconds)
            }
            ContractObjective::DestroyShip { destroyed, .. } => {
                if *destroyed { "Done".into() } else { "Hunting".into() }
            }
        }
    }

    /// Check if the objective is fulfilled.
    pub fn is_objective_complete(&self) -> bool {
        match &self.objective {
            ContractObjective::Kill { current_count, target_count, .. } => current_count >= target_count,
            ContractObjective::ExplorePoi { discovered, .. } => *discovered,
            ContractObjective::ReachDepth { reached, .. } => *reached,
            ContractObjective::RetrieveSalvage { current_count, target_count, .. } => current_count >= target_count,
            ContractObjective::CaptureLive { captured, .. } => *captured,
            ContractObjective::SurveyZone { elapsed_seconds, required_seconds, .. } => elapsed_seconds >= required_seconds,
            ContractObjective::DestroyShip { destroyed, .. } => *destroyed,
        }
    }

    /// Star display string like "[**]" or "[***]".
    pub fn star_display(&self) -> String {
        format!("[{}]", "*".repeat(self.star_rating as usize))
    }
}

// ============================================================================
// RESOURCES
// ============================================================================

/// Number of contract boards — one per station slot in the galaxy (every
/// system carries world::home_base::STATIONS_PER_SYSTEM of them). Boards are
/// generated lazily on first visit, so unvisited slots cost an empty Vec.
pub const STATION_COUNT: usize = crate::world::home_base::TOTAL_STATION_SLOTS;

#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct ContractState {
    /// One board per station (see STATION_COUNT) — each station offers its
    /// own bounties, generated lazily the first time its board is opened.
    pub available_by_station: Vec<Vec<Contract>>,
    /// Accepted contracts, shared across every station — there's no cap on
    /// how many can be active at once, and a completed one can be turned in
    /// at any station, not just the one that offered it.
    pub active_contracts: Vec<Contract>,
    pub next_id: u32,
    pub contracts_completed_total: u32,
    pub contracts_failed_total: u32,
}

impl ContractState {
    /// Board for a given station index, generating an empty slot on first
    /// access so indexing never panics even before any board has been rolled.
    pub fn board_mut(&mut self, station: usize) -> &mut Vec<Contract> {
        if self.available_by_station.len() < STATION_COUNT {
            self.available_by_station.resize(STATION_COUNT, Vec::new());
        }
        &mut self.available_by_station[station]
    }
}

/// Which station's board is currently open in the mission board UI.
#[derive(Resource, Default)]
pub struct ViewingStation(pub usize);

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct FactionReputation {
    pub research: f32,
    pub navy: f32,
    pub salvage: f32,
}

impl Default for FactionReputation {
    fn default() -> Self {
        Self {
            research: 10.0,
            navy: 10.0,
            salvage: 10.0,
        }
    }
}

impl FactionReputation {
    pub fn get(&self, faction: &Faction) -> f32 {
        match faction {
            Faction::ResearchInstitute => self.research,
            Faction::Navy => self.navy,
            Faction::SalvageGuild => self.salvage,
        }
    }

    pub fn add(&mut self, faction: &Faction, amount: f32) {
        let val = match faction {
            Faction::ResearchInstitute => &mut self.research,
            Faction::Navy => &mut self.navy,
            Faction::SalvageGuild => &mut self.salvage,
        };
        *val = (*val + amount).clamp(0.0, 100.0);
    }

    /// Max star rating unlocked for a given faction.
    pub fn max_star(&self, faction: &Faction) -> u8 {
        let rep = self.get(faction);
        if rep >= 80.0 { 5 }
        else if rep >= 50.0 { 4 }
        else if rep >= 25.0 { 3 }
        else if rep >= 10.0 { 2 }
        else { 1 }
    }

    /// Display stars like "***" for the max star rating.
    pub fn star_string(&self, faction: &Faction) -> String {
        "*".repeat(self.max_star(faction) as usize)
    }
}

#[derive(Resource, Default)]
pub struct MissionBoardOpen(pub bool);

// ============================================================================
// PLUGIN
// ============================================================================

pub struct ContractsPlugin;

impl Plugin for ContractsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<ContractState>()
            .init_resource::<FactionReputation>()
            .init_resource::<MissionBoardOpen>()
            .init_resource::<ViewingStation>()
            .init_resource::<ui::MissionBoardSelection>()
            .init_resource::<ui::ContractHudVisible>()
            // Turn in completed contracts on docking at Haven
            .add_systems(OnEnter(GameState::StationDocked), tracking::turn_in_contracts)
            // Tracking + claim-anywhere during exploration
            .add_systems(Update, (
                tracking::track_kill_contracts,
                tracking::track_poi_contracts,
                tracking::track_depth_contracts,
                tracking::track_salvage_contracts,
                tracking::track_survey_contracts,
                tracking::track_capture_contracts,
                tracking::track_destroy_ship_contracts,
                tracking::check_contract_completion,
                // Last claim on the F press - docking turns contracts in by
                // itself on arrival, so this convenience must never beat it.
                tracking::turn_in_at_station_proximity
                    .after(crate::world::home_base::station_docking),
            ).chain().run_if(in_state(GameState::Exploring)))
            // Mission board UI — usable docked at Haven or just flying near
            // any station (Haven or an outpost), each with its own board
            .add_systems(Update, (
                ui::toggle_mission_board,
                ui::mission_board_input,
                ui::update_mission_board_display,
            ).chain().run_if(in_state(GameState::StationDocked).or_else(in_state(GameState::Exploring))))
            // Contract HUD during exploration
            .add_systems(OnEnter(GameState::Exploring), (ui::spawn_contract_hud, bounty_nav::spawn_bounty_arrow))
            .add_systems(OnExit(GameState::Exploring), ui::despawn_contract_hud)
            .add_systems(Update, (ui::toggle_contract_hud, ui::update_contract_hud).chain()
                .run_if(in_state(GameState::Exploring)))
            // Bounty navigation: HUD arrow toward the nearest active target,
            // plus a floating label over the specific tagged ship once it's
            // close enough to be a real entity. Long-range point-to-point
            // travel is the M-key map + G-hold warp dash (see ui/mod.rs).
            .add_systems(Update, (
                bounty_nav::update_bounty_arrow,
                bounty_nav::spawn_bounty_markers,
                bounty_nav::update_bounty_markers,
            ).run_if(in_state(GameState::Exploring)))
            // Failure on game over
            .add_systems(OnEnter(GameState::GameOver), tracking::handle_contract_failure)
            // Ungated: a load from the menu lands docked or exploring, and
            // the system waits until there is a simulation to tag in.
            .add_systems(Update, tracking::reattach_bounties_after_load);
    }
}
