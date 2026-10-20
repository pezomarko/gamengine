//! What the worn things bring to a kit (GEAR.md 2): from the six worn template keys of a
//! hub's reading to the `Worn` a sheet is compiled with. The zone derives it here with
//! its looks (which ability holds which prop, which template is which model) and tells the
//! owning client the result, so a client reads no item content.

use gm_core::build::{Build, ContentPack, Slot, Worn};
use gm_core::vocab::Mode;

use crate::looks::Looks;

/// The right hand's and the left hand's index in a reading's templates
/// (`gm_hub_proto::protocol::PLACE_NAMES`: weapon, off, chest, head, legs, accessory).
const RIGHT: usize = 0;
const LEFT: usize = 1;
const CHEST: usize = 2;
const HEAD: usize = 3;
const LEGS: usize = 4;
/// What the head's and the legs' base mitigation count for beside the chest's whole
/// (BODIES.md 3.7).
const SMALL_PLACE: f32 = 1.0 / 3.0;

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
/// - **The second move** (BODIES.md 3.4 and 3.6): the left hand's own secondary where its
///   template names one (a shield's bash, the pistol, a focus's mend; the pistol in the
///   gun mode only, nothing else there); else the right hand's weapon's, among the
///   secondaries whose `secondary_of` names the weapon's model, picked as the primary is
///   (the build's own when it is one of them, else the first whose aspect the build
///   holds, else the first with no aspect); else nothing, and the build's own plays.
/// - **The spell** (BODIES.md 7.2): the active whose `spell_of` names the right hand's
///   weapon and whose element the body holds (a staff's fireball, nova, briar,
///   thunderclap, fortify or blink), the build's own when it holds one; nothing for a
///   weapon of no element or a body of none.
/// - **The techniques** (BODIES.md 3.8): every primary the right hand's weapon holds, for
///   the K page to offer.
/// - **The chest, the head and the legs** bring their base mitigation (BODIES.md 3.7),
///   the chest whole, the other two a third; their weight on the speed and the regen
///   multiplies. An armour guards by its own numbers besides, the gear term (ITEMS.md 3),
///   which the zone reads apart.
pub fn worn_of(looks: &Looks, pack: &ContentPack, build: &Build, templates: &[String]) -> Worn {
    let template = |place: usize| {
        templates
            .get(place)
            .map(String::as_str)
            .filter(|t| !t.is_empty())
            .and_then(|t| looks.templates.iter().find(|l| l.key == t))
    };
    let gun = build.mode == Mode::Fps;
    // The build's own among `holders`, else the first the build's aspect allows, else
    // the first of no aspect.
    let pick = |holders: &[u16], own: u16| -> Option<u16> {
        if holders.contains(&own) {
            return Some(own);
        }
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
    let mut worn = Worn::NONE;
    if let Some(t) = template(RIGHT)
        && let Some(model) = &t.model
    {
        let holds = |slot: Slot, names: &dyn Fn(&crate::looks::AbilityLook) -> bool| {
            let mut holders: Vec<u16> = Vec::new();
            for a in looks.abilities.iter().filter(|a| names(a)) {
                if let Some(i) = pack.find(&a.key)
                    && let Some(def) = pack.abilities.get(i as usize)
                    && def.slot == slot
                    && !def.creature
                    && (def.ability.firearm.is_some() == gun)
                {
                    holders.push(i);
                }
            }
            holders
        };
        let primaries = holds(Slot::Primary, &|a| a.prop.as_deref() == Some(model));
        worn.primary = pick(&primaries, build.primary);
        worn.techniques = primaries;
        let secondaries = holds(Slot::Secondary, &|a| a.secondary_of.contains(model));
        worn.secondary = pick(&secondaries, build.secondary);
        // The weapon's spell (BODIES.md 7.2): among the actives whose `spell_of` names
        // the weapon, the build's own when it holds one, else the first of the body's
        // element; a weapon of no element carries none.
        let spells = holds(Slot::Active, &|a| a.spell_of.contains(model));
        let own = build
            .actives
            .iter()
            .copied()
            .find(|i| spells.contains(i))
            .unwrap_or(u16::MAX);
        worn.spell = pick(&spells, own);
    }
    if let Some(t) = template(LEFT) {
        if !gun
            && let Some(key) = &t.guard
            && let Some(i) = pack.find(key)
            && pack.abilities[i as usize].slot == Slot::Guard
        {
            worn.guard = Some(i);
            worn.block = t.block.clone();
        }
        if let Some(key) = &t.secondary
            && let Some(i) = pack.find(key)
            && let Some(def) = pack.abilities.get(i as usize)
            && def.slot == Slot::Secondary
            && !def.creature
            && (def.ability.firearm.is_some() == gun)
            && def.aspect.is_none_or(|needs| build.aspects.contains(needs))
        {
            worn.secondary = Some(i);
        }
    }
    let (mut armour, mut ward, mut speed, mut regen) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
    for (place, share) in [(CHEST, 1.0), (HEAD, SMALL_PLACE), (LEGS, SMALL_PLACE)] {
        if let Some(t) = template(place) {
            armour += t.armour * share;
            ward += t.ward * share;
            speed *= 1.0 - (1.0 - t.speed) * share;
            regen *= 1.0 - (1.0 - t.regen) * share;
        }
    }
    worn.armour = armour;
    worn.ward = ward;
    worn.speed = (speed * 100.0).round().clamp(1.0, 200.0) as u8;
    worn.regen = (regen * 100.0).round().clamp(1.0, 200.0) as u8;
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
    fn the_chest_piece_brings_its_base_mitigation_and_nothing_to_the_kit() {
        // GEAR.md 12 (2026-10-17): no armour class; a chest piece brings no ability. Since
        // BODIES.md 3.7 (2026-10-19) it brings its base mitigation and its weight: the
        // chest whole, the head and the legs a third.
        let (looks, pack) = load();
        let blade = pack.build("blade").unwrap().clone();
        let worn = |chest: &str, head: &str, legs: &str| {
            let mut six = six("", "");
            six[CHEST] = chest.into();
            six[HEAD] = head.into();
            six[LEGS] = legs.into();
            worn_of(&looks, &pack, &blade, &six)
        };
        assert_eq!(worn("", "", ""), Worn::NONE);
        for (t, armour, ward, speed, regen) in [
            ("robe", 0.0, 0.15, 100, 100),
            ("cuirass", 0.08, 0.0, 100, 100),
            ("mail_shirt", 0.15, 0.0, 100, 90),
            ("plate", 0.22, -0.05, 92, 80),
        ] {
            let w = worn(t, "", "");
            assert!((w.armour - armour).abs() < 1e-6, "{t}: {}", w.armour);
            assert!((w.ward - ward).abs() < 1e-6, "{t}: {}", w.ward);
            assert_eq!((w.speed, w.regen), (speed, regen), "{t}");
            assert_eq!(
                (w.primary, w.secondary, w.guard),
                (None, None, None),
                "{t}: no ability"
            );
        }
        // A helm and trousers: a third of 0.05 each over the plate's; the hood a third of
        // its ward.
        let w = worn("plate", "helm", "trousers");
        assert!((w.armour - (0.22 + 0.1 / 3.0)).abs() < 1e-6, "{}", w.armour);
        let w = worn("robe", "hood", "");
        assert!((w.ward - (0.15 + 0.05 / 3.0)).abs() < 1e-6, "{}", w.ward);
    }

    #[test]
    fn the_weapon_brings_its_second_move_and_the_off_hand_its_own() {
        // BODIES.md 3.4 and 3.6: a staff's second move is the blast of the body's element,
        // a hammer's the slam, a sword's the pommel, a dagger's the knife; the left hand's
        // own comes over it (a shield's bash, a focus's mend); the pistol is the gun's.
        let (looks, pack) = load();
        let by = |k: &str| pack.find(k).unwrap();
        let w = |preset: &str, right: &str, left: &str| {
            worn_of(
                &looks,
                &pack,
                pack.build(preset).unwrap(),
                &six(right, left),
            )
        };
        assert_eq!(
            w("frostweaver", "staff", "").secondary,
            Some(by("chill_wave"))
        );
        assert_eq!(w("mender", "staff", "").secondary, Some(by("static")));
        assert_eq!(w("ironclad", "staff", "").secondary, Some(by("stomp")));
        assert_eq!(w("shaman", "staff", "").secondary, Some(by("briar_burst")));
        assert_eq!(w("shade", "staff", "").secondary, Some(by("gust")));
        assert_eq!(w("shade", "wand", "").secondary, Some(by("gust")));
        // The blade is Water: the nova; a body of no element gets the sweep.
        assert_eq!(w("blade", "staff", "").secondary, Some(by("chill_wave")));
        // The staff's spell (BODIES.md 7.2): the active of the body's element; the
        // frostweaver's own nova; nothing for a neutral build or a sword.
        assert_eq!(w("frostweaver", "staff", "").spell, Some(by("frost_nova")));
        assert_eq!(w("mender", "staff", "").spell, Some(by("thunderclap")));
        assert_eq!(w("ironclad", "staff", "").spell, Some(by("fortify")));
        assert_eq!(w("shaman", "wand", "").spell, Some(by("briar")));
        assert_eq!(w("shade", "staff", "").spell, Some(by("blink")));
        assert_eq!(w("blade", "sword", "").spell, None);
        let mut neutral = pack.build("blade").unwrap().clone();
        neutral.aspects = gm_core::matrix::Aspects::NONE;
        assert_eq!(
            worn_of(&looks, &pack, &neutral, &six("staff", "")).secondary,
            Some(by("staff_sweep"))
        );
        assert_eq!(w("blade", "sword", "").secondary, Some(by("pommel")));
        assert_eq!(w("blade", "katana", "").secondary, Some(by("pommel")));
        assert_eq!(w("shaman", "hammer", "").secondary, Some(by("slam")));
        assert_eq!(
            w("shade", "dagger", "").secondary,
            Some(by("throwing_knife"))
        );
        // A bow brings no second move: the build's plays.
        assert_eq!(w("blade", "bow", "").secondary, None);
        // The left hand's own over the weapon's.
        assert_eq!(
            w("blade", "sword", "shield").secondary,
            Some(by("shield_bash"))
        );
        assert_eq!(
            w("blade", "sword", "buckler").secondary,
            Some(by("shield_bash"))
        );
        assert_eq!(w("mender", "staff", "focus").secondary, Some(by("mend")));
        // The pistol in the gun mode, and nowhere else; a gun build's staff brings none.
        assert_eq!(
            w("musketeer", "musket", "pistol").secondary,
            Some(by("pistol"))
        );
        assert_eq!(w("blade", "sword", "pistol").secondary, Some(by("pommel")));
        assert_eq!(w("musketeer", "staff", "").secondary, None);
        // The techniques (BODIES.md 3.8): what the sword in hand can do, the build's
        // own stored primary among them picked.
        let sword = w("blade", "sword", "");
        assert_eq!(sword.techniques, vec![by("sword"), by("thrust")]);
        assert_eq!(sword.primary, Some(by("sword")));
        let mut thruster = pack.build("blade").unwrap().clone();
        thruster.primary = by("thrust");
        assert_eq!(
            worn_of(&looks, &pack, &thruster, &six("sword", "")).primary,
            Some(by("thrust"))
        );
        assert_eq!(
            w("blade", "bow", "").techniques,
            vec![by("bow"), by("volley")]
        );
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
