//! Each faction's way of fighting, in one place.
//!
//! A faction's doctrine is what makes meeting it feel like meeting *them*:
//! the range it holds, what it shoots out first, how hard a fleet piles onto
//! one target, how true its gunners aim, when its launchers let go, what kind
//! of prey it looks for, how far its calls for help carry, and what breaks it.
//! These used to be scattered (fighting bands in movement.rs, aim lists in
//! combat.rs, temperaments in awareness.rs); they live together here so each
//! faction reads as one decision.
//!
//! The counters are part of the doctrine too, on purpose: the navy turns
//! slowly, the empire scatters when its command ship dies, the mercenaries
//! leave when it gets expensive, the brawlers have no reach.

use super::awareness::Temperament;
use super::components::AiShipType;
use crate::components::ModuleCategory;

/// What a faction looks for in a target, over the plain sum of firepower for
/// damage taken.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hunt {
    /// Extra worth for a hull already broken: 1.0 makes a half-wrecked ship
    /// worth half again. The Preserve prunes the sick; the Corpse Stars
    /// finish what is dying.
    pub wounded: f32,
    /// Leaning toward big hulls (positive) or small ones (negative). The
    /// empire goes for capitals; swarms and the Shepherd pick off the small.
    pub size: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Doctrine {
    pub temper: Temperament,
    /// The distances it likes to fight at (from its target's hull), or None
    /// for a faction that flies straight in.
    pub band: Option<(f32, f32)>,
    /// Which enemy subsystems it shoots out, in order; empty is centre of mass.
    pub aim: &'static [ModuleCategory],
    /// How hard a fleet piles onto its called target, over the shared dial.
    pub focus: f32,
    /// How widely its built guns scatter, over the gun's own spread. The
    /// Hegemony's sensors "see thirty seconds ahead".
    pub spread: f32,
    /// Seconds between the salvos its launchers hold for (0 fires as ready),
    /// and where in that beat its salvo falls, so two factions never share one.
    pub salvo_beat: f32,
    pub salvo_phase: f32,
    pub hunt: Hunt,
    /// How far its calls for help carry, over the usual distress radius.
    pub rally: f32,
    /// Fire rate gained as it breaks: rate × (1 + fury × hull lost). The
    /// Corpse Stars grow stronger as they break.
    pub fury: f32,
}

pub fn doctrine(faction: AiShipType) -> Doctrine {
    use AiShipType::*;
    use ModuleCategory::*;
    let temper = |withdraw, cunning, jink, kites| Temperament { withdraw, cunning, jink, kites, leader_bound: false };
    let base = Doctrine {
        temper: temper(1.0, 0.5, 0.6, false),
        band: Some((12_000.0, 18_000.0)),
        aim: &[],
        focus: 1.0,
        spread: 1.0,
        salvo_beat: 8.0,
        salvo_phase: 0.0,
        hunt: Hunt::default(),
        rally: 1.0,
        fury: 0.0,
    };
    match faction {
        // The line: a navy holding long range in a battle line, every gun on
        // one target, armour-piercers in one timed salvo, the best gunnery
        // there is. Disciplined, slow to give ground, slow to turn.
        TerranHegemony => Doctrine {
            temper: temper(0.3, 0.8, 0.4, true),
            band: Some((18_000.0, 22_000.0)),
            aim: &[Weapons, Power],
            focus: 1.6,
            spread: 0.55,
            salvo_beat: 10.0,
            ..base
        },
        // The throne: every fleet fights round its command ship and never
        // gives ground while it stands; when it falls, the fleet scatters.
        // Methodical: guns, then the reactor, and the biggest prey first.
        EternalHegemony => Doctrine {
            temper: Temperament { leader_bound: true, ..temper(0.0, 0.6, 0.2, true) },
            band: Some((14_000.0, 20_000.0)),
            aim: &[Weapons, Power],
            focus: 1.3,
            spread: 0.85,
            salvo_beat: 8.0,
            salvo_phase: 2.0,
            hunt: Hunt { wounded: 0.0, size: 0.5 },
            ..base
        },
        // Hired precision: lasers from long range, round the flank, your guns
        // then your drive. Not paid to die: they leave when it gets costly.
        GildedThrone => Doctrine {
            temper: temper(0.65, 1.0, 1.0, true),
            band: Some((18_000.0, 22_000.0)),
            aim: &[Weapons, Propulsion],
            spread: 0.75,
            salvo_beat: 0.0,
            rally: 0.8,
            ..base
        },
        // Business: escorts screen while the traders run, and shoot your
        // engines so you stop chasing. Nobody here fights to the death.
        MerchantPrinces => Doctrine {
            temper: temper(1.0, 0.5, 0.6, false),
            aim: &[Propulsion],
            focus: 0.8,
            salvo_beat: 0.0,
            ..base
        },
        // Brawl: cheap crews who close hard, in groups that answer each
        // other's calls from a long way off. No reach to speak of.
        FreeHaulersCompact => Doctrine {
            temper: temper(0.6, 0.3, 0.6, false),
            band: Some((3_000.0, 6_000.0)),
            spread: 1.2,
            salvo_beat: 0.0,
            rally: 2.0,
            ..base
        },
        // The swarm: small hulls rushing in from several sides at once, each
        // on its own prey, at the power first, close enough for plasma.
        SynthesisCollective => Doctrine {
            temper: temper(0.0, 0.6, 0.5, false),
            band: Some((4_000.0, 8_000.0)),
            aim: &[Power],
            focus: 0.6,
            salvo_beat: 0.0,
            hunt: Hunt { wounded: 0.0, size: -0.4 },
            rally: 1.5,
            ..base
        },
        // Catch and keep: they don't trade blows. They stop a ship (drive,
        // then power) and prefer the ones already failing.
        StellarPreserve => Doctrine {
            temper: temper(1.0, 0.5, 0.6, false),
            band: Some((6_000.0, 10_000.0)),
            aim: &[Propulsion, Power],
            salvo_beat: 0.0,
            hunt: Hunt { wounded: 1.5, size: 0.0 },
            ..base
        },
        // The scream: ghosts that come straight on and never flee, missiles
        // in waves.
        BrokenChoir => Doctrine {
            temper: temper(0.0, 0.0, 0.3, false),
            band: Some((5_000.0, 9_000.0)),
            salvo_beat: 6.0,
            salvo_phase: 3.0,
            rally: 1.5,
            ..base
        },
        // Entropy: drive and reactor first, then the bone prow. They grow
        // stronger as they break, and only a gutted ship turns away.
        CorpseStars => Doctrine {
            temper: temper(0.15, 0.5, 0.4, false),
            band: Some((3_000.0, 7_000.0)),
            aim: &[Propulsion, Power],
            salvo_beat: 8.0,
            salvo_phase: 5.0,
            hunt: Hunt { wounded: 0.5, size: 0.0 },
            fury: 0.8,
            ..base
        },
        // The hook: a lone apex hunter, power then guns, at close range, and
        // it picks the one it can carry off.
        Shepherd => Doctrine {
            temper: temper(0.0, 0.8, 0.1, false),
            band: Some((4_000.0, 8_000.0)),
            aim: &[Power, Weapons],
            salvo_beat: 0.0,
            hunt: Hunt { wounded: 0.5, size: -0.3 },
            ..base
        },
        // Numbers: copies that fly straight in, each on whatever is nearest,
        // and let every rocket go at once.
        RecursiveKingdom => Doctrine {
            temper: temper(0.0, 0.3, 0.7, false),
            band: None,
            focus: 0.6,
            spread: 1.3,
            salvo_beat: 14.0,
            salvo_phase: 7.0,
            rally: 1.5,
            ..base
        },
        // They never start a fight; they watch, and they leave.
        TheSilence => base,
    }
}

/// How much more a hull is worth to a faction for what its doctrine hunts.
pub fn hunt_bias(hunt: &Hunt, blocks_left: f32, blocks: u32) -> f32 {
    let wounded = 1.0 + hunt.wounded * (1.0 - blocks_left.clamp(0.0, 1.0));
    // A 1,500-block hull is the even point; ten times bigger or smaller is
    // worth up to about twice (or half) as much at full lean.
    let size = ((blocks.max(50) as f32) / 1_500.0).powf(hunt.size * 0.3).clamp(0.25, 4.0);
    wounded * size
}

/// The fire-rate scale of a ship this broken.
pub fn fury_scale(fury: f32, blocks_left: f32) -> f32 {
    1.0 + fury * (1.0 - blocks_left.clamp(0.0, 1.0))
}

/// Should a launcher on this doctrine let go now? Launchers across a fleet
/// wait for the same beat, so a salvo arrives together and swamps point
/// defence; each faction's beat falls at its own moment. `scale` is the
/// shared dial (Mind::salvo_beat / 8).
pub fn salvo_open(now: f32, d: &Doctrine, scale: f32) -> bool {
    let beat = d.salvo_beat * scale;
    beat <= 0.0 || ((now + d.salvo_phase * scale) / beat).fract() * beat < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [AiShipType; 12] = [
        AiShipType::TerranHegemony, AiShipType::EternalHegemony, AiShipType::GildedThrone, AiShipType::MerchantPrinces,
        AiShipType::FreeHaulersCompact, AiShipType::SynthesisCollective, AiShipType::StellarPreserve, AiShipType::BrokenChoir,
        AiShipType::CorpseStars, AiShipType::TheSilence, AiShipType::Shepherd, AiShipType::RecursiveKingdom,
    ];

    #[test]
    fn every_band_is_a_band() {
        for f in ALL {
            if let Some((lo, hi)) = doctrine(f).band {
                assert!(lo > 0.0 && hi > lo, "{f:?} band {lo}..{hi}");
            }
        }
        assert!(doctrine(AiShipType::RecursiveKingdom).band.is_none(), "the Kingdom rams");
    }

    #[test]
    fn the_navy_aims_best_and_the_copies_worst() {
        let best = ALL.iter().map(|f| doctrine(*f).spread).fold(f32::MAX, f32::min);
        let worst = ALL.iter().map(|f| doctrine(*f).spread).fold(0.0, f32::max);
        assert_eq!(doctrine(AiShipType::TerranHegemony).spread, best);
        assert_eq!(doctrine(AiShipType::RecursiveKingdom).spread, worst);
    }

    #[test]
    fn brawlers_close_in_and_snipers_stand_off() {
        let close = |f| doctrine(f).band.unwrap().1;
        let far = |f| doctrine(f).band.unwrap().0;
        for brawler in [AiShipType::FreeHaulersCompact, AiShipType::CorpseStars, AiShipType::SynthesisCollective, AiShipType::Shepherd] {
            assert!(close(brawler) < far(AiShipType::TerranHegemony), "{brawler:?}");
        }
    }

    #[test]
    fn salvos_of_two_factions_fall_apart() {
        let terran = doctrine(AiShipType::TerranHegemony);
        let choir = doctrine(AiShipType::BrokenChoir);
        // Over a minute, the moments both have a salvo open are few.
        let mut both = 0;
        let mut either = 0;
        for i in 0..600 {
            let t = i as f32 * 0.1;
            let (a, b) = (salvo_open(t, &terran, 1.0), salvo_open(t, &choir, 1.0));
            both += (a && b) as u32;
            either += (a || b) as u32;
        }
        assert!(either > 0);
        assert!(both * 4 < either, "{both} of {either}");
        // Firing as ready is always open.
        assert!(salvo_open(3.3, &doctrine(AiShipType::GildedThrone), 1.0));
    }

    #[test]
    fn the_preserve_prefers_the_failing_and_the_swarm_the_small() {
        let preserve = doctrine(AiShipType::StellarPreserve).hunt;
        assert!(hunt_bias(&preserve, 0.3, 1_500) > hunt_bias(&preserve, 1.0, 1_500) * 1.8);
        let swarm = doctrine(AiShipType::SynthesisCollective).hunt;
        assert!(hunt_bias(&swarm, 1.0, 400) > hunt_bias(&swarm, 1.0, 6_000));
        let empire = doctrine(AiShipType::EternalHegemony).hunt;
        assert!(hunt_bias(&empire, 1.0, 6_000) > hunt_bias(&empire, 1.0, 400));
        // No lean is no change.
        assert_eq!(hunt_bias(&Hunt::default(), 0.2, 9_000), 1.0);
    }

    #[test]
    fn the_corpse_stars_grow_stronger_as_they_break() {
        let fury = doctrine(AiShipType::CorpseStars).fury;
        assert_eq!(fury_scale(fury, 1.0), 1.0);
        assert!(fury_scale(fury, 0.3) > 1.5);
        assert_eq!(fury_scale(doctrine(AiShipType::TerranHegemony).fury, 0.3), 1.0);
    }

    #[test]
    fn only_the_empire_is_bound_to_its_throne() {
        for f in ALL {
            assert_eq!(doctrine(f).temper.leader_bound, f == AiShipType::EternalHegemony, "{f:?}");
        }
    }
}
