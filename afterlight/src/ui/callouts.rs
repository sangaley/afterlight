//! Callout lines: notes pinned to the ship (the "B" look of the UI rework).
//!
//! Nothing is pinned unless it matters right now, most urgent first:
//!
//! - a breach in the hull, from the hit until it stops venting (or 12 s);
//! - point defence tracking missiles, while the warners see any;
//! - the locked target, while you're locked on (with corner brackets);
//! - a big gun reloading, from the shot until it's loaded again;
//! - the heading arrow, only while you thrust.
//!
//! At most `callout_max` at once. Settings (ui::menu_buttons::GameSettings):
//! Full draws a leader and the words; Small draws a short stub and a number,
//! and the words go into the title block (`CalloutLegend`); Off draws none.
//! Lines are gizmos in the world; the words are UI text placed on screen.

use std::collections::HashMap;

use bevy::prelude::*;

use super::menu_buttons::GameSettings;
use super::theme::{Sheet, ThemeColors, ThemeFonts};
use crate::components::{Ship, Velocity, WeaponCooldown};
use crate::states::GameState;

/// Labels kept ready for the notes (more than `callout_max` can ever ask).
const POOL: usize = 7;
/// Leader: the diagonal leg, then the flat one, in screen pixels.
const LEG_DIAGONAL: f32 = 34.0;
const LEG_FLAT: f32 = 64.0;
/// Small mode's stub.
const LEG_SMALL: f32 = 13.0;
/// A gun only gets a note while its reload has this long left: the 20 mm
/// cycling every half second would flicker otherwise.
const RELOAD_WORTH_A_NOTE: f32 = 1.5;
/// How long a breach note stays.
const BREACH_NOTE_SECS: f32 = 12.0;
/// Fade in and out over a third of a second.
const FADE_PER_SEC: f32 = 3.0;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct CalloutGizmos;

/// The words of the Small-mode notes, inside the title block.
#[derive(Component)]
pub struct CalloutLegend;

#[derive(Component)]
struct CalloutLabel(usize);

#[derive(Component)]
struct CalloutLayer;

/// What a note is about. Its order is its urgency.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Kind {
    Breach,
    Missiles,
    Lock,
    Reload,
    Heading,
}

struct Note {
    kind: Kind,
    at: Vec2,
    words: String,
    color: Color,
}

/// Breaches heard, by segment, with how long their note has left.
#[derive(Resource, Default)]
struct Breaches(HashMap<Entity, f32>);

pub struct CalloutsPlugin;

impl Plugin for CalloutsPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<CalloutGizmos>()
            .init_resource::<Breaches>()
            .add_systems(Startup, (setup_gizmos, spawn_layer))
            .add_systems(Update, (hear_breaches, draw_callouts).chain());
    }
}

fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<CalloutGizmos>();
    config.line.width = 1.0;
}

fn spawn_layer(mut commands: Commands) {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            Pickable::IGNORE,
            GlobalZIndex(-1),
            CalloutLayer,
        ))
        .with_children(|layer| {
            for i in 0..POOL {
                layer.spawn((
                    Node { position_type: PositionType::Absolute, padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), ..default() },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                    TextColor(Color::NONE),
                    TextLayout::no_wrap(),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                    CalloutLabel(i),
                ));
            }
        });
}

fn hear_breaches(
    time: Res<Time>,
    mut heard: MessageReader<crate::events::HullBreached>,
    parents: Query<&ChildOf>,
    ships: Query<(), With<Ship>>,
    mut breaches: ResMut<Breaches>,
) {
    for b in heard.read() {
        if parents.get(b.segment).is_ok_and(|p| ships.contains(p.parent())) {
            breaches.0.insert(b.segment, BREACH_NOTE_SECS);
        }
    }
    let dt = time.delta_secs();
    breaches.0.retain(|_, left| {
        *left -= dt;
        *left > 0.0
    });
}

/// Everything worth a note this frame, most urgent first.
#[allow(clippy::too_many_arguments)]
fn gather(
    ship: (Entity, Vec2, Vec2),
    settings: &GameSettings,
    thrusting: bool,
    breaches: &Breaches,
    places: &Query<&GlobalTransform>,
    lock: &crate::combat::targeting::AimLock,
    warning: Option<&crate::combat::missile_defence::MissileWarning>,
    guns: &Query<(Entity, &crate::combat::gun_assembly::GunAssembly, &GlobalTransform, Option<&WeaponCooldown>, Has<crate::combat::targeting::fire_groups::InterceptMode>)>,
) -> Vec<Note> {
    let (me, pos, vel) = ship;
    let mut notes = Vec::new();
    if let Some(at) = breaches.0.keys().find_map(|e| places.get(*e).ok()) {
        notes.push(Note { kind: Kind::Breach, at: at.translation().truncate(), words: "hull breach".into(), color: ThemeColors::STATUS_DANGER });
    }
    let missiles = warning.map_or(0, |w| w.spotted.len());
    if missiles > 0 {
        let pd = guns.iter().find(|(_, a, _, _, pd)| *pd && a.ship == Some(me)).map(|(_, _, gt, _, _)| gt.translation().truncate());
        notes.push(Note {
            kind: Kind::Missiles,
            at: pd.unwrap_or(pos),
            words: format!("point defence  ·  {missiles} missile{}", if missiles == 1 { "" } else { "s" }),
            color: ThemeColors::ACCENT_CYAN,
        });
    }
    if let Some(target) = lock.ship.and_then(|e| places.get(e).ok()) {
        let at = target.translation().truncate();
        notes.push(Note {
            kind: Kind::Lock,
            at,
            words: format!("locked  ·  {}", super::format_range_km(at.distance(pos))),
            color: Sheet::PICKED,
        });
    }
    // The biggest gun still reloading.
    if let Some((a, gt, cd)) = guns
        .iter()
        .filter(|(_, a, _, _, pd)| !*pd && a.ship == Some(me))
        .filter_map(|(_, a, gt, cd, _)| cd.filter(|c| c.timer.remaining_secs() > RELOAD_WORTH_A_NOTE).map(|c| (a, gt, c)))
        .max_by(|x, y| x.0.stats.gauge_mm.total_cmp(&y.0.stats.gauge_mm))
    {
        notes.push(Note {
            kind: Kind::Reload,
            at: gt.translation().truncate(),
            words: format!("{:.0} mm  ·  {:.0} s", a.stats.gauge_mm, cd.timer.remaining_secs().ceil()),
            color: ThemeColors::ACCENT_YELLOW,
        });
    }
    if thrusting && settings.callout_heading && vel.length() > 5.0 {
        notes.push(Note { kind: Kind::Heading, at: pos, words: format!("{:.0} m/s", vel.length()), color: ThemeColors::ACCENT_CYAN });
    }
    notes.sort_by_key(|n| n.kind);
    notes
}

#[allow(clippy::too_many_arguments)]
fn draw_callouts(
    time: Res<Time>,
    state: Res<State<GameState>>,
    settings: Res<GameSettings>,
    input: Res<crate::resources::InputState>,
    lock: Res<crate::combat::targeting::AimLock>,
    breaches: Res<Breaches>,
    ui_scale: Res<UiScale>,
    ship: Query<(Entity, &Transform, Option<&Velocity>, Option<&crate::combat::missile_defence::MissileWarning>), With<Ship>>,
    places: Query<&GlobalTransform>,
    guns: Query<(Entity, &crate::combat::gun_assembly::GunAssembly, &GlobalTransform, Option<&WeaponCooldown>, Has<crate::combat::targeting::fire_groups::InterceptMode>)>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    windows: Query<&Window>,
    mut gizmos: Gizmos<CalloutGizmos>,
    mut labels: Query<(&CalloutLabel, &mut Node, &mut Text, &mut TextColor, &mut BackgroundColor, &mut BorderColor, &mut Visibility), Without<CalloutLegend>>,
    mut legend: Query<(&mut Text, &mut Node), (With<CalloutLegend>, Without<CalloutLabel>)>,
    mut alpha: Local<HashMap<Kind, f32>>,
) {
    let hide_all = |labels: &mut Query<(&CalloutLabel, &mut Node, &mut Text, &mut TextColor, &mut BackgroundColor, &mut BorderColor, &mut Visibility), Without<CalloutLegend>>| {
        for (_, _, _, _, _, _, mut vis) in labels.iter_mut() {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    };
    let style = settings.callout_style % 3;
    let flying = *state.get() == GameState::Exploring;
    let (Ok((me, tf, vel, warning)), Ok((cam, cam_gt)), Ok(window)) = (ship.single(), camera.single(), windows.single()) else {
        hide_all(&mut labels);
        return;
    };
    let notes = if flying && style != 2 {
        let thrusting = input.movement.length() > 0.1 || input.autopilot.length() > 0.1;
        let mut n = gather(
            (me, tf.translation.truncate(), vel.map_or(Vec2::ZERO, |v| v.0)),
            &settings,
            thrusting,
            &breaches,
            &places,
            &lock,
            warning,
            &guns,
        );
        n.truncate(settings.callout_max.clamp(1, 6) as usize);
        n
    } else {
        Vec::new()
    };

    // Fade each kind toward shown or gone.
    let dt = time.delta_secs();
    for kind in [Kind::Breach, Kind::Missiles, Kind::Lock, Kind::Reload, Kind::Heading] {
        let want = if notes.iter().any(|n| n.kind == kind) { 1.0 } else { 0.0 };
        let a = alpha.entry(kind).or_insert(0.0);
        *a = if want > *a { (*a + dt * FADE_PER_SEC).min(1.0) } else { (*a - dt * FADE_PER_SEC).max(0.0) };
    }

    let scale = ui_scale.0.max(0.01);
    let size = window.size();
    let centre = size / 2.0;
    let to_world = |p: Vec2| cam.viewport_to_world_2d(cam_gt, p).ok();
    let mut taken: Vec<Rect> = Vec::new();
    let mut legend_lines: Vec<String> = Vec::new();
    let mut used = 0;
    for note in &notes {
        let a = alpha.get(&note.kind).copied().unwrap_or(0.0);
        let Ok(anchor) = cam.world_to_viewport(cam_gt, note.at.extend(0.0)) else { continue };
        let color = note.color.with_alpha(note.color.alpha() * a);
        let line = Sheet::LABEL.with_alpha(a);
        let side = if anchor.x >= centre.x { 1.0 } else { -1.0 };
        let up = if anchor.y <= centre.y { -1.0 } else { 1.0 };

        // The heading arrow points along the ship's travel, not out to a label.
        if note.kind == Kind::Heading {
            let dir = vel.map_or(Vec2::X, |v| v.0.normalize_or(Vec2::X));
            let screen_dir = Vec2::new(dir.x, -dir.y);
            let tip = anchor + screen_dir * 56.0;
            if let (Some(p0), Some(p1)) = (to_world(anchor), to_world(tip)) {
                gizmos.arrow_2d(p0, p1, color);
            }
            if style == 0 && settings.callout_words {
                place_label(&mut labels, used, tip + Vec2::new(6.0, -8.0), false, &note.words, color, a, scale, size);
                used += 1;
            }
            continue;
        }

        // Corner brackets round the locked target.
        if note.kind == Kind::Lock {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let c = anchor + Vec2::new(sx * 26.0, sy * 20.0);
                let pts = [c - Vec2::new(sx * 9.0, 0.0), c, c - Vec2::new(0.0, sy * 9.0)];
                let world: Vec<Vec2> = pts.iter().filter_map(|p| to_world(*p)).collect();
                if world.len() == 3 {
                    gizmos.linestrip_2d(world, color);
                }
            }
        }

        if let Some(w) = to_world(anchor) {
            let r = to_world(anchor + Vec2::X * 3.0).map_or(4.0, |e| e.distance(w));
            gizmos.circle_2d(w, r, color);
        }
        if style == 1 {
            // Small: a stub and a number; the words go to the title block.
            let end = anchor + Vec2::new(side, up) * LEG_SMALL;
            if let (Some(p0), Some(p1)) = (to_world(anchor), to_world(end)) {
                gizmos.line_2d(p0, p1, line);
            }
            let n = legend_lines.len() + 1;
            place_label(&mut labels, used, end + Vec2::new(if side > 0.0 { 2.0 } else { -2.0 }, -8.0), side < 0.0, &n.to_string(), color, a, scale, size);
            used += 1;
            legend_lines.push(format!("{n}  {}", note.words));
            continue;
        }

        // Full: a diagonal leg out from the ship, then a flat one, kept clear
        // of the notes already placed.
        let mut knee = anchor + Vec2::new(side, up) * LEG_DIAGONAL;
        let mut end = knee + Vec2::new(side * LEG_FLAT, 0.0);
        let rect = |e: Vec2| Rect::from_center_size(e + Vec2::new(side * 60.0, 0.0), Vec2::new(130.0, 16.0));
        let mut tries = 0;
        while tries < 6 && taken.iter().any(|t| !t.intersect(rect(end)).is_empty()) {
            knee.y += up * 18.0;
            end.y += up * 18.0;
            tries += 1;
        }
        taken.push(rect(end));
        let pts: Vec<Vec2> = [anchor, knee, end].iter().filter_map(|p| to_world(*p)).collect();
        if pts.len() == 3 {
            gizmos.linestrip_2d(pts, line);
        }
        if settings.callout_words {
            place_label(&mut labels, used, end + Vec2::new(side * 4.0, -8.0), side < 0.0, &note.words, color, a, scale, size);
            used += 1;
        }
    }

    // Spare labels hide.
    for (label, _, _, _, _, _, mut vis) in labels.iter_mut() {
        if label.0 >= used && *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    }
    if let Ok((mut text, mut node)) = legend.single_mut() {
        let words = legend_lines.join("\n");
        if text.0 != words {
            text.0 = words;
        }
        let display = if legend_lines.is_empty() { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Put pooled label `i` on screen at `at` (screen px): its left edge there,
/// or its right edge when `leftward`.
#[allow(clippy::too_many_arguments)]
fn place_label(
    labels: &mut Query<(&CalloutLabel, &mut Node, &mut Text, &mut TextColor, &mut BackgroundColor, &mut BorderColor, &mut Visibility), Without<CalloutLegend>>,
    i: usize,
    at: Vec2,
    leftward: bool,
    words: &str,
    color: Color,
    alpha: f32,
    scale: f32,
    screen: Vec2,
) {
    for (label, mut node, mut text, mut tc, mut bg, mut edge, mut vis) in labels.iter_mut() {
        if label.0 != i {
            continue;
        }
        if text.0 != words {
            text.0 = words.to_string();
        }
        tc.0 = color;
        bg.0 = Sheet::BG.with_alpha(0.8 * alpha);
        *edge = BorderColor::all(Color::NONE);
        node.top = Val::Px(at.y / scale);
        if leftward {
            node.left = Val::Auto;
            node.right = Val::Px((screen.x - at.x) / scale);
        } else {
            node.right = Val::Auto;
            node.left = Val::Px(at.x / scale);
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}
