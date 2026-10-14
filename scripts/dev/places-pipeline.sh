#!/bin/bash
# The body's other places (EQUIPMENT.md 5.2) on the box: Blender cuts every overlay of the
# striker from the packs (scripts/dev/blender-quaternius-hero.py: the cuirass without its
# pauldrons and bracers, and the pauldrons, bracers, hood, trousers and boots as overlays
# of their own, and our cape as a cloth plane), gltf-atlas.py packs each one's texture
# (the cape's is a 64² swatch drawn here), and the files go to the repo.
# The body itself is not copied (hero-pipeline.sh does that). Then `gm-tools content
# check` and `build` as hero-pipeline.sh runs them.
set -e
cd ~/model-src
B="ubc/extracted/Universal Base Characters[Standard]/Base Characters/Textures"
M="mcof/extracted/Modular Character Outfits - Fantasy[Standard]/Textures"
P="mcof/extracted/Modular Character Outfits - Fantasy[Standard]/Exports/glTF (Godot-Unreal)/Modular Parts"
for part in Male_Ranger_Head_Hood Male_Ranger_Legs Male_Ranger_Feet_Boots Male_Ranger_Body Male_Ranger_Acc_Pauldron; do
  [[ -f parts/$part.gltf ]] || cp "$P/$part.gltf" "$P/$part.bin" parts/
done
docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b --python blender-quaternius-hero.py -- "ubc/extracted/Universal Base Characters[Standard]/Base Characters/Godot - UE/Superhero_Male_FullBody.gltf" parts out "$@" 2>&1 | grep "^TRIS\|DONE\|Traceback\|Error:" | grep -v "Error: Image\|Error: Cannot read"
for name in cuirass pauldrons bracers hood trousers boots; do
  venv/bin/python -I gltf-atlas.py out/$name.glb out/${name}_atlas.glb --size 512 --region "MI_Ranger=$M/Ranger/T_Ranger_BaseColor.png:0,0,512,512"
  cp out/${name}_atlas.glb ~/gamengine/assets/content/models/armour/${name}_striker.glb
done
# The cape's swatch (EQUIPMENT.md 5.3, LICENSES.md): red cloth with a gold hem, 64².
venv/bin/python -I -c '
from PIL import Image
import random
random.seed(7)
im = Image.new("RGB", (64, 64)); px = im.load()
for y in range(64):
    for x in range(64):
        n = random.randint(-7, 7)
        c = (176, 138, 58) if y >= 57 else (60, 12, 16) if y >= 55 else (118, 22, 30)
        px[x, y] = tuple(v + n for v in c)
im.save("parts/T_Cape.png")
'
venv/bin/python -I gltf-atlas.py out/cape.glb out/cape_atlas.glb --size 64 --region "MI_Cape=parts/T_Cape.png:0,0,64,64"
cp out/cape_atlas.glb ~/gamengine/assets/content/models/armour/cape_striker.glb
cd ~/gamengine
source ~/.cargo/env 2>/dev/null || true
cargo run -q -p gm-tools -- content check 2>&1 | tail -3
cargo run -q -p gm-tools -- content build 2>&1 | tail -3
echo PLACES-DONE
