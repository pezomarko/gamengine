# A rigid armour overlay (CONTENT.md 3.3, EQUIPMENT.md 5.2) from a flat-coloured .obj of
# Quaternius' CC0 *LowPoly Animated Knight* pack (a helmet, the shoulder plates), bound
# whole to one bone of the striker's rig, in Blender 3.3 headless:
#
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-rigid-overlay.py -- RIG.glb IN.obj OUT.glb BONE WIDTH DX DY DZ [RATIO]
#
# RIG.glb is the official avatar the piece is cut for (models/avatars/striker_mail.glb):
# its armature is kept, its meshes are measured and dropped. The piece is scaled so that
# it is WIDTH times as wide (along X) as the part of the body the BONE moves (the
# vertices it weighs more than half), and centred on that part, moved DX DY DZ metres
# (Blender's axes: Y back, Z up); RATIO decimates. Every vertex is weighed 1.0 to BONE, so
# the piece rides the bone as a helmet rides a head. The .obj is Y up, facing +Z, as the
# pack writes it, which Blender's importer turns to Z up facing -Y, the rig's way. An
# overlay wants a texture (a prop is given swatches by `gm-tools content build`, an
# overlay is not), so the materials' flat colours are painted into one 64² swatch image,
# each face's UVs put at its colour's cell, and the image goes into the .glb.
import sys
import bpy
import bmesh
from mathutils import Vector

argv = sys.argv[sys.argv.index('--') + 1:]
rig, src, out, bone = argv[0], argv[1], argv[2], argv[3]
width = float(argv[4])
off = Vector([float(x) for x in argv[5:8]])
ratio = float(argv[8]) if len(argv) >= 9 else 1.0

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=rig)
arm = next(o for o in bpy.data.objects if o.type == 'ARMATURE')
lo, hi = Vector((1e9,) * 3), Vector((-1e9,) * 3)
for o in [o for o in bpy.data.objects if o.type == 'MESH']:
    names = {g.index: g.name for g in o.vertex_groups}
    for v in o.data.vertices:
        if any(names.get(g.group) == bone and g.weight > 0.5 for g in v.groups):
            w = o.matrix_world @ v.co
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
    bpy.data.objects.remove(o)
print('BONEBOX', bone, [round(x, 3) for x in lo], [round(x, 3) for x in hi])
centre = (lo + hi) / 2

before = set(bpy.data.objects)
bpy.ops.import_scene.obj(filepath=src)
objs = [o for o in bpy.data.objects if o not in before and o.type == 'MESH']
bpy.ops.object.select_all(action='DESELECT')
for o in objs:
    o.select_set(True)
bpy.context.view_layer.objects.active = objs[0]
if len(objs) > 1:
    bpy.ops.object.join()
o = bpy.context.view_layer.objects.active
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
if ratio < 1.0:
    m = o.modifiers.new('dec', 'DECIMATE')
    m.ratio = ratio
    bpy.ops.object.modifier_apply(modifier='dec')
xs = [v.co.copy() for v in o.data.vertices]
plo = Vector([min(v[i] for v in xs) for i in range(3)])
phi = Vector([max(v[i] for v in xs) for i in range(3)])
scale = width * (hi.x - lo.x) / (phi.x - plo.x)
move = centre + off - (plo + phi) / 2 * scale
for v in o.data.vertices:
    v.co = v.co * scale + move
o.data.update()
xs = [v.co for v in o.data.vertices]
print('SCALE', round(scale, 4))
print('BOUNDS', [round(min(v[i] for v in xs), 3) for i in range(3)], [round(max(v[i] for v in xs), 3) for i in range(3)])
print('TRIS', sum(len(p.vertices) - 2 for p in o.data.polygons))

# The swatch atlas: one 16² cell a material, four a row.
cols = []
for slot in o.material_slots:
    m = slot.material
    c = (0.5, 0.5, 0.5)
    if m is not None:
        c = tuple(m.diffuse_color[:3])
        if m.use_nodes:
            bsdf = next((n for n in m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED'), None)
            if bsdf is not None:
                c = tuple(bsdf.inputs['Base Color'].default_value[:3])
    cols.append(c)
cols = cols or [(0.5, 0.5, 0.5)]
img = bpy.data.images.new('swatch', 64, 64, alpha=False)
px = [0.0] * (64 * 64 * 4)
for i, c in enumerate(cols[:16]):
    cx, cy = (i % 4) * 16, (i // 4) * 16
    for y in range(cy, cy + 16):
        for x in range(cx, cx + 16):
            at = (y * 64 + x) * 4
            px[at:at + 4] = [c[0], c[1], c[2], 1.0]
img.pixels = px
img.pack()
index = [p.material_index for p in o.data.polygons]
for uv in list(o.data.uv_layers):
    o.data.uv_layers.remove(uv)
uv = o.data.uv_layers.new(name='UVMap')
for p in o.data.polygons:
    i = min(index[p.index], 15)
    u, v = ((i % 4) * 16 + 8) / 64, ((i // 4) * 16 + 8) / 64
    for li in p.loop_indices:
        uv.data[li].uv = (u, v)
mat = bpy.data.materials.new('atlas')
mat.use_nodes = True
nodes = mat.node_tree.nodes
tex = nodes.new('ShaderNodeTexImage')
tex.image = img
bsdf = next(n for n in nodes if n.type == 'BSDF_PRINCIPLED')
mat.node_tree.links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
o.data.materials.clear()
o.data.materials.append(mat)
print('SWATCHES', len(cols))

g = o.vertex_groups.new(name=bone)
g.add(list(range(len(o.data.vertices))), 1.0, 'REPLACE')
mod = o.modifiers.new('arm', 'ARMATURE')
mod.object = arm
o.parent = arm
o.matrix_parent_inverse = arm.matrix_world.inverted()

bpy.ops.object.select_all(action='DESELECT')
arm.select_set(True)
o.select_set(True)
bpy.ops.export_scene.gltf(filepath=out, export_format='GLB', use_selection=True, export_animations=False,
                          export_skins=True, export_apply=True, export_yup=True, export_def_bones=False,
                          export_image_format='AUTO', export_colors=False)
print('DONE', out)
