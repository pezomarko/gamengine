# Skills rework: the plan

Spec: the director's page "Skill Rework Proposal" v5, 2026-10-22
(https://claude.ai/artifact/NWu6LLTW7dENuCFtqTq8qi). Five steps, one PR each; a merged
step ships to the LAN stack. Numbers are the page's; what a step measures goes in
MATRIX.md 24.

## 1. Fixes

- Timers in the zone's rate. The stamina pause after a spend, the leap's lift, Freeze and
  the Chill immunity were counted in 64 Hz ticks in every zone: the pause lasted 3.25 s in
  the 20 Hz town. `tick::ms_ticks(ms, dt)`; `Statuses::apply` takes `dt`.
- Taunt: the taunter's threat is set once, to the top of the table times the switch margin.
  It was added every frame of the mind (64 a second in the dungeon, 20 in the town).
- Bows: 2 ST a draw, 3 a span, as before PR 71.
- Tests: a parry pauses ST for 1 s at 20 and at 64 Hz; Freeze lasts 1 s at both; a taunt
  leaves the taunter on top by the margin.

## 2. Rest

Core:
- `buttons::SIT` takes the unused `INTERACT` bit. `Mover.seat`: up, down since a tick, or
  rising until one. A SIT press on the ground, out of a script, a guard, a dash and an item
  use, sits. The next SIT press, a move, a jump, a crouch, a blow, a skill, the guard, an
  item, a reload or the command stance stands the body up over `STAND_MS` (500), doing
  nothing meanwhile. Stagger, Shock,
  Knockdown and Launched stand it at once; the mover reads them from its statuses, so the
  prediction follows the server's.
- Seated: no movement, no activation, no guard. Hits land in full as they do today on a body
  that neither guards nor dodges.
- `Derived`: `stamina_regen` 0.5 + 0.03·AGI standing; `rest_stamina` 1 + 0.06·AGI,
  `rest_focus` 0.3 + 0.03·SPR, `rest_health` 1 + 0.04·CON. Resting is seated for
  `REST_MS` (3000). Standing regenerates ST only. Damage does not restart the rest.
- HP is the zone's: `Player.rest_hp` gathers `rest_health·dt` while resting and adds whole
  points. No `Healed` event, so no healing threat.
- `anim::SIT` (16) while seated: snapshots carry it, no new flag.

Net: PROTOCOL 33. AI: `Body.seated`; a seated body's threat gets no proximity weight.
Client: Insert and Z hold SIT; a touch button; a seated pose; the C sheet shows the
standing and resting rates.

Tests: sitting and standing, the rest's delay and rates, damage keeps the rest, a stagger
ends it, the rising lock, HP on the zone, no threat from it, the AI weight.

## 3. Skills

Engine:
- `Status::Fury` (17): damage dealt × (1 + m), as Weaken in reverse. `Status::is_boon()`
  (Haste, Fortify, Regen, Stealth, Fury) replaces the three hard-coded lists.
- `Status::Cleanse` (18) and `Status::Dispel` (19) go through `ApplyStatus` and are never
  stored: Cleanse removes every status that is not a boon, Dispel every boon.
- `Cost.stamina_share`, `Cost.focus_share`: per mille of the pool's maximum (the mover reads
  it from the sheet), needed in hand and spent.
- `retired = true` on an ability row: `worn_of` never picks it.

Content (keys and row order stay; new rows are appended):
- the page's costs, cooldowns and durations: ST for physical second moves, tricks and
  weapon spells, MP for the rest;
- the fifteen names; Mend 4 a second for 8 s; the staff's blasts are the staff's alone and
  the wand's second move is Dispel;
- new: Illusion Slash (sword, katana: a 450 u bolt through three bodies), Tiger Roar (axes),
  Whirlwind (big blades), Deadeye (crossbow), Dispel (wand), Purify (the Elder's skill in
  place of Sanctuary);
- Bull Rush knocks down the body it stops on (a short arc after the charge); Leap is
  untouchable for 100 ms;
- Backstab retired; content rebuilt.

Tests: Fury, Cleanse, Dispel, the share costs, retired rows; the arena legs re-measured.

## 4. The Essence

- Content: the ten forms, appended, each with `essence_of` (the weapon props it fits);
  Sanctuary becomes Healing Spring, the wand's form. Signatures cost a share of 600.
- Hub: migration 0020, `characters.essence_form` and `characters.essence_item`. A `rite`
  trial (dungeon, warden, no conditions): its first pass awakens the Essence in the worn
  weapon if that weapon has a form. `ZoneEconOp::Infuse { character, item, fee }`: the
  smith moves it to another weapon of the same form for `ESSENCE_FEE` (10 silver) to the
  sink. `GearReading.essence`: the form when the worn weapon holds it.
- Core and zone: `Worn.signature`, put after the spell by `kit_slots` (the third active:
  keys 3, 5 and 6, as the hotbar has them). `Mover::rearm` carries each cooldown over by
  `AbilityId` to wherever that ability sits in the new kit, so a swap resets nothing.
- Client: the smith's screen moves it; the C sheet names it.
- HUB_VERSION 23, PROTOCOL 34, content.

Tests: awaken once and only with a formed weapon; infuse within the form; the fee; the
refusals; the kit; rearm.

## 5. The ladder

- The steps are trials: `yard` (town, encounter `yard`), `gate`, `hall`, `antechamber`
  (dungeon), `rite` (dungeon, `warden`). The hub turns the passes into a stage and sends it
  in the gear reading; it rides in `Worn` (the zone and the client compile the same kit), and
  `Build::kit_slots_in` drops what is not earned: the second move needs `yard`, the trick
  `gate`, the spell `hall`; the signature needs the Essence.
- HUD: the MP bar after `yard`; one line saying what to do next.
- The arena runs with `--requires rite`; play.sh grants its bots the pass.
- Town: the yard in the south street, a sparring master beside the dummies (a slow,
  telegraphed, parryable swing for 1; 30 HP).
- Dungeon (`gm-tools map gen-dungeon`): the gate's sentinels become a shield-bearer; a pack
  in the passage (`hall`); bleeding adds before the stair (`antechamber`) with a shrine
  (`gm_shrine`: E cleanses); a campfire (`gm_campfire`: near it the client shows
  "Insert: sit"); the Warden is the rite. Maps and content rebuilt.

Tests: the kit by passes; the arena's refusal; the gm-ai map tests moved with the rooms.

## Not built

MP draughts and the respawn's half pools (the director's to decide); the premium NPC that
re-forms an Essence; the six element blows folded into one; the Essence's glow.

## Review

Gemini 3.1 Pro on the plan with the code it touches, 2026-10-22. Taken:
- `rearm` must move cooldowns by `AbilityId`, not by kit index (step 4 says so).
- The kit cannot see trial passes: the ladder's stage rides in the gear reading and `Worn`.
- The share costs need the pool's maximum: the mover has the sheet.
- Reload and the command stance stand a sitter up.
- `Status::ALL` takes the new statuses.

Not taken:
- "A charge that stops early leaves the actor idle until the arc": the arc is timed at the
  charge's end, 0.4 s at most, and hits the body it stopped against; a test proves it.
- "The taunt must know it is new": it does, by the taunter it last saw.
- "A signature shifts the active indices": the hotbar reads the kit, and no preset has one.
- "Derived travels on the wire": both sides compute it from the build.
- "Fury needs its own attacker field": damage dealt is already scaled by `1 − weaken`;
  the attacker's weaken is Weaken's magnitude less Fury's.
