use bevy::prelude::*;
use bevy::app::AppExit;

use crate::ai_ship::components::{AiShip, AiShipState};
use crate::components::{HullSegment, Module, Ship};

// ============================================================================
// PERFORMANCE READOUT
//
// What a frame costs, and what it is carrying, so ship size can be argued
// about with numbers. Two ways in:
//
//   F8                  — toggle the on-screen readout.
//   AFTERLIGHT_PERF=1       — also log one `PERF` line a second (fps, frame-time
//                          percentiles, entities, blocks by owner), for runs
//                          nobody is watching. tools/perf_summary.py reads them.
//   AFTERLIGHT_PERF_OVERLAY=1 — start with the readout showing (for screenshots).
//   AFTERLIGHT_PERF_CHURN=<n> — knock out n random blocks of YOUR ship a second
//                          (AFTERLIGHT_PERF_CHURN_TYPE=<ModuleType>: only that type;
//                          AFTERLIGHT_PERF_SCRAP=<n>: start with n ScrapMetal to rebuild)
//                          (health to 0, through the normal destruction
//                          path), after warm-up. A fight's worth of block loss
//                          on demand, without needing an enemy to get through
//                          the shield -- the cost a capital pays for being hit.
//   AFTERLIGHT_PERF_QUIT=<s> — print a `PERF SUMMARY` line and exit after s
//                          seconds, so two runs can be compared like for like.
//                          The first AFTERLIGHT_PERF_WARMUP seconds (default 10:
//                          shader compiles, first spawns) are left out of it.
//
// Two times per frame:
//   frame — wall clock between frames (what the player sees as fps). macOS
//           throttles a covered window to 60 Hz whatever vsync says, so an
//           unattended run's frame time can lock to 16.67 ms mid-run.
//   sim   — from the start of the First schedule to the end of Last: the
//           game's own systems, without the wait for the GPU or the display.
//           This is the number ship size moves, and it can't be throttled.
// Both come from the real clock, not the game clock: a paused or slowed game
// still costs what it costs.
// ============================================================================

pub struct PerfPlugin;

/// True for a perf run: AFTERLIGHT_PERF or AFTERLIGHT_PERF_QUIT is set. Such a run
/// turns vsync off (main.rs, and the settings system that would turn it
/// back on), because a frame held to the display's refresh hides its cost.
pub fn perf_mode() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("AFTERLIGHT_PERF").is_ok() || std::env::var("AFTERLIGHT_PERF_QUIT").is_ok())
}

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        let on = |k: &str| std::env::var(k).ok().as_deref() == Some("1");
        let quit_after = std::env::var("AFTERLIGHT_PERF_QUIT").ok().and_then(|v| v.parse::<f32>().ok());
        let warmup = std::env::var("AFTERLIGHT_PERF_WARMUP").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(10.0);
        app.insert_resource(PerfState {
            log: on("AFTERLIGHT_PERF") || quit_after.is_some(),
            overlay: on("AFTERLIGHT_PERF_OVERLAY"),
            quit_after,
            warmup,
            ..default()
        })
        .init_resource::<SimStart>()
        .add_systems(Startup, spawn_perf_overlay)
        .add_systems(First, mark_sim_start)
        .add_systems(Update, churn_player_blocks.run_if(|| churn_rate() > 0.0))
        .add_systems(Update, give_scrap.run_if(|| std::env::var("AFTERLIGHT_PERF_SCRAP").is_ok()))
        .add_systems(Last, (record_frame, report_perf).chain());
    }
}

#[derive(Resource, Default)]
pub struct PerfState {
    log: bool,
    overlay: bool,
    quit_after: Option<f32>,
    warmup: f32,
    /// Real seconds since startup.
    elapsed: f32,
    /// Seconds until the next once-a-second report.
    next_report: f32,
    /// This second's frame times, in milliseconds.
    window: Vec<f32>,
    /// Every frame time after warm-up, for the summary.
    all: Vec<f32>,
    /// The same two for sim time (First..Last).
    sim_window: Vec<f32>,
    sim_all: Vec<f32>,
    /// The worst one-second averages seen after warm-up.
    worst_second_ms: f32,
    worst_second_sim_ms: f32,
    /// Peak block counts seen, for the summary.
    peak_blocks: BlockCounts,
    peak_entities: usize,
    summarised: bool,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct BlockCounts {
    pub player: usize,
    pub ai: usize,
    pub wreck: usize,
}

impl BlockCounts {
    pub fn total(&self) -> usize {
        self.player + self.ai + self.wreck
    }
}

#[derive(Component)]
struct PerfOverlayText;

fn spawn_perf_overlay(mut commands: Commands, state: Res<PerfState>) {
    commands.spawn((
        PerfOverlayText,
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::srgb(0.75, 1.0, 0.75)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(8.0),
            bottom: Val::Px(8.0),
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        },
        GlobalZIndex(i32::MAX - 1),
        if state.overlay { Visibility::Visible } else { Visibility::Hidden },
    ));
}

fn churn_rate() -> f32 {
    static RATE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *RATE.get_or_init(|| std::env::var("AFTERLIGHT_PERF_CHURN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0))
}

/// AFTERLIGHT_PERF_CHURN: zero the health of random live blocks on the player
/// ship. Zero health is all it takes -- the ship's own damage systems mark
/// the block destroyed, tint it, and remove it, exactly as a hit would.
fn churn_player_blocks(
    time: Res<Time<Real>>,
    state: Res<PerfState>,
    mut owed: Local<f32>,
    player: Query<&Children, With<Ship>>,
    turret_decks: Query<&Children, With<crate::combat::turret_deck::TurretBody>>,
    mut hulls: Query<&mut HullSegment, Without<crate::components::HullDestroyed>>,
    mut modules: Query<&mut Module, Without<crate::components::DestroyedModule>>,
) {
    if state.elapsed < state.warmup {
        return;
    }
    let Ok(children) = player.single() else { return };
    *owed += churn_rate() * time.delta_secs();
    while *owed >= 1.0 {
        *owed -= 1.0;
        // AFTERLIGHT_PERF_CHURN_TYPE=<ModuleType>: only blocks of that type (a
        // test that needs one thing shot out, say a turret ring).
        let only = std::env::var("AFTERLIGHT_PERF_CHURN_TYPE").ok();
        // What stands on the ship's turret decks counts too.
        let live: Vec<Entity> = children
            .iter()
            .chain(children.iter().filter_map(|c| turret_decks.get(c).ok()).flat_map(|t| t.iter()))
            .filter(|c| match &only {
                Some(t) => modules.get(*c).is_ok_and(|m| m.health > 0.0 && format!("{:?}", m.module_type) == *t),
                None => hulls.get(*c).is_ok_and(|h| h.health > 0.0) || modules.get(*c).is_ok_and(|m| m.health > 0.0),
            })
            .collect();
        if live.is_empty() {
            return;
        }
        let pick = live[rand::random::<u32>() as usize % live.len()];
        if let Ok(mut h) = hulls.get_mut(pick) {
            h.health = 0.0;
        } else if let Ok(mut m) = modules.get_mut(pick) {
            m.health = 0.0;
        }
    }
}

/// AFTERLIGHT_PERF_SCRAP=<n>: put n ScrapMetal in the hold once, so the crew can
/// rebuild what churn knocks out.
fn give_scrap(mut inventory: ResMut<crate::resources::Inventory>, mut done: Local<bool>) {
    if *done {
        return;
    }
    *done = true;
    let n = std::env::var("AFTERLIGHT_PERF_SCRAP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    inventory.max_capacity = inventory.max_capacity.max(n as f32 * 5.0 + 100.0);
    inventory.add_item(crate::resources::ItemType::ScrapMetal, n);
}

/// When this frame's First schedule began.
#[derive(Resource, Default)]
struct SimStart(Option<std::time::Instant>);

fn mark_sim_start(mut start: ResMut<SimStart>) {
    start.0 = Some(std::time::Instant::now());
}

fn record_frame(time: Res<Time<Real>>, start: Res<SimStart>, mut state: ResMut<PerfState>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    state.elapsed += dt;
    let ms = dt * 1000.0;
    let sim = start.0.map_or(0.0, |t| t.elapsed().as_secs_f32() * 1000.0);
    state.window.push(ms);
    state.sim_window.push(sim);
    if state.elapsed > state.warmup {
        state.all.push(ms);
        state.sim_all.push(sim);
    }
}

/// (mean, p50, p95, p99, max) of a set of samples; sorts them in place.
fn stats(samples: &mut [f32]) -> (f32, f32, f32, f32, f32) {
    samples.sort_by(f32::total_cmp);
    let mean = samples.iter().sum::<f32>() / samples.len().max(1) as f32;
    (
        mean,
        percentile(samples, 0.5),
        percentile(samples, 0.95),
        percentile(samples, 0.99),
        samples.last().copied().unwrap_or(0.0),
    )
}

/// The value below which `p` of the samples fall. `sorted` must be ascending.
pub fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f32 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

#[allow(clippy::too_many_arguments)]
fn report_perf(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<PerfState>,
    entities: Query<Entity>,
    colliders: Query<(), With<crate::ship::collision::Collider>>,
    detached: Query<(), With<crate::combat::severance::DetachedSection>>,
    blocks: Query<&ChildOf, Or<(With<HullSegment>, With<Module>)>>,
    player: Query<(), With<Ship>>,
    ai: Query<&AiShipState, With<AiShip>>,
    mut overlay: Query<(&mut Text, &mut Visibility), With<PerfOverlayText>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::F8) {
        state.overlay = !state.overlay;
        for (_, mut vis) in overlay.iter_mut() {
            *vis = if state.overlay { Visibility::Visible } else { Visibility::Hidden };
        }
    }

    state.next_report -= time.delta_secs();
    if state.next_report > 0.0 {
        return;
    }
    state.next_report = 1.0;
    if !state.log && !state.overlay && state.quit_after.is_none() {
        state.window.clear();
        state.sim_window.clear();
        return;
    }

    let (avg, p50, p95, p99, max) = stats(&mut std::mem::take(&mut state.window));
    let (sim_avg, _, sim_p95, sim_p99, sim_max) = stats(&mut std::mem::take(&mut state.sim_window));
    let fps = if avg > 0.0 { 1000.0 / avg } else { 0.0 };

    let mut counts = BlockCounts::default();
    for parent in blocks.iter() {
        let owner = parent.parent();
        if player.contains(owner) {
            counts.player += 1;
        } else if let Ok(s) = ai.get(owner) {
            if s.is_destroyed { counts.wreck += 1 } else { counts.ai += 1 }
        }
    }
    let entity_count = entities.iter().count();

    if state.elapsed > state.warmup {
        state.worst_second_ms = state.worst_second_ms.max(avg);
        state.worst_second_sim_ms = state.worst_second_sim_ms.max(sim_avg);
        if counts.total() > state.peak_blocks.total() {
            state.peak_blocks = counts;
        }
        state.peak_entities = state.peak_entities.max(entity_count);
    }

    if state.log {
        info!(
            "PERF t={:.0} fps={:.1} avg={:.2}ms p50={:.2} p95={:.2} p99={:.2} max={:.2} sim_avg={:.2} sim_p95={:.2} sim_p99={:.2} sim_max={:.2} entities={} player={} ai={} wreck={} colliders={} detached={}",
            state.elapsed, fps, avg, p50, p95, p99, max, sim_avg, sim_p95, sim_p99, sim_max,
            entity_count, counts.player, counts.ai, counts.wreck, colliders.iter().count(), detached.iter().count()
        );
    }
    if state.overlay {
        for (mut text, _) in overlay.iter_mut() {
            text.0 = format!(
                "FPS {fps:.0}   frame {avg:.1} ms  (p99 {p99:.1}, max {max:.1})\n\
                 sim {sim_avg:.1} ms  (p99 {sim_p99:.1}, max {sim_max:.1})\n\
                 entities {entity_count}\n\
                 blocks: you {}  ai {}  wrecks {}",
                counts.player, counts.ai, counts.wreck
            );
        }
    }

    if let Some(limit) = state.quit_after {
        if state.elapsed >= limit && !state.summarised {
            state.summarised = true;
            let frames = state.all.len();
            let (avg, p50, p95, p99, max) = stats(&mut std::mem::take(&mut state.all));
            let (s_avg, s_p50, s_p95, s_p99, s_max) = stats(&mut std::mem::take(&mut state.sim_all));
            info!(
                "PERF SUMMARY frames={} avg={:.2}ms fps={:.1} p50={:.2} p95={:.2} p99={:.2} max={:.2} worst_second={:.2} sim_avg={:.2} sim_p50={:.2} sim_p95={:.2} sim_p99={:.2} sim_max={:.2} sim_worst_second={:.2} peak_entities={} player={} ai={} wreck={}",
                frames, avg, if avg > 0.0 { 1000.0 / avg } else { 0.0 }, p50, p95, p99, max, state.worst_second_ms,
                s_avg, s_p50, s_p95, s_p99, s_max, state.worst_second_sim_ms,
                state.peak_entities, state.peak_blocks.player, state.peak_blocks.ai, state.peak_blocks.wreck,
            );
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod perf_tests {
    use super::*;

    #[test]
    fn percentiles_pick_from_the_sorted_samples() {
        let s: Vec<f32> = (1..=100).map(|v| v as f32).collect();
        assert_eq!(percentile(&s, 0.0), 1.0);
        assert_eq!(percentile(&s, 0.5), 51.0);
        assert_eq!(percentile(&s, 0.99), 99.0);
        assert_eq!(percentile(&s, 1.0), 100.0);
        assert_eq!(percentile(&[], 0.5), 0.0);
    }
}
