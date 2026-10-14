# The lean striker (CONTENT.md 3.2, 7) and its armour overlays from Quaternius' CC0 packs
# *Universal Base Characters* (Superhero_Male_FullBody.gltf) and *Modular Character
# Outfits - Fantasy* (the Peasant and Ranger parts), in Blender 3.3 headless:
#
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-quaternius-hero.py -- BASE.gltf PARTS_DIR OUT_DIR \
#       [body eyes brows hair legs feet ranger_body belt pauldron bracer peasant_body sleeves]
#
# The cape (EQUIPMENT.md 5.3) is ours, made here and not from a pack: a cloth plane hung
# from the shoulders down the back to the knees (`cape`), see cape_mesh below.
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
                  [0.215, 0.2, 0.1, 0.3, 0.5, 0.17, 0.23, 0.12, 0.11, 0.08, 0.28, 0.3, 0.3, 0.5, 0.16]))
# (The boots at 0.17 are 1,559 triangles, over an overlay's 1,500; 0.16 is the committed 1,467.)
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
# The hood 35 mm in all: at 12 and at 20 the buzzed hair came through its cloth (EQUIPMENT.md 10.8).
for o in hood:
    inflate(o, 0.023)


def clear(o, target, margin):
    """Push every vertex of `o` that lies inside `target` (or nearer its surface than
    `margin`) out to the surface and `margin` beyond it, along the surface's normal; the
    rest stay. The packs' pieces were modelled on their own body, not this one: the
    jerkin's small of the back and the shoulders sat 2-3 cm inside the hero's skin, which
    came through where the body is drawn under the overlay (the hips, the arms)."""
    bpy.context.view_layer.objects.active = o
    mod = o.modifiers.new('clear', 'SHRINKWRAP')
    mod.target = target
    mod.wrap_method = 'NEAREST_SURFACEPOINT'
    mod.wrap_mode = 'ABOVE_SURFACE'
    mod.offset = margin
    bpy.ops.object.modifier_apply(modifier=mod.name)


# (Not the robe: its tunic and sleeves overlap at the shoulders, and either pushed out
# comes through the other; the tunic as it comes clears the hero's back already. Not the
# trousers and the boots: the body's legs were cut above, so there is no surface to clear,
# and against the torso's underside they collapsed; they fit as they come.)
for o in ranger_body + hood:
    clear(o, body, 0.012)


def raise_waist(o, above, by):
    """Stretch the waistband up: every vertex above `above` (metres, world) goes up `by`.
    The trousers cover the hips (EQUIPMENT.md 5.2, items.toml), whose skin reaches 1.07 m
    on the hero while the ranger's waistband stopped at 1.06: under a jerkin that lifts
    with the chest the band between showed."""
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


def cape_mesh():
    """The cape (EQUIPMENT.md 5.3): a grid of cloth hung from the shoulder line (just under
    the neck) down the back to the knees, each row as far out as the body's back is at that
    height and 2.5 cm more, never nearer than the row above (it hangs), its edges brought
    forward round the sides most at the shoulders; skinned by height to the chest, the
    spine and the hips. Two-sided, one material (MI_Cape, a swatch in the atlas), UVs
    across and down."""
    head = {n: (arm.matrix_world @ arm.data.bones[n].head_local).copy()
            for n in ['hips', 'spine', 'chest', 'neck', 'upper_arm_l', 'shin_l']}
    top = head['neck'].z - 0.03
    bottom = head['shin_l'].z + 0.02
    half_top = abs(head['upper_arm_l'].x) * 0.80
    half_bottom = half_top * 1.35
    cols, rows = 9, 13
    pts = [o.matrix_world @ v.co for o in [body] + legs for v in o.data.vertices]

    def weights_at(z):
        c, s, h = head['chest'].z, head['spine'].z, head['hips'].z
        if z >= c:
            return {'chest': 1.0}
        if z >= s:
            k = (z - s) / (c - s)
            return {'chest': k, 'spine': 1.0 - k}
        if z >= h:
            k = (z - h) / (s - h)
            return {'spine': k, 'hips': 1.0 - k}
        return {'hips': 1.0}

    verts, uvs, weights = [], [], []
    prev = None
    for r in range(rows):
        t = r / (rows - 1)
        z = top + (bottom - top) * t
        half = half_top + (half_bottom - half_top) * t
        band = [p.y for p in pts if abs(p.z - z) < 0.06 and abs(p.x) < half]
        y = (max(band) if band else (prev if prev is not None else head['chest'].y)) + 0.025
        if prev is not None and y < prev:
            y = prev
        prev = y
        for c in range(cols):
            u = c / (cols - 1)
            wrap = (abs(u - 0.5) * 2.0) ** 2 * (0.07 * (1.0 - t) + 0.015)
            verts.append(((u - 0.5) * 2.0 * half, y - wrap, z))
            uvs.append((u, 1.0 - t))
            weights.append(weights_at(z))
    faces = []
    for r in range(rows - 1):
        for c in range(cols - 1):
            a = r * cols + c
            faces.append((a, a + 1, a + cols + 1, a + cols))   # the normal out of the back (+Y)
    me = bpy.data.meshes.new('Cape')
    me.from_pydata(verts, [], faces)
    me.update()
    uv = me.uv_layers.new(name='UVMap')
    for poly in me.polygons:
        for li in poly.loop_indices:
            uv.data[li].uv = uvs[me.loops[li].vertex_index]
    mat = bpy.data.materials.new('MI_Cape')
    mat.use_backface_culling = False
    me.materials.append(mat)
    ob = bpy.data.objects.new('Cape', me)
    bpy.context.collection.objects.link(ob)
    ob.parent = arm
    ob.matrix_parent_inverse = arm.matrix_world.inverted()
    ob.modifiers.new('Armature', 'ARMATURE').object = arm
    for name in ['chest', 'spine', 'hips']:
        g = ob.vertex_groups.new(name=name)
        for i, w in enumerate(weights):
            if name in w:
                g.add([i], w[name], 'REPLACE')
    return ob


cape = cape_mesh()

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
# The legs' overlay is the trousers and the boots in one (EQUIPMENT.md 10.8: the feet are
# no place of their own, and the ranger's trousers end where its boots begin).
export('trousers', ranger_legs + boots)
export('cape', [cape])
export('robe', peasant_body + [sleeves])
print('DONE')
