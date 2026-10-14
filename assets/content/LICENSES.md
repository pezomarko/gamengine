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
| `armour/cuirass_striker.glb` | Quaternius *Modular Character Outfits - Fantasy* (`Male_Ranger_Body.gltf` with its two belts, `T_Ranger_BaseColor.png`), the same scripts (`scripts/dev/places-pipeline.sh` since 2026-10-13): the overlay of the cuirass on the striker's body (CONTENT.md 3.3), 881 triangles, pushed 12 mm out; until the evening of 2026-10-13 with the pauldrons and the bracers, which are overlays of their own now (EQUIPMENT.md 5.2) | CC0 1.0 |
| `armour/pauldrons_striker.glb` | the same pack (`Male_Ranger_Acc_Pauldron.gltf` mirrored to both shoulders), the same scripts: the shoulders' overlay, 302 triangles | CC0 1.0 |
| `armour/bracers_striker.glb` | the same pack (the bracers of `Male_Ranger.gltf`), the same scripts: the hands' overlay, 290 triangles | CC0 1.0 |
| `armour/hood_striker.glb` | the same pack (`Male_Ranger_Head_Hood.gltf`), the same scripts: the head's overlay, 639 triangles | CC0 1.0 |
| `armour/trousers_striker.glb` | the same pack (`Male_Ranger_Legs.gltf`), the same scripts: the legs' overlay, 564 triangles | CC0 1.0 |
| `armour/boots_striker.glb` | the same pack (`Male_Ranger_Feet_Boots.gltf`), the same scripts: the feet's overlay, under 1,500 triangles | CC0 1.0 |
| `armour/cape_striker.glb` | ours: a cloth plane hung from the shoulders down the back to the knees, following the body's profile and wrapped a little round it, skinned to `chest`, `spine` and `hips` by height, two-sided, 192 triangles, made in the same Blender pass (`cape` in `scripts/dev/blender-quaternius-hero.py`); its 64² swatch (red cloth, a gold hem) is drawn by `scripts/dev/places-pipeline.sh` | CC-BY-SA 4.0 |
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
| `armour/plate_pauldrons_striker.glb` | the same pack (`OBJ/ShoulderPads.obj`), the same script, bound to `chest` (96 triangles) | CC0 1.0 |
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
row above says otherwise.
