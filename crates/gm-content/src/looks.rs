//! The looks of content (CONTENT.md 3): what the rows say about pictures, props, models and
//! sounds, by key. Zones load this beside the pack to name every prop key in the pack's
//! `props` list and to choose what a body holds (LOOK.md 6.1); the tool loads it to check
//! every link and build the bundle; clients never see these files, only the bundle's
//! manifest. Nothing here touches what a thing does.

use std::path::Path;

use crate::items::{self, Fit};
use crate::{AbilitiesFile, BuildsFile, ContentError, CreaturesFile};

/// `Look.held` for a hand with nothing in it (LOOK.md 6.2).
pub const NONE: u16 = u16::MAX;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AbilityLook {
    pub key: String,
    pub icon: Option<String>,
    pub prop: Option<String>,
    pub sound: Option<String>,
    /// A secondary's weapons (BODIES.md 3.6): the props whose weapon brings it as its
    /// second move (`["staff", "wand"]`), as a primary's `prop` names the weapon that
    /// swings it.
    pub secondary_of: Vec<String>,
    /// An active's weapons (BODIES.md 7.2): the props whose weapon brings it as its
    /// spell, the active after the body skill (`["staff", "wand"]`).
    pub spell_of: Vec<String>,
    /// Offered by no weapon (SKILLS.md 3).
    pub retired: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateLook {
    pub key: String,
    pub kind: String,
    pub model: Option<String>,
    pub icon: Option<String>,
    /// `right` (the default), `left`, `either` or `back` (CONTENT.md 3).
    pub held: String,
    pub fit: Fit,
    pub fit_view: Option<Fit>,
    /// An armour's overlay: the standard bones whose part of the body it covers
    /// (CONTENT.md 3.3), as a mask over the rig.
    pub covers: u32,
    /// An armour seen as a patch on the body's atlas (CONTENT.md 3.3), not an overlay.
    pub patch: bool,
    /// A shield's guard ability (GEAR.md 2), brought to whoever wears it in the left hand.
    pub guard: Option<String>,
    /// A shield's own block (GEAR.md 7), played in place of the guard ability's.
    pub block: Option<gm_core::vocab::Block>,
    /// The off hand's second move (BODIES.md 3.4): a shield's bash, the pistol, a focus's
    /// mend, brought to whoever wears it in the left hand over the weapon's own.
    pub secondary: Option<String>,
    /// An armour's base mitigation and weight (BODIES.md 3.7): armour and ward added to
    /// the body's before the caps, speed and stamina regen as factors (1 for nothing).
    pub armour: f32,
    pub ward: f32,
    pub speed: f32,
    pub regen: f32,
    pub retired: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialLook {
    pub key: String,
    pub layer: String,
    pub icon: Option<String>,
    pub tint: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreatureLook {
    pub key: String,
    pub model: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildLook {
    pub name: String,
    pub icon: Option<String>,
}

/// A row of `avatars.toml` (CONTENT.md 3.2): the official look of a frame, worn by every
/// character of that frame that has no model of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct AvatarLook {
    pub frame: gm_core::vocab::ArchetypeFrame,
    /// `models/avatars/<model>.glb`.
    pub model: String,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AvatarsFile {
    #[serde(default)]
    avatar: Vec<AvatarRow>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AvatarRow {
    frame: String,
    model: String,
}

/// Every look field of the content, by key, and the props list a zone sends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Looks {
    pub abilities: Vec<AbilityLook>,
    pub templates: Vec<TemplateLook>,
    pub materials: Vec<MaterialLook>,
    pub creatures: Vec<CreatureLook>,
    pub builds: Vec<BuildLook>,
    /// The official avatars, at most one per frame (CONTENT.md 3.2).
    pub avatars: Vec<AvatarLook>,
    /// The keys of every `ability.prop` and `template.model`, in order of first appearance
    /// (LOOK.md 6.2): a `Look.held` on the wire is an index into this list.
    pub props: Vec<String>,
}

/// Keys are lowercase ASCII letters, digits, `_` and at most one `/` (CONTENT.md 2).
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 48
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '/')
        && key.matches('/').count() <= 1
        && !key.starts_with('/')
        && !key.ends_with('/')
}

impl Looks {
    /// Read the look fields of the five tables in `dir`; the tables that do not exist are
    /// empty. Keys of look fields are checked for shape here; that their files exist is
    /// the tool's check (CONTENT.md 5.1), because zones have no bundle.
    pub fn load_dir(dir: &Path) -> Result<Looks, ContentError> {
        let read = |name: &str, optional: bool| {
            let path = dir.join(name);
            match std::fs::read_to_string(&path) {
                Ok(text) => Ok(text),
                Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
                Err(e) => Err(ContentError::Io {
                    path: path.display().to_string(),
                    source: e,
                }),
            }
        };
        Looks::load_str(
            &read("abilities.toml", false)?,
            &read("builds.toml", true)?,
            &read("creatures.toml", true)?,
            &read("items.toml", true)?,
            &read("avatars.toml", true)?,
        )
    }

    pub fn load_str(
        abilities: &str,
        builds: &str,
        creatures: &str,
        items: &str,
        avatars: &str,
    ) -> Result<Looks, ContentError> {
        fn parse<T: serde::de::DeserializeOwned>(
            text: &str,
            path: &str,
        ) -> Result<T, ContentError> {
            toml::from_str(text).map_err(|e| ContentError::Toml {
                path: path.into(),
                source: e,
            })
        }
        let af: AbilitiesFile = parse(abilities, "abilities.toml")?;
        let bf: BuildsFile = if builds.is_empty() {
            BuildsFile { build: Vec::new() }
        } else {
            parse(builds, "builds.toml")?
        };
        let cf: CreaturesFile = parse(creatures, "creatures.toml")?;
        let it: items::ItemContent = parse(items, "items.toml")?;
        let vf: AvatarsFile = if avatars.trim().is_empty() {
            AvatarsFile::default()
        } else {
            parse(avatars, "avatars.toml")?
        };
        let mut looks = Looks::default();
        let mut props: Vec<String> = Vec::new();
        let mut name_prop = |key: &Option<String>, owner: &str| -> Result<(), ContentError> {
            if let Some(k) = key {
                if !valid_key(k) {
                    return Err(ContentError::Invalid(format!(
                        "{owner}: `{k}` is not a key (lowercase letters, digits, `_`)"
                    )));
                }
                if !props.contains(k) {
                    props.push(k.clone());
                }
            }
            Ok(())
        };
        for a in &af.ability {
            name_prop(&a.prop, &format!("ability {}", a.key))?;
            for (field, value) in [("icon", &a.icon), ("sound", &a.sound)] {
                if let Some(v) = value
                    && !valid_key(v)
                {
                    return Err(ContentError::Invalid(format!(
                        "ability {}: {field} `{v}` is not a key",
                        a.key
                    )));
                }
            }
            looks.abilities.push(AbilityLook {
                key: a.key.clone(),
                icon: a.icon.clone(),
                prop: a.prop.clone(),
                secondary_of: a.secondary_of.clone(),
                spell_of: a.spell_of.clone(),
                sound: a.sound.clone(),
                retired: a.retired,
            });
        }
        let it_abilities: Vec<String> = af.ability.iter().map(|a| a.key.clone()).collect();
        for t in &it.templates {
            name_prop(&t.model, &format!("template {}", t.id))?;
            let held = t.held().to_string();
            if !["right", "left", "either", "back", "two"].contains(&held.as_str()) {
                return Err(ContentError::Invalid(format!(
                    "template {}: held must be right, left, either, back or two, not `{held}`",
                    t.id
                )));
            }
            if !t.covers.is_empty() && t.kind != "armour" {
                return Err(ContentError::Invalid(format!(
                    "template {}: only an armour covers the body",
                    t.id
                )));
            }
            if let Some(look) = &t.look
                && !["overlay", "patch"].contains(&look.as_str())
            {
                return Err(ContentError::Invalid(format!(
                    "template {}: look must be overlay or patch, not `{look}`",
                    t.id
                )));
            }
            if t.look.is_some() && t.kind != "armour" {
                return Err(ContentError::Invalid(format!(
                    "template {}: only an armour has a look on the body",
                    t.id
                )));
            }
            if t.is_patch() && !t.covers.is_empty() {
                return Err(ContentError::Invalid(format!(
                    "template {}: a patch covers no bone (the cloth is the skin)",
                    t.id
                )));
            }
            for bone in &t.covers {
                if gm_model::rig::bone_by_name(bone).is_none() {
                    return Err(ContentError::Invalid(format!(
                        "template {}: covers `{bone}`, which is no bone of the rig (MODELS.md 2)",
                        t.id
                    )));
                }
            }
            if let Some(v) = &t.icon
                && !valid_key(v)
            {
                return Err(ContentError::Invalid(format!(
                    "template {}: icon `{v}` is not a key",
                    t.id
                )));
            }
            for fit in [t.fit.as_ref(), t.fit_view.as_ref()].into_iter().flatten() {
                let finite = fit.mov.iter().chain(&fit.turn).all(|v| v.is_finite())
                    && fit.scale.is_finite()
                    && fit.scale > 0.0
                    && fit.scale <= 100.0;
                if !finite {
                    return Err(ContentError::Invalid(format!(
                        "template {}: a fit must be finite with a scale in (0, 100]",
                        t.id
                    )));
                }
            }
            for (what, key) in [("guard", &t.guard), ("secondary", &t.secondary)] {
                if let Some(key) = key
                    && !it_abilities.contains(key)
                {
                    return Err(ContentError::Invalid(format!(
                        "template {}: its {what} {key:?} is no ability",
                        t.id
                    )));
                }
            }
            looks.templates.push(TemplateLook {
                key: t.id.clone(),
                kind: t.kind.clone(),
                model: t.model.clone(),
                icon: t.icon.clone(),
                held,
                fit: t.fit.unwrap_or_default(),
                fit_view: t.fit_view,
                covers: items::covers_mask(&t.covers),
                patch: t.is_patch(),
                guard: t.guard.clone(),
                block: t.block.as_ref().map(|b| b.to_block()),
                secondary: t.secondary.clone(),
                armour: t.armour,
                ward: t.ward,
                speed: t.speed,
                regen: t.regen,
                retired: t.retired,
            });
        }
        for m in &it.materials {
            if let Some(v) = &m.icon
                && !valid_key(v)
            {
                return Err(ContentError::Invalid(format!(
                    "material {}: icon `{v}` is not a key",
                    m.id
                )));
            }
            if let Some(t) = &m.tint
                && !t.iter().all(|c| c.is_finite() && (0.0..=1.0).contains(c))
            {
                return Err(ContentError::Invalid(format!(
                    "material {}: a tint is three numbers in [0, 1]",
                    m.id
                )));
            }
            looks.materials.push(MaterialLook {
                key: m.id.clone(),
                layer: m.layer().to_string(),
                icon: m.icon.clone(),
                tint: m.tint,
            });
        }
        for c in &cf.creature {
            for (field, value) in [("model", &c.model), ("icon", &c.icon)] {
                if let Some(v) = value
                    && !valid_key(v)
                {
                    return Err(ContentError::Invalid(format!(
                        "creature {}: {field} `{v}` is not a key",
                        c.key
                    )));
                }
            }
            looks.creatures.push(CreatureLook {
                key: c.key.clone(),
                model: c.model.clone(),
                icon: c.icon.clone(),
            });
        }
        for b in &bf.build {
            if let Some(v) = &b.icon
                && !valid_key(v)
            {
                return Err(ContentError::Invalid(format!(
                    "build {}: icon `{v}` is not a key",
                    b.name
                )));
            }
            looks.builds.push(BuildLook {
                name: b.name.clone(),
                icon: b.icon.clone(),
            });
        }
        for v in &vf.avatar {
            use gm_core::vocab::ArchetypeFrame;
            let frame = match v.frame.as_str() {
                "colossus" => ArchetypeFrame::Colossus,
                "striker" => ArchetypeFrame::Striker,
                "caster" => ArchetypeFrame::Caster,
                "infiltrator" => ArchetypeFrame::Infiltrator,
                other => {
                    return Err(ContentError::Invalid(format!(
                        "avatar: frame `{other}` is not colossus, striker, caster or infiltrator"
                    )));
                }
            };
            if !valid_key(&v.model) {
                return Err(ContentError::Invalid(format!(
                    "avatar {}: model `{}` is not a key",
                    v.frame, v.model
                )));
            }
            if looks.avatars.iter().any(|a| a.frame == frame) {
                return Err(ContentError::Invalid(format!(
                    "avatar {}: two rows; a frame has one look",
                    v.frame
                )));
            }
            looks.avatars.push(AvatarLook {
                frame,
                model: v.model.clone(),
            });
        }
        if props.len() >= NONE as usize {
            return Err(ContentError::Invalid("too many props".into()));
        }
        looks.props = props;
        Ok(looks)
    }

    /// The index of a prop key in the list a zone sends; `NONE` for a key it has not.
    pub fn prop_index(&self, key: &str) -> u16 {
        self.props
            .iter()
            .position(|p| p == key)
            .map_or(NONE, |i| i as u16)
    }

    /// What a body holds (LOOK.md 6.1): the model of the weapon template it wears, else the
    /// prop of the ability in its hand (by the ability's place in the pack: the gun mode's
    /// firearm, which is no item yet), else nothing. A template or an ability the content
    /// does not know is an empty hand; a melee build with no weapon worn holds nothing.
    pub fn held(&self, worn_weapon: Option<&str>, ability: Option<usize>) -> u16 {
        if let Some(t) = worn_weapon
            && let Some(look) = self.templates.iter().find(|l| l.key == t)
            && let Some(model) = &look.model
        {
            return self.prop_index(model);
        }
        if let Some(i) = ability
            && let Some(a) = self.abilities.get(i)
            && let Some(prop) = &a.prop
        {
            return self.prop_index(prop);
        }
        NONE
    }

    /// What a body holds in its off hand (LOOK.md 6.5): the model of the item worn there
    /// (a shield, a second sword), else nothing. A guard ability puts nothing in the hand
    /// since 2026-10-13: a shield wall without a shield worn is an empty hand.
    pub fn off(&self, worn_off: Option<&str>) -> u16 {
        if let Some(t) = worn_off
            && let Some(look) = self.templates.iter().find(|l| l.key == t)
            && let Some(model) = &look.model
        {
            return self.prop_index(model);
        }
        NONE
    }

    /// What a body wears on it (LOOK.md 6.6): the model of its armour's template, which
    /// a client draws as an overlay over the body, else nothing.
    pub fn worn(&self, armour: Option<&str>) -> u16 {
        if let Some(t) = armour
            && let Some(look) = self.templates.iter().find(|l| l.key == t)
            && let Some(model) = &look.model
        {
            return self.prop_index(model);
        }
        NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABILITIES: &str = r#"
[[ability]]
key = "sword"
name = "Sword"
slot = "primary"
prop = "sword_plain"
icon = "sword"
[[ability]]
key = "hammer"
name = "Hammer"
slot = "primary"
prop = "hammer_plain"
[[ability]]
key = "firebolt"
name = "Firebolt"
slot = "secondary"
sound = "whoosh"
"#;

    const ITEMS: &str = r#"
[[template]]
id = "sword"
kind = "weapon"
strikes = "slash"
layers = ["core", "frame"]
model = "sword_iron"
fit = { move = [0.0, -0.02, 0.0], turn = [0, 90, 0], scale = 1.0 }
[[template]]
id = "hammer"
kind = "weapon"
strikes = "blunt"
layers = ["core", "frame"]
model = "hammer_plain"
held = "back"
[[template]]
id = "shield"
kind = "shield"
guards = "physical"
layers = ["core", "frame"]
model = "shield_round"
held = "left"
[[template]]
id = "cuirass"
kind = "armour"
guards = "physical"
layers = ["core", "frame"]
model = "cuirass_leather"
covers = ["spine", "chest"]
[[material]]
id = "core/iron"
edge = 30
tint = [0.8, 0.8, 0.85]
"#;

    #[test]
    fn props_are_listed_by_first_appearance_and_a_body_holds_what_it_wears_or_swings() {
        let looks = Looks::load_str(ABILITIES, "", "", ITEMS, "").unwrap();
        assert_eq!(
            looks.props,
            [
                "sword_plain",
                "hammer_plain",
                "sword_iron",
                "shield_round",
                "cuirass_leather"
            ]
        );
        // Worn: the item's model. Bare: the prop of the ability in hand (the gun mode's
        // firearm), which a zone names only then. Neither: nothing.
        assert_eq!(looks.held(Some("sword"), Some(0)), 2);
        assert_eq!(looks.held(None, Some(0)), 0);
        assert_eq!(looks.held(None, Some(1)), 1);
        assert_eq!(looks.held(None, Some(2)), NONE);
        assert_eq!(looks.held(None, None), NONE);
        assert_eq!(looks.held(Some("nothing"), None), NONE);
        assert_eq!(looks.held(None, Some(99)), NONE);
        // The off hand (LOOK.md 6.5): what is worn there, else nothing.
        assert_eq!(looks.off(Some("shield")), 3);
        assert_eq!(looks.off(Some("sword")), 2);
        assert_eq!(looks.off(None), NONE);
        assert_eq!(looks.off(Some("nothing")), NONE);
        // The body (LOOK.md 6.6): the armour's overlay, by the same list.
        assert_eq!(looks.worn(Some("cuirass")), 4);
        assert_eq!(looks.worn(Some("sword")), 2);
        assert_eq!(looks.worn(None), NONE);
        assert_eq!(looks.templates[1].held, "back");
        assert_eq!(looks.templates[3].covers, (1 << 1) | (1 << 2));
        assert_eq!(looks.templates[0].fit.turn, [0.0, 90.0, 0.0]);
        assert_eq!(looks.materials[0].tint, Some([0.8, 0.8, 0.85]));
        assert_eq!(looks.abilities[2].sound.as_deref(), Some("whoosh"));
    }

    #[test]
    fn a_key_has_a_shape() {
        assert!(valid_key("sword_iron") && valid_key("core/iron") && valid_key("m5x7"));
        assert!(!valid_key("Sword") && !valid_key("a b") && !valid_key("a/b/c") && !valid_key(""));
        let bad = ABILITIES.replace("sword_plain", "Sword Plain");
        assert!(Looks::load_str(&bad, "", "", ITEMS, "").is_err());
        let bad = ITEMS.replace("held = \"back\"", "held = \"teeth\"");
        assert!(Looks::load_str(ABILITIES, "", "", &bad, "").is_err());
        let bad = ITEMS.replace("covers = [\"spine\", \"chest\"]", "covers = [\"tail\"]");
        assert!(Looks::load_str(ABILITIES, "", "", &bad, "").is_err());
        let bad = ITEMS.replace("held = \"back\"", "held = \"back\"\ncovers = [\"chest\"]");
        assert!(Looks::load_str(ABILITIES, "", "", &bad, "").is_err());
        // An avatar row names a frame and a key, once per frame.
        let one = "[[avatar]]\nframe = \"striker\"\nmodel = \"striker_mail\"\n";
        let looks = Looks::load_str(ABILITIES, "", "", ITEMS, one).unwrap();
        assert_eq!(looks.avatars.len(), 1);
        assert_eq!(looks.avatars[0].model, "striker_mail");
        assert!(Looks::load_str(ABILITIES, "", "", ITEMS, &format!("{one}{one}")).is_err());
        assert!(
            Looks::load_str(ABILITIES, "", "", ITEMS, &one.replace("striker", "ogre")).is_err()
        );
        assert!(
            Looks::load_str(
                ABILITIES,
                "",
                "",
                ITEMS,
                &one.replace("striker\"", "giant\"")
            )
            .is_err()
        );
    }

    #[test]
    fn the_shipped_content_loads() {
        let dir =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/content"));
        let looks = Looks::load_dir(dir).unwrap();
        assert_eq!(
            looks.abilities.len(),
            crate::load_dir(dir, gm_core::tick::TickRate::COMBAT)
                .unwrap()
                .abilities
                .len()
        );
    }
}
