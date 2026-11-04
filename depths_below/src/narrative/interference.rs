//! The instruments stop being trustworthy.
//!
//! Ported from `src/parked/abyss_horror.rs`, which is 812 finished lines that
//! were compiled out because they depended on live creature AI and, with
//! creature spawning off, produced "false scares with nothing behind them".
//! Only some of that file needs creatures. The instrument, camera and tint
//! work does not, and under this story a reading with nothing behind it is
//! the point rather than the bug.
//!
//! One rule carried over from the original, and it is the whole craft of it:
//! **a fake reading is formatted exactly like a real one**. A depth that
//! scrambles to "ERR" while every other gauge reads normally is a UI bug. A
//! depth that quietly reads a plausible wrong number is a haunted instrument.
//! The formats below mirror what `update_hud` actually writes, which is not
//! what the parked file assumed — the HUD was redesigned since it was written,
//! and a straight port would have produced "Hull: ???" next to a gauge that
//! now renders "100%".
//!
//! One thing that fell out of this and is worth keeping: only the numbers are
//! overwritten, not the bars beneath them. So a corrupted readout shows 46%
//! hull above a full green bar, and the ship is visibly disagreeing with
//! itself rather than simply going dark. That is a better effect than a
//! blackout and it costs nothing — leave the bars alone.

use bevy::prelude::*;
use rand::Rng;

use crate::camera::CameraState;
use crate::states::GameState;
use crate::ui::{DepthText, FuelText, HullText, NoiseText, PowerText};

use super::CascadeState;

/// Cascade level at which the instruments first lie at all.
const FIRST_DOUBT: f32 = 0.30;

/// How often a glitch is attempted, and how long one lasts, by phase.
const ATTEMPT_EVERY: f32 = 11.0;

#[derive(Resource, Default)]
pub struct Interference {
    timer: f32,
    /// Seconds of corruption remaining. While positive the readouts lie.
    active: f32,
    /// 0 = honest, 1 = a single gauge drifts, 2 = several do, 3 = nothing reads true.
    pub phase: u8,
    /// A glitch something else asked for (a log just read, narrative::leads):
    /// seconds left, and how bad.
    forced: f32,
    forced_phase: u8,
}

impl Interference {
    /// Make the instruments lie for `secs`, at least as badly as `phase`,
    /// whatever the arc's level says.
    pub fn jolt(&mut self, secs: f32, phase: u8) {
        self.forced = secs;
        self.forced_phase = phase.min(3);
    }
}

fn phase_for(level: f32) -> u8 {
    if level < FIRST_DOUBT {
        0
    } else if level < 0.55 {
        1
    } else if level < 0.80 {
        2
    } else {
        3
    }
}

/// How near ships have to be to count as a crowd.
const CROWD_RANGE: f32 = 30_000.0;

/// Once the Preserve's specimen log has said it (Summit Record step 4: it
/// feeds on connection), the instruments act it out: no ships near, they stay
/// honest; the more ships near, the more often and the worse they lie.
/// Returns (attempt at all, extra chance, worst phase).
pub fn crowd_effect(crowd: usize) -> (bool, f32, u8) {
    let phase = if crowd >= 8 { 3 } else if crowd >= 3 { 2 } else { 1 };
    (crowd > 0, (crowd as f32 * 0.08).min(0.6), phase)
}

#[cfg(test)]
mod crowd_tests {
    use super::crowd_effect;

    /// Alone, honest; in a crowd, worse the bigger it is.
    #[test]
    fn crowds_feed_it() {
        assert!(!crowd_effect(0).0);
        assert!(crowd_effect(1).0);
        assert!(crowd_effect(3).2 > crowd_effect(1).2);
        assert!(crowd_effect(10).1 > crowd_effect(2).1);
        assert_eq!(crowd_effect(20).2, 3);
    }
}

#[allow(clippy::type_complexity)]
fn drive_interference(
    time: Res<Time>,
    cascade: Res<CascadeState>,
    mut state: ResMut<Interference>,
    contacts: Res<crate::diplomacy::contacts::Contacts>,
    player: Query<&GlobalTransform, With<crate::components::Ship>>,
    ships: Query<&GlobalTransform, (With<crate::ai_ship::components::AiShip>, Without<crate::ai_ship::components::AiShipWreck>)>,
) {
    if state.forced > 0.0 {
        state.forced -= time.delta_secs();
        state.phase = phase_for(cascade.level).max(state.forced_phase);
        state.active = state.forced.max(0.0);
        return;
    }
    // After step 4 the crowd around you decides how bad it gets (crowd_effect).
    let crowd = (contacts.summit.done >= 4).then(|| {
        let at = player.single().map(|t| t.translation().truncate()).unwrap_or_default();
        crowd_effect(ships.iter().filter(|t| t.translation().truncate().distance(at) < CROWD_RANGE).count())
    });
    state.phase = phase_for(cascade.level);
    if state.phase == 0 {
        state.active = 0.0;
        return;
    }
    state.phase = state.phase.max(crowd.map_or(0, |c| c.2));

    if state.active > 0.0 {
        state.active -= time.delta_secs();
        return;
    }

    state.timer += time.delta_secs();
    if state.timer < ATTEMPT_EVERY {
        return;
    }
    state.timer = 0.0;

    let mut rng = rand::thread_rng();
    // Alone, after step 4, they stay honest.
    if crowd.is_some_and(|c| !c.0) {
        return;
    }
    let extra = crowd.map_or(0.0, |c| c.1);
    // Longer and more certain the further along the run is. Phase 1 is a
    // flicker you might put down to your own eyes.
    let (chance, dur) = match state.phase {
        1 => (0.35, rng.gen_range(0.15..0.4)),
        2 => (0.55, rng.gen_range(0.4..1.0)),
        _ => (0.75, rng.gen_range(0.9..2.0)),
    };
    if rng.gen::<f32>() < (chance + extra).min(0.95) {
        state.active = dur;
    }
}

/// Overwrite the readouts while a glitch is live.
///
/// `update_hud` rewrites these every frame, so this has to run after it and
/// simply win. That also means nothing needs restoring: the moment the glitch
/// ends, the real value is back on the next frame by itself.
#[allow(clippy::type_complexity)]
fn corrupt_instruments(
    state: Res<Interference>,
    mut sets: ParamSet<(
        Query<&mut Text, With<DepthText>>,
        Query<&mut Text, With<HullText>>,
        Query<&mut Text, With<PowerText>>,
        Query<&mut Text, With<FuelText>>,
        Query<&mut Text, With<NoiseText>>,
    )>,
) {
    if state.active <= 0.0 || state.phase == 0 {
        return;
    }
    let mut rng = rand::thread_rng();

    // Phase 1: one gauge, plausible. The range readout is the right one to
    // start with — it is the number the player is steering by.
    if let Ok(mut t) = sets.p0().single_mut() {
        let fake = rng.gen_range(0.4..940.0) * 1000.0;
        **t = crate::ui::format_range_km(fake);
    }
    if state.phase == 1 {
        return;
    }

    // Phase 2: the ship starts disagreeing with itself. Still every value
    // shaped like a real one.
    if let Ok(mut t) = sets.p1().single_mut() {
        **t = format!("{}%", rng.gen_range(0..101));
    }
    if let Ok(mut t) = sets.p4().single_mut() {
        **t = format!("{}", rng.gen_range(0..900));
    }
    if state.phase == 2 {
        return;
    }

    // Phase 3: the formats survive, the numbers stop pretending.
    if let Ok(mut t) = sets.p2().single_mut() {
        **t = format!("{}/{}", rng.gen_range(0..2000), rng.gen_range(0..2000));
    }
    if let Ok(mut t) = sets.p3().single_mut() {
        **t = format!("{}%", rng.gen_range(0..400));
    }
}

/// A heartbeat in the camera, and the black getting a colour it should not
/// have. Both are things the parked layer already did; both are cheap because
/// `CameraState` and `ClearColor` are ordinary writable state.
fn unsettle_the_view(
    time: Res<Time>,
    state: Res<Interference>,
    mut camera: ResMut<CameraState>,
    mut clear: ResMut<ClearColor>,
) {
    if state.phase == 0 {
        return;
    }
    let t = time.elapsed_secs();

    // sin^4 gives a long trough and a short rise — a pulse rather than a wave.
    let beat = (t * 0.9).sin().powi(4);
    let strength = match state.phase {
        1 => 0.012,
        2 => 0.03,
        _ => 0.06,
    };
    camera.shake_intensity = camera.shake_intensity.max(beat * strength);

    // The void should look very slightly wrong before anything says so.
    let base = Color::srgb(0.05, 0.15, 0.35);
    let sick = Color::srgb(0.07, 0.11, 0.26);
    let mix = match state.phase {
        1 => 0.15,
        2 => 0.4,
        _ => 0.7,
    } * (0.75 + 0.25 * beat);
    let a = base.to_linear();
    let b = sick.to_linear();
    clear.0 = Color::linear_rgb(
        a.red + (b.red - a.red) * mix,
        a.green + (b.green - a.green) * mix,
        a.blue + (b.blue - a.blue) * mix,
    );
}

/// Put the void back when the run ends, or the main menu inherits the tint.
fn restore_view(mut clear: ResMut<ClearColor>) {
    clear.0 = Color::srgb(0.05, 0.15, 0.35);
}

pub struct InterferencePlugin;

impl Plugin for InterferencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Interference>()
            .add_systems(
                Update,
                (drive_interference, unsettle_the_view).run_if(in_state(GameState::Exploring)),
            )
            // After the HUD writes the true values, so the lie is what survives
            // to the frame. Nothing has to be restored afterwards.
            .add_systems(
                PostUpdate,
                corrupt_instruments.run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnExit(GameState::Exploring), restore_view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The instruments must be honest for the whole opening. If the first
    /// hour cannot be trusted, nothing later is a turn.
    #[test]
    fn the_instruments_start_honest() {
        assert_eq!(phase_for(0.0), 0);
        assert_eq!(phase_for(FIRST_DOUBT - 0.01), 0);
        assert!(FIRST_DOUBT > 0.25, "doubt starts too early to have been earned");
    }

    /// And they must get worse in order, reaching the top by the end.
    #[test]
    fn it_escalates_and_tops_out() {
        let phases: Vec<u8> = [0.0, 0.35, 0.6, 0.9, 1.0].iter().map(|l| phase_for(*l)).collect();
        for w in phases.windows(2) {
            assert!(w[1] >= w[0], "interference went backwards: {phases:?}");
        }
        assert_eq!(phase_for(1.0), 3, "the worst phase is never reached");
    }
}
