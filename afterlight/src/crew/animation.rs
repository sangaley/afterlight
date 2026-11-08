//! Crew sprite animation.
//!
//! Crew are drawn from straight down like everything else, which has one
//! consequence worth stating up front: there is exactly ONE sprite set. From
//! directly above a person turning genuinely rotates, so `walk_crew` points
//! the transform along the direction of travel and these frames only ever
//! carry animation, never heading. That is why there are no 4- or 8-direction
//! sheets here.
//!
//! Frame counts follow the small-sprite convention: six for a walk cycle
//! (enough to read, cheap to make), three for the idle and work loops, one
//! static pose for the dead.

use bevy::prelude::*;

use std::collections::HashSet;

use crate::components::{CrewMember, CrewState, CrewStation};
use crate::crew::walking::CrewPath;

/// Pixel size of one frame in every crew sheet.
const FRAME: u32 = 64;

/// Seconds per frame while walking.
///
/// Crew move at CREW_WALK_SPEED (50 u/s) across a 66-unit cell, so a cell
/// takes ~1.32s. At this rate a six-frame cycle plays a little under twice per
/// cell, which is what stops the classic moonwalk-slide of feet that do not
/// keep up with the body.
const WALK_FRAME_SECS: f32 = 0.13;

/// Seconds per frame while idle, working, or dead.
const IDLE_FRAME_SECS: f32 = 0.55;
const WORK_FRAME_SECS: f32 = 0.30;

/// Which sheet a crew member is currently playing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CrewAnimState {
    #[default]
    Idle,
    Walk,
    Work,
    Dead,
}

impl CrewAnimState {
    pub fn frames(self) -> usize {
        match self {
            CrewAnimState::Walk => 6,
            CrewAnimState::Idle | CrewAnimState::Work => 3,
            CrewAnimState::Dead => 1,
        }
    }

    fn frame_secs(self) -> f32 {
        match self {
            CrewAnimState::Walk => WALK_FRAME_SECS,
            CrewAnimState::Work => WORK_FRAME_SECS,
            _ => IDLE_FRAME_SECS,
        }
    }

    pub fn sheet(self) -> &'static str {
        match self {
            CrewAnimState::Walk => "sprites/crew/crew_walk.png",
            CrewAnimState::Idle => "sprites/crew/crew_idle.png",
            CrewAnimState::Work => "sprites/crew/crew_work.png",
            CrewAnimState::Dead => "sprites/crew/crew_dead.png",
        }
    }
}

/// Per-crew animation cursor.
#[derive(Component)]
pub struct CrewAnimation {
    pub state: CrewAnimState,
    pub frame: usize,
    pub timer: Timer,
}

impl Default for CrewAnimation {
    fn default() -> Self {
        Self {
            state: CrewAnimState::Idle,
            frame: 0,
            timer: Timer::from_seconds(IDLE_FRAME_SECS, TimerMode::Repeating),
        }
    }
}

/// One shared atlas layout per sheet, built once at startup.
///
/// The creature code adds a fresh `TextureAtlasLayout` for every entity it
/// spawns (creatures/mod.rs, ecosystem.rs). With 20 crew on a starter hull and
/// 40-60 on a maxed one that is a lot of identical layouts, so crew share.
#[derive(Resource)]
pub struct CrewAtlases {
    pub walk: Handle<TextureAtlasLayout>,
    pub idle: Handle<TextureAtlasLayout>,
    pub work: Handle<TextureAtlasLayout>,
    pub dead: Handle<TextureAtlasLayout>,
}

impl CrewAtlases {
    pub fn for_state(&self, state: CrewAnimState) -> Handle<TextureAtlasLayout> {
        match state {
            CrewAnimState::Walk => self.walk.clone(),
            CrewAnimState::Idle => self.idle.clone(),
            CrewAnimState::Work => self.work.clone(),
            CrewAnimState::Dead => self.dead.clone(),
        }
    }
}

pub fn setup_crew_atlases(
    mut commands: Commands,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let mut strip = |n: u32| {
        layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(FRAME),
            n,
            1,
            None,
            None,
        ))
    };
    commands.insert_resource(CrewAtlases {
        walk: strip(6),
        idle: strip(3),
        work: strip(3),
        dead: strip(1),
    });
}

/// World size of a crew sprite.
///
/// Up from the old 16x16 rectangle. A cell is 66 units, so this is about a
/// third of a tile - big enough to read as a figure, small enough that two
/// crew pass each other in a corridor without overlapping.
pub const CREW_SIZE: f32 = 24.0;

/// The crew sprite, built in one place.
///
/// Four separate spawn sites used to hand-roll this and had already drifted -
/// two set z 0.5, two used CREW_Z (0.6). Anything that spawns a crew member
/// should call this.
pub fn crew_sprite(assets: &AssetServer, atlases: &CrewAtlases) -> (Sprite, CrewAnimation) {
    (
        Sprite {
            image: assets.load(CrewAnimState::Idle.sheet()),
            custom_size: Some(Vec2::splat(CREW_SIZE)),
            texture_atlas: Some(TextureAtlas {
                layout: atlases.idle.clone(),
                index: 0,
            }),
            ..default()
        },
        CrewAnimation::default(),
    )
}

/// A body left where a crew member died.
///
/// Carries no `CrewMember`, so nothing counts it as alive, routes it, or
/// feeds it oxygen -- it is purely a marker that someone was lost here.
#[derive(Component)]
pub struct CrewCorpse {
    /// Who this was. Carried through to `crew::burial::DriftingBody` when they
    /// are put out of the lock, so the drifting dead are somebody rather than
    /// scenery — the hook for a memorial, a recovery contract, or a scavenger
    /// who can tell you whose ship this came off.
    pub name: String,
}

/// Spawn the body a dead crew member leaves behind.
///
/// `handle_crew_death` despawns the crew entity the instant it dies, which
/// meant the dead pose could never appear on screen. Rather than keep the
/// original entity alive (every staffing and oxygen system would then have to
/// learn to skip it), death leaves this stripped-down stand-in.
pub fn spawn_corpse(
    commands: &mut Commands,
    assets: &AssetServer,
    atlases: &CrewAtlases,
    transform: Transform,
    ship: Option<Entity>,
    name: String,
) {
    let mut body = commands.spawn((
        Sprite {
            image: assets.load(CrewAnimState::Dead.sheet()),
            custom_size: Some(Vec2::splat(CREW_SIZE)),
            texture_atlas: Some(TextureAtlas {
                layout: atlases.dead.clone(),
                index: 0,
            }),
            ..default()
        },
        transform,
        CrewCorpse { name },
    ));
    if let Some(ship) = ship {
        body.insert(ChildOf(ship));
    }
}

/// What this crew member should be playing right now.
///
/// Walking is decided by the PRESENCE OF `CrewPath`, not by
/// `CrewState::Moving`. The walker deliberately never writes to
/// `CrewMember::state` (see crew::walking), so `Moving` is never assigned
/// anywhere in the codebase - keying off it would mean the walk cycle never
/// played at all.
fn desired_state(
    crew: &CrewMember,
    path: Option<&CrewPath>,
    posted: bool,
) -> CrewAnimState {
    if crew.health <= 0.0 || crew.state == CrewState::Unconscious {
        return CrewAnimState::Dead;
    }
    if path.is_some() {
        return CrewAnimState::Walk;
    }
    // "Working" means standing a post. `CrewState::Working` is never assigned
    // by anything in the game (only Idle / Repairing / Panicking / Salvaging
    // ever are), so the real signal is whether a CrewStation has claimed this
    // entity - which is also what actually decides whether their reactor or
    // gun produces anything.
    if posted {
        return CrewAnimState::Work;
    }
    match crew.state {
        CrewState::Repairing | CrewState::Salvaging => CrewAnimState::Work,
        _ => CrewAnimState::Idle,
    }
}

pub fn animate_crew_sprites(
    time: Res<Time>,
    assets: Res<AssetServer>,
    atlases: Res<CrewAtlases>,
    stations: Query<&CrewStation>,
    mut crew_query: Query<(
        Entity,
        &CrewMember,
        Option<&CrewPath>,
        &mut CrewAnimation,
        &mut Sprite,
    )>,
) {
    let posted: HashSet<Entity> = stations
        .iter()
        .filter_map(|station| station.assigned_crew)
        .collect();

    for (entity, crew, path, mut anim, mut sprite) in crew_query.iter_mut() {
        let want = desired_state(crew, path, posted.contains(&entity));

        // Switching sheets swaps both the image and the atlas layout, since
        // the sheets have different frame counts.
        if want != anim.state {
            anim.state = want;
            anim.frame = 0;
            anim.timer
                .set_duration(std::time::Duration::from_secs_f32(want.frame_secs()));
            anim.timer.reset();
            sprite.image = assets.load(want.sheet());
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                atlas.layout = atlases.for_state(want);
                atlas.index = 0;
            }
        }

        let frames = anim.state.frames();
        if frames <= 1 {
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                atlas.index = 0;
            }
            continue;
        }

        anim.timer.tick(time.delta());
        if !anim.timer.just_finished() {
            continue;
        }
        anim.frame = (anim.frame + 1) % frames;

        // Idle holds its extreme frames longer than the middle one. Even
        // timing on a three-frame breath reads mechanical; this is the same
        // trick hand-animators use to stop an idle looking like a metronome.
        if anim.state == CrewAnimState::Idle {
            let hold = if anim.frame == 1 { 0.55 } else { 1.35 };
            anim.timer
                .set_duration(std::time::Duration::from_secs_f32(IDLE_FRAME_SECS * hold));
        }

        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = anim.frame;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hand(health: f32, state: CrewState) -> CrewMember {
        CrewMember {
            name: "T".into(),
            health,
            max_health: 100.0,
            oxygen: 100.0,
            morale: 100.0,
            state,
        }
    }

    /// Walking must key off CrewPath. `CrewState::Moving` is never assigned by
    /// anything in the game, so a regression to keying off it would silently
    /// leave every crew member standing still while they slide around.
    #[test]
    fn walking_is_decided_by_path_not_state() {
        let idle = hand(100.0, CrewState::Idle);
        let path = CrewPath { cells: vec![IVec2::ZERO], index: 0, nav_version: 0 };
        assert_eq!(desired_state(&idle, Some(&path), false), CrewAnimState::Walk);
        assert_eq!(desired_state(&idle, None, false), CrewAnimState::Idle);
    }

    /// A crew member standing a post animates as working. `CrewState::Working`
    /// is never assigned by anything, so keying off it left this unreachable.
    #[test]
    fn a_posted_crew_member_works() {
        let idle = hand(100.0, CrewState::Idle);
        assert_eq!(desired_state(&idle, None, true), CrewAnimState::Work);
        assert_eq!(desired_state(&idle, None, false), CrewAnimState::Idle);
    }

    #[test]
    fn death_outranks_everything() {
        let dead = hand(0.0, CrewState::Repairing);
        let path = CrewPath { cells: vec![IVec2::ZERO], index: 0, nav_version: 0 };
        assert_eq!(desired_state(&dead, Some(&path), true), CrewAnimState::Dead);
    }

    #[test]
    fn every_sheet_exists_and_has_the_frames_claimed() {
        use std::path::Path;
        for state in [
            CrewAnimState::Walk,
            CrewAnimState::Idle,
            CrewAnimState::Work,
            CrewAnimState::Dead,
        ] {
            let rel = state.sheet();
            let path = Path::new("assets").join(rel);
            assert!(path.exists(), "missing crew sheet: {}", rel);
            // A sheet whose width disagrees with the declared frame count
            // renders as a garbled sprite rather than an error - the same
            // latent mismatch that exists between ecosystem.rs's hardcoded 6
            // frames and the 4-frame void_drifter sheet.
            let dims = image_dims(&path);
            assert_eq!(
                dims.0,
                FRAME * state.frames() as u32,
                "{} is {}px wide but claims {} frames of {}",
                rel, dims.0, state.frames(), FRAME
            );
            assert_eq!(dims.1, FRAME, "{} should be a single row", rel);
        }
    }

    /// Minimal PNG header read - avoids pulling an image crate into the build
    /// just for a dimension check.
    fn image_dims(path: &std::path::Path) -> (u32, u32) {
        let bytes = std::fs::read(path).expect("read png");
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (w, h)
    }
}
