# Bodies, gear skills and the softer matrix (v1, 2026-10-19)

Status: **taken in words** ("everything seems ok to me", 2026-10-19) and **being built**;
section 6 says what stands as built and where it departs from the plan. The review it comes from is the artifact "Gear and Body Redesign" (v2, with the
eight concept sheets); this is its implementation plan. It stands on GEAR.md 2 and 12
(the weapon brings the primary, there is no armour class), MATRIX.md 2, 3, 5, 6 and 9,
ITEMS.md 3 and PLAN.md 0. It moves three decided things on purpose and says so in 1.

## 1. What changes, in one paragraph

A player no longer picks a preset. They pick a **body** (eight, each a frame, five
attribute floors and one body skill), put thirty points on the attributes, and pick the
element, all on the creation screen. The weapon in hand brings the primary **and the
secondary**; the off hand brings the guard and overrides the secondary where it holds one;
the body skill fills the first active; points buy spells and a second element and nothing
else. A caster weapon's bolt and blast are the body's element, so a mender with a staff and
Electric has Spark and Thunderclap for free. Armour carries a **base mitigation** of its
type. The element matrix softens to **0.75 / 1.25** and the gear term widens to **1.5×**
against nothing. The three decided things that move: MATRIX.md principle 1 (counters no
longer move damage more than stats; the attribute band is the largest lever), PLAN.md 0's
25% gear edge (50%), and MATRIX.md 9's presets (bots' and creatures' only).

The director's reading of the mender after this: it does not trade staff blows with a
brute; it is thin and ranged, and wins by execution, kiting a big slow body with elemental
bolts. The arena's brain plays every build as a duelist (MATRIX.md 15), so the measured
mender row will stay low until the brain kites; the number to read after this job is the
mender's share with a kiting brain, or a human's, not the round robin's.

## 2. The constants (step 1, half a day)

- `gm_core::matrix::ELEMENT_MATRIX` (matrix.rs 75): every 2.0 → **1.25**, every 0.5 →
  **0.75**. `element_mult` and `Aspects::mult_against` are unchanged. The tests that
  assert a 4× hole or a 0.25× wall assert 1.5625 and 0.5625. MATRIX.md 5's table is
  rewritten with the same shape (every element beats one, is beaten by one, none further
  than one step from even) and principle 1 becomes: *stats and gear move damage as much as
  counters do; a counter tilts a fight, it does not decide it.* Logged as MATRIX.md v8.
- `Gear::SCALE` (matrix.rs 381): 2000 → **1100**. With `Gear::MAX` 250 standing, top
  against nothing is (1350/1100)² = 1.51, top against iron-and-oak (1350/1140)² = 1.40.
  ITEMS.md 3.1's bounds and the item words' scale (`e / 20` per cent, `100·e/(2000+e)`)
  are recomputed from `SCALE`, not retyped. PLAN.md 0's "25% at most" becomes 50%.
- The arena's cycle test (`gm-bot/tests/arena.rs`) asks for legs of 1.38–3.4 today; after
  the matrix the legs narrow to around 1.5. The test's gate becomes "every leg of the cycle
  still points the same way and the mirror is even", with the measured numbers written
  into MATRIX.md 18.
- Shipped to the LAN stack alone, the same evening, so the director feels the softer
  counter before the bodies land on top of it.

## 3. Bodies (step 2, two days plus the models)

### 3.1 Content: `bodies.toml`

```toml
[[body]]
key = "brute"
name = "Brute"
frame = "colossus"
floors = { str = 7, agi = 3, con = 9, int = 3, spr = 3 }   # sum 25
skill = "charge"                                           # the body's active, free
stamina = 100                                              # percent of the pool; the whelp 115
avatar = "brute"                                           # models/avatars/brute.glb
blurb = "A big man in his thirties. Takes and gives blows. Slow, and no head for magic."
```

The eight and their floors are the review's section 2 table (brute, veteran, soldier,
huntress, sorceress, elder, whelp, thief). The content validates: floors sum to exactly
25, each in 3..=25, the skill is an `active` with no `aspect`, the frame exists, the avatar
file exists when a `models/` dir is given (the mannequin stands in otherwise, as today).
`avatars.toml` gains a row per body (`body = "brute"`) and the frame row stays as the
fallback for a body without a model. Content version +1.

### 3.2 Core: the floor is the body's

- `Attributes::MIN` 5 → **3** and `FREE_POINTS` stays 30. `Attributes::cost` counts
  points above the body's floors, not above `MIN`: `Attributes::spent(&self, floors)`.
  The flat `MIN` stays only as the range check.
- `Build` gains `body: Option<u16>` (a pack index; `None` for a creature, a bot preset and
  every stored build from before). `Build::validate`: with a body, each attribute ≥ its
  floor and the sum above the floors ≤ 30; without one, as today. `Build::repaired` lifts
  each attribute to the floor first, then scales the rest down to 30.
- `Build.frame` is read from the body when there is one (`Build::frame(pack)`); the stored
  field stays for the bodiless.
- The kit (3.4) counts the body skill as active 1 at cost 0; `Build::cost` skips it.
- `PLAYER_VERSION` +1: a stored build without a `body` reads as before; the hub offers
  the body screen to such a character on its next login (3.5).

### 3.3 Presets

`builds.toml` stays, for `gm-ai`, the creatures, the arena and the Keeper's spar bots. The
hub's `New character` no longer takes a preset name; it takes a body key, five attributes
and the aspects (3.5). `showcase.rs`'s "a build is named by the preset it is" (line 94)
becomes "a build is named by its body" (`Brute | colossus`), and the selector's ribbon
and the equipment window's ribbon say the body.

### 3.4 The kit from the hands and the body

`Build::kit_slots(worn)` (build.rs 289) today: the worn primary in place of the build's,
the worn guard in place of the build's. After:

| slot | from | fallback |
|---|---|---|
| primary | the right hand's weapon (`Worn.primary`) | the build's (empty hands) |
| secondary | the left hand's item if its template names a `secondary` (shield → `shield_bash`, pistol → `pistol`, focus → `mend`); else the right hand's weapon's `secondary` (3.6) | `kick` |
| guard | the left hand's (`Worn.guard`); a two-hander → `brace` | `parry` |
| active 1 | the body's `skill` | the build's first active (bodiless) |
| actives 2–3 | the build's `actives` | |

`Worn` gains `secondary: Option<u16>` and `worn_of` (gm-content worn.rs 31) fills it from
the templates' new `secondary` key (items.toml: `secondary = "shield_bash"` on the three
shields, `"pistol"` on the pistol, `"mend"` on the focus). `Build.secondary` and
`Build.guard` stay stored and are what plays in nothing (creatures, companions, bots); the
K page stops offering them to a player with a body. `PROTOCOL_VERSION` +1 for the field in
`FromZone::Worn`.

### 3.5 The hub and the screens

- Hub: `CreateCharacter { body, attributes, aspects, actives }` replaces the preset form;
  `HUB_VERSION` +1. A character stored without a body gets `body = None` and the client
  shows the body screen before the town, once; choosing writes the body and repairs the
  attributes to its floors. Migration: none on the rows (the body is in the build JSON).
- The selector (`showcase.rs`): the new-character screen is the eight bodies in the
  3D selector, each in its avatar, the ribbon saying frame, floors and skill; then the
  attribute screen (the K page's attribute rows with the floors greyed as the budget's
  rows are greyed today, `why_not`: "the Brute's floor"); then the element screen (the
  six, with what each casts and is weak to, from the matrix); then spells.
- The K page: attributes, elements, spells. The primary, secondary and guard rows go;
  the equipment window says what the hands bring, as it does today for the primary.
- The trainer respecs all of it but the body (a body is a new character).

### 3.6 The caster weapon's two moves (step 3, a day)

An ability row may say `secondary_of = "staff"` as a primary says `prop`; `worn_of` picks
the secondary for the right hand as it picks the primary (GEAR.md 2: the build's stored
one if it has that prop, else the first whose aspect the build holds, else the plain one).

| prop | element | primary (built) | secondary |
|---|---|---|---|
| staff, wand | Fire | firebolt | **flame burst** (new): cone 60 fire, 2.5 s, 15 focus, Burn |
| | Water | ice shard | **frost nova** (moves from the actives; `slot = "secondary"`) |
| | Grass | **thorn** (new): bolt 45 grass, Bleed 2 s | **briar burst** (new): cylinder 40 grass, Root 0.8 s |
| | Electric | spark | **thunderclap** (moves) |
| | Ground | stone throw | **stomp** (moves) |
| | Air | shadow dart | **gust** (new): cone 30 air, knockback 300 |
| | none | staff blow | **sweep** (new): arc 30 blunt, stagger 40 |
| hammer | | hammer | slam (exists) |
| sword, katana, axe | | the blow | **pommel** (new): 25 blunt, stagger 50, 1.5 s |
| dagger | | dagger | throwing knife (exists) |
| the rest | | the blow | kick |

The wand's six are the staff's with `projectile` shapes (narrower, faster, 0.8×). Frost
nova, thunderclap and stomp leave the preset actives: the frostweaver's and the mender's
and the ironclad's `actives` lists in builds.toml lose them and the budget they cost
(10 each) is left unspent or spent on a spell that stays. Every number is tuned at the
dummy before it ships (MATRIX.md 9.1).

### 3.7 Base mitigation on armour (step 5, half a day)

A template may say `armour = 0.15` or `ward = 0.15` (and `speed = 0.92`, `regen = 0.8`).
`Worn` gains the summed `armour`, `ward`, `speed`, `regen` of the six places (chest whole,
head and legs a third, the rest nothing); `Derived::compute` adds them before the caps
(0.50 and 0.60). The client predicts speed and regen, so they ride in `Worn` (protocol).
Numbers: robe ward +0.15; cuirass armour +0.08; mail shirt +0.15, regen ×0.9; plate
+0.22, ward −0.05, speed ×0.92, regen ×0.8; hood ward +0.05; helms armour +0.05;
trousers +0.05.

### 3.8 Techniques (step 6, a day)

A second primary per prop for sword (thrust), hammer (sweep), bow (volley) and dagger
(backstab), as rows; `worn_of` already takes the build's stored primary when it has the
prop, so the K page's primary row returns as "technique" listing the primaries of the
weapon in hand. No engine work.

## 4. Order, versions and gates

| step | what | versions | gate |
|---|---|---|---|
| 1 | the matrix 0.75 / 1.25, `Gear::SCALE` 1100, docs | content | arena cycle test rewritten; round robin logged in MATRIX.md 18 |
| 2 | bodies.toml, floors, body skill, hub create, the three screens | content, player, hub, protocol | the look gate walks each body; hub tests create by body; `Build::repaired` tests lift to the floor |
| 3 | the caster weapon's secondary, the six blasts, the three spells moved | content, protocol | dummy readings of each blast; the mender in the arena with a kiting brain |
| 4 | the off hand's secondary, the body skill in active 1 | protocol | hub tests wear a shield and read the bash |
| 5 | base mitigation | content, protocol | dummy: a brute in plate at CON 20 reads 50% |
| 6 | techniques | content | the K page lists two for a sword |

Each step its own PR on `main`, shipped to the LAN stack as it merges, the director
playing every one before the next. The eight models are a parallel job: the concept
sheets are the briefs; until a body has a mesh its frame's avatar stands in.

## 5. What the director decides before step 2

1. The eight floors as proposed, or his own numbers (they must sum to 25).
2. The Whelp's stamina 115%, or a floor only.
3. A character from before the change: the body screen once at login (recommended), or
   every old character becomes a Soldier.
4. Frost nova, thunderclap and stomp as the staff's secondary only (recommended), or
   kept as spells too for a caster with a sword in hand.

## 6. As built (2026-10-19)

The director's four decisions (5): the floors as proposed; the Whelp's stamina 115%; the
three moved spells as the staff's secondary only; and for the two body skills that carried
an aspect (the Elder's fortify, Ground; the Thief's vanish, Air) **aspectless skills
instead**: the Elder's is **sanctuary**, the Thief's a new **slip** (a three-second vanish
of no element, 20 stamina, 10 s). On the characters from before: his word was that they
may all go, so nothing asks an old character for a body; one plays on as the preset it is
(a bodiless build keeps the flat floor of five) and a new character is made with a body.

### 6.1 Step 1, the constants (MATRIX.md 19)

As planned: `MATRIX` 0.75 / 1.25 (`BEATS`, `RESISTED`), `Gear::SCALE` 1100, the words and
the bounds recomputed from the constant, MATRIX.md principle 1 rewritten as v8, PLAN.md 0
and ITEMS.md 3.1 at 50%. Measured on the reference machine: ironclad : blade 49 : 19
(2.58), frostweaver : ironclad 31 : 19 (1.63, from 3.0–3.6), blade : frostweaver 23 : 25,
mirror 40 : 33. The arena gate asks 1.2 on the two decided legs; the bot's counter-pick
margin is 1.2 (one matrix step and a little) where it was 1.5.

### 6.2 Step 2, the bodies

- **Content**: `bodies.toml` (3.1), eight rows: `key`, `name`, `frame`, `floors`,
  `skill`, `stamina` (default 100), `blurb`. No `avatar` field: only the striker has an
  official model today, so a body's look is its frame's (CONTENT.md 3.2) until the eight
  models exist. Validated by `ContentPack::validate`: floors in 3..=25 summing to 25, the
  skill a plain active (no aspect, no creature's), stamina above zero, and a build of the
  body in each mode validating with nothing bought. The fixture pack mirrors the eight.
  Content v22.
- **Core**: `Attributes::MIN` 3, `BASE` 5 (the bodiless floor), `FLOOR_SUM` 25,
  `spent(floors)`, `at_floors`. `Build.body: Option<u16>` (serde default `None`: every
  stored build reads), `BodyDef` in `ContentPack.bodies`. `Build::validate` with a body:
  the frame is the body's, every attribute at or above its floor, thirty points above
  the floors, at most three bought actives (the body skill is the fourth). `Build::cost`
  with a body counts the aspects and the bought actives only: **the hands cost nothing**
  (3.4, step 4's rule, taken here because a build must validate on creation). The kit
  (`kit_slots_in`) puts the body skill first among the actives; `Kit::from_build`,
  `check_kit` and `squad_capacity` read it there. `Build::of_body` makes a build of a
  body with the pack's **bare hands** (`ContentPack::bare_hands`: the cheapest plain
  primary of the mode, the kick, the parry; a gun build's firearm and no guard): what
  plays in nothing until something is worn. `Build::repaired` lifts to the floors first.
  `derived_in(pack)` scales the stamina pool by the body's percent; `Sheet` uses it.
- **Wire**: `BuildChoice::Body(BodyChoice { body, mode, attributes, aspects, actives })`
  (protocol v28, `HUB_VERSION` 21, `PLAYER_VERSION` 15). The hub's `CreateCharacter` and
  `SetBuild` and the zone's `Hello` and `Respec` take it; `Preset` stays for the bots,
  the arena's F-keys and the scripts. The trainer respecs everything but the body: a
  respec that would change a bodied character's body is refused in words.
- **Screens**: the new-character screen is the eight bodies in the selector (tabs by key,
  the body's name over the doll, its blurb under it, the floors plus six each on the
  left, the bare hands and the body skill on the right); **Next** opens the build
  editor on the draft (the K page's: the body's row with the mode, the attributes with
  `-` stopping at the floor and the floor said beside the row, the six elements, the
  spells up to three; the weapon, secondary and guard lists are gone for a body and a
  paragraph says what the hands bring); **Create** sends the `BodyChoice`. The plan's
  three screens after the body (points, element, spells) are one editor here: the K
  page already had the three and the room. The K page in the world is the same editor;
  a build is named by its body (`Soldier  |  striker  |  no aspect  |  action mode`) in
  the selector, the characters' list and the equipment window's ribbon.
- The gates' UI scripts make their characters as a Soldier (`click soldier`, `click
  Next`, `click Create`).

### 6.3 Step 3, the caster weapon's two moves

- `secondary_of` on an ability row (CONTENT.md 3), the secondary's counterpart of a
  primary's `prop`; `worn_of` picks the right hand's second move as it picks its primary
  (the build's own among the holders, else the first whose aspect the build holds, else
  the plain one). `Worn.secondary` rides to the client; the kit plays it over the build's
  (`kit_slots`). The wand holds the staff's six blasts (`secondary_of = ["staff",
  "wand"]`); the wand's own narrower bolts of 3.6 are **not built** (the wand's one blunt
  bolt stands), a content job for later.
- Moved: `frost_nova`, `thunderclap`, `stomp` are secondaries now (`slot =
  "secondary"`), the staff's and the wand's. New: `thorn` (Grass's bolt, 45, Bleed 2 s),
  `flame_burst` (Fire: a cone of 60, Burn), `briar_burst` (Grass: a cylinder of 40, Root
  0.8 s), `gust` (Air: a cone of 30, knockback 300), `staff_sweep` (no element: an arc of
  30 blunt, stagger 40), `pommel` (a sword's, a katana's and an axe's: 25 blunt, stagger
  50, 1.5 s). The slam is the hammer's, the warhammer's and the mace's; the throwing
  knife the dagger's. Numbers are the review's, **not yet tuned at the dummy**.
- The presets: the frostweaver stores the nova as its secondary (what plays in nothing)
  and takes blink; the mender keeps sanctuary and haste; the ironclad takes fortify
  beside the bellow. A creature may hold a secondary among its actives (the Warden's
  stomp), as it may a second primary.
- **Measured** (the arena, three seeds × 60 s, in nothing): ironclad : blade 50 : 16
  (3.1), frostweaver : ironclad 25 : 12 (2.1), blade : frostweaver **13 : 25**: the
  frostweaver has the nova as a second move now, used in reach every eight seconds, and
  the third leg turned. The gate asks only that the blade is not wiped out; the leg is
  the director's to tune.

### 6.4 Step 4, the off hand's second move

`secondary` on a template (CONTENT.md 3): `shield_bash` on the three shields, `pistol`
on the pistol (the gun mode only), `mend` on the focus (its aspect none, so any body
mends). `worn_of` takes it over the weapon's own. The body skill in active 1 was step 2's.

### 6.5 Step 5, base mitigation

`armour`, `ward`, `speed`, `regen` on an armour template, the review's numbers (robe ward
+0.15; cuirass armour +0.08; mail shirt +0.15, regen ×0.9; plate +0.22, ward −0.05, speed
×0.92, regen ×0.8; hood ward +0.05; the three helms armour +0.05; trousers +0.05). `worn_of`
sums the chest whole and the head and the legs a third into `Worn`; `Build::derived_wearing`
adds them before the caps and multiplies the speed and the regen; the sheet is built from
it on both ends. **Not yet read at the dummy** (the gate: a brute in plate at CON 20 reads
50%).

### 6.6 Step 6, techniques

Four rows: `thrust` (the sword's point, 70 pierce, reach 84, one target), `hammer_sweep`
(55 blunt, arc 140, five targets), `volley` (three arrows of 30, spread 2, 35 stamina),
`backstab` (the dagger's one heavy stab, 55 pierce, unparryable; the review's ×3 from
behind is not in the vocabulary, so it is a stab). `Worn.techniques` lists what the weapon
in hand can do; the K page offers them as the weapon's technique and the build's stored
primary is the one that swings (GEAR.md 2 already took it).

### 6.7 The creation screen, second pass (2026-10-19, evening)

The director played 6.2's screens and said: "Instead of having base attribute points
that we should add to freehand after model (base char) selection, we are capped. And
skills are totaly weird and broken, shouldn't really offer any skills to select at char
selection, and primary skill staff weapon wth, it's broken totally." Found behind it: the
draft spread the thirty points six each before the editor opened (so it opened at "0 of
30 left" with every `+` off), `bare_hands` took the cheapest plain primary of the mode,
which is the staff at cost 0, the spells list was every active (the white rows other
bodies' free skills), and the mode's chooser was drawn in a row a button tall where it
needs a label's line over its buttons. His decisions: a weak empty-handed punch and a
knife for the gun mode; neutral stays; the elements as pictures on a wheel, one or two
picked, what each does and is weak to on the pointer; the points start at the floors
("brute can't go below 7 con"). Built:

- **The bare hands**: `fists` (a two-blow chain, 18 and 22 blunt at 48 u, no prop: the
  fist is the grip) and `knife_hands` (the knife of MODES.md 3.7 held as the weapon,
  `gun_melee = true`: MODES.md 2's rule is now *a firearm, or a `gun_melee` blade*).
  Appended to abilities.toml (stored builds name abilities by position); `bare_hands`
  finds them by key and falls back to the old rule in a pack without them. Content v23.
  The fixture pack mirrors both.
- **The points**: the draft is `Build::of_body` at the floors, thirty in hand; the
  selector says `+ 30 points to place` under the floors; the editor's head reads `30 of
  30 points to place`; `-` stops at the floor and says so, `+` at the cap or when all are
  placed. Two buttons under the rows: **Suggested spread** (the thirty laid along the
  floors, the remainder to the highest) and **Back to the floors**. Create takes a draft
  with points in hand (the trainer places them too) and says how many are left.
- **The wheel** (`character::element_wheel`, `Ui::picture_box`): six pictures on a ring,
  Fire at the top and clockwise in `Element::ALL`'s order; a click takes an element or
  lets it go, a third is off ("two at most: let one go first"); the middle says the picks
  and the cost; the pointer on one reads its line, what the pack lets it cast, what it
  hits harder and weaker, what it resists and takes more from (from the matrix). The
  pictures are `icons/elem_<element>.png` at the four densities, made with the Gemini
  image model (`gemini-3.1-flash-image`) from one prompt each and packed as icons of no
  row (`gm-tools content build` takes them when the files are there; a pack without them
  draws the element's initial).
- **The creation editor** (`character::creation_editor`): the body's row with the mode
  (a field's height: the label over the three buttons), the attributes on the left, the
  wheel and a paragraph on the right (what the empty hands are, what the hands bring,
  the body skill, "spells are bought at the trainer in town"). No spells, no lists of
  the hands. The K page and the game master's page keep the full editor with the
  aspects as check boxes (the wheel needs more height than the body column has there).
- The characters list says where a character is without naming its body again (the
  ribbon over it does).

Decided 2026-10-20 (MATRIX.md 20): there is no trainer and nothing is bought, so a
body's skill is the body's alone by construction; the groups (melee, magic, ranged), the
weapons' spells and the stacking are section 7, the next job.

