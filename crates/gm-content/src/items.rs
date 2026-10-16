//! Item content (ECONOMY.md 4): templates and materials from `items.toml`, with the gear
//! edge cap (PLAN.md 0) checked at load, and what a crafted item does to damage
//! (ITEMS.md 3.2).

use std::path::Path;

use gm_core::matrix::{Element, Gear, OnHit};
use gm_core::vocab::{DamageType, Status};
use serde::Deserialize;

use crate::ContentError;

/// The layers of a craft (ECONOMY.md 4). The shard went on 2026-10-16 (GEAR.md 6).
pub const LAYERS: [&str; 4] = ["core", "catalyst", "frame", "gem"];
/// Gem sockets per item.
pub const MAX_GEMS: u32 = 2;
/// The largest total edge of a fully crafted item over a standard one, per mille.
pub const MAX_EDGE_PER_MILLE: u32 = 250;
/// GEAR.md 6: how long an element's status lasts on the wearer of an armour, a shield or a
/// cape whose catalyst is that element, percent.
pub const CATALYST_WARD: u8 = 60;
/// How long the warded element's status lasts on the wearer of a piece with that gem.
pub const GEM_WARD: u8 = 50;
/// How long the warded element's status, put on by a blow of a weapon with that gem, lasts.
pub const GEM_EDGE: u8 = 150;
/// The floor of what wards stack to on one body, percent.
pub const WARD_FLOOR: u8 = 25;
/// The ceiling of what a weapon's gems stack to, percent.
pub const EDGE_CEILING: u8 = 200;

/// What a craft's layers do besides its edge (GEAR.md 6; `ItemContent::jobs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jobs {
    /// A weapon's catalyst: its element's status on a landing blow.
    pub on_hit: Option<OnHit>,
    /// Per element (`Element::ALL`'s order): how long that element's status lasts on the
    /// wearer, percent; 100 for nothing. A guard piece's catalyst and gems.
    pub status_taken: [u8; 6],
    /// Per element: how long that element's status, put on by this weapon's blows, lasts
    /// on whoever they land on, percent. A weapon's gems.
    pub status_dealt: [u8; 6],
    /// The frame: the stamina of the blows (a weapon) or of the blocks (a guard piece),
    /// percent of the cost.
    pub stamina: u8,
}

impl Jobs {
    pub const NONE: Jobs = Jobs {
        on_hit: None,
        status_taken: [100; 6],
        status_dealt: [100; 6],
        stamina: 100,
    };

    /// A ward on `e` of `pct` percent, multiplied into what is there, under the floor.
    pub fn ward(&mut self, e: Element, pct: u8) {
        let i = e.slot();
        let v = self.status_taken[i] as u32 * pct as u32 / 100;
        self.status_taken[i] = v.max(WARD_FLOOR as u32) as u8;
    }

    /// A weapon's gem on `e` of `pct` percent, multiplied into what is there, under the
    /// ceiling.
    pub fn edge(&mut self, e: Element, pct: u8) {
        let i = e.slot();
        let v = self.status_dealt[i] as u32 * pct as u32 / 100;
        self.status_dealt[i] = v.min(EDGE_CEILING as u32) as u8;
    }
}

/// The element of a damage type, for the six that are one.
fn dtype_element(d: DamageType) -> Option<Element> {
    d.element()
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemTemplate {
    pub id: String,
    pub kind: String,
    pub layers: Vec<String>,
    /// A weapon: the physical kind it strikes with (`slash`, `pierce`, `blunt`). Its core's
    /// edge sharpens that kind and no other (ITEMS.md 3.2).
    #[serde(default)]
    pub strikes: Option<String>,
    /// An armour: what its core guards against, `physical` (the three kinds of blow) or
    /// `elements` (the five).
    #[serde(default)]
    pub guards: Option<String>,
    /// An armour's place on the body (EQUIPMENT.md 10.8): `chest` (the default), `head`,
    /// `legs`; a trinket's: `accessory`.
    #[serde(default)]
    pub place: Option<String>,
    /// A shield's guard (GEAR.md 2): the key of the guard ability the thing in the left
    /// hand brings to whoever wears it (`shield_wall`, `brace`), in place of the build's.
    #[serde(default)]
    pub guard: Option<String>,
    /// A chest piece's armour class (GEAR.md 3, MATRIX.md 4): `cloth`, `leather`, `mail`
    /// or `plate`, what the body wearing it is in; a chest piece that names none is cloth.
    #[serde(default)]
    pub class: Option<String>,
    /// A stack (MODES.md 11.1): the most of it one holder carries (the strict limit), and
    /// what one of it heals when it is a kit. No layers, no materials.
    #[serde(default)]
    pub cap: Option<u32>,
    #[serde(default)]
    pub heals: Option<i32>,
    /// The look fields (CONTENT.md 3): the prop drawn in the hand of whoever wears an item
    /// of this template, its picture (baked from the model when absent), which hand, how
    /// the file is fitted into the hand and into the first-person view, and whether the
    /// template is retired (nothing new is made of it; what exists keeps its row).
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// Which hand (CONTENT.md 3): `right` (the default), `left`, `either` (a one-handed
    /// weapon worn in the right hand, or in the left beside another), `two` (both hands,
    /// EQUIPMENT.md 3: worn, it takes the left hand too), `back`.
    #[serde(default)]
    pub held: Option<String>,
    #[serde(default)]
    pub fit: Option<Fit>,
    #[serde(default)]
    pub fit_view: Option<Fit>,
    /// An armour's overlay (CONTENT.md 3.3): the standard bones (MODELS.md 2) whose part
    /// of the body it covers, hidden under it when it is worn.
    #[serde(default)]
    pub covers: Vec<String>,
    /// How an armour is seen on the body (CONTENT.md 3.3): `overlay` (the default), a
    /// skinned piece drawn over the body with its own texture, or `patch`, cloth painted
    /// into the official avatar's own atlas (LOOK.md 6.6), which covers no bone.
    #[serde(default)]
    pub look: Option<String>,
    #[serde(default)]
    pub retired: bool,
}

impl ItemTemplate {
    /// Whether an item of this template sharpens what its wearer deals (a weapon, a ring)
    /// or dulls what it takes (an armour, a shield, a cape): ITEMS.md 3.2, EQUIPMENT.md
    /// 10.8. A trinket that `guards` is for what its wearer takes; one that names no
    /// guard is for what it deals.
    pub fn deals(&self) -> bool {
        self.kind == "weapon" || (self.kind == "trinket" && self.guards.is_none())
    }

    pub fn held(&self) -> &str {
        self.held.as_deref().unwrap_or("right")
    }

    /// An armour whose look is a patch on the body's atlas (CONTENT.md 3.3).
    pub fn is_patch(&self) -> bool {
        self.look.as_deref() == Some("patch")
    }

    /// Worn, it takes both hands (EQUIPMENT.md 3).
    pub fn two_handed(&self) -> bool {
        self.held() == "two"
    }
}

/// How a prop's file is moved into hand space (CONTENT.md 3.1): metres, degrees about
/// glTF's X, Y and Z in that order, a scale; scaled first, then turned, then moved.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    #[serde(default, rename = "move")]
    pub mov: [f32; 3],
    #[serde(default)]
    pub turn: [f32; 3],
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

impl Default for Fit {
    fn default() -> Fit {
        Fit {
            mov: [0.0; 3],
            turn: [0.0; 3],
            scale: 1.0,
        }
    }
}

impl Eq for ItemTemplate {}

const PHYSICAL: [DamageType; 3] = [DamageType::Slash, DamageType::Pierce, DamageType::Blunt];
const ELEMENTS: [DamageType; 6] = [
    DamageType::Fire,
    DamageType::Water,
    DamageType::Grass,
    DamageType::Electric,
    DamageType::Ground,
    DamageType::Air,
];
/// The nine kinds of damage as a person reads them, in `DamageType`'s order.
const KINDS: [&str; 9] = [
    "slash", "pierce", "blunt", "fire", "water", "grass", "electric", "ground", "air",
];

/// An item as a person is shown it (ITEMS.md 6): everything a screen says about what it
/// is and does, so that no client works any of it out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemView {
    /// Where it is worn; `None` for a part, and for what this content does not know.
    pub place: Option<Place>,
    /// It may be worn in the off hand as well (ITEMS.md 2: a one-handed weapon `held =
    /// "either"`), beside its `place`.
    pub off: bool,
    /// Its edge per damage type, per mille (zeros for what is not worn).
    pub edge: [u16; 9],
    /// What it is: `a weapon, 220 of 250`, `a core, for crafting`.
    pub what: String,
    /// What it does, the strongest first: `slash +11.0%`, `fire +9.5%`.
    pub does: Vec<String>,
}

/// An edge in per mille as what it does (ITEMS.md 3.1): a weapon deals that much more of
/// the kind, an armour takes what `SCALE / (SCALE + edge)` leaves off it; in tenths of a
/// per cent, rounded.
fn per_cent(deals: bool, edge: u16) -> String {
    let e = edge as u32;
    match deals {
        true => {
            let more = (e * 1000 + Gear::SCALE / 2) / Gear::SCALE;
            format!("+{}.{}%", more / 10, more % 10)
        }
        false => {
            let off = (e * 1000 + (Gear::SCALE + e) / 2) / (Gear::SCALE + e);
            format!("-{}.{}%", off / 10, off % 10)
        }
    }
}

/// The edges in words, the strongest first. Kinds of one group that share an edge are
/// said together (`physical`, `elements`, or `other elements` beside one that stands out).
/// What the layers' jobs say (GEAR.md 6), after the edge: the catalyst's status on the
/// blows, the gems' wards, the frame's stamina.
fn job_words(deals: bool, jobs: &Jobs) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(oh) = jobs.on_hit {
        let verb = match oh.status {
            Status::Burn => "burns",
            Status::Chill => "chills",
            Status::Shock => "shocks",
            Status::Weaken => "weakens",
            Status::Slow => "slows",
            Status::Expose => "exposes",
            _ => "marks",
        };
        const COUNT: [&str; 11] = [
            "", "", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ];
        let one_in = match oh.chance {
            100 => "every blow".to_string(),
            c if c > 0 && 100 % c == 0 && 100 / c <= 10 => {
                format!("one blow in {}", COUNT[(100 / c) as usize])
            }
            c => format!("{c}% of blows"),
        };
        out.push(format!(
            "{verb} {one_in} for {:.1} s",
            oh.duration_ms as f32 / 1000.0
        ));
    }
    let table = if deals {
        &jobs.status_dealt
    } else {
        &jobs.status_taken
    };
    for (e, pct) in Element::ALL.into_iter().zip(table) {
        if *pct != 100 {
            let noun = match e {
                Element::Fire => "burns",
                Element::Water => "chills",
                Element::Electric => "shocks",
                Element::Grass => "weakenings",
                Element::Ground => "slows",
                Element::Air => "exposures",
            };
            out.push(if deals {
                format!("its {noun} last {pct}% as long")
            } else {
                format!("{noun} last {pct}% as long on you")
            });
        }
    }
    if jobs.stamina != 100 {
        out.push(format!(
            "{} cost {}% of their stamina",
            if deals { "blows" } else { "blocks" },
            jobs.stamina
        ));
    }
    out
}

fn words(deals: bool, edge: &[u16; 9]) -> Vec<String> {
    let mut said: Vec<(u16, String)> = Vec::new();
    for (group, name) in [(&PHYSICAL[..], "physical"), (&ELEMENTS[..], "elements")] {
        let of = |v: u16| group.iter().filter(|k| edge[**k as usize] == v).count();
        // What most of the group shares, if two or more share anything.
        let shared = group
            .iter()
            .map(|k| edge[*k as usize])
            .filter(|v| *v > 0 && of(*v) >= 2)
            .max_by_key(|v| (of(*v), *v));
        let mut alone = false;
        for k in group {
            let v = edge[*k as usize];
            if v > 0 && Some(v) != shared {
                said.push((v, KINDS[*k as usize].to_string()));
                alone = true;
            }
        }
        match shared {
            Some(v) if of(v) == group.len() => said.push((v, name.to_string())),
            // All but one, and that one said by its own name: these are the others.
            Some(v) if of(v) == group.len() - 1 && alone => said.push((v, format!("other {name}"))),
            // Some of the group, and no word for which: each by its own name.
            Some(v) => {
                for k in group.iter().filter(|k| edge[**k as usize] == v) {
                    said.push((v, KINDS[*k as usize].to_string()));
                }
            }
            None => {}
        }
    }
    said.sort_by_key(|(v, _)| std::cmp::Reverse(*v));
    said.into_iter()
        .map(|(v, what)| format!("{what} {}", per_cent(deals, v)))
        .collect()
}

/// Where an item is worn (ITEMS.md 2, EQUIPMENT.md 10.8, 2026-10-14): the two hands, the
/// body's three places (the chest, `Armour`, named as it was when it was the whole body;
/// the head; the legs) and one accessory (a ring or a cape: numbers, nothing drawn).
/// Twelve places between the evening of 2026-10-13 and this: the director simplified it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Weapon,
    Off,
    Armour,
    Head,
    Legs,
    Accessory,
}

/// How many places a character has (`Place::ALL`).
pub const PLACES: usize = 6;

impl Place {
    /// Every place, in the order of a reading's `templates` and the hub's list.
    pub const ALL: [Place; PLACES] = [
        Place::Weapon,
        Place::Off,
        Place::Armour,
        Place::Head,
        Place::Legs,
        Place::Accessory,
    ];
    /// The body's places (EQUIPMENT.md 10.8): where an overlay is drawn, in the order
    /// `Look.worn` lists them on the wire.
    pub const BODY: [Place; 3] = [Place::Armour, Place::Head, Place::Legs];

    pub fn name(self) -> &'static str {
        match self {
            Place::Weapon => "weapon",
            Place::Off => "off",
            Place::Armour => "armour",
            Place::Head => "head",
            Place::Legs => "legs",
            Place::Accessory => "accessory",
        }
    }

    pub fn parse(name: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| p.name() == name)
    }

    /// The place's index in a reading's `templates` (ITEMS.md 3.3).
    pub fn index(self) -> usize {
        self as usize
    }

    /// The place in a person's words: where a thing does not go.
    pub fn in_words(self) -> &'static str {
        match self {
            Place::Weapon => "the right hand",
            Place::Off => "the left hand",
            Place::Armour => "the body",
            Place::Head => "the head",
            Place::Legs => "the legs",
            Place::Accessory => "the accessory",
        }
    }

    /// What the place counts for of its item's edge (EQUIPMENT.md 5.5 and 10.8), as a
    /// fraction: the right hand whole, the left hand half, the body by its parts (the
    /// chest 12 of 20, the legs 5, the head 3: a full set reaches the cap), the accessory
    /// half. A two-hander in the right hand counts 1.5 (3), which is the reading's to
    /// say, not the place's.
    pub fn share(self) -> (u32, u32) {
        match self {
            Place::Weapon => (1, 1),
            Place::Off => (1, 2),
            Place::Armour => (12, 20),
            Place::Legs => (5, 20),
            Place::Head => (3, 20),
            Place::Accessory => (1, 2),
        }
    }

    /// An armour's `place` word (EQUIPMENT.md 10.8); nothing said is the chest.
    pub fn of_armour(word: Option<&str>) -> Option<Place> {
        match word.unwrap_or("chest") {
            "chest" => Some(Place::Armour),
            "head" => Some(Place::Head),
            "legs" => Some(Place::Legs),
            _ => None,
        }
    }

    /// A trinket's `place` word (EQUIPMENT.md 10.8): `accessory`, the one there is.
    pub fn of_trinket(word: Option<&str>) -> Option<Place> {
        match word? {
            "accessory" => Some(Place::Accessory),
            _ => None,
        }
    }
}

fn physical(kind: &str) -> Option<DamageType> {
    match kind {
        "slash" => Some(DamageType::Slash),
        "pierce" => Some(DamageType::Pierce),
        "blunt" => Some(DamageType::Blunt),
        _ => None,
    }
}

/// A list of standard bone names (MODELS.md 2, matched as the ingest matches joints) as a
/// bit mask over the rig; a name the rig does not know is skipped.
pub fn covers_mask(names: &[String]) -> u32 {
    names
        .iter()
        .filter_map(|n| gm_model::rig::bone_by_name(n))
        .fold(0, |m, b| m | (1 << b))
}

fn element(name: &str) -> Option<DamageType> {
    match name {
        "fire" => Some(DamageType::Fire),
        "water" => Some(DamageType::Water),
        "grass" => Some(DamageType::Grass),
        "electric" => Some(DamageType::Electric),
        "ground" => Some(DamageType::Ground),
        "air" => Some(DamageType::Air),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    /// `layer/name`.
    pub id: String,
    /// Per mille of the item's base stats.
    pub edge: u32,
    #[serde(default)]
    pub element: Option<String>,
    #[serde(default)]
    pub note: String,
    /// A catalyst's job (GEAR.md 6): on a weapon, the status its element puts on a landing
    /// blow (`burn`, `chill`, `shock`, `weaken`, `slow`, `expose`: the element's own,
    /// `Status::element`), at `chance` percent, for `duration_ms`, at `magnitude`; on an
    /// armour, a shield or a cape, that status lasts `CATALYST_WARD` percent as long on the
    /// wearer.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub chance: Option<u8>,
    #[serde(default)]
    pub duration_ms: Option<u16>,
    #[serde(default)]
    pub magnitude: Option<f32>,
    /// A frame's job (GEAR.md 6): the stamina of the blows (a weapon) or of the blocks (a
    /// shield, an armour), in percent of the cost; absent: 100.
    #[serde(default)]
    pub stamina: Option<u8>,
    /// A gem's job (GEAR.md 6): the element it wards, by name, or `catalyst` for the item's
    /// own catalyst's; on an armour, a shield or a cape that element's status lasts
    /// `GEM_WARD` percent as long on the wearer, on a weapon `GEM_EDGE` percent as long on
    /// whoever the blow lands on.
    #[serde(default)]
    pub ward: Option<String>,
    /// The look fields (CONTENT.md 3): a picture for the crafting screens, and the tint a
    /// worn item's texture takes when this is its core.
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub tint: Option<[f32; 3]>,
}

impl Eq for Material {}

impl Material {
    pub fn layer(&self) -> &str {
        self.id.split('/').next().unwrap_or("")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemContent {
    #[serde(default, rename = "template")]
    pub templates: Vec<ItemTemplate>,
    #[serde(default, rename = "material")]
    pub materials: Vec<Material>,
}

impl ItemContent {
    /// The best edge a craft of this template can reach, per mille.
    pub fn best_edge(&self, template: &ItemTemplate) -> u32 {
        template
            .layers
            .iter()
            .map(|layer| {
                let best = self
                    .materials
                    .iter()
                    .filter(|m| m.layer() == layer)
                    .map(|m| m.edge)
                    .max()
                    .unwrap_or(0);
                if layer == "gem" {
                    best * MAX_GEMS
                } else {
                    best
                }
            })
            .sum()
    }

    pub fn template_ids(&self) -> Vec<String> {
        self.templates.iter().map(|t| t.id.clone()).collect()
    }

    /// Where an item of this template is worn; `None` for a component, and for a template
    /// this content does not know.
    pub fn template(&self, id: &str) -> Option<&ItemTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    pub fn place(&self, template: &str) -> Option<Place> {
        let t = self.templates.iter().find(|t| t.id == template)?;
        Self::default_place(t)
    }

    /// Where an item of this template goes when nobody says (ITEMS.md 2, EQUIPMENT.md 5):
    /// a weapon into the right hand (the left when it is `held = "left"`), a shield into
    /// the left, an armour onto its place of the body, a trinket onto the accessory's
    /// place; a stack or a part nowhere.
    fn default_place(t: &ItemTemplate) -> Option<Place> {
        match t.kind.as_str() {
            "weapon" if t.held() == "left" => Some(Place::Off),
            "weapon" => Some(Place::Weapon),
            "shield" => Some(Place::Off),
            "armour" => Place::of_armour(t.place.as_deref()),
            "trinket" => Place::of_trinket(t.place.as_deref()),
            _ => None,
        }
    }

    /// Whether an item of `template` may be worn in `place` (ITEMS.md 2): a weapon in the
    /// hand its `held` names, and in either when it says so; a shield in the off hand; an
    /// armour on its place of the body; a trinket on the accessory's place. `false` for a
    /// template this content does not know.
    pub fn may_wear_in(&self, template: &str, place: Place) -> bool {
        let Some(t) = self.templates.iter().find(|t| t.id == template) else {
            return false;
        };
        match (t.kind.as_str(), place) {
            ("weapon", Place::Weapon) => t.held() != "left",
            ("weapon", Place::Off) => matches!(t.held(), "left" | "either"),
            ("shield", Place::Off) => true,
            ("armour" | "trinket", p) => Self::default_place(t) == Some(p),
            _ => false,
        }
    }

    /// Whether an item of `template` takes both hands when worn (EQUIPMENT.md 3); false
    /// for a template this content does not know.
    pub fn two_handed(&self, template: &str) -> bool {
        self.templates
            .iter()
            .find(|t| t.id == template)
            .is_some_and(|t| t.two_handed())
    }

    /// The template's `covers` (CONTENT.md 3.3) as a mask of standard bones; unknown
    /// names are refused by the look loader, so here they count for nothing.
    pub fn covers(&self, template: &str) -> u32 {
        self.templates
            .iter()
            .find(|t| t.id == template)
            .map_or(0, |t| covers_mask(&t.covers))
    }

    /// The prop an item of this template puts in the hand (CONTENT.md 3): what a build's
    /// abilities must hold for the item to be worn as a weapon (ITEMS.md 2). `None` for a
    /// template without a model, or one this content does not know.
    pub fn model(&self, template: &str) -> Option<&str> {
        self.templates
            .iter()
            .find(|t| t.id == template)?
            .model
            .as_deref()
    }

    /// A stack template (MODES.md 11.1): its cap and what one of it heals; `None` for
    /// anything else.
    pub fn stack(&self, template: &str) -> Option<(u32, Option<i32>)> {
        let t = self.templates.iter().find(|t| t.id == template)?;
        (t.kind == "stack").then(|| (t.cap.unwrap_or(1).max(1), t.heals))
    }

    /// What a stack of `quantity` is, in words: `balls ×12 of 30`.
    pub fn stack_words(&self, template: &str, quantity: u32) -> String {
        match self.stack(template) {
            Some((cap, _)) => format!("{template} ×{quantity} of {cap}"),
            None => template.to_string(),
        }
    }

    /// What an item of `template` made of `materials` does (ITEMS.md 3.2): where it is
    /// worn, its edge per damage type in `DamageType`'s order (per mille: a weapon's is
    /// what its wearer deals more of, an armour's what its wearer takes less of), and its
    /// whole edge. Each layer puts its material's edge into some of the eight:
    ///
    /// - the core into what the item is made for: the kind a weapon strikes with, the
    ///   three kinds of blow or the five elements an armour guards against;
    /// - the catalyst into its element;
    /// - shard, frame and gems into every kind the core and the catalyst reach, and no
    ///   other: an item is for one build and not for another.
    ///
    /// A material this content does not know, or of a layer the template has no room for,
    /// adds nothing.
    pub fn edges<'a>(
        &self,
        template: &str,
        materials: impl IntoIterator<Item = &'a str>,
    ) -> Option<(Place, [u16; 9], u16)> {
        let t = self.templates.iter().find(|t| t.id == template)?;
        let place = Self::default_place(t)?;
        let known: Vec<&Material> = materials
            .into_iter()
            .filter_map(|id| self.materials.iter().find(|m| m.id == id))
            .filter(|m| t.layers.iter().any(|l| l == m.layer()))
            .collect();
        // What the item is for. A trinket (EQUIPMENT.md 5.4) has no core: it is for the
        // elements, and its catalyst says which one it reaches most.
        let mut made_for = [false; 9];
        if t.kind == "trinket" {
            for kind in &ELEMENTS {
                made_for[*kind as usize] = true;
            }
        } else if t.deals() {
            if let Some(kind) = t.strikes.as_deref().and_then(physical) {
                made_for[kind as usize] = true;
            }
        } else {
            let kinds = match t.guards.as_deref() {
                Some("elements") => &ELEMENTS[..],
                _ => &PHYSICAL[..],
            };
            for kind in kinds {
                made_for[*kind as usize] = true;
            }
        }
        let core = made_for;
        for m in &known {
            if m.layer() == "catalyst"
                && let Some(e) = m.element.as_deref().and_then(element)
            {
                made_for[e as usize] = true;
            }
        }
        let mut out = [0u32; 9];
        let mut whole = 0u32;
        for m in &known {
            whole += m.edge;
            let mut into = [false; 9];
            match m.layer() {
                "core" => into = core,
                "catalyst" => {
                    if let Some(e) = m.element.as_deref().and_then(element) {
                        into[e as usize] = true;
                    }
                }
                _ => into = made_for,
            }
            for (kind, on) in into.iter().enumerate() {
                if *on {
                    out[kind] += m.edge;
                }
            }
        }
        Some((
            place,
            out.map(|e| e.min(MAX_EDGE_PER_MILLE) as u16),
            whole.min(u16::MAX as u32) as u16,
        ))
    }

    /// What the layers of a craft do besides its edge (GEAR.md 6): the catalyst's status,
    /// the gems' wards, the frame's stamina, as the hub folds them into a body's gear and as
    /// the words say them. `None`: a template the content does not know, or a stack.
    pub fn jobs<'a>(
        &self,
        template: &str,
        materials: impl IntoIterator<Item = &'a str>,
    ) -> Option<Jobs> {
        let t = self.templates.iter().find(|t| t.id == template)?;
        Self::default_place(t)?;
        let known: Vec<&Material> = materials
            .into_iter()
            .filter_map(|id| self.materials.iter().find(|m| m.id == id))
            .filter(|m| t.layers.iter().any(|l| l == m.layer()))
            .collect();
        let deals = t.deals();
        let mut jobs = Jobs::NONE;
        let catalyst = known
            .iter()
            .find(|m| m.layer() == "catalyst")
            .and_then(|m| m.element.as_deref())
            .and_then(element)
            .and_then(dtype_element);
        for m in &known {
            match m.layer() {
                "catalyst" => {
                    let Some(e) = catalyst else { continue };
                    if deals {
                        if let (Some(status), Some(chance), Some(duration_ms), Some(magnitude)) = (
                            m.status.as_deref().and_then(Status::parse),
                            m.chance,
                            m.duration_ms,
                            m.magnitude,
                        ) {
                            jobs.on_hit = Some(OnHit {
                                status,
                                chance,
                                duration_ms,
                                magnitude_pm: (magnitude * 1000.0).round().clamp(0.0, 65535.0)
                                    as u16,
                            });
                        }
                    } else {
                        jobs.ward(e, CATALYST_WARD);
                    }
                }
                "gem" => {
                    let e = match m.ward.as_deref() {
                        Some("catalyst") => catalyst,
                        Some(name) => element(name).and_then(dtype_element),
                        None => None,
                    };
                    let Some(e) = e else { continue };
                    if deals {
                        jobs.edge(e, GEM_EDGE);
                    } else {
                        jobs.ward(e, GEM_WARD);
                    }
                }
                "frame" => {
                    if let Some(pct) = m.stamina {
                        jobs.stamina = jobs.stamina.min(pct);
                    }
                }
                _ => {}
            }
        }
        Some(jobs)
    }

    /// The item as a person is shown it. `components` are `(layer, material)`.
    pub fn view<'a>(
        &self,
        template: &str,
        components: impl IntoIterator<Item = (&'a str, &'a str)> + Clone,
    ) -> ItemView {
        let parts: Vec<(&str, &str)> = components.into_iter().collect();
        let materials = parts.iter().map(|(_, m)| *m);
        let jobs = self.jobs(template, parts.iter().map(|(_, m)| *m));
        match self.edges(template, materials) {
            Some((place, edge, whole)) => {
                let deals = self.template(template).is_some_and(|t| t.deals());
                let mut does = words(deals, &edge);
                if let Some(jobs) = jobs {
                    does.extend(job_words(deals, &jobs));
                }
                let t = self.templates.iter().find(|t| t.id == template);
                let kind = t.map_or("weapon", |t| t.kind.as_str());
                ItemView {
                    place: Some(place),
                    off: place != Place::Off && self.may_wear_in(template, Place::Off),
                    edge,
                    what: format!(
                        "{} {kind}, {whole} of {MAX_EDGE_PER_MILLE}",
                        if kind == "armour" { "an" } else { "a" },
                    ),
                    does,
                }
            }
            None => match self.stack(template) {
                Some((_, heals)) => ItemView {
                    what: format!("{template}, a stack"),
                    does: heals
                        .into_iter()
                        .map(|h| format!("heals {h}, used with F"))
                        .collect(),
                    ..ItemView::default()
                },
                None => ItemView {
                    what: match parts.first().copied() {
                        Some((layer, _)) if template == "component" => {
                            format!("a {layer}, for crafting")
                        }
                        _ => "nothing this world knows".to_string(),
                    },
                    ..ItemView::default()
                },
            },
        }
    }

    /// The layers a craft of this template may fill; `None`: the content does not know it.
    pub fn layers(&self, template: &str) -> Option<&[String]> {
        self.templates
            .iter()
            .find(|t| t.id == template)
            .map(|t| &t.layers[..])
    }

    fn validate(&self) -> Result<(), ContentError> {
        let bad = |s: String| Err(ContentError::Invalid(s));
        for (i, m) in self.materials.iter().enumerate() {
            let Some((layer, name)) = m.id.split_once('/') else {
                return bad(format!("material {:?} is not layer/name", m.id));
            };
            if !LAYERS.contains(&layer) || name.is_empty() {
                return bad(format!("material {:?} names no known layer", m.id));
            }
            if self.materials[..i].iter().any(|o| o.id == m.id) {
                return bad(format!("material {:?} is defined twice", m.id));
            }
            if let Some(e) = &m.element {
                if layer != "catalyst" {
                    return bad(format!(
                        "material {:?}: only catalysts carry an element",
                        m.id
                    ));
                }
                if element(e).is_none() {
                    return bad(format!("material {:?}: unknown element {e:?}", m.id));
                }
            }
            // The jobs (GEAR.md 6): each layer's own, and a catalyst's status its element's.
            let has_status = m.status.is_some()
                || m.chance.is_some()
                || m.duration_ms.is_some()
                || m.magnitude.is_some();
            if has_status {
                if layer != "catalyst" {
                    return bad(format!(
                        "material {:?}: only a catalyst puts a status on a blow",
                        m.id
                    ));
                }
                let (Some(status), Some(chance), Some(duration), Some(_)) =
                    (&m.status, m.chance, m.duration_ms, m.magnitude)
                else {
                    return bad(format!(
                        "material {:?}: a status wants status, chance, duration_ms and magnitude",
                        m.id
                    ));
                };
                let Some(st) = Status::parse(status) else {
                    return bad(format!("material {:?}: unknown status {status:?}", m.id));
                };
                let own = m
                    .element
                    .as_deref()
                    .and_then(element)
                    .and_then(dtype_element);
                if st.element() != own {
                    return bad(format!(
                        "material {:?}: {status} is not the {}'s status",
                        m.id,
                        m.element.as_deref().unwrap_or("")
                    ));
                }
                if !(1..=100).contains(&chance) || duration == 0 {
                    return bad(format!(
                        "material {:?}: chance is 1–100 and duration_ms at least 1",
                        m.id
                    ));
                }
            }
            if let Some(pct) = m.stamina {
                if layer != "frame" {
                    return bad(format!(
                        "material {:?}: only a frame moves the stamina",
                        m.id
                    ));
                }
                if !(25..=100).contains(&pct) {
                    return bad(format!("material {:?}: stamina is 25–100 percent", m.id));
                }
            }
            if let Some(w) = &m.ward {
                if layer != "gem" {
                    return bad(format!("material {:?}: only a gem wards", m.id));
                }
                if w != "catalyst" && element(w).is_none() {
                    return bad(format!(
                        "material {:?}: wards catalyst or an element, not {w:?}",
                        m.id
                    ));
                }
            }
        }
        for (i, t) in self.templates.iter().enumerate() {
            if t.id.is_empty() || t.id == "component" {
                return bad(format!("template id {:?} is reserved or empty", t.id));
            }
            if self.templates[..i].iter().any(|o| o.id == t.id) {
                return bad(format!("template {:?} is defined twice", t.id));
            }
            if !["weapon", "armour", "shield", "stack", "trinket"].contains(&t.kind.as_str()) {
                return bad(format!("template {:?}: unknown kind {:?}", t.id, t.kind));
            }
            // Its place (EQUIPMENT.md 10.8): an armour's on the body, a trinket's the
            // accessory, nothing else's named.
            match t.kind.as_str() {
                "armour" if Place::of_armour(t.place.as_deref()).is_none() => {
                    return bad(format!(
                        "template {:?}: an armour's place is chest, head or legs",
                        t.id
                    ));
                }
                "trinket" if Place::of_trinket(t.place.as_deref()).is_none() => {
                    return bad(format!(
                        "template {:?}: a trinket's place is accessory",
                        t.id
                    ));
                }
                "armour" | "trinket" => {}
                _ if t.place.is_some() => {
                    return bad(format!(
                        "template {:?}: only an armour or a trinket names a place",
                        t.id
                    ));
                }
                _ => {}
            }
            if t.kind == "stack" {
                // A stack (MODES.md 11.1): a cap within the hub's column, a heal within
                // reason, nothing of a craft.
                match t.cap {
                    Some(1..=1000) => {}
                    _ => return bad(format!("template {:?}: a stack's cap is 1–1000", t.id)),
                }
                if let Some(h) = t.heals
                    && !(1..=1000).contains(&h)
                {
                    return bad(format!("template {:?}: heals is 1–1000", t.id));
                }
                if !t.layers.is_empty() || t.strikes.is_some() || t.guards.is_some() {
                    return bad(format!(
                        "template {:?}: a stack has no layers, strikes with nothing and guards against nothing",
                        t.id
                    ));
                }
                continue;
            }
            if t.cap.is_some() || t.heals.is_some() {
                return bad(format!(
                    "template {:?}: only a stack has a cap or heals",
                    t.id
                ));
            }
            match (t.kind.as_str(), t.strikes.as_deref(), t.guards.as_deref()) {
                ("weapon", Some(kind), None) if physical(kind).is_some() => {}
                ("weapon", _, _) => {
                    return bad(format!(
                        "template {:?}: a weapon strikes with slash, pierce or blunt, and guards against nothing",
                        t.id
                    ));
                }
                // A trinket is for the elements by nature (EQUIPMENT.md 5.4 and 10.8): it
                // strikes with nothing; it guards against the elements (a cape, for what
                // its wearer takes) or names no guard (a ring, for what it deals).
                ("trinket", None, None | Some("elements")) => {}
                ("trinket", _, _) => {
                    return bad(format!(
                        "template {:?}: a trinket strikes with nothing and guards against the elements or nothing",
                        t.id
                    ));
                }
                (_, None, Some("physical" | "elements")) => {}
                _ => {
                    return bad(format!(
                        "template {:?}: an armour or a shield guards against physical or elements, and strikes with nothing",
                        t.id
                    ));
                }
            }
            // The armour class is the chest piece's and nobody else's (GEAR.md 3).
            if let Some(class) = &t.class {
                if t.kind != "armour" || t.place.as_deref().is_some_and(|p| p != "chest") {
                    return bad(format!(
                        "template {:?}: only an armour on the chest carries an armour class",
                        t.id
                    ));
                }
                if !["cloth", "leather", "mail", "plate"].contains(&class.as_str()) {
                    return bad(format!(
                        "template {:?}: the class is cloth, leather, mail or plate, not {class:?}",
                        t.id
                    ));
                }
            }
            // A shield is worn in the left hand and nowhere else; a weapon in the hand its
            // `held` names (ITEMS.md 2).
            if t.kind == "shield" && t.held() != "left" {
                return bad(format!("template {:?}: a shield is held left", t.id));
            }
            if !["right", "left", "either", "back", "two"].contains(&t.held()) {
                return bad(format!(
                    "template {:?}: held must be right, left, either, back or two",
                    t.id
                ));
            }
            if t.two_handed() && t.kind != "weapon" {
                return bad(format!(
                    "template {:?}: only a weapon takes both hands",
                    t.id
                ));
            }
            for layer in &t.layers {
                if !LAYERS.contains(&layer.as_str()) {
                    return bad(format!("template {:?}: unknown layer {layer:?}", t.id));
                }
            }
            // A weapon, an armour and a shield are built round a core on a frame; a
            // trinket round a catalyst (EQUIPMENT.md 5.4).
            let needed: &[&str] = if t.kind == "trinket" {
                &["catalyst"]
            } else {
                &["core", "frame"]
            };
            for needed in needed {
                if !t.layers.iter().any(|l| l == needed) {
                    return bad(format!("template {:?} must take a {needed}", t.id));
                }
            }
            let edge = self.best_edge(t);
            if edge > MAX_EDGE_PER_MILLE {
                return bad(format!(
                    "template {:?}: the best craft gives {edge} per mille, over the cap of {MAX_EDGE_PER_MILLE}",
                    t.id
                ));
            }
        }
        Ok(())
    }
}

pub fn load_items_str(text: &str) -> Result<ItemContent, ContentError> {
    let content: ItemContent = toml::from_str(text).map_err(|e| ContentError::Toml {
        path: "items.toml".into(),
        source: e,
    })?;
    content.validate()?;
    Ok(content)
}

pub fn load_items(dir: &Path) -> Result<ItemContent, ContentError> {
    let path = dir.join("items.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| ContentError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    load_items_str(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/content");

    #[test]
    fn shipped_items_load_under_the_edge_cap() {
        let items = load_items(Path::new(DIR)).expect("items load");
        assert!(items.templates.len() >= 7 && items.materials.len() >= 15);
        let best = items
            .templates
            .iter()
            .map(|t| items.best_edge(t))
            .max()
            .unwrap();
        // The best sword: dragonbone 60 + catalyst 30 + whalebone 45 + two opals (the shard's
        // 55 went with the layer, GEAR.md 6).
        assert_eq!(best, 195);
    }

    #[test]
    fn each_layer_sharpens_its_own_kind_of_damage() {
        let items = load_items(Path::new(DIR)).expect("items load");
        let at = |t: DamageType| t as usize;
        // The best sword, with an ember catalyst (ITEMS.md 3.2): its core is for the
        // sword's own kind, its catalyst for its element, the rest for those two.
        let best = [
            "core/dragonbone",
            "catalyst/ember",
            "frame/whalebone",
            "gem/opal",
            "gem/opal",
        ];
        let (place, sword, whole) = items.edges("sword", best).unwrap();
        assert_eq!((place, whole), (Place::Weapon, 195));
        let mut expect = [0u16; 9];
        expect[at(DamageType::Slash)] = 165;
        expect[at(DamageType::Fire)] = 135;
        assert_eq!(sword, expect, "nothing for a kind the sword is not for");
        // The same parts in a hammer sharpen Blunt, not Slash.
        let (_, hammer, _) = items.edges("hammer", best).unwrap();
        assert_eq!(
            (hammer[at(DamageType::Blunt)], hammer[at(DamageType::Slash)]),
            (165, 0)
        );
        // A cuirass guards against blows and takes no catalyst: one handed to it (an item
        // made before the rule) adds nothing.
        let plain = ["core/dragonbone", "frame/whalebone", "gem/opal", "gem/opal"];
        let (place, cuirass, whole) = items.edges("cuirass", plain).unwrap();
        assert_eq!((place, whole), (Place::Armour, 165));
        assert_eq!(cuirass, [165, 165, 165, 0, 0, 0, 0, 0, 0]);
        assert_eq!(items.edges("cuirass", best).unwrap().1, cuirass);
        // A robe guards against the elements, and its catalyst against one the more.
        let (_, robe, whole) = items.edges("robe", best).unwrap();
        assert_eq!(whole, 195);
        assert_eq!(robe, [0, 0, 0, 195, 165, 165, 165, 165, 165]);
        let (_, robe, _) = items
            .edges("robe", ["core/iron", "catalyst/rime", "frame/oak"])
            .unwrap();
        assert_eq!(robe[at(DamageType::Water)], 70);
        assert_eq!(robe[at(DamageType::Fire)], 40);
        assert_eq!(robe[at(DamageType::Slash)], 0);
        // A component is worn nowhere; nor is what the content does not know. A material it
        // does not know adds nothing.
        assert_eq!(items.edges("component", ["core/iron"]), None);
        assert_eq!(items.place("sword"), Some(Place::Weapon));
        assert_eq!(items.place("lute"), None);
        let (_, odd, _) = items
            .edges("sword", ["core/iron", "core/moonsilver"])
            .unwrap();
        assert_eq!(odd[at(DamageType::Slash)], 30);
        // Nothing an item can be made of goes past the cap in any one number.
        for t in items.templates.iter().filter(|t| t.kind != "stack") {
            let all: Vec<&str> = items.materials.iter().map(|m| m.id.as_str()).collect();
            let (_, edges, _) = items.edges(&t.id, all).unwrap();
            assert!(edges.iter().all(|e| *e <= 250), "{}", t.id);
        }
        // The three places (ITEMS.md 2): a sword goes into the right hand, and may go
        // into the left; a shield into the left only; a cuirass onto the body only.
        assert_eq!(items.place("shield"), Some(Place::Off));
        assert!(
            items.may_wear_in("sword", Place::Weapon) && items.may_wear_in("sword", Place::Off)
        );
        assert!(!items.may_wear_in("sword", Place::Armour));
        assert!(
            items.may_wear_in("shield", Place::Off) && !items.may_wear_in("shield", Place::Weapon)
        );
        assert!(
            items.may_wear_in("cuirass", Place::Armour)
                && !items.may_wear_in("cuirass", Place::Off)
        );
        assert!(
            !items.may_wear_in("hammer", Place::Off) && !items.may_wear_in("lute", Place::Weapon)
        );
        // A shield guards like an armour and is said as a shield; a sword says it may go
        // into the off hand too, a shield (there by nature) does not.
        let (place, shield, whole) = items.edges("shield", plain).unwrap();
        assert_eq!((place, whole), (Place::Off, 165));
        assert_eq!(shield, [165, 165, 165, 0, 0, 0, 0, 0, 0]);
        let view = items.view("shield", [("core", "core/iron"), ("frame", "frame/oak")]);
        assert_eq!(view.what, "a shield, 40 of 250");
        assert_eq!(view.does, ["physical -2.0%"]);
        assert!(!view.off);
        assert!(
            items
                .view("sword", [("core", "core/iron"), ("frame", "frame/oak")])
                .off
        );
        assert!(
            !items
                .view("cuirass", [("core", "core/iron"), ("frame", "frame/oak")])
                .off
        );
        // The robe hides the chest under it; the cuirass is a patch on the skin and hides
        // nothing (CONTENT.md 3.3).
        assert_eq!(items.covers("robe"), (1 << 1) | (1 << 2));
        assert_eq!(items.covers("cuirass"), 0);
        assert!(items.template("cuirass").is_some_and(|t| t.is_patch()));
        assert!(items.template("robe").is_some_and(|t| !t.is_patch()));
        assert_eq!(items.covers("sword"), 0);
        // A stack (MODES.md 11.1) is worn nowhere, has a cap, and a kit heals.
        assert_eq!(items.place("ball"), None);
        assert_eq!(items.edges("ball", []), None);
        assert_eq!(items.stack("ball"), Some((30, None)));
        assert_eq!(items.stack("kit"), Some((5, Some(300))));
        assert_eq!(items.stack("sword"), None);
        assert_eq!(items.stack_words("ball", 12), "ball ×12 of 30");
        assert_eq!(
            items.view("kit", []).does,
            vec!["heals 300, used with F".to_string()]
        );
    }

    #[test]
    fn an_item_is_put_into_words_by_the_content() {
        let items = load_items(Path::new(DIR)).expect("items load");
        let parts = |list: &[&'static str]| -> Vec<(&'static str, &'static str)> {
            list.iter()
                .map(|m| (m.split_once('/').unwrap().0, *m))
                .collect()
        };
        let best = parts(&[
            "core/dragonbone",
            "catalyst/ember",
            "frame/whalebone",
            "gem/opal",
            "gem/opal",
        ]);
        // A place counts for half its item's edge (ITEMS.md 3.1): 165 per mille is 8.3%;
        // then the layers' jobs (GEAR.md 6), each in a line.
        let sword = items.view("sword", best.clone());
        assert_eq!(sword.what, "a weapon, 195 of 250");
        assert_eq!(
            sword.does,
            [
                "slash +8.3%",
                "fire +6.8%",
                "burns one blow in five for 2.0 s",
                "its burns last 200% as long",
                "blows cost 75% of their stamina"
            ]
        );
        // An armour takes off what 2000 / 2105 leaves: 5.0%; its whalebone prices the blocks.
        let cuirass = items.view("cuirass", parts(&["core/dragonbone", "frame/whalebone"]));
        assert_eq!(cuirass.what, "an armour, 105 of 250");
        assert_eq!(
            cuirass.does,
            ["physical -5.0%", "blocks cost 75% of their stamina"]
        );
        // The robe's ember and two opals ward fire down to the floor.
        let robe = items.view("robe", best.clone());
        assert_eq!(
            robe.does,
            [
                "fire -8.9%",
                "other elements -7.6%",
                "burns last 25% as long on you",
                "blocks cost 75% of their stamina"
            ]
        );
        let robe = items.view("robe", parts(&["core/iron", "frame/oak"]));
        assert_eq!(robe.does, ["elements -2.0%"]);
        // Half a per mille is rounded, not lost: 45 is 2.3%.
        let staff = items.view("staff", parts(&["core/tin", "frame/ash", "gem/quartz"]));
        assert_eq!(
            staff.does,
            ["blunt +2.3%", "blows cost 90% of their stamina"]
        );
        // A part says what it is; what nobody knows says so.
        let part = items.view("component", parts(&["catalyst/ember"]));
        assert_eq!(
            (part.place, part.what.as_str()),
            (None, "a catalyst, for crafting")
        );
        assert!(part.does.is_empty() && part.edge == [0; 9]);
        assert_eq!(items.view("lute", best).what, "nothing this world knows");
        assert_eq!(items.layers("crossbow").unwrap().len(), 3);
        assert_eq!(items.layers("lute"), None);
    }

    /// EQUIPMENT.md 3: `held = "two"` is a weapon's, goes into the right hand and never
    /// the left; `either` goes into both.
    #[test]
    fn a_two_handed_weapon_goes_right_and_nowhere_else() {
        let items = load_items(Path::new(DIR)).expect("items load");
        assert!(items.two_handed("staff") && !items.two_handed("sword"));
        assert!(
            items.may_wear_in("staff", Place::Weapon) && !items.may_wear_in("staff", Place::Off)
        );
        assert!(
            items.may_wear_in("sword", Place::Weapon) && items.may_wear_in("sword", Place::Off)
        );
        assert!(!items.two_handed("nothing"));
        let text = std::fs::read_to_string(Path::new(DIR).join("items.toml")).unwrap();
        let bad = text.replace(
            "id = \"shield\"\nkind = \"shield\"",
            "id = \"shield\"\nkind = \"shield\"\nheld = \"two\"",
        );
        assert!(bad != text);
        assert!(load_items_str(&bad).is_err(), "a shield in both hands");
    }

    #[test]
    fn an_item_over_the_cap_is_refused() {
        let text = r#"
            [[template]]
            id = "sword"
            kind = "weapon"
            strikes = "slash"
            layers = ["core", "frame", "gem"]
            [[material]]
            id = "core/a"
            edge = 150
            [[material]]
            id = "frame/b"
            edge = 60
            [[material]]
            id = "gem/c"
            edge = 25
        "#;
        let err = load_items_str(text).unwrap_err().to_string();
        assert!(err.contains("260 per mille"), "{err}");
        assert!(load_items_str(&text.replace("edge = 25", "edge = 20")).is_ok());
        // A template without a frame, and a material without a layer, are refused.
        assert!(
            load_items_str(&text.replace(r#"["core", "frame", "gem"]"#, r#"["core", "gem"]"#))
                .is_err()
        );
        assert!(load_items_str(&text.replace("core/a", "a")).is_err());
        // A weapon says what it strikes with, and only a weapon does.
        let ok = text.replace("edge = 25", "edge = 20");
        assert!(load_items_str(&ok.replace("strikes = \"slash\"", "")).is_err());
        assert!(load_items_str(&ok.replace("strikes = \"slash\"", "strikes = \"fire\"")).is_err());
        assert!(load_items_str(&ok.replace("kind = \"weapon\"", "kind = \"armour\"")).is_err());
        // An armour says what it guards against, and only an armour does.
        let armour = ok.replace("kind = \"weapon\"", "kind = \"armour\"");
        let guards = armour.replace("strikes = \"slash\"", "guards = \"elements\"");
        assert!(load_items_str(&guards).is_ok());
        assert!(load_items_str(&guards.replace("elements", "everything")).is_err());
        assert!(
            load_items_str(&ok.replace(
                "strikes = \"slash\"",
                "strikes = \"slash\"\nguards = \"physical\""
            ))
            .is_err()
        );
    }
}
