# Gear brings the build (v0, PROPOSED 2026-10-16)

Status: **proposed**, after the item catalog of 2026-10-14 (every template and material
as of 1ecfa41, and its seven findings) and a design talk on 2026-10-16. The director
took the direction in words: "Everything else I agree with you", with one decision made
on the spot, the mode (section 5). Nothing here is built; section 9 is the order of work
and section 10 what the director decides.

It stands on ITEMS.md (what is worn, the gear term), EQUIPMENT.md (the six places, the
hands, whose hands a weapon is for), MATRIX.md 4, 9 and 10 (the armour class, the budget,
kits as data), MODES.md 2 (the mode), ECONOMY.md 4 (templates, materials, layers) and
PLAN.md 0 (an edge of 25% at most, never a tier), 3.1 (no classes), 3.4 (gear unbound and
un-classed, balanced by penalties) and 5.3 (what each layer is for). It moves none of the
decided ones; it finishes two of them (3.4 and 5.3) that Phase 11 left as plain edge.

## 1. What is wrong, in one line

A worn item is one number stretched over fifty-six things. A plain sword is +2.0% slash,
a plain cuirass −1.2% physical, the best kit anyone can wear wins an even exchange by 23%,
and beside it the armour class moves slash by 2.5× and a dual aspect by 4×, and neither
is on any item. Twenty-two materials in five layers all pay the same number. Eleven of
twenty weapons fit no preset. Two-handers lose to a sword with a shield. The stalls sell
every make at 120 silver.

The fix is not a larger cap (PLAN.md 0 is locked). It is that the things a player buys
decide things a player can feel: **the weapon decides the swing, the off hand the guard,
the chest the class, and each layer of a craft does its own promised job.** The K page
stops asking for a class and a primary; the shop starts answering both.

## 2. The weapon brings the primary, the off hand the guard

As built, the kit gates the weapon: an item of a weapon template is worn only by a build
whose kit holds an ability with that `prop` (ITEMS.md 2, the director's rule of
2026-10-08). Every weapon already has exactly one primary that names it (`sword`,
`katana`, `mace`, `warhammer`: all twenty), so the inversion is small:

- **The worn weapon fills the primary slot.** The sheet's kit is derived from the build
  *and* the worn templates (`Sheet::new(build, worn, pack)`, one function in `gm-core`,
  run by the zone and by the owning client): the primary is the ability whose `prop` is
  the right hand's model. Where one prop has several primaries (the staff's blow and the
  five elemental bolts; the musket and the carbine), the build's stored primary is taken
  when it has that prop, else the first whose aspect the build holds, else the plain one
  (the staff's blow). **Nothing in the right hand: the build's primary, as today**, so a
  character in nothing, a companion (ITEMS.md 2: a hire fights in nothing) and a creature
  play exactly as they do now, and no stored build breaks.
- **The off hand fills the guard.** A shield template names its guard (`guard =
  "shield_wall"`) and may override its block (section 7); the focus names `brace`.
  Nothing in the left hand, or a two-hander in the right: the build's guard (a parry with
  a claymore is a choice). A second one-hander in the left hand brings no guard (as
  today: an edge, until a dual-wield extra exists).
- **The secondary and the actives stay the build's.** They are the choices the budget
  is for.
- **The weapon-fit rule becomes the mode's alone** (section 5): a gun build's right hand
  takes a firearm and nothing else, an action or RPG build's takes no firearm; everything
  else fits every hand. `economy::fits` keeps its shape and its words ("a gun build's
  hands are for a firearm: not a mace"). The `hands:` line on a weapon (EQUIPMENT.md 2)
  says the presets whose kits *prefer* it, as today, since it is still true and still
  useful; the dim icon and the refused drop stay for what the mode refuses.
- **The build's `primary` keeps its slot on the K page**, renamed in words to what it is:
  the hands when they are empty, and the choice among a staff's bolts. It costs 0–2 as
  today. A later cut may take it off the page once empty hands have an ability of their
  own (fists); not now.

What this gives at once: the eleven orphan weapons are worn by anyone who buys one, the
Weaponsmith's twelve things sell, and no preset kit changes, so what MATRIX.md 15–17
measured still holds for a character that wears its preset's weapon. A blade with a
warhammer is a build nobody measured; PLAN.md 0 says that is on the player, and section
9's last step measures the worst of them.

**Wearing is refused in a fight** (ITEMS.md 5, ten seconds), so a kit changes only out
of one. The kit's cooldowns are reset at the change as at a respec.

## 3. The chest brings the armour class

MATRIX.md 4 kept the class on the build "until gear could carry it"; ITEMS.md 9 lists the
move as open. The class is the largest single lever a melee fight has (slash 1.25 on
cloth, 0.50 on plate; speed 1.00 to 0.90; stamina regen 1.00 to 0.75) and it is the one
thing an opponent must read at a glance. It goes onto the **chest piece**, and only the
chest: one place, one lever, readable.

| Chest worn | Class | Model |
|---|---|---|
| nothing, `robe` | cloth | the cloth tunic (have) |
| `cuirass` | leather | the ranger's jerkin (have) |
| `mail_shirt` (new) | mail | the official avatar is "the striker in mail" already: a `patch` look painted into the atlas first (LOOK.md 6.6); a mesh when one is found |
| `plate` (new) | plate | the knight's plate is in Quaternius' paid tier (EQUIPMENT.md 7); a `patch` first, a mesh later |

A template says it: `class = "mail"` beside `guards`. Head and legs carry no class (they
keep their share of the edge). A class's speed, regen, ward and evasion bonuses and the
weapon triangle of MATRIX.md 4.1 apply exactly as today, from the worn chest instead of
the build's field. The penalties PLAN.md 3.4 asked for are these: plate is slower and
regenerates less, and now a player buys into them.

**The build points.** The class cost 0 / 4 / 8 / 12 of the kit's 40. A thing that is
bought, traded and taken off cannot cost build points, so the cost goes and the budget
shrinks. What the presets spend on the kit without the class:

| Preset | Spent today | Class | Kit without it |
|---|---|---|---|
| ironclad | 40 | 12 | 28 |
| blade | 30 | 8 | 22 |
| frostweaver | 26 | 0 | 26 |
| shade | 37 | 4 | 33 |
| mender | 34 | 0 | 34 |
| musketeer | 21 | 4 | 17 |
| shaman | 32 | 4 | 28 |

- **`BUDGET` 34** keeps every preset valid untouched. The ironclad gains 6 points of room
  and the blade 12, which is the cost of their classes handed back: both may take a
  third active. Recommended, and the round robin is run again after (9, last step).
- **30** keeps the power where it is but breaks the shade (33) and the mender (34): the
  hub's repair would rewrite them to a preset's kit. Not recommended.

`Build.armour` stays in the stored JSON and on the wire for one version, ignored by
`validate` and `cost`, so no migration is needed: `Build::repaired` already takes a
build whose budget no longer fits. The K page loses the class row and shows the class
the worn chest gives, in the ribbon as today (`Ironclad | colossus in plate`).

## 4. The wire: the worn templates reach the owning client

ITEMS.md principle 4 kept gear off the client's prediction: nothing a client predicts
with (speed, stamina, timings) depended on gear. Sections 2 and 3 end that, on purpose,
and the cost is one message:

- The hub's `GearReading` already carries the six worn template keys to the zone
  (`templates: [String; PLACES]`). The zone, which has the content pack, derives the
  class and the kit from them. Nothing changes hub → zone.
- **Zone → the owning client**: the six keys travel with `Respawned` and on every change
  of gear the zone applies (`FromZone::Worn { templates }`, or a field of `Respawned`
  and of the gear answer). The client rebuilds its sheet with the same `Sheet::new` the
  zone used, so its predicted speed, stamina and hotbar agree with the zone's by
  construction. A protocol version. Other bodies need nothing new: `Look.worn` already
  draws their overlays, and the class they are read by is the zone's business.
- Everything of section 6 that touches stamina rides on the same message. One change
  buys the class, the kit and the frame's job.

## 5. The mode stays the build's (decided 2026-10-16)

Per the director: "let's for now use some different flag for mode and use build for it
for now but so we can change it mid game later at NPC or something, maybe even with
weapon, at some point."

- `Build.mode` stays as it is (`builds.toml`, MODES.md 2): a field of its own on the
  build, not derived from anything worn. A gun build's camera, controls and HUD do not
  change because a sword was picked up; the fit rule of section 2 refuses the sword
  instead.
- **Changing it mid-game** is a respec with the same kit at the trainer, which exists
  (MATRIX.md 9.1); an NPC that changes only the mode is a `Respec` the client builds
  from the current build with one field changed, and nothing new on the wire. Later, if
  the director wants the weapon to carry it, a weapon's ability may list the modes that
  may hold it (`modes = ["gun"]`) and the derivation of section 2 picks the mode from the
  right hand. Not now.

## 6. The layers do their promised jobs

PLAN.md 5.3 decided: shard → a passive trait, core → the physical bias, catalyst → the
element, frame → speed/stamina, gems → counter-meta. ECONOMY.md 4 and `items.toml`
carry the notes ("whalebone: stamina", "boss scale: thorns", "garnet: wards fire") and
none of it exists: every material is plain edge. The proposal from outside (2026-10-16)
to cut the five layers to three is **not taken**: only core and frame are mandatory
today, two gems at most, and the layers already have the right names. They get their
second stat. **The edge stays as it is under the cap of 250**; what follows is on top,
and none of it is a number on damage.

| Layer | On a weapon | On an armour, a shield, a cape |
|---|---|---|
| core | the edge, as today | the edge, as today |
| catalyst | its element's **status on hit**, at a chance per landing blow (not on pulses): ember Burn 2 s, rime one stack of Chill 2 s, thunderstone Shock (10%), umbra Weaken 10% for 2 s, basalt Slow 20% for 1.5 s, zephyr Expose 1.5 s; 20% unless said | the durations of its element's status **taken** × 0.6 (MATRIX.md 6's status duration factor, per element) |
| shard | **Bleed on hit** at 15%: glass m 1, riverglass m 2, boss scale m 4, for 3 s | **thorns**: a melee attacker takes flat pierce on each blow landed, no gear, no matrix (as Bleed's pulse): glass 3, riverglass 6, boss scale 12 |
| frame | the **stamina** of the weapon's blows × : oak 1.0, yew 0.95, ash 0.9, ironwood 1.0 with +10 stagger, whalebone 0.75 | a block's `stamina_per_hit` × the same |
| gem | a status **dealt** lasts × 1.5: garnet Burn, opal the item's catalyst's element, quartz nothing (plain edge) | a status **taken** lasts × 0.5: the same three |

Every number is proposed and is tuned at the dummy (MATRIX.md 9.1) before it ships; the
chances are per blow that lands, rolled by the zone (the client predicts none of it, as
with damage). What it buys: the six catalysts stop being one number with six labels;
garnet is no longer strictly worse than opal; glass and riverglass have a reason to
exist; a whalebone frame is felt in the stamina bar; and a crafted item reads as a
sentence ("a dragonbone sword on whalebone with an ember catalyst and a garnet: slash
+9.5%, burns one blow in five for 3 s, blows cost a quarter less").

The item's words (`ItemSummary.does`) say each, after the edge, in the order of the
table. The catalyst's status on an armour piece and the gem's factor stack by
multiplication; the cap of three Chill stacks and the controls' diminishing returns
(MATRIX.md 8) apply as to any status.

## 7. Two-handers, the bow, the three shields

The catalog's findings 4 and 5, in numbers rather than systems:

- **A heavy blow** (`heavy = true` on the ability: greatsword, claymore, greataxe,
  warhammer, scythe, spear): blocked, it costs the blocker **twice** the guard's
  `stamina_per_hit`; and once its windup has passed, **Stagger does not interrupt its
  active frames** (`interrupt = "on_stagger"` reads the windup only). A shield wall
  that eats three warhammer blows is out of stamina; a sword that taps a claymore's
  swing no longer stops it. The 1.5 share of the edge stays.
- **The bow** gets an arrow stack (`arrow`, cap 60, the quartermaster's at 1 silver,
  as the balls) and 75 damage for 60, so it is a gun-mode alternative in the action
  mode rather than a free crossbow.
- **The three shields** differ on the template, overriding `shield_wall`'s block:

  | Shield | arc | mitigation | stamina per hit | move speed | projectiles |
  |---|---|---|---|---|---|
  | buckler | 90 | 0.7 | 8 | 0.8 | no |
  | shield (heater) | 150 | 0.8 | 12 | 0.5 | yes (as today) |
  | tower shield | 180 | 0.9 | 16 | 0.35 | yes |

- **The helmets** stay three looks of one number until the head's share (0.15) is worth
  a difference; the hood's difference (elements) is real already. Written down, not
  done.
- **The guns** are not touched here: the musket is the opener the outside proposal
  asked for already (110, headshot ×4, slow on hit) and its pace is the gun mode's,
  measured apart (MODES.md 3).

## 8. The shop

Finding 7: two makes exist, plain and fine, both at 120 silver, no loose component for
sale, so nobody crafts from a purchase. Proposed for the economy pass, last in order:

- `play.sh` sells by make: a plain item 30 silver; the fine make 20 gold; the
  Outfitter sells **loose components** priced by edge, so the ladder the materials
  describe (10, 20, 30, 45, 60) can be walked: tin 5, bronze 10, iron 15 silver;
  steel 1 gold 50; oak 5, yew 30, ash 60 silver; ironwood 1 gold 50; a catalyst 1
  gold; glass 10, riverglass 80 silver; quartz 20 silver, garnet 1 gold 50, opal 3
  gold. Dragonbone, whalebone and boss scale stay the bosses' (PLAN.md 5.2).
- A listing's price is the seller's as today; these are the bots' asks, test fixtures
  that at least describe the intended scale (20–50 silver standard, 10–30 gold top).

## 9. The order of work, and what each step costs

1. **The wire** (4): `Sheet::new(build, worn, pack)` in `gm-core`; the worn keys to the
   owning client at `Respawned` and at every gear change; the client rebuilds its sheet
   from them. Protocol version. Nothing visible changes yet. **A day.**
2. **The weapon brings the primary, the off hand the guard** (2); `fits` becomes the
   mode's; the K page's words. The look gate wears a mace on the blade and swings it.
   **A day.**
3. **The class on the chest** (3): `class` on the template, `mail_shirt` and `plate` as
   patches, `BUDGET` 34, `Build.armour` ignored, the K page's class row gone, the
   showcase's ribbon from the worn chest. **A day, plus the two patches.**
4. **The layers' jobs** (6): the catalyst's status, the shard's bleed and thorns, the
   frame's stamina, the gem's factor; the words; a gate that wears each and reads the
   dummy. **Two days.**
5. **Heavy blows, the arrow, the shields** (7). **A day.**
6. **The shop** (8): the bots' asks by make, the Outfitter's components. **Half a day.**
7. **Measure**: the round robin (`gm-bot` arena, MATRIX.md 11) over the seven presets in
   their own gear, then the worst cases the open hands allow (a blade with a warhammer,
   an ironclad with two swords, a frostweaver in plate), and the numbers of 6 and 7
   tuned until no preset is pushed out of the 40–60% band. **A day.**

Each step is its own PR on `main`, shipped to the LAN stack as it merges, so the director
plays every one before the next.

## 10. What the director decides

1. The budget after the refund: **34** (recommended; every preset stands) or 30 (the
   shade and the mender are repaired to a preset).
2. The class on the chest only (recommended), or spread over chest, head and legs.
3. Mail and plate as patches painted into the atlas first, meshes later (recommended),
   or wait for meshes.
4. The empty right hand plays the build's primary (recommended, nothing breaks), or
   fists as a new ability now.
5. The layer table of 6 as the first numbers to tune, or a different job for a layer.
6. Heavy blows: double block stamina and uninterruptible active frames (recommended),
   or more damage instead (20–30 on each two-hander).
7. The shop's asks of 8, or the economy pass later with its own numbers.
8. The order of 9, or a different one.
