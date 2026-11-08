//! Faction paint, the way Cosmoteer does it: a colour scheme on the hull.
//!
//! A faction's ships are painted, not flooded: the plating -- the outer hull
//! and the armour plates -- takes a base colour and an accent in a pattern
//! of the faction's own (bands, chevrons, a painted bow, a split), while the
//! rooms, guns and machinery inside keep their own art. So a ship says whose
//! it is at any zoom without losing what it is. The base is the faction's
//! galaxy-map colour, so a colour means the same faction everywhere.
//!
//! Paint multiplies over the plate's own texture and material tint, so it
//! reads as painted steel rather than a flat fill.

use bevy::prelude::*;

use super::components::AiShipType;
use crate::components::{BaseSpriteColor, ModuleType};

/// Where a faction puts its accent colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// Bands across the hull, every few blocks along its length.
    Bands,
    /// Forward-pointing chevrons.
    Chevrons,
    /// The bow painted in the accent.
    Bow,
    /// Port side one colour, starboard the other.
    Split,
    /// A checker of two-block squares.
    Checker,
    /// A stripe down each flank, along the length.
    Stripe,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Livery {
    pub base: Color,
    pub accent: Color,
    pub pattern: Pattern,
}

pub fn livery(faction: AiShipType) -> Livery {
    use AiShipType::*;
    let base = super::components::faction_map_color(faction);
    let (accent, pattern) = match faction {
        TerranHegemony => (Color::srgb(0.93, 0.95, 0.98), Pattern::Bands),
        EternalHegemony => (Color::srgb(0.95, 0.78, 0.3), Pattern::Chevrons),
        GildedThrone => (Color::srgb(0.95, 0.8, 0.35), Pattern::Bow),
        BrokenChoir => (Color::srgb(0.85, 0.9, 0.85), Pattern::Checker),
        SynthesisCollective => (Color::srgb(0.3, 0.9, 0.8), Pattern::Stripe),
        CorpseStars => (Color::srgb(0.9, 0.86, 0.75), Pattern::Chevrons),
        StellarPreserve => (Color::srgb(0.95, 0.97, 0.95), Pattern::Split),
        TheSilence => (Color::srgb(0.25, 0.27, 0.3), Pattern::Bands),
        Shepherd => (Color::srgb(0.5, 0.33, 0.2), Pattern::Bow),
        RecursiveKingdom => (Color::srgb(0.18, 0.16, 0.15), Pattern::Chevrons),
        MerchantPrinces => (Color::srgb(0.93, 0.9, 0.75), Pattern::Bands),
        FreeHaulersCompact => (Color::srgb(0.95, 0.82, 0.2), Pattern::Stripe),
    };
    Livery { base, accent, pattern }
}

impl Livery {
    /// Whether this cell of a ship spanning `lo..=hi` (bow to +x) takes the
    /// accent.
    pub fn accent_at(&self, cell: IVec2, lo: IVec2, hi: IVec2) -> bool {
        let from_bow = hi.x - cell.x;
        let mid_y = (lo.y + hi.y) as f32 * 0.5;
        let off = cell.y as f32 - mid_y;
        match self.pattern {
            Pattern::Bands => cell.x.rem_euclid(6) < 2,
            Pattern::Chevrons => (cell.x + (off.abs() as i32)).rem_euclid(7) < 2,
            Pattern::Bow => from_bow < ((hi.x - lo.x + 1) / 5).max(2),
            Pattern::Split => off > 0.0,
            Pattern::Checker => (cell.x.div_euclid(2) + cell.y.div_euclid(2)).rem_euclid(2) == 0,
            Pattern::Stripe => {
                let half = (hi.y - lo.y) as f32 * 0.5;
                let band = off.abs() / half.max(1.0);
                (0.45..0.7).contains(&band)
            }
        }
    }

    /// The paint for this cell over a plate tinted `material`.
    pub fn paint(&self, material: Color, accent: bool) -> Color {
        // The material's own lightness carries through, so titanium paints
        // lighter than steel and the plate texture still shows.
        let m = material.to_srgba();
        let light = ((m.red + m.green + m.blue) / 3.0 * 1.45).clamp(0.5, 1.0);
        let c = if accent { self.accent } else { self.base }.to_srgba();
        Color::srgb(c.red * light, c.green * light, c.blue * light)
    }
}

/// Plating a faction paints: the armour plate blocks (the hull's outer layer
/// is painted where it's spawned).
pub fn is_plating(t: ModuleType) -> bool {
    matches!(
        t,
        ModuleType::ArmorPlate
            | ModuleType::AngledArmorPlate
            | ModuleType::BevelledArmorPlate
            | ModuleType::SlopeArmorPlateLow
            | ModuleType::SlopeArmorPlateHigh
            | ModuleType::SlopeArmorPlateLowMirror
            | ModuleType::SlopeArmorPlateHighMirror
            | ModuleType::HalfArmorPlate
            | ModuleType::CornerArmorPlate
            | ModuleType::StaggeredArmorPlate
            | ModuleType::HullReinforcePlate
            | ModuleType::AngledHullPlate
            | ModuleType::AblativeArmor
            | ModuleType::AngledAblativeArmor
            | ModuleType::ReflectivePlate
            | ModuleType::AngledReflectivePlate
    )
}

/// Paint to put on a block once its sprite exists.
#[derive(Component, Clone, Copy)]
pub struct Paint(pub Color);

/// Puts freshly spawned plating in its faction's paint.
pub fn apply_paint(mut commands: Commands, mut painted: Query<(Entity, &Paint, &mut Sprite, Option<&mut BaseSpriteColor>), Added<Paint>>) {
    for (e, paint, mut sprite, base) in painted.iter_mut() {
        // A wedge draws in child quads and leaves its own sprite clear.
        if sprite.color.alpha() > 0.0 {
            sprite.color = paint.0;
        }
        match base {
            Some(mut b) => b.0 = paint.0,
            None => {
                commands.entity(e).try_insert(BaseSpriteColor(paint.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_faction_has_its_own_scheme() {
        use AiShipType::*;
        let all = [
            TerranHegemony, EternalHegemony, GildedThrone, BrokenChoir, SynthesisCollective, CorpseStars,
            StellarPreserve, TheSilence, Shepherd, RecursiveKingdom, MerchantPrinces, FreeHaulersCompact,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                let (la, lb) = (livery(*a), livery(*b));
                assert!(la.base != lb.base || la.accent != lb.accent || la.pattern != lb.pattern, "{a:?} and {b:?} look the same");
            }
        }
    }

    #[test]
    fn bands_paint_some_cells_and_not_others() {
        let l = livery(AiShipType::TerranHegemony);
        let (lo, hi) = (IVec2::new(-10, -5), IVec2::new(10, 5));
        let painted = (-10..=10).filter(|x| l.accent_at(IVec2::new(*x, 5), lo, hi)).count();
        assert!(painted > 3 && painted < 18, "{painted}");
    }

    #[test]
    fn paint_keeps_the_material_lightness() {
        let l = livery(AiShipType::TerranHegemony);
        let steel = l.paint(Color::srgb(0.55, 0.55, 0.6), false).to_srgba();
        let titanium = l.paint(Color::srgb(0.7, 0.7, 0.75), false).to_srgba();
        assert!(titanium.red > steel.red);
    }
}
