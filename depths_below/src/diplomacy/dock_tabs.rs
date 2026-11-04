//! The station window's Bar and Comm directory tabs (ui::spawn_docking_menu
//! builds the window and its Market).
//!
//! - **Bar:** one to three people with something to say, and what they say is
//!   true: drawn from what the simulation is doing right now -- a battle
//!   group nearby, a convoy running through, a border slipping, a hunter
//!   wing someone put on you.
//! - **Comm directory:** the contacts at this starport (contacts.rs): pick one,
//!   talk, and they get to know you -- and offer you work (jobs.rs), which A
//!   takes. The contact who holds the Summit Record's current step offers it
//!   two ways, 1 for the deal and 2 for force (narrative::summit).

use bevy::prelude::*;
use rand::{seq::SliceRandom, Rng, SeedableRng};

use super::contacts::{self, Contacts};
use super::jobs::{self, JobStatus, Jobs};
use super::FactionStanding;
use crate::ai_ship::components::{faction_display_name, AiShipType, SimBehavior, WorldSimulation};
use crate::ai_ship::fleet::{FleetKind, Mission};
use crate::celestial::resources::{GalaxyMap, SystemDiscovery, SystemStreamingManager};
use crate::celestial::territory::{border_factions, BorderWar};
use crate::components::*;
use crate::resources::MarketEvents;
use crate::narrative::summit;
use crate::ui::theme::{ThemeColors, ThemeFonts};

/// Q / E switch tabs; in the Comm directory Up / Down pick a contact, Enter
/// talks to them (and asks for work), A takes the work they offer, and 1 / 2
/// take the story's step the deal way or by force.
#[allow(clippy::too_many_arguments)]
pub fn dock_tab_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut DockTabState, With<DockingOverlay>>,
    market: Res<MarketEvents>,
    galaxy: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    mut contacts: ResMut<Contacts>,
    mut work: ResMut<Jobs>,
    world: (Res<WorldSimulation>, Res<BorderWar>),
    mut inventory: ResMut<crate::resources::Inventory>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut currency: ResMut<crate::resources::Currency>,
    mut story: MessageWriter<jobs::SummitStepWon>,
) {
    let Ok(mut state) = windows.single_mut() else { return };
    if !state.tabs {
        return;
    }
    let i = DockTab::ALL.iter().position(|t| *t == state.tab).unwrap_or(0);
    if keyboard.just_pressed(KeyCode::KeyE) {
        state.tab = DockTab::ALL[(i + 1) % DockTab::ALL.len()];
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        state.tab = DockTab::ALL[(i + DockTab::ALL.len() - 1) % DockTab::ALL.len()];
    }
    if state.tab != DockTab::Comm {
        return;
    }
    let (Some(faction), Some(system)) = (market.dock_terms.as_ref().and_then(|t| t.faction), streaming.loaded_system) else {
        return;
    };
    let here = contacts::present_at(&galaxy, &contacts, system, faction);
    if here.is_empty() {
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        state.contact = (state.contact + 1) % here.len();
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        state.contact = (state.contact + here.len() - 1) % here.len();
    }
    state.contact = state.contact.min(here.len() - 1);
    let def = here[state.contact];
    if keyboard.just_pressed(KeyCode::Enter) {
        contacts.talk(def);
        // Meeting the Abbess opens the chain.
        if def.id == summit::ABBESS && !contacts.summit.commissioned {
            contacts.summit.commissioned = true;
            contacts.summit.hailed = true;
            info!("SUMMIT commissioned by the Abbess");
        }
        if work.of(&def.id).is_none() {
            let (sim, war) = (&world.0, &world.1);
            let at = jobs::World { sim, galaxy: &galaxy, war, here: system };
            if let Some(job) = jobs::offer_for(def, contacts.relationship(&def.id), &at, &mut rand::thread_rng()) {
                info!("JOB offered by {}: {} ({} cr) - {}", def.name, job.title, job.reward, job.brief);
                work.push(job);
            }
        }
        // Change-detection on the state is what redraws the panel.
        state.set_changed();
    }
    let way = if keyboard.just_pressed(KeyCode::Digit1) {
        Some(false)
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(true)
    } else {
        None
    };
    if let (Some(forced), Some(step)) = (way, contacts.summit.current().filter(|_| contacts.summit.holds_current(def))) {
        let (sim, war) = (&world.0, &world.1);
        let at = jobs::World { sim, galaxy: &galaxy, war, here: system };
        let (message, kind) = match jobs::story_offer(step, forced, def, &at, &mut rand::thread_rng()) {
            jobs::StoryOffer::Pay(price) if currency.credits >= price => {
                currency.credits -= price;
                story.write(jobs::SummitStepWon(jobs::StoryMark { step, forced }));
                (format!("Paid {price} credits."), crate::events::NotificationType::Info)
            }
            jobs::StoryOffer::Pay(price) => (format!("That takes {price} credits. You have {}.", currency.credits), crate::events::NotificationType::Warning),
            jobs::StoryOffer::Work(job) => {
                let title = job.title.clone();
                info!("SUMMIT step {step} taken ({}): {} - {}", if forced { "force" } else { "deal" }, job.title, job.brief);
                match work.begin_story(job, |item, count| inventory.add_item(item, count)) {
                    Ok(()) => (format!("Taken: {title}"), crate::events::NotificationType::Info),
                    Err(why) => (why.to_string(), crate::events::NotificationType::Warning),
                }
            }
        };
        notes.write(crate::events::ShowNotification { message, notification_type: kind, duration: 4.0 });
        state.set_changed();
    }
    if keyboard.just_pressed(KeyCode::KeyA) && work.of(&def.id).is_some_and(|j| j.status == JobStatus::Offered) {
        let taken = work.accept(&def.id, |item, count| inventory.add_item(item, count));
        info!("JOB accept from {}: {:?} ({} in hand)", def.name, taken, work.active().count());
        let (message, kind) = match taken {
            Ok(()) => (format!("Job taken: {}", work.of(&def.id).map_or("", |j| j.title.as_str())), crate::events::NotificationType::Info),
            Err(why) => (why.to_string(), crate::events::NotificationType::Warning),
        };
        notes.write(crate::events::ShowNotification { message, notification_type: kind, duration: 3.5 });
        state.set_changed();
    }
}

/// The window's cargo list: drawn when the window opens and whenever the hold
/// changes while you're docked.
pub fn refresh_dock_hold(
    mut commands: Commands,
    lists: Query<Entity, Added<DockHoldList>>,
    all_lists: Query<Entity, With<DockHoldList>>,
    inventory: Res<crate::resources::Inventory>,
) {
    let targets: Vec<Entity> = if inventory.is_changed() { all_lists.iter().collect() } else { lists.iter().collect() };
    for list in targets {
        commands.entity(list).despawn_related::<Children>();
        let mut items: Vec<_> = inventory.items.iter().collect();
        items.sort_by_key(|(item, _)| item.name());
        commands.entity(list).with_children(|l| {
            if items.is_empty() {
                l.spawn(line("Empty", ThemeFonts::BODY_SMALL, ThemeColors::TEXT_MUTED));
            }
            for (item, n) in items {
                l.spawn(line(format!("{} x{n}", item.name()), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_SECONDARY));
            }
        });
    }
}

/// Offers you didn't take lapse when you undock.
pub fn lapse_offers(mut work: ResMut<Jobs>) {
    work.drop_offers();
}

/// Redraw the tabs when the window opens or its state changes: which panel
/// shows, which tab button is lit, the key hints, and the Bar and Comm
/// directory's contents.
#[allow(clippy::too_many_arguments)]
pub fn refresh_dock_tabs(
    mut commands: Commands,
    windows: Query<Ref<DockTabState>, With<DockingOverlay>>,
    mut panels: Query<(Entity, &DockTabPanel, &mut Node)>,
    mut buttons: Query<(&DockTabButton, &mut BackgroundColor, &mut BorderColor, &Children)>,
    mut button_text: Query<&mut TextColor>,
    mut footer: Query<&mut Text, With<DockFooterHint>>,
    world: (Res<WorldSimulation>, Res<GalaxyMap>, Res<BorderWar>, Res<FactionStanding>, Res<SystemStreamingManager>, Res<Time>),
    market: Res<MarketEvents>,
    contacts: Res<Contacts>,
    work: Res<Jobs>,
) {
    let Ok(state) = windows.single() else { return };
    // A story step paid for at the counter is settled a system later
    // (summit::advance), so a change to Contacts redraws too.
    if !state.is_changed() && !contacts.is_changed() {
        return;
    }
    let (sim, galaxy, war, standing, streaming, time) = world;
    for (entity, panel, mut node) in panels.iter_mut() {
        let show = panel.0 == state.tab;
        node.display = if show { Display::Flex } else { Display::None };
        if show && panel.0 != DockTab::Market {
            commands.entity(entity).despawn_related::<Children>();
            let owner = market.dock_terms.as_ref().and_then(|t| t.faction);
            match panel.0 {
                DockTab::Bar => fill_bar(&mut commands, entity, &sim, &galaxy, &war, &standing, &streaming, owner, time.elapsed_secs()),
                DockTab::Comm => fill_comm(&mut commands, entity, &galaxy, &contacts, &work, &streaming, owner, state.contact),
                DockTab::Market => {}
            }
        }
    }
    for (button, mut bg, mut border, children) in buttons.iter_mut() {
        let on = button.0 == state.tab;
        bg.0 = if on { ThemeColors::BG_ELEVATED } else { ThemeColors::BG_CARD };
        *border = BorderColor::all(if on { ThemeColors::ACCENT_CYAN } else { Color::NONE });
        for child in children.iter() {
            if let Ok(mut color) = button_text.get_mut(child) {
                color.0 = if on { ThemeColors::TEXT_PRIMARY } else { ThemeColors::TEXT_MUTED };
            }
        }
    }
    if let Ok(mut text) = footer.single_mut() {
        let want = match state.tab {
            DockTab::Market => "Up/Down  select    Left/Right  cargo choice    Enter  buy    Esc  undock",
            DockTab::Bar => "Q / E  switch tab    Esc  undock",
            DockTab::Comm => "Up/Down  contact    Enter  talk    A  take the job    1 / 2  story: deal or force    Q / E  tab    Esc  undock",
        };
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

/// A line of text in the window's style.
fn line(text: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (Text::new(text.into()), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color))
}

fn card() -> (Node, BackgroundColor) {
    (
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)), ..default() },
        BackgroundColor(ThemeColors::BG_CARD),
    )
}

/// Who's at the bar, by the station's owner.
fn patrons(owner: Option<AiShipType>) -> &'static [&'static str] {
    use AiShipType::*;
    match owner {
        Some(TerranHegemony) => &["An off-duty rating", "A dock foreman", "A pilot in a Hegemony jacket"],
        Some(GildedThrone) => &["A junior auditor", "A bodyguard on her break", "A man counting his chips twice"],
        Some(MerchantPrinces) => &["A pedlar between contracts", "A Guild clerk", "Someone selling the table they sit at"],
        Some(FreeHaulersCompact) => &["A tug pilot", "A miner with a broken hand", "A union steward"],
        Some(TheSilence) => &["A pilgrim, writing instead of speaking", "A novice who whispers", "A visitor who stopped talking"],
        Some(BrokenChoir) => &["Three voices from one mouth", "A chorister, off key", "Somebody humming your name"],
        Some(SynthesisCollective) => &["A body the Collective is using today", "A graft-tender", "Two of them, talking as one"],
        Some(CorpseStars) => &["A procession hand", "An old cultist, older than he looks", "A mourner"],
        Some(StellarPreserve) => &["A keeper off shift", "A ranger with herd-dust on her boots", "A specimen handler"],
        Some(RecursiveKingdom) => &["Rex, aged thirty", "Rex, aged twelve", "Rex, aged sixty-one"],
        _ => &["A hauler", "A pilot", "A dock hand"],
    }
}

/// What the bar is talking about, drawn from what is actually happening:
/// one to three rumours, the same for a few minutes at a time.
pub fn rumours(
    sim: &WorldSimulation,
    galaxy: &GalaxyMap,
    war: &BorderWar,
    standing: &FactionStanding,
    system_id: u32,
    seed: u64,
) -> Vec<String> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let name_of = |id: u32| galaxy.systems.iter().find(|s| s.id == id).map_or("somewhere".to_string(), |s| s.name.clone());
    let mut near: Vec<u32> = vec![system_id];
    near.extend(crate::celestial::galaxy::nearest_neighbors(galaxy, system_id, 3));
    let mut said: Vec<String> = Vec::new();
    let members = |fleet: u32| sim.ships.iter().filter(|s| s.fleet == fleet && s.behavior != SimBehavior::Dead).collect::<Vec<_>>();

    // A battle group nearby.
    if let Some(f) = sim.fleets.iter().filter(|f| f.kind == FleetKind::BattleGroup && near.contains(&f.system_id) && f.mission == Mission::None).collect::<Vec<_>>().choose(&mut rng) {
        let m = members(f.id);
        if let Some(lead) = m.first() {
            let class = lead.class.and_then(|k| crate::ai_ship::roster::find(lead.faction, k)).map_or("capital ship", |c| c.name);
            let at = if f.system_id == system_id { "here, in this system".to_string() } else { format!("in {}", name_of(f.system_id)) };
            said.push(format!("A {} battle group is {at}: {} hulls behind a {class}.", faction_display_name(f.faction), m.len()));
        }
    }
    // A convoy running through.
    if let Some(f) = sim.fleets.iter().filter(|f| f.kind == FleetKind::Convoy && f.system_id == system_id).collect::<Vec<_>>().choose(&mut rng) {
        let m = members(f.id);
        let civilian = |s: &&&crate::ai_ship::components::SimulatedShip| {
            crate::ai_ship::fleet::role_of(s.faction, s.class) == crate::ai_ship::roster::ClassRole::Civilian
        };
        let haulers = m.iter().filter(civilian).count();
        let guards = m.len() - haulers;
        let guard = match m.iter().find(|s| s.faction != f.faction) {
            Some(g) => format!(", with {guards} {} guns riding along", faction_display_name(g.faction)),
            None if guards > 0 => format!(", {guards} guarding"),
            None => ", and nobody guarding it".to_string(),
        };
        said.push(format!("A {} convoy is running through here: {haulers} haulers{guard}.", faction_display_name(f.faction)));
    }
    // A border slipping.
    let slipping: Vec<(u32, AiShipType, f32)> = near
        .iter()
        .filter_map(|&id| war.contest(id))
        .flat_map(|c| c.claims.iter().filter(|(_, s)| *s >= 25.0).map(move |&(f, s)| (c.system_id, f, s)))
        .collect();
    if let Some(&(id, f, share)) = slipping.choose(&mut rng) {
        said.push(format!("{} is slipping. The {} hold {share:.0}% of it now.", name_of(id), faction_display_name(f)));
    }
    // Someone after you.
    if let Some(f) = sim.fleets.iter().filter(|f| matches!(f.mission, Mission::Hunt { .. }) && f.system_id == system_id).collect::<Vec<_>>().choose(&mut rng) {
        said.push(format!(
            "Someone's sent a {} wing after a ship like yours. {} hulls. I'd leave by a different door.",
            faction_display_name(f.faction),
            members(f.id).len()
        ));
    }
    // The news.
    if let Some((id, _, to)) = war.news.iter().rev().find(|(id, ..)| galaxy.systems.iter().any(|s| s.id == *id && s.discovery != SystemDiscovery::Unknown)) {
        said.push(format!("Did you hear? {} fell to the {}.", name_of(*id), faction_display_name(*to)));
    }
    // Who wants you dead.
    let haters: Vec<AiShipType> = border_factions(galaxy, system_id).into_iter().filter(|f| standing.attacks_player(*f)).collect();
    if let Some(f) = haters.choose(&mut rng) {
        if let Some(their) = galaxy.systems.iter().find(|s| s.faction == Some(*f) && near.contains(&s.id)) {
            said.push(format!("The {} count you hostile, you know. Their patrols cross {}.", faction_display_name(*f), their.name));
        }
    }
    // Somewhere nobody goes -- or, here.
    if let Some(h) = galaxy.systems.iter().find(|s| s.haunt.is_some() && near.contains(&s.id)) {
        let boss = faction_display_name(h.haunt.unwrap());
        said.push(if h.id == system_id {
            format!("Nobody leaves the dock here if they can help it. The {boss} is out there.")
        } else {
            format!("Nobody goes to {}. The {boss} is there.", h.name)
        });
    }
    said.shuffle(&mut rng);
    let keep = rng.gen_range(1..=3);
    said.truncate(keep);
    if said.is_empty() {
        said.push("A quiet night. Nobody here has anything worth selling you.".into());
    }
    said
}

#[allow(clippy::too_many_arguments)]
fn fill_bar(
    commands: &mut Commands,
    panel: Entity,
    sim: &WorldSimulation,
    galaxy: &GalaxyMap,
    war: &BorderWar,
    standing: &FactionStanding,
    streaming: &SystemStreamingManager,
    owner: Option<AiShipType>,
    now: f32,
) {
    let system = streaming.loaded_system.unwrap_or(0);
    // The same talk for five minutes at a stretch, not a new crowd every tab.
    let seed = system as u64 * 1_000_003 + (now / 300.0) as u64;
    let said = rumours(sim, galaxy, war, standing, system, seed);
    let who = patrons(owner);
    commands.entity(panel).with_children(|p| {
        p.spawn(line("WHO'S TALKING", ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED));
        for (i, rumour) in said.into_iter().enumerate() {
            p.spawn(card()).with_children(|c| {
                c.spawn(line(who[i % who.len()], ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED));
                c.spawn(line(format!("\"{rumour}\""), 16.0, ThemeColors::TEXT_PRIMARY));
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn fill_comm(
    commands: &mut Commands,
    panel: Entity,
    galaxy: &GalaxyMap,
    contacts: &Contacts,
    work: &Jobs,
    streaming: &SystemStreamingManager,
    owner: Option<AiShipType>,
    selected: usize,
) {
    let here = match (owner, streaming.loaded_system) {
        (Some(f), Some(system)) => contacts::present_at(galaxy, contacts, system, f),
        _ => Vec::new(),
    };
    commands.entity(panel).with_children(|p| {
        if here.is_empty() {
            p.spawn(line("No one here takes calls.", ThemeFonts::BODY, ThemeColors::TEXT_MUTED));
            return;
        }
        p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), ..default() }).with_children(|row| {
            // Who's here.
            row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), width: Val::Px(270.0), flex_shrink: 0.0, ..default() })
                .with_children(|list| {
                    list.spawn(line("CONTACTS", ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED));
                    for (i, def) in here.iter().enumerate() {
                        let on = i == selected.min(here.len() - 1);
                        let rel = contacts.relationship(&def.id);
                        let met = contacts.state(&def.id).is_some_and(|s| s.met);
                        list.spawn((
                            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)), border: UiRect::left(Val::Px(3.0)), ..default() },
                            BackgroundColor(if on { ThemeColors::BG_ELEVATED } else { ThemeColors::BG_CARD }),
                            BorderColor::all(if on { ThemeColors::ACCENT_CYAN } else { Color::NONE }),
                        )).with_children(|c| {
                            let star = if def.story { "  *" } else { "" };
                            c.spawn(line(format!("{}{star}", def.name), 15.0, if on { Color::WHITE } else { ThemeColors::TEXT_PRIMARY }));
                            let known = if met { format!("{}  |  relationship {rel:.0}", def.role.label()) } else { format!("{}  |  not met", def.role.label()) };
                            c.spawn(line(known, ThemeFonts::CAPTION, ThemeColors::TEXT_SECONDARY));
                        });
                    }
                });
            // The one you're talking to.
            let def = here[selected.min(here.len() - 1)];
            let state = contacts.state(&def.id);
            let rel = state.map_or(0.0, |s| s.relationship);
            let said = contacts::line_for(def, rel, state.map_or(0, |s| s.talks)).to_string();
            row.spawn((
                Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::all(Val::Px(16.0)), flex_grow: 1.0, ..default() },
                BackgroundColor(ThemeColors::BG_CARD),
            )).with_children(|d| {
                d.spawn(line(def.name.clone(), ThemeFonts::H3, ThemeColors::TEXT_TITLE));
                d.spawn(line(format!("{}  |  {}", faction_display_name(def.faction), def.role.label()), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_SECONDARY));
                // Relationship, as a bar.
                d.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), ..default() }, BackgroundColor(ThemeColors::BG_INPUT)))
                    .with_children(|bar| {
                        bar.spawn((Node { width: Val::Percent(rel.clamp(0.0, 100.0)), height: Val::Percent(100.0), ..default() }, BackgroundColor(ThemeColors::ACCENT_GREEN)));
                    });
                d.spawn((line(format!("\"{said}\""), 17.0, ThemeColors::TEXT_PRIMARY), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
                story_box(d, def, contacts, work, galaxy);
                // The work they have for you.
                let held = work.held();
                d.spawn((
                    Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(12.0)), margin: UiRect::top(Val::Px(10.0)), border: UiRect::left(Val::Px(3.0)), ..default() },
                    BackgroundColor(ThemeColors::BG_ELEVATED),
                    BorderColor::all(ThemeColors::ACCENT_YELLOW),
                )).with_children(|w| match work.of(&def.id) {
                    Some(job) if job.status == JobStatus::Offered => {
                        w.spawn(line("WORK", ThemeFonts::CAPTION, ThemeColors::ACCENT_YELLOW));
                        w.spawn(line(job.title.clone(), 16.0, ThemeColors::TEXT_TITLE));
                        w.spawn(line(job.brief.clone(), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY));
                        w.spawn(line(format!("Pays {} cr", job.reward), ThemeFonts::BODY_SMALL, ThemeColors::ACCENT_GREEN));
                        let take = if held >= jobs::MAX_ACTIVE { "Your hands are full: three jobs at a time.".to_string() } else { format!("A  take the job    ({held}/{} in hand)", jobs::MAX_ACTIVE) };
                        w.spawn(line(take, ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED));
                    }
                    Some(job) => {
                        w.spawn(line("IN HAND", ThemeFonts::CAPTION, ThemeColors::ACCENT_YELLOW));
                        w.spawn(line(job.title.clone(), 16.0, ThemeColors::TEXT_TITLE));
                        w.spawn(line(job.brief.clone(), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY));
                        w.spawn(line(format!("{}  |  pays {} cr", job.progress(), job.reward), ThemeFonts::CAPTION, ThemeColors::TEXT_SECONDARY));
                    }
                    None => {
                        w.spawn(line("WORK", ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED));
                        w.spawn(line("Talk to them and ask.", ThemeFonts::BODY_SMALL, ThemeColors::TEXT_MUTED));
                    }
                });
            });
        });
    });
}

/// What a contact has to do with the Summit Record: the Abbess's reading of
/// where it stands, the current step's two ways, or a note that they hold a
/// piece for later.
fn story_box(d: &mut ChildSpawnerCommands, def: &contacts::ContactDef, contacts: &Contacts, work: &Jobs, galaxy: &GalaxyMap) {
    let progress = &contacts.summit;
    let boxed = (
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(12.0)), margin: UiRect::top(Val::Px(10.0)), border: UiRect::left(Val::Px(3.0)), ..default() },
        BackgroundColor(ThemeColors::BG_ELEVATED),
        BorderColor::all(ThemeColors::ACCENT_ORANGE),
    );
    let header = |n: u8| format!("THE SUMMIT RECORD   {n} of {}", summit::STEPS_IN_ALL);
    if def.id == summit::ABBESS {
        d.spawn(boxed).with_children(|b| {
            b.spawn(line(header(progress.done), ThemeFonts::CAPTION, ThemeColors::ACCENT_ORANGE));
            let said = if !progress.commissioned {
                "Talk to her.".to_string()
            } else if progress.done == 0 {
                summit::commission(galaxy)
            } else {
                summit::abbess_after(progress.done, galaxy)
            };
            b.spawn(line(said, ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY));
        });
        return;
    }
    if !def.story {
        return;
    }
    let Some(n) = progress.current().filter(|_| progress.holds_current(def)) else {
        let had = (1..=progress.done).any(|n| summit::contact_of(n).is_some_and(|c| c.id == def.id));
        let note = if had { "*  You have their piece of the summit record" } else { "*  Holds a piece of the summit record" };
        d.spawn(line(note, ThemeFonts::CAPTION, ThemeColors::ACCENT_YELLOW));
        return;
    };
    let Some(step) = summit::step(n) else { return };
    d.spawn(boxed).with_children(|b| {
        b.spawn(line(format!("{}   {}", header(progress.done), step.name), ThemeFonts::CAPTION, ThemeColors::ACCENT_ORANGE));
        b.spawn(line(format!("\"{}\"", summit::pitch(n, galaxy)), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY));
        if let Some(job) = work.story().filter(|j| j.story.is_some_and(|m| m.step == n)) {
            b.spawn(line(format!("IN HAND   {}", job.title), 15.0, ThemeColors::TEXT_TITLE));
            b.spawn(line(job.brief.clone(), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY));
            b.spawn(line(format!("{}  |  pays {} cr", job.progress(), job.reward), ThemeFonts::CAPTION, ThemeColors::TEXT_SECONDARY));
            let other = if job.story.is_some_and(|m| m.forced) { "1  change your mind: the deal" } else { "2  change your mind: force" };
            b.spawn(line(other, ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED));
        } else {
            let deal = if n == 1 { format!("1  {}   ({} cr)", step.deal, crate::diplomacy::jobs::MINUTES_PRICE) } else { format!("1  {}", step.deal) };
            b.spawn(line(deal, 15.0, ThemeColors::ACCENT_GREEN));
            b.spawn(line(format!("2  {}", step.force), 15.0, ThemeColors::ACCENT_RED));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_always_has_something_to_say_and_it_is_true() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut sim = WorldSimulation::default();
        let capital = galaxy.systems.iter().find(|s| s.capital && s.faction == Some(AiShipType::TerranHegemony)).unwrap();
        // An empty world: a quiet night.
        let quiet = rumours(&sim, &galaxy, &BorderWar::default(), &FactionStanding::default(), capital.id, 1);
        assert_eq!(quiet.len(), 1);
        // A populated one: real fleets in this system get talked about.
        crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, capital.id);
        let mut heard_fleet = false;
        for seed in 0..40 {
            let said = rumours(&sim, &galaxy, &BorderWar::default(), &FactionStanding::default(), capital.id, seed);
            assert!((1..=3).contains(&said.len()));
            heard_fleet |= said.iter().any(|s| s.contains("battle group") || s.contains("convoy"));
        }
        assert!(heard_fleet, "forty nights at the Hegemony's capital and nobody mentioned a fleet");
    }

    /// The comm keys drive the story: Enter on the Abbess opens the chain,
    /// and 1 at the step's contact takes the deal (step 1: pay for the
    /// minutes), which reports the step won.
    #[test]
    fn the_comm_keys_open_the_chain_and_take_a_step() {
        use crate::resources::{Currency, DockTerms, Inventory};
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let capital = |f: AiShipType| galaxy.systems.iter().find(|s| s.capital && s.faction == Some(f)).unwrap().id;
        let (silence, princes) = (capital(AiShipType::TheSilence), capital(AiShipType::MerchantPrinces));
        let terms = |f: AiShipType| MarketEvents {
            dock_terms: Some(DockTerms { faction: Some(f), name: "x".into(), standing: "Neutral", buy: 1.0, sell: 1.0 }),
            ..Default::default()
        };

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::events::ShowNotification>()
            .add_message::<jobs::SummitStepWon>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(terms(AiShipType::TheSilence))
            .insert_resource(galaxy)
            .insert_resource(SystemStreamingManager { loaded_system: Some(silence), ..default() })
            .init_resource::<Contacts>()
            .init_resource::<Jobs>()
            .init_resource::<WorldSimulation>()
            .init_resource::<BorderWar>()
            .init_resource::<Inventory>()
            .insert_resource(Currency { credits: 5_000 })
            .add_systems(Update, dock_tab_keys);
        app.world_mut().spawn((DockingOverlay, DockTabState { tab: DockTab::Comm, tabs: true, contact: 0 }));
        let press = |app: &mut App, key: KeyCode| {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
            app.update();
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(key);
            keys.clear();
        };

        // The Abbess is first in the Silence capital's directory.
        press(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<Contacts>().summit.commissioned, "talking to the Abbess didn't open the chain");

        // Fly to the Princes; the broker holds step 1 and is listed first.
        app.insert_resource(terms(AiShipType::MerchantPrinces));
        app.world_mut().resource_mut::<SystemStreamingManager>().loaded_system = Some(princes);
        press(&mut app, KeyCode::Digit1);
        let paid = 5_000 - app.world().resource::<Currency>().credits;
        assert_eq!(paid, jobs::MINUTES_PRICE);
        let won = app.world().resource::<Messages<jobs::SummitStepWon>>();
        assert_eq!(won.len(), 1, "paying didn't report the step");

        // 2 instead takes it by force: real work, in hand, outside the three-job limit.
        press(&mut app, KeyCode::Digit2);
        let work = app.world().resource::<Jobs>();
        let story = work.story().expect("force gave no work");
        assert_eq!(story.story, Some(jobs::StoryMark { step: 1, forced: true }));
        assert_eq!(work.held(), 0);
    }
}
