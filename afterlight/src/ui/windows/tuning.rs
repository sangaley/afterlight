use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::building::customization::tuning::*;
use crate::building::GridOccupancy;
use crate::combat::ammo_types::KineticAmmoType;
use crate::components::{Module, ModuleType, Weapon};
use crate::resources::PowerState;
use crate::ui::theme::ThemeColors;
use super::framework::*;
use super::tooltip::Tooltip;

// ============================================================================
// WEAPON TUNING WINDOW
// Right-click a weapon while docked → sliders for velocity / fire rate /
// damage, ammo type selector for kinetics, live power + DPS readout.
// Editing is docked-only; windows are despawned on undock.
// ============================================================================

// Derived from theme.rs rather than hand-picked, so this window shares a
// palette with the rest of the game instead of drifting on its own.
const ACCENT: Color = ThemeColors::ACCENT_ORANGE;
const SLIDER_BG: Color = ThemeColors::BG_INPUT;
const SLIDER_FILL: Color = ThemeColors::ACCENT_BLUE;
const BASELINE_TICK: Color = Color::srgba(0.9, 0.9, 0.9, 0.35);
const AMMO_SELECTED_BG: Color = ThemeColors::BG_PRESSED;
const AMMO_BG: Color = ThemeColors::BG_INPUT;

/// The roster lives on the type itself, so a new round shows up in this
/// picker the moment it's added rather than being silently unpickable.
const ALL_AMMO: [KineticAmmoType; 15] = KineticAmmoType::ALL;

#[derive(Component)]
pub struct TuningWindow {
    pub module_entity: Entity,
    pub module_type: ModuleType,
}

/// The clickable/draggable track of one slider
#[derive(Component)]
pub struct TuningSliderTrack {
    pub field: TuningField,
}

#[derive(Component)]
pub struct TuningSliderFill {
    pub field: TuningField,
}

#[derive(Component)]
pub struct TuningValueText {
    pub field: TuningField,
}

#[derive(Component)]
pub struct AmmoTypeButton {
    pub ammo: KineticAmmoType,
}

#[derive(Component)]
pub struct AmmoDescText;

#[derive(Component)]
pub struct PowerReadoutText;

#[derive(Component)]
pub struct ShipPowerBarFill;

#[derive(Component)]
pub struct ShipPowerText;

#[derive(Component)]
pub struct StatsReadoutText;

#[derive(Component)]
pub struct ResetTuningButton;

/// Which slider is currently being dragged (survives the cursor leaving the
/// track mid-drag, unlike hover-based Interaction alone).
#[derive(Resource, Default)]
pub struct ActiveSliderDrag(pub Option<TuningField>);

fn mult_to_fraction(mult: f32) -> f32 {
    (mult - TUNING_MIN) / (TUNING_MAX - TUNING_MIN)
}

// ============================================================================
// OPEN — right-click a weapon module while docked
// ============================================================================

pub fn right_click_open_tuning(
    mouse: Res<ButtonInput<MouseButton>>,
    build_state: Res<State<crate::states::BuildState>>,
    occupancy: Res<GridOccupancy>,
    module_query: Query<(Entity, &Module), With<WeaponTuning>>,
    // A gun standing on a hull tile: the tile owns the cell in the
    // occupancy map, so the gun is found by its footprint instead.
    player_guns: Query<(Entity, &Module, &ChildOf), With<WeaponTuning>>,
    // Turret guns stand on their turret's grid, not the hull's.
    turret_guns: Query<(Entity, &Module, &GlobalTransform), (With<WeaponTuning>, With<crate::combat::turret_deck::OnTurret>)>,
    existing: Query<(Entity, &TuningWindow)>,
    parent_query: Query<&ChildOf>,
    floating: Query<Entity, With<FloatingWindow>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    ship_query: Query<&GlobalTransform, (With<crate::components::Ship>, Without<Camera>)>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }

    // Right-click is overloaded in build mode: while Placing it DELETES the
    // block under the cursor (see handle_module_removal). Only open the
    // tuning window when no build tool is active, otherwise right-clicking a
    // weapon removed it and opened tuning for a block that no longer existed.
    if *build_state.get() != crate::states::BuildState::Inactive {
        return;
    }

    let Ok(ship_gt) = ship_query.single() else { return };
    // AFTERLIGHT_BUILD_CELL: a scripted run points at this ship cell instead.
    let scripted = crate::building::build_cell_override()
        .map(|c| ship_gt.transform_point(crate::building::grid_to_local(c).extend(0.0)).truncate());
    let Some(cursor_screen) = windows.single().ok().and_then(|w| w.cursor_position())
        .or(scripted.map(|_| Vec2::new(420.0, 120.0)))
    else { return };
    let Some(cursor_world) = scripted.or_else(|| camera_query.single().ok()
        .and_then(|(cam, gt)| cam.viewport_to_world_2d(gt, cursor_screen).ok()))
    else { return };

    // Grid cells are ship-local — transform the world-space cursor into the
    // ship's frame first (same math as the build ghost, see building/mod.rs).
    let local = ship_gt.rotation().inverse()
        * (Vec3::new(cursor_world.x, cursor_world.y, 0.0) - ship_gt.translation());
    let grid_pos = IVec2::new(
        (local.x / 66.0).round() as i32,
        ((local.y + 33.0) / 66.0).round() as i32,
    );

    let player = parent_query.get(occupancy.cells.get(&grid_pos).copied().unwrap_or(Entity::PLACEHOLDER)).ok().map(|p| p.parent());
    let on_hull = occupancy.cells.get(&grid_pos).and_then(|e| module_query.get(*e).ok()).or_else(|| {
        player_guns
            .iter()
            .filter(|(_, _, p)| Some(p.parent()) == player)
            .find(|(_, m, _)| {
                GridOccupancy::cells_for(m.grid_position, m.size, m.rotation, crate::building::footprints::footprint_override(m.module_type))
                    .contains(&grid_pos)
            })
            .map(|(e, m, _)| (e, m))
    });
    let on_turret = || {
        turret_guns
            .iter()
            .filter(|(_, _, gt)| gt.translation().truncate().distance(cursor_world) < 33.0)
            .map(|(e, m, _)| (e, m))
            .next()
    };
    let Some((entity, module)) = on_hull.or_else(on_turret) else { return };

    // One tuning window at a time — clicking another weapon retargets by
    // replacing the window instead of stacking them.
    for (content_entity, tw) in existing.iter() {
        if tw.module_entity == entity {
            return; // already open for this weapon
        }
        commands.entity(find_window_root(content_entity, &parent_query, &floating)).despawn();
    }

    spawn_tuning_window(&mut commands, entity, module.module_type, cursor_screen + Vec2::new(24.0, -12.0));
}

/// TuningWindow sits on the content node of a floating window — walk up to
/// the FloatingWindow root so despawning removes the whole frame.
fn find_window_root(
    content: Entity,
    parent_query: &Query<&ChildOf>,
    floating: &Query<Entity, With<FloatingWindow>>,
) -> Entity {
    let mut current = content;
    while let Ok(child_of) = parent_query.get(current) {
        current = child_of.parent();
        if floating.get(current).is_ok() {
            break;
        }
    }
    current
}

// ============================================================================
// SPAWN
// ============================================================================

fn spawn_tuning_window(
    commands: &mut Commands,
    module_entity: Entity,
    module_type: ModuleType,
    position: Vec2,
) {
    // A built gun gets the From-the-Depths window: parts, shell and what
    // each change does, side by side.
    if module_type == ModuleType::FiringPiece {
        super::gun_window::spawn_gun_window(commands, module_entity);
        return;
    }
    let title = if module_type == ModuleType::LaserEmitter {
        "LASER EMITTER".to_string()
    } else if module_type == ModuleType::FiringPiece {
        "BUILT GUN - SHELL & TUNING".to_string()
    } else if crate::combat::missile_design::is_built_launcher(module_type) {
        format!("{} - MISSILE & TUNING", module_type.name()).to_uppercase()
    } else {
        format!("{:?} - WEAPON TUNING", module_type).to_uppercase()
    };
    let content = spawn_floating_window(
        commands,
        &format!("tuning_{:?}", module_entity),
        &title,
        Vec2::new(340.0, 0.0), // height grows with content
        position,
    );

    commands.entity(content).insert(TuningWindow { module_entity, module_type });

    // A laser's numbers are its bank's (combat::lasers); its emitter only
    // chooses what it fires at.
    if module_type == ModuleType::LaserEmitter {
        spawn_window_section(commands, content, "LASER");
        let note = commands
            .spawn((
                Text::new("Fires its bank's charge. Lenses in a line in front of it carry the beam further. On point defence it shoots only at missiles the warners have spotted."),
                TextFont { font_size: FontSize::Px(10.0), ..default() },
                TextColor(WindowStyle::TEXT_DIM),
            ))
            .id();
        commands.entity(content).add_child(note);
        super::shell_designer::spawn_point_defence_switch(commands, content);
        return;
    }

    // AMMO SELECTOR — kinetics only
    if is_kinetic_weapon(module_type) {
        spawn_window_section(commands, content, "AMMUNITION");

        let grid = commands.spawn(
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
        ).id();

        for ammo in ALL_AMMO {
            let selected = ammo == KineticAmmoType::AP; // spawner default
            let btn = commands.spawn((
                Node {
                    padding: UiRect::new(Val::Px(7.0), Val::Px(7.0), Val::Px(3.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected { AMMO_SELECTED_BG } else { AMMO_BG }),
                BorderColor::all(if selected { ACCENT } else { WindowStyle::BORDER_COLOR }),
                Interaction::None,
                Button,
                AmmoTypeButton { ammo },
                Tooltip {
                    text: ammo.description().into(),
                    // Price is the only thing separating the exotic rounds
                    // from being a straight upgrade, so it belongs on the
                    // button rather than buried in the docking bill.
                    detail: Some(format!(
                        "{:.2}c/round  -  {:.0}% weight  -  {:.0}% muzzle vel",
                        crate::combat::ammo_types::rearm_price(module_type, Some(ammo), 1),
                        ammo.weight_mult() * 100.0,
                        ammo.velocity_mult() * 100.0,
                    )),
                },
            )).id();
            let label = commands.spawn((
                Text::new(ammo.name()),
                TextFont { font_size: FontSize::Px(10.0), ..default() },
                TextColor(if selected { ACCENT } else { WindowStyle::TEXT_COLOR }),
            )).id();
            commands.entity(btn).add_child(label);
            commands.entity(grid).add_child(btn);
        }
        commands.entity(content).add_child(grid);

        let desc = commands.spawn((
            Text::new(KineticAmmoType::AP.description()),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(WindowStyle::TEXT_DIM),
            AmmoDescText,
        )).id();
        commands.entity(content).add_child(desc);
    }

    // MISSILE DESIGNER — a launcher's missile is designed part by part.
    if crate::combat::missile_design::is_built_launcher(module_type) {
        super::missile_designer::spawn_missile_section(commands, content, &crate::combat::missile_design::MissileDesign::default_for(module_type));
    }

    spawn_tuning_controls(commands, content, module_type);
}

/// The sliders, the power draw, the live stats and RESET: the lower half of
/// every weapon's window, and the first column of a built gun's
/// (super::gun_window).
pub fn spawn_tuning_controls(commands: &mut Commands, content: Entity, module_type: ModuleType) {
    // SLIDERS
    spawn_window_section(commands, content, "TUNING");

    let mut fields = vec![
        (TuningField::FireRate, "FIRE RATE"),
        (TuningField::Damage, "DAMAGE"),
    ];
    // Lasers are instant beams — no projectile to speed up.
    if module_type != ModuleType::Laser {
        fields.insert(0, (TuningField::Velocity, velocity_label(module_type)));
    }
    // Turret guns get a traverse (aim-speed) slider.
    if crate::sprite_map::turret_barrel_sprite(module_type).is_some() {
        fields.push((TuningField::Traverse, "TRAVERSE"));
    }

    for (field, label) in fields {
        spawn_slider_row(commands, content, field, label);
    }

    // POWER
    spawn_window_section(commands, content, "POWER");

    let power_text = commands.spawn((
        Text::new("DRAW: -"),
        TextFont { font_size: FontSize::Px(12.0), ..default() },
        TextColor(ACCENT),
        PowerReadoutText,
        Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
    )).id();
    commands.entity(content).add_child(power_text);

    // Ship budget bar
    let bar_bg = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(8.0),
            margin: UiRect::vertical(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(SLIDER_BG),
    )).id();
    let bar_fill = commands.spawn((
        Node {
            width: Val::Percent(0.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.7, 0.4)),
        ShipPowerBarFill,
    )).id();
    commands.entity(bar_bg).add_child(bar_fill);
    commands.entity(content).add_child(bar_bg);

    let ship_power_text = commands.spawn((
        Text::new("SHIP: -"),
        TextFont { font_size: FontSize::Px(10.0), ..default() },
        TextColor(WindowStyle::TEXT_DIM),
        ShipPowerText,
    )).id();
    commands.entity(content).add_child(ship_power_text);

    // LIVE STATS
    let stats = commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(11.0), ..default() },
        TextColor(WindowStyle::TEXT_COLOR),
        StatsReadoutText,
        Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
    )).id();
    commands.entity(content).add_child(stats);

    // RESET
    let bottom = commands.spawn(
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        },
    ).id();
    let reset = commands.spawn((
        Node {
            padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(4.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(AMMO_BG),
        BorderColor::all(WindowStyle::BORDER_COLOR),
        Interaction::None,
        Button,
        ResetTuningButton,
        Tooltip { text: "Reset all sliders to 1.0x".into(), detail: None },
    )).id();
    let reset_label = commands.spawn((
        Text::new("RESET"),
        TextFont { font_size: FontSize::Px(11.0), ..default() },
        TextColor(WindowStyle::TEXT_DIM),
    )).id();
    commands.entity(reset).add_child(reset_label);
    commands.entity(bottom).add_child(reset);
    commands.entity(content).add_child(bottom);
}

fn spawn_slider_row(
    commands: &mut Commands,
    parent: Entity,
    field: TuningField,
    label: &str,
) {
    let row = commands.spawn(
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
    ).id();

    // Label + value line
    let header = commands.spawn(
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
    ).id();
    let label_text = commands.spawn((
        Text::new(label),
        TextFont { font_size: FontSize::Px(11.0), ..default() },
        TextColor(WindowStyle::TEXT_COLOR),
    )).id();
    let value_text = commands.spawn((
        Text::new("1.00x"),
        TextFont { font_size: FontSize::Px(11.0), ..default() },
        TextColor(ACCENT),
        TuningValueText { field },
    )).id();
    commands.entity(header).add_children(&[label_text, value_text]);

    // Track (interactive) with fill + baseline tick. FocusPolicy::Block so
    // dragging over the track doesn't also drag the window underneath.
    let track = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(14.0),
            margin: UiRect::top(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(SLIDER_BG),
        Interaction::None,
        Button,
        FocusPolicy::Block,
        TuningSliderTrack { field },
    )).id();

    let fill = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(mult_to_fraction(1.0) * 100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(SLIDER_FILL),
        FocusPolicy::Pass,
        TuningSliderFill { field },
    )).id();

    // Baseline tick at 1.0×
    let tick = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(mult_to_fraction(1.0) * 100.0),
            top: Val::Px(0.0),
            width: Val::Px(2.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(BASELINE_TICK),
        FocusPolicy::Pass,
    )).id();

    commands.entity(track).add_children(&[fill, tick]);
    commands.entity(row).add_children(&[header, track]);
    commands.entity(parent).add_child(row);
}

// ============================================================================
// INTERACTION
// ============================================================================

/// Press on a track starts a drag; the value keeps following the cursor until
/// the button is released, even if the cursor slides off the track.
pub fn tuning_slider_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<ActiveSliderDrag>,
    // UiGlobalTransform, NOT GlobalTransform: Bevy 0.19 no longer keeps the
    // classic GlobalTransform in sync for UI nodes, so reading it gives a
    // stale/identity value — same bug as map_click_system (see ui/mod.rs).
    tracks: Query<(&TuningSliderTrack, &Interaction, &ComputedNode, &bevy::ui::UiGlobalTransform)>,
    windows: Query<&Window>,
    tuning_windows: Query<&TuningWindow>,
    mut tuning_query: Query<&mut WeaponTuning>,
) {
    if mouse.just_released(MouseButton::Left) {
        drag.0 = None;
    }

    let Ok(window) = windows.single() else { return };
    // cursor_position() is logical pixels; ComputedNode/UiGlobalTransform are
    // physical — without the scale factor every hit lands wrong on Retina.
    let Some(cursor) = window.cursor_position().map(|p| p * window.scale_factor()) else { return };

    // Start a drag
    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        for (track, interaction, _, _) in tracks.iter() {
            if *interaction == Interaction::Pressed {
                drag.0 = Some(track.field);
                break;
            }
        }
    }

    let Some(active_field) = drag.0 else { return };
    if !mouse.pressed(MouseButton::Left) {
        drag.0 = None;
        return;
    }

    // Value from cursor position over the active track. normalize_point maps
    // into the node's local space (−0.5..0.5 inside), and keeps extrapolating
    // beyond that when the cursor slides off the track mid-drag — the clamp
    // pins it to the ends.
    for (track, _, node, transform) in tracks.iter() {
        if track.field != active_field { continue; }
        let Some(norm) = node.normalize_point(*transform, cursor) else { continue };
        let normalized = (norm.x + 0.5).clamp(0.0, 1.0);
        let value = TUNING_MIN + normalized * (TUNING_MAX - TUNING_MIN);

        let Ok(tw) = tuning_windows.single() else { return };
        if let Ok(mut tuning) = tuning_query.get_mut(tw.module_entity) {
            tuning.set(active_field, value);
        }
        return;
    }
}

pub fn ammo_button_click(
    buttons: Query<(&AmmoTypeButton, &Interaction), Changed<Interaction>>,
    tuning_windows: Query<&TuningWindow>,
    mut ammo_query: Query<&mut SelectedAmmo>,
    mut all_buttons: Query<(&AmmoTypeButton, &mut BackgroundColor, &mut BorderColor, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut desc_query: Query<&mut Text, With<AmmoDescText>>,
) {
    let mut clicked: Option<KineticAmmoType> = None;
    for (btn, interaction) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            clicked = Some(btn.ammo);
        }
    }
    let Some(ammo) = clicked else { return };
    let Ok(tw) = tuning_windows.single() else { return };
    let Ok(mut selected) = ammo_query.get_mut(tw.module_entity) else { return };
    selected.0 = ammo;

    // Update visuals: highlight selection, dim the rest
    for (btn, mut bg, mut border, children) in all_buttons.iter_mut() {
        let is_sel = btn.ammo == ammo;
        *bg = BackgroundColor(if is_sel { AMMO_SELECTED_BG } else { AMMO_BG });
        *border = BorderColor::all(if is_sel { ACCENT } else { WindowStyle::BORDER_COLOR });
        for child in children.iter() {
            if let Ok(mut tc) = text_colors.get_mut(child) {
                tc.0 = if is_sel { ACCENT } else { WindowStyle::TEXT_COLOR };
            }
        }
    }

    if let Ok(mut desc) = desc_query.single_mut() {
        desc.0 = ammo.description().to_string();
    }
}

pub fn reset_tuning_click(
    buttons: Query<&Interaction, (With<ResetTuningButton>, Changed<Interaction>)>,
    tuning_windows: Query<&TuningWindow>,
    mut tuning_query: Query<&mut WeaponTuning>,
) {
    for interaction in buttons.iter() {
        if *interaction != Interaction::Pressed { continue; }
        let Ok(tw) = tuning_windows.single() else { return };
        if let Ok(mut tuning) = tuning_query.get_mut(tw.module_entity) {
            *tuning = WeaponTuning::default();
        }
    }
}

// ============================================================================
// LIVE REFRESH — fills, value texts, power bar, stat readout
// ============================================================================

pub fn tuning_window_refresh(
    tuning_windows: Query<&TuningWindow>,
    power: Res<PowerState>,
    module_query: Query<(&Module, &Weapon, &WeaponTuning, Option<&SelectedAmmo>)>,
    mut fills: Query<(&TuningSliderFill, &mut Node)>,
    mut value_texts: Query<(&TuningValueText, &mut Text)>,
    mut power_text: Query<&mut Text, (With<PowerReadoutText>, Without<TuningValueText>, Without<ShipPowerText>, Without<StatsReadoutText>)>,
    mut ship_bar: Query<(&mut Node, &mut BackgroundColor), (With<ShipPowerBarFill>, Without<TuningSliderFill>)>,
    mut ship_text: Query<&mut Text, (With<ShipPowerText>, Without<TuningValueText>, Without<PowerReadoutText>, Without<StatsReadoutText>)>,
    mut stats_text: Query<&mut Text, (With<StatsReadoutText>, Without<TuningValueText>, Without<PowerReadoutText>, Without<ShipPowerText>)>,
) {
    let Ok(tw) = tuning_windows.single() else { return };
    let Ok((module, weapon, tuning, ammo)) = module_query.get(tw.module_entity) else { return };

    for (fill, mut node) in fills.iter_mut() {
        node.width = Val::Percent(mult_to_fraction(tuning.get(fill.field)) * 100.0);
    }
    for (vt, mut text) in value_texts.iter_mut() {
        text.0 = format!("{:.2}x", tuning.get(vt.field));
    }

    if let Ok(mut text) = power_text.single_mut() {
        text.0 = format!("DRAW: {:.0} MW  ({:.2}x base)", module.power_consumption, tuning.power_factor());
    }

    let gen = power.total_power_generation.max(1.0);
    let ratio = (power.total_power_consumption / gen).clamp(0.0, 1.0);
    if let Ok((mut node, mut bg)) = ship_bar.single_mut() {
        node.width = Val::Percent(ratio * 100.0);
        bg.0 = if ratio < 0.7 {
            Color::srgb(0.3, 0.7, 0.4)
        } else if ratio < 0.95 {
            Color::srgb(0.85, 0.7, 0.25)
        } else {
            Color::srgb(0.85, 0.3, 0.25)
        };
    }
    if let Ok(mut text) = ship_text.single_mut() {
        text.0 = format!("SHIP: {:.0} / {:.0} MW", power.total_power_consumption, power.total_power_generation);
    }

    if let Ok(mut text) = stats_text.single_mut() {
        let ammo_dmg = ammo.map(|a| a.0.damage_mult()).unwrap_or(1.0);
        let dps = weapon.damage * ammo_dmg * weapon.fire_rate;
        let mut line = format!("DPS {:.0}   ROF {:.2}/s", dps, weapon.fire_rate);
        if is_kinetic_weapon(tw.module_type) {
            let ammo_vel = ammo.map(|a| a.0.velocity_mult()).unwrap_or(1.0);
            let vel = base_projectile_speed(tw.module_type) * tuning.velocity * ammo_vel;
            line.push_str(&format!("   VEL {:.0}", vel));
        }
        // Heat: the real cost of hot tuning. Above ambient cooling the gun
        // can't sustain fire — it will thermally throttle mid-fight.
        let heat = weapon_heat_per_second(tuning.power_factor());
        if heat > AMBIENT_COOLING_RATE {
            line.push_str(&format!("\nHEAT {:.1}/s - OVERHEATS UNDER SUSTAINED FIRE", heat));
        } else {
            line.push_str(&format!("\nHEAT {:.1}/s (sustainable)", heat));
        }
        text.0 = line;
    }
}

/// OnExit(StationDocked): tuning is a dock-side workshop activity — close any
/// open windows when leaving the station.
pub fn despawn_tuning_windows(
    content_query: Query<Entity, With<TuningWindow>>,
    parent_query: Query<&ChildOf>,
    floating: Query<Entity, With<FloatingWindow>>,
    mut drag: ResMut<ActiveSliderDrag>,
    mut commands: Commands,
) {
    drag.0 = None;
    for content in content_query.iter() {
        commands.entity(find_window_root(content, &parent_query, &floating)).despawn();
    }
}
