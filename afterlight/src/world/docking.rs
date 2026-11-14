//! Docking and casting off, shown rather than cut.
//!
//! F near a station used to put the ship straight on its berth. Now (unless
//! Settings > Docking says Instant) traffic control calls a berth, the lane's
//! lights come on toward it, the ship is flown in hands-off -- round the
//! station if it was on the far side, never through it -- squares up, and a
//! walkway swings out to it. Then the old docking step runs: berth, top-ups,
//! the station's services. Launching runs it backwards: the walkway pulls in
//! and the ship is drawn out along the lane before the controls come back.
//! F skips either at any point.
//!
//! The ship is moved by setting its transform after physics, with its speed
//! held at zero, so nothing else has to know a sequence is running.

use bevy::prelude::*;

use super::home_base::{berth_position, SystemStations};
use crate::components::{Ship, ShipPhysics, Velocity};
use crate::states::GameState;

/// How docking is shown (Settings > Docking): 0 Full, 1 Short, 2 Instant.
pub fn style(settings: &crate::ui::menu_buttons::GameSettings) -> u8 {
    settings.docking % 3
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leg {
    /// Traffic control calls the berth; the lane lights up.
    Call,
    /// Flown in along the lane.
    Approach,
    /// The walkway swings out.
    Berth,
    /// Casting off: the walkway pulls in.
    CastOff,
    /// Drawn out along the lane.
    Leave,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub station: usize,
    pub leg: Leg,
    /// Seconds into this leg.
    pub t: f32,
    /// The path's four points (a cubic): start, two guides, the end.
    pub path: [Vec2; 4],
    pub from_angle: f32,
    /// Where the walkway meets the station, and the ship's near side.
    pub tip: Vec2,
    pub ship_edge: f32,
    pub short: bool,
}

impl Run {
    fn length(&self, leg: Leg) -> f32 {
        let k = if self.short { 0.4 } else { 1.0 };
        match leg {
            Leg::Call => 1.0 * k,
            Leg::Approach => {
                let d = self.path[0].distance(self.path[1]) + self.path[1].distance(self.path[2]) + self.path[2].distance(self.path[3]);
                (d / 7_000.0).clamp(3.0, 6.0) * k.max(0.35)
            }
            Leg::Berth => 1.2 * k,
            Leg::CastOff => 0.8 * k,
            Leg::Leave => 2.6 * k.max(0.5),
        }
    }
}

/// The sequence under way, if any; and the station the ship is moored at.
#[derive(Resource, Default)]
pub struct Docking {
    pub run: Option<Run>,
    /// Arrived: home_base::station_docking finishes docking at this station.
    pub arrived: Option<usize>,
    /// Moored here (set on docking, taken on launch).
    pub moored_at: Option<usize>,
}

impl Docking {
    pub fn busy(&self) -> bool {
        self.run.is_some()
    }
}

/// A point on a cubic.
pub fn bezier(p: &[Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    p[0] * u * u * u + p[1] * 3.0 * u * u * t + p[2] * 3.0 * u * t * t + p[3] * t * t * t
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The way in to a berth at `berth` for a station at `station` reaching
/// `extent` from its centre: down the lane from the east, and round the
/// north or south side first if the ship starts west of the berth.
pub fn approach_path(from: Vec2, berth: Vec2, station: Vec2, extent: f32) -> [Vec2; 4] {
    let entry = berth + Vec2::X * 9_000.0;
    if from.x > berth.x - 1_000.0 {
        // Already on the open side: straight in, finishing along the lane.
        [from, from.lerp(entry, 0.5), entry, berth]
    } else {
        let side = if from.y >= station.y { 1.0 } else { -1.0 };
        let round = Vec2::new(berth.x + 4_000.0, station.y + side * (extent + 6_000.0));
        [from, round, entry, berth]
    }
}

/// Start docking at `station`'s berth. Returns false if it's Instant.
#[allow(clippy::too_many_arguments)]
pub fn begin_docking(
    docking: &mut Docking,
    settings: &crate::ui::menu_buttons::GameSettings,
    station: usize,
    station_pos: Vec2,
    station_radius: f32,
    extent: f32,
    from: Vec2,
    from_angle: f32,
    ship_radius: f32,
) -> bool {
    if style(settings) == 2 || docking.busy() {
        return false;
    }
    let berth = berth_position(station_pos, station_radius, ship_radius);
    docking.run = Some(Run {
        station,
        leg: Leg::Call,
        t: 0.0,
        path: approach_path(from, berth, station_pos, extent),
        from_angle,
        tip: station_pos + Vec2::X * station_radius,
        ship_edge: ship_radius * 0.8,
        short: style(settings) == 1,
    });
    true
}

/// The lane's lights and the player's walkway.
#[derive(Component)]
pub struct LaneLight {
    pub order: usize,
}

#[derive(Component)]
pub struct PlayerWalkway;

/// The traffic note on screen.
#[derive(Component)]
pub struct TrafficNote;

/// Move the ship along the sequence, after physics so nothing undoes it.
#[allow(clippy::too_many_arguments)]
pub fn run_docking(
    mut commands: Commands,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut docking: ResMut<Docking>,
    stations: Res<SystemStations>,
    art: Option<Res<super::raft_art::RaftArt>>,
    mut ship: Query<(&mut Transform, &mut Velocity, &mut ShipPhysics), With<Ship>>,
    mut lane: Query<(Entity, &LaneLight, &mut Sprite), Without<PlayerWalkway>>,
    mut walkway: Query<(Entity, &mut Sprite, &mut Transform), (With<PlayerWalkway>, Without<Ship>, Without<LaneLight>)>,
    notes: Query<Entity, With<TrafficNote>>,
) {
    let Some(run) = docking.run.as_mut() else { return };
    let Ok((mut tf, mut vel, mut physics)) = ship.single_mut() else { return };
    let Some(site) = stations.sites.iter().find(|s| s.index == run.station).cloned() else {
        // The station's gone (a jump mid-sequence): drop the sequence and
        // everything it put up, so nothing is left hanging on screen.
        docking.run = None;
        for (e, ..) in &lane {
            commands.entity(e).despawn();
        }
        for (e, ..) in &walkway {
            commands.entity(e).despawn();
        }
        for e in &notes {
            commands.entity(e).despawn();
        }
        return;
    };
    // F skips the rest.
    let skip = keyboard.just_pressed(KeyCode::KeyF) && run.t > 0.1;
    run.t += time.delta_secs();

    // Lights and walkway exist for the whole run.
    if lane.is_empty() && matches!(run.leg, Leg::Call | Leg::Approach) {
        if let Some(art) = &art {
            for i in 0..8 {
                let x = run.path[3].x + 9_000.0 * (1.0 - i as f32 / 8.0);
                for side in [-1.0f32, 1.0] {
                    commands.spawn((
                        Sprite { image: art.glow.clone(), color: Color::srgba(1.0, 0.85, 0.45, 0.0), custom_size: Some(Vec2::splat(700.0)), ..default() },
                        Transform::from_xyz(x, run.path[3].y + side * 2_200.0, 0.2),
                        LaneLight { order: i },
                    ));
                }
            }
        }
        spawn_note(&mut commands, &site.name, "berth - east street", "hands off  -  F skips");
    }
    if walkway.is_empty() {
        if let Some(art) = &art {
            commands.spawn((
                Sprite { image: art.tube.clone(), custom_size: Some(Vec2::new(1.0, 260.0)), color: Color::NONE, ..default() },
                Transform::from_xyz(run.tip.x, run.tip.y, 0.25),
                PlayerWalkway,
            ));
        }
    }

    let len = run.length(run.leg);
    let f = (run.t / len).clamp(0.0, 1.0);
    let berth = run.path[3];
    let mut pos = tf.translation.truncate();
    let mut angle = physics.rotation;
    let mut walk = 0.0;
    match run.leg {
        Leg::Call => {
            walk = 0.0;
        }
        Leg::Approach => {
            let e = ease(f);
            pos = bezier(&run.path, e);
            // Square up over the first two thirds.
            let a = ease(f / 0.66);
            let d = (0.0 - run.from_angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            angle = run.from_angle + d * a;
        }
        Leg::Berth => {
            pos = berth;
            angle = 0.0;
            walk = ease(f);
        }
        Leg::CastOff => {
            pos = berth;
            angle = 0.0;
            walk = 1.0 - ease(f);
        }
        Leg::Leave => {
            pos = berth.lerp(berth + Vec2::X * 8_000.0, ease(f));
            angle = 0.0;
        }
    }
    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
    tf.rotation = Quat::from_rotation_z(angle);
    physics.rotation = angle;
    physics.angular_velocity = 0.0;
    physics.throttle = 0.0;
    vel.0 = Vec2::ZERO;

    // The lane lights chase in toward the berth.
    let lit_t = if run.leg == Leg::Call { run.t } else if run.leg == Leg::Approach { run.t + run.length(Leg::Call) } else { -1.0 };
    for (_, light, mut sprite) in &mut lane {
        let on = if lit_t < 0.0 { 0.0 } else { (((lit_t * 6.0 - light.order as f32) * 0.7).sin() * 0.5 + 0.5).powi(2) };
        sprite.color = Color::srgba(1.0, 0.85, 0.45, 0.25 + on * 0.75);
    }
    // The walkway reaches from the berth tip to the ship's near side.
    for (_, mut sprite, mut wtf) in &mut walkway {
        let reach = (berth.x - run.ship_edge - run.tip.x).max(1.0);
        let l = (reach * walk).max(1.0);
        sprite.custom_size = Some(Vec2::new(l, 260.0));
        sprite.color = if walk > 0.0 { Color::WHITE } else { Color::NONE };
        wtf.translation.x = run.tip.x + l * 0.5;
        wtf.translation.y = run.tip.y;
    }

    if f >= 1.0 || skip {
        let next = match run.leg {
            Leg::Call => Some(Leg::Approach),
            Leg::Approach => Some(Leg::Berth),
            Leg::Berth => None,
            Leg::CastOff => Some(Leg::Leave),
            Leg::Leave => None,
        };
        let next = if skip { None } else { next };
        match next {
            Some(leg) => {
                run.leg = leg;
                run.t = 0.0;
                if leg == Leg::Berth {
                    for e in &notes {
                        commands.entity(e).despawn();
                    }
                    spawn_note(&mut commands, &site.name, "clamps on  -  walkway out", "");
                }
            }
            None => {
                let docking_in = matches!(run.leg, Leg::Call | Leg::Approach | Leg::Berth);
                let station = run.station;
                if docking_in {
                    // Put it on the berth exactly, walkway out.
                    tf.translation.x = berth.x;
                    tf.translation.y = berth.y;
                    tf.rotation = Quat::IDENTITY;
                    physics.rotation = 0.0;
                    for (_, mut sprite, mut wtf) in &mut walkway {
                        let reach = (berth.x - run.ship_edge - run.tip.x).max(1.0);
                        sprite.custom_size = Some(Vec2::new(reach, 260.0));
                        sprite.color = Color::WHITE;
                        wtf.translation.x = run.tip.x + reach * 0.5;
                    }
                    docking.arrived = Some(station);
                } else {
                    // Cast off and clear: the walkway's gone.
                    for (e, ..) in &walkway {
                        commands.entity(e).despawn();
                    }
                }
                for (e, ..) in &lane {
                    commands.entity(e).despawn();
                }
                for e in &notes {
                    commands.entity(e).despawn();
                }
                docking.run = None;
            }
        }
    }
}

fn spawn_note(commands: &mut Commands, station: &str, line: &str, keys: &str) {
    use crate::ui::theme::{Sheet, ThemeFonts};
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(110.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-150.0)),
                width: Val::Px(300.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Sheet::BG),
            BorderColor::all(Sheet::EDGE),
            GlobalZIndex(60),
            TrafficNote,
        ))
        .with_children(|n| {
            n.spawn((Text::new(format!("{} TRAFFIC", station.to_uppercase())), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(Sheet::TITLE)));
            n.spawn((Text::new(line), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.86, 0.88, 0.92))));
            if !keys.is_empty() {
                n.spawn((Text::new(keys), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(Sheet::LABEL)));
            }
        });
}

/// Moored: remember where, so launching knows to cast off. Every frame
/// docked until it knows: a new game opens docked a frame or two before the
/// station list is filled.
pub fn note_moored(
    mut docking: ResMut<Docking>,
    stations: Res<SystemStations>,
    ship: Query<&Transform, With<Ship>>,
) {
    if docking.moored_at.is_some() {
        return;
    }
    let Ok(tf) = ship.single() else { return };
    docking.moored_at = stations.nearest_in_range(tf.translation.truncate()).map(|s| s.index);
}

/// Launching: cast off and be drawn out, unless docking is Instant.
pub fn begin_cast_off(
    mut commands: Commands,
    mut docking: ResMut<Docking>,
    settings: Res<crate::ui::menu_buttons::GameSettings>,
    stations: Res<SystemStations>,
    ship: Query<(&Transform, Option<&crate::ship::collision::Collider>), With<Ship>>,
    walkways: Query<Entity, With<PlayerWalkway>>,
) {
    let Some(station) = docking.moored_at.take() else { return };
    if style(&settings) == 2 || docking.busy() {
        for e in &walkways {
            commands.entity(e).despawn();
        }
        return;
    }
    let Ok((tf, collider)) = ship.single() else { return };
    let Some(site) = stations.sites.iter().find(|s| s.index == station) else { return };
    let ship_radius = collider.map(|c| c.bound_center.length() + c.bound_radius).unwrap_or(1_200.0);
    let at = tf.translation.truncate();
    docking.run = Some(Run {
        station,
        leg: Leg::CastOff,
        t: 0.0,
        path: [at, at, at, at],
        from_angle: 0.0,
        tip: site.pos + Vec2::X * site.radius(),
        ship_edge: ship_radius * 0.8,
        short: style(&settings) == 1,
    });
    spawn_note(&mut commands, &site.name, "casting off", "F skips");
}

pub struct DockingPlugin;

impl Plugin for DockingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Docking>()
            .add_systems(Update, note_moored.run_if(in_state(GameState::StationDocked)))
            .add_systems(OnEnter(GameState::Exploring), begin_cast_off)
            .add_systems(
                PostUpdate,
                run_docking
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(GameState::Exploring)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_way_in_ends_on_the_berth_along_the_lane() {
        let berth = Vec2::new(20_000.0, 0.0);
        let p = approach_path(Vec2::new(60_000.0, 9_000.0), berth, Vec2::ZERO, 30_000.0);
        assert_eq!(bezier(&p, 1.0), berth);
        // The last stretch comes in from the east, along the lane.
        let near = bezier(&p, 0.95);
        assert!(near.x > berth.x && near.y.abs() < 1_000.0, "{near}");
    }

    #[test]
    fn from_the_far_side_it_goes_round_not_through() {
        let station = Vec2::ZERO;
        let extent = 30_000.0;
        let berth = Vec2::new(20_000.0, 0.0);
        let p = approach_path(Vec2::new(-45_000.0, 8_000.0), berth, station, extent);
        for i in 0..=100 {
            let at = bezier(&p, i as f32 / 100.0);
            // Never across the middle of the station.
            assert!(at.distance(station) > extent * 0.5, "passes over the station at {at}");
        }
    }
}
