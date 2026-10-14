# A prop (CONTENT.md 3.1) from a flat-coloured .obj of Quaternius' CC0 *LowPoly Medieval
# Weapons* pack, in Blender 3.3 headless:
#
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-obj-prop.py -- IN.obj OUT.glb SCALE AXES GRIP_X GRIP_Y GRIP_Z [RATIO]
#
# AXES names the source axes that become glTF's X, Y, Z, each with a sign: `x,-z,y` for a
# shield modelled flat (its face along +Z, its length along +Y) to stand with its face along
# +Y and its length along +Z. GRIP (source units, before the scale) is the point that
# becomes the origin: where the fist closes. RATIO decimates (the pack's quads are two
# triangles each). The materials' colours stay flat; `gm-tools content build` makes the
# swatch atlas.
import sys
import bpy
from mathutils import Matrix, Vector

argv = sys.argv[sys.argv.index('--') + 1:]
src, out, scale, axes = argv[0], argv[1], float(argv[2]), argv[3].split(',')
grip = Vector([float(x) for x in argv[4:7]]) if len(argv) >= 7 else Vector((0, 0, 0))
ratio = float(argv[7]) if len(argv) >= 8 else 1.0
bpy.ops.wm.read_factory_settings(use_empty=True)
# The .obj's coordinates as written: no axis conversion on the way in.
bpy.ops.import_scene.obj(filepath=src, axis_forward='Y', axis_up='Z')
objs = [o for o in bpy.data.objects if o.type == 'MESH']
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
# Source coordinates as in the file (Blender's importer with these axes keeps them as is),
# then: move the grip to the origin, permute the axes, scale.
rows = []
for a in axes:
    sign = -1.0 if a.startswith('-') else 1.0
    i = 'xyz'.index(a.lstrip('+-'))
    row = [0.0, 0.0, 0.0]
    row[i] = sign
    rows.append(row)
perm = Matrix([rows[0] + [0.0], rows[1] + [0.0], rows[2] + [0.0], [0.0, 0.0, 0.0, 1.0]])
o.data.transform(Matrix.Scale(scale, 4) @ perm @ Matrix.Translation(-grip))
o.data.update()
print('TRIS', sum(len(p.vertices) - 2 for p in o.data.polygons))
xs = [v.co for v in o.data.vertices]
print('BOUNDS', [round(min(v[i] for v in xs), 3) for i in range(3)], [round(max(v[i] for v in xs), 3) for i in range(3)])
# glTF export: the exporter turns Blender's Z up into glTF's Y up, so hand it the data as
# Blender would hold it (X, -Z, Y of what we want).
o.data.transform(Matrix([[1, 0, 0, 0], [0, 0, -1, 0], [0, 1, 0, 0], [0, 0, 0, 1]]))
bpy.ops.export_scene.gltf(filepath=out, export_format='GLB', use_selection=True, export_animations=False,
                          export_skins=False, export_apply=True, export_yup=True, export_image_format='NONE')
print('DONE', out)
