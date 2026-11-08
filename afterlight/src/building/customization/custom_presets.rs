use bevy::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use super::presets::Preset;

// ============================================================================
// CUSTOM PRESET LIBRARY
// Player-saved weapon builds. Saved under the same "weapon_<type>" key used
// by the built-in presets, so beginners and pros share one preset list —
// a pro's "crazy" build sits right next to the curated defaults.
// ============================================================================

#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct CustomPresetLibrary {
    pub presets: HashMap<String, Vec<Preset>>,
}

const CUSTOM_PRESETS_PATH: &str = "meta/custom_presets.json";

impl CustomPresetLibrary {
    pub fn next_build_name(&self, key: &str) -> String {
        let count = self.presets.get(key).map(|v| v.len()).unwrap_or(0);
        format!("My Build #{}", count + 1)
    }
}

/// Load saved custom presets from disk on startup
pub fn load_custom_presets(mut library: ResMut<CustomPresetLibrary>) {
    if let Ok(data) = std::fs::read_to_string(CUSTOM_PRESETS_PATH) {
        if let Ok(loaded) = serde_json::from_str::<CustomPresetLibrary>(&data) {
            *library = loaded;
            info!("Loaded custom presets from {}", CUSTOM_PRESETS_PATH);
        }
    }
}

/// Persist the custom preset library to disk immediately (called right after a save)
pub fn save_custom_presets(library: &CustomPresetLibrary) {
    let _ = std::fs::create_dir_all("meta");
    if let Ok(data) = serde_json::to_string_pretty(library) {
        let _ = std::fs::write(CUSTOM_PRESETS_PATH, data);
    }
}
