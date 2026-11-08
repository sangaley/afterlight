use bevy::prelude::*;

/// Main game states
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    Loading,
    StationDocked,  // At station, full building mode
    Exploring,      // In space, playing
    Docked,         // At outpost or wreck
    Paused,
    GameOver,
    /// The ending. Scripted, non-interactive, and its own state rather than a
    /// mode of GameOver, because GameOver owns the run-summary screen and this
    /// sequence has to be able to contradict it.
    Truth,
}

/// Sub-states for more granular control
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum BuildState {
    #[default]
    Inactive,
    Placing,            // Placing a new module
    Moving,             // Moving existing module
    Connecting,         // Connecting power/systems
    Deleting,           // Removing modules
    PlacingComponent,   // Placing components within a module's internal grid
    CustomizingPiece,   // Customizing a specific component piece
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShipSet {
    Input,
    Movement,
    Physics,
    Power,
    Heat,
    Oxygen,
    Hull,
    State,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CombatSet {
    WeaponFire,
    Cleanup,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RadarSet {
    Input,
    Update,
    Visibility,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpatialSet {
    /// Spatial hash grids are rebuilt here; anything doing "nearby entity"
    /// queries (creature AI perception, weapon targeting, collision) should
    /// run `.after(SpatialSet::Update)`.
    Update,
}
