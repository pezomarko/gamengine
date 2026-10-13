# Licences of the content's files

Every file under `assets/content/` that is not written by this project is listed here with
where it came from and under what terms (CONTENT.md 2 and 11: CC0 and ours only). The
official content as a whole is CC-BY-SA 4.0 (LICENSE.md at the root).

## Models (`models/`)

Props under `props/`, official avatars under `avatars/` (CONTENT.md 2).

| File | From | Licence |
|---|---|---|
| `props/sword.glb` | Quaternius *RPG Characters* (`glTF/Warrior.gltf`: the `Warrior_Sword` mesh with `Warrior_Sword_Texture`), https://quaternius.com/packs/rpgcharacters.html; split off by `scripts/dev/blender-quaternius-warrior.py`, packed by `gm-tools content import prop` (until 2026-10-13 the KayKit Adventurers `sword_1handed.gltf`, CC0, which the dagger, staff and crossbow still come from) | CC0 1.0 |
| `avatars/striker_mail.glb` | Quaternius *RPG Characters* (`glTF/Warrior.gltf`: the body, the face and the shoulder pads with `Warrior_Texture`), https://quaternius.com/packs/rpgcharacters.html; made by `scripts/dev/blender-quaternius-warrior.py` (bones renamed to MODELS.md 2, feet and thighs reparented, the face decimated to 938 triangles, the sword split off, animations dropped) and `scripts/dev/glb-retarget.py --scale 0.675 0.675 0.6075` (into the striker's envelope, 10% thinner front to back) | CC0 1.0 |
| `props/dagger.glb` | the same pack (`dagger.gltf` + `rogue_texture.png`) | CC0 1.0 |
| `props/staff.glb` | the same pack (`staff.gltf` + `mage_texture.png`) | CC0 1.0 |
| `props/crossbow.glb` | the same pack (`crossbow_1handed.gltf` + `rogue_texture.png`) | CC0 1.0 |
| `props/hammer.glb` | ours: `gm-tools content synth hammer` | CC-BY-SA 4.0 |
| `props/greatsword.glb` | ours: `gm-tools content synth greatsword` | CC-BY-SA 4.0 |
| `props/shield.glb` | ours: `gm-tools content synth shield` | CC-BY-SA 4.0 |
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
