//! AUTOPLAY — unattended full-run director. Dev tooling, not a game feature.
//!
//! Where `demo.rs` flies at the nearest enemy and takes a photo every four
//! seconds, this plays the actual game: it sits through flight training, buys
//! a loadout, takes bounties off the board, hunts them down, strips the
//! wrecks, docks, turns in, and warps to the next system when the local board
//! runs dry — until a credit target is hit or the run wedges.
//!
//! **It plays through the real input layer.** Every decision comes out as a
//! synthetic `ButtonInput<KeyCode>` press (the same bridge `gamepad.rs` uses
//! for pad buttons) plus `InputState.gamepad_aim` for facing. Nothing here
//! writes game state directly, so a run exercises the same code a human does
//! — if the director can't do something, a player with a controller can't
//! either, and that's a finding rather than a harness bug.
//!
//! Env:
//!   AFTERLIGHT_AUTOPLAY=1              — arm the director
//!   AFTERLIGHT_AUTOPLAY_TARGET=15000   — credits to stop at (default 15000)
//!   AFTERLIGHT_AUTOPLAY_DIR=<path>     — journal + screenshots (default /tmp/afterlight_run)
//!   AFTERLIGHT_AUTOPLAY_MAXSPEED=6     — max time dilation on empty transits (default 6)
//!   AFTERLIGHT_AUTOPLAY_DEADLINE=7200  — wall-clock seconds before giving up (default 7200)

use bevy::ecs::system::SystemParam;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::ai_ship::components::{AiShip, AiShipWreck, SimBehavior, WorldSimulation};
use crate::celestial::poi::{SpacePoi, SpacePoiType};
use crate::combat::targeting::selection::TargetSelection;
use crate::components::{DockingMenuSelection, DockingOverlay, Ship, Velocity, Wreck};
use crate::contracts::{ContractObjective, ContractState, ContractStatus, MissionBoardOpen};
use crate::crew::eva_salvage::EvaSalvaging;
use crate::events::{AiShipDestroyed, NotificationType, ShowNotification};
use crate::resources::{Currency, FuelState, HullState, InputState, StaffingState};
use crate::states::{BuildState, GameState};
use crate::celestial::components::WarpCharging;
use crate::celestial::resources::{
    GalaxyMap, GalaxyWarpTarget, PendingGalaxyWarpTarget, SystemStreamingManager,
};
use crate::ui::{MapWarpCharging, PendingWarpTarget};
use crate::tutorial::{Advance, Tutorial};
use crate::world::home_base::{SystemStations, DOCK_RANGE};

// ============================================================================
// TUNING
// ============================================================================

/// Where the director wants to sit while fighting. The starter battery reaches
/// 2200 (gatling) to 9600 (heavy missile), and `auto_engage` will pick blocks
/// out to 9000, so parking well outside knife range still puts most guns on
/// target. The old 750 flew the ship through its whole envelope without firing
/// and then rammed — momentum is forever in this game, so closing to contact
/// is a decision, not a default.
const STANDOFF: f32 = 2600.0;
/// Open fire at this range. Past the gatling's reach, comfortably inside the
/// cannon/railgun/missile envelope.
const FIRE_RANGE: f32 = 5000.0;
/// Closing faster than this inside standoff means retro-thrust, or we overshoot
/// and end up hull-to-hull.
const CLOSING_SPEED_CAP: f32 = 240.0;
/// Break off and run for a station below this hull fraction.
const RETREAT_HULL: f32 = 0.35;
/// Loot interact range (poi.rs loots derelicts/anomalies within 1400).
const INTERACT_RANGE: f32 = 1200.0;
/// Where to park to work a wreck. eva_salvage dispatches inside ORDER_RANGE
/// (3000) and breaks the detail off past BREAK_RANGE (4500 from the rim).
/// Sitting close still matters: every unit of standoff is a longer spacewalk.
const SALVAGE_HOLD: f32 = 1300.0;
/// Drift past this while a detail is out and we go back to closing the gap.
const SALVAGE_LEASH: f32 = 1650.0;
/// Give up on a wreck after this long rather than parking there forever.
const SALVAGE_TIMEOUT: f32 = 150.0;
/// No credit movement for this long (game seconds) → log a stall and re-plan.
const STALL_SECONDS: f32 = 240.0;
/// How far off course we'll go for a wreck or a lootable POI. Hunt and Strip
/// MUST read the same number: they used to disagree by a thousand units, and
/// anything sitting in that band made Hunt divert to Strip, Strip decide it
/// was too far and bounce back, forever — six phase flips in twelve seconds.
const DIVERT_RANGE: f32 = 9000.0;
/// How long to sweep an apparently empty system before writing it off. Was
/// three minutes, which is three minutes of flying in a straight line past
/// nothing once the local board is stripped — the ship has an interstellar
/// drive for exactly this, so give up early and use it.
const SWEEP_SECONDS: f32 = 45.0;
/// Burn or jump? The dash always stops WARP_DASH_ARRIVAL_BUFFER (3000) short
/// of its mark and costs a flat two seconds to spin up, so a short hop buys
/// almost no ground for the wait. Past this it wins outright — and on fuel
/// too: a jump is 1 unit per 1000 travelled, where the main drive burns that
/// in a couple of seconds of thrust.
const WARP_MIN_DISTANCE: f32 = 7000.0;
/// Matches ui::WARP_DASH_ARRIVAL_BUFFER — how far short the drive drops us.
const WARP_ARRIVAL_BUFFER: f32 = 3000.0;
/// Fuel kept back from any jump, so a dash can never strand us out here with
/// nothing left to fly home on.
const WARP_FUEL_RESERVE: f32 = 250.0;
/// A refused jump (no destination, too close, drive busy) never spawns the
/// charge component. Give up holding G after this and fly it the slow way.
const WARP_GIVE_UP: f32 = 3.0;
/// A wreck this close to one already written off counts as that wreck.
const SPENT_SPOT_RADIUS: f32 = 600.0;
/// Minimum gap between screenshots, so a chatty phase can't fill the disk.
const SHOT_COOLDOWN: f32 = 3.0;
/// Hard cap on saved frames. At one every few seconds a two-hour run can write
/// well over a gigabyte of PNGs, and this machine ran itself out of disk mid-
/// session doing exactly that — which took the compiler down with it.
const MAX_SHOTS: u32 = 400;

// ============================================================================
// STATE
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Main menu — start a new expedition (which arms the tutorial).
    Boot,
    /// Sit through flight training, doing what each step asks.
    Training,
    /// Docked: spend credits on a fighting loadout.
    Outfit,
    /// Docked: open the board and take everything affordable.
    Board,
    /// Docked: launch into open space.
    Launch,
    /// Fly to the objective and kill it.
    Hunt,
    /// Strip the wreck / loot the derelict we're parked on.
    Strip,
    /// Head home, dock, turn in, sell.
    Home,
    /// Docked: work the station services before touching the mission board.
    Trade,
    /// Local space is stripped — charge the interstellar drive and move on.
    Jump,
    /// Credit target reached.
    Done,
    /// Deadline or unrecoverable stall.
    Aborted,
}

#[derive(Resource)]
pub struct Director {
    pub phase: Phase,
    /// AFTERLIGHT_AUTOPLAY_PHASE=<Phase>@<secs>: idle until then, then start in
    /// that phase -- to film one behaviour (Home: fly in and dock) alone.
    forced_start: Option<(Phase, f32)>,
    phase_elapsed: f32,
    run_elapsed: f32,
    /// Keys held down this frame.
    held: HashSet<KeyCode>,
    /// Keys to flash for exactly one frame (so `just_pressed` fires once).
    taps: Vec<KeyCode>,
    /// What we pressed last frame, so we know what to release.
    emulated: HashSet<KeyCode>,
    /// Facing, fed to InputState.gamepad_aim.
    aim: Option<Vec2>,
    target: u32,
    deadline: f32,
    max_speed: f32,
    dir: String,
    journal: Option<File>,
    shots: u32,
    shot_cooldown: f32,
    last_credits: u32,
    stall_timer: f32,
    kills: u32,
    /// Guards one-shot work inside a phase (outfit buys, board sweep).
    step: u32,
    /// Tutorial info-steps need Space on a rhythm, not every frame.
    beat: f32,
    /// Frame delta, so phases can run their own timers.
    dt: f32,
    /// Salvage is dispatch-and-wait: F sends a detail out and pressing it
    /// again RECALLS them. These track that handshake so we press it once.
    detail_sent: bool,
    detail_seen: bool,
    strip_wait: f32,
    loot_cooldown: f32,
    /// Wrecks we tried and got nothing from. `Wreck::loot_remaining` already
    /// filters stripped hulks, but a wreck can also refuse a detail because
    /// there is no idle crew — without remembering those, Hunt sees the wreck,
    /// diverts to Strip, fails, and bounces straight back forever.
    spent_wrecks: HashSet<Entity>,
    /// Where the written-off wrecks were. Chunk-layer wrecks (Haven's approach
    /// is full of them) despawn and respawn as new entities when their chunk
    /// reloads, so the entity set forgot them and the director looped:
    /// launch, see the "new" wreck, press F, dock, trade, launch.
    spent_spots: Vec<Vec2>,
    /// The wreck the last F press was aimed at, so a press the game spent on
    /// docking instead can write that wreck off.
    strip_wreck: Option<Entity>,
    strip_spot: Option<Vec2>,
    /// Grace period after leaving Strip so a bad wreck can't re-trigger it on
    /// the very next frame.
    strip_cooldown: f32,
    /// A capture came back blank and is owed a retake.
    shot_pending: bool,
    shot_retries: u32,
    blanks_seen: u32,
    /// Seconds until the next periodic status line.
    status_in: f32,
    /// How long we've been holding station for an EVA detail.
    eva_hold: f32,
    /// At a settlement berth: Up has been pressed onto Undock, Enter is next.
    undock_armed: bool,
    /// Training's crew lesson opened the crew window; close it before going
    /// on, as a player would, or it sat over a third of every later frame.
    crew_window_open: bool,
    /// How long the current recovered log has been up unread.
    log_up_for: f32,
    /// Station discs (centre, keep-out radius) in the loaded system, refreshed
    /// each frame so fly_to can route round them instead of grinding into a
    /// kilometres-wide hull -- one run lost an armour plate doing exactly that.
    obstacles: Vec<(Vec2, f32)>,
    /// A recall has gone out to the detail currently on the hull.
    recall_sent: bool,
    /// Seconds spent searching this system with nothing to fight. Kept across
    /// Hunt/Strip hops: on phase_elapsed, every detour to a nearby POI reset
    /// the sweep clock, so a system was never declared dry and the run
    /// circled Haven for its whole deadline.
    search_for: f32,
    /// Hunt swept the system and found nothing. Cleared once we've jumped.
    system_dry: bool,
    /// How long the interstellar drive has been charging.
    jump_for: f32,
    jump_seen: bool,
    /// Mid-jump: G is being held and the phase logic is suspended until the
    /// drive fires. Everything else waits — steering during a charge is how
    /// you cancel it by accident.
    warping: bool,
    warp_for: f32,
    /// The charge component actually appeared, so its disappearance means the
    /// jump completed rather than the keypress having been refused outright.
    warp_seen: bool,
}

impl Director {
    fn tap(&mut self, key: KeyCode) {
        self.taps.push(key);
    }
    fn hold(&mut self, key: KeyCode) {
        self.held.insert(key);
    }

    /// Structured line into the run journal. This is the artifact a human
    /// actually reads afterwards, so it gets timestamps and phase context.
    fn log(&mut self, kind: &str, msg: &str) {
        let line = format!(
            "{{\"t\":{:.1},\"phase\":\"{:?}\",\"kind\":\"{}\",\"msg\":{}}}",
            self.run_elapsed,
            self.phase,
            kind,
            json_string(msg)
        );
        info!("[AUTOPLAY] {} {}", kind, msg);
        if let Some(f) = self.journal.as_mut() {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
        }
    }

    fn go(&mut self, phase: Phase) {
        if self.phase == phase {
            return;
        }
        let msg = format!("{:?} -> {:?}", self.phase, phase);
        self.log("phase", &msg);
        self.phase = phase;
        self.phase_elapsed = 0.0;
        self.step = 0;
        self.detail_sent = false;
        self.detail_seen = false;
        self.strip_wait = 0.0;
        // Changing phase IS progress. Carrying the clock across meant the
        // watchdog fired inside a station phase that legitimately earns
        // nothing yet, and yanked the director out mid-sequence.
        self.stall_timer = 0.0;
    }
}

/// Minimal JSON string escaping — notification text is game-authored and can
/// contain quotes, and a malformed journal is worse than no journal.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct AutoplayPlugin;

impl Plugin for AutoplayPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("AFTERLIGHT_AUTOPLAY").ok().as_deref() != Some("1") {
            return;
        }

        let dir = std::env::var("AFTERLIGHT_AUTOPLAY_DIR")
            .unwrap_or_else(|_| "/tmp/afterlight_run".to_string());
        let _ = std::fs::create_dir_all(&dir);
        let journal = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(format!("{}/journal.jsonl", dir))
            .ok();

        let target = env_f32("AFTERLIGHT_AUTOPLAY_TARGET", 15000.0) as u32;
        let mut director = Director {
            phase: Phase::Boot,
            forced_start: std::env::var("AFTERLIGHT_AUTOPLAY_PHASE").ok().and_then(|v| {
                let (name, at) = v.split_once('@').unwrap_or((v.as_str(), "0"));
                let phase = match name {
                    "Home" => Phase::Home,
                    "Hunt" => Phase::Hunt,
                    "Jump" => Phase::Jump,
                    "Launch" => Phase::Launch,
                    _ => return None,
                };
                Some((phase, at.parse().unwrap_or(0.0)))
            }),
            phase_elapsed: 0.0,
            run_elapsed: 0.0,
            held: HashSet::new(),
            taps: Vec::new(),
            emulated: HashSet::new(),
            aim: None,
            target,
            deadline: env_f32("AFTERLIGHT_AUTOPLAY_DEADLINE", 7200.0),
            max_speed: env_f32("AFTERLIGHT_AUTOPLAY_MAXSPEED", 6.0),
            dir,
            journal,
            shots: 0,
            shot_cooldown: 0.0,
            last_credits: 0,
            stall_timer: 0.0,
            system_dry: false,
            jump_for: 0.0,
            jump_seen: false,
            warping: false,
            warp_for: 0.0,
            warp_seen: false,
            kills: 0,
            step: 0,
            beat: 0.0,
            dt: 0.0,
            detail_sent: false,
            detail_seen: false,
            strip_wait: 0.0,
            loot_cooldown: 0.0,
            spent_wrecks: HashSet::new(),
            spent_spots: Vec::new(),
            strip_wreck: None,
            strip_spot: None,
            strip_cooldown: 0.0,
            shot_pending: false,
            shot_retries: 0,
            blanks_seen: 0,
            status_in: 0.0,
            eva_hold: 0.0,
            undock_armed: false,
            crew_window_open: false,
            log_up_for: 0.0,
            search_for: 0.0,
            obstacles: Vec::new(),
            recall_sent: false,
        };
        director.log("start", &format!("target {}c", target));

        info!("AUTOPLAY: unattended run armed, target {}c", target);

        app.insert_resource(director)
            // After Bevy's own input pass, so emulated presses get correct
            // just_pressed semantics — same reasoning as gamepad.rs.
            .add_systems(PreUpdate, apply_synthetic_input.after(InputSystems))
            .add_systems(
                Update,
                (
                    director_clock,
                    director_brain,
                    director_timescale,
                    director_status,
                    director_watch,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                apply_aim
                    .in_set(crate::states::ShipSet::Movement)
                    .before(crate::ship::ship_movement),
            );
    }
}

// ============================================================================
// INPUT BRIDGE
// ============================================================================

/// Presses/releases the keys the brain asked for. Held keys are pressed every
/// frame (idempotent); taps land for one frame and are released the next, so
/// each tap produces exactly one `just_pressed`.
fn apply_synthetic_input(mut keyboard: ResMut<ButtonInput<KeyCode>>, mut d: ResMut<Director>) {
    let mut desired: HashSet<KeyCode> = d.held.clone();
    for key in d.taps.drain(..) {
        desired.insert(key);
    }

    let stale: Vec<KeyCode> = d.emulated.difference(&desired).copied().collect();
    for key in stale {
        keyboard.release(key);
    }
    for key in desired.iter() {
        keyboard.press(*key);
    }

    d.emulated = desired;
    d.held.clear();
}

/// Facing goes through the controller aim path — ship_movement and the
/// dumb-fire weapons both already read it, so nothing here special-cases
/// autoplay.
fn apply_aim(mut input_state: ResMut<InputState>, d: Res<Director>) {
    if let Some(aim) = d.aim {
        input_state.gamepad_aim = Some(aim);
    }
}

// ============================================================================
// CLOCK / BOOKKEEPING
// ============================================================================

fn director_clock(time: Res<Time<Real>>, mut d: ResMut<Director>) {
    let dt = time.delta_secs();
    d.run_elapsed += dt;
    d.phase_elapsed += dt;
    d.shot_cooldown = (d.shot_cooldown - dt).max(0.0);
    d.loot_cooldown = (d.loot_cooldown - dt).max(0.0);
    d.strip_cooldown = (d.strip_cooldown - dt).max(0.0);
    d.beat += dt;
    d.dt = dt;
}

// ============================================================================
// THE BRAIN
// ============================================================================

#[derive(SystemParam)]
struct World1<'w, 's> {
    ships: Query<'w, 's, (&'static Transform, &'static Velocity), With<Ship>>,
    enemies: Query<'w, 's, (Entity, &'static Transform), (With<AiShip>, Without<AiShipWreck>, Without<Ship>)>,
    wrecks: Query<'w, 's, (Entity, &'static Transform, &'static Wreck), Without<Ship>>,
    pois: Query<'w, 's, (&'static Transform, &'static SpacePoi), Without<Ship>>,
    eva: Query<'w, 's, (), With<EvaSalvaging>>,
    dock_menu: Query<'w, 's, &'static DockingMenuSelection, With<DockingOverlay>>,
    log_card: Query<'w, 's, &'static Visibility, With<crate::narrative::reader::LogCardRoot>>,
    records: Query<'w, 's, (&'static Transform, &'static crate::components::LogEntry), Without<Ship>>,
}

/// Both drives, bundled: the G-key local dash and the V-key interstellar
/// jump. Grouped into one SystemParam so director_brain stays under Bevy's
/// 16-parameter ceiling.
#[derive(SystemParam)]
struct Warp1<'w, 's> {
    local_target: ResMut<'w, PendingWarpTarget>,
    local_charging: Query<'w, 's, (), With<MapWarpCharging>>,
    galaxy_target: ResMut<'w, PendingGalaxyWarpTarget>,
    galaxy_charging: Query<'w, 's, (), With<WarpCharging>>,
    galaxy_map: Res<'w, GalaxyMap>,
    streaming: Res<'w, SystemStreamingManager>,
    /// Where accepted bounties' ships actually are (see bounty_lead).
    sim: Res<'w, WorldSimulation>,
    /// The next expedition record and the system it's in (narrative::trail).
    expedition: Res<'w, crate::narrative::expedition::Expedition>,
}

/// The system and position of the ship an accepted, unfinished bounty wants
/// dead. Without this the director took bounties off the board and then
/// patrolled whatever system it was in, so three runs in a row ended with
/// one kill - the training raider - and combat went untested.
fn bounty_lead(contracts: &ContractState, sim: &WorldSimulation) -> Option<(u32, Vec2)> {
    contracts
        .active_contracts
        .iter()
        .filter(|c| c.status != ContractStatus::Completed)
        .find_map(|c| match c.objective {
            ContractObjective::DestroyShip { target_id, destroyed: false, .. } => sim
                .ships
                .iter()
                .find(|s| s.bounty_id == Some(target_id) && s.behavior != SimBehavior::Dead)
                .map(|s| (s.system_id, s.position)),
            _ => None,
        })
}

fn director_brain(
    mut d: ResMut<Director>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
    tutorial: Res<Tutorial>,
    currency: Res<Currency>,
    hull: Res<HullState>,
    fuel: Res<FuelState>,
    contracts: Res<ContractState>,
    board_open: Res<MissionBoardOpen>,
    stations: Res<SystemStations>,
    selection: Res<TargetSelection>,
    build_state: Res<State<BuildState>>,
    mut warp: Warp1,
    w: World1,
    board: (Res<crate::contracts::ViewingStation>, Res<crate::contracts::ui::MissionBoardSelection>),
) {
    if matches!(d.phase, Phase::Done | Phase::Aborted) {
        return;
    }
    d.obstacles = stations.sites.iter().map(|s| (s.pos, s.radius() * 1.1 + 1_300.0)).collect();

    // ---- goal check -------------------------------------------------------
    if currency.credits >= d.target {
        let msg = format!(
            "target reached: {}c after {:.0}s, {} kills, {} frames saved ({} blank)",
            currency.credits,
            d.run_elapsed,
            d.kills,
            SAVED_CAPTURES.load(Ordering::Relaxed),
            BLANK_CAPTURES.load(Ordering::Relaxed)
        );
        d.log("done", &msg);
        d.go(Phase::Done);
        return;
    }
    if d.run_elapsed > d.deadline {
        let msg = format!(
            "deadline hit at {}c, {} kills, {} frames saved ({} blank)",
            currency.credits,
            d.kills,
            SAVED_CAPTURES.load(Ordering::Relaxed),
            BLANK_CAPTURES.load(Ordering::Relaxed)
        );
        d.log("abort", &msg);
        d.go(Phase::Aborted);
        return;
    }

    // ---- credit-progress watchdog ----------------------------------------
    if currency.credits != d.last_credits {
        if currency.credits > d.last_credits {
            let msg = format!("+{}c (now {})", currency.credits - d.last_credits, currency.credits);
            d.log("credits", &msg);
        }
        d.last_credits = currency.credits;
        d.stall_timer = 0.0;
    }

    // StationDocked is the one that has build mode, the board and hiring.
    // `Docked` is a different thing entirely (outpost/wreck), and Loading /
    // Paused are transients. Treating anything non-Exploring as "not docked"
    // is what made the director thrash Outfit -> Hunt -> Boot in three frames
    // the moment it touched an outpost.
    let at_station = *state.get() == GameState::StationDocked;
    let flying = *state.get() == GameState::Exploring;

    // A recovered log stays up until [Space]. Give it a reader's pause, then
    // close it like a player would -- left alone it covered the middle of
    // the screen for the rest of the run.
    let log_up = w.log_card.iter().any(|v| *v != Visibility::Hidden);
    d.log_up_for = if log_up { d.log_up_for + d.dt } else { 0.0 };
    if d.log_up_for > 8.0 {
        d.log_up_for = 0.0;
        d.log("log", "read the recovered log - closing it");
        d.tap(KeyCode::Space);
    }

    // Crew outside pins the ship. A detail breaks off past BREAK_RANGE, so
    // flying anywhere with people on the hull strands them — which is exactly
    // how an earlier run lost a detail six crew at a time while it hurried off
    // to dock. Hold station and let them finish.
    if flying && !w.eva.is_empty() {
        // Record that a detail really did go out. Strip's completion test runs
        // below this guard and would otherwise never observe it, and would
        // write the wreck off as "no detail launched" the moment they landed.
        d.detail_seen = true;
        d.eva_hold += d.dt;

        if let Ok((_, vel_c)) = w.ships.single() {
            if vel_c.0.length() > 30.0 {
                d.hold(KeyCode::ShiftLeft);
            }
        }

        // A detail that never comes home would pin the ship for the rest of
        // the run. Recall them the way a player would and carry on -- at once
        // when training is waiting on the dock lesson, which says docking
        // recalls them; the crew can strip plating for minutes otherwise.
        let dock_lesson = tutorial.pending() == Some(Advance::Dock);
        if !d.recall_sent && (d.eva_hold > 150.0 || dock_lesson) && d.beat > 1.5 {
            d.beat = 0.0;
            d.eva_hold = 0.0;
            // Once: F again before they're aboard would only re-send the
            // recall and repeat its notice every beat.
            d.recall_sent = true;
            d.log("salvage", "detail out too long - recalling");
            d.tap(KeyCode::KeyF);
        }
        return;
    }
    d.eva_hold = 0.0;
    d.recall_sent = false;

    // ---- mid-jump ---------------------------------------------------------
    // A charging drive owns the ship. Hold G and touch nothing else: steering
    // is fine, but a release and a second press would cancel the charge.
    if d.warping {
        let charge_up = !warp.local_charging.is_empty();
        if charge_up {
            d.warp_seen = true;
        }

        if !flying {
            // Docked or paused mid-charge — the jump is moot.
            d.warping = false;
            d.warp_seen = false;
        } else if d.warp_seen && !charge_up {
            d.warping = false;
            d.warp_seen = false;
            d.log("warp", "arrived");
        } else if d.warp_for > WARP_GIVE_UP && !d.warp_seen {
            // G never took. Don't keep holding a key the game is ignoring.
            d.warping = false;
            warp.local_target.0 = None;
            d.log("warp", "drive refused the jump - burning there instead");
        } else {
            d.warp_for += d.dt;
            d.hold(KeyCode::KeyG);
            return;
        }
    }

    match *state.get() {
        GameState::Paused => {
            if d.beat > 1.0 {
                d.beat = 0.0;
                d.tap(KeyCode::Escape);
            }
            return;
        }
        GameState::Docked => {
            // A settlement berth. Its menu opens on Repair Hull and Undock is
            // the last row, one Up away. Bare Enter bought hull repairs on a
            // loop ("Hull already at full integrity" every second) and never
            // left -- which wedged a whole run in flight training.
            if d.beat > 1.0 {
                d.beat = 0.0;
                if d.undock_armed {
                    d.tap(KeyCode::Enter);
                } else {
                    d.tap(KeyCode::ArrowUp);
                }
                d.undock_armed = !d.undock_armed;
            }
            return;
        }
        GameState::Loading | GameState::GameOver => return,
        _ => {}
    }

    if let Some((phase, at)) = d.forced_start {
        if d.run_elapsed < at {
            return;
        }
        d.forced_start = None;
        d.go(phase);
    }

    // ---- phase logic ------------------------------------------------------
    match d.phase {
        Phase::Boot => {
            // Enter on the main menu starts a new expedition and arms training.
            if *state.get() == GameState::MainMenu {
                if d.beat > 0.7 {
                    d.beat = 0.0;
                    d.tap(KeyCode::Enter);
                }
            } else if at_station {
                d.go(Phase::Training);
            }
        }

        Phase::Training => {
            match tutorial.pending() {
                None => {
                    d.log("tutorial", "training complete");
                    d.go(if at_station { Phase::Outfit } else { Phase::Home });
                }
                Some(step) => {
                    // Training is a nice-to-have on a run whose point is the
                    // economy loop. If a step can't be satisfied, dismiss it
                    // ([;]) and get on with the game rather than burning the
                    // whole deadline on the tutorial card.
                    if d.phase_elapsed > 240.0 {
                        let msg = format!("stuck on {:?} for 240s - dismissing training", step);
                        d.log("tutorial", &msg);
                        d.tap(KeyCode::Semicolon);
                    } else {
                        drive_tutorial(&mut d, step, &w, &stations, flying)
                    }
                }
            }
        }

        Phase::Outfit => {
            if !at_station {
                d.go(Phase::Hunt);
                return;
            }
            // The starter hull is already a working ship; the honest thing to
            // check here is that the shipyard opens and closes cleanly. Actual
            // module buying is mouse-driven placement (see module docs).
            if d.crew_window_open {
                d.crew_window_open = false;
                d.tap(KeyCode::KeyC);
                return;
            }
            if d.step == 0 && d.phase_elapsed > 1.5 {
                d.tap(KeyCode::KeyB);
                d.step = 1;
                d.log("outfit", "opened shipyard");
            } else if d.step == 1 && d.phase_elapsed > 4.0 {
                d.tap(KeyCode::Escape);
                d.step = 2;
            } else if d.step == 2 && d.phase_elapsed > 5.5 {
                d.go(Phase::Board);
            }
        }

        Phase::Trade => {
            if !at_station {
                d.go(Phase::Hunt);
                return;
            }
            // What a player actually does on docking, in the order that makes
            // sense: empty the hold FIRST so there are credits to pay for the
            // rest, then patch the ship up. Indices are rows in
            // get_docking_services: 5 Sell Cargo, 0 Repair Hull, 2 Refuel,
            // 3 Rearm Weapons, 6 Repair Modules. Entering a row that isn't
            // applicable just prints a notice, so this is safe to run blind.
            const ROUTINE: [usize; 5] = [5, 0, 2, 3, 6];

            if d.step as usize >= ROUTINE.len() {
                // Close the services first: the board opens behind them.
                if w.dock_menu.single().is_ok() {
                    if d.beat > 0.5 {
                        d.beat = 0.0;
                        d.tap(KeyCode::KeyU);
                    }
                    return;
                }
                d.go(Phase::Board);
                return;
            }
            // Don't wait on the overlay forever. Without this the phase sits
            // here until the stall watchdog fires, which sends us Home, which
            // sees us docked and routes straight back into Trade - a loop with
            // a four-minute period that looks like a hang.
            if d.phase_elapsed > 20.0 {
                d.log("trade", "services menu never came up - skipping to the board");
                d.go(Phase::Board);
                return;
            }
            let Ok(sel) = w.dock_menu.single() else {
                // At a station the services open on U (the Shop button); a
                // settlement opens them on arrival. Ask on a beat, then wait.
                if d.beat > 1.0 {
                    d.beat = 0.0;
                    d.tap(KeyCode::KeyU);
                }
                return;
            };
            if d.beat <= 0.35 {
                return;
            }
            d.beat = 0.0;
            let want = ROUTINE[d.step as usize];
            // Module repair spends whatever it can now. Run every visit, it
            // turned all the money into hull plating and left nothing for a
            // bounty deposit or fuel; keep a working fund instead.
            if want == 6 && currency.credits < 2_500 {
                d.step += 1;
                return;
            }
            if sel.0 != want {
                d.tap(KeyCode::ArrowDown);
            } else {
                d.tap(KeyCode::Enter);
                d.step += 1;
            }
        }

        Phase::Board => {
            if !at_station {
                d.go(Phase::Hunt);
                return;
            }
            let active = contracts
                .active_contracts
                .iter()
                .filter(|c| c.status == ContractStatus::Active)
                .count();

            // A bounty on offer is worth a visit even with three jobs open:
            // surveys and retrieval jobs can sit unfinished for a long time,
            // and at three active the board was never opened again - so the
            // only work that ends in a fight was never taken.
            let bounty_on_offer = contracts
                .available_by_station
                .get(board.0 .0)
                .is_some_and(|list| {
                    list.iter().any(|c| {
                        matches!(c.objective, ContractObjective::DestroyShip { .. })
                            && c.deposit <= currency.credits
                    })
                });
            let have_bounty = contracts.active_contracts.iter().any(|c| {
                c.status == ContractStatus::Active
                    && matches!(c.objective, ContractObjective::DestroyShip { .. })
            });
            let room = active < 3 || (bounty_on_offer && !have_bounty && active < 5);
            if !board_open.0 {
                if !room || d.step > 24 {
                    d.go(Phase::Launch);
                } else if d.beat > 0.6 {
                    d.beat = 0.0;
                    d.tap(KeyCode::KeyJ);
                }
                return;
            }

            // Board is open. Bounties first: they're the work that ends in a
            // fight, and taking whatever sat at the top gave survey after
            // survey and three runs with no combat in them. Walk the cursor
            // to the first bounty the deposit can be paid for and take it.
            let (viewing, cursor) = (&board.0, &board.1);
            let bounty_row = contracts
                .available_by_station
                .get(viewing.0)
                .and_then(|list| {
                    list.iter().position(|c| {
                        matches!(c.objective, ContractObjective::DestroyShip { .. })
                            && c.deposit <= currency.credits
                    })
                });
            if d.beat > 0.45 {
                d.beat = 0.0;
                d.step += 1;
                if !room || d.step > 24 {
                    d.tap(KeyCode::KeyJ); // close board
                } else if let Some(row) = bounty_row {
                    let at = cursor.index();
                    d.tap(if at < row {
                        KeyCode::ArrowDown
                    } else if at > row {
                        KeyCode::ArrowUp
                    } else {
                        KeyCode::Enter
                    });
                } else if d.step % 2 == 1 {
                    d.tap(KeyCode::Enter);
                } else {
                    d.tap(KeyCode::ArrowDown);
                }
            }
        }

        Phase::Launch => {
            if flying {
                d.log("launch", "in open space");
                // A system we already swept is not worth sweeping twice.
                if d.system_dry {
                    d.go(Phase::Jump);
                } else {
                    d.go(Phase::Hunt);
                }
            } else if at_station && d.beat > 0.8 {
                d.beat = 0.0;
                // handle_menu_input refuses Enter while build mode, the module
                // panel, customization or the board is up. An earlier run left
                // the shipyard open and then pressed Enter into the void for
                // the rest of its deadline. Back out first, always.
                if *build_state.get() != BuildState::Inactive || board_open.0 {
                    d.tap(KeyCode::Escape);
                } else {
                    d.tap(KeyCode::Enter);
                }
            }
        }

        Phase::Hunt => {
            if !flying {
                if at_station {
                    d.go(Phase::Trade);
                }
                return;
            }
            let integrity = hull.hull_integrity;
            if integrity < RETREAT_HULL || fuel.current_fuel < fuel.max_fuel * 0.15 {
                let msg = format!("breaking off: hull {:.0}%, fuel {:.0}", integrity * 100.0, fuel.current_fuel);
                d.log("retreat", &msg);
                d.go(Phase::Home);
                return;
            }

            let Ok((tf, vel_c)) = w.ships.single() else { return };
            let pos = tf.translation.truncate();
            let vel = vel_c.0;

            // Nearest live hostile first; that's what a contract wants dead.
            let mut best: Option<(f32, Vec2)> = None;
            for (_, etf) in w.enemies.iter() {
                let p = etf.translation.truncate();
                let dist = pos.distance(p);
                if best.map_or(true, |(b, _)| dist < b) {
                    best = Some((dist, p));
                }
            }

            if best.is_some() {
                d.search_for = 0.0;
            } else {
                d.search_for += d.dt;
            }
            if let Some((dist, tp)) = best {
                // Acquire a contact so `auto_engage` spreads the battery across
                // its silhouette instead of every barrel drilling one tile.
                // Acquire, and re-cycle if what we locked is no longer among
                // the living — a defeated hull stays an AiShip entity, so a
                // stale lock means the battery keeps drilling a corpse.
                let live_lock = selection
                    .target
                    .is_some_and(|t| w.enemies.iter().any(|(e, _)| e == t));
                if !live_lock && d.beat > 1.2 {
                    d.beat = 0.0;
                    d.tap(KeyCode::Backslash);
                }
                engage(&mut d, pos, vel, tp, dist);
            } else if let Some(at) = warp
                .expedition
                .next
                .filter(|(sys, _)| Some(*sys) == warp.streaming.loaded_system)
                .and_then(|(_, title)| w.records.iter().find(|(_, log)| log.title == title))
                .map(|(t, _)| t.translation.truncate())
            {
                // The next expedition record is in this system: go and read
                // it, the way the tracker tells a player to. Pickup is within
                // LOG_PICKUP_RANGE of the derelict; close right in.
                //
                // Ahead of stripping wrecks: the director plays a captain
                // following the story, which is what a pacing run measures.
                // Strip-first spent six minutes on Haven's hulks with the
                // first record 18 km away.
                d.search_for = 0.0;
                let dist = pos.distance(at);
                if !try_warp(&mut d, &mut warp.local_target, &fuel, at, dist) {
                    fly_to(&mut d, pos, at, dist, 250.0);
                }
            } else if d.strip_cooldown <= 0.0
                && (salvageable(&w, &d, pos).is_some_and(|(dist, _, _)| dist < DIVERT_RANGE)
                    || loot_nearby(&w, pos).is_some_and(|(dist, _)| dist < DIVERT_RANGE))
            {
                d.go(Phase::Strip);
            } else if let Some((sys, at)) = bounty_lead(&contracts, &warp.sim) {
                // A bounty to chase. In this system: go to where its ship is,
                // dashing if it's far, and it becomes a real contact on the
                // way in. Elsewhere: call this system done so Home -> Jump
                // takes us to it (the jump picks the bounty's system).
                if Some(sys) == warp.streaming.loaded_system {
                    d.search_for = 0.0;
                    let dist = pos.distance(at);
                    if !try_warp(&mut d, &mut warp.local_target, &fuel, at, dist) {
                        fly_to(&mut d, pos, at, dist, 1500.0);
                    }
                } else {
                    d.system_dry = true;
                    d.log("hunt", "bounty target is in another system - heading out");
                    d.go(Phase::Home);
                }
            } else {
                // Nothing in sight: ping for contacts, and drift outward so
                // the sweep covers new space rather than the same empty box.
                if d.beat > 4.0 {
                    d.beat = 0.0;
                    d.tap(KeyCode::KeyZ);
                }
                // Do NOT strike out on a bearing. Content clusters around
                // the station and the system's POIs, so burning off into open
                // space on an arbitrary heading is the one search pattern
                // guaranteed to find nothing - and it looked exactly as daft
                // as it sounds: a dead-straight line into the void the moment
                // the local board was clear.
                //
                // Not a warp either: a dash needs a destination. Gating one on
                // phase_elapsed chained 49 jumps, threw the ship 300k units out
                // and drank the whole tank without finding a thing.
                match nearest_station(&stations, pos) {
                    // Drift back toward populated space while we scan.
                    Some(sp) if pos.distance(sp) > DOCK_RANGE * 2.5 => {
                        let back = pos.distance(sp);
                        fly_to(&mut d, pos, sp, back, DOCK_RANGE * 2.0);
                    }
                    // Already in the neighbourhood: hold still and listen.
                    _ => {
                        if vel.length() > 40.0 {
                            d.hold(KeyCode::ShiftLeft);
                        }
                    }
                }
                if d.search_for > SWEEP_SECONDS {
                    d.search_for = 0.0;
                    d.system_dry = true;
                    let msg = format!("nothing in {:.0}s - this system is stripped", SWEEP_SECONDS);
                    d.log("hunt", &msg);
                    // Home first: sell the hold and turn in what we can before
                    // leaving the only station we know in this system.
                    d.go(Phase::Home);
                }
            }
        }

        Phase::Strip => {
            if !flying {
                // F docked us instead of sending a detail: inside docking
                // range the game gives the press to the station unless we're
                // alongside the wreck AND nearer it. A wreck parked off a
                // station -- the struck training raider beside Haven -- then
                // looped the director: launch, see it, press F, dock, trade,
                // launch, forever. Write it off like a refused dispatch.
                if d.detail_sent && !d.detail_seen {
                    if let Some(wreck) = d.strip_wreck.take() {
                        d.spent_wrecks.insert(wreck);
                        let spot = d.strip_spot.take();
                        d.spent_spots.extend(spot);
                        d.strip_cooldown = 8.0;
                        d.log("salvage", "F docked instead of dispatching - writing this wreck off");
                    }
                }
                d.go(Phase::Trade);
                return;
            }
            let Ok((tf, vel_c)) = w.ships.single() else { return };
            let pos = tf.translation.truncate();
            let speed = vel_c.0.length();
            let detail_out = !w.eva.is_empty();
            let dt = d.dt;

            // Freebie on the way past: E loots a derelict outright, no crew
            // errand involved. Cooldown so one POI isn't hammered every frame.
            if d.loot_cooldown <= 0.0 {
                if let Some((dist, _)) = loot_nearby(&w, pos) {
                    if dist < INTERACT_RANGE {
                        d.loot_cooldown = 3.0;
                        d.tap(KeyCode::KeyE);
                    }
                }
            }

            let Some((dist, wp, wreck_entity)) = salvageable(&w, &d, pos) else {
                // No wreck. Chase a lootable POI if one is close, else hunt.
                match loot_nearby(&w, pos) {
                    Some((d2, p2)) if d2 < DIVERT_RANGE => {
                        fly_to(&mut d, pos, p2, d2, INTERACT_RANGE * 0.7)
                    }
                    _ => d.go(Phase::Hunt),
                }
                return;
            };

            // Long haul to the wreck: jump most of it. The dash drops us
            // WARP_ARRIVAL_BUFFER short, still well outside the hold, so the
            // approach below picks up normally on arrival.
            if !d.detail_sent && try_warp(&mut d, &mut warp.local_target, &fuel, wp, dist) {
                return;
            }

            if !d.detail_sent {
                let vel = vel_c.0;
                dispatch_salvage(&mut d, &stations, pos, vel, wp, dist);
                if d.detail_sent {
                    d.strip_wreck = Some(wreck_entity);
                    d.strip_spot = Some(wp);
                    d.strip_wait = 0.0;
                    d.log("salvage", "detail dispatched");
                }
            } else {
                // Detail is out. Do NOT touch F again — that recalls them.
                // The only job now is holding station inside the leash.
                if dist > SALVAGE_LEASH {
                    fly_to(&mut d, pos, wp, dist, SALVAGE_HOLD);
                } else if speed > 40.0 {
                    d.hold(KeyCode::ShiftLeft);
                }

                d.strip_wait += dt;
                if detail_out {
                    d.detail_seen = true;
                } else if d.detail_seen {
                    d.log("salvage", "detail back aboard");
                    d.go(Phase::Hunt);
                    return;
                } else if d.strip_wait > 6.0 {
                    // F was pressed but nobody went out — no idle crew, or the
                    // wreck was already stripped. Write it off so Hunt doesn't
                    // send us straight back to it.
                    d.spent_wrecks.insert(wreck_entity);
                    d.spent_spots.push(wp);
                    d.strip_cooldown = 8.0;
                    d.log("salvage", "no detail launched - writing this wreck off");
                    d.go(Phase::Hunt);
                    return;
                }

                if d.strip_wait > SALVAGE_TIMEOUT {
                    d.spent_wrecks.insert(wreck_entity);
                    d.spent_spots.push(wp);
                    d.strip_cooldown = 8.0;
                    d.log("salvage", "timed out on this wreck");
                    d.go(Phase::Hunt);
                }
            }
        }

        Phase::Home => {
            if at_station {
                d.log("dock", "docked - working the services");
                d.go(Phase::Trade);
                return;
            }
            if !flying {
                return;
            }
            let Ok((tf, _vel)) = w.ships.single() else { return };
            let pos = tf.translation.truncate();

            // The run home is the longest trip in the loop — jump it rather
            // than burning across the system a second time.
            if let Some(sp) = nearest_station(&stations, pos) {
                if try_warp(&mut d, &mut warp.local_target, &fuel, sp, pos.distance(sp)) {
                    return;
                }
            }

            if !head_for_dock(&mut d, &stations, pos, !w.eva.is_empty(), nearest_wreck(&w, pos)) {
                // No station loaded here — this is where an interstellar hop
                // would go once warp routing is wired in.
                d.log("home", "no station in this system");
                d.go(Phase::Hunt);
            }
        }

        Phase::Jump => {
            if !flying {
                // Docked mid-sequence; get back out and try again.
                d.go(Phase::Launch);
                return;
            }

            let charge_up = !warp.galaxy_charging.is_empty();
            if charge_up {
                d.jump_seen = true;
                d.jump_for += d.dt;
                return; // G starts the charge and it runs on its own.
            }
            if d.jump_seen {
                d.log("jump", "arrived in a new system");
                d.system_dry = false;
                d.jump_seen = false;
                d.jump_for = 0.0;
                // Drop the target we just used. Left set, the next dry system
                // would re-target where we already are, the drive would answer
                // "Already there", and Jump would burn its whole timeout doing
                // nothing before falling through to Hunt.
                warp.galaxy_target.0 = None;
                d.go(Phase::Hunt);
                return;
            }

            d.jump_for += d.dt;
            if d.jump_for > 12.0 {
                // The drive never took. Don't strand the run here.
                d.log("jump", "no jump available - working this system anyway");
                d.system_dry = false;
                d.jump_for = 0.0;
                // Left set, it would turn every later G into a jump attempt
                // instead of a dash: one key serves both now.
                warp.galaxy_target.0 = None;
                d.go(Phase::Hunt);
                return;
            }

            // Pick the nearest system that isn't this one. Targeting by id
            // doesn't need it discovered first - that's the same thing the
            // galaxy map's click-anywhere does, just without the clicking.
            // The story first, then the money (see Hunt).
            if warp.galaxy_target.0.is_none() {
                if let Some((sys, _)) = warp.expedition.next {
                    if Some(sys) != warp.streaming.loaded_system {
                        let msg = format!("targeting {} to jump - next record", crate::celestial::galaxy::system_name(sys));
                        warp.galaxy_target.0 = Some(GalaxyWarpTarget::System(sys));
                        d.log("jump", &msg);
                    }
                }
            }
            if warp.galaxy_target.0.is_none() {
                if let Some((sys, _)) = bounty_lead(&contracts, &warp.sim) {
                    if Some(sys) != warp.streaming.loaded_system {
                        let msg = format!("targeting {} to jump - bounty", crate::celestial::galaxy::system_name(sys));
                        warp.galaxy_target.0 = Some(GalaxyWarpTarget::System(sys));
                        d.log("jump", &msg);
                    }
                }
            }
            if warp.galaxy_target.0.is_none() {
                let here = warp.streaming.current_galaxy_pos;
                let nearest = warp
                    .galaxy_map
                    .systems
                    .iter()
                    .filter(|s| s.galaxy_pos.distance(here) > 1.0)
                    .min_by(|a, b| {
                        a.galaxy_pos
                            .distance(here)
                            .total_cmp(&b.galaxy_pos.distance(here))
                    });
                match nearest {
                    Some(sys) => {
                        let msg = format!("targeting {} to jump", sys.name);
                        warp.galaxy_target.0 = Some(GalaxyWarpTarget::System(sys.id));
                        d.log("jump", &msg);
                    }
                    None => {
                        d.log("jump", "nowhere to jump to - staying put");
                        d.system_dry = false;
                        d.go(Phase::Hunt);
                        return;
                    }
                }
            }

            if d.beat > 1.0 {
                d.beat = 0.0;
                d.tap(KeyCode::KeyG);
            }
        }

        Phase::Done | Phase::Aborted => {}
    }

    // Pause/menu states shouldn't count against the stall watchdog, and
    // neither does training: it earns nothing by design and has its own
    // 240s way out. Real seconds, too -- this added 1/60 per frame, and an
    // offscreen run at several hundred fps "stalled" 44s in, walking out of
    // the salvage lesson so training never finished.
    if (flying || at_station) && d.phase != Phase::Training {
        d.stall_timer += d.dt;
    }
    if d.stall_timer > STALL_SECONDS {
        d.stall_timer = 0.0;
        if *build_state.get() != BuildState::Inactive {
            d.tap(KeyCode::Escape);
        }
        let msg = format!("no credit movement in {:.0}s during {:?}", STALL_SECONDS, d.phase);
        d.log("stall", &msg);
        d.go(Phase::Home);
    }

    let _ = &mut next_state;
}

/// Point at `tp` and manage throttle so we close to `hold` and sit there.
/// Cover a long haul with the warp drive instead of the main engine. Sets the
/// destination the way a map click does, then leaves `warping` set so the brain
/// holds G until the drive fires. Returns true if a jump was started, in which
/// case the caller must yield the frame — the charge owns the ship now.
fn try_warp(
    d: &mut Director,
    warp_target: &mut PendingWarpTarget,
    fuel: &FuelState,
    tp: Vec2,
    dist: f32,
) -> bool {
    if d.warping || dist < WARP_MIN_DISTANCE {
        return false;
    }
    let jump = dist - WARP_ARRIVAL_BUFFER;
    let cost = jump / 1000.0;
    // Never spend the tank down to where we can't fly home afterwards.
    if fuel.current_fuel < cost + WARP_FUEL_RESERVE {
        return false;
    }
    warp_target.0 = Some(tp);
    d.warping = true;
    d.warp_for = 0.0;
    d.warp_seen = false;
    let msg = format!("dashing {:.0} units for {:.0} fuel", jump, cost);
    d.log("warp", &msg);
    true
}

fn fly_to(d: &mut Director, pos: Vec2, tp: Vec2, dist: f32, hold: f32) {
    let delta = detour(&d.obstacles, pos, tp) - pos;
    if delta.length_squared() > 1.0 {
        d.aim = Some(delta.normalize());
    }
    if dist > hold * 1.2 {
        d.hold(KeyCode::KeyW);
    } else if dist < hold * 0.6 {
        d.hold(KeyCode::KeyS);
    }
}

/// Where to steer for `tp` from `pos` without crossing any obstacle disc:
/// `tp` itself if the way is clear, otherwise a point off the obstacle's rim
/// on our side of it.
fn detour(obstacles: &[(Vec2, f32)], pos: Vec2, tp: Vec2) -> Vec2 {
    let path = tp - pos;
    let len = path.length();
    if len < 1.0 {
        return tp;
    }
    let dir = path / len;
    for &(center, keep_out) in obstacles {
        // Already inside the keep-out (docking, salvaging beside it): no detour.
        if pos.distance(center) < keep_out || tp.distance(center) < keep_out {
            continue;
        }
        let along = (center - pos).dot(dir);
        if along <= 0.0 || along >= len {
            continue;
        }
        let closest = pos + dir * along;
        if closest.distance(center) >= keep_out {
            continue;
        }
        // Go round on whichever side the line already leans to.
        let side = (closest - center).normalize_or(Vec2::new(-dir.y, dir.x));
        return center + side * (keep_out * 1.15);
    }
    tp
}

/// Fight at weapon range: hold `STANDOFF`, retro-thrust rather than coast into
/// contact, drift sideways so we aren't a stationary target, and keep the
/// trigger down the whole time we're inside `FIRE_RANGE`.
fn engage(d: &mut Director, pos: Vec2, vel: Vec2, tp: Vec2, dist: f32) {
    let delta = tp - pos;
    if delta.length_squared() > 1.0 {
        d.aim = Some(delta.normalize());
    }

    // Closing speed along the line to the target — positive means we're
    // gaining on it.
    let closing = if delta.length_squared() > 1.0 {
        vel.dot(delta.normalize())
    } else {
        0.0
    };

    if dist > STANDOFF * 1.15 {
        // Still far: burn in, but stop accelerating once we're already fast
        // enough that we'd sail straight past.
        if closing < CLOSING_SPEED_CAP * 2.5 {
            d.hold(KeyCode::KeyW);
        }
    } else if dist < STANDOFF * 0.7 {
        d.hold(KeyCode::KeyS);
    } else if closing > CLOSING_SPEED_CAP {
        // In the pocket but still drifting in — kill the closure.
        d.hold(KeyCode::ShiftLeft);
    }

    // Orbit slowly so we present a moving target instead of a parked one.
    if dist < STANDOFF * 1.6 {
        if (d.run_elapsed as i32 / 6) % 2 == 0 {
            d.hold(KeyCode::KeyA);
        } else {
            d.hold(KeyCode::KeyD);
        }
    }

    if dist < FIRE_RANGE {
        d.hold(KeyCode::Space);
    }
}

/// Fly to the nearest station and press F only once actually in range.
///
/// This has to be one shared routine: out in open space F does NOT dock, it
/// toggles an EVA salvage detail. The first version tapped F on the spot for
/// the tutorial's dock step and spent two minutes dispatching and recalling
/// the same seven crew instead of ever flying home.
fn head_for_dock(
    d: &mut Director,
    stations: &SystemStations,
    pos: Vec2,
    crew_out: bool,
    wreck_near: Option<(f32, Vec2)>,
) -> bool {
    let Some(site) = stations.sites.iter().min_by(|a, b| {
        pos.distance(a.pos)
            .partial_cmp(&pos.distance(b.pos))
            .unwrap_or(std::cmp::Ordering::Equal)
    }) else {
        return false;
    };

    let dist = pos.distance(site.pos);
    // Aim for a point just off the hull on our side of the station, with a
    // tight arrival band. Holding a standoff from the centre instead left the
    // 20% slack of fly_to's band at kilometre scale: the ship parked 4,400-
    // 4,800 from Haven's centre, outside its 4,400 docking range, for good.
    //
    // F docks unless the ship is alongside a wreck and nearer it than the
    // station (eva_salvage::station_takes_the_press). Parked with a hulk
    // alongside, every press put a detail on the wreck and the dock lesson
    // recalled it -- 125 s of out-and-back in one run. So with a wreck
    // alongside, close right in on the hull first.
    let edge = (dist - site.radius()).max(0.0);
    // `wreck_near` is root to root and the game measures to the wreck's
    // nearest block, so allow a margin on "alongside".
    let alongside = crate::crew::eva_salvage::ALONGSIDE + 1_000.0;
    let station_wins = wreck_near.is_none_or(|(w, _)| w > alongside || edge < w);
    match wreck_near {
        // A wreck alongside takes the press: back off it until it isn't.
        // This used to close to 300 off the station's radius instead, to get
        // nearer the station than the wreck -- but along a docking arm that
        // point is inside the arm, and the director rammed Haven's arm tip at
        // full thrust until the tank was dry.
        Some((w, wreck_pos)) if !station_wins && dist < site.dock_range() * 0.8 => {
            let away = pos + (pos - wreck_pos).normalize_or(Vec2::X) * (alongside - w + 800.0);
            fly_to(d, pos, away, pos.distance(away), 150.0);
        }
        _ => {
            let approach = site.pos
                + (pos - site.pos).normalize_or(Vec2::X)
                    * (site.radius() + crate::celestial::warp::STATION_ARRIVAL_STANDOFF);
            fly_to(d, pos, approach, pos.distance(approach), 150.0);
        }
    }

    // Only inside the real docking radius, and on a slow beat so we aren't
    // mashing a key that means something else the moment we drift out.
    // F is overloaded: at a station it docks, and within eva_salvage's
    // ORDER_RANGE of a wreck it ALSO throws a detail out. One press did both
    // in an earlier run, docking the ship with nineteen crew on the hull.
    if dist < site.dock_range() * 0.95 && station_wins && d.beat > 1.0 && !crew_out {
        d.beat = 0.0;
        d.tap(KeyCode::KeyF);
    }
    true
}

/// Position of the nearest station, if any.
fn nearest_station(stations: &SystemStations, pos: Vec2) -> Option<Vec2> {
    stations
        .sites
        .iter()
        .map(|s| s.pos)
        .fold(None, |best: Option<Vec2>, p| match best {
            Some(b) if pos.distance(b) <= pos.distance(p) => best,
            _ => Some(p),
        })
}

/// Park alongside a wreck and send exactly one detail out.
///
/// Momentum is forever here, so "close enough AND slow enough" has to be a
/// window the ship can actually settle into: an earlier version demanded
/// dist < 300, overshot it every pass, and burned a third of the fuel tank
/// circling a wreck it never dispatched to. Close to within half a hold,
/// brake to a stop, THEN press.
///
/// The last guard is the F collision: inside a station's dock radius that key
/// docks the ship instead of launching anyone, so back away from the station
/// before pressing rather than pressing and hoping.
fn dispatch_salvage(
    d: &mut Director,
    stations: &SystemStations,
    pos: Vec2,
    vel: Vec2,
    wreck: Vec2,
    dist: f32,
) {
    // The usual standoff everywhere, stations included. Near a station it
    // used to be 300 off the wreck's centre, to keep out of the dock radius;
    // with docking at 7 km that caught Haven's training wreck, and 300 off a
    // hull's centre is inside the hull, so the ship circled it for four
    // minutes and never got close enough to press F.
    let hold = SALVAGE_HOLD;

    if dist > hold * 1.5 {
        fly_to(d, pos, wreck, dist, hold);
        return;
    }
    if vel.length() > 60.0 {
        d.hold(KeyCode::ShiftLeft);
        return;
    }

    // Inside a station's docking range F only salvages alongside the wreck
    // and nearer it than the station; anywhere else in range a press docks.
    if let Some(site) = stations.nearest_in_range(pos) {
        let edge = (pos.distance(site.pos) - site.radius()).max(0.0);
        if crate::crew::eva_salvage::station_takes_the_press(edge, dist) {
            fly_to(d, pos, wreck, dist, 900.0);
            return;
        }
    }

    d.tap(KeyCode::KeyF);
    d.detail_sent = true;
}

fn nearest(points: impl Iterator<Item = Vec2>, from: Vec2) -> Option<(f32, Vec2)> {
    points.fold(None, |best: Option<(f32, Vec2)>, p| {
        let dist = from.distance(p);
        match best {
            Some((b, _)) if b <= dist => best,
            _ => Some((dist, p)),
        }
    })
}

/// Nearest wreck that still has something in it and hasn't already refused us.
fn salvageable(w: &World1, d: &Director, pos: Vec2) -> Option<(f32, Vec2, Entity)> {
    w.wrecks
        .iter()
        .filter(|(e, t, wreck)| {
            let p = t.translation.truncate();
            wreck.loot_remaining > 0
                && !d.spent_wrecks.contains(e)
                && !d.spent_spots.iter().any(|s| s.distance(p) < SPENT_SPOT_RADIUS)
        })
        .map(|(e, t, _)| {
            let p = t.translation.truncate();
            (pos.distance(p), p, e)
        })
        .fold(None, |best: Option<(f32, Vec2, Entity)>, cur| match best {
            Some((b, _, _)) if b <= cur.0 => best,
            _ => Some(cur),
        })
}

/// Distance to the nearest wreck of any kind -- stripped or not, since the
/// game still sends a detail to a gutted hulk with plating left on it.
fn nearest_wreck(w: &World1, pos: Vec2) -> Option<(f32, Vec2)> {
    nearest(w.wrecks.iter().map(|(_, t, _)| t.translation.truncate()), pos)
}

fn loot_nearby(w: &World1, pos: Vec2) -> Option<(f32, Vec2)> {
    // Only what E can actually take. Asteroid nodes and waystations are
    // SpacePois too and are never "looted", so chasing every unlooted one sent
    // the ship off to stare at rocks for the rest of the run.
    nearest(
        w.pois
            .iter()
            .filter(|(_, poi)| {
                !poi.looted && matches!(poi.poi_type, SpacePoiType::DerelictShip | SpacePoiType::Anomaly)
            })
            .map(|(t, _)| t.translation.truncate()),
        pos,
    )
}

/// Flight training reads real keys, so the director answers each step with the
/// key that step is waiting for.
fn drive_tutorial(
    d: &mut Director,
    step: Advance,
    w: &World1,
    stations: &SystemStations,
    flying: bool,
) {
    // Info steps just want [Space], but on a human rhythm — one press per
    // beat, not one per frame.
    let beat_ready = d.beat > 0.9;
    match step {
        Advance::Launch => {
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::Enter);
            }
        }
        Advance::Continue => {
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::Space);
            }
        }
        Advance::Thrust => {
            d.hold(KeyCode::KeyW);
        }
        Advance::Inside => {
            // [.] in, then [.] out on the next beat.
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::Period);
            }
        }
        Advance::Scan => {
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::KeyZ);
            }
        }
        Advance::Lock => {
            // No mouse here: `\` picking the raider counts as the lock.
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::Backslash);
            }
        }
        Advance::Kill => {
            if !flying {
                return;
            }
            let Ok((tf, vel_c)) = w.ships.single() else { return };
            let pos = tf.translation.truncate();
            let vel = vel_c.0;
            if let Some((dist, tp)) = nearest(w.enemies.iter().map(|(_, t)| t.translation.truncate()), pos) {
                // The training raider is clamped to 700 range, so this is safe
                // to do from well outside its reach.
                engage(d, pos, vel, tp, dist);
            } else if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::KeyZ);
            }
        }
        Advance::Salvage => {
            if !flying {
                // Docked with a salvage lesson outstanding — almost always
                // because F docked us instead of dispatching. Get back out.
                if beat_ready {
                    d.beat = 0.0;
                    d.tap(KeyCode::Enter);
                }
                return;
            }
            let Ok((tf, vel_c)) = w.ships.single() else { return };
            let pos = tf.translation.truncate();
            // The step completes the moment a detail is actually out, so once
            // anyone is on the hull there is nothing left to do here.
            if !w.eva.is_empty() {
                return;
            }
            // One dispatch — but a press that produced no detail must not latch
            // forever. It wedged a full run on this step for four minutes:
            // detail_sent stayed true, nobody was outside, and the branch
            // returned early on every frame from then on.
            if d.detail_sent {
                d.strip_wait += d.dt;
                if d.strip_wait < 5.0 {
                    return;
                }
                d.log("salvage", "training dispatch did not take - retrying");
                d.detail_sent = false;
                d.strip_wait = 0.0;
            }
            if let Some((dist, tp, _)) = salvageable(w, d, pos) {
                dispatch_salvage(d, stations, pos, vel_c.0, tp, dist);
                if d.detail_sent {
                    d.log("salvage", "training detail dispatched");
                }
            }
        }
        Advance::Dock => {
            if !flying {
                return;
            }
            let Ok((tf, _vel)) = w.ships.single() else { return };
            let pos = tf.translation.truncate();
            head_for_dock(d, stations, pos, !w.eva.is_empty(), nearest_wreck(w, pos));
        }
        Advance::Build => {
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::KeyB);
            }
        }
        Advance::Contracts => {
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::KeyJ);
            }
        }
        Advance::Crew => {
            if beat_ready {
                d.beat = 0.0;
                d.tap(KeyCode::KeyC);
                d.crew_window_open = true;
            }
        }
    }
}

// ============================================================================
// TIME DILATION
// ============================================================================

/// Empty transits are the boring part of a two-hour run, so they run fast;
/// anything with a hostile, a wreck, or a menu on screen drops back to 1x so
/// what the run finds is what a player would have seen.
fn director_timescale(
    d: Res<Director>,
    mut virtual_time: ResMut<Time<Virtual>>,
    enemies: Query<(), With<AiShip>>,
    ships: Query<&Transform, With<Ship>>,
    board_open: Res<MissionBoardOpen>,
) {
    if d.max_speed <= 1.0 {
        return;
    }
    let contact = !enemies.is_empty();
    let busy = matches!(
        d.phase,
        Phase::Boot | Phase::Training | Phase::Outfit | Phase::Board | Phase::Strip
    ) || board_open.0;

    let speed = if contact || busy {
        1.0
    } else if ships.single().is_ok() {
        d.max_speed
    } else {
        1.0
    };

    if (virtual_time.relative_speed() - speed).abs() > 0.01 {
        virtual_time.set_relative_speed(speed);
    }
}

// ============================================================================
// OBSERVATION — screenshots + everything the game told the player
// ============================================================================

/// Captures that came back empty. macOS stops updating an occluded window's
/// surface, so any frame taken while the game is behind another window is
/// solid black — 57KB of nothing. Counting them lets the director retry
/// instead of filling the run directory with blanks it will never look at.
static BLANK_CAPTURES: AtomicU32 = AtomicU32::new(0);
static SAVED_CAPTURES: AtomicU32 = AtomicU32::new(0);

/// Save a capture only if it actually contains an image.
///
/// Replaces bevy's `save_to_disk`, which writes whatever it is handed. Samples
/// a sparse grid of pixels rather than the whole frame — a real frame lights
/// up within the first handful of samples, and a blank one costs a few hundred
/// comparisons to rule out.
fn save_if_legible(path: PathBuf) -> impl FnMut(On<ScreenshotCaptured>) {
    move |captured| {
        let Ok(dyn_img) = captured.image.clone().try_into_dynamic() else {
            return;
        };
        let rgb = dyn_img.to_rgb8();

        let mut lit = 0u32;
        for px in rgb.pixels().step_by(101) {
            if px.0[0] as u32 + px.0[1] as u32 + px.0[2] as u32 > 24 {
                lit += 1;
                if lit >= 8 {
                    break;
                }
            }
        }

        if lit < 8 {
            BLANK_CAPTURES.fetch_add(1, Ordering::Relaxed);
            return;
        }

        match rgb.save(&path) {
            Ok(()) => {
                SAVED_CAPTURES.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => error!("[AUTOPLAY] could not save {}: {e}", path.display()),
        }
    }
}

/// A heartbeat line every 30s. Staffing is on it because a gun with nobody on
/// it does not fire (combat::weapon_is_crewed) — a run that quietly stops
/// shooting looks identical in the journal to one that never found a target,
/// and this is what tells the two apart.
fn director_status(
    mut d: ResMut<Director>,
    currency: Res<Currency>,
    hull: Res<HullState>,
    fuel: Res<FuelState>,
    staffing: Res<StaffingState>,
    tutorial: Res<Tutorial>,
) {
    d.status_in -= d.dt;
    if d.status_in > 0.0 {
        return;
    }
    d.status_in = 30.0;

    let msg = format!(
        "{}c | hull {:.0}% | fuel {:.0} | crew {}/{} berths | {}/{} posts manned | {} kills{}",
        currency.credits,
        hull.hull_integrity * 100.0,
        fuel.current_fuel,
        staffing.total_crew,
        staffing.total_berths,
        staffing.staffed_stations,
        staffing.total_stations,
        d.kills,
        match tutorial.pending() {
            Some(step) => format!(" | training: {:?}", step),
            None => String::new(),
        },
    );
    d.log("status", &msg);
}

fn director_watch(
    mut commands: Commands,
    mut d: ResMut<Director>,
    mut notifications: MessageReader<ShowNotification>,
    mut deaths: MessageReader<AiShipDestroyed>,
    mut last_phase: Local<Option<Phase>>,
    offscreen: Option<Res<crate::demo::OffscreenTarget>>,
) {
    let mut want_shot = false;

    // Every hostile that stopped fighting, and how. `cause` is the interesting
    // half — Struck vs Meltdown vs Gutted says which of the combat rules
    // actually decided the fight.
    let kills: Vec<String> = deaths
        .read()
        .map(|k| format!("{:?} killed ({:?})", k.ship_type, k.cause))
        .collect();
    for kill in kills {
        d.kills += 1;
        d.log("kill", &kill);
        want_shot = true;
    }

    // A phase change is always worth a frame.
    if *last_phase != Some(d.phase) {
        *last_phase = Some(d.phase);
        want_shot = true;
    }

    // The notification stream is the game narrating itself — log all of it,
    // and photograph the ones that mean something went right or wrong.
    let notes: Vec<(String, NotificationType)> = notifications
        .read()
        .map(|n| (n.message.clone(), n.notification_type))
        .collect();
    for (message, kind) in notes {
        let tag = match kind {
            NotificationType::Danger => "danger",
            NotificationType::Warning => "warning",
            NotificationType::Success => "success",
            NotificationType::Info => "info",
        };
        d.log(tag, &message);
        if matches!(kind, NotificationType::Danger | NotificationType::Success) {
            want_shot = true;
        }
    }

    // A blank capture means the window was behind something. Keep the request
    // alive and retake on a short cycle so the shot lands as soon as the game
    // is visible again, rather than losing the moment entirely.
    let blanks = BLANK_CAPTURES.load(Ordering::Relaxed);
    if blanks != d.blanks_seen {
        d.blanks_seen = blanks;
        if d.shot_retries < 40 {
            d.shot_pending = true;
            d.shot_retries += 1;
            d.shot_cooldown = 2.0;
        } else if d.shot_pending {
            d.shot_pending = false;
            d.log("capture", "gave up on a shot - window occluded too long");
        }
    }

    if d.shots >= MAX_SHOTS {
        if d.shot_pending || want_shot {
            d.shot_pending = false;
            if d.shots == MAX_SHOTS {
                d.shots += 1;
                d.log("capture", "frame cap reached - no more screenshots this run");
            }
        }
        return;
    }

    if (want_shot || d.shot_pending) && d.shot_cooldown <= 0.0 {
        if want_shot {
            d.shot_retries = 0;
        }
        d.shot_cooldown = SHOT_COOLDOWN;
        d.shot_pending = false;
        let path = PathBuf::from(format!("{}/shot_{:04}_{:?}.png", d.dir, d.shots, d.phase));
        d.shots += 1;
        commands
            .spawn(crate::demo::screenshot_of(offscreen.as_deref()))
            .observe(save_if_legible(path));
    }
}

#[cfg(test)]
mod detour_tests {
    use super::*;

    #[test]
    fn a_clear_line_goes_straight() {
        let obstacles = [(Vec2::new(0.0, 10_000.0), 3_000.0)];
        assert_eq!(detour(&obstacles, Vec2::new(-8_000.0, 0.0), Vec2::new(8_000.0, 0.0)), Vec2::new(8_000.0, 0.0));
    }

    #[test]
    fn a_station_in_the_way_is_flown_round() {
        let obstacles = [(Vec2::ZERO, 3_000.0)];
        let from = Vec2::new(-8_000.0, 200.0);
        let wp = detour(&obstacles, from, Vec2::new(8_000.0, 0.0));
        assert!(wp.distance(Vec2::ZERO) > 3_000.0, "waypoint {wp} is inside the station");
        assert!(wp.y > 0.0, "should go round the side the line leans to");
    }
}
