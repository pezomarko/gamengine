# Licences of the content's files

Every file under `assets/content/` that is not written by this project is listed here with
where it came from and under what terms (CONTENT.md 2 and 11: CC0 and ours only). The
official content as a whole is CC-BY-SA 4.0 (LICENSE.md at the root).

## Models (`models/`)

Props under `props/`, official avatars under `avatars/`, armour overlays under `armour/`
(CONTENT.md 2).

| File | From | Licence |
|---|---|---|
| `props/sword.glb` | Quaternius *RPG Characters* (`glTF/Warrior.gltf`: the `Warrior_Sword` mesh with `Warrior_Sword_Texture`), https://quaternius.com/packs/rpgcharacters.html; split off by `scripts/dev/blender-quaternius-warrior.py`, packed by `gm-tools content import prop` (until 2026-10-13 the KayKit Adventurers `sword_1handed.gltf`, CC0, which the dagger, staff and crossbow still come from) | CC0 1.0 |
| `avatars/striker_mail.glb` | Quaternius *Universal Base Characters* (`Superhero_Male_FullBody.gltf`, `Hair_Buzzed.gltf`, `T_Superhero_Male_Ligh.png`, `T_Eye_Brown.png`, `T_Hair_1_BaseColor.png`), https://quaternius.com/packs/universalbasecharacters.html, with the trousers and boots of Quaternius *Modular Character Outfits - Fantasy* (`Male_Peasant_Legs.gltf`, `Male_Peasant_Feet.gltf`, `T_Peasant_BaseColor.png`), https://quaternius.com/packs/modularcharacteroutfitsfantasy.html; made by `scripts/dev/blender-quaternius-hero.py` (one armature, bones renamed to MODELS.md 2, `prop_r`/`prop_l`/`prop_back` added, the legs under the trousers cut, decimated to 3,448 triangles) and `scripts/dev/gltf-atlas.py` (one 1024² atlas); `scripts/dev/hero-pipeline.sh` runs it all. Until 2026-10-13 the Quaternius *RPG Characters* warrior, `scripts/dev/blender-quaternius-warrior.py` | CC0 1.0 |
| `armour/cuirass_striker_mail.png` | the cuirass as a patch on the striker in mail (CONTENT.md 3.3, LOOK.md 6.6, 2026-10-15): the same cut (`armour/cuirass_striker.glb`, its texture `T_Ranger_BaseColor.png`) baked onto the body's own atlas by `scripts/dev/blender-bake-patch.py` and joined by `scripts/dev/patch-join.py` (`scripts/dev/places-pipeline.sh` runs them); the cut itself stays for the icon | CC0 1.0 |
| `armour/trousers_striker_mail.png` | the trousers and boots the same way, from `armour/trousers_striker.glb` | CC0 1.0 |
| `avatars/<body>.glb` for the seven bodies below | Quaternius *Universal Base Characters* (the superhero man or woman, `T_Eye_Brown.png`, a hairstyle of `Hairstyles/Rigged to Head Bone`, its `T_Hair_1_BaseColor.png` or `T_Hair_2_BaseColor.png` mask tinted) with the peasant trousers and shoes of Quaternius *Modular Character Outfits - Fantasy* (`T_Peasant_BaseColor.png`), the two packs above; made by `scripts/dev/bodies-pipeline.py` (2026-10-23, BODIES.md 3.1): `scripts/dev/blender-quaternius-body.py` (one armature, bones renamed to MODELS.md 2, the prop bones added, the legs under the trousers cut, decimated to at most 3,450 triangles), `scripts/dev/gltf-atlas.py` (one 1024² atlas) and `scripts/dev/glb-retarget.py` (scaled to the body's frame). Beside each: `armour/hood_<body>.glb` and `armour/robe_<body>.glb`, the same pack's ranger hood and peasant tunic and sleeves of the body's sex, cut for it the same way; `armour/helm_<body>.glb`, `armour/barbute_<body>.glb` and `armour/great_helm_<body>.glb`, the knight pack's three helms bound to its head by `scripts/dev/blender-rigid-overlay.py`; `armour/cuirass_<body>.png` and `armour/trousers_<body>.png`, the ranger jerkin and trousers baked onto its atlas by `scripts/dev/blender-bake-patch.py` and `scripts/dev/patch-join.py` | CC0 1.0 |
| `avatars/brute.glb`, `armour/hood_brute.glb`, `armour/robe_brute.glb`, `armour/helm_brute.glb`, `armour/barbute_brute.glb`, `armour/great_helm_brute.glb`, `armour/cuirass_brute.png`, `armour/trousers_brute.png` | the brute: the man (`Superhero_Male_FullBody.gltf`), `Hair_Buzzed.gltf` and `Hair_Beard.gltf`, `T_Superhero_Male_Ligh.png`, with `Male_Peasant_Legs.gltf` and `Male_Peasant_Feet.gltf`; scaled 1.25 wide, 1.07 tall, 1.22 deep to the colossus | CC0 1.0 |
| `avatars/veteran.glb`, `armour/hood_veteran.glb`, `armour/robe_veteran.glb`, `armour/helm_veteran.glb`, `armour/barbute_veteran.glb`, `armour/great_helm_veteran.glb`, `armour/cuirass_veteran.png`, `armour/trousers_veteran.png` | the veteran: the man, `Hair_SimpleParted.gltf` and `Hair_Beard.gltf` tinted grey, the same trousers and shoes; scaled 1.12 wide, 1.07 tall, 1.12 deep to the colossus | CC0 1.0 |
| `avatars/huntress.glb`, `armour/hood_huntress.glb`, `armour/robe_huntress.glb`, `armour/helm_huntress.glb`, `armour/barbute_huntress.glb`, `armour/great_helm_huntress.glb`, `armour/cuirass_huntress.png`, `armour/trousers_huntress.png` | the huntress: the woman (`Superhero_Female_FullBody.gltf`), `Hair_Long.gltf` tinted auburn, `T_Superhero_Female_Light_BaseColor.png`, with `Female_Peasant_Legs.gltf` and `Female_Peasant_Feet.gltf`; as the pack has her, a striker | CC0 1.0 |
| `avatars/sorceress.glb`, `armour/hood_sorceress.glb`, `armour/robe_sorceress.glb`, `armour/helm_sorceress.glb`, `armour/barbute_sorceress.glb`, `armour/great_helm_sorceress.glb`, `armour/cuirass_sorceress.png`, `armour/trousers_sorceress.png` | the sorceress: the woman, `Hair_Long.gltf` tinted black, `T_Superhero_Female_Dark_BaseColor.png`, the same trousers and shoes; scaled 0.97 wide, 1.02 tall, 0.97 deep, a caster | CC0 1.0 |
| `avatars/elder.glb`, `armour/hood_elder.glb`, `armour/robe_elder.glb`, `armour/helm_elder.glb`, `armour/barbute_elder.glb`, `armour/great_helm_elder.glb`, `armour/cuirass_elder.png`, `armour/trousers_elder.png` | the elder: the woman, `Hair_Buns.gltf` tinted white, the light skin, the same trousers and shoes; scaled 0.94 tall, a caster | CC0 1.0 |
| `avatars/whelp.glb`, `armour/hood_whelp.glb`, `armour/robe_whelp.glb`, `armour/helm_whelp.glb`, `armour/barbute_whelp.glb`, `armour/great_helm_whelp.glb`, `armour/cuirass_whelp.png`, `armour/trousers_whelp.png` | the whelp: the man, `Hair_SimpleParted.gltf` tinted light brown, no beard, the male trousers and shoes; scaled 0.86 wide, 0.90 tall, 0.86 deep to the infiltrator | CC0 1.0 |
| `avatars/thief.glb`, `armour/hood_thief.glb`, `armour/robe_thief.glb`, `armour/helm_thief.glb`, `armour/barbute_thief.glb`, `armour/great_helm_thief.glb`, `armour/cuirass_thief.png`, `armour/trousers_thief.png` | the thief: the woman, `Hair_BuzzedFemale.gltf` tinted black, the dark skin, the female trousers and shoes; scaled 0.92 wide, 0.93 tall, 0.92 deep to the infiltrator | CC0 1.0 |
| `armour/cuirass_striker.glb` | Quaternius *Modular Character Outfits - Fantasy* (`Male_Ranger_Body.gltf` with its two belts, `T_Ranger_BaseColor.png`), the same scripts (`scripts/dev/places-pipeline.sh` since 2026-10-13): the overlay of the cuirass on the striker's body (CONTENT.md 3.3), 881 triangles, pushed 12 mm out; until the evening of 2026-10-13 with the pauldrons and the bracers, which were overlays of their own until the places went (EQUIPMENT.md 10.8) | CC0 1.0 |
| `armour/hood_striker.glb` | the same pack (`Male_Ranger_Head_Hood.gltf`), the same scripts: the head's overlay, 639 triangles, pushed 35 mm out (the hair came through at 12) | CC0 1.0 |
| `armour/trousers_striker.glb` | the same pack (`Male_Ranger_Legs.gltf` with `Male_Ranger_Feet_Boots.gltf`, one overlay since EQUIPMENT.md 10.8: the feet are no place), the same scripts: the legs' overlay, under 1,500 triangles | CC0 1.0 |
| `armour/robe_striker.glb` | the same pack (`Male_Peasant_Body.gltf`, the sleeves of `Male_Peasant_Arms.gltf`, `T_Peasant_BaseColor.png`), the same scripts: the robe's overlay, 1,500 triangles | CC0 1.0 |
| `props/dagger.glb` | the same pack (`dagger.gltf` + `rogue_texture.png`) | CC0 1.0 |
| `props/staff.glb` | the same pack (`staff.gltf` + `mage_texture.png`) | CC0 1.0 |
| `props/crossbow.glb` | the same pack (`crossbow_1handed.gltf` + `rogue_texture.png`) | CC0 1.0 |
| `props/axe.glb` | the same pack (`axe_1handed.gltf` + `barbarian_texture.png`), `gm-tools content import prop` | CC0 1.0 |
| `props/wand.glb` | the same pack (`wand.gltf` + `mage_texture.png`), `gm-tools content import prop` | CC0 1.0 |
| `props/focus.glb` | the same pack (`spellbook_closed.gltf` + `mage_texture.png`), `gm-tools content import prop`: the caster's focus, worn in the left hand | CC0 1.0 |
| `props/spear.glb` | Quaternius *LowPoly Medieval Weapons* (`OBJ/Spear.obj`), https://quaternius.com/packs/lowpolymedievalweapons.html; made by `scripts/dev/blender-obj-prop.py` (the grip two fifths up the shaft, the length along +Z, decimated to three quarters; flat colours, so `gm-tools content build` gives it swatches) | CC0 1.0 |
| `props/bow.glb` | the same pack (`OBJ/Bow_Wooden.obj`), the same script (the grip at the handle, the length along +Z) | CC0 1.0 |
| `props/mace.glb` | the same pack (`OBJ/Hammer_Small.obj`), the same script (the grip low on the haft, the length along +Z, decimated to 958 triangles) | CC0 1.0 |
| `props/warhammer.glb` | the same pack (`OBJ/Hammer_Double.obj`), the same script (958 triangles) | CC0 1.0 |
| `props/claymore.glb` | the same pack (`OBJ/Sword_Big.obj`: the pack's `Claymore` is a thin red blade that reads as a line), the same script (830 triangles) | CC0 1.0 |
| `props/scythe.glb` | the same pack (`OBJ/Scythe.obj`), the same script (the grip at the middle of the snath, 941 triangles) | CC0 1.0 |
| `props/katana.glb` | Quaternius *LowPoly Animated Knight* (`OBJ/Katana.obj`), https://quaternius.itch.io/lowpoly-animated-knight; the same script (512 triangles) | CC0 1.0 |
| `props/greataxe.glb` | KayKit *Adventurers* (`axe_2handed.gltf` + `barbarian_texture.png`), `gm-tools content import prop` | CC0 1.0 |
| `props/buckler.glb` | the same pack (`shield_round.gltf` + `knight_texture.png`), `gm-tools content import prop` | CC0 1.0 |
| `props/tower_shield.glb` | the same pack (`shield_square.gltf` + `knight_texture.png`), `gm-tools content import prop` | CC0 1.0 |
| `armour/helm_striker.glb` | Quaternius *LowPoly Animated Knight* (`OBJ/Helmet2.obj`, the open helm), bound whole to the striker's `head` by `scripts/dev/blender-rigid-overlay.py` (772 triangles; flat colours, so `gm-tools content build` gives it swatches) | CC0 1.0 |
| `armour/barbute_striker.glb` | the same pack (`OBJ/Helmet1.obj`, the helm with cheek guards), the same script (568 triangles) | CC0 1.0 |
| `armour/great_helm_striker.glb` | the same pack (`OBJ/Helmet3.obj`, the closed helm with the cross slit), the same script (656 triangles) | CC0 1.0 |
| `props/hammer.glb` | ours: `gm-tools content synth hammer` | CC-BY-SA 4.0 |
| `props/greatsword.glb` | ours: `gm-tools content synth greatsword` | CC-BY-SA 4.0 |
| `props/shield.glb` | Quaternius *LowPoly Medieval Weapons* (`OBJ/Shield_Heater.obj`), https://quaternius.com/packs/lowpolymedievalweapons.html; made by `scripts/dev/blender-obj-prop.py` (the grip at the back strap, the face along +Y, 984 triangles; flat colours, so `gm-tools content build` gives it swatches). Until 2026-10-13 ours, `gm-tools content synth shield` | CC0 1.0 |
| `props/musket.glb` | ours: `gm-tools content synth musket` | CC-BY-SA 4.0 |
| `props/pistol.glb` | ours: `gm-tools content synth pistol` | CC-BY-SA 4.0 |

The pack's files were packed into single `.glb`s by `gm-tools content import prop`
(the glTF JSON, its `.bin` and its texture, unchanged otherwise).

## Fonts (`ui/`)

| File | From | Licence |
|---|---|---|
| `ui/fira_sans_medium.ttf` | Fira Sans Medium, The Mozilla Foundation and Telefonica S.A., via github.com/google/fonts (`ofl/firasans/FiraSans-Medium.ttf`) | SIL OFL 1.1 (`ui/OFL-fira_sans.txt`) |
| `ui/medievalsharp.ttf` | MedievalSharp, Wojciech Kalinowski, via github.com/google/fonts (`ofl/medievalsharp`) | SIL OFL 1.1 (`ui/OFL-medievalsharp.txt`) |

## The skin (`ui/*.png`) and icons (`icons/*.png`)

Drawn for this project (`scripts/dev/skin-gen.py` writes the skin, each piece once per
density: `name.png`, `name@2x.png`, `name@3x.png`, `name@4x.png`), CC-BY-SA 4.0 unless a
row above says otherwise. The materials' icons (`icons/mat_*.png`, 2026-10-19: an ingot
a core, a stave a frame, a crystal a catalyst, a cut stone a gem, in the material's
colour) were drawn by a script of this project the same way (ITEMS.md 6.6).
