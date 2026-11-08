//! The ship that is built the way you build.
//!
//! Every enemy in the game is already assembled from a `Blueprint` loaded off
//! disk, so making one out of the player's own design is mostly plumbing
//! rather than new machinery. What it buys is the one thing a written log
//! cannot: an opponent that manoeuvres, ranges and fires the way the player
//! does, because it is using their decisions.
//!
//! It only happens out past the midpoint, it happens once per run, and the
//! wreck it leaves is worth reading.

use bevy::prelude::*;
use rand::Rng;

use crate::ai_ship::components::AiShipType;
use crate::building::blueprint::DesignCapture;
use crate::building::registry::ModuleRegistry;
use crate::components::Ship;
use crate::events::{NotificationType, ShowNotification};
use crate::states::GameState;

use super::CascadeState;

/// The faction it wears. The Broken Choir are already described as ghost
/// ships, already damaged and erratic, and are hostile to everything — the
/// idea was sitting in the roster fully built.
const HOST: AiShipType = AiShipType::BrokenChoir;

/// Cascade level below which this never happens. The first half of a run is
/// supposed to be an ordinary salvage game.
const EARLIEST: f32 = 0.45;

/// How far out it appears, in world units. Far enough that it arrives as a
/// contact on the radar rather than on top of the player.
const SPAWN_MIN: f32 = 2_600.0;
const SPAWN_MAX: f32 = 4_200.0;

/// Seconds between checks, and the chance each check takes.
const CHECK_EVERY: f32 = 30.0;

/// Never within this of a station.
const STATION_KEEP_CLEAR: f32 = 6_000.0;
const CHANCE: f32 = 0.35;

#[derive(Resource, Default)]
pub struct Doppelganger {
    /// Once per run. Two of them would read as a spawner rather than an event.
    pub spent: bool,
    timer: f32,
}

/// Keep the spawner's copy of the player's design current.
///
/// Runs while docked, which is the only time the ship changes shape, so the
/// thing that comes looking is built the way the player last chose to build —
/// not the way they started.
fn capture_player_design(
    ship: Query<&Children, With<Ship>>,
    capture: DesignCapture,
) {
    let Ok(children) = ship.single() else { return };
    let design = capture.capture(children, "mirror");
    crate::ai_ship::spawner::set_mirror_design(design);
}

fn maybe_send_it(
    time: Res<Time>,
    cascade: Res<CascadeState>,
    mut state: ResMut<Doppelganger>,
    ship: Query<&GlobalTransform, With<Ship>>,
    mut commands: Commands,
    registry: Res<ModuleRegistry>,
    assets: Res<AssetServer>,
    mut notifications: MessageWriter<ShowNotification>,
    stations: Res<crate::world::home_base::SystemStations>,
    faction_stations: Res<crate::diplomacy::stations::FactionStations>,
    contacts: Res<crate::diplomacy::contacts::Contacts>,
) {
    // Once the Choir have played you your own voice (Summit Record step 3),
    // it comes for certain: what the piece said, the game now does.
    let certain = contacts.summit.done >= 3;
    if state.spent || (cascade.level < EARLIEST && !certain) {
        return;
    }
    if !crate::ai_ship::spawner::has_mirror_design() {
        return;
    }

    state.timer += time.delta_secs();
    if state.timer < CHECK_EVERY {
        return;
    }
    state.timer = 0.0;

    let mut rng = rand::thread_rng();
    if !certain && rng.gen::<f32>() > CHANCE {
        return;
    }

    let Ok(ship_gt) = ship.single() else { return };
    let origin = ship_gt.translation().truncate();

    // Not on the doorstep. Somebody who comes home late in a run should not
    // have this turn up in the station's own parking orbit — it reads as a
    // spawner glitch rather than as something that followed them.
    // A faction starport counts too, measured from its hull: the player
    // berths and undocks there, and a hostile copy arriving on top of that
    // read as the station itself hitting them.
    if stations
        .sites
        .iter()
        .any(|s| s.pos.distance(origin) < STATION_KEEP_CLEAR)
        || faction_stations
            .post
            .is_some_and(|(_, at, extent)| at.distance(origin) < STATION_KEEP_CLEAR + extent)
    {
        return;
    }
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist = rng.gen_range(SPAWN_MIN..SPAWN_MAX);
    let at = origin + Vec2::new(angle.cos(), angle.sin()) * dist;

    if crate::ai_ship::spawner::spawn_mirror_ship(HOST, at, &mut commands, &registry, &assets)
        .is_some()
    {
        state.spent = true;
        // Deliberately reads as an instrument report, not an announcement.
        // The player is meant to notice what it is by fighting it.
        notifications.write(ShowNotification {
            message: "Contact: hull profile matches this vessel. Transponder silent.".into(),
            notification_type: NotificationType::Warning,
            duration: 7.0,
        });
        info!("[mirror] spawned at {:?}, {:.0}u out", at, dist);
    }
}

fn clear_on_new_run(mut state: ResMut<Doppelganger>) {
    *state = Doppelganger::default();
    crate::ai_ship::spawner::set_mirror_design(None);
}

pub struct DoppelgangerPlugin;

impl Plugin for DoppelgangerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Doppelganger>()
            .add_systems(
                Update,
                capture_player_design.run_if(in_state(GameState::StationDocked)),
            )
            .add_systems(Update, maybe_send_it.run_if(in_state(GameState::Exploring)))
            .add_systems(OnEnter(GameState::MainMenu), clear_on_new_run);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// It must not be possible in the opening. The first half of a run is an
    /// ordinary salvage game and the whole effect depends on that being true.
    #[test]
    fn it_cannot_happen_early() {
        assert!(EARLIEST > 0.4, "too early to be a turn");
        assert!(EARLIEST < 1.0, "unreachable is worse than early");
    }

    /// It arrives as a contact, not an ambush. Spawning on top of the player
    /// would read as a cheap scare rather than something that came looking.
    #[test]
    fn it_arrives_at_a_distance() {
        assert!(SPAWN_MIN > 2_000.0, "too close to register as a contact first");
        assert!(SPAWN_MAX > SPAWN_MIN);
    }

    /// Once per run. A second one turns an event into a spawner.
    #[test]
    fn a_fresh_run_has_not_spent_it() {
        assert!(!Doppelganger::default().spent);
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    /// It must keep well clear of stations. Coming home late in a run and
    /// finding this in the parking orbit reads as a spawner glitch, not as
    /// something that followed you.
    #[test]
    fn it_keeps_clear_of_stations() {
        assert!(STATION_KEEP_CLEAR > SPAWN_MAX,
            "the keep-clear has to exceed the spawn radius, or it can still \
             appear beside a station: keep_clear={STATION_KEEP_CLEAR} max={SPAWN_MAX}");
    }
}
