//! The character matrix: the data mirror of `docs/MATRIX.md`. Elements, physical kinds,
//! armour classes, frame modifiers, derived stats and the damage pipeline (MATRIX.md 7).
//!
//! Keep this file and the document in lockstep. When they disagree, the document wins and the
//! code is wrong.

use crate::vocab::{ArchetypeFrame, Bypass, DamagePacket, DamageType, Status};

/// The six elements, in the order of MATRIX.md 5's table (the well-known chart, with
/// Air for its Flying): a type beats what its row says, and nothing more.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Element {
    Fire = 0,
    Water = 1,
    Grass = 2,
    Electric = 3,
    Ground = 4,
    Air = 5,
}

impl Element {
    /// Its place in `ALL` and in every per-element table (GEAR.md 6).
    pub const fn slot(self) -> usize {
        self as usize
    }
    pub const COUNT: usize = 6;
    /// The bits of `Aspects` that name an element.
    pub const MASK: u8 = 0x3f;

    pub const ALL: [Element; 6] = [
        Element::Fire,
        Element::Water,
        Element::Grass,
        Element::Electric,
        Element::Ground,
        Element::Air,
    ];

    pub const fn from_index(i: u8) -> Option<Element> {
        match i {
            0 => Some(Element::Fire),
            1 => Some(Element::Water),
            2 => Some(Element::Grass),
            3 => Some(Element::Electric),
            4 => Some(Element::Ground),
            5 => Some(Element::Air),
            _ => None,
        }
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn name(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Water => "water",
            Element::Grass => "grass",
            Element::Electric => "electric",
            Element::Ground => "ground",
            Element::Air => "air",
        }
    }
}

/// The element matrix of MATRIX.md 5, attacker by row, defending aspect by column, in
/// `Element`'s order. Three values: 1.25 (beats), 1 (nothing), 0.75 (resisted). The chart's
/// two immunities are 0.75 here (MATRIX.md 1: no immunity). 2 and 0.5 until 2026-10-19
/// (MATRIX.md 19, v8): a counter tilts a fight now, it does not decide it.
const MATRIX: [[f32; Element::COUNT]; Element::COUNT] = [
    // into: Fire, Water, Grass, Electric, Ground, Air
    [0.75, 0.75, 1.25, 1.00, 1.00, 1.00], // Fire
    [1.25, 0.75, 0.75, 1.00, 1.25, 1.00], // Water
    [0.75, 1.25, 0.75, 1.00, 1.25, 0.75], // Grass
    [1.00, 1.25, 0.75, 0.75, 0.75, 1.25], // Electric
    [1.25, 1.00, 0.75, 1.25, 1.00, 0.75], // Ground
    [1.00, 1.00, 1.25, 0.75, 1.00, 1.00], // Air
];

/// What an element does into the aspect it beats, and into the one that resists it
/// (MATRIX.md 5): one step either way, and a dual-aspect hole or wall is the square.
pub const BEATS: f32 = 1.25;
pub const RESISTED: f32 = 0.75;
/// BODIES.md 7.3 (2026-10-20): a spell of one of the attacker's own elements deals this
/// much more. A body's, like the group's factor; a preset in nothing has neither.
pub const AFFINITY: f32 = 1.25;
/// BODIES.md 7.3: the magic group's spells and the melee group's blows deal this much
/// more; the magic group's blows and the melee and ranged groups' spells this much less.
/// "Fire affinity + magical + INT build = near max damage dealt by fireball"; a Brute's
/// fireball is a puff, and its fist is not.
pub const GROUP_FAVOURED: f32 = 1.25;
pub const GROUP_UNFAVOURED: f32 = 0.75;

/// Damage multiplier of an `attack` element into one defending `aspect` (MATRIX.md 5).
pub fn element_mult(attack: Element, aspect: Element) -> f32 {
    MATRIX[attack as usize][aspect as usize]
}

/// A set of 0..=2 aspects as a bitmask over `Element` indices; none is neutral (MATRIX.md 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aspects(pub u8);

impl Aspects {
    pub const NONE: Aspects = Aspects(0);

    pub fn one(e: Element) -> Aspects {
        Aspects(1 << e as u8)
    }

    pub fn two(a: Element, b: Element) -> Aspects {
        Aspects((1 << a as u8) | (1 << b as u8))
    }

    pub fn contains(self, e: Element) -> bool {
        self.0 & (1 << e as u8) != 0
    }

    pub fn count(self) -> u32 {
        (self.0 & Element::MASK).count_ones()
    }

    pub fn iter(self) -> impl Iterator<Item = Element> {
        Element::ALL.into_iter().filter(move |e| self.contains(*e))
    }

    /// Product of the element multipliers over the aspects; 1 when there are none.
    pub fn mult_against(self, attack: Element) -> f32 {
        self.iter()
            .map(|asp| element_mult(attack, asp))
            .product::<f32>()
    }
}

/// Physical damage kinds (MATRIX.md 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Kind {
    Slash = 0,
    Pierce = 1,
    Blunt = 2,
}

/// Frame modifiers (MATRIX.md 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameMods {
    pub mass: f32,
    pub mobility: f32,
    pub armour: f32,
    pub ward: f32,
    pub evasion: f32,
    pub focus: i32,
}

impl ArchetypeFrame {
    pub const fn mods(self) -> FrameMods {
        match self {
            ArchetypeFrame::Colossus => FrameMods {
                mass: 1.5,
                mobility: 0.92,
                armour: 0.10,
                ward: 0.0,
                evasion: 0.0,
                focus: 0,
            },
            ArchetypeFrame::Striker => FrameMods {
                mass: 1.0,
                mobility: 1.0,
                armour: 0.05,
                ward: 0.0,
                evasion: 0.0,
                focus: 0,
            },
            ArchetypeFrame::Caster => FrameMods {
                mass: 0.9,
                mobility: 0.96,
                armour: 0.0,
                ward: 0.10,
                evasion: 0.0,
                focus: 2,
            },
            ArchetypeFrame::Infiltrator => FrameMods {
                mass: 0.8,
                mobility: 1.06,
                armour: 0.0,
                ward: 0.0,
                evasion: 0.05,
                focus: 0,
            },
        }
    }
}

/// The five attributes (MATRIX.md 2), each `MIN..=MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Attributes {
    pub str_: u8,
    pub agi: u8,
    pub con: u8,
    pub int: u8,
    pub spr: u8,
}

impl Attributes {
    /// The least an attribute may be (MATRIX.md 2): a body's floor may stand here where
    /// the body is plainly bad at something (BODIES.md 3.2; 5 until 2026-10-19).
    pub const MIN: u8 = 3;
    /// The most an attribute may be: no cap of its own (25 until 2026-10-19; the director:
    /// "if someone wants, they can put all to one stat"), only what the highest floor a
    /// body may have (`FLOOR_SUM` less four at `MIN`) and every free point on it reach.
    pub const MAX: u8 = Self::FLOOR_SUM as u8 - 4 * Self::MIN + Self::FREE_POINTS as u8;
    /// The floor of a build without a body (a preset, a creature, a bot): five each, as
    /// every character had until the bodies. A body brings its own floors, summing to
    /// `FLOOR_SUM`.
    pub const BASE: u8 = 5;
    /// What a body's five floors sum to (BODIES.md 3.1): five fives, spread the body's way.
    pub const FLOOR_SUM: u32 = 25;
    /// Points a character has to raise its attributes above the floor (MATRIX.md 2): its
    /// own, apart from the kit's budget. Fewer may be spent; more is refused.
    pub const FREE_POINTS: u32 = 30;

    pub const fn flat(v: u8) -> Attributes {
        Attributes {
            str_: v,
            agi: v,
            con: v,
            int: v,
            spr: v,
        }
    }

    pub const fn new(str_: u8, agi: u8, con: u8, int: u8, spr: u8) -> Attributes {
        Attributes {
            str_,
            agi,
            con,
            int,
            spr,
        }
    }

    pub fn as_array(self) -> [u8; 5] {
        [self.str_, self.agi, self.con, self.int, self.spr]
    }

    /// Attribute points spent above the bodiless floor (`BASE`): every point above it
    /// costs one of `FREE_POINTS`. A build with a body counts above the body's floors
    /// (`spent`; `Build::points_spent`).
    pub fn cost(self) -> u32 {
        self.spent(Attributes::flat(Self::BASE))
    }

    /// Attribute points spent above `floors`: every point above a floor costs one of
    /// `FREE_POINTS`; a value under its floor counts nothing (and does not validate).
    pub fn spent(self, floors: Attributes) -> u32 {
        self.as_array()
            .iter()
            .zip(floors.as_array())
            .map(|(&v, f)| v.saturating_sub(f) as u32)
            .sum()
    }

    /// Every attribute at or above its floor.
    pub fn at_floors(self, floors: Attributes) -> bool {
        self.as_array()
            .iter()
            .zip(floors.as_array())
            .all(|(&v, f)| v >= f)
    }

    /// The sum of the five.
    pub fn sum(self) -> u32 {
        self.as_array().iter().map(|&v| v as u32).sum()
    }

    pub fn in_range(self) -> bool {
        self.as_array()
            .iter()
            .all(|v| (Self::MIN..=Self::MAX).contains(v))
    }

    /// In range and within the free points.
    pub fn affordable(self) -> bool {
        self.in_range() && self.cost() <= Self::FREE_POINTS
    }
}

impl Default for Attributes {
    fn default() -> Self {
        // Every attribute at 11: the thirty points spread evenly.
        Attributes::flat(11)
    }
}

/// Everything the simulation reads per character (MATRIX.md 6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Derived {
    /// HP.
    pub health: i32,
    /// ST, the stamina: what a block takes (MATRIX.md 23).
    pub stamina: f32,
    pub stamina_regen: f32,
    /// MP, the magic points: what every skill but the primary takes (MATRIX.md 23).
    pub focus: f32,
    pub focus_regen: f32,
    pub max_speed: f32,
    pub physical_mult: f32,
    pub elemental_mult: f32,
    pub armour: f32,
    pub ward: f32,
    pub evasion: f32,
    pub knockback_taken: f32,
    pub knockback_dealt: f32,
    pub status_duration: f32,
    pub stagger_threshold: f32,
}

pub const MAX_ARMOUR: f32 = 0.50;
pub const MAX_WARD: f32 = 0.60;
pub const MAX_EVASION: f32 = 0.50;
/// Evasion lingers this many ticks after a `MoveSelf` ends (MATRIX.md 6).
pub const EVADING_GRACE_TICKS: u32 = 2;
/// Stagger build-up decays this many points per second (MATRIX.md 7).
pub const STAGGER_DECAY_PER_S: f32 = 20.0;
/// Stagger applied when the threshold is crossed, milliseconds.
pub const STAGGER_MS: u32 = 400;
/// Build-up is discarded this long after a stagger ends (MATRIX.md 7).
pub const STAGGER_IMMUNITY_MS: u32 = 1000;
/// Freeze: Root this long, then Chill immunity (MATRIX.md 8).
pub const FREEZE_MS: u32 = 1000;
pub const CHILL_IMMUNITY_MS: u32 = 2000;
/// Chill slows this much per stack.
pub const CHILL_PER_STACK: f32 = 0.15;
/// Damage- and heal-over-time pulses per second: one, so a magnitude of 1 reads as a
/// point a second (the numbers are a tenth of what they were, 2026-10-20).
pub const DOT_PULSES_PER_S: u32 = 1;

impl Derived {
    pub fn compute(attrs: Attributes, frame: ArchetypeFrame) -> Derived {
        let f = frame.mods();
        let (str_, agi, con, int, spr) = (
            attrs.str_ as f32,
            attrs.agi as f32,
            attrs.con as f32,
            attrs.int as f32,
            attrs.spr as f32,
        );
        Derived {
            health: 50 + 4 * attrs.con as i32,
            // ST and MP a tenth of what they were, as HP is (MATRIX.md 23, 2026-10-22).
            stamina: 6.0 + 0.3 * (con + agi),
            stamina_regen: 1.0 + 0.06 * agi,
            focus: 4.0 + 0.5 * int + f.focus as f32,
            focus_regen: 0.5 + 0.06 * spr,
            max_speed: (280.0 + 2.0 * agi) * f.mobility,
            physical_mult: 0.60 + 0.04 * str_,
            elemental_mult: 0.60 + 0.04 * int,
            armour: (0.015 * con + f.armour).min(MAX_ARMOUR),
            ward: (0.02 * spr + f.ward).min(MAX_WARD),
            evasion: (0.015 * agi + f.evasion).min(MAX_EVASION),
            knockback_taken: 1.0 / f.mass,
            knockback_dealt: 0.60 + 0.04 * str_,
            status_duration: 1.30 - 0.03 * spr,
            stagger_threshold: 40.0 + 3.0 * con,
        }
    }
}

/// What worn gear does to damage (ITEMS.md 3): the edge of the worn weapon and of the worn
/// armour, per mille, one number per damage type in `DamageType`'s order. `dealt` raises
/// what the body deals of that type, `taken` lowers what it takes. A body in nothing has
/// all eighteen at 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct Gear {
    pub dealt: [u16; 9],
    pub taken: [u16; 9],
    /// GEAR.md 6: the worn weapon's catalyst, its element's status on a landing blow.
    pub on_hit: Option<OnHit>,
    /// Per element (`Element::slot`), percent: how long that element's status lasts on this
    /// body (the catalysts and the gems of what guards it); 100 for nothing.
    pub status_taken: [u8; 6],
    /// Per element, percent: how long that element's status, put on by this body's blows,
    /// lasts on whoever they land on (the worn weapon's gems); 100 for nothing.
    pub status_dealt: [u8; 6],
    /// The worn weapon's frame: the stamina of the blows, percent of the cost.
    pub stamina_blows: u8,
    /// The guard pieces' frames: the stamina of a block and of the guard, percent.
    pub stamina_block: u8,
}

/// The status a weapon's catalyst puts on a landing blow (GEAR.md 6): at `chance` percent
/// per blow that lands, for `duration_ms`, at `magnitude_pm` per mille of the status's
/// magnitude. Rolled by the zone; a client predicts none of it, as with damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct OnHit {
    pub status: Status,
    pub chance: u8,
    pub duration_ms: u16,
    pub magnitude_pm: u16,
}

impl Default for Gear {
    fn default() -> Gear {
        Gear::NONE
    }
}

impl Gear {
    pub const NONE: Gear = Gear {
        dealt: [0; 9],
        taken: [0; 9],
        on_hit: None,
        status_taken: [100; 6],
        status_dealt: [100; 6],
        stamina_blows: 100,
        stamina_block: 100,
    };
    /// The most any one of the sixteen may be: the cap on an item's whole edge (PLAN.md 0).
    pub const MAX: u16 = 250;
    /// What an edge is counted against (ITEMS.md 3.1): an edge of `e` per mille is a
    /// factor of `(SCALE + e) / SCALE`. 2000 until 2026-10-19 (a place counted for half
    /// of what its item's edge said, a 25% exchange at most, PLAN.md 0); 1100 since
    /// (MATRIX.md 19): the cap against nothing is `(1350 / 1100)² = 1.51`, an exchange
    /// of one and a half, and 1.40 against iron and oak.
    pub const SCALE: u32 = 1100;

    /// The same within the cap, whatever it was told.
    pub fn clamped(self) -> Gear {
        Gear {
            dealt: self.dealt.map(|e| e.min(Gear::MAX)),
            taken: self.taken.map(|e| e.min(Gear::MAX)),
            ..self
        }
    }
}

/// MATRIX.md 7's `gear` term: what the attacker's weapon adds to this type over what the
/// defender's armour takes off it. Between `SCALE / (SCALE + MAX)` and its inverse
/// (1/1.227 and 1.227), and exactly 1 for two bodies in nothing; a body in the best of
/// both against one in nothing has 1.227 each way, an exchange of 1.51 at the very most.
pub fn gear_factor(dealt: &[u16; 9], taken: &[u16; 9], dtype: DamageType) -> f32 {
    let t = dtype as usize;
    let side = |edge: u16| (Gear::SCALE + edge.min(Gear::MAX) as u32) as f32;
    side(dealt[t]) / side(taken[t])
}

/// The attacker's side of the pipeline, captured when the packet is created so a dead or
/// departed owner (area pulses, projectiles in flight) still resolves consistently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackerStats {
    pub physical_mult: f32,
    pub elemental_mult: f32,
    /// The attacker's own elements (BODIES.md 7.3): a spell of one of them deals
    /// `AFFINITY` more. `NONE` for a body without, a preset in nothing, and a pulse.
    pub affinity: Aspects,
    /// The attacker's group (BODIES.md 7.3), with a body; `None` for a preset in
    /// nothing, a creature, and a pulse: no factor.
    pub group: Option<crate::build::Group>,
    pub knockback_dealt: f32,
    /// `Weaken` magnitude, 0 when absent.
    pub weaken: f32,
    /// The edge of its worn weapon, per mille per damage type (`Gear::dealt`).
    pub gear: [u16; 9],
    /// The worn weapon's catalyst (GEAR.md 6), rolled when a blow lands.
    pub on_hit: Option<OnHit>,
    /// The worn weapon's gems (GEAR.md 6): how long an element's status lasts, percent.
    pub status_dealt: [u8; 6],
}

impl AttackerStats {
    pub const NEUTRAL: AttackerStats = AttackerStats {
        physical_mult: 1.0,
        elemental_mult: 1.0,
        affinity: Aspects::NONE,
        group: None,
        knockback_dealt: 1.0,
        on_hit: None,
        status_dealt: [100; 6],
        weaken: 0.0,
        gear: [0; 9],
    };

    pub fn from_derived(
        d: &Derived,
        weaken: f32,
        gear: &Gear,
        affinity: Aspects,
        group: Option<crate::build::Group>,
    ) -> AttackerStats {
        AttackerStats {
            physical_mult: d.physical_mult,
            elemental_mult: d.elemental_mult,
            affinity,
            group,
            knockback_dealt: d.knockback_dealt,
            weaken,
            gear: gear.dealt,
            on_hit: gear.on_hit,
            status_dealt: gear.status_dealt,
        }
    }
}

/// The defender's side of the pipeline at the moment of the hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefenderStats {
    pub aspects: Aspects,
    pub armour: f32,
    pub ward: f32,
    pub evasion: f32,
    /// `Fortify` magnitude, 0 when absent.
    pub fortify: f32,
    pub evading: bool,
    /// `Expose` on the defender: attackers bypass armour.
    pub exposed: bool,
    /// Block mitigation when the defender guards toward the attacker and the guard applies
    /// to this packet (melee, or a projectile against a shield), else `None`.
    pub block: Option<f32>,
    /// The edge of its worn armour, per mille per damage type (`Gear::taken`).
    pub gear: [u16; 9],
}

/// BODIES.md 7.3: what a group's blows (`elemental` false) and spells (true) are
/// multiplied by. Magic: spells `GROUP_FAVOURED`, blows `GROUP_UNFAVOURED`; melee: blows
/// `GROUP_FAVOURED`, spells `GROUP_UNFAVOURED`; ranged: blows as they are (a bow, a gun:
/// the range is the edge), spells `GROUP_UNFAVOURED`. No group: 1.
pub fn group_mult(group: Option<crate::build::Group>, elemental: bool) -> f32 {
    use crate::build::Group;
    match (group, elemental) {
        (None, _) => 1.0,
        (Some(Group::Magic), true) | (Some(Group::Melee), false) => GROUP_FAVOURED,
        (Some(Group::Magic), false) | (Some(Group::Melee | Group::Ranged), true) => {
            GROUP_UNFAVOURED
        }
        (Some(Group::Ranged), false) => 1.0,
    }
}

/// MATRIX.md 7, the gear factor included (ITEMS.md 3.1): the factor multiplies before the
/// rounding, so its bounds are on the unrounded number. Never returns less than 1.
pub fn resolve_damage(packet: &DamagePacket, a: &AttackerStats, d: &DefenderStats) -> i32 {
    let bypass = if d.exposed {
        packet.bypass.union(Bypass::ARMOR)
    } else {
        packet.bypass
    };
    let mut x = packet.amount as f32 * (1.0 - a.weaken).max(0.0);
    match packet.dtype.kind() {
        // A physical kind (slash, pierce, blunt) is a damage type the worn gear's edge
        // reads (`Gear::taken`); it multiplies nothing else since GEAR.md 12 (2026-10-17:
        // no armour class, no weapon triangle).
        Some(_) => {
            x *= a.physical_mult * group_mult(a.group, false);
            if !bypass.contains(Bypass::ARMOR) {
                x *= 1.0 - d.armour;
            }
        }
        None => {
            let element = packet
                .dtype
                .element()
                .expect("non-physical types are elements");
            x *= a.elemental_mult * d.aspects.mult_against(element);
            // BODIES.md 7.3: the caster's own element, and its group.
            if a.affinity.contains(element) {
                x *= AFFINITY;
            }
            x *= group_mult(a.group, true);
            if !bypass.contains(Bypass::MAGIC_SHIELD) {
                x *= 1.0 - d.ward;
            }
        }
    }
    if d.fortify > 0.0 && !bypass.contains(Bypass::MAGIC_SHIELD) {
        x *= 1.0 - d.fortify;
    }
    if d.evading && !bypass.contains(Bypass::EVASION) {
        x *= 1.0 - d.evasion;
    }
    if let Some(m) = d.block {
        x *= 1.0 - m;
    }
    x *= gear_factor(&a.gear, &d.gear, packet.dtype);
    (x + 0.5).floor().max(1.0) as i32
}

impl DamageType {
    /// The physical kind, or `None` for elements.
    pub const fn kind(self) -> Option<Kind> {
        match self {
            DamageType::Slash => Some(Kind::Slash),
            DamageType::Pierce => Some(Kind::Pierce),
            DamageType::Blunt => Some(Kind::Blunt),
            _ => None,
        }
    }

    pub const fn element(self) -> Option<Element> {
        match self {
            DamageType::Fire => Some(Element::Fire),
            DamageType::Water => Some(Element::Water),
            DamageType::Grass => Some(Element::Grass),
            DamageType::Electric => Some(Element::Electric),
            DamageType::Ground => Some(Element::Ground),
            DamageType::Air => Some(Element::Air),
            _ => None,
        }
    }

    pub const fn is_physical(self) -> bool {
        self.kind().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chart_is_lopsided_but_nobody_dominates() {
        // MATRIX.md 5: every element beats at least one and is beaten by at least one;
        // nothing is immune; the net standing (beats + walls - weak - walled) is within
        // one of even for every element.
        for a in Element::ALL {
            let beats = Element::ALL
                .iter()
                .filter(|&&d| element_mult(a, d) == BEATS)
                .count();
            let weak = Element::ALL
                .iter()
                .filter(|&&d| element_mult(d, a) == BEATS)
                .count();
            let walls = Element::ALL
                .iter()
                .filter(|&&d| d != a && element_mult(d, a) < 1.0)
                .count();
            let walled = Element::ALL
                .iter()
                .filter(|&&d| d != a && element_mult(a, d) < 1.0)
                .count();
            assert!(beats >= 1 && weak >= 1, "{a:?}");
            let net = beats as i32 + walls as i32 - weak as i32 - walled as i32;
            assert!((-1..=1).contains(&net), "{a:?} net {net}");
            // Four resist themselves; Ground and Air do not (the chart's numbers).
            let self_mult = element_mult(a, a);
            assert!(
                self_mult == RESISTED || self_mult == 1.0,
                "{a:?} into itself"
            );
            for d in Element::ALL {
                assert!(element_mult(a, d) > 0.0, "{a:?} into {d:?}: no immunity");
            }
        }
        // The readings of MATRIX.md 5.
        assert_eq!(element_mult(Element::Fire, Element::Grass), BEATS);
        assert_eq!(element_mult(Element::Water, Element::Fire), BEATS);
        assert_eq!(element_mult(Element::Water, Element::Ground), BEATS);
        assert_eq!(element_mult(Element::Grass, Element::Water), BEATS);
        assert_eq!(element_mult(Element::Grass, Element::Ground), BEATS);
        assert_eq!(element_mult(Element::Electric, Element::Water), BEATS);
        assert_eq!(element_mult(Element::Electric, Element::Air), BEATS);
        assert_eq!(element_mult(Element::Ground, Element::Fire), BEATS);
        assert_eq!(element_mult(Element::Ground, Element::Electric), BEATS);
        assert_eq!(element_mult(Element::Air, Element::Grass), BEATS);
        // The chart's immunities are resists here.
        assert_eq!(element_mult(Element::Electric, Element::Ground), RESISTED);
        assert_eq!(element_mult(Element::Ground, Element::Air), RESISTED);
        // Neutral pairs exist now.
        assert_eq!(element_mult(Element::Fire, Element::Ground), 1.0);
        assert_eq!(element_mult(Element::Air, Element::Water), 1.0);
        assert_eq!(element_mult(Element::Ground, Element::Ground), 1.0);
        assert_eq!(element_mult(Element::Water, Element::Water), RESISTED);
    }

    #[test]
    fn dual_aspects_stack_to_a_hole_and_a_wall() {
        // MATRIX.md 5: Water + Air has one hole (Electric, 1.25² = 1.5625); Grass + Ground
        // walls Electric at 0.75² = 0.5625.
        let water_air = Aspects::two(Element::Water, Element::Air);
        assert_eq!(water_air.mult_against(Element::Electric), BEATS * BEATS);
        let grass_ground = Aspects::two(Element::Grass, Element::Ground);
        assert_eq!(
            grass_ground.mult_against(Element::Electric),
            RESISTED * RESISTED
        );
        // The pairs with a hole are exactly the document's four.
        let mut holed = Vec::new();
        for a in Element::ALL {
            for b in Element::ALL {
                if a < b {
                    let pair = Aspects::two(a, b);
                    if Element::ALL
                        .iter()
                        .any(|&e| pair.mult_against(e) == BEATS * BEATS)
                    {
                        holed.push((a, b));
                    }
                }
            }
        }
        assert_eq!(
            holed,
            [
                (Element::Fire, Element::Electric),
                (Element::Fire, Element::Ground),
                (Element::Water, Element::Ground),
                (Element::Water, Element::Air),
            ]
        );
        assert_eq!(Aspects::NONE.mult_against(Element::Fire), 1.0);
        assert_eq!(Aspects::two(Element::Fire, Element::Fire).count(), 1);
        assert_eq!(Aspects(0xff).count(), 6);
    }

    #[test]
    fn derived_bands_match_the_document() {
        let lo = Derived::compute(Attributes::flat(5), ArchetypeFrame::Striker);
        let hi = Derived::compute(Attributes::flat(25), ArchetypeFrame::Striker);
        assert_eq!((lo.health, hi.health), (70, 150));
        assert_eq!((lo.stamina, hi.stamina), (9.0, 21.0));
        assert_eq!((lo.focus, hi.focus), (6.5, 16.5));
        assert!((lo.max_speed - 290.0).abs() < 1e-3 && (hi.max_speed - 330.0).abs() < 1e-3);
        assert!((lo.physical_mult - 0.8).abs() < 1e-6 && (hi.physical_mult - 1.6).abs() < 1e-6);
        assert!((lo.armour - 0.125).abs() < 1e-6 && (hi.armour - 0.425).abs() < 1e-6);
        assert!((lo.ward - 0.10).abs() < 1e-6 && (hi.ward - 0.50).abs() < 1e-6);
        // A colossus: slower by its mobility and nothing else (no armour class since
        // GEAR.md 12, 2026-10-17), armoured by its frame, heavy against knockback.
        let big = Derived::compute(Attributes::flat(25), ArchetypeFrame::Colossus);
        assert!((big.max_speed - 330.0 * 0.92).abs() < 1e-3);
        assert!((big.armour - 0.475).abs() < 1e-6);
        assert!((big.knockback_taken - 1.0 / 1.5).abs() < 1e-6);
        // Thirty points: all of them, or fewer; never more, never past the cap.
        assert_eq!(Attributes::new(20, 20, 10, 5, 5).cost(), 35);
        assert!(!Attributes::new(20, 20, 10, 5, 5).affordable());
        assert!(Attributes::new(25, 15, 5, 5, 5).affordable());
        assert!(Attributes::new(5, 5, 5, 5, 5).affordable());
        assert!(!Attributes::new(2, 20, 10, 5, 5).in_range());
        // A body's floors (BODIES.md 3.2): 3 is in range, and the points count above the
        // floors, not above five.
        assert!(Attributes::new(3, 9, 5, 4, 4).in_range());
        let whelp = Attributes::new(3, 9, 5, 4, 4);
        assert_eq!(whelp.sum(), Attributes::FLOOR_SUM);
        assert_eq!(Attributes::new(3, 25, 10, 8, 9).spent(whelp), 30);
        assert!(Attributes::new(3, 25, 10, 8, 9).at_floors(whelp));
        assert!(!Attributes::new(3, 8, 10, 8, 9).at_floors(whelp));
        // No cap but the points (2026-10-19): every point on one stat is affordable, one
        // past them is not, and `MAX` is exactly what the highest floor plus the points reach.
        assert!(Attributes::new(35, 5, 5, 5, 5).affordable());
        assert!(!Attributes::new(36, 5, 5, 5, 5).affordable());
        assert_eq!(Attributes::MAX, 43);
        assert!(!Attributes::new(44, 5, 5, 5, 5).in_range());
        assert_eq!(Attributes::default().cost(), Attributes::FREE_POINTS);
    }

    fn packet(amount: u16, dtype: DamageType, bypass: Bypass) -> DamagePacket {
        DamagePacket {
            amount,
            dtype,
            bypass,
            knockback: 0.0,
            stagger: 0,
        }
    }

    fn defender(aspects: Aspects) -> DefenderStats {
        DefenderStats {
            aspects,
            armour: 0.2,
            ward: 0.2,
            evasion: 0.2,
            fortify: 0.0,
            evading: false,
            exposed: false,
            block: None,
            gear: [0; 9],
        }
    }

    #[test]
    fn gear_moves_each_type_by_its_own_edge_and_never_past_the_cap_a_side() {
        const SCALE: u32 = Gear::SCALE;
        const SIDE: f32 = (SCALE + Gear::MAX as u32) as f32 / SCALE as f32;
        let d = defender(Aspects::one(Element::Electric));
        let bare = AttackerStats {
            physical_mult: 1.0,
            elemental_mult: 1.0,
            ..AttackerStats::NEUTRAL
        };
        // The best sword the content allows, with an ember catalyst (ITEMS.md 3.2):
        // Slash 220, Fire 190, nothing else.
        let mut sword = [0u16; 9];
        sword[DamageType::Slash as usize] = 220;
        sword[DamageType::Fire as usize] = 190;
        let armed = AttackerStats {
            gear: sword,
            ..bare
        };
        // The best cuirass: the three physical kinds 220, and an edge on one element.
        let mut cuirass = [0u16; 9];
        for kind in [DamageType::Slash, DamageType::Pierce, DamageType::Blunt] {
            cuirass[kind as usize] = 220;
        }
        cuirass[DamageType::Fire as usize] = 250;
        let clad = DefenderStats { gear: cuirass, ..d };
        for dtype in DamageType::ALL {
            let p = packet(1000, dtype, Bypass::NONE);
            let plain = resolve_damage(&p, &bare, &d) as f32;
            let t = dtype as usize;
            // Each side alone, and both: the plain number times the factor, to the unit.
            for (a, dd, factor) in [
                (&armed, &d, (SCALE + sword[t] as u32) as f32 / SCALE as f32),
                (
                    &bare,
                    &clad,
                    SCALE as f32 / (SCALE + cuirass[t] as u32) as f32,
                ),
                (
                    &armed,
                    &clad,
                    (SCALE + sword[t] as u32) as f32 / (SCALE + cuirass[t] as u32) as f32,
                ),
            ] {
                let got = resolve_damage(&p, a, dd) as f32;
                assert!(
                    (got - plain * factor).abs() <= 1.0,
                    "{dtype:?}: {got} against {plain} x {factor}"
                );
            }
            // The term itself, whatever a zone was told: never past the cap a side.
            for dealt in [0u16, 1, 40, 220, 250, 251, 9000, u16::MAX] {
                for taken in [0u16, 1, 40, 220, 250, 251, 9000, u16::MAX] {
                    let f = gear_factor(&[dealt; 9], &[taken; 9], dtype);
                    assert!((1.0 / SIDE..=SIDE).contains(&f), "{dealt} {taken}: {f}");
                }
            }
        }
        // A sword's own kind against the same in armour is a wash; nothing worn is 1.
        assert_eq!(gear_factor(&sword, &cuirass, DamageType::Slash), 1.0);
        assert_eq!(gear_factor(&[0; 9], &[0; 9], DamageType::Water), 1.0);
        // Whatever a zone is told, no number counts for more than the cap.
        let wild = Gear {
            dealt: [9000; 9],
            taken: [0, 251, 250, 3, 0, 0, 0, 0, 65535],
            ..Gear::NONE
        };
        assert_eq!(wild.clamped().dealt, [250; 9]);
        assert_eq!(wild.clamped().taken, [0, 250, 250, 3, 0, 0, 0, 0, 250]);
        assert_eq!(gear_factor(&wild.dealt, &[0; 9], DamageType::Air), SIDE);
        assert_eq!(
            gear_factor(&[0; 9], &wild.taken, DamageType::Air),
            1.0 / SIDE
        );
        // What PLAN.md 0 bounds is a character's edge: the best of both places against a
        // body in nothing wins an exchange of its own kind by 44%, against iron and oak
        // (an edge of 40) by 34%, and by 51% at the very cap (MATRIX.md 19).
        let exchange = |mine: u16, theirs: u16| {
            let (a, b) = ([mine; 9], [theirs; 9]);
            gear_factor(&a, &b, DamageType::Slash) / gear_factor(&b, &a, DamageType::Slash)
        };
        assert!(
            (exchange(220, 0) - 1.44).abs() < 1e-3,
            "{}",
            exchange(220, 0)
        );
        assert!(
            (exchange(220, 40) - 1.3407).abs() < 1e-3,
            "{}",
            exchange(220, 40)
        );
        assert!(
            (exchange(250, 0) - 1.5062).abs() < 1e-3,
            "{}",
            exchange(250, 0)
        );
        // The smallest hit stays a hit.
        let weak = AttackerStats {
            weaken: 0.99,
            ..bare
        };
        assert_eq!(
            resolve_damage(&packet(1, DamageType::Slash, Bypass::NONE), &weak, &clad),
            1
        );
    }

    /// BODIES.md 7.3: a Sorceress of Fire at INT 25 throws a staff's fireball at 1.6 x
    /// 1.25 x 1.25 of its row; a Brute of no element at INT 3 with the same staff at
    /// 0.72 x 0.75. The attribute, the affinity and the group multiply; a melee body's
    /// blow is a quarter more than its STR makes it, a magic body's a quarter less.
    #[test]
    fn affinity_and_group_stack_on_a_spell() {
        use crate::build::Group;
        let d = DefenderStats {
            aspects: Aspects::NONE,
            armour: 0.0,
            ward: 0.0,
            evasion: 0.0,
            fortify: 0.0,
            evading: false,
            exposed: false,
            block: None,
            gear: [0; 9],
        };
        let fire = packet(100, DamageType::Fire, Bypass::NONE);
        let slash = packet(100, DamageType::Slash, Bypass::NONE);
        let sorceress = AttackerStats {
            elemental_mult: 1.6,
            physical_mult: 0.72,
            affinity: Aspects::one(Element::Fire),
            group: Some(Group::Magic),
            ..AttackerStats::NEUTRAL
        };
        let brute = AttackerStats {
            elemental_mult: 0.72,
            physical_mult: 0.88,
            affinity: Aspects::NONE,
            group: Some(Group::Melee),
            ..AttackerStats::NEUTRAL
        };
        assert_eq!(resolve_damage(&fire, &sorceress, &d), 250);
        assert_eq!(resolve_damage(&fire, &brute, &d), 54);
        assert_eq!(resolve_damage(&slash, &sorceress, &d), 54);
        assert_eq!(resolve_damage(&slash, &brute, &d), 110);
        // Water with the Fire affinity: the affinity is the element's, not any spell's.
        let water = packet(100, DamageType::Water, Bypass::NONE);
        assert_eq!(resolve_damage(&water, &sorceress, &d), 200);
        // A preset in nothing (no group, no affinity): the attribute alone.
        let preset = AttackerStats {
            elemental_mult: 1.6,
            ..AttackerStats::NEUTRAL
        };
        assert_eq!(resolve_damage(&fire, &preset, &d), 160);
    }

    #[test]
    fn pipeline_follows_the_document() {
        let a = AttackerStats {
            physical_mult: 1.1,
            elemental_mult: 1.2,
            affinity: Aspects::NONE,
            group: None,
            knockback_dealt: 1.0,
            weaken: 0.0,
            gear: [0; 9],
            on_hit: None,
            status_dealt: [100; 6],
        };
        // Slash: 100 * 1.1 * (1 - 0.2) = 88 (no armour class since GEAR.md 12: the
        // kind multiplies nothing but the gear's edge).
        let d = defender(Aspects::one(Element::Ground));
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Slash, Bypass::NONE), &a, &d),
            88
        );
        // Armour bypass skips the mitigation: 110.
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Slash, Bypass::ARMOR), &a, &d),
            110
        );
        // Water into Ground (1.25x) ignores the armour, pays the ward:
        // 100 * 1.2 * 1.25 * 0.8 = 120.
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Water, Bypass::NONE), &a, &d),
            120
        );
        // Electric into Ground is 0.75 (the chart's immunity, MATRIX.md 1): 72.
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Electric, Bypass::NONE), &a, &d),
            72
        );
        // Fire into Ground is nothing either way: 96.
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Fire, Bypass::NONE), &a, &d),
            96
        );
        // Fortify and evasion stack multiplicatively, blunt ignores the bubble:
        // 120 * 0.5 * 0.8 = 48.
        let mut d2 = d;
        d2.fortify = 0.5;
        d2.evading = true;
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Water, Bypass::NONE), &a, &d2),
            48
        );
        assert_eq!(
            resolve_damage(
                &packet(100, DamageType::Blunt, Bypass::MAGIC_SHIELD),
                &a,
                &d2
            ),
            70
        );
        // Expose grants armour bypass; a block removes its mitigation; never below 1.
        let mut d3 = d;
        d3.exposed = true;
        d3.block = Some(0.8);
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Slash, Bypass::NONE), &a, &d3),
            22
        );
        assert_eq!(
            resolve_damage(&packet(1, DamageType::Slash, Bypass::NONE), &a, &d3),
            1
        );
        // Weaken on the attacker.
        let weak = AttackerStats { weaken: 0.3, ..a };
        assert_eq!(
            resolve_damage(&packet(100, DamageType::Slash, Bypass::NONE), &weak, &d),
            62
        );
    }
}
