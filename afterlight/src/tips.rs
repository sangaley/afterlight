//! First-time tips: the shakedown (crate::tutorial) only teaches the loop, so
//! Chief Marlow speaks up the first time the player meets one of the ship's
//! deeper systems. Each tip shows once per player, ever (meta/tips.json, so a
//! new expedition doesn't repeat them), and never during the shakedown.
//!
//! Missile defence has no tip on purpose: working out what stops a missile is
//! left to the player.

use std::collections::BTreeSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::Ship;
use crate::events::HullBreached;
use crate::narrative::reader::LogQueue;
use crate::resources::{BuildDeck, BuildingState, PowerChannels};
use crate::states::GameState;
use crate::tutorial::Tutorial;

const TIPS_PATH: &str = "meta/tips.json";

/// How long a brownout has to last before Marlow mentions it: a gun cycling
/// for a moment isn't worth a card.
const BROWNOUT_SECS: f32 = 3.0;

struct Tip {
    id: &'static str,
    title: &'static str,
    text: &'static str,
}

const BREACH: Tip = Tip {
    id: "breach",
    title: "We're holed",
    text: "Air's going out through the gap, and anyone in that room goes with it. Press [Q] to see the air. The crew will seal it if they can reach it, and the doors hold the rest of the ship. Keep that side out of the fight until they do.",
};

const TURRET_DECK: Tip = Tip {
    id: "turret_deck",
    title: "The turret deck",
    text: "Everything that turns sits up here. A gun is a firing piece with loaders behind it and barrel in front of it. Its shells come up the hoist from a magazine on the deck below. Leave turrets room to swing: crowd them and they jam on each other. [PgUp] again for the armoured roof.",
};

const BROWNOUT: Tip = Tip {
    id: "brownout",
    title: "Running short of power",
    text: "We're drawing more than the reactors make, so weapons, shields and engines are all running slow. [U] shows where the power goes. Back home, add reactors or take something off.",
};

/// The tips this player has already been shown.
#[derive(Resource, Default, Serialize, Deserialize)]
pub struct TipsSeen(pub BTreeSet<String>);

impl TipsSeen {
    /// Marks the tip seen; true only the first time.
    fn first_time(&mut self, tip: &Tip) -> bool {
        self.0.insert(tip.id.to_string())
    }

    /// Queue the tip's card if the player hasn't had it yet.
    fn show(&mut self, tip: &Tip, queue: &mut LogQueue) {
        if !self.first_time(tip) {
            return;
        }
        queue.push_card("CHIEF MARLOW", tip.title.to_string(), tip.text.to_string());
        let _ = std::fs::create_dir_all("meta");
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(TIPS_PATH, data);
        }
    }
}

pub struct TipsPlugin;

impl Plugin for TipsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TipsSeen>()
            .add_systems(Startup, load_tips)
            .add_systems(Update, (breach_tip, turret_deck_tip, brownout_tip));
    }
}

fn load_tips(mut seen: ResMut<TipsSeen>) {
    if let Some(loaded) = std::fs::read_to_string(TIPS_PATH).ok().and_then(|d| serde_json::from_str(&d).ok()) {
        *seen = loaded;
    }
}

fn training(tutorial: &Tutorial) -> bool {
    tutorial.active
}

/// The first breach in the player's own hull.
fn breach_tip(
    mut breaches: MessageReader<HullBreached>,
    tutorial: Res<Tutorial>,
    parents: Query<&ChildOf>,
    ships: Query<(), With<Ship>>,
    mut seen: ResMut<TipsSeen>,
    mut queue: ResMut<LogQueue>,
) {
    let ours = breaches
        .read()
        .any(|b| parents.get(b.segment).is_ok_and(|p| ships.contains(p.parent())));
    if ours && !training(&tutorial) {
        seen.show(&BREACH, &mut queue);
    }
}

/// The first time build mode goes up onto a turret deck.
fn turret_deck_tip(
    building: Res<BuildingState>,
    tutorial: Res<Tutorial>,
    mut seen: ResMut<TipsSeen>,
    mut queue: ResMut<LogQueue>,
) {
    if building.deck == BuildDeck::Turret && !training(&tutorial) {
        seen.show(&TURRET_DECK, &mut queue);
    }
}

/// The first brownout that lasts, in flight.
fn brownout_tip(
    time: Res<Time>,
    power: Option<Res<PowerChannels>>,
    state: Res<State<GameState>>,
    tutorial: Res<Tutorial>,
    mut since: Local<f32>,
    mut seen: ResMut<TipsSeen>,
    mut queue: ResMut<LogQueue>,
) {
    let short = power.is_some_and(|p| p.brownout) && *state.get() == GameState::Exploring;
    *since = if short { *since + time.delta_secs() } else { 0.0 };
    if *since >= BROWNOUT_SECS && !training(&tutorial) {
        seen.show(&BROWNOUT, &mut queue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tip comes up once, however often its moment comes round, and one
    /// tip seen doesn't use up another.
    #[test]
    fn each_tip_comes_up_once() {
        let mut seen = TipsSeen::default();
        assert!(seen.first_time(&BREACH));
        assert!(!seen.first_time(&BREACH));
        assert!(seen.first_time(&BROWNOUT));
    }

    /// One seen on an earlier run queues nothing (and so writes nothing).
    #[test]
    fn a_tip_seen_before_queues_nothing() {
        let mut seen = TipsSeen(BTreeSet::from([BREACH.id.to_string()]));
        let mut queue = LogQueue::default();
        seen.show(&BREACH, &mut queue);
        assert!(queue.pending.is_empty());
    }
}
