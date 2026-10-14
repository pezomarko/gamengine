# The official striker in mail (CONTENT.md 3.2, 7) from Quaternius' CC0 RPG Characters pack
# (https://quaternius.com/packs/rpgcharacters.html, glTF/Warrior.gltf), in Blender 3.3
# headless; nothing on the machine but Docker:
#
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-quaternius-warrior.py -- Warrior.gltf body.glb sword.glb 0.35
#   scripts/dev/glb-retarget.py body.glb assets/content/models/avatars/striker_mail.glb \
#       --scale 0.675 0.675 0.6075
#   gm-tools content import prop sword sword.glb
#
# What it does: the sword mesh goes to a file of its own at the origin (a prop, CONTENT.md
# 3.1: the row's `fit` lays it in the hand); the face (2,682 triangles, half the body) is
# decimated by RATIO; the bones are renamed to MODELS.md 2; the IK feet are reparented under
# the shins and the thighs under the hips (Quaternius hangs them off `Body` and `Root`); the
# pole targets go; animations are not exported. The scale that puts the head pivot in the
# striker's band (MODELS.md 4) and thins the body front to back is glb-retarget.py's.
import sys
import bpy
import mathutils

argv = sys.argv[sys.argv.index('--') + 1:]
src, out_body, out_sword, face_ratio = argv[0], argv[1], argv[2], float(argv[3])
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
arm = next(o for o in bpy.data.objects if o.type == 'ARMATURE')

sword = next(o for o in bpy.data.objects if 'Sword' in o.name)
sword.parent = None
sword.matrix_world = mathutils.Matrix.Identity(4)
bpy.ops.object.select_all(action='DESELECT')
sword.select_set(True)
bpy.ops.export_scene.gltf(filepath=out_sword, export_format='GLB', use_selection=True,
                          export_animations=False, export_skins=False, export_apply=True, export_yup=True)
bpy.data.objects.remove(sword)

face = next(o for o in bpy.data.objects if o.name.startswith('Face'))
m = face.modifiers.new('dec', 'DECIMATE')
m.ratio = face_ratio
bpy.context.view_layer.objects.active = face
bpy.ops.object.modifier_apply(modifier='dec')

ren = {'Hips': 'hips', 'Abdomen': 'spine', 'Torso': 'chest', 'Neck': 'neck', 'Head': 'head',
       'Shoulder.L': 'clavicle_l', 'UpperArm.L': 'upper_arm_l', 'LowerArm.L': 'forearm_l', 'Fist.L': 'hand_l',
       'Shoulder.R': 'clavicle_r', 'UpperArm.R': 'upper_arm_r', 'LowerArm.R': 'forearm_r', 'Fist.R': 'hand_r',
       'UpperLeg.L': 'thigh_l', 'LowerLeg.L': 'shin_l', 'Foot.L': 'foot_l',
       'UpperLeg.R': 'thigh_r', 'LowerLeg.R': 'shin_r', 'Foot.R': 'foot_r', 'Weapon.R': 'prop_r'}
bpy.context.view_layer.objects.active = arm
bpy.ops.object.mode_set(mode='EDIT')
eb = arm.data.edit_bones
for side in 'LR':
    f = eb['Foot.' + side]; f.parent = eb['LowerLeg.' + side]; f.use_connect = False
    t = eb['UpperLeg.' + side]; t.parent = eb['Hips']; t.use_connect = False
for b in list(eb):
    if b.name.startswith('PoleTarget'):
        eb.remove(b)
for b in eb:
    if b.name in ren:
        b.name = ren[b.name]
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.export_scene.gltf(filepath=out_body, export_format='GLB', export_animations=False,
                          export_skins=True, export_apply=True, export_yup=True, export_def_bones=False)
print('DONE', out_body, out_sword)
