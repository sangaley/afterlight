//! What the story remembers about a run.
//!
//! One small record of facts: the Abbess's first jobs, which way each piece
//! was taken, where the summit was, who has hailed you, what became of
//! Marlow. It rides on `Contacts` (diplomacy::contacts), beside the Summit
//! Record's progress, because `Contacts` already saves, loads and resets with
//! a run and the save system is at Bevy's parameter limit. Every field
//! defaults, so a save from before any of this loads as a run that simply
//! has nothing remembered yet.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ai_ship::components::AiShipType;

/// Where the deck chief is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarlowFate {
    /// A save from before he was aboard as a person: the ship speaks for
    /// itself.
    #[default]
    Unknown,
    Aboard,
    Dead,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoryState {
    /// The Abbess's first jobs finished (narrative::summit::ERRANDS of them).
    pub errands: u8,
    /// The Abbess's first jobs handed out, done or not: one ahead of
    /// `errands` while one is offered or in hand.
    pub errands_given: u8,
    /// How each finished step was taken: true is by force. One per step done.
    pub ways: Vec<bool>,
    /// Where the summit station was, in galaxy units, pinned when the black
    /// box says what happened there (step 2).
    pub site: Option<Vec2>,
    /// The Abbess has asked you to carry the pieces to the site.
    pub carry: bool,
    /// The last sequence has played.
    pub ending_seen: bool,
    /// Ids of the beats that have fired (narrative::beats).
    pub beats_fired: Vec<String>,
    /// Seconds of flight this run, as the beats count it.
    pub flight: f32,
    /// `flight` when the last beat fired.
    pub last_beat_at: f32,
    /// The old factions that have already called you the last one.
    pub heard_last_one: Vec<AiShipType>,
    pub marlow: MarlowFate,
    /// Ships you destroyed.
    pub ship_kills: u32,
    /// Times the dark reached you and threw you home.
    pub reached: u32,
}

impl StoryState {
    /// Which way step `n` (from 1) was taken: Some(true) by force,
    /// Some(false) by a deal, None if it isn't done or wasn't recorded.
    pub fn way(&self, n: u8) -> Option<bool> {
        (n >= 1).then(|| self.ways.get(n as usize - 1).copied()).flatten()
    }

    /// Note how step `n` was taken. Steps recorded out of order (an old save
    /// picked up mid-chain) pad with deals.
    pub fn record_way(&mut self, n: u8, forced: bool) {
        let i = n.max(1) as usize - 1;
        if self.ways.len() <= i {
            self.ways.resize(i + 1, false);
        }
        self.ways[i] = forced;
    }
}

#[cfg(test)]
mod story_tests {
    use super::*;
    use crate::diplomacy::contacts::Contacts;

    /// A save written before the story record existed loads with nothing
    /// remembered, and one written now comes back whole.
    #[test]
    fn an_old_save_loads_with_an_empty_story() {
        let old: Contacts = serde_json::from_str(r#"{"known":[]}"#).unwrap();
        assert_eq!(old.story, StoryState::default());
        // A record with only some of today's fields is filled out, not refused.
        let partial: Contacts = serde_json::from_str(r#"{"known":[],"story":{"errands":2}}"#).unwrap();
        assert_eq!(partial.story.errands, 2);
        assert_eq!(partial.story.marlow, MarlowFate::Unknown);

        let mut c = Contacts::default();
        c.story.errands = 3;
        c.story.record_way(2, true);
        c.story.site = Some(Vec2::new(4.0, -2.0));
        c.story.heard_last_one.push(AiShipType::GildedThrone);
        c.story.marlow = MarlowFate::Dead;
        let back: Contacts = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.story, c.story);
    }

    #[test]
    fn the_way_each_step_was_taken_is_kept_in_order() {
        let mut s = StoryState::default();
        assert_eq!(s.way(1), None);
        s.record_way(1, true);
        s.record_way(2, false);
        assert_eq!((s.way(1), s.way(2), s.way(3)), (Some(true), Some(false), None));
        // Out of order pads with deals instead of panicking.
        s.record_way(4, true);
        assert_eq!((s.way(3), s.way(4)), (Some(false), Some(true)));
        assert_eq!(s.way(0), None);
    }
}
