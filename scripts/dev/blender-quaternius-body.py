# A body (BODIES.md 3.1, CONTENT.md 3.2) and the overlays cut for it from Quaternius' CC0
# packs *Universal Base Characters* (the Superhero male or female full body, a hairstyle,
# the beard) and *Modular Character Outfits - Fantasy* (the Peasant and Ranger parts of the
# same sex), in Blender 3.3 headless. blender-quaternius-hero.py made the striker from the
# male alone; this is that script with the sex, the hair, the beard and the ratios read
# from a JSON spec, so that the eight bodies come from one file each:
#
#   docker run --rm -v "$HOME:$HOME" -w "$PWD" nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-quaternius-body.py -- SPEC.json
#
#   { "base": "Superhero_Female_FullBody.gltf", "sex": "Female", "parts": "parts",
#     "out": "out/huntress", "hair": "Hair_Long", "beard": false, "ratios": { "sleeves": 0.12 } }
#
# PARTS_DIR holds the outfit parts (<Sex>_Peasant_*, <Sex>_Ranger_*) and the hairstyles
# rigged to the head bone. Every part of both packs is skinned to one UE-style armature, so
# the parts are re-bound to the base body's armature and exported with it: the body (the
# torso and head, the peasant's trousers and shoes, the hair, the beard) and each overlay
# (the ranger jerkin and belts as `cuirass`, the hood as `hood`, the ranger trousers and
# boots as `trousers`, the peasant tunic and sleeves as `robe`) share one skeleton and one
# set of rest pivots. Bones are renamed to MODELS.md 2; `prop_r`, `prop_l` (in the palms)
# and `prop_back` (behind the chest) are added. The body's legs under the trousers are cut;
# each object is decimated by its ratio; the head is protected from the body's decimation;
# the hair's ratio is whatever fills the avatar's 3,500 after the rest. An overlay is pushed
# 12 mm out along its normals. Everything is exported at the pack's size, without images:
# scripts/dev/bodies-pipeline.sh packs the atlas (gltf-atlas.py) and scales the files to the
# body's frame (glb-retarget.py).
import json
import sys
import bpy
import bmesh
from mathutils import Vector

argv = sys.argv[sys.argv.index('--') + 1:]
spec = json.load(open(argv[0]))
base, parts, out, sex = spec['base'], spec['parts'], spec['out'], spec['sex']
hair_name, beard = spec.get('hair'), spec.get('beard', False)
ratios = dict(body=0.215, eyes=0.2, brows=0.1, hair=0.3, beard=0.2, legs=0.5, feet=0.17,
              ranger_body=0.23, belt=0.12, peasant_body=0.28, sleeves=0.3, hood=0.3,
              ranger_legs=0.4, boots=0.09)
ratios.update(spec.get('ratios', {}))
# The avatar's triangles (CONTENT.md 4) less a margin for the exporter's seams.
BUDGET = int(spec.get('budget', 3450))
INFLATE = 0.012
pre = sex + '_'

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=base)
arm = next(o for o in bpy.data.objects if o.type == 'ARMATURE')
base_meshes = [o for o in bpy.data.objects if o.type == 'MESH']
body = max(base_meshes, key=lambda o: len(o.data.vertices))
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


hair = bring(hair_name) if hair_name else []
beard_objs = bring('Hair_Beard') if beard else []
legs = bring(pre + 'Peasant_Legs')
feet = bring(pre + 'Peasant_Feet')
ranger_body = bring(pre + 'Ranger_Body')  # the jerkin and two belts
peasant_body = bring(pre + 'Peasant_Body')
peasant_arms = bring(pre + 'Peasant_Arms')
hood = bring(pre + 'Ranger_Head_Hood')
ranger_legs = bring(pre + 'Ranger_Legs')
boots = bring('Male_Ranger_Feet_Boots' if sex == 'Male' else 'Female_Ranger_Feet')

# The peasant arms: the sleeves (MI_Peasant) only; a skin primitive goes.
sleeves = peasant_arms[0]
skin_slots = [i for i, s in enumerate(sleeves.material_slots) if 'Peasant' not in s.material.name]
if skin_slots:
    bm = bmesh.new()
    bm.from_mesh(sleeves.data)
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


def tris(o):
    o.data.update()
    return sum(len(p.vertices) - 2 for p in o.data.polygons)


def decimate(o, ratio, protect=None):
    """Collapse to `ratio` of the triangles; `protect(z)` says how much a vertex at a
    height is spared (0 untouched, 1 decimated as any)."""
    if ratio >= 1.0:
        return
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


decimate(eyes, ratios['eyes'])
decimate(brows, ratios['brows'])
for o in beard_objs:
    decimate(o, ratios['beard'])
for o in legs:
    decimate(o, ratios['legs'])
for o in feet:
    decimate(o, ratios['feet'])
# The body takes what the budget leaves after the small parts and the hair's share (the
# woman's torso cut at the waist is more triangles than the man's), up to its ratio; the
# hair takes what is then left, up to its own.
HAIR_SHARE = 420
small = sum(tris(o) for o in [eyes, brows] + beard_objs + legs + feet)
hair_share = min(HAIR_SHARE, int(sum(tris(o) for o in hair) * ratios['hair']))
ratio = min(ratios['body'], (BUDGET - small - hair_share) / max(1, tris(body)))
print('BODY ratio %.3f of %d' % (ratio, tris(body)))
decimate(body, ratio, lambda z: 0.35 if z > 1.5 else 1.0)
rest = small + tris(body)
for o in hair:
    ratio = min(ratios['hair'], max(0.05, (BUDGET - rest) / max(1, tris(o))))
    print('HAIR ratio %.3f of %d' % (ratio, tris(o)))
    decimate(o, ratio)
for o in ranger_body:
    decimate(o, ratios['belt'] if 'Belt' in o.name else ratios['ranger_body'])
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


for o in ranger_body + peasant_body + [sleeves] + hood + ranger_legs + boots:
    inflate(o, INFLATE)
# The hood 35 mm in all: at 12 and at 20 the buzzed hair came through its cloth (EQUIPMENT.md 10.8).
for o in hood:
    inflate(o, 0.023)


def clear(o, target, margin):
    """Push every vertex of `o` that lies inside `target` (or nearer its surface than
    `margin`) out to the surface and `margin` beyond it, along the surface's normal; the
    rest stay. The packs' pieces were modelled on their own body, not this one."""
    bpy.context.view_layer.objects.active = o
    mod = o.modifiers.new('clear', 'SHRINKWRAP')
    mod.target = target
    mod.wrap_method = 'NEAREST_SURFACEPOINT'
    mod.wrap_mode = 'ABOVE_SURFACE'
    mod.offset = margin
    bpy.ops.object.modifier_apply(modifier=mod.name)


for o in ranger_body + hood:
    clear(o, body, 0.012)
# The hood covers no bone (items.toml): long hair and buns under it are drawn, so the
# cloth is pushed out past the hair as it is past the skin.
for o in hood:
    for h in hair:
        clear(o, h, 0.012)


def raise_waist(o, above, by):
    """Stretch the waistband up: every vertex above `above` (metres, world) goes up `by`."""
    bm = bmesh.new()
    bm.from_mesh(o.data)
    for v in bm.verts:
        if (o.matrix_world @ v.co).z > above:
            v.co += o.matrix_world.inverted().to_3x3() @ Vector((0.0, 0.0, by))
    bm.to_mesh(o.data)
    bm.free()


for o in ranger_legs:
    raise_waist(o, 1.0, 0.03)

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
        print('TRIS', o.name, tris(o))


def export(name, objs):
    bpy.ops.object.select_all(action='DESELECT')
    arm.select_set(True)
    for o in objs:
        o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=f'{out}/{name}.glb', export_format='GLB', use_selection=True,
                              export_animations=False, export_skins=True, export_apply=True,
                              export_yup=True, export_def_bones=False, export_image_format='NONE',
                              export_colors=False)
    print('TRIS', name, sum(tris(o) for o in objs))


export('body', [body, eyes, brows] + hair + beard_objs + legs + feet)
export('cuirass', ranger_body)
export('hood', hood)
export('trousers', ranger_legs + boots)
export('robe', peasant_body + [sleeves])
print('DONE')
