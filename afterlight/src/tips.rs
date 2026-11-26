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

/// A first-time tip, by its id in `data/marlow.json`: its words are data,
/// in Marlow's voice while he lives and the ship's own once he doesn't
/// (narrative::marlow).
struct Tip {
    id: &'static str,
}

const BREACH: Tip = Tip { id: "breach" };
const TURRET_DECK: Tip = Tip { id: "turret_deck" };
const BROWNOUT: Tip = Tip { id: "brownout" };

/// The tips this player has already been shown.
#[derive(Resource, Default, Serialize, Deserialize)]
pub struct TipsSeen(pub BTreeSet<String>);

impl TipsSeen {
    /// Marks the tip seen; true only the first time.
    fn first_time(&mut self, tip: &Tip) -> bool {
        self.0.insert(tip.id.to_string())
    }

    /// Queue the tip's card if the player hasn't had it yet.
    fn show(&mut self, tip: &Tip, queue: &mut LogQueue, marlow: bool) {
        let Some((title, text)) = crate::narrative::marlow::tip(tip.id, marlow) else { return };
        if !self.first_time(tip) {
            return;
        }
        queue.push_card(crate::narrative::marlow::kicker(marlow), title.to_string(), text.to_string());
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

/// Whether the tip is in Marlow's voice: only while he is aboard and alive.
fn marlow_speaks(contacts: Option<&crate::diplomacy::contacts::Contacts>) -> bool {
    contacts.is_some_and(crate::narrative::marlow::aboard)
}

/// The first breach in the player's own hull.
fn breach_tip(
    mut breaches: MessageReader<HullBreached>,
    tutorial: Res<Tutorial>,
    parents: Query<&ChildOf>,
    ships: Query<(), With<Ship>>,
    mut seen: ResMut<TipsSeen>,
    mut queue: ResMut<LogQueue>,
    contacts: Option<Res<crate::diplomacy::contacts::Contacts>>,
) {
    let ours = breaches
        .read()
        .any(|b| parents.get(b.segment).is_ok_and(|p| ships.contains(p.parent())));
    if ours && !training(&tutorial) {
        seen.show(&BREACH, &mut queue, marlow_speaks(contacts.as_deref()));
    }
}

/// The first time build mode goes up onto a turret deck.
fn turret_deck_tip(
    building: Res<BuildingState>,
    tutorial: Res<Tutorial>,
    mut seen: ResMut<TipsSeen>,
    mut queue: ResMut<LogQueue>,
    contacts: Option<Res<crate::diplomacy::contacts::Contacts>>,
) {
    if building.deck == BuildDeck::Turret && !training(&tutorial) {
        seen.show(&TURRET_DECK, &mut queue, marlow_speaks(contacts.as_deref()));
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
    contacts: Option<Res<crate::diplomacy::contacts::Contacts>>,
) {
    let short = power.is_some_and(|p| p.brownout) && *state.get() == GameState::Exploring;
    *since = if short { *since + time.delta_secs() } else { 0.0 };
    if *since >= BROWNOUT_SECS && !training(&tutorial) {
        seen.show(&BROWNOUT, &mut queue, marlow_speaks(contacts.as_deref()));
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
        seen.show(&BREACH, &mut queue, true);
        assert!(queue.pending.is_empty());
    }

    /// A tip is headed by whoever is there to give it: the chief, or with
    /// him dead, the ship.
    #[test]
    fn a_tip_is_in_the_voice_of_whoever_is_left() {
        for (marlow, who) in [(true, "CHIEF MARLOW"), (false, "HALYARD")] {
            let mut seen = TipsSeen::default();
            let mut queue = LogQueue::default();
            seen.0.insert("not_this_one".into());
            let (title, text) = crate::narrative::marlow::tip(BROWNOUT.id, marlow).unwrap();
            // Queue by hand what show() would, without writing meta/tips.json.
            assert!(seen.first_time(&BROWNOUT));
            queue.push_card(crate::narrative::marlow::kicker(marlow), title.to_string(), text.to_string());
            assert_eq!(queue.pending.len(), 1);
            assert_eq!(crate::narrative::marlow::kicker(marlow), who);
        }
    }
}
