# Content v3 (2026-10-09, content version 6: the six elements of MATRIX.md 5; v2 2026-10-06; v1 Phase 3)

Nothing is priced since 2026-10-20 (MATRIX.md 20: no kit points, no trainer; the mode
`gun` is `fps`). `abilities.toml`, `builds.toml`, `creatures.toml` and `trials.toml` are the shipped content
(`items.toml` the items and what they look like worn, `avatars.toml` the official looks of the frames: CONTENT.md 3)
(MATRIX.md 10, COMPANIONS.md 8 and 11). `gm-content` compiles them against the zone tick rate
and validates them against VOCABULARY.md 11 and MATRIX.md 9; the zone sends the compiled pack
to every client after `Welcome`. New abilities go at the end of `abilities.toml`: stored
builds name abilities by their place in it.
`gm_core::sim::test_content` mirrors this pack in code for tests; `gm-content`'s tests fail
when the two drift.

| Key | Slot | Aspect | What it does |
|---|---|---|---|
| sword | primary | — | 60 slash, 90° arc, 150/60/300 ms, 600 ms |
| hammer | primary | — | 90 blunt, ignores magic shields, heavy stagger, 1 s: the shaman's (MATRIX.md 17) |
| greatsword | primary | — | 95 slash, 120° arc of 96 u, 320/90/520 ms, 1.1 s: the colossus's (MATRIX.md 16) |
| staff | primary | — | 40 blunt, 600 ms |
| dagger | primary | — | 35 slash, 400 ms |
| crossbow | primary | — | 80 pierce bolt, 1.8 s |
| firebolt | primary | fire | 55 fire bolt + Burn 12/s for 3 s, 1.2 s |
| ice_shard | primary | water | 50 water shard + 1 Chill stack (3 freeze), 1 s |
| stone_throw | primary | ground | 75 ground, heavy knockback and stagger, 1.8 s |
| shadow_dart | primary | air | 40 air, fast, ignores evasion, 0.8 s |
| spark | primary | electric | 50 electric + Shock (interrupt), 1.2 s |
| musket | primary | — | 110 pierce, flat and loud, 3 s |
| kick | secondary | — | 25 blunt, heavy knockback and stagger 40, 1.5 s |
| shield_bash | secondary | — | 35 blunt, stagger 70, cannot be parried, 4 s |
| throwing_knife | secondary | — | 40 pierce, flies 300 u and no further, 2 s |
| mend | secondary | — | a dart that deals nothing: Regen 100/s for 3 s on whoever it hits, friend or foe; 12 focus, 2 s |
| shield_wall | guard | — | block 150°, 80%, stops projectiles, 12 stamina per hit |
| parry | guard | — | 150 ms window, 90°; success staggers 600 ms and ripostes 40 slash |
| brace | guard | — | block 120°, 50%, no projectiles |
| dash | active | — | 900 u/s for 150 ms, 30 stamina |
| leap | active | — | 400 forward, 320 up |
| charge | active | — | 700 u/s for 400 ms, stops on hit |
| blink | active | air | 256 u along the facing, 30 focus |
| overhead | active | — | 70 slash, 60° arc, slow |
| stomp | secondary | ground | 128 u cylinder at the feet: 35 ground, Slow 30% 2 s, knockback |
| fortify | active | ground | Fortify 40% for 5 s (hammers ignore it) |
| bellow | active | — | Taunt 2.5 s on every enemy within 256 u (turned to the roarer and held; a control), Fortify 30% 4 s on the roarer |
| frost_nova | active | water | 160 u sphere: 30 water + 2 Chill stacks; the Water staff's spell (BODIES.md 7.2) |
| haste | active | — | Haste 25% for 5 s |
| thunderclap | active | electric | 140 u sphere: 40 electric + Shock; the Electric staff's spell |
| vanish | active | air | Stealth 256 u for 6 s |
| poison_cloud | active | air | 96 u cloud ahead for 5 s: Bleed 6/s |
| fireball | active | fire | 20 fire bolt, 96 u splash 30 fire + Burn |
| sanctuary | active | — | a circle of 140 u where the caster aims (within 500 u) for 6 s: Regen 60/s on every body in it; 35 focus, 14 s |
| war_standard | active | — | two more squad slots (five companions); used: Fortify 15% for 8 s on every body within 256 u, enemies included |
| maul | primary | — | 55 blunt through magic shields, 120° arc, 96 u reach, 550 ms windup, heavy knockback, one every 2 s |
| quake | active | ground | a circle of 150 u under whatever is aimed at within 700 u; breaks 1.3 s later for 70 ground |
| slam | secondary | — | the hammer on the ground: 96 u circle at the feet, 30 blunt, knockback 300, stagger 40, 4 s |
| briar | active | grass | a patch of 140 u where the shaman aims (within 500 u) for 6 s: 8 grass a second on everyone in it, Regen 30/s on the shaman's own side in it; 35 focus, 14 s |
| entangle | active | grass | a circle of 112 u where the shaman aims (within 500 u): 15 grass + Root 1.5 s; 25 focus, 14 s |
| axe | primary | — | 70 slash, 80° arc, two targets, heavy knockback, 200/60/350 ms, 800 ms: a custom build's (EQUIPMENT.md 5.6) |
| spear | primary | — | 65 pierce, a line of 24° and 100 u, one target, 220/60/380 ms, 900 ms; both hands |
| bow | primary | — | 60 pierce arrow that drops (gravity 0.4), 1.1 s; both hands |
| wand | primary | — | 38 blunt bolt of force, fast, 6 focus, 700 ms; a focus in the left hand |
| mace | primary | — | 60 blunt, 75° arc, stagger 55, 180/60/340 ms, 750 ms: a custom build's (EQUIPMENT.md 10.7) |
| katana | primary | — | 50 slash, 70° arc, 120/50/260 ms, 520 ms: the quickest blade |
| claymore | primary | — | 85 slash, 110° arc of 90 u, two targets, 280/80/480 ms, 1 s; both hands |
| greataxe | primary | — | 90 slash, 90° arc, two targets, knockback 300, 300/80/500 ms, 1.1 s; both hands |
| warhammer | primary | — | 95 blunt, 100° arc, two targets, knockback 320, stagger 70, 320/90/520 ms, 1.2 s; both hands |
| scythe | primary | — | 75 slash, 150° arc, three targets at 0.6, 300/80/500 ms, 1.1 s; both hands |
| slip | active | — | the Thief's body skill: a three-second vanish of no element, 20 stamina, 10 s |
| pistol | secondary | — | the FPS sidearm, the pistol in the off hand |
| carbine | primary | — | the lighter gun |
| thorn | primary | grass | 45 grass bolt + Bleed 2 s; the Grass staff's bolt |
| flame_burst | secondary | fire | a cone of 60 fire + Burn; the Fire staff's second move |
| briar_burst | secondary | grass | a cylinder of 40 grass + Root 0.8 s; the Grass staff's second move |
| gust | secondary | air | a cone of 30 air, knockback 300; the Air staff's second move |
| staff_sweep | secondary | — | an arc of 30 blunt, stagger 40; the plain staff's second move |
| pommel | secondary | — | 25 blunt, stagger 50, 1.5 s; a sword's, a katana's and an axe's second move |
| thrust | primary | — | 70 pierce, reach 84, one target; the sword's technique |
| hammer_sweep | primary | — | 55 blunt, arc 140, five targets; the hammer's technique |
| volley | primary | — | three arrows of 30, 35 stamina; the bow's technique |
| backstab | primary | — | 55 pierce, unparryable; the dagger's technique |
| fists | primary | — | the melee group's empty hands (BODIES.md 7.1): two blows, 18 and 22 blunt |
| knife_hands | primary | — | the ranged group's empty hands: the knife held as the weapon |
| force_blow | primary | — | the magic group's empty hands with no element: a bolt of 22 force, 4 focus |
| fire_blow | primary | fire | the magic group's empty hands with Fire: a bolt of 22; so for each element below |
| water_blow | primary | water | as the fire blow, of water |
| grass_blow | primary | grass | of grass |
| electric_blow | primary | electric | of electric |
| ground_blow | primary | ground | of ground |
| air_blow | primary | air | of air |
| chill_wave | secondary | water | a cone of 30 water, 20 focus, 4 s; the Water staff's second move (BODIES.md 7.2) |
| static | secondary | electric | a cone of 30 electric, 20 focus, 4 s; the Electric staff's second move |

The range is the weapon's (MATRIX.md 10, 2026-10-06): a primary is a blade or the bow; a
secondary is a short utility or the weapon's second move; an active is a body's skill or
a weapon's spell (`spell_of`, BODIES.md 7.2). Presets (thirty attribute points each, a body
each since BODIES.md 7.5):
**ironclad** (colossus, plate, ground; greatsword, shield bash, a shield in the off hand; stomp, bellow), **blade** (striker, mail, water;
sword, kick; charge), **frostweaver** (caster, cloth, water + air; ice shard, kick), **shade**
(infiltrator, leather, air; dagger, knife), **mender** (caster, cloth, electric: the healer;
staff, mend), **musketeer** (striker, leather, neutral; musket, pistol: the gun) and **shaman**
(caster, leather, grass; hammer, slam; briar, entangle: MATRIX.md 17). MATRIX.md
11 records how the first four fare against each other; a bot that counter-picks takes the
first preset listed among those that score the same.

## Creatures and trials

`creatures.toml` (COMPANIONS.md 8.1): a creature is a build without a budget, with its health
set here, how far it sees, how far it is leashed to its post, when it comes back, and for a
boss what it drops. Abilities marked `creature = true` are theirs alone. A map places them
with `gm_creature` entities.

| Key | Body | Health | Kit | Sight / leash | Back after | Drops |
|---|---|---|---|---|---|---|
| sentinel | striker, water | 120 | sword, crossbow, parry; overhead, dash | 700 / 900 | 600 s | nothing |
| warden (boss) | colossus, ground | 1,500 (stagger 400) | maul, stone_throw; quake, stomp | 900 / 1,100 | 120 s | 3 components (standard: core/iron, frame/ash, catalyst/basalt; top: core/dragonbone, frame/whalebone, catalyst/basalt), 30 silver |
| dummy (`still`) | colossus, ground | 500 | nothing it uses | — | 5 s | nothing |
| yard_master | striker | 30 | drill, kick | 220 / 320 | 5 s | nothing |
| shieldbearer | striker, ground | 160 | mace, shield_bash, shield_wall | 400 / 700 | 600 s | nothing |
| cutthroat | infiltrator, air | 60 | dagger, throwing_knife; dash | 520 / 400 | 600 s | nothing |
| venomer | caster, grass | 50 | venom_thorn, kick | 420 / 640 | 600 s | nothing |

`trials.toml` (COMPANIONS.md 11): an encounter of a map judged through a lens. The four of
the tutorial are on the Warden, 300 s, one human: `warden_leader` (500‰ of the party's damage
under the candidate's orders, at most one party death), `warden_vanguard` (500‰ of the blows),
`warden_striker` (350‰ of the damage), `warden_mender` (500‰ of the healing, no death). The
ladder's (SKILLS.md 5) are passed by clearing: `yard` in the town, `gate`, `hall` and
`antechamber` in the dungeon, and `rite`, the Warden, which wakes the Essence.

## Looks (Phase 14, CONTENT.md 3)

Every row may name what it looks and sounds like: an ability's `icon`, `prop` (held while
it is the primary and nothing is worn) and `sound`; a template's `model` (a prop under
`models/props/`), `icon`, `held`, `fit` and `fit_view`; a material's `icon` and `tint`; a
creature's and a build's `model` and `icon`. `VERSION` holds the content's number;
`LICENSES.md` names every file that is not ours. `gm-tools content check` resolves every
link, `build` writes `assets/built/content/` (committed), `report` lists it all.

| Key | Slot | Aspect | What it does |
|---|---|---|---|
| musket | secondary | — | 55 pierce ball at 2,600 u/s, flat, one shot every 2.5 s (the gun; CONTENT.md 11) |

## Items

`items.toml` holds the item templates and the materials of the five component layers
(ECONOMY.md 4). `gm_content::items` validates it: materials are `layer/name`, only catalysts
carry an element, every template takes a core and a frame, and the best possible craft of any
template gives at most 250 per mille over a standard item (PLAN.md 0). The hub refuses crafts
of templates that are not in this file. Thirty-four templates since content v14 (2026-10-14,
EQUIPMENT.md 10.8: six places): twelve weapons a preset or a custom build swings, the
crossbow, the musket and the pistol, six armours over three body places (the ranger's
jerkin, hood and trousers, the peasant's robe, the knight's helm, barbute and great helm),
three shields and the focus, the ring and the cape for the one accessory, and the four
stacks; twenty-two materials.
