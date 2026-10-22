# Character Matrix

Status: v8, 2026-10-19 (the softer matrix and the wider gear term, section 19; v7 2026-10-17, section 18; v6 2026-10-09, section 17; v5 the same day, section 16; v4 section 15; v3 the six elements of section 5, section 14; v2 2026-10-06, v1 Phase 3; the shaman, the seventh preset and the first of Grass, is v6, section 17). This document is the contract for point-buy
characters, the type matrix and the damage pipeline. v2 (section 13): the character's own
thirty attribute points apart from the kit's budget, wider bands and seven times the health,
the range as the weapon's, the trainer and the dummy. `gm-core::matrix` and `gm-core::build` mirror it; when they disagree,
the document wins and the code is wrong. Changes to either go in one commit.

PLAN.md 3.1, 3.2 and 0 are binding: no levels, no gear tiers, a fixed budget, rock-paper-scissors
counters with stacking multipliers and a dual-type hole, compressed stat bands, top gear a
bounded edge (the `gear` term of section 7, built in Phase 11: ITEMS.md 3; 15–25% until
2026-10-19, 50% since, section 19). PLAN.md 12 listed "full type matrix and attribute set" as open; this document is
the proposal that Phase 3 implements. The director's calls are recorded in section 12.

## 1. Principles

1. **Stats and gear move damage as much as counters do; a counter tilts a fight, it does
   not decide it** (v8, 2026-10-19, section 19). A derived stat spans at most 2× between
   the worst and the best attribute spread (health 620 → 1,500, damage ×0.72 → ×1.6); the
   matrix moves damage by 1.25 or 0.75 a step (a 1.67× exchange, 2.78× for a dual-aspect
   hole), and the best gear against nothing by 1.5×. The attribute band is the largest
   lever; the body, the element and the gear are each a tilt. (Until v8 the matrix was 2
   and 0.5, a 4× exchange, and "counters move damage more than stats do"; v1 compressed the
   bands to 1.5× and 95–140 health: a fight was over in a second and a point bought nothing;
   section 13.)
2. **Readable in three seconds.** Frame = silhouette, armour class = how the body moves,
   aspect = aura colour. Everything that changes the matrix is visible.
3. **Two budgets, reallocatable.** A character has thirty attribute points of its own and a
   kit worth at most 40. Both are redone, free, at the trainer in the town (section 9.1); in
   the arena at the next respawn.
4. **No immunity.** Every multiplier is 0.5625–1.5625 (0.25–4 until v8). Everything can be
   killed by anything, slower.
5. **No RNG in damage.** Spread on projectiles is the only randomness in combat.

## 2. Attributes

Five attributes, integer, each from **3** up to what the points reach (the cap of 25 went
on 2026-10-19: a player may put every point on one; `Attributes::MAX` is only the
highest floor plus thirty, 43). The floor is the **body's** (BODIES.md 3:
five floors summing to 25, a 3 where the body is plainly bad at something; v8,
2026-10-19) and a flat **5** for a build without a body (a preset, a creature, a bot: as
every character was before the bodies); a character has **thirty points** to put above
its floors, however it likes (all in one, or spread; fewer may be spent, more is
refused). They are not the kit's budget (section 9): a build does not trade a stat for an
ability. The presets spend all thirty.

| Attribute | Governs |
|---|---|
| STR | physical damage multiplier, knockback dealt |
| AGI | move speed, stamina pool and regeneration, evasion |
| CON | health, stamina pool, physical mitigation (armour), stagger threshold |
| INT | elemental damage multiplier, focus pool |
| SPR | elemental mitigation (ward), focus regeneration, status duration taken |

## 3. Frames

The archetype frame is free: its price is the hitbox and the mobility it fixes
(VOCABULARY.md 3). World collision is the 32 × 32 × 56 hull for all four.

| Frame | Hitbox (r × h) | Mass | Mobility | Armour | Ward | Evasion | Focus |
|---|---|---|---|---|---|---|---|
| Colossus | 18 × 60 | 1.5 | 0.92 | +0.10 | 0 | 0 | 0 |
| Striker | 14 × 56 | 1.0 | 1.00 | +0.05 | 0 | 0 | 0 |
| Caster | 13 × 56 | 0.9 | 0.96 | 0 | +0.10 | 0 | +20 |
| Infiltrator | 12 × 52 | 0.8 | 1.06 | 0 | 0 | +0.05 | 0 |

Mass divides knockback taken. Mobility multiplies move speed.

## 4. Armour class — gone (2026-10-17)

There is no armour class. The director: "class is wrong name for what armor provides, it's
like defense type that's provided, either physical or magical, no? makes no sense to
complicate further." What an armour provides is on its template already: `guards`
(`physical` or `elements`, ITEMS.md 3) and the gear term its layers make (the edge per
damage type, `Gear::taken`). A body's speed, stamina regen, ward and evasion are its
frame's and its attributes' (6), whatever it wears; the three physical kinds (slash,
pierce, blunt) stay damage types the gear's edge reads and multiply nothing else. The
look is the frame's (CONTENT.md 3.2), the gait the frame's (MODELS.md 9).

From Phase 3 to 2026-10-16 the class (cloth, leather, mail, plate) was a build choice at
0 / 4 / 8 / 12 points with a speed and regen penalty, a ward or evasion bonus and the
"weapon triangle" of physical kind × class (slash 1.25 on cloth, 0.50 on plate; blunt the
reverse; pierce the honest middle); on 2026-10-16 it moved onto the worn chest piece
(GEAR.md 3); on 2026-10-17 it went (GEAR.md 12). The table and the triangle are in the git
history of this section.

## 5. Elements and aspects

Six elements (v3, 2026-10-09; v1 and v2 had five in a pentagram, section 14). A build has
**none, one or two aspects** (the first is free, the second costs 10 points). Aspects are
the build's defensive element types **and** gate which elemental abilities it may slot (an
ability tagged Fire needs the Fire aspect). **No aspect is neutral**: the body takes 1× from
every element, and slots nothing elemental.

The matrix is the well-known one (the chart everybody has played against), with **Air** for
its Flying, cut to the six types below and with its two immunities turned into resists
(principle 4). It is deliberately lopsided: an element may beat one and be beaten by two,
and pairs are neutral; what holds is that **every element beats at least one, is beaten by
at least one, and no element stands more than one step from even** (beats + resists −
beaten − resisted, over the other five). Attacker by row, defending aspect by column:

| Attack \ Aspect | Fire | Water | Grass | Electric | Ground | Air |
|---|---|---|---|---|---|---|
| Fire | 0.75 | 0.75 | **1.25** | 1 | 1 | 1 |
| Water | **1.25** | 0.75 | 0.75 | 1 | **1.25** | 1 |
| Grass | 0.75 | **1.25** | 0.75 | 1 | **1.25** | 0.75 |
| Electric | 1 | **1.25** | 0.75 | 0.75 | 0.75 | **1.25** |
| Ground | **1.25** | 1 | 0.75 | **1.25** | 1 | 0.75 |
| Air | 1 | 1 | **1.25** | 0.75 | 1 | 1 |

Readings: fire burns grass; water drowns fire and washes the ground away; grass drinks the
water and splits the ground; lightning strikes the water and whatever flies; the ground
smothers fire and takes the lightning; the wind tears the grass. The chart's immunities
(lightning into the ground, the ground into what flies) are 0.75 here. The steps are
**1.25 and 0.75** since v8 (2026-10-19, section 19; 2 and 0.5 from v3 to v7): one step is
a 1.67× exchange, a tilt the thirty points or the gear can answer.

| | beats | beaten by | resists | resisted by | net |
|---|---|---|---|---|---|
| Fire | 1 | 2 | 1 | 1 | −1 |
| Water | 2 | 2 | 1 | 1 | 0 |
| Grass | 2 | 2 | 3 | 2 | +1 |
| Electric | 2 | 1 | 1 | 2 | 0 |
| Ground | 2 | 2 | 1 | 2 | −1 |
| Air | 1 | 1 | 2 | 1 | +1 |

**Stacking.** Against a dual-aspect defender the two multipliers multiply. Four of the
fifteen pairs have a **1.5625× hole** (a predator of both; 4× until v8): Fire + Electric
(Ground), Fire + Ground (Water), Water + Ground (Grass), Water + Air (Electric). Every pair
but Water + Electric, Electric + Air and Ground + Air has at least one **0.5625× wall**
(0.25× until v8); Grass + Air walls two (Grass and Ground). Spiky or safe is the build choice; both cost 10 points, both still
eat the physical table. Neutral is the third choice: no hole, no wall, and no element to cast.

Physical packets ignore this table; elemental packets ignore the armour-class table.

## 6. Derived stats

`attr` is the attribute value (3 to 43, section 2; the bands below are for the common
5..=20). All results are rounded to integers where they are pools; multipliers stay f32.
Bands are deliberately narrow.

| Stat | Formula | Range (5 → 25) |
|---|---|---|
| HP (health) | `50 + 4·CON` | 70 → 150 (a tenth of v10's, section 22) |
| ST (stamina) | `6 + 0.3·(CON + AGI)` | 9 → 21 (a tenth, section 23) |
| ST regen /s | `1 + 0.06·AGI` | 1.3 → 2.5 |
| MP (magic points; `focus` in the code) | `4 + 0.5·INT + frame.focus` | 6.5 → 16.5 (+2 Caster) |
| MP regen /s | `0.5 + 0.06·SPR` | 0.8 → 2.0 |
| Move speed (u/s) | `(280 + 2·AGI) · frame.mobility` | 290 → 330 before the frame |
| Physical damage × | `0.60 + 0.04·STR` | 0.80 → 1.60 |
| Elemental damage × | `0.60 + 0.04·INT` | 0.80 → 1.60 |
| Armour (physical mitigation) | `min(0.50, 0.015·CON + frame.armour)` | 0.075 → 0.375 |
| Ward (elemental mitigation) | `min(0.60, 0.02·SPR + frame.ward)` | 0.10 → 0.50 |
| Evasion | `min(0.50, 0.015·AGI + frame.evasion)` | 0.075 → 0.375 |
| Knockback taken × | `1 / frame.mass` | |
| Knockback dealt × | `0.60 + 0.04·STR` | |
| Status duration taken × | `1.30 − 0.03·SPR` | 1.15 → 0.55 |
| Stagger threshold | `40 + 3·CON` | 55 → 115 |

Max speed feeds `MoveVars.max_speed`; jump velocity and acceleration are not attribute-scaled
(movement skill is movement skill). ST regen pauses for 1 s after any ST spend. Since
2026-10-22 (section 23) the three pools are named HP, MP and ST everywhere a player reads
them; the primary costs nothing, every other skill costs MP, and ST is the guard's (a block's
`stamina_per_hit`, a parry). CON buys HP and DEF (the armour) both.

Evasion is positional and readable: it applies only while the defender is **evading**, i.e.
inside a `MoveSelf` (dash, leap, charge, blink) or for 2 ticks after one ends. Running or
strafing is not evading; a standing target has no evasion. Packets with `bypass EVASION`
("tracking") ignore it.

## 7. The damage pipeline

For a `DamagePacket` from attacker A landing on defender D:

```
base   = amount
       × (A.physical_mult  if kind is physical  else A.elemental_mult)
       × (A.Weaken ? 1 − weaken : 1)
type   = kind_table[kind][D.armour_class]        physical
       | Π element_matrix[element][aspect]        elemental, over D's 1–2 aspects
body   = group_mult(A.group, kind)               BODIES.md 7.3 (2026-10-20): magic ×1.25 on
       × (elemental && element ∈ A.aspects ? 1.25 : 1)   spells and ×0.75 on blows, melee the
                                                  reverse, ranged ×0.75 on spells; the caster's
                                                  own element ×1.25; a body's, 1 for a creature
                                                  and for a pulse
layer  = physical  && !bypass ARMOR        ? 1 − D.armour            : 1
       × elemental && !bypass MAGIC_SHIELD ? 1 − D.ward              : 1
       × D.Fortify && !bypass MAGIC_SHIELD ? 1 − fortify             : 1
       × D.evading && !bypass EVASION      ? 1 − D.evasion           : 1
guard  = D guarding toward A && (melee || stops_projectiles) ? 1 − block.mitigation : 1
gear   = (2000 + A.dealt[t]) / (2000 + D.taken[t])              ITEMS.md 3.1: the edge of
         A's worn weapon and of D's worn armour on the packet's type, per mille, at most 250
         each; 1 for the pulses of a status, and for two bodies in nothing
zone   = the bolt is a firearm's and entered the hull within its top 12 u ? headshot : 1
         (the head band, MODES.md 3.5: before armour, on the amount)
damage = max(1, round(base × type × body × layer × guard × gear × zone))
```

`Expose` on the defender grants every attacker `bypass ARMOR`. A parry inside its window
negates the packet entirely (the only zero) and runs the parry's `on_success` on the attacker.

**A packet whose `amount` is 0 is not an attack** (Phase 7): it never enters this pipeline.
It deals nothing (not the minimum of 1), knocks nobody back, builds no stagger, is not
blocked, parried or evaded and interrupts nothing; only its triggers land. It is how a
projectile carries a boon: the healer's dart is a zero packet with Regen on whoever it hits.

The three true bypasses of PLAN.md 3.2 are structural, not special-cased:
- **magic through armour**: elemental packets never consult `D.armour` or the kind table;
- **blunt through magic shields**: hammers are content with `bypass MAGIC_SHIELD`, so they
  ignore `Fortify` bubbles and the ward (but Blunt vs Cloth is 0.75, so they are not free);
- **tracking through evasion**: wide arcs and seeking-free tracking shots carry
  `bypass EVASION`.

Stagger: every packet adds its `stagger` value to D's build-up, which decays 20 points per
second. Guard mitigation does not reduce build-up (a shield wall is broken by weight of
blows, not by damage). Crossing the threshold applies `Stagger` (section 8) for 400 ms,
resets the build-up and starts a **1 s stagger immunity** (build-up is discarded) so no cadence
can chain-lock. Knockback is applied after damage, scaled by `knockback_dealt × 1/mass`.

Guard break: when a blocked hit costs more stamina than the defender has, the guard breaks:
the hit applies without the block factor, the stamina goes to 0, the guard is released and
the defender is Staggered (immunity rules apply).

Order within a tick: statuses from `ApplyStatus` steps (VOCABULARY.md 7 step 3) land before
projectiles (step 4) and areas (step 5) resolve; a status attached to a hit (`on_hit`, or a
`Hit`-targeted step) lands after that hit's damage, so it benefits the next packet, not the one
that carried it.

## 8. Statuses

The initial set of VOCABULARY.md 5.4 with resolved semantics. `m` is the magnitude from the
`ApplyStatus` verb; durations are multiplied by the defender's status duration factor
(section 6), except Stagger and Shock. Statuses tick after all verbs (VOCABULARY.md 7).

| Status | Effect | Stacking default |
|---|---|---|
| Slow | move speed × (1 − m) | Refresh |
| Haste | move speed × (1 + m) | Refresh |
| Root | move speed 0; turning, abilities and guard still work | Refresh |
| Bleed | m physical damage per second (as Pierce, bypass ARMOR), 1 pulse/s | Independent, max 5 |
| Burn | m Flame damage per second through the matrix and ward, 1 pulse/s | Refresh |
| Chill | move speed × (1 − 0.15 per stack); at `max_stacks` it becomes Freeze: Root for 1 s, all stacks clear and the target is immune to Chill for 2 s | Extend, max 3 |
| Shock | interrupts the running script and guard; lasts 1 tick | — |
| Silence | elemental abilities cannot be activated | Refresh |
| Stagger | interrupts; no activation or guard; move speed × 0.3 | Refresh |
| Fortify | damage taken × (1 − m) unless bypass MAGIC_SHIELD | Refresh |
| Weaken | damage dealt × (1 − m) | Refresh |
| Expose | attackers gain bypass ARMOR | Refresh |
| Regen | heals m health per second, 1 pulse/s; heals enemies too | Refresh |
| Stealth | included in snapshots only within `m` units (never past PVS) | Refresh |
| Knockdown | on the ground (MODES.md 4.5): interrupts; no activation, guard or movement; hits land in full | Refresh |
| Launched | as Knockdown, with a lift of `m` u/s straight up when it lands | Refresh |
| Taunt | the body is turned to the slot's source (who applied it) and held there, tick by tick, while the source is on its wire; the view turns with the body on the client; a mind under it fights the source (gm-ai, gm-bot). Lands on enemies only (an ally in the roar is untouched). v5, section 16 | Refresh |

**Diminishing returns on controls** (MODES.md 4.5, `gm_core::sim::CONTROL_WINDOW_MS`):
Knockdown, Launched, an explicit Root and Taunt are controls, each its own kind. The second of a kind within ten
seconds of the last lasts half, the third does nothing; ten seconds after the last, the
count is forgotten. Freeze's Root (Chill at its stacks) is not under them: Chill has its own
immunity.

Dispellable statuses are removed by `ApplyStatus { status: Cleanse }`-style content later;
Phase 3 content has no dispel. A status with `max_stacks` 1 and `Refresh` restarts its
duration on reapplication; `Extend` adds the duration (capped at 3× the verb's duration);
`Independent` tracks each application separately.

## 9. Builds and the budget

```
Build {
  body:         one of bodies.toml, or none (BODIES.md 3)        free; fixes the frame, the floors, the first active
  frame:        Colossus | Striker | Caster | Infiltrator       free (the body's, with a body)
  attributes:   STR AGI CON INT SPR, each 3..=25                 Σ(attr − floor) ≤ 30, the character's own
  aspects:      0..=2 of Fire Water Grass Electric Ground Air    0 / 10 (none: neutral)
  kit:          primary, secondary, guard (optional), up to 4 actives   Σ ability cost
                (with a body: the hands cost nothing, the body skill is active 1, up to 3 bought)
}
aspects + kit ≤ 34 (the kit's budget; 40 with the armour in it until 2026-10-16, GEAR.md 3); unspent is allowed.
Under a hub the worn weapon fills the primary and the worn off hand the guard (GEAR.md 2).
A player's character has a body (2026-10-19); the presets are the bots', the creatures' and the arena's.
```

Rules enforced by `gm-core::build::validate`:
- every slot holds a distinct ability of the matching slot type (primary, secondary, guard,
  active); an ability's `aspect` requirement must be in the build's aspects;
- at most one ability per cooldown group;
- the attribute floor and cap, the thirty points, the kit's budget, at most two aspects.

A rejected build never enters the zone. A stored build the rules no longer take (the v1
presets had 56–68 attribute points, and a bolt in the secondary slot) is **repaired** by the
hub when the character enters (`Build::repaired`): the points are scaled down to thirty in
their own proportion, and a kit the pack refuses becomes the preset's with the same primary
(else the first); what cannot be repaired is refused in words.

### 9.1 The trainer, the dummy and the arena

`FromClient::Respec(Build)` is validated at once, and then:
- in a **zone of the world** (the town; any zone that posts creatures) it is worn **now**,
  with full pools, if the body is alive, out of any fight (ITEMS.md 5's ten seconds) and
  within `TRAINER_REACH` = 160 u of the **trainer**: a creature marked `npc` (nothing hurts
  it; the town posts one beside its board). Else it is refused with the reason;
- in a **team zone** (the arena, the practice ground) it is worn at the next respawn, as in
  v1, so a match can be re-specced mid-way (section 11).

The client's page (`K`, CLIENT.md 4.5) edits the points and the kit anywhere and offers
"Wear it" by the trainer; a game master wears any build at once, anywhere (GM.md 2).

**The build chosen is the character's** (2026-10-08). Under a hub the zone, having taken
a respec, saves the character at once (`Save`, HUB.md 3.2) and answers `RespecResult`
only when the hub has: `Ok` means worn (now in the world, at the next respawn in a team
zone) *and* the hub's, so it is what the character wears when it next enters anywhere,
after a logout or a travel. In a team zone the build saved is the one chosen, whether or
not the respawn that wears it came before the character left. A save the hub refuses is
answered `Err("worn here, but not saved: ...")`: the zone wore it, the next claim would
not have. Before this a build worn at the trainer reached the hub only with the next
periodic save (up to 30 s later) or the leaving one, and one chosen in a team zone was
lost when the character left before its respawn; the client never called the hub's
`SetBuild`, which is for a character that is offline (HUB.md 2), and never will from a
zone: while a character plays, its zone is the only writer of it.

Three **training dummies** (`dummy`: a creature marked `still`, 500 health, back after
5 s) stand in a triangle in the gated south street behind the trainer (2026-10-06; the one
on the green north of the market was out of the way): each is hit like any body and the
attacker reads what it dealt (LOOK.md 13.8). A `still` creature has no mind: it is driven an empty frame a tick.

## 10. Kits as data

Abilities and preset builds live in `assets/content/*.toml`, loaded and validated at zone
start against VOCABULARY.md 11 and this document. The zone sends the loaded content to every
client after `Welcome` (`FromZone::Content`), so a client can never run different numbers than
its zone and needs no content parser.

Each ability carries: `slot` (primary | secondary | guard | active), `cost` (kit points),
`aspect` (optional; the required aspect, and every elemental packet in the script must be of
that element), and the verb script of VOCABULARY.md 6. Price bands: primaries 0–2,
secondaries 2–4, guards 4–6, actives 6–12. **The range is the weapon's** (v2): a primary is
either a melee weapon (sword, hammer, staff, dagger; the axe and the spear since 2026-10-14,
EQUIPMENT.md 5.6) or the ranged one (crossbow, musket, the bow and the wand's bolt of force
since the same day, and the five elemental bolts); a secondary is a short utility (kick, shield bash, a knife of
300 u, mend). A melee build has nothing that reaches past the knife; a ranged build's primary
is the whole of its damage at range. (v1 gave every build a bolt in the secondary slot:
melee had free range and range had no edge; section 13.) A creature may hold a second
primary in its secondary slot (the sentinel's crossbow, the Warden's stone). The content
ships the abilities and seven presets (`ironclad`, `blade`, `frostweaver`, `shade`, `mender`,
`musketeer`, section 15; `shaman`, section 17); `assets/content/README.md` has the table. Phase 7 adds `mend`
(secondary, 4), `sanctuary` (active, 10) and `war_standard` (active, 10; two more squad
slots), the presets `mender` and `captain` (cut 2026-10-09, section 15), and two more files: `creatures.toml` (creatures
are builds without a budget, with abilities marked `creature = true` that no player build may
slot) and `trials.toml` (COMPANIONS.md 8, 11). An ability may carry `squad = N`: companions
it adds to the squad of whoever has it in the build.

## 11. Acceptance: a dominant build can be countered by re-speccing

PLAN.md 11.8 Phase 3. Measured with bot matches in the arena map (`gm-bot` arena test), eight
against eight, identical brains on both sides, several seeds:

1. **`ironclad`** (Colossus, Plate, Stone; hammer, stone throw, stomp, fortify, shield wall)
   against **`blade`** (Striker, Mail, Flame; sword, firebolt, dash, overhead, parry): ironclad
   must win the kill count by at least 1.3 : 1. Slash into Plate is 0.5, Flame into Stone is
   0.5, the hammer crushes Mail at 1.0 and ignores Fortify.
2. The blade side re-specs to **`frostweaver`** (Caster, Cloth, Frost; staff, ice shard,
   frost nova, haste, brace): frostweaver must now win by at least 1.3 : 1. Frost into Stone is
   2 and ignores armour; Plate's 0.90 speed cannot close on a hasted caster; Blunt into Cloth
   is 0.75.

3. The cycle closes: **`blade`** against **`frostweaver`** must be decided for the blade
   (Flame is 4× into Frost + Shadow).
4. A mirror match (blade against blade) must be even within noise, or the map or the brains
   are lopsided and the other results mean nothing.

`scripts/check-matrix.sh` runs these (`crates/gm-bot/tests/arena.rs`, three seeds, 60 s each);
`crates/gm-server/tests/counterpick.rs` repeats the first two legs over the real protocol at
150 ms and 3% loss, with the blades re-speccing to frostweavers through `FromClient::Respec` after
ten seconds. The ratios are recorded in PLAN.md 11.10 with the seeds. The test fails if any leg
is not decided, which would mean the matrix is not doing its job.

Measured 2026-10-01 (v1 content, three seeds × 60 s, 8 v 8, offline): ironclad : blade
**98 : 34** (2.9); frostweaver : ironclad **156 : 5** (31); blade : frostweaver **100 : 58**
(1.7); mirror 74 : 75. The second leg is a hard counter: Frost is 2× into Stone and ignores
plate, Stone is 0.25× into Frost + Shadow, and a hasted caster kites a plate colossus
indefinitely. Its counter-counter is the third leg, or a teammate with Flame.

v3 (section 14) keeps the cycle with the six elements: Water is 2× into the ironclad's
Ground and the ironclad's Ground is 0.5× into Water + Air; the blade wears Water, so the
frostweaver's shards are 0.5× into it as before. The arena gate (`gm-bot/tests/arena.rs`)
re-run on the v3 content (2026-10-09, three seeds × 60 s, 8 v 8, offline): ironclad : blade
**34 : 12** (2.8); frostweaver : ironclad **50 : 1** (50); blade : frostweaver **21 : 12**
(1.8); mirror 34 : 28. The second leg is harder still than in v1: Water is 2× into Ground and
ignores plate, and the ironclad's stomp into Water + Air is halved.

## 12. Decisions and review log

**Proposed by Phase 3 (director to confirm or change):** the attribute set and ranges
(section 2), the pentagram of five elements (section 5; six since v3, section 14), the kind × class table (4.1), the
formulas of section 6, the exact-100 budget (section 9). Everything here is tuned in
playtests; the structure (two tables, multiplicative layers, three structural bypasses) is
what the code depends on.

**2026-10-01, v1 draft reviewed by Gemini 3.1 Pro** (independent review before
implementation; verdicts are ours):
- Accepted: evasion while merely running was an unreadable permanent 30% reduction for a
  circle-strafing melee (now only inside a `MoveSelf` and 2 ticks after); stagger with no
  immunity chain-locked at any cadence above the threshold (1 s immunity added); guard break
  was undefined (defined above); Freeze could be re-applied the moment it ended (2 s Chill
  immunity); the tie-break between a status and the packet that carried it was undefined
  (defined above); attributes at 1 point per 2% were overpriced against abilities (price
  bands in section 10 so the budget binds); effective-HP spread of 2.4× contradicted the
  "compressed" principle (health is now 80 + 3·CON, armour capped at 0.35, and principle 1
  states the real bands).
- Corrected, not as proposed: "every dual aspect has a 4× hole" was false, but the suggested
  rule (`i` beats `i + 1`, `i + 2`) cannot fix it either: in any balanced 5-element matrix
  exactly five of the ten pairs share a predator. Section 5 now states the two pair shapes.
- Noted for tuning, not a document change: with placeholder damage numbers the frostweaver
  loses a pure DPS race to the ironclad; the content numbers are set by the acceptance
  matches (section 11), not by this document.
- Rejected: capping armour at 0.15 (the physical table is the counter layer; a plate colossus
  at 7% from the front with a shield up is the design, broken by stagger, stamina and magic).

**2026-10-01, implementation reviewed by Gemini 3.1 Pro** (after the acceptance matches passed):
- Accepted: the client only adopted the server's stamina, focus, statuses and guard state when
  the position disagreed, so a blocked hit's stamina or a Bleed never reached the prediction
  (now adopted and replayed whenever they differ, without counting a correction); a block's
  facing test used the shooter's current position for projectiles instead of the shot's
  direction; a swing pushed during its windup still landed after a stagger cleared the script
  (swings now require their script, ripostes excepted, and a stagger drops them); damage over
  time rounded each pulse so a magnitude of 1 dealt nothing and 6 dealt 8 (pulses now
  accumulate to exactly the magnitude per second); the parry's cooldown group was not applied;
  `Extend` could shorten a longer application. Blink's landing is now nudged out of solid as a
  hardening, though the tracer already pulls back from the plane.
- Rejected: sending the chill and stagger immunity timers to the client (the client never
  predicts a status an enemy applies, so its timers are never consulted).
- Deferred to Phase 4: the per-tick body list is O(N²) at 200 players (spatial partition with
  the swarm test).

## 13. v2 (2026-10-06): the director played it

The director, after playing Phase 14 with the fx and the GM hand: "char stats editable in
game like Tales of Pirates or Ether Saga: start with 30 points to distribute however we like;
an NPC for stat reset and a training dummy in town; very unfair that everybody has a right
click fireball shooting far and DD one-hit killing spellcasters; fights are tap tap tap dead;
'tactical' meant rock-paper-scissors, not the top view." Measured before the change: health
95–140, the sword 35 slash at 300 ms ×1.25 on cloth = a caster dead in two hits, about one
second of contact; the firebolt needed five; the presets spent 56–68 of 100 points on
attributes, so every build was near 20 on four stats and a point moved health by 3 of 140.

Done, as proposed and accepted ("do it all as you proposed"):
- attributes 5..=25 (uncapped since 2026-10-19) with thirty points of the character's own (section 2), the kit's budget
  40 apart from them (section 9); `Attributes::default()` is 11 across;
- the bands of section 6 widened (health ×7, damage ×0.8–1.6), armour cap 0.50, ward 0.60;
- the swings slowed and made heavier (sword 60 slash, 150/60/300 ms, 600 ms cooldown; the
  hammer 90 at 1 s; the dagger 35 at 400 ms), the bolts made primaries at about one and a
  half times their damage, and four secondaries in their place (section 10);
- the trainer, the dummy and the arena rule (9.1); the character's page `K`; old builds
  repaired at the hub; creature health ×3 (sentinel 1,200, Warden 15,000 at twice a player's
  blows: `might`, COMPANIONS.md 8.1) and the heals ×5 (Mend 100/s, Sanctuary 60/s: the
  healer's share of a body a second is what it was);
- the tactical viewport removed from the client (PLAN.md 4.3 [REVERSED]; COMPANIONS.md 6):
  1.3 KB of code and about 2% of the browser download, not the bloat, but no longer wanted.

Numbers measured after the change are in PLAN.md 11.10 and section 11 (re-run). The
director's call still open: the fight length he wants in play (the tempo and these numbers
are the knobs, GM.md 3).

## 14. v3 (2026-10-09): the elements everybody knows

The director: "take the basic elements from Pokémon and their weaknesses, they are well known
already"; of the candidate sets, the classic core plus three (Fire, Water, Grass, Electric,
Ground, Flying), "loose immunity, just 0.5 instead; flying we should call air"; then "maybe
also have no elements as neutral (normal) with no strengths or weaknesses". The sets were
searched by machine: every five- and six-type subset of the chart in which each type beats
one and is beaten by one, ranked by how far the best and worst stood apart; the six chosen
are the most recognisable of the sets within one step of even. The matchups are the chart's
numbers unchanged, the names are plain words, and a matchup chart is a game rule.

Done:
- section 5: six elements, the table and the pair shapes; `Element` is a table, not a
  formula; `Aspects` 0..=2, none neutral (`validate` no longer refuses an empty set; the
  character page says so; no ring at a neutral body's feet);
- the old five re-mapped: Flame → Fire, Frost → Water, Storm → Electric, Stone → Ground,
  Shadow → Air (the blink, the vanishing, the dart and the cloud are on the wind; the shade,
  the musketeer and the frostweaver's second aspect). The blade, the captain and the
  sentinel wear **Water**, not Fire: they have no element to cast, their aspect is a
  defence, and Fire would have made the frostweaver's shards 2× into them and broken the
  cycle of section 11. The trainer keeps Fire. The catalysts keep their names
  (`catalyst/umbra` is Grass now) and `catalyst/zephyr` joins for Air;
- the damage types are nine (`DamageType`: the three kinds and the six elements), so the
  edge arrays of ITEMS.md 3 are nine wide: `PLAYER_VERSION` 6 (ITEMS.md 4); the snapshot's
  `aspects` are six bits: `PROTOCOL_VERSION` 16 (PROTOCOL.md 28);
- the bot's counter-pick follows the new table (mender counters a Water + Air frostweaver:
  Electric is its 4× hole).

Open: nobody casts Fire among the presets (the firebolt and the fireball are slotted by
nobody; that was so before) and nothing at all is Grass yet (the catalyst `umbra` is its
only content); whether the blade's Water stays or the blade goes neutral is the director's
to play. The arena gate: see section 11.

---
## 15. v4 (2026-10-09): six presets, each with its own face

The director: "there seems to be too many of them, we need to simplify somehow and make them a
bit more unique"; then "cut to six, distinct ones, but I'd leave gun (FPS) neutral". The
eight presets had been measured first: every preset against every other, eight against eight,
both sides, six seeds of 60 s (`round_robin` in `gm-bot/tests/arena.rs`, ignored by default:
`ARENA_SEEDS=6 cargo test -p gm-bot --release --test arena round_robin -- --ignored
--nocapture`). The blade and the captain were 49 : 51, the same build; four of eight were
strikers, dash sat in four kits, Water and Air in three each, Fire and Grass in none.

Done:
- **captain** and **marksman** cut. The war standard stays an active any build may slot (a
  party leader is a choice, not a class: `squad_capacity`); the crossbow stays a weapon a
  build may hold under the weapon-fit rule (ITEMS.md 3). The sim tests that shot a crossbow
  build one from the frostweaver's kit;
- **blade**: `charge` for the dash, so its gap-closer is nobody else's. It keeps **Water**: a
  neutral blade (measured: 9 : 33 against the frostweaver) breaks the cycle of section 11,
  since nothing among the six but Water resists Water; the gun is the neutral build instead;
- **frostweaver**: Water + Air still (the Air is what makes it the ironclad's counter, Ground
  0.5x in, and the counter-pick's elemental score needs it: Water alone tied the blade and
  nobody re-specced), but the blink is gone (the blink and the vanish are the shade's) and so
  is the haste (the mender's): `frost_nova` alone. Measured against the blade on the gate's
  three seeds: nova alone 22 : 16 (1.38); nova + haste 17 : 14 (1.21); nova + leap 8 : 13;
- **musketeer**: **neutral** (`aspects = []`), `dash`, `leap`; the vanish is the shade's;
- the duelist brain (gm-bot): melee or ranged is the primary weapon's verb, not the frame's
  (a shade with a dagger closes in, a caster with a staff swings it); a charge is used from
  the distance it covers; the offline arena issues every firearm its rounds through
  `set_stacks` (the musketeer had one shot a life before: 3 kills in 48 fights).

Measured (six seeds × 60 s, 8 v 8, both sides; the row's share of the kills):

| | blade | frostweaver | ironclad | mender | musketeer | shade |
|---|---|---|---|---|---|---|
| blade | - | 58% | 27% | 100% | 52% | 61% |
| frostweaver | 42% | - | 94% | 79% | 64% | 58% |
| ironclad | 73% | 6% | - | 100% | 51% | 79% |
| mender | 0% | 21% | 0% | - | 0% | 9% |
| musketeer | 48% | 36% | 49% | 100% | - | 65% |
| shade | 39% | 42% | 21% | 91% | 35% | - |

The mender's row is the brain's, not the build's: a healer measured as a duelist heals
nobody and swings a staff at mail. The gate of section 11 on its three seeds: ironclad : blade
32 : 14 (2.3), frostweaver : ironclad 51 : 1, blade : frostweaver 22 : 16 (1.38; the bar is
1.3, and the leg sits at noise level: a blade with the dash measured the same). Open: a Fire
caster for the firebolt and the fireball nobody slots; Grass has no content; a mender
measured in a mixed team rather than as a duelist.

## 16. v5 (2026-10-09): the colossus's arms and the bellow

The director: "can you make some oversized sword for our colossus class and also shield,
so it looks a bit more appropriate. Maybe should have a some skill that affects certain
range and causes all hostile players to focus/target/look at him, some kind of rage aggro
skill?"

Done:
- **greatsword** (primary, 2; `props/greatsword.glb`, ours, 1.4 m of blade and a two-hand
  grip, held in the one fist like the hammer): 95 slash, a 120° arc of 96 u, 320/90/520 ms,
  1.1 s, four targets. The ironclad holds it; the **hammer** stays an ability and a weapon
  template any build may slot under the weapon-fit rule, but no preset's. Slash where blunt
  was: the ironclad now shreds cloth (×1.25) and glances off plate (×0.5) where the hammer
  did the reverse, and nothing in its kit ignores a magic shield any more;
- **the shield**: the off hand holds the prop of the build's **guard** ability (LOOK.md
  6.5, `Look.off`, protocol v17): `shield_wall` carries `prop = "shield"`, a round shield
  (`props/shield.glb`, ours) on `prop_l` with the left grip, raised in front in the guard
  stance;
- **bellow** (active, 8, no aspect, 12 s, 20 focus): a roar; every enemy within 256 u and
  in sight is **Taunted** for 2.5 s (section 8: its body is turned to the roarer and held
  there tick by tick, its view with it, and a bot's or a creature's mind fights the roarer),
  and the roarer takes Fortify 30% for 4 s for the blows it asked for. A control, its own
  kind under the diminishing returns (MODES.md 4.5). The **fortify** active stays in the
  content for any Ground build; the ironclad's kit is stomp + bellow (12 + 2 + 2 + 6 + 10 +
  8 = 40).

Stored ironclads keep the hammer and the fortify they were saved with (both abilities stand
at their places; a build the rules take is not repaired).

Measured (the gate of section 11, `ironclad_beats_blade_and_frostweaver_beats_ironclad`,
three seeds × 60 s, 8 v 8): ironclad : blade **31 : 10 (3.10)** (2.3 with the hammer);
frostweaver : ironclad 46 : 5 (9.20) (51 : 1 before: the blade's slash into cloth gives the
colossus a few kills the hammer's blunt did not); blade : frostweaver 22 : 16 (1.38),
unchanged. The gate passes. The duelist brain classes the bellow as a burst at the feet and
uses it in reach of an enemy; a taunted bot fights the roarer (gm-bot `nearest`, gm-ai
companion and creature minds).

## 17. v6 (2026-10-09): the shaman, the seventh preset and the first of Grass

The director, after the hammer left the ironclad (section 16): "it may be good if we
introduce some shaman wielding hammer, class that could attack, mostly AoE and heal"; then
"seventh preset could be grass that's currently missing"; then "it doesn't need mend, since
it's hybrid between atk and heal ... leather, secondary is hammer hit ground -> AoE kick
skill and sanctuary becomes some area grass that regens allies and bleeds enemies"; "grass
alone".

Done:
- **shaman** (caster, leather, Grass; STR 13 AGI 8 CON 14 INT 15 SPR 5; mode rpg): the
  **hammer** (its build again, no preset's since section 16), the **slam** as secondary
  (4: the hammer brought down, a 96 u circle at the feet, 30 blunt, knockback 300, stagger
  40, 4 s), brace, the **briar** (10, Grass: a patch of 140 u where it aims within 500 u,
  for 6 s; 8 grass a second on everyone in it, Regen 30/s on its own side in it; 35 focus,
  14 s) and the **entangle** (10, Grass: a circle of 112 u where it aims; 15 grass and Root
  1.5 s; 25 focus, 14 s). Kit 32 of 40. Its face: walls every caster in the roster (Water,
  Ground and Electric are 0.5x into Grass; the shade's Air alone is 2x) and is cut by steel
  (leather: slash 1.0, pierce 1.0);
- **`target = "allies"`** on a status (VOCABULARY.md 5.4; `StatusTarget::Allies`): the
  briar's Regen lands on the caster's side only, the caster among them, where its packets
  are friendly fire like every packet (VOCABULARY.md 8). The director asked for a patch that
  "regens allies and bleeds enemies"; Bleed is physical and would ignore the Grass of the
  matrix, so the patch ticks grass damage instead, and the only side a status may check is
  the one the Taunt already checks (section 8);
- the Keeper stocks the hammer where it stocked the crossbow (no preset's since section
  15); the bot's counter-pick: against Water the shaman now (Grass 2x in, Water 0.5x back)
  over the frostweaver, and a frostweaver re-specs to it; against Grass the shade.

**Measured** (`round_robin`, six seeds x 60 s, 8 v 8, both sides; the row's share of the
kills). The first cut, as the director wrote it with Regen 60/s and 15 grass a pulse, won
everything but the gun: 71% of the blade, 79% of the frostweaver, 65% of the ironclad, 63%
of the shade, 41% of the musketeer, a mean of 70%. One lever at a time, each against the
same seeds: cloth 66%, Regen 40 66%, STR 8 69%, the briar's and the entangle's damage
halved 67%, the root a slow 69%, the root on three 67%, no entangle 66%, a staff for the
hammer 57%, Regen 20 58%, **no briar 51%** (the blade 57%, the frostweaver 54%). The patch
is the build, and eight shamans keep a whole team under one Regen for the whole fight:
Regen is `refresh`, so eight patches heal like one, but one is always standing. Regen 30
with the softer damage:

| | blade | frostweaver | ironclad | mender | musketeer | shade | shaman |
|---|---|---|---|---|---|---|---|
| blade | - | 58% | 21% | 100% | 52% | 61% | 46% |
| frostweaver | 42% | - | 90% | 79% | 64% | 58% | 30% |
| ironclad | 79% | 10% | - | 100% | 70% | 82% | 52% |
| mender | 0% | 21% | 0% | - | 0% | 9% | 1% |
| musketeer | 48% | 36% | 30% | 100% | - | 65% | 72% |
| shade | 39% | 42% | 18% | 91% | 35% | - | 42% |
| shaman | 54% | 70% | 48% | 99% | 28% | 58% | - |

The shaman is even with the blade and the ironclad, walls the frostweaver (70 : 30), and
the gun cuts it (28 : 72); its mean is 59% with the mender's row, 52% without. Cloth moved
the blade's share by three points (49%) and was not taken: the director said leather. The
gate of section 11 is unchanged (ironclad : blade 37 : 11 (3.36), frostweaver : ironclad
51 : 3, blade : frostweaver 22 : 16 (1.38), mirror 26 : 26); the shaman is in no leg of it.

Open: the 8 v 8 mirror overstates an area heal (eight casters, one patch always up) as it
understates a healer (the mender's row is the brain's, section 15); a mixed team is the
measure of both. Fire still has no build.

## 18. v7 (2026-10-17): no armour class

The director, after the gear job put the class on the chest piece and the blade walked
into the town as a leather-tinted mannequin: "we can actually completely get rid of the
classes, model is independant of class, class was just preset build of build stats (sta,
str, def etc) but now that we allow any weapon to any model + weapon carries skills, we
are good", and "class is wrong name for what armor provides, it's like defense type that's
provided, either physical or magical".

- `ArmourClass`, `ArmourMods` and `kind_mult` are gone from `gm-core`; `Derived::compute`
  takes the attributes and the frame; `DefenderStats` has no class; the physical kinds
  multiply nothing but the gear's edge (ITEMS.md 3). The derived table (6) lost its
  armour terms: cloth's +0.05 ward and leather's +0.05 evasion went with mail's and
  plate's penalties.
- `Build.armour`, `Worn.class` and `Sheet.class` are gone (`PLAYER_VERSION` 13: a stored
  build's `armour` is ignored on read); `builds.toml` and `creatures.toml` name none; a
  template names no `class`.
- On the wire `SpawnInfo::Player` lost its two bits and a stall its keeper's class
  (`PROTOCOL_VERSION` 26, `HUB_VERSION` 19).
- The minds: a squad's "heavy" body is the colossus, not the colossus in mail or plate
  (`gm-ai`).
- The look: one official avatar a frame (`avatars.toml`: `frame` and `model`), the
  mannequin one grey, one portrait a frame, the gait's weight 0 for everyone
  (CONTENT.md 3.2, MODELS.md 9, LOOK.md 6.6); the K page lost its armour row and the
  equipment window's ribbon says `Blade | striker`.
- What stands: the presets as starting builds with their names (EQUIPMENT.md 2), the
  frames (3), the attributes (2), the aspects (5), and the armour's own numbers. **Next,
  the director's word**: frame-bound abilities ("thin one would have dash, big ones like
  charge or sth, wizardish ones would have something else"), a `frames` list on an
  ability in `abilities.toml` that `validate` reads; not built.
- The round robin after the change is to be run (11): the arena's presets fought in
  their classes, and the ironclad's plate is now its frame alone. **First number**: the
  cycle test (`gm-bot/tests/arena.rs`) found the blade's leg against the frostweaver
  near even (CI 19:18, the reference machine 25:18; it was a clear win), because that
  edge was slash 1.25 on cloth; the ironclad's over the blade (2.3–2.6) and the
  frostweaver's over the ironclad (3.0–3.6) stand. The test now asks that leg only not
  to be dominated; whether the sword or the frostweaver moves to make the cycle again is
  the director's.

## 19. v8 (2026-10-19): the softer matrix and the wider gear term

The first step of BODIES.md (the bodies, the gear skills and the caster's blast follow
it). The director's numbers, from the review "Gear and Body Redesign": the element matrix
**0.75 / 1.25** instead of 0.5 / 2, and the gear term widened so that the best gear wins
**1.5×** against nothing instead of 1.23×.

- `gm_core::matrix::MATRIX`: every 2 is 1.25, every 0.5 is 0.75 (`BEATS`, `RESISTED`).
  The table's shape stands (5): every element beats one, is beaten by one, none further
  than one step from even. One step is a 1.67× exchange; a dual-aspect hole 1.5625×, a
  wall 0.5625×.
- `Gear::SCALE` 2000 → **1100**, `Gear::MAX` 250 standing: one side is at most 1.227,
  the exchange at the very cap (1350/1100)² = **1.51**, the best craft (220) against
  nothing **1.44**, against iron and oak (40) **1.34**. ITEMS.md 3.1 and the item words
  (`gm-content::items::per_cent`) read `SCALE`; PLAN.md 0's "15–25%" is 50%.
- Principle 1 is rewritten: stats and gear move damage as much as counters do; a counter
  tilts a fight, it does not decide it. The attribute band (2×) is the largest lever.
- The arena's cycle test (`gm-bot/tests/arena.rs`) asked for legs of 1.3 and more; the
  legs narrow with the matrix. It asks now that every leg of the cycle still points the
  same way (ironclad over blade, frostweaver over ironclad, blade not dominated) and that
  the mirror is even. Measured on this change (three seeds × 60 s, 8 v 8, offline):
  ironclad : blade **49 : 19** (2.58; 2.3–2.6 before); frostweaver : ironclad **31 : 19**
  (1.63; 3.0–3.6 before); blade : frostweaver **23 : 25** (0.92; near even, as since v7);
  mirror 40 : 33. The cycle stands, the counter leg at half its old width: a Water
  caster still beats the Ground colossus, by play and by kiting now, not by the chart.
  The gate asks 1.2 on the two decided legs (1.3 before) and no domination on the third.

## 20. v9 (2026-10-20): no kit points, no trainer

The director, after the skills overview (the doc "Skills and Abilities Overview", with
Gemini 3.1 Pro as judge): "get rid of that NPC and KIT stuff, no kit for skills or such,
was idiotic in the first place, just wanted to do some tests with it." Decided with it:
a character's attacks are grouped **melee, magic, ranged** by its body (the Soldier's by
the mode: FPS ranged, else melee), with fists, a weak magical blow and the knife as the
empty hands of each; casting is the weapon's (anyone throws a staff's fireball, weakly
without INT); the element's affinity, the group and INT stack on a spell's damage; the
gun mode is called **FPS**. This section is the first step, the tearing out; the groups,
the weapons' spells and the stacking follow in BODIES.md 7.

- **No kit points.** `AbilityDef.cost`, `BUDGET`, `SECOND_ASPECT_COST`, `Build::cost`
  and `BuildError::Budget` are gone; `abilities.toml` rows carry no `cost`; the price
  bands of section 10 and the budget of section 9 are history. A build is a body,
  thirty attribute points, up to two elements and the mode; what it plays is the
  gear's and the body's. A player's build has no bought actives; the presets of
  `builds.toml` (the bots', the arena's, the creatures') keep theirs until BODIES.md 7
  moves them onto the weapons.
- **No trainer.** The creature is gone from `creatures.toml` and the town's map; the
  world zone refuses `FromClient::Respec` in words (`a build is set at creation: there
  is no trainer`); a team zone (the arena, the practice ground) still takes a preset
  at the next respawn for the arena's keys, the bots' counter-pick and the gates. The
  hub's `SetBuild` is gone (`HUB_VERSION` 22). The K page is a sheet, read (CLIENT.md
  4.5); the creation screen places every point or does not create (BODIES.md 6.7's
  "points in hand" is over: there is nobody to place them later). A character of
  today is a new character, by the director's word; the DB is the test harness.
- **FPS.** `Mode::Fps`, named `fps`; a stored or authored `gun` reads as it
  (`PLAYER_VERSION` 16). `PROTOCOL_VERSION` 29 (the content on the wire lost the
  cost), content v24.
- Section 9.1 (the trainer, the respec) and the budget lines of 9 and 10 are
  superseded by this section; the dummies and the arena rule stand.

## 21. v10 (2026-10-20): the groups, the weapons' spells and the stacking, measured

BODIES.md 7 as built in five steps after section 20: the attack group of the body
(7.1), the weapon's spell (7.2), the affinity and the group in the pipeline (7.3, the
`body` term of section 7), the hotbar from the kit (7.4), the presets as bodies (7.5).

**Measured** (`gm-bot/tests/arena.rs`, three seeds × 60 s, 8 v 8, offline, the presets
as bodies in nothing): ironclad : blade **47 : 21 (2.24)** (2.58 in v8); frostweaver :
ironclad **40 : 23 (1.74)** (1.63); blade : frostweaver **17 : 36 (0.47)** (0.92 in v8,
13 : 25 after the staff's second move); mirror 42 : 31. The gate passes: the two
decided legs point the same way and the blade is not wiped out. What moved it: the
frostweaver is a Sorceress now, so its shards and its nova are ×1.25 affinity × 1.25
magic on top of INT 20, and the ironclad's stomp went to the staff while its bellow
and fortify are ×0.75 as a melee body's spells. The blade's leg against the
frostweaver is the one to tune, as it has been since v7: the director's numbers for
`AFFINITY` and `group_mult`, or a Water blade with a spell of its own.

## 22. v11 (2026-10-20): the numbers a tenth of what they were

The director: "we have 700-1500 hp and dealing damage 30-200 ... if we cut it, would be
70-150hp, damage 3-20, which is much better, easier to read and calculate." Everything
that is a point of health is a tenth of v10, and nothing else moved:

- health `50 + 4·CON` (70 → 150; the test sheet's 900 is 90); creatures a tenth (sentinel
  120, Warden 1,500, dummy 500, pup 100; `MAX_CREATURE_HEALTH` 6,000);
- every `damage.amount` a tenth, rounded, never below 1 (the sword 6, the hammer 9, the
  musket 11, the dagger 4, the shaman's spores 1); the pipeline's `max(1, round(…))` is
  unchanged, so a 5-point blow is 4.8 rounded, and the gear term moves it by a point;
- the kit heals 30; the trials ask for 35 damage and 50 healing;
- **the pulses are one a second** (`DOT_PULSES_PER_S` 1, was 4): a bleed or a burn of 1
  is a point a second, where four pulses of a quarter would have been floored to nothing.
  Bleed 1, burn 1 (the fireball's 12 and the catalyst's 3 both land on 1), Mend 10/s,
  Sanctuary 6/s, the shaman's spores 3/s. A pulse still takes the matrix and the wards.

Stamina, focus, stagger, knockback and the per-mille gear edges are what they were: they
were never points of health.

## 23. v12 (2026-10-22): HP, MP and ST, and only the primary is free

The director, the morning after v11: "we reduced HP by factor of 10, but now we have more
stamina and MP than HP, so we need to reduce them as well. Then we make sure that our
non-primary/secondary skills use MANA, or MP, only primary is infinite... let's clearly call
it HP (health points), MP (magic points), and ST (Stamina). Also make sure CON stat increases
both HP and DEF. Pressing button C should open character sheet where we see equipment and all
stats."

- **The pools a tenth**: ST `6 + 0.3·(CON + AGI)` (9 → 21), regen `1 + 0.06·AGI`; MP
  `4 + 0.5·INT + frame` (6.5 → 16.5, the Caster's frame +2), regen `0.5 + 0.06·SPR`. The
  guard's `stamina_per_hit` a tenth (the shield wall 1, the brace 1), the parry 1 ST. The
  frames' percents (GEAR.md 6: a whalebone frame's blows at 75%) stand, on the block alone
  now.
- **Only the primary is free**: every primary's cost is gone (the crossbow's 30 ST a span
  and the bow's 20 a draw of GEAR.md 11 included: the director's word was "only primary is
  infinite"; their spans stay the limit). Every secondary, active and weapon spell costs MP,
  a tenth of what it cost, rounded, never below 1: a dash 3 (was 30 ST), a leap 2, a charge
  3, a blink 3, a fireball 4, Mend 1, the shaman's briar 4, a flame burst 2. The body's
  tricks (dash, slip, overhead, the war standard) moved from ST to MP with the rest. What
  this does to a low-INT body's dashes (an MP of 8 is two dashes, then a wait of a second
  and a half each) is the director's to play and re-tune; the ratios between the skills are
  what they were.
- **ST is the guard's**: it is spent by a block's hit and a parry and nothing else. A
  `Cost.stamina` on a non-guard ability is not refused by the loader, so a future skill may
  still tire the body; none does today.
- **The names**: the HUD's bars say `HP`, `ST`, `MP`; the K page's and the selector's words
  say `3 MP`, `1 ST a hit`; the attribute rows say what CON buys as `HP 86 DEF 23%` (the
  armour is DEF to a player; `armour` in the code and the matrix above). The code keeps
  `focus` for MP and `stamina` for ST: a rename across the content, the protocol and the hub
  would move the versions for words.
- **CON buys HP and DEF**: it always did (`armour = 0.015·CON + frame`); now the row says
  so.
- **The character sheet, `C`** (EQUIPMENT.md 4.6): the equipment with every stat beside it,
  in every mode. `C` was the key held beside Ctrl (the action mode's guard, the gun mode's
  crouch; WEB.md 3.4: a browser keeps Ctrl+W): **`X`** is that key now, on the hotbar's
  guard cell and the touch hotbar too. `K` still opens the build's page.
## 24. v13 (2026-10-22): the skills rework

The plan is docs/SKILLS.md; each step adds its numbers here.

- **Step 1, fixes.** The pause after a stamina spend, the leap's lift, Freeze and the Chill
  immunity are counted in the zone's rate (they were 64 Hz ticks everywhere: the pause
  measured 3.25 s in the 20 Hz town, 1.0 s in the dungeon and the arena). A taunt sets the
  taunter's threat once, to the top of the table × 1.3 (it was added every frame of the
  mind: 64 a second in the dungeon). The bow costs 2 ST a draw and the crossbow 3 a span
  again: section 23 had made them free with every primary.
