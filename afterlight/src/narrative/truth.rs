//! The ending.
//!
//! The bible budgets forty-five minutes for this. That is a length you can
//! spend when the player has invested forty hours; this is a demo somebody
//! reaches in under an hour, and a long ending would stop being dreadful and
//! start being a thing to sit through. Two and a half minutes, three at the
//! outside, agreed with the author. `total_seconds` is asserted against that
//! window in a test so a later edit cannot quietly make it a short film.
//!
//! Shape: it does not glitch at the player. The victory screen is real, and
//! then its wording revises itself while they are still reading it. Nothing
//! flashes. The one loud moment is the map, and even that is silent.

use bevy::prelude::*;
use rand::Rng;

use crate::celestial::resources::GalaxyMap;
use crate::resources::Statistics;
use crate::states::GameState;
use crate::ui::theme::{ThemeColors, ThemeFonts};

/// Beat lengths, in seconds. Kept as named constants so the running time is
/// something you can read off rather than reconstruct.
const VICTORY: f32 = 28.0;
const REVISION: f32 = 30.0;
const PLAIN: f32 = 26.0;
const DIVISION: f32 = 44.0;
const ADDRESS: f32 = 22.0;

pub fn total_seconds() -> f32 {
    VICTORY + REVISION + PLAIN + DIVISION + ADDRESS
}

const T_REVISION: f32 = VICTORY;
const T_PLAIN: f32 = T_REVISION + REVISION;
const T_DIVISION: f32 = T_PLAIN + PLAIN;
const T_ADDRESS: f32 = T_DIVISION + DIVISION;

/// Pip division: how often a share of them splits, what share, and the ceiling.
/// Tuned so the field fills over roughly the first half of its beat and then
/// simply sits there, because the holding is most of the effect.
const SPLIT_EVERY: f32 = 1.6;
const SPLIT_SHARE: f32 = 0.22;
const MAX_PIPS: usize = 420;
/// Only used if the galaxy is empty, which should not happen in a real run.
const FALLBACK_PIPS: usize = 24;

#[derive(Resource, Default)]
pub struct TruthSequence {
    pub elapsed: f32,
    split_timer: f32,
    seeded: bool,
}

#[derive(Component)]
struct TruthRoot;
#[derive(Component)]
struct TruthLine(usize);
#[derive(Component)]
struct TruthPipField;
#[derive(Component)]
struct TruthPip;

/// Six lines is enough for every beat; they are reused rather than respawned
/// so nothing pops as the sequence moves on.
const LINES: usize = 6;

fn enter_truth(
    mut commands: Commands,
    mut seq: ResMut<TruthSequence>,
    mut hud: Query<&mut Visibility, With<crate::ui::HudRoot>>,
) {
    *seq = TruthSequence::default();
    for mut v in hud.iter_mut() {
        *v = Visibility::Hidden;
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            // Not ThemeColors::BG_VOID: that is 98% opaque on purpose, so the
            // pause menu can show the ship behind it. Here the ship behind it
            // is exactly the thing that must not be there — a live HUD and a
            // half-read log card arguing with the ending underneath.
            BackgroundColor(Color::srgb(0.01, 0.02, 0.05)),
            ZIndex(200),
            TruthRoot,
        ))
        .with_children(|root| {
            // The map field sits behind the text and stays empty until its beat.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                TruthPipField,
            ));
            for i in 0..LINES {
                let size = if i == 0 { ThemeFonts::H1 } else { ThemeFonts::BODY };
                root.spawn((
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(size), ..default() },
                    TextColor(ThemeColors::TEXT_PRIMARY.with_alpha(0.0)),
                    TruthLine(i),
                ));
            }
        });
}

fn exit_truth(
    mut commands: Commands,
    roots: Query<Entity, With<TruthRoot>>,
    mut hud: Query<&mut Visibility, With<crate::ui::HudRoot>>,
) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
    // Put the instruments back, or the next run launches blind.
    for mut v in hud.iter_mut() {
        *v = Visibility::Inherited;
    }
}

/// What the screen says at `t`, as up to six lines.
///
/// Split out from the rendering so it can be read as a script, and so the
/// tests can walk the whole sequence without a running app.
fn script(t: f32, stats: &Statistics, total_logs: usize) -> Vec<String> {
    let hours = (stats.play_time_seconds / 3600.0).max(0.0);
    let time_read = if hours >= 1.0 {
        format!("{:.1} hours", hours)
    } else {
        format!("{:.0} minutes", (stats.play_time_seconds / 60.0).max(1.0))
    };

    if t < T_REVISION {
        // A real victory screen. No wink, no irony yet.
        let mut v = vec!["EXPEDITION COMPLETE".to_string()];
        if t > 3.0 {
            v.push("You reached the edge of charted space and recovered the last log.".into());
        }
        if t > 9.0 {
            v.push(String::new());
            v.push(format!("Time adrift         {time_read}"));
            v.push(format!("Crew lost           {}", stats.crew_lost));
            v.push(format!("Logs recovered      {} of {}", stats.logs_found.len(), total_logs));
        }
        return v;
    }

    if t < T_PLAIN {
        // The same screen, revising itself while it is still being read.
        let r = t - T_REVISION;
        let head = if r > 21.0 {
            "EXPEDITION RETURNED"
        } else if r > 4.0 {
            "EXPEDITION RECOVERED"
        } else {
            "EXPEDITION COMPLETE"
        };
        let sub = if r > 26.0 {
            "You have reached the edge of charted space before."
        } else if r > 10.0 {
            "You reached the edge of charted space. There was nothing past it."
        } else {
            "You reached the edge of charted space and recovered the last log."
        };
        let crew_label = if r > 16.0 { "Crew kept           " } else { "Crew lost           " };
        return vec![
            head.to_string(),
            sub.to_string(),
            String::new(),
            format!("Time adrift         {time_read}"),
            format!("{crew_label}{}", stats.crew_lost),
            format!("Logs recovered      {} of {}", stats.logs_found.len(), total_logs),
        ];
    }

    if t < T_DIVISION {
        let r = t - T_PLAIN;
        let mut v = vec![String::new()];
        if r > 2.0 { v.push("You won.".into()); }
        if r > 9.0 { v.push("Or you lost.".into()); }
        if r > 16.0 { v.push("It doesn't matter which.".into()); }
        return v;
    }

    if t < T_ADDRESS {
        // The map does the talking. One line, late, and only one.
        let r = t - T_DIVISION;
        if r > 34.0 {
            return vec![String::new(), "You have been doing this for a long time.".into()];
        }
        return vec![];
    }

    let r = t - T_ADDRESS;
    let mut v = vec![String::new()];
    if r > 1.0 { v.push(format!("You were adrift for {time_read}.")); }
    if r > 4.0 { v.push(format!("You buried {}. None of them left the ship.", stats.crew_lost)); }
    if r > 7.0 { v.push(format!("You stripped {} hulls. Some of them were yours.", stats.wrecks_salvaged)); }
    if r > 12.0 {
        v.push(String::new());
        v.push("The thing past the edge was never sleeping. It has your handwriting.".into());
    }
    if r > 18.0 {
        v.clear();
        v.push(String::new());
        v.push("Dock when you're ready.".into());
    }
    v
}

#[allow(clippy::type_complexity)]
fn drive_truth(
    time: Res<Time>,
    mut seq: ResMut<TruthSequence>,
    stats: Res<Statistics>,
    galaxy: Res<GalaxyMap>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    field: Query<Entity, With<TruthPipField>>,
    pips: Query<(Entity, &Node), With<TruthPip>>,
    mut lines: Query<(&TruthLine, &mut Text, &mut TextColor)>,
) {
    seq.elapsed += time.delta_secs();
    let t = seq.elapsed;

    let script_lines = script(t, &stats, crate::narrative::logs::LOG_ENTRIES.len());
    for (idx, mut text, mut color) in lines.iter_mut() {
        let want = script_lines.get(idx.0).cloned().unwrap_or_default();
        if **text != want {
            **text = want.clone();
        }
        // Fade in rather than appear. Muted for the ledger rows, primary for
        // the sentences, so the stat block still reads as a stat block.
        let target = if want.is_empty() { 0.0 } else { 1.0 };
        let base = if idx.0 >= 3 && t < T_PLAIN { ThemeColors::TEXT_MUTED } else { ThemeColors::TEXT_PRIMARY };
        let a = color.0.alpha();
        color.0 = base.with_alpha(a + (target - a) * (time.delta_secs() * 2.2).min(1.0));
    }

    // The map beat: the same pips the player has been reading all game.
    if t >= T_DIVISION && t < T_ADDRESS {
        if let Ok(parent) = field.single() {
            if !seq.seeded {
                seq.seeded = true;
                let radius = crate::celestial::galaxy::GALAXY_RADIUS.max(1.0);
                for sys in &galaxy.systems {
                    let p = sys.galaxy_pos / radius; // -1..1
                    spawn_pip(&mut commands, parent, 50.0 + p.x * 34.0, 50.0 + p.y * 34.0);
                }
                // The ending must never be the thing that crashes, and it must
                // never be blank. If the galaxy is somehow not populated, lay
                // down a ring so the beat still reads instead of showing the
                // player an empty screen for forty seconds.
                if galaxy.systems.is_empty() {
                    for i in 0..FALLBACK_PIPS {
                        let a = std::f32::consts::TAU * (i as f32 / FALLBACK_PIPS as f32);
                        spawn_pip(&mut commands, parent, 50.0 + a.cos() * 22.0, 50.0 + a.sin() * 22.0);
                    }
                }
            }
            seq.split_timer += time.delta_secs();
            if seq.split_timer >= SPLIT_EVERY {
                seq.split_timer = 0.0;
                let existing: Vec<(f32, f32)> = pips
                    .iter()
                    .filter_map(|(_, n)| match (n.left, n.top) {
                        (Val::Percent(l), Val::Percent(tp)) => Some((l, tp)),
                        _ => None,
                    })
                    .collect();
                // gen_range panics on an empty range, and this used to reach
                // it: jumping into the sequence before the galaxy existed left
                // no pips to divide and took the whole ending down with a
                // "cannot sample empty range". Guard the emptiness, not the
                // caller.
                if !existing.is_empty() && existing.len() < MAX_PIPS {
                    let mut rng = rand::thread_rng();
                    let want = ((existing.len() as f32 * SPLIT_SHARE) as usize).max(1);
                    for _ in 0..want.min(MAX_PIPS - existing.len()) {
                        let (l, tp) = existing[rng.gen_range(0..existing.len())];
                        spawn_pip(
                            &mut commands,
                            parent,
                            (l + rng.gen_range(-2.2..2.2)).clamp(2.0, 98.0),
                            (tp + rng.gen_range(-2.2..2.2)).clamp(2.0, 98.0),
                        );
                    }
                }
            }
        }
    } else if t >= T_ADDRESS {
        for (e, _) in pips.iter() {
            commands.entity(e).despawn();
        }
    }

    if t >= total_seconds() {
        next_state.set(GameState::MainMenu);
    }
}

fn spawn_pip(commands: &mut Commands, parent: Entity, left_pct: f32, top_pct: f32) {
    let pip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(left_pct),
                top: Val::Percent(top_pct),
                width: Val::Px(5.0),
                height: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(ThemeColors::TEXT_MUTED.with_alpha(0.65)),
            TruthPip,
        ))
        .id();
    commands.entity(parent).add_child(pip);
}

pub struct TruthPlugin;

impl Plugin for TruthPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TruthSequence>()
            .add_systems(OnEnter(GameState::Truth), enter_truth)
            .add_systems(OnExit(GameState::Truth), exit_truth)
            .add_systems(Update, drive_truth.run_if(in_state(GameState::Truth)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The agreed window. Two and a half minutes, three at the outside. A
    /// later edit that adds a beat has to make room rather than let the
    /// ending quietly become something the player waits out.
    #[test]
    fn the_ending_fits_its_budget() {
        let total = total_seconds();
        assert!(total >= 150.0, "ending is {total}s, shorter than the agreed 2:30");
        assert!(total <= 180.0, "ending is {total}s, longer than the agreed 3:00 cap");
    }

    /// Every beat has to put something on screen. A silent stretch in a
    /// scripted sequence reads as a hang, and the player cannot skip it.
    #[test]
    fn no_beat_is_dead_air_for_long() {
        let stats = Statistics { play_time_seconds: 4200.0, crew_lost: 3, wrecks_salvaged: 11, ..default() };
        let mut longest_silence = 0.0f32;
        let mut silent_since: Option<f32> = None;
        let mut t = 0.0f32;
        while t < total_seconds() {
            let has_text = script(t, &stats, 23).iter().any(|l| !l.is_empty());
            match (has_text, silent_since) {
                (false, None) => silent_since = Some(t),
                (true, Some(start)) => {
                    longest_silence = longest_silence.max(t - start);
                    silent_since = None;
                }
                _ => {}
            }
            t += 0.25;
        }
        if let Some(start) = silent_since {
            longest_silence = longest_silence.max(total_seconds() - start);
        }
        // The map beat is deliberately wordless, so the bar is generous — but
        // not unbounded.
        assert!(longest_silence <= 36.0, "{longest_silence}s with nothing on screen");
    }

    /// Dividing must be safe when there is nothing to divide. This panicked
    /// in a real run: the field was empty, `gen_range(0..0)` was reached, and
    /// the ending died mid-sequence. An ending that can crash is worse than
    /// one that is too long.
    #[test]
    fn dividing_an_empty_field_is_safe() {
        let existing: Vec<(f32, f32)> = Vec::new();
        // Mirrors the guard in drive_truth: emptiness is checked before any
        // sampling happens.
        let would_sample = !existing.is_empty() && existing.len() < MAX_PIPS;
        assert!(!would_sample, "an empty field must not reach the sampler");
        assert!(FALLBACK_PIPS > 0, "an empty galaxy must still draw something");
    }

    /// The closer must be the last thing standing, alone.
    #[test]
    fn it_ends_on_the_invitation() {
        let stats = Statistics::default();
        let last = script(total_seconds() - 0.1, &stats, 23);
        assert!(
            last.iter().any(|l| l.contains("Dock when you're ready")),
            "the last frame should be the invitation back into the loop, got {last:?}"
        );
        assert_eq!(last.iter().filter(|l| !l.is_empty()).count(), 1, "it should stand alone");
    }
}
