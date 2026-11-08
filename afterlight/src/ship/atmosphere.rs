use bevy::prelude::*;
use rand::Rng;

use crate::events::{ShowNotification, NotificationType};
use crate::narrative::CascadeState;

/// Tracks timing for atmospheric events
#[derive(Resource)]
pub struct AtmosphereState {
    pub timer: Timer,
    pub base_interval: f32,
    pub min_interval: f32,
}

impl Default for AtmosphereState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(30.0, TimerMode::Once),
            base_interval: 30.0,
            min_interval: 8.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AtmosphericEventType {
    HullCreaking,
    RadarGhost,
    InstrumentGlitch,
    HullBump,
    CosmicFlash,
    LightsFlicker,
    // --- the ones that are not the ship ---
    /// Traffic on a channel nobody is transmitting on.
    Crosstalk,
    /// The roster is the right length and one of the names is wrong.
    RosterDrift,
    /// An entry in the ship's own log that nobody remembers writing.
    OwnHandwriting,
}

impl AtmosphericEventType {
    /// How far into the run this becomes possible, 0.0 to 1.0.
    ///
    /// This used to gate on DepthState — radial distance from the world origin
    /// — which is a submarine-era measure that survived the conversion. It
    /// made the ramp meaningless in both directions: at Haven the player sits
    /// about a hundred units out so only the mildest event could ever fire,
    /// and every other system centres hundreds of thousands of units away, so
    /// arriving anywhere at all unlocked the whole table at once and pinned
    /// the interval to its floor. Cascade level is the same axis the rest of
    /// the story rides.
    fn min_cascade(&self) -> f32 {
        match self {
            Self::HullCreaking => 0.0,
            Self::HullBump => 0.10,
            Self::RadarGhost => 0.18,
            Self::CosmicFlash => 0.25,
            Self::InstrumentGlitch => 0.35,
            Self::LightsFlicker => 0.45,
            // Nothing below speaks in the ship's ordinary voice, and none of
            // it can happen in the first half of a run.
            Self::Crosstalk => 0.55,
            Self::RosterDrift => 0.70,
            Self::OwnHandwriting => 0.82,
        }
    }

    /// Weight for random selection (deeper events are rarer at their unlock depth)
    fn weight(&self) -> f32 {
        match self {
            Self::HullCreaking => 3.0,
            Self::RadarGhost => 2.0,
            Self::InstrumentGlitch => 1.5,
            Self::HullBump => 2.0,
            Self::CosmicFlash => 1.5,
            Self::LightsFlicker => 1.0,
            // Rarer than the mundane ones even once unlocked. These land
            // harder if they are not the thing that happens every minute.
            Self::Crosstalk => 0.8,
            Self::RosterDrift => 0.6,
            Self::OwnHandwriting => 0.5,
        }
    }

    fn notification_type(&self) -> NotificationType {
        match self {
            Self::HullCreaking => NotificationType::Warning,
            Self::RadarGhost => NotificationType::Warning,
            Self::InstrumentGlitch => NotificationType::Warning,
            Self::HullBump => NotificationType::Danger,
            Self::CosmicFlash => NotificationType::Info,
            Self::LightsFlicker => NotificationType::Warning,
            // Deliberately Info. A red alert would tell the player how to feel
            // about it; a routine-looking line they have to read twice is
            // worse.
            Self::Crosstalk => NotificationType::Info,
            Self::RosterDrift => NotificationType::Info,
            Self::OwnHandwriting => NotificationType::Info,
        }
    }

    fn random_message(&self, rng: &mut impl Rng) -> &'static str {
        match self {
            Self::HullCreaking => match rng.gen_range(0..4) {
                // Was "...under radiation stress": radiation damage is off,
                // so the line named a threat the ship can't actually face.
                0 => "The hull groans under some unseen strain...",
                1 => "Metal creaks ominously around you.",
                2 => "A deep, resonant groan echoes through the hull.",
                _ => "The bulkheads shudder with a low creak.",
            },
            Self::RadarGhost => match rng.gen_range(0..4) {
                0 => "Radar picks up a faint contact... then nothing.",
                1 => "A phantom blip appears on radar and vanishes.",
                2 => "Radar echo returns something massive... probably an asteroid.",
                _ => "Brief radar contact - too fast to identify.",
            },
            Self::InstrumentGlitch => match rng.gen_range(0..3) {
                0 => "Navigation instruments flicker momentarily.",
                1 => "Distance gauge spikes, then returns to normal.",
                _ => "Compass spins wildly for a second, then stabilizes.",
            },
            Self::HullBump => match rng.gen_range(0..4) {
                0 => "Something bumps against the hull!",
                1 => "A heavy thud reverberates through the ship.",
                2 => "Impact detected - external contact on the starboard side.",
                _ => "The ship shudders from an unseen collision.",
            },
            Self::CosmicFlash => match rng.gen_range(0..3) {
                0 => "A cascade of cosmic energy drifts past the viewport.",
                1 => "Strange luminous particles pulse in the darkness outside.",
                _ => "The void shimmers with an eerie blue-green light.",
            },
            Self::LightsFlicker => match rng.gen_range(0..3) {
                0 => "Interior lights flicker and dim briefly.",
                1 => "The lights cut out for a heartbeat, then return.",
                _ => "Electrical systems stutter - lights blink twice.",
            },
            Self::Crosstalk => match rng.gen_range(0..4) {
                0 => "Carrier tone on a channel nothing is transmitting on.",
                1 => "Comms logged four seconds of traffic. Origin: this vessel.",
                2 => "Someone used our callsign. The syntax was ours. The timing was not.",
                _ => "A transmission answered a question that has not been asked yet.",
            },
            Self::RosterDrift => match rng.gen_range(0..4) {
                0 => "Roster reconciled. Headcount correct. One name is not one of ours.",
                1 => "Duty log shows a shift worked by a hand who is off watch.",
                2 => "Bunk assignment updated. Nobody submitted the change.",
                _ => "A berth reads occupied. The berth is empty.",
            },
            Self::OwnHandwriting => match rng.gen_range(0..4) {
                0 => "New entry in the ship's log. Authored by this vessel. Not by anyone aboard.",
                1 => "A maintenance note has been filed ahead of the fault it describes.",
                2 => "The log contains an entry dated later than now. It is in our format.",
                _ => "Something has been writing in the record, and it writes the way we write.",
            },
        }
    }
}

const ALL_EVENTS: [AtmosphericEventType; 9] = [
    AtmosphericEventType::HullCreaking,
    AtmosphericEventType::RadarGhost,
    AtmosphericEventType::InstrumentGlitch,
    AtmosphericEventType::HullBump,
    AtmosphericEventType::CosmicFlash,
    AtmosphericEventType::LightsFlicker,
    AtmosphericEventType::Crosstalk,
    AtmosphericEventType::RosterDrift,
    AtmosphericEventType::OwnHandwriting,
];

pub fn atmospheric_event_system(
    time: Res<Time>,
    cascade: Res<CascadeState>,
    mut state: ResMut<AtmosphereState>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    state.timer.tick(time.delta());

    if !state.timer.just_finished() {
        return;
    }

    let mut rng = rand::thread_rng();
    let level = cascade.level.clamp(0.0, 1.0);

    let eligible: Vec<(AtmosphericEventType, f32)> = ALL_EVENTS
        .iter()
        .filter(|e| level >= e.min_cascade())
        .map(|e| (*e, e.weight()))
        .collect();

    if let Some(event) = weighted_pick(&eligible, &mut rng) {
        let message = event.random_message(&mut rng);
        notifications.write(ShowNotification {
            message: message.to_string(),
            notification_type: event.notification_type(),
            duration: 4.0,
        });
    }

    // The further along the run, the less quiet between them.
    let interval = state.base_interval + (state.min_interval - state.base_interval) * level;
    state.timer = Timer::from_seconds(interval, TimerMode::Once);
}

fn weighted_pick(
    items: &[(AtmosphericEventType, f32)],
    rng: &mut impl Rng,
) -> Option<AtmosphericEventType> {
    if items.is_empty() {
        return None;
    }
    let total: f32 = items.iter().map(|(_, w)| w).sum();
    let mut roll = rng.gen_range(0.0..total);
    for (event, weight) in items {
        roll -= weight;
        if roll <= 0.0 {
            return Some(*event);
        }
    }
    items.last().map(|item| item.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh run must only ever get the mildest event. The opening depends
    /// on nothing being wrong yet, and the old depth gate could not deliver
    /// that reliably: arriving in any system at all unlocked the whole table.
    #[test]
    fn the_opening_is_almost_quiet() {
        let open: Vec<_> = ALL_EVENTS.iter().filter(|e| e.min_cascade() <= 0.0).collect();
        assert_eq!(open.len(), 1, "more than one event can fire at cascade 0: {open:?}");
        assert_eq!(*open[0], AtmosphericEventType::HullCreaking);
    }

    /// Nothing that speaks in a voice other than the ship's may happen in the
    /// first half. Those lines are the only ones that say what is going on,
    /// and hearing them early gives the whole thing away.
    #[test]
    fn the_wrong_voices_wait_for_the_second_half() {
        for e in [
            AtmosphericEventType::Crosstalk,
            AtmosphericEventType::RosterDrift,
            AtmosphericEventType::OwnHandwriting,
        ] {
            assert!(e.min_cascade() > 0.5, "{e:?} can fire in the first half");
        }
    }

    /// Everything must become reachable by the end, or content was written
    /// that no player will ever see.
    #[test]
    fn everything_is_reachable_by_the_end() {
        for e in ALL_EVENTS.iter() {
            assert!(e.min_cascade() <= 1.0, "{e:?} is gated past the end of the run");
        }
        let at_full = ALL_EVENTS.iter().filter(|e| 1.0 >= e.min_cascade()).count();
        assert_eq!(at_full, ALL_EVENTS.len());
    }

    /// The table has to open up gradually rather than all at once.
    #[test]
    fn it_unlocks_in_stages() {
        let count_at = |l: f32| ALL_EVENTS.iter().filter(|e| l >= e.min_cascade()).count();
        let counts = [count_at(0.0), count_at(0.3), count_at(0.6), count_at(0.9)];
        for w in counts.windows(2) {
            assert!(w[1] >= w[0], "the table shrank: {counts:?}");
        }
        assert!(counts[3] > counts[0], "nothing ever unlocks: {counts:?}");
    }
}
