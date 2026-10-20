//! Builds and content packs (MATRIX.md 9 and 10): the slots, validation, and the runtime
//! [`Kit`] a build compiles to. Nothing is priced in points since 2026-10-20 (MATRIX.md 20):
//! a build is a body, thirty attribute points, its elements and its mode; what it plays is
//! the gear's and the body's.

use crate::matrix::{Aspects, Attributes, Derived, Element};
use crate::tick::{Tick, TickRate};
use crate::vocab::{
    Ability, ArchetypeFrame, DamageType, Guard, Mode, MoveKind, MoveSelf, Riposte, Trigger, Verb,
};

pub const MAX_ACTIVES: usize = 4;
/// Companions a character commands without a leadership ability, and with every bonus
/// (COMPANIONS.md 3.2).
pub const SQUAD_BASE: usize = 3;
pub const SQUAD_MAX: usize = 5;
/// The largest health a creature definition may override to (it rides the wire as a u16).
pub const MAX_CREATURE_HEALTH: u16 = 60_000;

/// What slot an ability may be put in (MATRIX.md 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Slot {
    Primary = 0,
    Secondary = 1,
    Guard = 2,
    Active = 3,
    /// Never slotted by a build: it comes with another ability (the stages of a chain,
    /// MODES.md 4.3) or with a mode (the knife of a gun build, MODES.md 3.7).
    Extra = 4,
}

/// An ability as content ships it: the verb script plus its slot and gating.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct AbilityDef {
    /// Stable content key (`"hammer"`), unique within a pack.
    pub key: String,
    pub ability: Ability,
    pub slot: Slot,
    /// Required aspect; also the element of every elemental packet in the script.
    pub aspect: Option<Element>,
    /// Squad slots the ability adds while slotted (COMPANIONS.md 3.2).
    pub squad: u8,
    /// Only a creature may slot it (COMPANIONS.md 8.1).
    pub creature: bool,
    /// A primary an FPS build may hold beside a firearm (MODES.md 2, BODIES.md 6.7 and
    /// 7): the empty hands of a body (the knife, the fists, the magic blows).
    pub gun_melee: bool,
}

impl Mode {
    /// Whether a build of this mode holds the ability `def` (MODES.md 2, GEAR.md 5.1): an
    /// FPS build's hands take what shoots (a firearm, a crossbow, a bow, a bolt-throwing
    /// wand) and the body's own melee (`gun_melee`); an action or RPG build's take
    /// anything but a firearm.
    pub fn holds(self, def: &AbilityDef) -> bool {
        match self {
            Mode::Fps => def.ability.shoots() || def.gun_melee,
            Mode::Action | Mode::Rpg => def.ability.firearm.is_none(),
        }
    }
}

/// What a boss drops (COMPANIONS.md 10): `components` items per kill, drawn in order from
/// `standard` for a party that brought companions and from `top` for a party of humans only.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct Loot {
    pub components: u8,
    pub standard: Vec<String>,
    pub top: Vec<String>,
    /// Silver, split among the recipients.
    pub coin: u32,
}

/// A creature (COMPANIONS.md 8.1): a build without a budget, with its health set by content.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct CreatureDef {
    pub key: String,
    pub name: String,
    /// Frame, attributes, armour class, aspects and kit: the body is read by the same rules
    /// as a player's.
    pub build: Build,
    pub health: u16,
    /// 0 = the derived threshold (MATRIX.md 6).
    pub stagger_threshold: u16,
    /// How far it perceives, and how far from its post it goes.
    pub sight: f32,
    pub leash: f32,
    pub boss: bool,
    /// Seconds after its encounter was cleared until it stands on its post again; 0 = never.
    pub respawn_s: u16,
    pub loot: Option<Loot>,
    /// Stands on its post and does nothing, whatever is done to it (the training dummy):
    /// no mind drives it.
    pub still: bool,
    /// Somebody of the town (the trainer): cannot be hurt, is spoken to with `E`.
    pub npc: bool,
    /// What an `npc` is for, by name (`trainer`: a build is worn beside it, MATRIX.md 9.1;
    /// `smith`: sells the materials and keeps the forge, ITEMS.md 6.6); empty for nobody in
    /// particular. The zone and the client read the same word.
    pub role: String,
    /// What an npc wears for its look alone (ITEMS.md 6.6: the smith's cuirass): a
    /// template a body place in the wire's order, chest, head, legs; empty for nothing.
    pub wears: [String; 3],
    /// What its blows and spells are multiplied by beyond its attributes (1 = a player's
    /// numbers): a creature's health is set apart from the bands, and so is its damage.
    pub might: f32,
}

/// A character build (MATRIX.md 9). Ability references are indices into the content pack.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Build {
    /// The game the character plays (MODES.md 2). Builds stored before it was there are
    /// read as `Action`, today's play.
    #[cfg_attr(feature = "serde", serde(default))]
    pub mode: Mode,
    /// The body (BODIES.md 3): an index into the pack's `bodies`, which fixes the frame,
    /// the attribute floors and the first active (the body skill). `None` for a preset,
    /// a creature and a bot: a build of the flat floor, as every build was before
    /// 2026-10-19.
    #[cfg_attr(feature = "serde", serde(default))]
    pub body: Option<u16>,
    /// The body's frame when there is a body; the build's own otherwise.
    pub frame: ArchetypeFrame,
    pub attributes: Attributes,
    pub aspects: Aspects,
    /// What plays in nothing (GEAR.md 2): the worn weapon brings its own in place of it.
    /// With a body these three are the bare hands' (`bare_hands`).
    pub primary: u16,
    pub secondary: u16,
    pub guard: Option<u16>,
    /// The actives beside the body skill, which stands before them with a body (so at
    /// most `MAX_ACTIVES - 1` here). A player's body has none since 2026-10-20 (MATRIX.md
    /// 20: nothing is bought); the presets of the bots and the creatures keep theirs.
    pub actives: Vec<u16>,
}

/// How a body attacks (BODIES.md 7, 2026-10-20): what its empty hands do, and what a
/// weapon upgrades. The melee group's hands are fists, the magic group's a weak bolt of
/// its element (force without one), the ranged group's the knife, until a weapon is
/// held. Of a build without a body (a preset, a creature) it is the mode's: FPS ranged,
/// RPG magic, action melee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Group {
    Melee,
    Magic,
    Ranged,
}

impl Group {
    pub const fn name(self) -> &'static str {
        match self {
            Group::Melee => "melee",
            Group::Magic => "magic",
            Group::Ranged => "ranged",
        }
    }

    /// A bodiless build's group: the mode's.
    pub const fn of_mode(mode: Mode) -> Group {
        match mode {
            Mode::Fps => Group::Ranged,
            Mode::Rpg => Group::Magic,
            Mode::Action => Group::Melee,
        }
    }

    pub fn from_name(s: &str) -> Option<Group> {
        [Group::Melee, Group::Magic, Group::Ranged]
            .into_iter()
            .find(|g| g.name() == s)
    }
}

/// A body's group (BODIES.md 7): its own, or `ByMode` (the Soldier: ranged in the FPS
/// mode, melee in the others).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub enum BodyGroup {
    Fixed(Group),
    ByMode,
}

impl BodyGroup {
    pub const fn in_mode(self, mode: Mode) -> Group {
        match self {
            BodyGroup::Fixed(g) => g,
            BodyGroup::ByMode => match mode {
                Mode::Fps => Group::Ranged,
                Mode::Action | Mode::Rpg => Group::Melee,
            },
        }
    }

    /// `melee`, `magic`, `ranged` or `by_mode`.
    pub fn from_name(s: &str) -> Option<BodyGroup> {
        if s == "by_mode" {
            return Some(BodyGroup::ByMode);
        }
        Group::from_name(s).map(BodyGroup::Fixed)
    }
}

/// A body (BODIES.md 3): who the character is. Its frame is the hitbox and the gait, its
/// floors are where the thirty points start, its skill is the first active, free, and
/// its stamina is the pool's share (the whelp's youth). A player picks one at creation
/// and keeps it; everything else is respecced at the trainer.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct BodyDef {
    /// Stable content key (`"brute"`), unique within a pack.
    pub key: String,
    pub name: String,
    pub frame: ArchetypeFrame,
    /// How it attacks (BODIES.md 7): what its empty hands do.
    pub group: BodyGroup,
    /// The five floors, each `Attributes::MIN..=MAX`, summing to `Attributes::FLOOR_SUM`.
    pub floors: Attributes,
    /// The body skill: an `active` of no aspect and no creature's, the first active of
    /// every build of this body at no cost.
    pub skill: u16,
    /// The stamina pool, percent of the derived one (100 for most; the whelp 115).
    pub stamina: u8,
    /// What the body is, in a line or two of plain words (at most 160 ASCII characters).
    pub blurb: String,
}

/// What a body wears, as its sheet reads it (GEAR.md 2 and 4): the kit the worn things
/// bring, as indices into the content pack. The zone derives it from the worn templates
/// with the item content (`gm_content::worn`) and tells the owning client, so that
/// neither the simulation nor a client reads `items.toml`, and both build the one sheet.
/// `NONE` is a body in nothing: the build's own kit, as before gear brought any.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Worn {
    /// The primary the weapon in the right hand brings (GEAR.md 2), in place of the
    /// build's; `None`: the hand is empty, the build's primary plays.
    pub primary: Option<u16>,
    /// The secondary the hands bring (BODIES.md 3.4 and 3.6): the left hand's own where
    /// it has one (a shield's bash, the pistol, a focus's mend), else the right hand's
    /// weapon's second move (a caster weapon's blast of the body's element, a hammer's
    /// slam, a sword's pommel); `None`: the build's secondary plays (the kick).
    pub secondary: Option<u16>,
    /// The guard the thing in the left hand brings (a shield's wall, a focus's brace), in
    /// place of the build's; `None`: the build's guard, or none.
    pub guard: Option<u16>,
    /// The spell the weapon in the right hand brings (BODIES.md 7.2): a staff's or a
    /// wand's of the body's element, the active after the body skill; `None`: nothing.
    pub spell: Option<u16>,
    /// The techniques of the weapon in the right hand (BODIES.md 3.8): every primary that
    /// holds its prop and the build may use, the one in `primary` among them; the K page
    /// offers them, and the build's stored primary picks one when it is one of them.
    pub techniques: Vec<u16>,
    /// The armour's base mitigation (BODIES.md 3.7): what the worn pieces add to the
    /// body's armour and ward before the caps (the chest whole, the head and the legs a
    /// third), and what they do to its speed and stamina regen (percent; 100 for nothing).
    pub armour: f32,
    pub ward: f32,
    pub speed: u8,
    pub regen: u8,
    /// The worn weapon's frame (GEAR.md 6): the stamina of the blows, percent of the cost.
    /// The client predicts stamina, so it rides here, not in the zone's gear alone.
    pub stamina_blows: u8,
    /// The guard pieces' frames: the stamina of the guard and of a block, percent.
    pub stamina_block: u8,
    /// A shield's own block (GEAR.md 7), played in place of the guard ability's numbers
    /// when the guard is the shield's.
    pub block: Option<crate::vocab::Block>,
}

impl Worn {
    pub const NONE: Worn = Worn {
        primary: None,
        secondary: None,
        guard: None,
        spell: None,
        techniques: Vec::new(),
        armour: 0.0,
        ward: 0.0,
        speed: 100,
        regen: 100,
        stamina_blows: 100,
        stamina_block: 100,
        block: None,
    };
}

/// A build with the name content gave it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedBuild {
    pub name: String,
    pub build: Build,
}

/// Everything a zone loaded from `assets/content` and sends to its clients (MATRIX.md 10).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct ContentPack {
    pub abilities: Vec<AbilityDef>,
    pub builds: Vec<NamedBuild>,
    pub creatures: Vec<CreatureDef>,
    pub trials: Vec<crate::trial::TrialDef>,
    /// The bodies a player chooses from (BODIES.md 3); empty for a pack of presets only.
    pub bodies: Vec<BodyDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    AttributeOutOfRange,
    AttributePoints {
        spent: u32,
    },
    TooManyAspects,
    UnknownAbility(u16),
    WrongSlot {
        index: u16,
        expected: Slot,
    },
    DuplicateAbility(u16),
    MissingAspect {
        index: u16,
        needs: Element,
    },
    SharedCooldownGroup(u8),
    TooManyActives,
    CreatureOnly(u16),
    /// A chain stage or a mode's extra, slotted by hand (MODES.md 4.3).
    NotSlottable(u16),
    /// An FPS build's primary does not shoot (MODES.md 2, GEAR.md 5.1).
    GunNeedsRanged,
    /// A firearm in a build of another mode.
    FirearmNeedsGun(u16),
    /// A gun build with a guard (MODES.md 3.7).
    GunHasNoGuard,
    /// A body the pack does not have (BODIES.md 3).
    UnknownBody(u16),
    /// The frame is not the body's.
    BodyFrame,
    /// An attribute under its body's floor.
    AttributeUnderFloor,
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuildError::AttributeOutOfRange => write!(
                f,
                "attributes must be {}..={}",
                Attributes::MIN,
                Attributes::MAX
            ),
            BuildError::TooManyAspects => write!(f, "at most two aspects"),
            BuildError::UnknownAbility(i) => write!(f, "unknown ability {i}"),
            BuildError::WrongSlot { index, expected } => {
                write!(f, "ability {index} does not fit the {expected:?} slot")
            }
            BuildError::DuplicateAbility(i) => write!(f, "ability {i} slotted twice"),
            BuildError::NotSlottable(i) => {
                write!(
                    f,
                    "ability {i} comes with another and is not slotted by hand"
                )
            }
            BuildError::GunNeedsRanged => {
                write!(f, "an FPS build's primary shoots: a firearm or a bow")
            }
            BuildError::FirearmNeedsGun(i) => {
                write!(f, "ability {i} is a firearm: only a gun build holds one")
            }
            BuildError::GunHasNoGuard => write!(f, "a gun build has no guard"),
            BuildError::MissingAspect { index, needs } => {
                write!(f, "ability {index} needs the {} aspect", needs.name())
            }
            BuildError::SharedCooldownGroup(g) => {
                write!(f, "two abilities share cooldown group {g}")
            }
            BuildError::TooManyActives => write!(f, "at most {MAX_ACTIVES} actives"),
            BuildError::AttributePoints { spent } => write!(
                f,
                "attributes take {spent} of {} points",
                Attributes::FREE_POINTS
            ),
            BuildError::CreatureOnly(i) => write!(f, "ability {i} is a creature's"),
            BuildError::UnknownBody(i) => write!(f, "unknown body {i}"),
            BuildError::BodyFrame => write!(f, "the frame is the body's"),
            BuildError::AttributeUnderFloor => write!(f, "an attribute is under its body's floor"),
        }
    }
}

impl core::error::Error for BuildError {}

#[derive(Debug, PartialEq, Eq)]
pub struct ContentError {
    pub key: String,
    pub reason: String,
}

impl core::fmt::Display for ContentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "content {}: {}", self.key, self.reason)
    }
}

impl core::error::Error for ContentError {}

impl Build {
    /// Every slotted ability index in kit order: primary, secondary, actives, guard.
    pub fn slots(&self) -> Vec<(u16, Slot)> {
        self.kit_slots(&Worn::NONE)
    }

    /// The body of this build, when the pack has it.
    pub fn body_def<'p>(&self, pack: &'p ContentPack) -> Option<&'p BodyDef> {
        pack.bodies.get(self.body? as usize)
    }

    /// The body skill (BODIES.md 3.4): the first active of a build with a body.
    pub fn body_skill(&self, pack: &ContentPack) -> Option<u16> {
        self.body_def(pack).map(|b| b.skill)
    }

    /// How this build attacks (BODIES.md 7): the body's group in this mode, else the
    /// mode's.
    pub fn group(&self, pack: &ContentPack) -> Group {
        self.body_def(pack)
            .map_or(Group::of_mode(self.mode), |b| b.group.in_mode(self.mode))
    }

    /// The bare hands of this build (BODIES.md 7): what its group plays in nothing.
    pub fn bare_hands(&self, pack: &ContentPack) -> (u16, u16, Option<u16>) {
        pack.bare_hands(self.group(pack), self.aspects, self.mode)
    }

    /// Where the thirty points start: the body's floors, or five each without one.
    pub fn floors(&self, pack: &ContentPack) -> Attributes {
        self.body_def(pack)
            .map_or(Attributes::flat(Attributes::BASE), |b| b.floors)
    }

    /// The attribute points spent above the floors (MATRIX.md 2).
    pub fn points_spent(&self, pack: &ContentPack) -> u32 {
        self.attributes.spent(self.floors(pack))
    }

    /// The derived stats with the body's say (BODIES.md 3.1: the stamina pool's share).
    pub fn derived_in(&self, pack: &ContentPack) -> Derived {
        let mut d = self.derived();
        if let Some(b) = self.body_def(pack) {
            d.stamina *= b.stamina as f32 / 100.0;
        }
        d
    }

    /// The derived stats of this build wearing `worn` (BODIES.md 3.7): the armour's base
    /// mitigation added before the caps, its weight on the speed and the stamina regen.
    pub fn derived_wearing(&self, worn: &Worn, pack: &ContentPack) -> Derived {
        let mut d = self.derived_in(pack);
        d.armour = (d.armour + worn.armour).clamp(0.0, crate::matrix::MAX_ARMOUR);
        d.ward = (d.ward + worn.ward).clamp(0.0, crate::matrix::MAX_WARD);
        d.max_speed *= worn.speed.clamp(1, 200) as f32 / 100.0;
        d.stamina_regen *= worn.regen.clamp(1, 200) as f32 / 100.0;
        d
    }

    /// The slots as the kit plays them with `worn` (GEAR.md 2): the worn weapon's primary
    /// in place of the build's, the off hand's guard in place of the build's (or added,
    /// when the build has none). Nothing worn: `slots()`. The body skill, when there is a
    /// body (`body_skill`, told apart from the pack: `kit_slots_in`), is the first
    /// active; this reading has no pack and lists the build's own actives.
    pub fn kit_slots(&self, worn: &Worn) -> Vec<(u16, Slot)> {
        self.kit_slots_with(worn, None)
    }

    /// `kit_slots` with the body skill first among the actives (BODIES.md 3.4).
    pub fn kit_slots_in(&self, worn: &Worn, pack: &ContentPack) -> Vec<(u16, Slot)> {
        self.kit_slots_with(worn, self.body_skill(pack))
    }

    fn kit_slots_with(&self, worn: &Worn, skill: Option<u16>) -> Vec<(u16, Slot)> {
        let mut out = vec![
            (worn.primary.unwrap_or(self.primary), Slot::Primary),
            (worn.secondary.unwrap_or(self.secondary), Slot::Secondary),
        ];
        out.extend(skill.map(|i| (i, Slot::Active)));
        // The weapon's spell (BODIES.md 7.2), after the body skill; a preset that holds
        // it among its own actives (the arena's frostweaver, the nova) has it once.
        out.extend(
            worn.spell
                .filter(|i| !self.actives.contains(i))
                .map(|i| (i, Slot::Active)),
        );
        out.extend(self.actives.iter().map(|&i| (i, Slot::Active)));
        if let Some(g) = worn.guard.or(self.guard) {
            out.push((g, Slot::Guard));
        }
        out
    }

    /// MATRIX.md 9: ranges, the free points, aspects, slots, gating, cooldown groups; with
    /// a body (BODIES.md 3.2), the body's frame, its floors and the
    /// points above them.
    pub fn validate(&self, pack: &ContentPack) -> Result<(), BuildError> {
        if !self.attributes.in_range() {
            return Err(BuildError::AttributeOutOfRange);
        }
        if let Some(i) = self.body {
            let body = pack
                .bodies
                .get(i as usize)
                .ok_or(BuildError::UnknownBody(i))?;
            if self.frame != body.frame {
                return Err(BuildError::BodyFrame);
            }
            if !self.attributes.at_floors(body.floors) {
                return Err(BuildError::AttributeUnderFloor);
            }
            if self.actives.len() + 1 > MAX_ACTIVES {
                return Err(BuildError::TooManyActives);
            }
        }
        let points = self.points_spent(pack);
        if points > Attributes::FREE_POINTS {
            return Err(BuildError::AttributePoints { spent: points });
        }
        self.check_kit(pack, false)
    }

    /// A build of a body (BODIES.md 3.5): the body's frame, its floors lifted to where
    /// `attributes` puts them, the aspects and the actives asked for, the bare hands for
    /// what plays in nothing. Validated by the caller.
    pub fn of_body(
        pack: &ContentPack,
        body: u16,
        mode: Mode,
        attributes: Attributes,
        aspects: Aspects,
        actives: Vec<u16>,
    ) -> Result<Build, BuildError> {
        let def = pack
            .bodies
            .get(body as usize)
            .ok_or(BuildError::UnknownBody(body))?;
        let (primary, secondary, guard) = pack.bare_hands(def.group.in_mode(mode), aspects, mode);
        Ok(Build {
            mode,
            body: Some(body),
            frame: def.frame,
            attributes,
            aspects,
            primary,
            secondary,
            guard,
            actives,
        })
    }

    /// A stored build the content no longer takes, made into one it does (MATRIX.md 9.1):
    /// attributes clamped to the range and, past the free points, scaled down to them; a
    /// kit the pack refuses replaced by the preset with the same primary, else the first
    /// preset. `None` when the build is fine as it is, or nothing can be made of it.
    pub fn repaired(&self, pack: &ContentPack) -> Option<Build> {
        if self.validate(pack).is_ok() {
            return None;
        }
        let mut out = self.clone();
        // A body the pack lost is no body: the build reads as a preset's would.
        if out
            .body
            .is_some_and(|i| pack.bodies.get(i as usize).is_none())
        {
            out.body = None;
        }
        if let Some(body) = out.body_def(pack) {
            out.frame = body.frame;
            if out.actives.len() + 1 > MAX_ACTIVES {
                out.actives.truncate(MAX_ACTIVES - 1);
            }
        }
        // Lifted to the floors (BODIES.md 3.2) and clamped to the range; past the free
        // points, scaled down to them.
        let floors = out.floors(pack).as_array();
        let mut attrs = out.attributes.as_array();
        for (a, f) in attrs.iter_mut().zip(floors) {
            *a = (*a).clamp(f, Attributes::MAX);
        }
        let spent: u32 = attrs.iter().zip(floors).map(|(&v, f)| (v - f) as u32).sum();
        if spent > Attributes::FREE_POINTS {
            // Each stat keeps its share of the points, rounded down; the remainder goes to
            // the largest ones, one each, so that every point is kept.
            let mut kept: Vec<u32> = attrs
                .iter()
                .zip(floors)
                .map(|(&v, f)| (v - f) as u32 * Attributes::FREE_POINTS / spent)
                .collect();
            let mut left = Attributes::FREE_POINTS - kept.iter().sum::<u32>();
            let mut order: Vec<usize> = (0..5).collect();
            order.sort_by_key(|&i| core::cmp::Reverse(attrs[i]));
            for i in order {
                if left == 0 {
                    break;
                }
                kept[i] += 1;
                left -= 1;
            }
            for ((a, k), f) in attrs.iter_mut().zip(kept).zip(floors) {
                *a = f + k as u8;
            }
        }
        let [str_, agi, con, int, spr] = attrs;
        out.attributes = Attributes::new(str_, agi, con, int, spr);
        if out.validate(pack).is_ok() {
            return Some(out);
        }
        // A kit the pack refuses: with a body, the bare hands and no spells; without
        // one, the preset with the same primary, else the first preset.
        if let Some(body) = out.body {
            let fixed = Build::of_body(
                pack,
                body,
                out.mode,
                out.attributes,
                Aspects(out.aspects.0 & Element::MASK),
                Vec::new(),
            )
            .ok()?;
            return fixed.validate(pack).is_ok().then_some(fixed);
        }
        let preset = pack
            .builds
            .iter()
            .find(|nb| nb.build.primary == self.primary)
            .or_else(|| pack.builds.first())?;
        out = Build {
            attributes: out.attributes,
            ..preset.build.clone()
        };
        out.validate(pack).is_ok().then_some(out)
    }

    /// A creature's build (COMPANIONS.md 8.1): the kit rules without the budget and without
    /// the attribute range, and creature abilities allowed.
    pub fn validate_creature(&self, pack: &ContentPack) -> Result<(), BuildError> {
        self.check_kit(pack, true)
    }

    /// Companions this build commands (COMPANIONS.md 3.2): three, plus the slots its
    /// abilities add, at most five.
    pub fn squad_capacity(&self, pack: &ContentPack) -> usize {
        let bonus: usize = self
            .kit_slots_in(&Worn::NONE, pack)
            .iter()
            .filter_map(|(i, _)| pack.abilities.get(*i as usize))
            .map(|a| a.squad as usize)
            .sum();
        (SQUAD_BASE + bonus).min(SQUAD_MAX)
    }

    /// Aspects, slot types, duplicates, aspect gating, cooldown groups.
    fn check_kit(&self, pack: &ContentPack, creature: bool) -> Result<(), BuildError> {
        if self.aspects.0 & !Element::MASK != 0 {
            return Err(BuildError::TooManyAspects);
        }
        // None is neutral (MATRIX.md 5): 1x on every element, and nothing elemental slots.
        if self.aspects.count() > 2 {
            return Err(BuildError::TooManyAspects);
        }
        if self.actives.len() > MAX_ACTIVES {
            return Err(BuildError::TooManyActives);
        }
        let mut seen: Vec<u16> = Vec::new();
        let mut groups: Vec<u8> = Vec::new();
        for (index, expected) in self.kit_slots_in(&Worn::NONE, pack) {
            let def = pack
                .abilities
                .get(index as usize)
                .ok_or(BuildError::UnknownAbility(index))?;
            if def.slot == Slot::Extra {
                return Err(BuildError::NotSlottable(index));
            }
            // A creature may hold a second primary (a bow beside its blade) where a
            // player holds a utility, and a second move (the staff's stomp, BODIES.md
            // 3.6) among its actives: the slot rules of MATRIX.md 10 are for players.
            let allowed = def.slot == expected
                || (creature && expected == Slot::Secondary && def.slot == Slot::Primary)
                || (creature && expected == Slot::Active && def.slot == Slot::Secondary);
            if !allowed {
                return Err(BuildError::WrongSlot { index, expected });
            }
            if def.creature && !creature {
                return Err(BuildError::CreatureOnly(index));
            }
            // The mode and the firearm go together (MODES.md 2); a creature's mind holds
            // what content gives it.
            if !creature {
                match (self.mode, expected) {
                    (Mode::Fps, Slot::Primary) if !self.mode.holds(def) => {
                        return Err(BuildError::GunNeedsRanged);
                    }
                    (Mode::Fps, Slot::Guard) => return Err(BuildError::GunHasNoGuard),
                    (Mode::Action | Mode::Rpg, _) if !self.mode.holds(def) => {
                        return Err(BuildError::FirearmNeedsGun(index));
                    }
                    _ => {}
                }
            }
            if seen.contains(&index) {
                return Err(BuildError::DuplicateAbility(index));
            }
            seen.push(index);
            if let Some(needs) = def.aspect
                && !self.aspects.contains(needs)
            {
                return Err(BuildError::MissingAspect { index, needs });
            }
            if let Some(g) = def.ability.cooldown.group {
                if groups.contains(&g) {
                    return Err(BuildError::SharedCooldownGroup(g));
                }
                groups.push(g);
            }
        }
        Ok(())
    }

    pub fn derived(&self) -> Derived {
        Derived::compute(self.attributes, self.frame)
    }
}

impl ContentPack {
    pub fn find(&self, key: &str) -> Option<u16> {
        self.abilities
            .iter()
            .position(|a| a.key == key)
            .map(|i| i as u16)
    }

    pub fn build(&self, name: &str) -> Option<&Build> {
        self.builds
            .iter()
            .find(|b| b.name == name)
            .map(|b| &b.build)
    }

    /// A body's index by its key (BODIES.md 3).
    pub fn body(&self, key: &str) -> Option<u16> {
        self.bodies
            .iter()
            .position(|b| b.key == key)
            .map(|i| i as u16)
    }

    /// What a body of `group` with `aspects` plays in nothing in `mode` (BODIES.md 7):
    /// the fists (melee), the bolt of the first element it holds or of force (magic),
    /// the knife (ranged); the kick; the parry (none in FPS). A pack without them (a
    /// fixture) falls back to the first plain primary the mode holds (a firearm for FPS,
    /// anything else otherwise). The worn things bring their own over these.
    pub fn bare_hands(
        &self,
        group: Group,
        aspects: Aspects,
        mode: Mode,
    ) -> (u16, u16, Option<u16>) {
        let plain = |slot: Slot, want_firearm: Option<bool>| -> Option<u16> {
            self.abilities
                .iter()
                .enumerate()
                .filter(|(_, d)| d.slot == slot && !d.creature && d.aspect.is_none())
                .filter(|(_, d)| {
                    want_firearm.is_none_or(|want| d.ability.firearm.is_some() == want)
                })
                .map(|(i, _)| i as u16)
                .next()
        };
        let gun = mode == Mode::Fps;
        let hands = match group {
            Group::Melee => self.find("fists"),
            Group::Ranged => self.find("knife_hands"),
            Group::Magic => aspects
                .iter()
                .find_map(|e| self.find(&format!("{}_blow", e.name())))
                .or_else(|| self.find("force_blow")),
        };
        let primary = hands
            .or_else(|| plain(Slot::Primary, Some(gun)))
            .unwrap_or(0);
        let secondary = self
            .find("kick")
            .or_else(|| plain(Slot::Secondary, Some(false)))
            .unwrap_or(0);
        let guard = if gun {
            None
        } else {
            self.find("parry").or_else(|| plain(Slot::Guard, None))
        };
        (primary, secondary, guard)
    }

    /// Every ability validates against VOCABULARY.md 11, keys are unique, elemental packets
    /// match the ability's aspect, guard abilities are exactly one `Guard` step, and every
    /// preset build validates.
    pub fn validate(&self, rate: TickRate) -> Result<(), ContentError> {
        for (i, def) in self.abilities.iter().enumerate() {
            let err = |reason: String| ContentError {
                key: def.key.clone(),
                reason,
            };
            if def.key.is_empty() || self.abilities[..i].iter().any(|o| o.key == def.key) {
                return Err(err("duplicate or empty key".into()));
            }
            def.ability
                .validate(rate)
                .map_err(|e| err(e.reason.to_string()))?;
            let is_guard =
                def.ability.steps.len() == 1 && matches!(def.ability.steps[0].verb, Verb::Guard(_));
            if (def.slot == Slot::Guard) != is_guard {
                return Err(err(
                    "guard slot abilities are exactly one Guard step, and only those".into(),
                ));
            }
            // A chain (MODES.md 4.3) names a stage that exists, and never comes round.
            let mut at = i;
            let mut hops = 0;
            while let Some(c) = self.abilities[at].ability.chain {
                let next = c.next as usize;
                if next >= self.abilities.len() {
                    return Err(err(format!(
                        "chain names ability {next}, which is not there"
                    )));
                }
                if self.abilities[next].slot != Slot::Extra {
                    return Err(err("a chain's next stage is an extra".into()));
                }
                hops += 1;
                if hops > crate::sim::MAX_ABILITIES || next == i {
                    return Err(err("a chain comes round to itself".into()));
                }
                at = next;
            }
            if def.ability.firearm.is_some() && !matches!(def.slot, Slot::Primary | Slot::Secondary)
            {
                return Err(err("a firearm is a primary or a secondary".into()));
            }
            let mut types = Vec::new();
            collect_types(&def.ability, &mut types);
            for t in types {
                match (t.element(), def.aspect) {
                    (Some(e), Some(a)) if e != a => {
                        return Err(err(format!(
                            "{} packet in a {} ability",
                            t.name(),
                            a.name()
                        )));
                    }
                    (Some(e), None) => {
                        return Err(err(format!(
                            "{} packet in an ability without an aspect",
                            e.name()
                        )));
                    }
                    _ => {}
                }
            }
        }
        for nb in &self.builds {
            nb.build.validate(self).map_err(|e| ContentError {
                key: nb.name.clone(),
                reason: e.to_string(),
            })?;
        }
        for (i, b) in self.bodies.iter().enumerate() {
            let err = |reason: String| ContentError {
                key: b.key.clone(),
                reason,
            };
            if b.key.is_empty() || self.bodies[..i].iter().any(|o| o.key == b.key) {
                return Err(err("duplicate or empty body key".into()));
            }
            if !b.floors.in_range() {
                return Err(err(format!(
                    "floors must be {}..={}",
                    Attributes::MIN,
                    Attributes::MAX
                )));
            }
            if b.floors.sum() != Attributes::FLOOR_SUM {
                return Err(err(format!(
                    "floors sum to {}, not {}",
                    b.floors.sum(),
                    Attributes::FLOOR_SUM
                )));
            }
            let skill = self
                .abilities
                .get(b.skill as usize)
                .ok_or_else(|| err(format!("unknown skill {}", b.skill)))?;
            if skill.slot != Slot::Active || skill.creature || skill.aspect.is_some() {
                return Err(err(format!(
                    "skill {:?} is not a plain active (a body skill has no aspect)",
                    skill.key
                )));
            }
            if b.stamina == 0 {
                return Err(err("stamina is a percent of the pool, above zero".into()));
            }
            if b.name.is_empty() {
                return Err(err("a body has a name".into()));
            }
            // Every body makes a build of every mode: the bare hands exist and the body
            // validates with its floors and nothing bought.
            for mode in [Mode::Action, Mode::Rpg, Mode::Fps] {
                let build =
                    Build::of_body(self, i as u16, mode, b.floors, Aspects::NONE, Vec::new())
                        .map_err(|e| err(e.to_string()))?;
                build
                    .validate(self)
                    .map_err(|e| err(format!("in {} mode: {e}", mode.name())))?;
            }
        }
        for (i, c) in self.creatures.iter().enumerate() {
            let err = |reason: String| ContentError {
                key: c.key.clone(),
                reason,
            };
            if c.key.is_empty() || self.creatures[..i].iter().any(|o| o.key == c.key) {
                return Err(err("duplicate or empty creature key".into()));
            }
            c.build
                .validate_creature(self)
                .map_err(|e| err(e.to_string()))?;
            if c.health == 0 || c.health > MAX_CREATURE_HEALTH {
                return Err(err(format!("health must be 1..={MAX_CREATURE_HEALTH}")));
            }
            // One that stands still has no sight and no leash to speak of.
            if !c.still && !(c.sight > 0.0 && c.leash > 0.0) {
                return Err(err("sight and leash must be positive".into()));
            }
            match &c.loot {
                Some(_) if !c.boss => return Err(err("only a boss drops (no junk loot)".into())),
                Some(l) if l.components > 0 && (l.standard.is_empty() || l.top.is_empty()) => {
                    return Err(err("loot needs a standard and a top list".into()));
                }
                _ => {}
            }
        }
        for (i, t) in self.trials.iter().enumerate() {
            let err = |reason: &str| ContentError {
                key: t.key.clone(),
                reason: reason.into(),
            };
            if t.key.is_empty() || self.trials[..i].iter().any(|o| o.key == t.key) {
                return Err(err("duplicate or empty trial key"));
            }
            if t.map.is_empty() || t.encounter.is_empty() {
                return Err(err("a trial names its map and its encounter"));
            }
            let l = &t.lens;
            if [l.damage, l.tank, l.healing, l.command]
                .iter()
                .any(|&v| v > 1000)
            {
                return Err(err("lens shares are per mille (0..=1000)"));
            }
        }
        Ok(())
    }

    /// A creature definition and its index (what `Roster` entries name).
    pub fn creature(&self, key: &str) -> Option<(u16, &CreatureDef)> {
        self.creatures
            .iter()
            .position(|c| c.key == key)
            .map(|i| (i as u16, &self.creatures[i]))
    }
}

fn collect_types(a: &Ability, out: &mut Vec<DamageType>) {
    let area = |ae: &crate::vocab::AreaEffect, out: &mut Vec<DamageType>| {
        if let Some(d) = ae.damage {
            out.push(d.dtype);
        }
    };
    for s in &a.steps {
        match &s.verb {
            Verb::MeleeArc(m) => out.push(m.damage.dtype),
            Verb::Projectile(p) => {
                out.push(p.damage.dtype);
                for t in p.on_hit.iter().chain(&p.on_expire) {
                    if let Trigger::Area(ae) = t {
                        area(ae, out);
                    }
                }
            }
            Verb::AreaEffect(ae) => area(ae, out),
            Verb::Guard(Guard::Parry(p)) => {
                for r in &p.on_success {
                    if let Riposte::Swing(arc) = r {
                        out.push(arc.damage.dtype);
                    }
                }
            }
            Verb::ApplyStatus(_) | Verb::MoveSelf(_) | Verb::Guard(Guard::Block(_)) => {}
        }
    }
}

/// The abilities in play for one character, in kit order: primary, secondary, actives,
/// guard. This is what [`crate::sim::step_mover`] runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    pub abilities: Vec<Ability>,
    /// The mode the kit is played in (MODES.md 2).
    pub mode: Mode,
    /// Ticks a script occupies the actor (windup + active + recovery of its verbs).
    pub durations: Vec<Tick>,
    /// Elemental abilities are refused under `Silence`.
    pub elemental: Vec<bool>,
    pub primary: Option<u8>,
    pub secondary: Option<u8>,
    pub actives: [Option<u8>; MAX_ACTIVES],
    pub guard: Option<u8>,
    /// Per kit ability, the kit index of its chain's next stage (MODES.md 4.3).
    pub chain_next: Vec<Option<u8>>,
    /// The knife of a gun build (MODES.md 3.7): the pack's `knife`, when it has one.
    pub knife: Option<u8>,
    /// GEAR.md 6: the stamina of the blows and of the guard, percent of the cost (the worn
    /// frames; 100 in nothing).
    pub stamina_blows: u8,
    pub stamina_block: u8,
}

impl Kit {
    /// Compile a validated build against its pack.
    pub fn from_build(build: &Build, worn: &Worn, pack: &ContentPack) -> Kit {
        let mut kit = Kit::empty();
        kit.mode = build.mode;
        kit.stamina_blows = worn.stamina_blows.clamp(1, 100);
        kit.stamina_block = worn.stamina_block.clamp(1, 100);
        // (pack index, kit index) of everything pushed: a chain's stages are shared.
        let mut pushed: Vec<(u16, u8)> = Vec::new();
        for (index, slot) in build.kit_slots_in(worn, pack) {
            // A worn index the pack does not know (a zone and a client on different
            // content: never, by the version check) plays as the build's.
            let Some(def) = pack.abilities.get(index as usize) else {
                continue;
            };
            let i = kit.push(def.ability.clone(), def.aspect.is_some());
            pushed.push((index, i));
            match slot {
                Slot::Primary => kit.primary = Some(i),
                Slot::Secondary => kit.secondary = Some(i),
                Slot::Guard => kit.guard = Some(i),
                Slot::Active => {
                    if let Some(free) = kit.actives.iter_mut().find(|a| a.is_none()) {
                        *free = Some(i);
                    }
                }
                Slot::Extra => {}
            }
        }
        // A shield's own block (GEAR.md 7): the guard it brought plays the shield's numbers.
        if let (Some(g), Some(block), true) = (kit.guard, &worn.block, worn.guard.is_some()) {
            for step in &mut kit.abilities[g as usize].steps {
                if let crate::vocab::Verb::Guard(crate::vocab::Guard::Block(b)) = &mut step.verb {
                    *b = block.clone();
                }
            }
        }
        // The stages of every chain (MODES.md 4.3), then the knife of a gun (3.7): they
        // come with what was slotted and cost nothing.
        let mut k = 0;
        while k < kit.abilities.len() && kit.abilities.len() < crate::sim::MAX_ABILITIES {
            if let Some(chain) = kit.abilities[k].chain
                && let Some(def) = pack.abilities.get(chain.next as usize)
            {
                let next = match pushed.iter().find(|(p, _)| *p == chain.next) {
                    Some((_, i)) => *i,
                    None => {
                        let i = kit.push(def.ability.clone(), def.aspect.is_some());
                        pushed.push((chain.next, i));
                        i
                    }
                };
                kit.chain_next[k] = Some(next);
            }
            k += 1;
        }
        if build.mode == Mode::Fps
            && kit.abilities.len() < crate::sim::MAX_ABILITIES
            && let Some(index) = pack.find("knife")
            && let Some(def) = pack.abilities.get(index as usize)
        {
            kit.knife = Some(kit.push(def.ability.clone(), def.aspect.is_some()));
        }
        kit
    }

    pub fn empty() -> Kit {
        Kit {
            abilities: Vec::new(),
            mode: Mode::default(),
            durations: Vec::new(),
            elemental: Vec::new(),
            primary: None,
            secondary: None,
            actives: [None; MAX_ACTIVES],
            guard: None,
            chain_next: Vec::new(),
            knife: None,
            stamina_blows: 100,
            stamina_block: 100,
        }
    }

    /// What a blow of `ab` costs in stamina with the worn weapon's frame (GEAR.md 6).
    pub fn blow_cost(&self, ab: &Ability) -> f32 {
        ab.cost.stamina as f32 * self.stamina_blows as f32 / 100.0
    }

    /// What the guard costs in stamina with the guard pieces' frames (GEAR.md 6).
    pub fn guard_cost(&self, ab: &Ability) -> f32 {
        ab.cost.stamina as f32 * self.stamina_block as f32 / 100.0
    }

    /// Append an ability and return its index.
    pub fn push(&mut self, ability: Ability, elemental: bool) -> u8 {
        self.durations.push(script_duration(&ability));
        self.elemental.push(elemental);
        self.chain_next.push(None);
        self.abilities.push(ability);
        (self.abilities.len() - 1) as u8
    }

    /// The guard verb, if the kit has one.
    pub fn guard_verb(&self) -> Option<&Guard> {
        let i = self.guard? as usize;
        match &self.abilities[i].steps[0].verb {
            Verb::Guard(g) => Some(g),
            _ => None,
        }
    }
}

fn verb_duration(v: &Verb) -> Tick {
    match v {
        Verb::MeleeArc(m) => m.timing.total(),
        Verb::MoveSelf(MoveSelf {
            kind: MoveKind::Dash { duration, .. } | MoveKind::Charge { duration, .. },
            ..
        }) => *duration,
        _ => 0,
    }
}

/// The tick of a script after which it is in its recovery (MODES.md 4.3, 4.4): the end of
/// the last active window of its arcs, or the moment of its last step.
pub fn script_commit(a: &Ability) -> Tick {
    a.steps
        .iter()
        .map(|s| match &s.verb {
            Verb::MeleeArc(m) => s.at + m.timing.windup + m.timing.active,
            other => s.at + verb_duration(other),
        })
        .max()
        .unwrap_or(0)
}

pub fn script_duration(a: &Ability) -> Tick {
    a.steps
        .iter()
        .map(|s| s.at + verb_duration(&s.verb))
        .max()
        .unwrap_or(0)
        .max(1)
}

/// Everything the simulation needs per character.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub build: Build,
    /// What the kit was compiled with (GEAR.md 2): nothing, for a creature or a companion.
    pub worn: Worn,
    pub kit: Kit,
    pub derived: Derived,
    /// The body's group (BODIES.md 7.3), for the damage pipeline's factor; `None`
    /// without a body (a preset in nothing, a creature): no factor, and no affinity.
    pub group: Option<Group>,
    /// Build points left after the kit; informational.
    pub team: u8,
}

impl Sheet {
    /// A body in nothing: the build's own kit.
    pub fn new(build: Build, pack: &ContentPack, team: u8) -> Sheet {
        Sheet::wearing(build, Worn::NONE, pack, team)
    }

    /// A body wearing `worn` (GEAR.md 2): the kit the worn things bring over the build's.
    /// The zone and the owning client both build their sheet here, from the same two.
    pub fn wearing(build: Build, worn: Worn, pack: &ContentPack, team: u8) -> Sheet {
        Sheet {
            kit: Kit::from_build(&build, &worn, pack),
            derived: build.derived_wearing(&worn, pack),
            group: build.body.map(|_| build.group(pack)),
            build,
            worn,
            team,
        }
    }

    /// A creature's sheet (COMPANIONS.md 8.1): its build's kit and derived stats, with the
    /// health and the stagger threshold the definition sets.
    pub fn creature(def: &CreatureDef, pack: &ContentPack, team: u8) -> Sheet {
        let mut sheet = Sheet::new(def.build.clone(), pack, team);
        sheet.derived.health = def.health as i32;
        if def.stagger_threshold > 0 {
            sheet.derived.stagger_threshold = def.stagger_threshold as f32;
        }
        sheet.derived.physical_mult *= def.might;
        sheet.derived.elemental_mult *= def.might;
        sheet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::test_content;

    /// GEAR.md 5.1: an FPS build's hands take what shoots, a firearm or a bow, and the
    /// body's own melee; an action or RPG build's take anything but a firearm.
    #[test]
    fn the_fps_mode_holds_what_shoots() {
        let pack = test_content::pack(TickRate::COMBAT);
        let def = |key: &str| {
            let i = pack.find(key).unwrap_or_else(|| panic!("no {key}"));
            &pack.abilities[i as usize]
        };
        assert!(Mode::Fps.holds(def("musket")));
        assert!(Mode::Fps.holds(def("crossbow")), "a crossbow shoots");
        assert!(Mode::Fps.holds(def("bow")));
        assert!(!Mode::Fps.holds(def("sword")));
        assert!(Mode::Rpg.holds(def("crossbow")));
        assert!(Mode::Rpg.holds(def("sword")));
        assert!(!Mode::Action.holds(def("musket")), "a firearm is FPS only");
        let mut musketeer = pack.build("musketeer").expect("musketeer").clone();
        musketeer.primary = pack.find("crossbow").unwrap();
        assert!(
            !matches!(musketeer.validate(&pack), Err(BuildError::GunNeedsRanged)),
            "an FPS build takes a crossbow as its primary"
        );
        musketeer.primary = pack.find("sword").unwrap();
        assert_eq!(musketeer.validate(&pack), Err(BuildError::GunNeedsRanged));
    }

    #[test]
    fn presets_validate_and_spend_all_their_points() {
        let pack = test_content::pack(TickRate::COMBAT);
        pack.validate(TickRate::COMBAT).unwrap();
        for nb in &pack.builds {
            assert_eq!(
                nb.build.attributes.cost(),
                Attributes::FREE_POINTS,
                "{}",
                nb.name
            );
            let kit = Kit::from_build(&nb.build, &Worn::NONE, &pack);
            assert!(
                kit.primary.is_some() && kit.secondary.is_some(),
                "{}",
                nb.name
            );
        }
    }

    #[test]
    fn what_is_worn_plays_over_the_build_s_hands_and_guard() {
        let pack = test_content::pack(TickRate::COMBAT);
        let blade = pack.build("blade").unwrap().clone();
        let hammer = pack.find("hammer").unwrap();
        let wall = pack.find("shield_wall").unwrap();
        let bare = Sheet::new(blade.clone(), &pack, 0);
        let armed = Sheet::wearing(
            blade.clone(),
            Worn {
                primary: Some(hammer),
                guard: Some(wall),
                ..Worn::NONE
            },
            &pack,
            0,
        );
        // The hammer swings in place of the sword, the wall stands in place of the parry;
        // the rest of the kit is the build's, and the build itself is untouched.
        let key = |s: &Sheet, i: Option<u8>| s.kit.abilities[i.unwrap() as usize].name.clone();
        assert_eq!(key(&bare, bare.kit.primary), "Sword");
        assert_eq!(key(&armed, armed.kit.primary), "Hammer");
        assert_eq!(key(&bare, bare.kit.guard), "Parry");
        assert_eq!(key(&armed, armed.kit.guard), "Shield wall");
        assert_eq!(
            key(&armed, armed.kit.secondary),
            key(&bare, bare.kit.secondary)
        );
        assert_eq!(armed.build, blade);
        assert_eq!(blade.kit_slots(&Worn::NONE), blade.slots());
        // A guard brought to a build that has none is added, not dropped.
        let musketeer = pack.build("musketeer").unwrap().clone();
        assert!(musketeer.guard.is_none());
        let slots = musketeer.kit_slots(&Worn {
            guard: Some(wall),
            ..Worn::NONE
        });
        assert_eq!(slots.last(), Some(&(wall, Slot::Guard)));
    }

    #[test]
    fn a_shield_s_own_block_plays_in_place_of_the_wall_s() {
        use crate::vocab::{Block, Guard, Verb};
        let pack = test_content::pack(TickRate::COMBAT);
        let blade = pack.build("blade").unwrap().clone();
        let wall = pack.find("shield_wall").unwrap();
        let buckler = Block {
            arc_deg: 90.0,
            mitigation: 0.7,
            stamina_per_hit: 8,
            stops_projectiles: false,
            move_speed_scale: 0.8,
        };
        let block_of = |sheet: &Sheet| {
            let g = sheet.kit.guard.unwrap();
            sheet.kit.abilities[g as usize]
                .steps
                .iter()
                .find_map(|s| match &s.verb {
                    Verb::Guard(Guard::Block(b)) => Some(b.clone()),
                    _ => None,
                })
                .unwrap()
        };
        let plain = Sheet::wearing(
            blade.clone(),
            Worn {
                guard: Some(wall),
                ..Worn::NONE
            },
            &pack,
            0,
        );
        let with = Sheet::wearing(
            blade.clone(),
            Worn {
                guard: Some(wall),
                block: Some(buckler.clone()),
                ..Worn::NONE
            },
            &pack,
            0,
        );
        assert_ne!(block_of(&plain), buckler);
        assert_eq!(block_of(&with), buckler);
        // The pack itself is untouched: the next body gets the wall as content wrote it.
        assert_ne!(
            block_of(&Sheet::wearing(
                blade,
                Worn {
                    guard: Some(wall),
                    ..Worn::NONE
                },
                &pack,
                0
            )),
            buckler
        );
    }

    #[test]
    fn a_chest_piece_changes_no_stat_of_the_sheet() {
        // GEAR.md 12 (2026-10-17): an armour guards by its own numbers (`Gear::taken`),
        // and the body's speed, regen, ward and evasion are the frame's and the
        // attributes' whatever it wears. A body in nothing and one dressed read alike.
        let pack = test_content::pack(TickRate::COMBAT);
        let blade = pack.build("blade").unwrap().clone();
        let bare = Sheet::new(blade.clone(), &pack, 0);
        let dressed = Sheet::wearing(blade.clone(), Worn::NONE, &pack, 0);
        assert_eq!(bare.derived, dressed.derived);
        assert_eq!(
            bare.derived,
            Derived::compute(blade.attributes, blade.frame)
        );
    }

    #[test]
    fn an_old_build_is_repaired_into_one_the_pack_takes() {
        let pack = test_content::pack(TickRate::COMBAT);
        let base = pack.build("blade").unwrap().clone();
        assert_eq!(base.repaired(&pack), None);
        // Sixty-eight attribute points, as the v1 presets had: scaled to thirty, in shape.
        let mut b = base.clone();
        b.attributes = Attributes::new(20, 20, 20, 15, 18);
        let r = b.repaired(&pack).expect("repairable");
        assert_eq!(r.attributes.cost(), Attributes::FREE_POINTS);
        assert!(r.attributes.str_ >= r.attributes.int, "{:?}", r.attributes);
        assert_eq!(r.validate(&pack), Ok(()));
        assert_eq!((r.primary, &r.actives), (base.primary, &base.actives));
        // A kit the pack refuses: the preset with that primary, the attributes kept.
        let mut b = base.clone();
        b.secondary = pack.find("ice_shard").unwrap();
        // (On the Soldier's floors, BODIES.md 7.5: the repair lifts nothing.)
        b.attributes = Attributes::new(25, 10, 7, 3, 3);
        let r = b.repaired(&pack).expect("repairable");
        assert_eq!(r.attributes, b.attributes);
        assert_eq!(r.secondary, base.secondary);
        assert_eq!(r.validate(&pack), Ok(()));
    }

    #[test]
    fn validation_catches_every_rule() {
        let pack = test_content::pack(TickRate::COMBAT);
        let base = pack.build("blade").unwrap().clone();
        let mut b = base.clone();
        b.attributes.str_ = Attributes::MAX + 1;
        assert_eq!(b.validate(&pack), Err(BuildError::AttributeOutOfRange));
        let mut b = base.clone();
        b.attributes.str_ += 1;
        assert!(matches!(
            b.validate(&pack),
            Err(BuildError::AttributePoints { .. })
        ));
        // No aspect is neutral (MATRIX.md 5): a blade without one is fine, and slots
        // nothing elemental.
        let mut b = base.clone();
        b.aspects = Aspects::NONE;
        assert_eq!(b.validate(&pack), Ok(()));
        b.primary = pack.find("ice_shard").unwrap();
        // (the ice shard is Water, which the blade had a moment ago)
        assert!(matches!(
            b.validate(&pack),
            Err(BuildError::MissingAspect { .. })
        ));
        let mut b = base.clone();
        b.primary = 999;
        assert_eq!(b.validate(&pack), Err(BuildError::UnknownAbility(999)));
        let mut b = base.clone();
        b.primary = b.secondary;
        assert!(matches!(
            b.validate(&pack),
            Err(BuildError::WrongSlot { .. })
        ));
        let mut b = base.clone();
        b.actives.push(b.actives[0]);
        assert!(matches!(
            b.validate(&pack),
            Err(BuildError::DuplicateAbility(_))
        ));
        // Fewer points than the free ones is a build; and since nothing is priced
        // (MATRIX.md 20) a second aspect under four plain actives is one too.
        let mut b = base.clone();
        b.attributes.str_ -= 1;
        assert_eq!(b.validate(&pack), Ok(()));
        let mut b = base.clone();
        b.aspects = Aspects::two(Element::Water, Element::Fire);
        // (Three bought beside the body skill, BODIES.md 7.5: the blade is a Soldier.)
        let skill = b.body_skill(&pack);
        b.actives = (0..pack.abilities.len() as u16)
            .filter(|&i| {
                pack.abilities[i as usize].slot == Slot::Active
                    && pack.abilities[i as usize].aspect.is_none()
                    && !pack.abilities[i as usize].creature
                    && skill != Some(i)
            })
            .take(MAX_ACTIVES - 1)
            .collect();
        assert_eq!(b.validate(&pack), Ok(()), "{b:?}");
        // Fire content needs the Fire aspect (the firebolt is a primary; the blade is Water).
        let mut b = base.clone();
        b.primary = pack.find("firebolt").unwrap();
        assert!(matches!(
            b.validate(&pack),
            Err(BuildError::MissingAspect { .. })
        ));
    }
}
