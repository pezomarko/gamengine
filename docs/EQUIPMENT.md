# Equipment: the places, the hands, the character window (v0, PROPOSED 2026-10-13)

Status: **proposed 2026-10-13 and taken** (the director: "now this seems like a good
plan, please implement it"); **built in full by 2026-10-14**: section 10 says what is
built, step by step. The sections above it stay as proposed, with the review's
corrections. Written the evening of 2026-10-13 after
the director played the lean hero with three places (ITEMS.md 2, LOOK.md 6.5 and 6.6):
"our blade class can't equip greatsword or 2 swords? can we at least have some char/class
names and show restrictions on equipables then? also change how inventory looks, we need
better representation of character and equipped items, like separate from main window we
could have char preview and slots in it to equip things. Then lay down proposal of all
equipment types."

It stands on ITEMS.md (what is worn, the gear term, the screens), LOOK.md 4 and 5 (grids,
the paperdoll), CONTENT.md 3 (props, overlays), MODELS.md 2 (the rig) and MATRIX.md 3, 4
and 15 (frames, armour class, the seven presets). PLAN.md 0 (gear is an edge, never a tier:
25% at most) and 3.4 (gear is unbound and un-classed) are decided and this document does
not move them. Section 8 is the review log; section 9 is what the director decides.

## 1. Why the blade holds one sword and nothing larger (as built)

Two different things, and only one of them is a bug.

**The greatsword is refused by the weapon-fit rule** (ITEMS.md 2, `gm_hub::hub::hands`,
`economy::fits`): a weapon is worn only when an ability of the character's build has the
template's `model` as its `prop`. The blade's kit (builds.toml) holds the Sword (prop
`sword`); its kick, parry, charge and overhead hold nothing. The Greatsword ability (prop
`greatsword`) is in the ironclad's kit and no other. So the hub says "this build's hands are
for the sword: not a greatsword", and the bag offers no Wear. The rule is the **kit's, not
the class's**: a striker who slots the Greatsword ability as its primary on the K page
(2 points, any frame may) holds one. The director asked for it this way on 2026-10-08
("my musket can equip a staff? make it say it can't and disable it").

The hands of the seven presets, as the rule reads them today:

| Preset | Frame, class | Hands (the props of its abilities) |
|---|---|---|
| blade | striker, mail | sword |
| ironclad | colossus, plate | greatsword, shield |
| shade | infiltrator, leather | dagger |
| frostweaver | caster, cloth | staff |
| mender | caster, cloth | staff |
| shaman | caster, leather | hammer |
| musketeer | striker, leather | musket, pistol, dagger |

**Two swords work since this afternoon's commit (20c3a87, in the play stack's 15:36
binaries), but only one way**: the second sword must be **dragged onto the small cell
named `off`** at the right edge of the inventory. The **Wear** button always puts a sword
into the right hand, so Wear on the second sword swaps it for the first; nothing says a
left hand exists; the `off` cell is one 36-dot slot in a column of three with the word
`off` in small print. The hub tests it (`gm-hub/tests/items.rs`, the plain sword into the
left hand) and the bag tests it (`bag.rs`, the drop on `off`), and a person cannot find
it. That is the screen's failure, and section 4 replaces the screen. One rule changes with
it (4.4): Wear puts a one-handed weapon into the **empty** hand first.

What the second sword is worth today: its edge under the one cap (two iron-and-oak swords
deal 4.0% more slash instead of 2.0%) and its look; no second blow, no second chain. That
stays until an ability holds "a sword in each hand" (6.3).

## 2. Classes have names; the character and the item say them

A character is a **build** (MATRIX.md 9: any frame, any class, any kit under 40 points),
and seven builds are **presets** with names (MATRIX.md 15 and 17). The name is the class.
What is missing is that nothing in the game calls a character by it and nothing on an item
says whose hands it is for. Proposed:

- **The class line.** Everywhere a character is shown with its build (the selector's
  `striker in mail | water | action mode`, the character window of 4, the K page's title,
  the party frames' tooltip), the preset's name leads it, in the title face, with its
  first letter up: `Blade`, `Ironclad`, `Frostweaver`, `Shade`, `Mender`, `Musketeer`,
  `Shaman`. A build that is no preset says `Custom` and, when every ability of a preset's
  kit is in it, `Custom (a Blade)`. The hub names it (`CharacterSummary.class: String`,
  the zone's roster the same), so the client works nothing out and a renamed preset
  renames every character. The build line (`striker in mail | water | action`) stays under
  it in the body face.
- **"Hands" on every weapon.** The hub already says `fits` and the words "this build's
  hands are for the sword: not a greatsword" (ITEMS.md 2). It adds a line that is true for
  everyone and does not depend on who asks: `hands: Blade, Ironclad` (the presets whose
  kits hold the template's model, by `hands()` over the preset list; a model no preset
  holds says `hands: no preset's (any kit with the Hammer)`). The line is in `does` as the
  words are today, so the tooltip, the stall, the storage and the trade all carry it, and
  nothing new goes on the wire. Armour, shields, parts and stacks say nothing: they fit
  everyone (PLAN.md 3.4).
- **A thing that does not fit looks it**: its icon is drawn dim with the off-colour rim
  (`RowMark::Off`'s colour, the grey of a button that is off) in the bag, the storage and
  at a stall, and the equip slot it is dragged over does not light. A drop of it on a slot
  **says why** on the notice line (the hub's words, known already from `fits`; no round
  trip): today the drop is silently nothing, which the director read as "can't equip".
- **The archetype page** (CLIENT.md 4.3) says the class's weapons in its blurb line:
  `Blade | sword, or two | mail`. The words are made from the preset's hands and class by
  the client from the content it already has.

Not proposed: binding an item to a class (`PLAN.md 3.4` decided against it, and the rule
of ITEMS.md 2 already gives the director what he asked for with no bound item: the same
sword sells to every build that swings one).

## 3. The hands: one-handed, two-handed, either

The content knows `held = right | left | either | back` (CONTENT.md 3.1) and says
two-handed holds are "a later phase". Proposed, the fifth value and the rule for it:

- **`held = "two"`**: the weapon takes **both hands**. Worn, it empties the left hand
  (what was there goes back to the bag as a Wear does; the hub refuses it when the bag is
  full, in words), and the `off` slot shows the two-hander's icon ghosted with `both
  hands` under it and takes no drop. A shield or a second weapon worn afterwards takes
  the two-hander off (one rule both ways: the hub's `wear` does it, as a Wear takes off
  what was in the place).
- **Its edge counts for both hands** under the one cap of 250: the right hand's whole and
  the left hand's half (5.5), 1.5 of its edge, exactly what a sword in each hand comes to.
  A two-hander of iron and oak deals 3.0% more; so do two such swords. A proposed number
  (9); the alternative is that it counts once and the ability's larger blow is the whole
  of its reason, which is where it stands today.
- **The greatsword stays in one fist, for everyone.** The first draft made it two-handed
  for every frame but the colossus; the review (8.1, 5) is right that an item which takes
  both hands of one body and one hand of another is gear that behaves by the wearer's
  frame, which PLAN.md 3.4 decided against. So the row stays `held = "right"` as built
  (MATRIX.md 16: held like the hammer, beside the shield; ironclad : blade 3.10 as
  measured), "too big for anyone else" is the fit rule's doing (only a kit with the
  Greatsword ability holds it) and the story's, and the **claymore** (7: the weapons pack
  has one) is the two-hander for a striker who wants one: the same ability's blow, both
  hands, no shield.
- The two-handed **hold** (both fists on the grip, the prop on `prop_r`, the left hand
  posed to it) is Phase 17's animation work (CONTENT.md 3.1 said so); until then a
  two-hander is drawn in the right fist and the left hand is empty. The rule can come
  first: it is a hub rule, a `held` value and a ghosted slot.

Which rows become two-handed, if the director takes it: staff, musket (the gun mode's
hand still draws it as it does), crossbow, and the new claymore, maul, double axe, spear
and bow. The hammer stays one-handed (the shaman's slam is a one-hand blow) and so does
the greatsword. Dagger, sword and pistol stay `either`; a pistol in the left hand beside a
sword is a later ability's.

## 4. The character window, apart from the bag

Today (LOOK.md 4) the inventory is one panel: the 6 × 4 grid, and to its right whatever
width is left holds the paperdoll's well with three cells in a column at the edge. Nothing
says what the body wears beyond three icons; the doll is a quarter of the body.

Proposed: **two screens that stand side by side** and drag between each other.

### 4.1 The character (`C`, and **Character** in the menu)

A panel 300 units wide, `PANEL_HIGH` tall, with the body in the middle and the places
around it, as Tales of Pirates and every game since draw it:

```
+--------------------------------------------+
| Marko   Blade · striker in mail | water    |   one ribbon line: the name and the class (2)
|  [head]                         [right]    |
|  [chest]       /\               [left ]    |   the body: the open paperdoll (LOOK.md 5),
|  [back]       /  \              [shldr]    |   whole, turned by a drag, with everything
|  [hands]     | () |             [neck ]    |   it wears: props in hands, every overlay
|  [legs]      |    |             [ring ]    |
|  [feet]      /    \             [ring ]    |
|  deals: slash +3.0%   takes: 3.8% less     |   the gear line (ITEMS.md 3.1, the hub's words)
|  sword  slash +2.0%  worn right            |   the hovered or picked thing, three lines
|  iron core, oak frame                      |
|  hands: Blade, Ironclad                    |
|  [Take off]                 [Bag] [Close]  |
+--------------------------------------------+
```

- The slots are `drop_slot`s as today, 36 dots, in **two columns of six** at the body's
  sides: the armour down the left in the order of the body (head to feet, the back among
  them), the hands and the trinkets down the right. Six cells and five gaps are 241 units;
  with the ribbon, the gear line, three lines of words and the buttons the panel is 350
  of `PANEL_HIGH`'s 360, which is why the ribbon is one line and the words are three. Only
  the places that **exist** are drawn (5): with three places it is `right`, `left`,
  `chest`; a page with twelve wells and nine of them "later" is a promise, not a screen.
  An empty slot says its name whole, as today.
- The body is the **open paperdoll** (`paperdoll_open`, the selector's camera: the whole
  body stands in the rectangle), not the chest-high well, over the skin's panel face. It
  wears what the zone says it wears (`Look`), so what is dropped on a slot is seen on the
  body the moment the zone answers. No new render pass: the one of LOOK.md 5, in a
  different rectangle.
- Under the body, **the gear line**: what the worn things add up to, on the kinds that are
  not zero (`deals: slash +4.0%  takes: slash 3.8% less`), the hub's words
  (`ItemSummary` already carries each item's `does`; the sum is a new field of the
  inventory's answer, `GearSummary`, so the client adds nothing up).
- **A drag from the bag onto a slot wears** (`Wear`, or `WearIn` for the left hand), a
  worn thing dragged into the bag comes off, a worn thing dragged onto another slot moves
  there when it may (a sword from the right hand to the left). The buttons: **Take off**
  (the picked worn thing), **Bag** (opens 4.2 beside it when it is closed), **Close**.
- Opening the character with the bag closed does **not** open the bag; dragging a wearable
  thing out of the bag with the character closed **opens the character** beside it, so a
  person who never pressed `C` finds the places by picking a sword up. (As built: Escape
  closes the screen whole, as on every screen; each panel's Close closes it alone.)

### 4.2 The bag (`I`, and **Inventory** in the menu)

The panel as today without the equip panel: the purse, the 6 × 4 grid, the item bar, the
picked thing's three lines, and the buttons **Wear** / **Take off**, **Sell**, **Store**,
**Storage**, **Character**, **Close**. 280 units wide. A worn thing in the grid keeps its
`worn` corner mark, and gains the place in words in its tooltip (`worn, right hand`).

### 4.3 Side by side

The two are **one layer** (LOOK.md 3: screens on layers 0–3) with two panels: the
character left of centre, the bag right of it, a gap between; each opens and closes
alone; `Escape` closes the one under the pointer, then the other. On a phone (780 × 360
units) 300 + 280 + the gap fit across with room; the body is what is left in its panel's
middle and nothing is cut (CLIENT.md 4.3 said the same of the selector).

### 4.4 The Wear button learns the hands

Wear on a one-handed weapon (`held = "either"`) puts it into the **empty hand** when one
hand holds a one-hander and the other is empty, the right hand otherwise. The hub's
`Wear` does it (it knows both hands; no new request): a second sword is then one click,
and a third swaps the right hand's as today. Wear on a shield goes left, on a two-hander
takes both (3), on an armour to its place.

### 4.5 What the UI scripts keep

Every name a script finds today stays (`weapon: sword`, `off: nothing`, `armour: cuirass`,
`items`, `bar F`); the slots move to the character page and the pages are named
`character` and `inventory`. `scripts/check-screens.sh` and the items gate run unchanged
but for the page a script opens first.

## 5. All the equipment types

> **Superseded in part by 10.8 (2026-10-14):** the director cut the twelve places to six:
> the two hands, the chest, the head, the legs and one accessory (a ring or a cape,
> nothing drawn). The shoulders, the hands, the feet, the back, the neck and the second
> finger of 5.2–5.4 are gone, with their overlays; the shares of 5.5 are the chest 12 of
> 20, the legs 5, the head 3, the accessory half. The rest of this section stands as the
> proposal it was.

Grounded in: the rig (MODELS.md 2: 24 bones, `covers` masks name them), the overlay
budgets (CONTENT.md 4: 1,500 triangles, 512², 262,144 bytes **per overlay per frame**),
the gear term (ITEMS.md 3.1: one cap of 250 per damage type over every place), and the
models we have or can have (7). Twelve places, in three groups; the director picks the
cut (9).

### 5.1 The hands (two places; exist)

| Place | Takes | Drawn | Exists |
|---|---|---|---|
| right hand | a weapon (`right`, `either`, `two`) | the prop on `prop_r` | yes |
| left hand | a shield, a one-hander (`either`), a caster's focus (5.4) | the prop on `prop_l` | yes |

### 5.2 The body (six places; one exists)

Each is an **overlay** (CONTENT.md 3.3) cut per frame, with the bones it `covers` hidden
under it. The packs' modular outfits come **already cut this way** (Body, Arms, Legs,
Feet, Head, Pauldrons: 7), so a place is a file and a row, no new mechanism.

| Place | Takes | `covers` (the bones hidden) | Guards | Overlay? |
|---|---|---|---|---|
| **chest** | cuirass, robe, mail shirt, plate | `spine`, `chest` | physical or elements, as today | yes (exists) |
| **head** | helm, hood, cap, circlet | `head` (a full helm), nothing (a hood, a circlet: drawn over) | physical or elements | yes |
| **shoulders** | pauldrons, mantle | nothing (sit on `clavicle_l/r`, `upper_arm_l/r`) | physical | yes |
| **hands** | gloves, bracers, gauntlets | `forearm_l/r`, `hand_l/r` | physical | yes |
| **legs** | trousers, greaves, cuisses | `thigh_l/r`, `shin_l/r` | physical or elements | yes |
| **feet** | boots, sabatons, sandals | `foot_l/r`, `toe_l/r` | physical | yes |

A frame's body with nothing in a place shows its own skin and trousers (the hero's). The
row's `covers` stays per template (a hood covers nothing, a great helm the head), as now.

### 5.3 The back (one place)

| Place | Takes | Drawn |
|---|---|---|
| **back** | a cape or cloak (an overlay off `chest`, `covers` nothing); a quiver (a prop on `prop_back`) | the overlay with the body's pose; the prop at `prop_back` |

Out of a fight the right hand's weapon is **sheathed** to `prop_back` (CONTENT.md 3.1
named it) whether or not the back place holds anything; the back place is for what is
worn there, not where the sword rests. The cape is where a guild's colour and a title's
badge would be drawn, if either is ever wanted (not proposed here). (As built, 10 step 6:
the cape is ours, made in the hero's Blender pass since no pack has one; the quiver waits
for the bow of 5.6.)

### 5.4 The trinkets (three places; no model)

Nothing is drawn; the place is numbers only, and the numbers are the **element layers**
that a weapon's core does not reach (ITEMS.md 9: a caster's edge is smaller than a
fighter's, 190 against 220; the trinkets are where a caster closes that, and where
PLAN.md 5.3's "counter-meta sockets" live):

| Place | Takes | Layers | Counts for |
|---|---|---|---|
| **neck** | an amulet | catalyst, gem | elements **taken** (a resist) |
| **ring** ×2 | a ring | catalyst, gem | elements **dealt** |

A caster's **focus** (an orb, a book, a wand's stone) is a `shield`-kind template with
`guards = "elements"` for the left hand (5.1), drawn as a prop; it is the caster's shield
and needs no new place.

### 5.5 What a place is worth: the gear term with twelve places

PLAN.md 0 is the bound: the best of everything against a body in nothing wins a
like-for-like exchange by about a quarter, never more. ITEMS.md 3.1 keeps it with **one
cap of 250 per damage type** over every place on a side (the off hand showed the way). More
places must reach the cap, not pass it. Proposed:

- The cap of 250 per type on `dealt` and on `taken` stands, over every place.
- **A place counts for its share**, not for half. Today the chest counts for half of its
  edge (the scale of 2000). With six body places a half each would let two top pieces
  reach the cap and leave four as costume. The share is the place's part of the body:

  | Place | Share | Place | Share |
  |---|---|---|---|
  | chest | 8/20 | head | 3/20 |
  | legs | 5/20 | shoulders | 1/20 |
  | hands | 2/20 | feet | 1/20 |
  | neck (elements taken) | 1/2, element layers only | ring ×2 (elements dealt) | 1/2 each, element layers only |

  A top cuirass alone is then 0.4 × 220 = 88 of `taken` where it is 110 today (a plain
  cuirass of iron and oak, 40 × 0.4 = 16: 0.8% less taken instead of 2.0%); a full top
  set is 220 × (8 + 5 + 3 + 2 + 1 + 1) / 20 = 220, under the cap, and the shield's 110
  on top of it is clamped at 250. The trinkets are on the element types only, where a
  robe set is (`guards = "elements"`), and the neck on the taken side, the rings on the
  dealt side: a top robe set and a top amulet come to 220 + 110 on an element and are
  clamped at 250, as the shield is on a physical kind. **The cap is the rule and the
  shares are the pace**: no body passes 250 on any type, and no two pieces reach it alone.
  The exchange numbers of ITEMS.md 3.1 stand (23.2% at the very best, 18.4% in iron and
  oak once every place is filled). Everything measured before this is measured with one
  place: a body in a cuirass and nothing else is a little weaker than it was, by design
  (it is a body half dressed).
- The right hand counts whole for `dealt` as today (its share is 1/1); a weapon in the
  left hand counts **half** (a proposed number: today it counts whole under the cap,
  ITEMS.md 9, so two iron-and-oak swords fall from 4.0% to 3.0% more slash); a two-hander
  counts for both, 1.5 (3), so a claymore and a pair of swords of the same make deal the
  same.
- The hub's `GearSummary` (4.1) says the sums; the zone takes them in the reading it
  takes the three edges in today (ITEMS.md 3.3), one number per type per side, so the sim
  and the client's prediction change nothing.

### 5.6 The weapon types, and whose hands

What the packs give us (7) and what each is in the matrix (MATRIX.md 4.1: slash, pierce,
blunt). A new weapon is a template row, a prop file and the **ability that holds it**;
no preset gains a weapon without the director's say, since the kit is what balance was
measured on (MATRIX.md 15–17). The column "hands" is the rule of ITEMS.md 2 as it would
read with the ability in these presets' kits:

| Weapon | Strikes | Held | Model | Hands (preset kits) | Note |
|---|---|---|---|---|---|
| sword | slash | either | have | blade | one in each hand (1) |
| dagger | slash / pierce | either | have | shade, musketeer | |
| greatsword | slash | two; one on the colossus | have | ironclad | 3 |
| claymore | slash | two | weapons pack (built 2026-10-14, 10.7: `Sword_Big`) | a custom build's | its own ability: wider and further than the greatsword, for less |
| katana | slash | either | knight pack (built 2026-10-14, 10.7) | a custom build's | the quickest blade |
| mace | blunt | either | weapons pack (built 2026-10-14, 10.7: `Hammer_Small`) | a custom build's | a one-hand blow that staggers |
| scythe | slash | two | weapons pack (built 2026-10-14, 10.7) | a custom build's | the widest sweep, three targets |
| axe | slash | either | KayKit (built 2026-10-14) | a custom build's | a one-hand slash with more knockback |
| greataxe | slash | two | KayKit `axe_2handed` (built 2026-10-14, 10.7) | a custom build's | the axe's weight twice over |
| hammer | blunt | right | have | shaman | |
| warhammer | blunt | two | weapons pack `Hammer_Double` (built 2026-10-14, 10.7) | a custom build's | the heaviest blow; the Warden's `maul` stays a creature's |
| spear | pierce | two | weapons pack (built 2026-10-14) | a custom build's | reach 100, a line, not an arc |
| staff | blunt (the bolt is the element's) | two | have | frostweaver, mender | |
| wand | blunt (a bolt of force) | right | KayKit (built 2026-10-14) | a custom build's, with the `focus` left | |
| bow | pierce | two | weapons pack (built 2026-10-14) | a custom build's | arrows as a stack would be the firearm's mechanism: not done |
| crossbow | pierce | two | have | no preset's since MATRIX.md 15 | |
| musket | pierce | two | have | musketeer | the gun mode |
| pistol | pierce | either | have | musketeer | |
| shield (heater), buckler (round), tower shield (square) | guards | left | have (the buckler and the tower shield KayKit's, built 2026-10-14, 10.7) | everyone (a shield fits all hands) | |

Proposed first additions, in order of what they give the roster: the **axe** (the blade
and the ironclad gain a second one-hander with a different swing; the ironclad's
sword-and-board without the greatsword), the **spear** (reach the matrix has no answer to),
the **bow** (a ranged action build beside the gun mode), the **wand and focus** (casters
with hands).

## 6. The order of work, and what each step costs

Each step is a phase's section, measured, and the director's decision before it:

1. **The names and the words** (2): `CharacterSummary.class`, the `hands:` line, the dim
   icon, the drop that says why. Hub and client, a day; no wire format but the summary's
   version (`PLAYER_VERSION` 10). Nothing on the body changes.
2. **The character window** (4): `character.rs` gains the page, `bag.rs` loses its equip
   panel, the layer holds two panels. The Wear button's hand rule (4.4) is a hub change of
   a few lines. Two days with its UI script and the screens gate; wasm ~15 KB (a page and
   its layout; the paperdoll exists).
3. **Two-handed** (3): `held = "two"`, `one_hand_for`, the ghosted slot, the edge
   counted twice. A day; the hold's animation is Phase 17's.
4. **The body places** (5.2, 5.5): migration 0015 (`worn.slot` takes the new names),
   `Place` grows, `GearSummary`, the share table, the protocol's `Look.worn` (a `u16`,
   LOOK.md 6.2) becomes a small list, one index per body place: twelve bytes where two
   are, in a `Look` that is sent on a change and not every tick; the client draws N
   overlays as it draws one (one `CharacterDraw` each; the mask is the union of their
   `covers`). Three to four days. **The cost that matters is content, and it does not
   fit where the overlays live today.** The committed bundle is 1,921,838 of its
   2,097,152 bytes (`budgets.toml [content]`): 175 KB of room, and the hero's six pieces
   from the free outfits are ~400 KB (the jerkin and the tunic, 2 of them, are in the
   bundle already), every frame after the striker as much again. Two ways, the director's
   choice (9): **raise the bundle's budget** (4 MiB would hold four frames' first sets; a
   browser fetches the directory file by file, so a first visit grows by what it draws),
   or **move overlays and avatars out of the bundle** into a second directory fetched on
   demand and counted under the browser's model cache (WEB.md 9: 16 MiB, entry by
   entry), leaving the bundle to the atlases and the props. The second keeps the
   bundle's cap as it is and is how player models already arrive; it is the one proposed.
5. **The trinkets** (5.4): rows and places, nothing drawn. A day.
6. **The back** (5.3): the cape overlay and the sheathed weapon at `prop_back`. The
   sheathing is an animation-state change (a `Look` bit, set when the fight lock of
   ITEMS.md 5 ends: 10 s after the last damage, `GEAR_AFTER_FIGHT`, the one timer the body
   already keeps); two days.

The content of 5.2 for the **other three frames** is the open cost of Phase 17 (PLAN.md
11.10: "overlays for the other three frames"): the free outfits are cut for one body, and
every piece is re-cut per frame through the hero pipeline.

## 7. Where the models come from

On the box (`~/model-src`), what is already downloaded, by licence as LICENSES.md records
them:

- **Modular Character Outfits – Fantasy** (Quaternius, free tier: Peasant and Ranger):
  each outfit in parts, `Body`, `Arms`, `Legs`, `Feet`, `Head_Hood`, `Acc_Pauldron(s)`,
  male and female: the six body places of 5.2 as files, for the hero's rig through the
  existing pipeline (`blender-quaternius-hero.py`). The leather set (Ranger) and the cloth
  set (Peasant); **no mail and no plate** in the free tier. The knight's plate is in the
  paid Source tier (PLAN.md 11.10 noted it).
- **Knight pack** (`knight/`): `Helmet1–3`, `ShoulderPads`, `ShortSword`, `Katana`,
  `Club`: rigid pieces for the head and shoulders, which an overlay can carry as a prop
  carries its mesh (bound whole to `head` or `clavicle`).
- **LowPoly Medieval Weapons** (`weapons/`): axes (one, double, small), bows (four),
  claymore, daggers, hammers (double, small), scythe, shields (heater ×2, round ×2,
  celtic), spear, swords (plain, big, golden), arrow.
- **KayKit Adventurers** (CC0): one- and two-handed axe, sword and crossbow, dagger,
  staff, wand, spellbook (open, closed: a caster's focus), quiver, five shields, smoke
  bomb, mugs.
- Our own synthesised props (`gm-tools content synth`): hammer, musket, pistol, shield,
  greatsword.

Every file goes through `gm-tools content build` under the prop or overlay budgets
(CONTENT.md 4) and `content check`; the bundle is committed and CI reproduces it.

## 8. Review log

### 8.1 Design review (Gemini 3.1 Pro, 2026-10-13, over v0 with ITEMS.md, LOOK.md and MATRIX.md)

Ten findings; the text above is after them.

1. **Accepted.** Eight slots down one side are 288 units, over `PANEL_HIGH`. Two columns
   of six, one ribbon line, three lines of words: 350 of 360 (4.1).
2. **Accepted.** The diagram had `[neck]` twice. Fixed (4.1).
3. **Accepted, as said now.** The trinkets add to the same cap and can overshoot it with a
   full set: that is what the cap is for, and 5.5 says so (the neck on the taken side,
   the rings on the dealt side, element types only; the clamp is the rule).
4. **Accepted.** The left hand at half made two swords 3.0% while section 3 claimed a
   two-hander equals two swords at 4.0%. A two-hander now counts 1.5 (right whole, left
   half), equal to a pair; two iron-and-oak swords fall from 4.0% to 3.0% (3, 5.5).
5. **Accepted.** A greatsword two-handed for every frame but the colossus is gear that
   behaves by the wearer's frame, against PLAN.md 3.4. The greatsword stays one-fisted
   for everyone, as built; the claymore is the two-hander (3).
6. **Rejected, for now.** A belt or waist place: Tales of Pirates and Ether Saga have
   one. It would be a fourth numbers-only trinket with nothing to carry that the rings and
   the neck do not; the director may add it under 5.4 with the trinkets if he wants a
   fourth. Noted in 9.
7. **Rejected.** An ammo place for arrows: the director decided on 2026-10-07 that rounds
   are a stack in the bag and "carried is loaded, there is nothing to equip" (MODES.md
   11.1, the musket's balls). Arrows are the bow's balls.
8. **Accepted.** `Look.worn` is a `u16`: six places are twelve bytes, not six (6, step 4).
9. **Accepted, and it changes the plan.** The bundle is 1,921,838 of 2,097,152 bytes;
   six overlays for one frame do not fit, let alone four frames. Step 4 now proposes the
   overlays and avatars out of the bundle, fetched on demand under the model cache, or a
   raised budget; the director decides (6, 9).
10. **Accepted.** Sheathing was "5 s out of a fight"; the body keeps one timer, the fight
    lock of 10 s (`GEAR_AFTER_FIGHT`), and the sheath follows it (6, step 6).

## 9. What the director decides

1. The class names as the presets' names, capitalised, and `Custom` (2); or other names.
2. The `hands:` line on every weapon, the dim icon and the drop that says why (2).
3. Two-handed as a `held` value; the greatsword one-fisted for everyone as built and the
   claymore the two-hander; a two-hander's edge at 1.5 (3).
4. The character window apart from the bag, `C` and `I`, side by side (4); and the Wear
   button's empty-hand rule (4.4).
5. The twelve places, or a cut of them: the recommendation is **eight first** (right,
   left, head, chest, hands, legs, feet, back) and the trinkets when a caster's edge is
   measured short (5); and whether a belt joins the trinkets (8.1, 6).
6. The share table of 5.5 (a place counts for its part of the body, the left hand for
   half, a two-hander for 1.5), or "half each, under the cap" as today.
7. The first weapons to add (5.6): the axe, the spear, the bow, the wand with a focus.
8. Where the overlays live (6, step 4): out of the bundle under the model cache (proposed),
   or the bundle's budget raised.
9. The order of 6, or a different one.

## 10. As built (2026-10-13, the same evening)

Steps 1 to 3 of section 6, in that order, one commit:

1. **The names and the words** (2). A preset's name is its class, capitalised
   (`showcase::class_name`: the preset a build is, else `Custom`, or `Custom (an
   Ironclad)` when a preset's whole kit is in it and the frame is its), leading the
   selector's ribbon (`Ironclad  |  colossus in plate  |  ground  |  action mode`); the
   archetype tabs keep the content's lowercase names, which the gates click. The hub
   says whose hands a weapon is for in a field of its own, `ItemSummary.hands` (the
   presets whose kits hold the template's model, by `hub::hands` over the preset list:
   `Blade`, `Frostweaver, Mender`, `no preset's`; empty for what is not a weapon), beside
   `fits`; `PLAYER_VERSION` 10, `HUB_VERSION` 14. The tooltip shows it as `hands: ...`;
   a thing that does not fit is drawn dim with the warning's rim (`SlotThing.bad`) in
   every grid, and dropped on a hand says the hub's words on the notice line.
2. **The equipment window** (4). Not `C`: that key is the gun mode's crouch (the ctrl
   alias), so **`O`** opens it, and its screen is named `equipment` because the K page
   is `character` already. It is the inventory screen with two panels (`Bag.show_grid`,
   `Bag.show_equip`), so every request, answer and gate of the bag is shared: each panel
   300 units wide, side by side in the middle (610 across: a phone's 780 holds them), or
   one alone; the equipment's rows sit three units apart and its slots two, so six rows
   with the ribbon, the words, the notice and the buttons stay under `PANEL_HIGH`. The
   equipment: the name as the panel's title with the class ribbon under it, the body in
   the open (`paperdoll_open`) between two columns of six rows of which only the places
   that exist are drawn (`armour` left; `weapon`, `off` right), what is worn in words
   (`right hand: sword  slash +2.0%`), **Take off**, **Bag**, **Close**. Opening by key,
   by the menu's **Equipment**, by the bag's button; a wearable thing dragged out of the
   bag opens it; Escape closes the screen whole, each Close its panel. **Wear** puts a
   one-hander into the empty left hand when the right holds a one-hander (4.4), in the
   hub's `wear`. The look gate's script clicks **Equipment** before it drags the dagger.
3. **Both hands** (3). `held = "two"` (content v10: the staff; the greatsword stays in
   one fist, 8.1 finding 5), `HELD_TWO` in the manifest, `ItemContent::two_handed`. The
   hub's `wear` empties the left hand under a two-hander and takes a two-hander off
   under a left-hand thing; the reading counts the left hand for **half** and a
   two-hander for **1.5** (5.5: two iron-and-oak swords 3.0%, a staff of the same 3.0%,
   the best sword in each hand still the cap). `ItemSummary.both_hands` lets the bag
   ghost the left slot with `both hands` beneath.

4. **The body places and the trinkets** (5.2, 5.4, 5.5; steps 4 and 5 of section 6
   together, since the plumbing is one). `Place` has twelve values (the chest keeps the
   name `armour`), `PLACES` = 12 is the width of a reading's `templates` and a
   character's `worn` (`HUB_VERSION` 15, `PLAYER_VERSION` 11), migration 0015 widens
   `worn.slot`, `Look.worn` is an array of seven (protocol v20), the zone fills it from
   the reading's body places and the client draws one `CharacterDraw` an overlay with
   the union of their `covers`. `Place::share` is the table of 5.5 as proposed; a
   two-hander's 1.5 is the reading's. An armour row says its `place`, a `trinket` kind
   (`amulet` on the neck, `ring` on either finger: the free one first, like the hands)
   is built round a catalyst and gems with no model, the elements its edge; the hub's
   craft rule knows it. The equipment draws all twelve slots in two columns of six, each
   by its place name. Content: the ranger set cut into one overlay a place by
   `scripts/dev/places-pipeline.sh` (the cuirass is the jerkin and its belts now; the
   pauldrons, the bracers, the hood, the trousers and the boots are their own, all
   under 1,500 triangles), every one of them on the striker and seen together in the
   fitting room (`--worn cuirass,hood,bracers,trousers,boots,pauldrons`). The bundle's
   cap counts the manifest, the atlases and the props (1.37 MB); the avatars and the
   overlays (1.34 MB) are counted apart under `max_wear_bytes` (8 MiB), fetched a file
   at a time as they are first seen (8.1 finding 9, the first way); the 4x atlas grew
   with 32 icons and its cap is 640 KiB. The numbers moved as 5.5 says: a cuirass alone
   is 88 of taken where it was 110 (the zone's items test now expects a sword against a
   cuirass to win by less than the sword adds).

Measured: the hub's items and economy tests (the hub test wears a hood, boots, two rings
and an amulet and reads `[220, 0, 0, 230, 10, 10, 10, 10, 10]` dealt and `[110, 110,
110, 11, 26, 11, 11, 11, 11]` taken beside the sword, the shield and the cuirass; with
the cape of step 6 the elements taken are 13 and 28), the zone's items test, the
client's 115 unit tests, the content's and the tool's pass; clippy is clean.

5. **The back** (5.3; step 6 of section 6, 2026-10-14). **The cape**: no pack we hold has
   one, so it is ours: `cape_mesh` in the hero's Blender pass hangs a grid of cloth (9 by
   13, 192 triangles) from the shoulder line down the back to the knees, each row as far
   out as the body's back is at that height and 2.5 cm more and never nearer than the
   row above, the edges brought forward round the sides most at the shoulders, skinned
   by height to `chest`, `spine` and `hips`, two-sided, its texture a 64² swatch of red
   cloth with a gold hem drawn by `scripts/dev/places-pipeline.sh`; the row `cape`
   (`place = "back"`, guards the elements, `covers = []`, content v11). **The sheath**:
   `Look.sheathed` (protocol v21, PROTOCOL.md 33), the zone's state of an RPG body: set
   once it has neither dealt nor taken damage nor begun a swing or a cast
   (`Player.acting`) for the fight lock's ten seconds (the one timer, 8.1 finding 10),
   cleared the tick it does any of those, told to everyone on the change when the body
   holds something and carried in the roster; a gun build's never. The client draws the
   held prop at `prop_back` instead of `prop_r` (`gm_model::pose::prop_attach_back`: the
   pivot lifted 10 units up and 5 to the right, over the right shoulder, the sheath's
   grip laying the business end down across the back towards the left hip at 32° from
   upright, the flat against the back); a two-hander the same way; no view model in the
   own first person while sheathed; the selector keeps the weapon in the hand. Fitting
   rooms: `gm-tools content look KEY --back`, `gm-client --offline --prop KEY
   --sheathed`. Tests: the zone's items test hears the buyer's sword sheathed once the
   fight lock has let it be worn and in the fist again the tick of the next swing; the
   hub's items test wears the cape on the back; the pose module's test proves the
   sheath's grip a rotation that puts a metre of blade from over the right shoulder to
   the left hip, behind the body. Open: a quiver on the back for the bow (5.6), the
   companions' and creatures' weapons (they hold nothing yet, LOOK.md 6.1).

6. **The weapons of 5.6** (2026-10-14, the director: "can you do it"). Four primaries,
   appended to `abilities.toml` and mirrored in `gm_core::sim::test_content` (content
   v12): the **axe** (70 slash, 80°, two targets, knockback 240, 200/60/350 ms, 800 ms;
   held either hand), the **spear** (65 pierce, a line of 24° and 100 units, one target,
   220/60/380 ms, 900 ms; both hands), the **bow** (an arrow of 60 pierce that drops,
   gravity 0.4, 1.1 s; both hands) and the **wand** (a bolt of force, 38 blunt, 6 focus,
   700 ms: no aspect, so any build may take it; the elemental bolts stay the staff's),
   with the **focus** (`kind = "shield"`, guards the elements, the left hand; KayKit's
   closed spellbook) as the wand's other hand. **No preset's kit changes** (MATRIX.md 10:
   the kits are what the balance was measured on, and the matrix gate plays the presets):
   these are a custom build's, and the hands rule says `no preset's` of each until a
   preset takes one; a build whose kit holds the ability wears the weapon (ITEMS.md 2).
   Models: KayKit's `axe_1handed`, `wand` and `spellbook_closed` by `gm-tools content
   import prop`; the LowPoly Medieval Weapons pack's `Spear` and `Bow_Wooden` by
   `scripts/dev/blender-obj-prop.py` (900 and 728 triangles). Holds: the axe and the wand
   in the fist like the sword (turned 90° about X, scaled 0.6 and 0.5); the spear along
   the arm like the crossbow (turned 90° about Y, down at rest and a thrust when the arm
   comes up, turned back in the view); the bow the plain hold, so it stands upright when
   the arm is raised to aim; the focus upright with its cover out. Sheathed, each takes the
   sword's place on the back with its own fit, so the spear slings point-up over the right
   shoulder and the bow stands upright along the back. Not done: arrows as a stack (the firearm's reserve is
   gun-mode machinery), the quiver, and whether a preset should gain one of them, which
   is the director's.

7. **The second batch** (2026-10-14, the director: "sellers for those so I can buy and
   test with my blade, but ok we need more content anyway"). **Sellers first**: three more
   stalls on the square from `scripts/dev/play.sh` (`sellers`, run by `people` and on its
   own against a running stack), the **Armourer** (every body place, the shields and the
   trinkets, which fit everyone), the **Weaponsmith** (the axe, the spear, the bow, the
   wand and the crossbow, plain and fine: a custom build's, said so on the stall by the
   hands rule) and the **Outfitter** (everything below, plain); a seller that still
   carries anything is not handed more, and one whose bot is alive is left alone. **Six
   more primaries** (content v13), appended and mirrored like 6: the **mace** (60 blunt,
   75°, stagger 55, 180/60/340 ms, 750 ms; either hand), the **katana** (50 slash, 70°,
   120/50/260 ms, 520 ms; either hand), the **claymore** (85 slash, 110° of 90 units, two
   targets, 280/80/480 ms, 1 s), the **greataxe** (90 slash, 90°, two targets, knockback
   300, 300/80/500 ms, 1.1 s), the **warhammer** (95 blunt, 100°, two targets, knockback
   320, stagger 70, 320/90/520 ms, 1.2 s) and the **scythe** (75 slash, 150°, three targets
   at 0.6, 300/80/500 ms, 1.1 s), the last four both hands; no preset's kit changes. **Two
   shields**: the `buckler` and the `tower_shield` (KayKit's round and square, stood with
   the face out: turned −90° about X, where the focus is turned +90°). **The knight's
   plate**, the first rigid overlays: `scripts/dev/blender-rigid-overlay.py` binds a
   flat-coloured piece whole to one bone of the striker's rig (every vertex weighed 1.0:
   a helmet rides the `head`, the shoulder plates the `chest`), scaled to a ratio of the
   width of the part that bone moves and centred on it, with a 64² swatch texture painted
   from the materials' colours (an overlay wants a texture where a prop is given swatches
   at the build); the `helm` (the open helm with a nasal, 772 triangles) and the `barbute`
   (cheek guards, the face open, 568) cover nothing, the `great_helm` (the bucket with the
   cross slit, 656) covers the head, the `plate_pauldrons` (96) sit across both shoulders.
   **Six materials** between the grades (bronze 20 and steel 45 for cores, yew 18 and
   ironwood 35 for frames, riverglass 15 for shards, garnet 20 for gems), none above its
   layer's best, so the best craft stays 250. The icon atlas's cap goes from 640 to 768 KiB
   (50 icons: 645,096 bytes at 4 texels a dot). Models: the LowPoly Medieval Weapons
   pack's `Hammer_Small`, `Hammer_Double`, `Sword_Big` (the pack's `Claymore` is a thin red
   blade that reads as a line) and `Scythe`, the knight pack's `Katana`, `Helmet1–3` and
   `ShoulderPads`, KayKit's `axe_2handed`, `shield_round` and `shield_square`
   (LICENSES.md). Holds chosen in the fitting room: the blades and hafts in the plain hold
   (cut along +Z, no turn), the greataxe like the axe (90° about X, scaled 0.9). Open: the
   quiver, the companions' and creatures' weapons, and whether a preset gains any of
   these, which is the director's.

8. **Six places** (2026-10-14, the director: "can we simplify it a bit so that we only
   have 2 hands, head, body, legs and 1 accessory. Accessory could be either ring or cape
   (we can go with no render for any atm). No shoulder things and such. Then test all the
   combos if they render correctly and fix it"). `gm_content::items::Place` is **weapon,
   off, armour (the chest), head, legs, accessory** (`PLACES` 6, `Place::BODY` the three
   overlays: chest, head, legs); the shares are the chest 12 of 20, the legs 5, the head
   3 (a full set of the best reaches the cap of 250 alone, as the six did), the accessory
   half. **The accessory** takes a `trinket`: the **ring** (no guard: it sharpens what
   its wearer deals of the elements) or the **cape** (`guards = "elements"`: it dulls what
   it takes; a trinket now, catalyst and gems, nothing drawn; its overlay is gone with the
   back and may return when the accessory is drawn). `ItemTemplate::deals` reads the
   guard instead of the place. **Gone**: the pauldrons, the bracers, the boots, the
   shoulder plates and the amulet with their places and files (`places-pipeline.sh`
   copies the cuirass, the hood and the trousers only). **The wire**: `Look.worn` is three
   (`PROTOCOL_VERSION` 22, PROTOCOL.md 34); the hub's reading and a character's list carry
   six templates (`HUB_VERSION` 16, `PLAYER_VERSION` 12, `PLACE_HEAD` 4, `PLACE_LEGS` 5,
   `PLACE_ACCESSORY` 6, `BODY_INDICES` [2, 3, 4]); migration 0016 takes off what was on
   the places that went (it stays in the bag), turns a worn ring or cape into the
   accessory (the ring first) and narrows `worn_slot_check`. **The window**: three slots
   a column, every other row, so they span the body; the third line of words is the
   body and the accessory. **The sellers** (10.7) are handed what they lack by template,
   so a standing stall takes a list that changed: the armourer sells the chest, head and
   legs pieces, the shields, the focus and the cape; the weaponsmith the twelve weapons
   the keeper does not; the outfitter the ring and the fine make. **The combos** (the
   director's test): every head (none, hood, helm, barbute, great helm) with every chest
   (none, cuirass, robe) and the legs (none, trousers), armed, and a bow, a claymore, a
   tower shield and the sheathed sword beside, rendered headless on the striker and
   looked at as one contact sheet. **Two things were wrong, both fixed in the cut**: the
   trousers left the hero's legs ending at the knee with the shoes lying between them
   (the ranger's trousers end where its boots begin, and the boots had gone with the
   feet's place; `covers` hid the shins, and the shoes, weighted to the shins too, lost
   their tops), so the legs' overlay is now the trousers **and** the boots in one
   (`blender-quaternius-hero.py`: 451 + 825 = 1,276 triangles at ratios 0.4 and 0.09,
   covering the thighs, the shins, the feet and the toes); and the buzzed hair came
   through the hood's cloth, at 12 mm and at 20, so the hood alone is pushed out 35 mm.
   Everything else drew as it should: the helm and the barbute over the hair, the great
   helm hiding the head, the cuirass and the robe under every helmet, each over the
   trousers, every weapon in the hand and the sword on the back. (The first sheet was
   taken on a striker mannequin, which has no hair and no separate shoes and so showed
   neither fault: a check of overlays must be on the official avatar, `--crowd 10` with
   the camera at `-876,-649,40,32` and `--bench 240`, where member 5 stands idle facing
   it.)
