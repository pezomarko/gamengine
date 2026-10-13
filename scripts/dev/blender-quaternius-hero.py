# The lean striker (CONTENT.md 3.2, 7) and its armour overlays from Quaternius' CC0 packs
# *Universal Base Characters* (Superhero_Male_FullBody.gltf) and *Modular Character
# Outfits - Fantasy* (the Peasant and Ranger parts), in Blender 3.3 headless:
#
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-quaternius-hero.py -- BASE.gltf PARTS_DIR OUT_DIR \
#       [body eyes brows hair legs feet ranger_body belt pauldron bracer peasant_body sleeves]
#
# PARTS_DIR holds the outfit parts (Male_Peasant_*, Male_Ranger_*, the whole Male_Ranger for
# its bracers) and the base pack's Hair_Buzzed rigged to the head bone.
#
# Every part of both packs is skinned to one UE-style armature (pelvis, spine_01..03, neck_01,
# Head, clavicle/upperarm/lowerarm/hand, thigh/calf/foot/ball, fingers), so the parts are
# re-bound to the base body's armature and exported with it: the body (the hero's torso and
# head, the peasant's trousers and shoes) and each overlay (the ranger jerkin, belts, pauldrons
# as `cuirass`; the pauldrons as `pauldrons`, the bracers as `bracers`, the ranger hood as
# `hood`, trousers as `trousers` and boots as `boots`, EQUIPMENT.md 5.2; the peasant tunic
# and sleeves as `robe`) share one skeleton and one set of rest pivots. Bones are renamed to MODELS.md 2; `prop_r`, `prop_l` (in the palms)
# and `prop_back` (behind the chest) are added. The body's legs under the trousers are cut;
# each object is decimated by its ratio (the numbers after OUT_DIR, defaults below); the
# head is protected from the body's decimation. An overlay is pushed 12 mm out along its
# normals: the outfits were cut for the pack's regular body and the hero's chest came through
# them. Materials are exported without images: scripts/dev/gltf-atlas.py packs the textures
# into one atlas and remaps the UVs.
import sys
import bpy
import bmesh
from mathutils import Vector

argv = sys.argv[sys.argv.index('--') + 1:]
base, parts, out = argv[0], argv[1], argv[2]
ratios = dict(zip(['body', 'eyes', 'brows', 'hair', 'legs', 'feet', 'ranger_body', 'belt', 'pauldron',
                   'bracer', 'peasant_body', 'sleeves', 'hood', 'ranger_legs', 'boots'],
                  [0.215, 0.2, 0.1, 0.3, 0.5, 0.17, 0.23, 0.12, 0.11, 0.08, 0.28, 0.3, 0.3, 0.5, 0.17]))
INFLATE = 0.012
for k, v in zip(list(ratios), argv[3:]):
    ratios[k] = float(v)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=base)
arm = next(o for o in bpy.data.objects if o.type == 'ARMATURE')
body = bpy.data.objects['SuperHero_Male']
eyes = bpy.data.objects['Eyes']
brows = bpy.data.objects['Eyebrows']


def bring(name):
    """Import a part file and bind its meshes to the base armature; the part's own goes."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=f'{parts}/{name}.gltf')
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == 'MESH']
    for o in meshes:
        o.parent = arm
        o.matrix_parent_inverse = arm.matrix_world.inverted()
        for m in o.modifiers:
            if m.type == 'ARMATURE':
                m.object = arm
    for o in new:
        if o.type != 'MESH':
            bpy.data.objects.remove(o)
    return meshes


hair = bring('Hair_Buzzed')
legs = bring('Male_Peasant_Legs')
feet = bring('Male_Peasant_Feet')
ranger_body = bring('Male_Ranger_Body')  # the jerkin and two belts
pauldron = bring('Male_Ranger_Acc_Pauldron')
ranger = bring('Male_Ranger')            # the whole outfit: only the bracers are not a part file
peasant_body = bring('Male_Peasant_Body')
peasant_arms = bring('Male_Peasant_Arms')
# The other places (EQUIPMENT.md 5.2): the ranger's hood, trousers and boots.
hood = bring('Male_Ranger_Head_Hood')
ranger_legs = bring('Male_Ranger_Legs')
boots = bring('Male_Ranger_Feet_Boots')

# Of the whole ranger, keep the bracers only (the arms' skin and sleeves are the body's).
bracer = [o for o in ranger if 'Bracer' in o.name]
for o in ranger:
    if 'Bracer' not in o.name:
        bpy.data.objects.remove(o)
# The peasant arms: the sleeves (MI_Peasant) only; the skin primitive goes.
sleeves = peasant_arms[0]
bm = bmesh.new()
bm.from_mesh(sleeves.data)
skin_slots = [i for i, s in enumerate(sleeves.material_slots) if 'Peasant' not in s.material.name]
bmesh.ops.delete(bm, geom=[f for f in bm.faces if f.material_index in skin_slots], context='FACES')
bm.to_mesh(sleeves.data)
bm.free()

# The body's legs are under the trousers: cut everything under 1.0 m.
bm = bmesh.new()
bm.from_mesh(body.data)
low = [v for v in bm.verts if (body.matrix_world @ v.co).z < 1.0]
bmesh.ops.delete(bm, geom=low, context='VERTS')
bm.to_mesh(body.data)
bm.free()

# The other pauldron: a mirrored copy with its vertex groups swapped side for side.
other = pauldron[0].copy()
other.data = pauldron[0].data.copy()
other.name = 'Male_Ranger_Acc_Pauldron_Other'
bpy.context.collection.objects.link(other)
bpy.context.view_layer.objects.active = other
bpy.ops.object.select_all(action='DESELECT')
other.select_set(True)
other.scale.x *= -1
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
bm = bmesh.new()
bm.from_mesh(other.data)
bmesh.ops.reverse_faces(bm, faces=bm.faces)
bm.to_mesh(other.data)
bm.free()
for g in other.vertex_groups:
    if g.name.endswith('_l'):
        g.name = g.name[:-2] + '_R_'
    elif g.name.endswith('_r'):
        g.name = g.name[:-2] + '_l'
for g in other.vertex_groups:
    if g.name.endswith('_R_'):
        g.name = g.name[:-3] + '_r'
pauldron.append(other)


def decimate(o, ratio, protect=None):
    """Collapse to `ratio` of the triangles; `protect(z)` says how much a vertex at a
    height is spared (0 untouched, 1 decimated as any)."""
    bpy.context.view_layer.objects.active = o
    if protect is not None:
        g = o.vertex_groups.new(name='decimate')
        for v in o.data.vertices:
            g.add([v.index], protect((o.matrix_world @ v.co).z), 'REPLACE')
    m = o.modifiers.new('dec', 'DECIMATE')
    m.ratio = ratio
    if protect is not None:
        m.vertex_group = 'decimate'
        m.vertex_group_factor = 1.0
    bpy.ops.object.modifier_apply(modifier='dec')


decimate(body, ratios['body'], lambda z: 0.35 if z > 1.5 else 1.0)
decimate(eyes, ratios['eyes'])
decimate(brows, ratios['brows'])
for o in hair:
    decimate(o, ratios['hair'])
for o in legs:
    decimate(o, ratios['legs'])
for o in feet:
    decimate(o, ratios['feet'])
for o in ranger_body:
    decimate(o, ratios['belt'] if 'Belt' in o.name else ratios['ranger_body'])
for o in pauldron:
    decimate(o, ratios['pauldron'])
for o in bracer:
    decimate(o, ratios['bracer'])
for o in peasant_body:
    decimate(o, ratios['peasant_body'])
decimate(sleeves, ratios['sleeves'])
for o in hood:
    decimate(o, ratios['hood'])
for o in ranger_legs:
    decimate(o, ratios['ranger_legs'])
for o in boots:
    decimate(o, ratios['boots'])


def inflate(o, by):
    bm = bmesh.new()
    bm.from_mesh(o.data)
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * by
    bm.to_mesh(o.data)
    bm.free()


for o in ranger_body + pauldron + bracer + peasant_body + [sleeves] + hood + ranger_legs + boots:
    inflate(o, INFLATE)

# The rig: MODELS.md 2 names, and the prop bones.
ren = {'pelvis': 'hips', 'spine_01': 'spine', 'spine_02': 'chest', 'neck_01': 'neck', 'Head': 'head'}
for s in 'lr':
    ren.update({f'upperarm_{s}': f'upper_arm_{s}', f'lowerarm_{s}': f'forearm_{s}',
                f'calf_{s}': f'shin_{s}', f'ball_{s}': f'toe_{s}'})
bpy.context.view_layer.objects.active = arm
bpy.ops.object.mode_set(mode='EDIT')
eb = arm.data.edit_bones
for b in eb:
    if b.name in ren:
        b.name = ren[b.name]
for s in 'lr':
    hand, mid = eb[f'hand_{s}'], eb[f'middle_01_{s}']
    p = eb.new(f'prop_{s}')
    p.head = hand.head + (mid.head - hand.head) * 0.55
    p.tail = p.head + (mid.head - hand.head).normalized() * 0.04
    p.parent = hand
    p.use_connect = False
chest = eb['chest']
p = eb.new('prop_back')
p.head = chest.head + Vector((0.0, 0.14, 0.12))
p.tail = p.head + Vector((0.0, 0.0, 0.05))
p.parent = chest
p.use_connect = False
bpy.ops.object.mode_set(mode='OBJECT')

for o in bpy.data.objects:
    if o.type == 'MESH':
        o.data.update()
        print('TRIS', o.name, sum(len(p.vertices) - 2 for p in o.data.polygons))


def export(name, objs):
    bpy.ops.object.select_all(action='DESELECT')
    arm.select_set(True)
    for o in objs:
        o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=f'{out}/{name}.glb', export_format='GLB', use_selection=True,
                              export_animations=False, export_skins=True, export_apply=True,
                              export_yup=True, export_def_bones=False, export_image_format='NONE',
                              export_colors=False)
    print('TRIS', name, sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in objs))


export('striker_mail', [body, eyes, brows] + hair + legs + feet)
# One overlay a place (EQUIPMENT.md 5.2): the cuirass is the jerkin and its belts since
# the pauldrons and the bracers got places of their own.
export('cuirass', ranger_body)
export('pauldrons', pauldron)
export('bracers', bracer)
export('hood', hood)
export('trousers', ranger_legs)
export('boots', boots)
export('robe', peasant_body + [sleeves])
print('DONE')
