#!/bin/bash
# The body's other places (EQUIPMENT.md 5.2 and 10.8) on the box (the ranger legs at 0.4 and
# the boots at 0.09 make the one `trousers` overlay under 1,500 triangles): Blender cuts every overlay of the
# striker from the packs (scripts/dev/blender-quaternius-hero.py: the cuirass without its
# pauldrons and bracers, and the pauldrons, bracers, hood, trousers and boots as overlays
# of their own, and our cape as a cloth plane), gltf-atlas.py packs each one's texture
# (the cape's is a 64² swatch drawn here), and the files go to the repo.
# The body itself is not copied (hero-pipeline.sh does that). Then `gm-tools content
# check` and `build` as hero-pipeline.sh runs them. (blender-bake-patch.py and patch-join.py
# are copies of scripts/dev/'s in ~/model-src, as blender-quaternius-hero.py is.)
set -e
cd ~/model-src
B="ubc/extracted/Universal Base Characters[Standard]/Base Characters/Textures"
M="mcof/extracted/Modular Character Outfits - Fantasy[Standard]/Textures"
P="mcof/extracted/Modular Character Outfits - Fantasy[Standard]/Exports/glTF (Godot-Unreal)/Modular Parts"
for part in Male_Ranger_Head_Hood Male_Ranger_Legs Male_Ranger_Feet_Boots Male_Ranger_Body Male_Ranger_Acc_Pauldron; do
  [[ -f parts/$part.gltf ]] || cp "$P/$part.gltf" "$P/$part.bin" parts/
done
docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b --python blender-quaternius-hero.py -- "ubc/extracted/Universal Base Characters[Standard]/Base Characters/Godot - UE/Superhero_Male_FullBody.gltf" parts out 0.215 0.2 0.1 0.3 0.5 0.17 0.23 0.12 0.11 0.08 0.28 0.3 0.3 0.4 0.09 "$@" 2>&1 | grep "^TRIS\|DONE\|Traceback\|Error:" | grep -v "Error: Image\|Error: Cannot read"
for name in cuirass hood trousers; do  # the other cuts (pauldrons, bracers, boots, cape) went with their places, EQUIPMENT.md 10.8
  venv/bin/python -I gltf-atlas.py out/$name.glb out/${name}_atlas.glb --size 512 --region "MI_Ranger=$M/Ranger/T_Ranger_BaseColor.png:0,0,512,512"
  cp out/${name}_atlas.glb ~/gamengine/assets/content/models/armour/${name}_striker.glb
done
# The cloth inside the body's silhouette is a patch on the body's own atlas (LOOK.md 6.6,
# CONTENT.md 3.3): the cuirass and the trousers baked onto the official avatar's atlas
# layout (hero-pipeline.sh made out/striker_mail_atlas.glb) from their cuts and the
# pack's texture, the colour and the coverage joined into one RGBA source. The cut (the
# glb above) stays for the icon.
mkdir -p bake
for name in cuirass trousers; do
  docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
    --python blender-bake-patch.py -- out/striker_mail_atlas.glb bake/$name 1024 \
    "out/$name.glb=$M/Ranger/T_Ranger_BaseColor.png" 2>&1 | grep "PIECE\|DONE\|Traceback\|Error:" || true
  venv/bin/python -I patch-join.py bake/$name ~/gamengine/assets/content/models/armour/${name}_striker_mail.png
done
cd ~/gamengine
source ~/.cargo/env 2>/dev/null || true
cargo run -q -p gm-tools -- content check 2>&1 | tail -3
cargo run -q -p gm-tools -- content build 2>&1 | tail -3
echo PLACES-DONE
