//! The people at a faction's starports: named contacts who know you, and
//! get to know you better the more you work for them.
//!
//! Three at each capital (military, trade, underworld) and the faction's
//! outpost master at every other station it holds; the Silence's Abbess is
//! the story's hub. Who they are and what they say is data
//! (`data/contacts.json`, drafted on the Capitals and Story Quests plan and
//! meant to be edited), compiled in. What they think of you is yours, and
//! saved.
//!
//! When a capital falls its people try to get out to the faction's next
//! capital; one in three don't make it. A contact who holds a step of the
//! story always does.

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::ai_ship::components::AiShipType;
use crate::celestial::resources::GalaxyMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContactRole {
    Hub,
    Military,
    Trade,
    Underworld,
    Outpost,
}

impl ContactRole {
    pub fn label(self) -> &'static str {
        match self {
            ContactRole::Hub => "Abbess",
            ContactRole::Military => "Military",
            ContactRole::Trade => "Trade",
            ContactRole::Underworld => "Underworld",
            ContactRole::Outpost => "Outpost",
        }
    }
}

/// Who a contact is and what they say.
#[derive(Clone, Debug, Deserialize)]
pub struct ContactDef {
    pub id: String,
    pub faction: AiShipType,
    pub role: ContactRole,
    pub name: String,
    pub greeting: String,
    pub offer: String,
    pub trusted: String,
    #[serde(default)]
    pub story: bool,
}

#[derive(Deserialize)]
struct ContactFile {
    contacts: Vec<ContactDef>,
}

/// Every contact, from `data/contacts.json`.
pub fn all() -> &'static [ContactDef] {
    static ALL: OnceLock<Vec<ContactDef>> = OnceLock::new();
    ALL.get_or_init(|| {
        let file: ContactFile = serde_json::from_str(include_str!("../../data/contacts.json"))
            .expect("data/contacts.json is malformed");
        file.contacts
    })
}

/// Relationship at which a contact trusts you.
pub const TRUSTED: f32 = 50.0;
/// Relationship gained the first time you talk to someone.
const FIRST_MEETING: f32 = 2.0;
/// Chance a capital's contact doesn't get out when it falls.
const LOST_IN_THE_FALL: f64 = 1.0 / 3.0;

/// What a contact thinks of you.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContactState {
    pub id: String,
    pub relationship: f32,
    pub met: bool,
    pub lost: bool,
    #[serde(default)]
    pub talks: u32,
}

/// Your history with every contact you've met (or lost).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contacts {
    pub known: Vec<ContactState>,
    /// How far along the Summit Record you are. It rides here, with the
    /// people who hold its pieces, so it saves and resets with them and the
    /// save system (at Bevy's parameter limit) needs nothing new.
    #[serde(default)]
    pub summit: crate::narrative::summit::SummitProgress,
}

impl Contacts {
    pub fn state(&self, id: &str) -> Option<&ContactState> {
        self.known.iter().find(|s| s.id == id)
    }

    pub fn state_mut(&mut self, id: &str) -> &mut ContactState {
        if let Some(i) = self.known.iter().position(|s| s.id == id) {
            return &mut self.known[i];
        }
        self.known.push(ContactState { id: id.to_string(), ..Default::default() });
        self.known.last_mut().unwrap()
    }

    pub fn relationship(&self, id: &str) -> f32 {
        self.state(id).map_or(0.0, |s| s.relationship)
    }

    pub fn is_lost(&self, id: &str) -> bool {
        self.state(id).is_some_and(|s| s.lost)
    }

    /// Talk to a contact: the first time you meet them, they warm to you a
    /// little. Returns the line they say now.
    pub fn talk(&mut self, def: &ContactDef) -> String {
        let s = self.state_mut(&def.id);
        if !s.met {
            s.met = true;
            s.relationship = (s.relationship + FIRST_MEETING).min(100.0);
        }
        s.talks += 1;
        line_for(def, s.relationship, s.talks).to_string()
    }
}

/// What a contact says, given what they think of you and how often you've
/// talked: their greeting the first time, then their work, and once they
/// trust you, the trusted line every other time.
pub fn line_for(def: &ContactDef, relationship: f32, talks: u32) -> &str {
    if talks <= 1 {
        &def.greeting
    } else if relationship >= TRUSTED && talks % 2 == 1 {
        &def.trusted
    } else {
        &def.offer
    }
}

/// The contacts at a faction's station in a system: its capital's people if
/// the system is that faction's capital, its outpost master otherwise.
/// Lost contacts are gone.
pub fn present_at<'a>(
    galaxy: &GalaxyMap,
    contacts: &Contacts,
    system_id: u32,
    faction: AiShipType,
) -> Vec<&'a ContactDef> {
    let Some(sys) = galaxy.systems.iter().find(|s| s.id == system_id) else { return Vec::new() };
    if sys.faction != Some(faction) {
        return Vec::new();
    }
    let at_capital = sys.capital;
    let mut here: Vec<&ContactDef> = all()
        .iter()
        .filter(|c| c.faction == faction && !contacts.is_lost(&c.id))
        .filter(|c| (c.role == ContactRole::Outpost) != at_capital)
        .collect();
    // The story's people first, then by role.
    here.sort_by_key(|c| (!c.story, c.role as u8));
    here
}

/// A faction's capital fell (to a rival, or to the dark): one in three of its
/// people don't get out. Story contacts always do. Returns the names of the
/// lost the player had met.
pub fn lose_in_the_fall(contacts: &mut Contacts, loser: AiShipType, rng: &mut impl Rng) -> Vec<String> {
    let mut lost = Vec::new();
    for def in all().iter().filter(|c| c.faction == loser && c.role != ContactRole::Outpost && !c.story) {
        if contacts.is_lost(&def.id) || !rng.gen_bool(LOST_IN_THE_FALL) {
            continue;
        }
        let met = contacts.state(&def.id).is_some_and(|s| s.met);
        contacts.state_mut(&def.id).lost = true;
        if met {
            lost.push(def.name.clone());
        }
    }
    lost
}

/// A capital fell: its people run for the faction's next capital, and some
/// don't make it. Story contacts always do.
pub fn contacts_flee_fallen_capitals(
    mut flips: MessageReader<crate::celestial::territory::SystemChangedHands>,
    galaxy: Res<GalaxyMap>,
    mut contacts: ResMut<Contacts>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut capitals: Local<Vec<(AiShipType, u32)>>,
) {
    let mut rng = rand::thread_rng();
    for flip in flips.read() {
        let Some(loser) = flip.from else { continue };
        let was_capital = capitals.iter().any(|&(f, id)| f == loser && id == flip.system_id);
        if !was_capital {
            continue;
        }
        let name = galaxy.systems.iter().find(|s| s.id == flip.system_id).map_or(String::new(), |s| s.name.clone());
        for who in lose_in_the_fall(&mut contacts, loser, &mut rng) {
            notes.write(crate::events::ShowNotification {
                message: format!("{who} did not make it out of {name}"),
                notification_type: crate::events::NotificationType::Warning,
                duration: 5.0,
            });
        }
    }
    // Where every faction rules from now, for the next fall.
    capitals.clear();
    for s in galaxy.systems.iter().filter(|s| s.capital) {
        if let Some(f) = s.faction {
            capitals.push((f, s.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_contact_parses_and_is_unique() {
        let all = all();
        assert!(all.len() >= 40, "only {} contacts", all.len());
        let mut ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "duplicate contact ids");
        for c in all {
            assert!(!c.greeting.is_empty() && !c.offer.is_empty() && !c.trusted.is_empty(), "{} is missing a line", c.id);
        }
    }

    #[test]
    fn every_faction_that_holds_ground_has_its_people() {
        for f in crate::celestial::territory::TERRITORIAL {
            let people: Vec<_> = all().iter().filter(|c| c.faction == f).collect();
            assert!(people.iter().any(|c| c.role == ContactRole::Outpost), "{f:?} has no outpost master");
            assert!(people.iter().filter(|c| c.role != ContactRole::Outpost).count() >= 3, "{f:?} has too few at its capital");
        }
        // The story's hub, and six more steps.
        assert_eq!(all().iter().filter(|c| c.story).count(), 7);
        assert!(all().iter().any(|c| c.role == ContactRole::Hub && c.faction == AiShipType::TheSilence));
    }

    #[test]
    fn a_capital_has_its_people_and_an_outpost_its_master() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let contacts = Contacts::default();
        let f = AiShipType::TerranHegemony;
        let capital = galaxy.systems.iter().find(|s| s.capital && s.faction == Some(f)).unwrap();
        let outpost = galaxy.systems.iter().find(|s| !s.capital && s.faction == Some(f)).unwrap();
        let at_capital = present_at(&galaxy, &contacts, capital.id, f);
        assert_eq!(at_capital.len(), 4, "the Hegemony's capital: Command, and three");
        assert!(at_capital[0].story, "the story's contact comes first");
        let at_outpost = present_at(&galaxy, &contacts, outpost.id, f);
        assert_eq!(at_outpost.len(), 1);
        assert_eq!(at_outpost[0].role, ContactRole::Outpost);
    }

    #[test]
    fn meeting_someone_warms_them_and_trust_unlocks_their_line() {
        let def = &all()[0];
        let mut contacts = Contacts::default();
        assert_eq!(contacts.talk(def), def.greeting);
        assert!(contacts.relationship(&def.id) > 0.0);
        assert_eq!(contacts.talk(def), def.offer);
        contacts.state_mut(&def.id).relationship = TRUSTED;
        let lines: Vec<String> = (0..2).map(|_| contacts.talk(def)).collect();
        assert!(lines.contains(&def.trusted));
    }

    #[test]
    fn what_they_think_of_you_survives_a_save() {
        let mut contacts = Contacts::default();
        contacts.talk(&all()[3]);
        contacts.state_mut(&all()[5].id).lost = true;
        let back: Contacts = serde_json::from_str(&serde_json::to_string(&contacts).unwrap()).unwrap();
        assert!(back.state(&all()[3].id).is_some_and(|s| s.met && s.relationship > 0.0));
        assert!(back.is_lost(&all()[5].id));
    }

    /// A fallen capital loses some of its people, never the story's.
    #[test]
    fn a_fallen_capital_loses_people_but_never_the_story() {
        use crate::celestial::territory::SystemChangedHands;
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let f = AiShipType::TerranHegemony;
        let capital = galaxy.systems.iter().find(|s| s.capital && s.faction == Some(f)).unwrap().id;
        let mut lost_any = false;
        for _ in 0..20 {
            let mut app = App::new();
            app.add_message::<SystemChangedHands>()
                .add_message::<crate::events::ShowNotification>()
                .insert_resource(crate::celestial::galaxy::generate_galaxy_map(42))
                .init_resource::<Contacts>()
                .add_systems(Update, contacts_flee_fallen_capitals);
            app.update(); // learns where the capitals are
            app.world_mut().write_message(SystemChangedHands { system_id: capital, from: Some(f), to: AiShipType::CorpseStars });
            app.update();
            let contacts = app.world().resource::<Contacts>();
            for def in all().iter().filter(|c| c.faction == f) {
                if def.story || def.role == ContactRole::Outpost {
                    assert!(!contacts.is_lost(&def.id), "{} was lost", def.name);
                }
                lost_any |= contacts.is_lost(&def.id);
            }
        }
        assert!(lost_any, "twenty falls and nobody was lost");
    }
}
