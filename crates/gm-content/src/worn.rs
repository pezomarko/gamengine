//! What the worn things bring to a kit (GEAR.md 2): from the six worn template keys of a
//! hub's reading to the `Worn` a sheet is compiled with. The zone derives it here with
//! its looks (which ability holds which prop, which template is which model) and tells the
//! owning client the result, so a client reads no item content.

use gm_core::build::{Build, ContentPack, Slot, Worn};
use gm_core::vocab::Mode;

use crate::looks::Looks;

/// The right hand's and the left hand's index in a reading's templates
/// (`gm_hub_proto::protocol::PLACE_NAMES`: weapon, off, ...).
const RIGHT: usize = 0;
const LEFT: usize = 1;
#[cfg(test)]
const CHEST: usize = 2;

/// The `Worn` of `build` in `templates` (the hub's six, by place; empty for nothing).
///
/// - **The right hand** (GEAR.md 2): the primary whose prop is the weapon's model. Of
///   several (the staff's blow and its bolts, the musket and the carbine) the build's own
///   primary when it is one of them, else the first whose aspect the build holds, else
///   the first with no aspect. A weapon the mode does not hold (a firearm out of the gun
///   mode, anything else in it: the hub refuses the wear, GEAR.md 5) brings nothing, and
///   so does an empty hand or a weapon no ability holds: the build's primary plays.
/// - **The left hand**: the template's `guard` ability, when it is one and the mode has a
///   guard (a gun build has none, MODES.md 2). A second one-hander brings nothing.
///   A shield's own `block` (GEAR.md 7) comes with its guard, played in its place.
/// - **The chest** brings nothing here (GEAR.md 12, 2026-10-17): an armour guards by
///   its own numbers, the gear term (ITEMS.md 3), which the zone reads apart.
pub fn worn_of(looks: &Looks, pack: &ContentPack, build: &Build, templates: &[String]) -> Worn {
    let template = |place: usize| {
        templates
            .get(place)
            .map(String::as_str)
            .filter(|t| !t.is_empty())
            .and_then(|t| looks.templates.iter().find(|l| l.key == t))
    };
    let mut worn = Worn::NONE;
    if let Some(t) = template(RIGHT)
        && let Some(model) = &t.model
    {
        let mut holders: Vec<u16> = Vec::new();
        for a in looks
            .abilities
            .iter()
            .filter(|a| a.prop.as_deref() == Some(model))
        {
            if let Some(i) = pack.find(&a.key)
                && let Some(def) = pack.abilities.get(i as usize)
                && def.slot == Slot::Primary
                && !def.creature
                && (def.ability.firearm.is_some() == (build.mode == Mode::Gun))
            {
                holders.push(i);
            }
        }
        worn.primary = if holders.contains(&build.primary) {
            Some(build.primary)
        } else {
            holders
                .iter()
                .copied()
                .find(|&i| {
                    pack.abilities[i as usize]
                        .aspect
                        .is_some_and(|needs| build.aspects.contains(needs))
                })
                .or_else(|| {
                    holders
                        .iter()
                        .copied()
                        .find(|&i| pack.abilities[i as usize].aspect.is_none())
                })
        };
    }
    if build.mode != Mode::Gun
        && let Some(t) = template(LEFT)
        && let Some(key) = &t.guard
        && let Some(i) = pack.find(key)
        && pack.abilities[i as usize].slot == Slot::Guard
    {
        worn.guard = Some(i);
        worn.block = t.block.clone();
    }
    worn
}

#[cfg(test)]
mod tests {
    use super::*;
    use gm_core::tick::TickRate;

    fn load() -> (Looks, ContentPack) {
        let dir =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/content"));
        let looks = Looks::load_dir(dir).unwrap();
        let pack = crate::load_dir(dir, TickRate::COMBAT).unwrap();
        (looks, pack)
    }

    fn six(right: &str, left: &str) -> Vec<String> {
        vec![
            right.into(),
            left.into(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]
    }

    #[test]
    fn the_weapon_brings_its_primary_and_the_shield_its_wall() {
        let (looks, pack) = load();
        let blade = pack.build("blade").unwrap().clone();
        let by = |k: &str| pack.find(k).unwrap();
        // The blade in nothing: the build's kit.
        assert_eq!(worn_of(&looks, &pack, &blade, &six("", "")), Worn::NONE);
        // Its own sword: the same primary, said.
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("sword", "")).primary,
            Some(by("sword"))
        );
        // A mace, a bow, a warhammer: each brings its swing (the eleven orphans of the
        // catalog, worn by anyone who buys one).
        for w in ["mace", "bow", "warhammer", "katana", "spear"] {
            assert_eq!(
                worn_of(&looks, &pack, &blade, &six(w, "")).primary,
                Some(by(w)),
                "{w}"
            );
        }
        // A shield brings the wall; a second sword brings no guard.
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("sword", "shield")).guard,
            Some(by("shield_wall"))
        );
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("sword", "buckler")).guard,
            Some(by("shield_wall"))
        );
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("sword", "focus")).guard,
            Some(by("brace"))
        );
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("sword", "sword")).guard,
            None
        );
    }

    #[test]
    fn a_staff_keeps_the_caster_s_bolt_and_the_mode_keeps_its_hands() {
        let (looks, pack) = load();
        let by = |k: &str| pack.find(k).unwrap();
        // The frostweaver's staff is its ice shard; the mender's its staff's blow; a blade
        // with a staff (Water) gets the first bolt its aspect allows: the ice shard.
        let frost = pack.build("frostweaver").unwrap().clone();
        assert_eq!(
            worn_of(&looks, &pack, &frost, &six("staff", "")).primary,
            Some(by("ice_shard"))
        );
        let mender = pack.build("mender").unwrap().clone();
        assert_eq!(
            worn_of(&looks, &pack, &mender, &six("staff", "")).primary,
            Some(by("staff"))
        );
        let blade = pack.build("blade").unwrap().clone();
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("staff", "")).primary,
            Some(by("ice_shard"))
        );
        // The shade (Air) gets the shadow dart, the ironclad (Ground) the stone; the
        // musketeer, of no aspect, would get the plain blow were a staff its mode's.
        let shade = pack.build("shade").unwrap().clone();
        assert_eq!(
            worn_of(&looks, &pack, &shade, &six("staff", "")).primary,
            Some(by("shadow_dart"))
        );
        let ironclad = pack.build("ironclad").unwrap().clone();
        assert_eq!(
            worn_of(&looks, &pack, &ironclad, &six("staff", "")).primary,
            Some(by("stone_throw"))
        );
        // A gun build: its musket is its musket, a sword in its hand brings nothing, and a
        // shield no guard; a blade with a musket brings nothing either.
        let gun = pack.build("musketeer").unwrap().clone();
        assert_eq!(
            worn_of(&looks, &pack, &gun, &six("musket", "")).primary,
            Some(by("musket"))
        );
        assert_eq!(
            worn_of(&looks, &pack, &gun, &six("sword", "shield")),
            Worn::NONE
        );
        assert_eq!(
            worn_of(&looks, &pack, &blade, &six("musket", "")).primary,
            None
        );
    }

    #[test]
    fn the_chest_piece_brings_nothing_to_the_kit() {
        // GEAR.md 12 (2026-10-17): no armour class. A chest piece is its gear term and
        // its look; the sheet's kit and numbers are the same in any of them.
        let (looks, pack) = load();
        let blade = pack.build("blade").unwrap().clone();
        for t in ["", "robe", "cuirass", "mail_shirt", "plate"] {
            let mut six = six("", "");
            six[CHEST] = t.into();
            assert_eq!(worn_of(&looks, &pack, &blade, &six), Worn::NONE, "{t}");
        }
    }

    #[test]
    fn the_layers_do_their_jobs() {
        use crate::items::Jobs;
        use gm_core::matrix::{Element, OnHit};
        use gm_core::vocab::Status;
        let dir =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/content"));
        let items = crate::items::load_items(dir).unwrap();
        // The best sword: an ember catalyst burns one blow in five, two opals make its
        // burns last twice as long (150% twice, under the ceiling), whalebone prices its
        // blows at three quarters.
        let best = [
            "core/dragonbone",
            "catalyst/ember",
            "frame/whalebone",
            "gem/opal",
            "gem/opal",
        ];
        let sword = items.jobs("sword", best).unwrap();
        assert_eq!(
            sword.on_hit,
            Some(OnHit {
                status: Status::Burn,
                chance: 20,
                duration_ms: 2000,
                magnitude_pm: 3000,
            })
        );
        assert_eq!(sword.status_dealt[Element::Fire.slot()], 200);
        assert_eq!(sword.status_dealt[Element::Water.slot()], 100);
        assert_eq!(sword.status_taken, [100; 6]);
        assert_eq!(sword.stamina, 75);
        // A plain sword of iron and oak does nothing but its edge.
        assert_eq!(
            items.jobs("sword", ["core/iron", "frame/oak"]),
            Some(Jobs::NONE)
        );
        // A cuirass has no catalyst, so its opals ward nothing; a garnet wards fire; ash
        // prices its blocks at nine tenths.
        let cuirass = items
            .jobs(
                "cuirass",
                ["core/iron", "frame/ash", "gem/opal", "gem/garnet"],
            )
            .unwrap();
        assert_eq!(cuirass.on_hit, None);
        assert_eq!(cuirass.status_taken[Element::Fire.slot()], 50);
        assert_eq!(cuirass.status_taken[Element::Water.slot()], 100);
        assert_eq!(cuirass.stamina, 90);
        // A robe on rime with an opal: chills last 60% of 50%, 30%, on its wearer.
        let robe = items
            .jobs(
                "robe",
                ["core/iron", "catalyst/rime", "frame/oak", "gem/opal"],
            )
            .unwrap();
        assert_eq!(robe.status_taken[Element::Water.slot()], 30);
        // The words say each, after the edge.
        let view = items.view(
            "sword",
            best.iter().map(|m| (m.split_once('/').unwrap().0, *m)),
        );
        assert!(
            view.does
                .contains(&"burns one blow in five for 2.0 s".to_string()),
            "{:?}",
            view.does
        );
        assert!(
            view.does
                .contains(&"its burns last 200% as long".to_string())
        );
        assert!(
            view.does
                .contains(&"blows cost 75% of their stamina".to_string())
        );
        let view = items.view(
            "robe",
            [
                ("core", "core/iron"),
                ("catalyst", "catalyst/rime"),
                ("frame", "frame/oak"),
                ("gem", "gem/opal"),
            ],
        );
        assert!(
            view.does
                .contains(&"chills last 30% as long on you".to_string()),
            "{:?}",
            view.does
        );
    }
}
