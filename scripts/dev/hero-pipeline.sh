#!/bin/bash
# The whole hero pipeline on the box: Blender → atlases → previews → repo → build → shot.
set -e
cd ~/model-src
B="ubc/extracted/Universal Base Characters[Standard]/Base Characters/Textures"
M="mcof/extracted/Modular Character Outfits - Fantasy[Standard]/Textures"
cp "ubc/extracted/Universal Base Characters[Standard]/Hairstyles/Rigged to Head Bone/glTF (Godot -Unreal)/Hair_Buzzed.gltf" "ubc/extracted/Universal Base Characters[Standard]/Hairstyles/Rigged to Head Bone/glTF (Godot -Unreal)/Hair_Buzzed.bin" parts/
docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b --python blender-quaternius-hero.py -- "ubc/extracted/Universal Base Characters[Standard]/Base Characters/Godot - UE/Superhero_Male_FullBody.gltf" parts out "$@" 2>&1 | grep "^TRIS\|DONE\|Traceback\|Error:" | grep -v "Error: Image\|Error: Cannot read"
# The atlas (MODELS.md 3): the hero's skin in 1024 x 640 at the top, the peasant trousers
# and shoes in 512 x 384 under it (they were 256 x 256 until 2026-10-15: the trousers'
# patch, LOOK.md 6.6, is painted in this space and 256 was a blur), the eyes and the hair
# in 128 x 128 beside them; 512 x 384 at the bottom right is free.
venv/bin/python -I gltf-atlas.py out/striker_mail.glb out/striker_mail_atlas.glb --size 1024 \
  --region "MI_Superhero_Male=$B/T_Superhero_Male_Ligh.png:0,0,1024,640" \
  --region "MI_Peasant=$M/Peasant/T_Peasant_BaseColor.png:0,640,512,384" \
  --region "MI_Eyes=$B/T_Eye_Brown.png:512,640,128,128" \
  --region "MI_Hair_1=$B/T_Hair_1_BaseColor.png:640,640,128,128" --tint "MI_Hair_1=0.42,0.30,0.20"
venv/bin/python -I gltf-atlas.py out/cuirass.glb out/cuirass_atlas.glb --size 512 --region "MI_Ranger=$M/Ranger/T_Ranger_BaseColor.png:0,0,512,512"
venv/bin/python -I gltf-atlas.py out/robe.glb out/robe_atlas.glb --size 512 --region "MI_Peasant=$M/Peasant/T_Peasant_BaseColor.png:0,0,512,512"
for set in "body out/striker_mail_atlas.glb" "cuirass out/striker_mail_atlas.glb out/cuirass_atlas.glb" "robe out/striker_mail_atlas.glb out/robe_atlas.glb"; do
  set -- $set; name=$1; shift
  docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b --python blender-preview.py -- shots/$name.png 640 "$@" 2>&1 | grep "DONE\|Traceback"
done
cd shots && ~/model-src/venv/bin/python -I -c '
from PIL import Image
for base in ["body","cuirass","robe"]:
    ims=[Image.open(f"{base}_{v}.png") for v in ["front","side","back"]]
    w=sum(i.width for i in ims); h=max(i.height for i in ims)
    out=Image.new("RGB",(w,h)); x=0
    for i in ims: out.paste(i,(x,0)); x+=i.width
    out.save(f"{base}_views.png")
'
cd ~/gamengine
cp ~/model-src/out/striker_mail_atlas.glb assets/content/models/avatars/striker_mail.glb
source ~/.cargo/env 2>/dev/null || true
cargo run -q -p gm-tools -- content check 2>&1 | tail -3
cargo run -q -p gm-tools -- content build 2>&1 | tail -3
cargo run -q -p gm-tools -- content report 2>&1 | grep -A2 "^avatar "
cargo run -q -p gm-client -- --offline --map assets/maps/built/town.bsp --headless --crowd 12 --prop sword --start -905,-645,40,32 --size 1280x720 --seconds 2 --screenshot ~/model-src/shots/hero.ppm 2>&1 | tail -1
~/model-src/venv/bin/python -I -c 'from PIL import Image; Image.open("/home/pezo/model-src/shots/hero.ppm").save("/home/pezo/model-src/shots/hero.png")'
echo PIPELINE-DONE
