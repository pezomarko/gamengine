# Render .glb files together, front and side, to a PNG (a fitting room on paper):
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-preview.py -- OUT.png SIZE FILE.glb [FILE.glb ...]
import sys, math
import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index('--') + 1:]
out, size, files = argv[0], int(argv[1]), argv[2:]
bpy.ops.wm.read_factory_settings(use_empty=True)
for f in files:
    bpy.ops.import_scene.gltf(filepath=f)
meshes = [o for o in bpy.data.objects if o.type == 'MESH']
lo = Vector((1e9,) * 3); hi = Vector((-1e9,) * 3)
for o in meshes:
    for c in o.bound_box:
        w = o.matrix_world @ Vector(c)
        lo = Vector(map(min, lo, w)); hi = Vector(map(max, hi, w))
centre = (lo + hi) / 2
extent = max(hi - lo) * 1.1
scene = bpy.context.scene
# Cycles on the CPU: the workbench and EEVEE want a display, a container has none.
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = 24
scene.cycles.use_denoising = False
scene.view_settings.view_transform = 'Standard'
sun = bpy.data.lights.new('sun', 'SUN'); sun.energy = 3.0
so = bpy.data.objects.new('sun', sun); scene.collection.objects.link(so)
so.rotation_euler = (math.radians(50), math.radians(20), math.radians(-30))
scene.render.resolution_x = size; scene.render.resolution_y = size
scene.render.film_transparent = False
scene.world = bpy.data.worlds.new('w'); scene.world.use_nodes = True
bg = scene.world.node_tree.nodes['Background']; bg.inputs[0].default_value = (0.35, 0.35, 0.4, 1); bg.inputs[1].default_value = 1.0
cam = bpy.data.cameras.new('cam'); cam.type = 'ORTHO'; cam.ortho_scale = extent
co = bpy.data.objects.new('cam', cam); scene.collection.objects.link(co); scene.camera = co
views = {'front': (Vector((0, -10, 0)), (math.pi / 2, 0, 0)), 'side': (Vector((10, 0, 0)), (math.pi / 2, 0, math.pi / 2)),
         'back': (Vector((0, 10, 0)), (math.pi / 2, 0, math.pi))}
base = out[:-4]
for name, (pos, rot) in views.items():
    co.location = centre + pos; co.rotation_euler = rot
    scene.render.filepath = f'{base}_{name}.png'
    bpy.ops.render.render(write_still=True)
print('DONE', base)
