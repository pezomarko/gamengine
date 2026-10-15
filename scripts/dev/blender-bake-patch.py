# Bake a cloth piece onto the body's own atlas (CONTENT.md 3.3, LOOK.md 6.6: a *patch*), in
# Blender 3.3 headless:
#
#   docker run --rm -v "$PWD:/work" -w /work nytimes/blender:3.3.1-cpu-ubuntu18.04 blender -b \
#       --python scripts/dev/blender-bake-patch.py -- BODY_atlas.glb OUT_PREFIX SIZE \
#       PIECE.glb=TEXTURE.png [PIECE.glb=TEXTURE.png ...]
#
# BODY_atlas.glb is the official avatar as the repo has it (its UVs already remapped into
# its one atlas by gltf-atlas.py), the pieces are the overlay cuts of blender-quaternius-hero.py
# with their pack texture named beside them (they are exported without images). Both stand
# in the same rest pose on the same skeleton, so a ray from the body's surface outward meets
# the cloth that hangs over that bit of skin. Cycles bakes "selected to active": the pieces
# are selected, each body mesh in turn is active; the colour the ray finds lands in the body's
# texel. Two passes: the colour (a diffuse bake, colour only, no light), and the coverage (the
# pieces' emission set to white, an emit bake): where no cloth is over the skin the ray finds
# nothing and the texel stays as the image was cleared. Writes OUT_PREFIX_color.png and
# OUT_PREFIX_mask.png; scripts/dev/places-pipeline.sh joins them into the patch's RGBA.
import os
import sys
import bpy

argv = sys.argv[sys.argv.index('--') + 1:]
body_glb, out, size = argv[0], argv[1], int(argv[2])
pieces = [a.split('=', 1) for a in argv[3:]]
# Rays start this far outside the skin and go in (the cloth is 12 mm out, a belt 3 cm); a
# ray that meets nothing within the distance writes nothing.
CAGE = 0.06
REACH = 0.14
# Texels past an island's edge take the nearest baked texel's: the bilinear filter at the
# edge of a body island sees cloth, not the skin's edge texel.
MARGIN = 4

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=body_glb)
body_objs = [o for o in bpy.data.objects if o.type == 'MESH']
body_mats = {s.material for o in body_objs for s in o.material_slots if s.material}
print('BODY', [o.name for o in body_objs], 'materials', [m.name for m in body_mats])

piece_objs = []
for glb, texture in pieces:
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=glb)
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == 'MESH']
    for o in new:
        if o.type != 'MESH':
            bpy.data.objects.remove(o)
    img = bpy.data.images.load(os.path.abspath(texture))
    print('TEXTURE', texture, img.size[:], 'readable', img.has_data)
    for o in meshes:
        for s in o.material_slots:
            m = s.material
            m.use_nodes = True
            nodes, links = m.node_tree.nodes, m.node_tree.links
            bsdf = next(n for n in nodes if n.type == 'BSDF_PRINCIPLED')
            tex = nodes.new('ShaderNodeTexImage')
            tex.image = img
            links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
            # glTF's metallic defaults to 1, and a metal has no diffuse colour to bake.
            bsdf.inputs['Metallic'].default_value = 0.0
            bsdf.inputs['Specular'].default_value = 0.0
            bsdf.inputs['Roughness'].default_value = 1.0
            bsdf.inputs['Emission'].default_value = (1.0, 1.0, 1.0, 1.0)
            bsdf.inputs['Emission Strength'].default_value = 1.0
    piece_objs += meshes
    print('PIECE', glb, [o.name for o in meshes])

scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = 4
scene.cycles.use_denoising = False


def bake(kind, name):
    img = bpy.data.images.new(name, size, size, alpha=True)
    img.generated_color = (0.0, 0.0, 0.0, 0.0)
    # The bake lands in the active image node of the active object's material.
    targets = []
    for m in body_mats:
        m.use_nodes = True
        node = m.node_tree.nodes.new('ShaderNodeTexImage')
        node.image = img
        m.node_tree.nodes.active = node
        targets.append((m, node))
    for body in body_objs:
        bpy.ops.object.select_all(action='DESELECT')
        for o in piece_objs:
            o.select_set(True)
        body.select_set(True)
        bpy.context.view_layer.objects.active = body
        if kind == 'DIFFUSE':
            bpy.ops.object.bake(type='DIFFUSE', pass_filter={'COLOR'}, use_selected_to_active=True,
                                cage_extrusion=CAGE, max_ray_distance=REACH, margin=MARGIN, use_clear=False)
        else:
            bpy.ops.object.bake(type='EMIT', use_selected_to_active=True,
                                cage_extrusion=CAGE, max_ray_distance=REACH, margin=MARGIN, use_clear=False)
        print('BAKED', kind, body.name)
    img.filepath_raw = f'{out}_{name}.png'
    img.file_format = 'PNG'
    img.save()
    for m, node in targets:
        m.node_tree.nodes.remove(node)


bake('DIFFUSE', 'color')
bake('EMIT', 'mask')
print('DONE')
