#!/usr/bin/env python3
"""The bodies' models (BODIES.md 3.1, CONTENT.md 3.2) on the box, each from Quaternius' CC0
packs by blender-quaternius-body.py: the body with its atlas, scaled to its frame; its hood
and robe overlays; its helms, bound to its head; and the cuirass and the trousers baked
onto its atlas as patches. Everything lands in the repo's assets/content/models/; the
avatars.toml rows and LICENSES.md are written by hand.

    scripts/dev/bodies-pipeline.py [KEY ...]        (every body but the soldier by default)

Needs ~/model-src (the packs extracted, parts/ filled, venv/ with numpy and Pillow, the
knight pack's OBJ/), Docker with nytimes/blender:3.3.1-cpu-ubuntu18.04, and a built
gm-tools (GM_TOOLS) for the ingest check. REPO is the checkout written to.
"""
import json
import os
import shutil
import subprocess
import sys

HOME = os.path.expanduser('~')
SRC = f'{HOME}/model-src'
REPO = os.environ.get('REPO', f'{HOME}/gamengine-bodies')
S = f'{REPO}/scripts/dev'
A = f'{REPO}/assets/content/models'
UBC = f'{SRC}/ubc/extracted/Universal Base Characters[Standard]'
B = f'{UBC}/Base Characters/Textures'
M = f'{SRC}/mcof/extracted/Modular Character Outfits - Fantasy[Standard]/Textures'
PY = [f'{SRC}/venv/bin/python', '-I']
BL = ['docker', 'run', '--rm', '--user', f'{os.getuid()}:{os.getgid()}', '-v', f'{HOME}:{HOME}',
      '-w', SRC, 'nytimes/blender:3.3.1-cpu-ubuntu18.04', 'blender', '-b', '--python']
GM = os.environ.get('GM_TOOLS', f'{HOME}/gamengine-main/target/release/gm-tools')

# The eight bodies but the soldier (striker_mail, hero-pipeline.sh). `scale` is in glTF
# axes (width, height, depth) against the pack's body (the man 1.81 m with his head pivot at
# 1.60, the woman 1.77 and 1.55): the frame's band is the mannequin's head pivot ±8%
# (MODELS.md 4: colossus 1.66 m, striker and caster 1.55, infiltrator 1.44).
BODIES = {
    'brute': dict(sex='Male', frame='colossus', hair='Hair_Buzzed', beard=True, skin='light',
                  tint=(0.25, 0.17, 0.11), scale=(1.25, 1.07, 1.22), ratios={'body': 0.19}),
    'veteran': dict(sex='Male', frame='colossus', hair='Hair_SimpleParted', beard=True, skin='light',
                    tint=(0.72, 0.72, 0.70), scale=(1.12, 1.07, 1.12), ratios={'body': 0.19}),
    'huntress': dict(sex='Female', frame='striker', hair='Hair_Long', beard=False, skin='light',
                     tint=(0.45, 0.20, 0.10), scale=(1.0, 1.0, 1.0), ratios={'sleeves': 0.12}),
    'sorceress': dict(sex='Female', frame='caster', hair='Hair_Long', beard=False, skin='dark',
                      tint=(0.10, 0.08, 0.08), scale=(0.97, 1.02, 0.97), ratios={'sleeves': 0.12}),
    'elder': dict(sex='Female', frame='caster', hair='Hair_Buns', beard=False, skin='light',
                  tint=(0.82, 0.82, 0.80), scale=(1.0, 0.94, 1.0), ratios={'sleeves': 0.12}),
    'whelp': dict(sex='Male', frame='infiltrator', hair='Hair_SimpleParted', beard=False, skin='light',
                  tint=(0.50, 0.34, 0.18), scale=(0.86, 0.90, 0.86), ratios={}),
    'thief': dict(sex='Female', frame='infiltrator', hair='Hair_BuzzedFemale', beard=False, skin='dark',
                  tint=(0.10, 0.08, 0.08), scale=(0.92, 0.93, 0.92), ratios={'sleeves': 0.12}),
}
SKIN = {('Male', 'light'): 'T_Superhero_Male_Ligh.png', ('Male', 'dark'): 'T_Superhero_Male_Dark.png',
        ('Female', 'light'): 'T_Superhero_Female_Light_BaseColor.png',
        ('Female', 'dark'): 'T_Superhero_Female_Dark_BaseColor.png'}
# The knight pack's helms as the striker wears them (LICENSES.md): the obj, and the width
# against the head's.
HELMS = [('helm', 'Helmet2', 1.4), ('barbute', 'Helmet1', 1.45), ('great_helm', 'Helmet3', 1.4)]
RANGER = f'{M}/Ranger/T_Ranger_BaseColor.png'
PEASANT = f'{M}/Peasant/T_Peasant_BaseColor.png'
NOISE = ('Error: Image', 'Error: Cannot read', 'Unable to pack file')


def run(cmd, keep=None):
    r = subprocess.run(cmd, capture_output=True, text=True)
    for line in (r.stdout + r.stderr).splitlines():
        if any(n in line for n in NOISE):
            continue
        if keep is None or any(k in line for k in keep):
            print('   ', line)
    if r.returncode != 0:
        sys.exit(f'failed: {" ".join(cmd)}')


def body(key, b):
    out = f'{SRC}/out/{key}'
    os.makedirs(out, exist_ok=True)
    spec = {'base': f'{UBC}/Base Characters/Godot - UE/Superhero_{b["sex"]}_FullBody.gltf',
            'sex': b['sex'], 'parts': 'parts', 'out': out, 'hair': b['hair'], 'beard': b['beard'],
            'ratios': b['ratios']}
    with open(f'{out}/spec.json', 'w') as f:
        json.dump(spec, f, indent=1)
    print(f'== {key}: Blender')
    run(BL + [f'{S}/blender-quaternius-body.py', '--', f'{out}/spec.json'],
        keep=('TRIS body', 'TRIS cuirass', 'TRIS hood', 'TRIS trousers', 'TRIS robe', 'HAIR', 'Traceback', 'Error'))
    # The atlas (MODELS.md 3): the skin in 1024 x 640 at the top, the peasant trousers and
    # shoes in 512 x 384 under it, the eyes and the two hair masks in 128 x 128 beside them.
    tint = ','.join(str(x) for x in b['tint'])
    run(PY + [f'{S}/gltf-atlas.py', f'{out}/body.glb', f'{out}/body_atlas.glb', '--size', '1024',
              '--region', f'MI_Superhero_{b["sex"]}={B}/{SKIN[(b["sex"], b["skin"])]}:0,0,1024,640',
              '--region', f'MI_Peasant={PEASANT}:0,640,512,384',
              '--region', f'MI_Eyes={B}/T_Eye_Brown.png:512,640,128,128',
              '--region', f'MI_Hair_1={B}/T_Hair_1_BaseColor.png:640,640,128,128', '--tint', f'MI_Hair_1={tint}',
              '--region', f'MI_Hair_2={B}/T_Hair_2_BaseColor.png:768,640,128,128', '--tint', f'MI_Hair_2={tint}'],
        keep=('bytes', 'Traceback', 'Error'))
    scale = ['--scale'] + [str(x) for x in b['scale']]
    run(PY + [f'{S}/glb-retarget.py', f'{out}/body_atlas.glb', f'{out}/{key}.glb'] + scale, keep=('wrote', 'Error'))
    shutil.copy(f'{out}/{key}.glb', f'{A}/avatars/{key}.glb')
    print(f'== {key}: ingest as a {b["frame"]}')
    run([GM, 'model', 'ingest', '--frame', b['frame'], f'{out}/{key}.glb', '--out', out])
    # The overlays worn over the body (CONTENT.md 3.3): the hood and the robe, each a 512
    # atlas, scaled as the body is.
    for name, mat, tex in [('hood', 'MI_Ranger', RANGER), ('robe', 'MI_Peasant', PEASANT)]:
        run(PY + [f'{S}/gltf-atlas.py', f'{out}/{name}.glb', f'{out}/{name}_atlas.glb', '--size', '512',
                  '--region', f'{mat}={tex}:0,0,512,512'], keep=('Traceback', 'Error'))
        run(PY + [f'{S}/glb-retarget.py', f'{out}/{name}_atlas.glb', f'{out}/{name}_{key}.glb'] + scale, keep=('Error',))
        shutil.copy(f'{out}/{name}_{key}.glb', f'{A}/armour/{name}_{key}.glb')
    # The patches (LOOK.md 6.6): the cuirass and the trousers, cut for this body and scaled
    # with it, baked onto its atlas.
    print(f'== {key}: patches')
    for name in ('cuirass', 'trousers'):
        run(PY + [f'{S}/glb-retarget.py', f'{out}/{name}.glb', f'{out}/{name}_scaled.glb'] + scale, keep=('Error',))
        run(BL + [f'{S}/blender-bake-patch.py', '--', f'{out}/{key}.glb', f'{out}/bake_{name}', '1024',
                  f'{out}/{name}_scaled.glb={RANGER}'], keep=('PIECE', 'Traceback', 'Error'))
        run(PY + [f'{S}/patch-join.py', f'{out}/bake_{name}', f'{A}/armour/{name}_{key}.png'])
    # The helms, bound whole to this body's head.
    print(f'== {key}: helms')
    for helm, obj, width in HELMS:
        run(BL + [f'{S}/blender-rigid-overlay.py', '--', f'{out}/{key}.glb', f'{SRC}/knight/extracted/OBJ/{obj}.obj',
                  f'{out}/{helm}_{key}.glb', 'head', str(width), '0', '0', '0.03', '1.0'],
            keep=('TRIS', 'Traceback', 'Error'))
        shutil.copy(f'{out}/{helm}_{key}.glb', f'{A}/armour/{helm}_{key}.glb')
    # Previews: bare, and in the hood and the robe.
    run(BL + [f'{S}/blender-preview.py', '--', f'{out}/bare.png', '640', f'{out}/{key}.glb'], keep=('Traceback',))
    run(BL + [f'{S}/blender-preview.py', '--', f'{out}/dressed.png', '640', f'{out}/{key}.glb',
              f'{out}/hood_{key}.glb', f'{out}/robe_{key}.glb'], keep=('Traceback',))
    print(f'== {key}: done')


keys = sys.argv[1:] or list(BODIES)
for key in keys:
    body(key, BODIES[key])
print('BODIES-DONE')
