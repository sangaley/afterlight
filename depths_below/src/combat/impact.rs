use bevy::math::{IVec2, Vec2};

use crate::building::Block;
use crate::combat::ammo_types::KineticAmmoType;

// ============================================================================
// IMPACT RESOLUTION — the one place a round meets a block.
// Both damage paths (player rounds into AI hulls, AI/creature rounds into the
// player) funnel their per-block step through here, so armour geometry has a
// single home.
// ============================================================================

/// Converts a weapon's caliber scale (`caliber_scale`: gatling 0.45 …
/// railgun 1.25) into the same units as `Block::thickness` (15/30/50/80), for
/// the overmatch rule. Tuned so a railgun overmatches bare Steel and nothing
/// overmatches Titanium or better — big guns beat thin plate regardless of
/// how it's angled, which is what stops sloping being a universal answer.
const CALIBER_TO_THICKNESS: f32 = 30.0;

/// What became of the round at this block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactOutcome {
    /// Stopped in the plate.
    Absorbed,
    /// Carried on — into whatever the plate covers, or the next cell.
    Penetrated,
    /// Skipped off the surface. The plate is barely scratched and the round
    /// is still flying; the caller decides where it goes.
    Ricochet,
}

/// Impact geometry for one step of the walk: how square-on the round struck,
/// and whether it bit or skipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obliquity {
    /// cos of the angle between the round and the plate's outward normal,
    /// after normalisation. 1.0 = dead-on, 0.0 = grazing.
    pub cos_impact: f32,
    pub ricochet: bool,
}

impl Obliquity {
    /// Square-on, no deflection. For damage with no geometry behind it —
    /// splash, burn ticks, radiation, a round already inside the block.
    pub const HEAD_ON: Self = Self { cos_impact: 1.0, ricochet: false };
}

/// cos(θ_ric) per round — the round bounces when cos(impact) falls BELOW this,
/// so a smaller number means a steeper angle is needed and the round is harder
/// to deflect. A long dart barely bounces at all; a squash head never does,
/// because it isn't trying to get through the plate in the first place.
fn ricochet_cos(ammo: Option<KineticAmmoType>) -> f32 {
    use KineticAmmoType::*;
    match ammo {
        Some(APFSDS) => 0.208,                       // 78° — long rod
        Some(AP) | None => 0.342,                    // 70° — also beams/rams
        Some(APHE) | Some(HEAT) => 0.423,            // 65°
        Some(HESH) => -1.0,                          // never: it squashes
        // Gravity has no angle of incidence to skip off, and a slug held out
        // of phase never really touches the plate to skip off it. The two
        // exotic rounds that can't be shrugged off by sloping the hull.
        Some(Singularity) | Some(PhaseSlug) => -1.0, // never
        Some(PlasmaSlug) => 0.259,                   // 75° — it melts in rather than biting
        Some(Antimatter) => 0.423,                   // 65° — a blunt bottle, like APHE
        Some(HEFrag) | Some(Flak) | Some(Incendiary) | Some(EMPShell)
        | Some(NaniteCanister) | Some(NeutronShell) => 0.574, // 55°
    }
}

/// (cos Δ, sin Δ) for the round's normalisation — how far it tips TOWARD the
/// normal as it bites. Applied through the angle-subtraction identity so the
/// hot path never calls acos.
fn normalisation(ammo: Option<KineticAmmoType>) -> (f32, f32) {
    let degrees: f32 = match ammo {
        Some(KineticAmmoType::APFSDS) => 2.0,   // rigid, barely tips
        Some(KineticAmmoType::APHE) => 6.0,     // heavy, blunt
        Some(KineticAmmoType::HEAT) => 0.0,     // jet doesn't care
        _ => 4.0,
    };
    let r = degrees.to_radians();
    (r.cos(), r.sin())
}

/// Where a round actually met a block, once the block's sub-cell shape is
/// accounted for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceHit {
    /// Outward normal of the face it met.
    pub normal: Vec2,
    /// Path length through real material, in cell widths. For a full block
    /// that's the walk's own span; for a wedge it's the clipped portion.
    pub span: f32,
}

/// Clip one walk step against the block's shape, in ship-local cell space.
///
/// The walk answers "which cells, in what order". This answers "and what did
/// the round actually meet inside this one" — which the block cannot answer
/// for itself, because a wedge has three faces and which one you get depends
/// entirely on where you came from.
///
/// A full block fills its cell, so nothing changes. A wedge fills only the
/// half on the facing side of a diagonal through the cell centre, giving
/// three outcomes:
///
/// - crossed the hypotenuse    → the sloped face, the one that deflects
/// - entered through a flat leg → square-on, the cell face, no help at all
/// - stayed in the hollow half  → `None`; it never touched the block
///
/// That last case is the one a single declared normal could not express: the
/// block used to claim its diagonal from every direction, so a round crossing
/// the empty corner still hit it.
pub fn clip_to_shape(
    block: &Block,
    entry_face: IVec2,
    cell: IVec2,
    entry: Vec2,
    exit: Vec2,
) -> Option<SurfaceHit> {
    let face = Vec2::new(entry_face.x as f32, entry_face.y as f32).normalize_or_zero();
    let Some(angle) = block.facing else {
        return Some(SurfaceHit { normal: face, span: (exit - entry).length() });
    };

    // `facing` is an outward SURFACE normal: it points away from the plate into
    // open space, so the material lies BEHIND it — the half-cell with p·n <= 0.
    // Putting the material on the +n side instead means a round meets the flat
    // cell edge first with the diagonal tucked behind it, which is backwards,
    // and leaves the plate's mass hanging off the hull instead of packed
    // against it.
    let n = Vec2::from_angle(angle);
    let centre = cell.as_vec2();
    let (a, b) = (entry - centre, exit - centre);
    // Measured from the cut, which needn't run through the centre.
    let (da, db) = (a.dot(n) - block.cut, b.dot(n) - block.cut);

    if da > 0.0 && db > 0.0 {
        return None; // crossed the open corner without reaching the plate
    }

    let (start, end) = if da <= 0.0 && db <= 0.0 {
        (a, b)
    } else {
        let cross = a.lerp(b, da / (da - db));
        if da <= 0.0 { (a, cross) } else { (cross, b) }
    };

    Some(SurfaceHit {
        // Already in the material on entry => came in through a flat leg, so
        // the surface is the cell face. Otherwise it crossed the hypotenuse.
        normal: if da <= 0.0 { face } else { n },
        span: (end - start).length(),
    })
}

/// Work out how obliquely a round met a block.
///
/// `entry_face` is the walk's outward normal for the face the round came in
/// through, in ship-local cell space; `dir_local` is the round's direction in
/// that same space. Both are already ship-local, so the ship's own heading is
/// baked in — which is the whole Layer-0 payoff: turning the ship off the
/// threat axis angles every flat plate on that side, with no new data.
pub fn obliquity(
    surface: Vec2,
    dir_local: Vec2,
    block: &Block,
    ammo: Option<KineticAmmoType>,
    caliber: f32,
) -> Obliquity {
    // A zero surface means the round began this step already inside the cell
    // (it penetrated last frame). There's nothing left to skip off.
    if surface == Vec2::ZERO || dir_local.length_squared() < 1e-12 {
        return Obliquity::HEAD_ON;
    }
    let n = surface.normalize();
    let v = dir_local.normalize();

    // facing <= 0 means the round reached the BACK of this face. A slope
    // protects the side it points at and nothing else — flanking defeats it,
    // and from behind a sloped plate is just a plate.
    let facing = -v.dot(n);
    let cos_t = if facing <= 0.0 { 1.0 } else { facing };

    // Normalisation straightens the round TOWARD the normal, but never past
    // it: theta <= delta (i.e. cos_t >= cos_norm) means it's already square
    // enough, and cos(theta - delta) would overshoot to more-than-dead-on.
    let (cos_norm, sin_norm) = normalisation(ammo);
    let cos_eff = if cos_t >= cos_norm {
        1.0
    } else {
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        (cos_t * cos_norm + sin_t * sin_norm).clamp(0.0, 1.0)
    };

    // Overmatch: a round far fatter than the plate punches through instead of
    // skipping, however the plate is angled.
    let overmatch = caliber * CALIBER_TO_THICKNESS >= 2.0 * block.thickness;
    let ricochet = block.thickness > 0.0 && !overmatch && cos_eff < ricochet_cos(ammo);

    Obliquity { cos_impact: cos_eff, ricochet }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    /// Damage the struck block takes.
    pub to_block: f32,
    /// Damage that carries on past it (into what it covers, or the next
    /// cell along the walk).
    pub through: f32,
    /// Armour actually in the round's way, in penetration units:
    /// thickness × line-of-sight span × angle terms.
    pub effective_thickness: f32,
    /// What became of the round here.
    pub outcome: ImpactOutcome,
    /// How square-on it struck. 1.0 = dead-on. Drives the hit readout.
    pub cos_impact: f32,
}

/// Below this share of the armour it meets, a round does nothing at all.
const NO_BITE_BELOW: f32 = 0.3;

/// How much of a round bites into armour it may not be able to get through:
/// all of it with the penetration to spare, falling steeply to none. A slope
/// counts as thicker, by the plate along the round's path. A round with no
/// penetration at all works by blast or spall, which this doesn't touch.
pub fn bite(penetration: f32, thickness: f32, cos_impact: f32) -> f32 {
    if penetration <= 0.0 || thickness <= 0.0 {
        return 1.0;
    }
    let share = penetration * cos_impact.max(0.3) / thickness;
    ((share - NO_BITE_BELOW) / (1.0 - NO_BITE_BELOW)).clamp(0.0, 1.0)
}

/// Fraction of a bounced round's energy the plate still eats. A skipping hit
/// scrapes rather than bites, and the more square-on it was the more it leaves
/// behind — so this scales with cos, not against it.
const RICOCHET_SCRAPE: f32 = 0.15;

/// Resolve one round against one block.
///
/// `span` is the walk's line-of-sight thickness for the cell (cell widths).
/// `pass_through` is the caller's rule for how much of the round carries
/// past the block: `Some(fraction)` for the ammo-table rule the AI-hull path
/// uses (armour-exposure split), `None` to absorb by effective thickness —
/// the player-hull rule, where a plate soaks up to its material rating and
/// the remainder continues inward.
pub fn resolve_impact(
    incoming: f32,
    block: &Block,
    span: f32,
    obl: Obliquity,
    pass_through: Option<f32>,
) -> Impact {
    let effective_thickness = block.thickness * span.max(0.0);

    // A skipping round never gets to spend its energy on the plate at all.
    if obl.ricochet {
        return Impact {
            to_block: incoming * RICOCHET_SCRAPE * obl.cos_impact,
            through: 0.0,
            effective_thickness,
            outcome: ImpactOutcome::Ricochet,
            cos_impact: obl.cos_impact,
        };
    }

    let through = match pass_through {
        Some(fraction) => incoming * fraction.clamp(0.0, 1.0),
        None => (incoming - effective_thickness).max(0.0),
    };
    Impact {
        to_block: (incoming - through).max(0.0),
        through,
        effective_thickness,
        outcome: if through > 0.0 { ImpactOutcome::Penetrated } else { ImpactOutcome::Absorbed },
        cos_impact: obl.cos_impact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_rounds_stop_on_armour_that_big_ones_ignore() {
        // 20 mm, 60 mm, 170 mm, 500 mm as they fly (combat::gun_assembly::penetration_for).
        let (twenty, sixty, one_seventy, five_hundred) = (5.5, 16.5, 47.0, 137.0);
        // Steel hull: a 20 mm barely marks it, a 60 mm goes straight in.
        assert!(bite(twenty, 15.0, 1.0) < 0.15);
        assert_eq!(bite(sixty, 15.0, 1.0), 1.0);
        // A hard plate: nothing from 20 mm, little from 60 mm, all from 170 mm.
        assert_eq!(bite(twenty, 40.0, 1.0), 0.0);
        assert!(bite(sixty, 40.0, 1.0) < 0.25);
        assert_eq!(bite(one_seventy, 40.0, 1.0), 1.0);
        // The thickest hull still gives a 170 mm trouble, not a 500 mm.
        assert!(bite(one_seventy, 80.0, 1.0) < 0.5);
        assert_eq!(bite(five_hundred, 80.0, 1.0), 1.0);
        // The thin armours let small guns at them.
        assert_eq!(bite(twenty, 5.0, 1.0), 1.0);
        assert!(bite(twenty, 8.0, 1.0) > 0.5);
    }

    #[test]
    fn a_slope_counts_as_thicker_and_blast_rounds_are_left_alone() {
        assert!(bite(47.0, 40.0, 0.5) < bite(47.0, 40.0, 1.0));
        assert_eq!(bite(0.0, 80.0, 1.0), 1.0);
        assert_eq!(bite(5.5, 0.0, 1.0), 1.0);
    }

    #[test]
    fn each_armour_has_its_own_job() {
        use crate::components::{ModuleType, Rotation};
        let block = |mt| Block::for_module(IVec2::ZERO, mt, Rotation::North);
        assert!(block(ModuleType::AblativeArmor).stops_round && block(ModuleType::AngledAblativeArmor).stops_round);
        assert!(!block(ModuleType::ArmorPlate).stops_round && !block(ModuleType::ReflectivePlate).stops_round);
        let reflective = block(ModuleType::ReflectivePlate);
        assert!(reflective.laser_thickness > reflective.thickness);
        assert_eq!(reflective.laser_thickness, block(ModuleType::AngledReflectivePlate).laser_thickness);
        let plate = block(ModuleType::ArmorPlate);
        assert_eq!(plate.laser_thickness, plate.thickness);
        // Every kind has its wedge.
        assert!(block(ModuleType::AngledArmorPlate).facing.is_some());
        assert!(block(ModuleType::AngledAblativeArmor).facing.is_some());
        assert!(block(ModuleType::AngledReflectivePlate).facing.is_some());
    }
    use crate::components::HullMaterial;
    use bevy::math::IVec2;

    // Entry faces come from the walk as the OUTWARD normal of the face the
    // round came in through: entering through the -x face gives (-1, 0), so a
    // round travelling +x meets it dead-on.
    const WEST_FACE: Vec2 = Vec2::new(-1.0, 0.0);

    fn steel() -> Block { Block::hull(IVec2::ZERO, HullMaterial::Steel) }

    /// The three things that can happen inside a wedge's cell. This is what
    /// the block could not answer for itself: which face you meet depends
    /// entirely on where you came from, so the ROUND has to ask.
    #[test]
    fn a_wedge_answers_differently_depending_where_you_come_from() {
        use crate::components::{ModuleType, Rotation};
        // North-facing wedge at cell (0,0): material fills the half where
        // x + y >= 0, hypotenuse running NW to SE.
        let wedge = Block::for_module(IVec2::ZERO, ModuleType::AngledArmorPlate, Rotation::North);
        let cell = IVec2::ZERO;

        // 1. Straight through the open NE corner: never reaches the plate.
        let missed = clip_to_shape(&wedge, IVec2::new(0, 1), cell,
            Vec2::new(0.3, 0.5), Vec2::new(0.5, 0.3));
        assert_eq!(missed, None, "a round crossing the open corner must miss");

        // 2. Crossing the hypotenuse: meets the sloped face.
        let sloped = clip_to_shape(&wedge, IVec2::new(0, 1), cell,
            Vec2::new(-0.1, 0.5), Vec2::new(-0.1, -0.5)).expect("crosses into material");
        let n = Vec2::from_angle(std::f32::consts::FRAC_PI_4);
        assert!(sloped.normal.distance(n) < 1e-4, "should meet the diagonal, got {:?}", sloped.normal);
        assert!(sloped.span < 1.0, "only part of the cell is material");

        // 3. Entering already inside the material, through a flat leg: the
        //    cell face, square-on, no help from the angle at all. The legs of a
        //    NE-facing wedge are its south and west edges.
        let flat = clip_to_shape(&wedge, IVec2::new(-1, 0), cell,
            Vec2::new(-0.5, -0.2), Vec2::new(0.1, -0.2)).expect("starts in material");
        assert!(flat.normal.distance(Vec2::NEG_X) < 1e-4, "should meet the flat leg, got {:?}", flat.normal);
    }

    /// A full block is unaffected: it fills its cell, so the clip hands back
    /// the walk's own span and the face it was entered through.
    #[test]
    fn a_full_block_is_untouched_by_the_shape_clip() {
        let hit = clip_to_shape(&steel(), IVec2::new(-1, 0), IVec2::ZERO,
            Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0)).expect("full cells always hit");
        assert!(hit.normal.distance(Vec2::NEG_X) < 1e-4);
        assert!((hit.span - 1.0).abs() < 1e-4);
    }

    /// Meeting the sloped face deflects where meeting the flat leg does not —
    /// same block, same round, different approach.
    #[test]
    fn the_same_wedge_bounces_or_bites_by_approach() {
        use crate::components::{ModuleType, Rotation};
        let wedge = Block::for_module(IVec2::ZERO, ModuleType::AngledArmorPlate, Rotation::North);
        let ap = Some(KineticAmmoType::AP);
        // Face points north-east, so a round that MEETS it is travelling
        // roughly south-west. 150deg comes in across the face at 75deg.
        let incoming = Vec2::from_angle(150f32.to_radians());

        let hypotenuse = Vec2::from_angle(std::f32::consts::FRAC_PI_4);
        assert!(obliquity(hypotenuse, incoming, &wedge, ap, 0.45).ricochet,
            "a glancing hit on the hypotenuse should skip");
        // The right angle sits at the NE corner, so the legs face north and
        // east. The same round meets the east leg 30deg off square: it bites.
        assert!(!obliquity(Vec2::X, incoming, &wedge, ap, 0.45).ricochet,
            "the same round onto a flat leg should bite");
    }


    /// Dead-on is dead-on: no deflection, and no thickness inflation, because
    /// the walk's span already owns the through-cell geometry.
    #[test]
    fn head_on_hit_neither_bounces_nor_thickens() {
        let o = obliquity(WEST_FACE, Vec2::X, &steel(), Some(KineticAmmoType::AP), 1.0);
        assert!(!o.ricochet);
        assert!((o.cos_impact - 1.0).abs() < 1e-3);
    }

    /// Layer 0, the payoff that needs no new blocks: because the direction is
    /// in SHIP-LOCAL space, turning the hull off the threat axis is what makes
    /// a flat plate glance. 30 degrees bites; 75 skips.
    #[test]
    fn angling_the_hull_makes_flat_armour_glance() {
        let shallow = Vec2::from_angle(30f32.to_radians());
        assert!(!obliquity(WEST_FACE, shallow, &steel(), Some(KineticAmmoType::AP), 0.45).ricochet);

        let steep = Vec2::from_angle(75f32.to_radians());
        assert!(obliquity(WEST_FACE, steep, &steel(), Some(KineticAmmoType::AP), 0.45).ricochet);
    }

    /// Slope protects the face it points at and nothing else. The same round
    /// arriving at the plate's BACK gets no benefit — flanking beats angling.
    #[test]
    fn a_slope_does_nothing_from_behind() {
        // Plate faces west; the round comes from the east travelling west, so
        // it arrives at the plate's unprotected back.
        let sloped = Block { facing: Some(std::f32::consts::PI), ..steel() };
        let from_behind = obliquity(Vec2::X, Vec2::NEG_X, &sloped, Some(KineticAmmoType::AP), 1.0);
        assert!(!from_behind.ricochet);
        assert_eq!(from_behind.cos_impact, 1.0);
    }

    /// Ammo decides what a slope is worth. A dart barely skips, a squash head
    /// never does (it isn't trying to get through), and flak bounces off
    /// anything angled.
    #[test]
    fn rounds_differ_in_what_they_bounce_off() {
        // 78deg: past AP's 70deg threshold even after 4deg of normalisation,
        // but still inside APFSDS's 78deg one.
        let steep = Vec2::from_angle(78f32.to_radians());
        let plate = Block::hull(IVec2::ZERO, HullMaterial::Composite);
        let bounced = |ammo| obliquity(WEST_FACE, steep, &plate, Some(ammo), 0.45).ricochet;

        assert!(!bounced(KineticAmmoType::APFSDS), "a long rod should bite at 78deg");
        assert!(!bounced(KineticAmmoType::HESH), "HESH squashes, it never ricochets");
        assert!(bounced(KineticAmmoType::HEFrag), "frag should skip off angled plate");
        assert!(bounced(KineticAmmoType::AP), "AP should skip past its 70deg threshold");
    }

    /// Overmatch: a round far fatter than the plate punches through however
    /// it's angled. A railgun beats bare steel; a gatling does not.
    #[test]
    fn a_big_enough_gun_overmatches_thin_plate() {
        let steep = Vec2::from_angle(80f32.to_radians());
        let railgun = obliquity(WEST_FACE, steep, &steel(), Some(KineticAmmoType::AP), 1.25);
        let gatling = obliquity(WEST_FACE, steep, &steel(), Some(KineticAmmoType::AP), 0.45);
        assert!(!railgun.ricochet, "railgun should overmatch 15mm-equivalent steel");
        assert!(gatling.ricochet, "gatling should skip off the same plate");

        // ...but nothing overmatches the good stuff.
        let alloy = Block::hull(IVec2::ZERO, HullMaterial::AbyssalAlloy);
        assert!(obliquity(WEST_FACE, steep, &alloy, Some(KineticAmmoType::AP), 1.25).ricochet);
    }

    /// A bounced round spends almost nothing on the plate and passes nothing
    /// inward — the block behind cover stays untouched.
    #[test]
    fn a_ricochet_scratches_the_plate_and_passes_nothing_through() {
        let steep = Vec2::from_angle(80f32.to_radians());
        let o = obliquity(WEST_FACE, steep, &steel(), Some(KineticAmmoType::AP), 0.45);
        assert!(o.ricochet);
        let hit = resolve_impact(100.0, &steel(), 1.0, o, Some(0.9));
        assert_eq!(hit.outcome, ImpactOutcome::Ricochet);
        assert_eq!(hit.through, 0.0, "a bounce must not drive damage into cover");
        assert!(hit.to_block < 20.0, "a skip should scratch, not bite");
    }

    /// Modules carry no armour, so there's nothing to skip off — an exposed
    /// engine eats the round whatever the angle.
    #[test]
    fn unarmoured_blocks_never_ricochet() {
        let steep = Vec2::from_angle(85f32.to_radians());
        let o = obliquity(WEST_FACE, steep, &Block::module(IVec2::ZERO), Some(KineticAmmoType::AP), 0.45);
        assert!(!o.ricochet);
    }

    /// A round that began the step already inside the cell penetrated last
    /// frame; there's no surface left to skip off.
    #[test]
    fn no_entry_face_means_no_deflection() {
        let o = obliquity(Vec2::ZERO, Vec2::X, &steel(), Some(KineticAmmoType::AP), 0.45);
        assert_eq!(o, Obliquity::HEAD_ON);
    }

    /// Machinery armours nothing — an exposed reactor eats the round. The
    /// plating modules are the exception, which is what makes putting armour
    /// on the outside a decision rather than a formality.
    #[test]
    fn only_plating_modules_carry_armour() {
        use crate::components::{ModuleType, Rotation};
        let armoured = |mt| Block::for_module(IVec2::ZERO, mt, Rotation::North).thickness;

        assert!(armoured(ModuleType::AngledArmorPlate) > 0.0);
        assert!(armoured(ModuleType::StaggeredArmorPlate) > 0.0);
        assert!(armoured(ModuleType::ArmorPlate) > 0.0);
        assert_eq!(armoured(ModuleType::SmallReactor), 0.0);
        assert_eq!(armoured(ModuleType::Cannon), 0.0);

        // A wedge trades material for angle: less steel than a flat plate.
        assert!(armoured(ModuleType::AngledArmorPlate) < armoured(ModuleType::ArmorPlate));
    }

    /// Rotating a wedge in build mode carries its face round the four corners.
    /// `R` has always cycled this; until now it only changed the sprite.
    #[test]
    fn rotating_a_wedge_moves_its_face_to_the_next_corner() {
        use crate::components::{ModuleType, Rotation};
        use std::f32::consts::FRAC_PI_4;

        let face = |r| Block::for_module(IVec2::ZERO, ModuleType::AngledArmorPlate, r)
            .facing
            .expect("a wedge declares a face");

        // Compare directions, not raw radians: Rotation::to_radians runs
        // 0/-90/180/+90, so consecutive facings are the right corners but
        // their numeric difference wraps.
        let d = std::f32::consts::FRAC_1_SQRT_2;
        for (rotation, expected) in [
            (Rotation::North, Vec2::new(d, d)),    // north-east
            (Rotation::East, Vec2::new(d, -d)),    // south-east
            (Rotation::South, Vec2::new(-d, -d)),  // south-west
            (Rotation::West, Vec2::new(-d, d)),    // north-west
        ] {
            let got = Vec2::from_angle(face(rotation));
            assert!(got.distance(expected) < 1e-4, "{rotation:?} faced {got:?}, wanted {expected:?}");
        }
        assert!((face(Rotation::North) - FRAC_PI_4).abs() < 1e-4);

        // A plain block declares nothing and takes the cell face it was hit on.
        assert_eq!(
            Block::for_module(IVec2::ZERO, ModuleType::ArmorPlate, Rotation::North).facing,
            None
        );
    }

    #[test]
    fn plate_absorbs_up_to_its_rating_and_passes_the_rest() {
        let steel = Block::hull(IVec2::ZERO, HullMaterial::Steel); // 15
        let hit = resolve_impact(40.0, &steel, 1.0, Obliquity::HEAD_ON, None);
        assert_eq!(hit.to_block, 15.0);
        assert_eq!(hit.through, 25.0);
        assert_eq!(hit.effective_thickness, 15.0);
    }

    #[test]
    fn weak_round_is_fully_absorbed() {
        let alloy = Block::hull(IVec2::ZERO, HullMaterial::AbyssalAlloy); // 80
        let hit = resolve_impact(30.0, &alloy, 1.0, Obliquity::HEAD_ON, None);
        assert_eq!(hit.to_block, 30.0);
        assert_eq!(hit.through, 0.0);
    }

    #[test]
    fn diagonal_span_thickens_the_plate() {
        let steel = Block::hull(IVec2::ZERO, HullMaterial::Steel);
        let hit = resolve_impact(40.0, &steel, 2f32.sqrt(), Obliquity::HEAD_ON, None);
        assert!((hit.effective_thickness - 21.213).abs() < 1e-2);
        assert!((hit.to_block - 21.213).abs() < 1e-2);
    }

    #[test]
    fn ammo_rule_splits_by_fraction_regardless_of_thickness() {
        let alloy = Block::hull(IVec2::ZERO, HullMaterial::AbyssalAlloy);
        let hit = resolve_impact(40.0, &alloy, 1.0, Obliquity::HEAD_ON, Some(0.3));
        assert!((hit.to_block - 28.0).abs() < 1e-4);
        assert!((hit.through - 12.0).abs() < 1e-4);
        let exposed = resolve_impact(40.0, &Block::module(IVec2::ZERO), 1.0, Obliquity::HEAD_ON, Some(0.0));
        assert_eq!(exposed.to_block, 40.0);
    }

    #[test]
    fn unarmoured_module_absorbs_nothing_by_thickness() {
        let hit = resolve_impact(40.0, &Block::module(IVec2::ZERO), 1.0, Obliquity::HEAD_ON, None);
        assert_eq!(hit.to_block, 0.0);
        assert_eq!(hit.through, 40.0);
    }
}
