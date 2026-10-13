#!/usr/bin/env python3
"""Pack the textures of a .glb into one base-colour atlas (MODELS.md 3: one texture) and
remap every primitive's UVs into its material's region of it:

    scripts/dev/gltf-atlas.py IN.glb OUT.glb --size 1024 \
        --region MATERIAL=IMAGE.png:x,y,w,h [--region ...] [--tint MATERIAL=r,g,b]

A region is in atlas pixels, (0, 0) the top left (glTF's UV origin). A material is matched
by its name without Blender's `.001` suffix; a material not named is an error; a tint
multiplies a region (the pack's hair is a grey mask for a shader to colour); the image is scaled into its region. Every other material, image, texture and
sampler goes; COLOR_* and TEXCOORD_1.. attributes go (the ingest reads neither). The output
has one material, one texture, the atlas as a PNG. Needs numpy and Pillow.
"""
import argparse, io, json, re, struct, sys
import numpy as np
from PIL import Image

ap = argparse.ArgumentParser()
ap.add_argument('src'); ap.add_argument('dst')
ap.add_argument('--size', type=int, default=1024)
ap.add_argument('--region', action='append', default=[], metavar='MATERIAL=IMAGE:x,y,w,h')
ap.add_argument('--tint', action='append', default=[], metavar='MATERIAL=r,g,b')
args = ap.parse_args()

regions = {}
for r in args.region:
    mat, rest = r.split('=', 1)
    img, box = rest.rsplit(':', 1)
    regions[mat] = (img, [int(x) for x in box.split(',')])

data = open(args.src, 'rb').read()
assert data[:4] == b'glTF', 'not a .glb'
jlen = struct.unpack_from('<I', data, 12)[0]
j = json.loads(data[20:20 + jlen])
off = 20 + jlen
blen = struct.unpack_from('<I', data, off)[0]
bin_ = bytearray(data[off + 8:off + 8 + blen])

CT = {5126: np.float32}
def accessor_floats(i):
    a = j['accessors'][i]
    bv = j['bufferViews'][a['bufferView']]
    assert a['componentType'] == 5126, 'UVs must be float'
    n = {'VEC2': 2}[a['type']]
    start = bv.get('byteOffset', 0) + a.get('byteOffset', 0)
    assert bv.get('byteStride', 8) == 8
    return start, np.frombuffer(bin_, dtype=np.float32, count=a['count'] * n, offset=start).reshape(-1, n).copy()

size = args.size
atlas = Image.new('RGB', (size, size), (128, 128, 128))
tints = {t.split('=')[0]: [float(c) for c in t.split('=')[1].split(',')] for t in args.tint}
for mat, (img, (x, y, w, h)) in regions.items():
    im = Image.open(img).convert('RGB').resize((w, h), Image.LANCZOS)
    if mat in tints:
        im = Image.fromarray((np.asarray(im).astype(np.float32) * np.array(tints[mat])).clip(0, 255).astype(np.uint8))
    atlas.paste(im, (x, y))

done = {}
for m in j['meshes']:
    for p in m['primitives']:
        name = j['materials'][p['material']]['name'] if 'material' in p else None
        name = re.sub(r'\.\d{3}$', '', name or '')
        if name not in regions:
            sys.exit(f'primitive of mesh {m.get("name")} has material {name!r}, which has no region')
        x, y, w, h = regions[name][1]
        acc = p['attributes']['TEXCOORD_0']
        if acc in done:
            assert done[acc] == name
        else:
            start, uv = accessor_floats(acc)
            uv[:, 0] = (x + uv[:, 0] * w) / size
            uv[:, 1] = (y + uv[:, 1] * h) / size
            bin_[start:start + uv.nbytes] = uv.astype(np.float32).tobytes()
            j['accessors'][acc]['min'] = uv.min(0).tolist()
            j['accessors'][acc]['max'] = uv.max(0).tolist()
            done[acc] = name
        p['material'] = 0
        for k in [k for k in p['attributes'] if k.startswith('COLOR_') or (k.startswith('TEXCOORD_') and k != 'TEXCOORD_0')]:
            del p['attributes'][k]

png = io.BytesIO()
atlas.save(png, format='PNG', optimize=True)
png = png.getvalue()
while len(bin_) % 4:
    bin_ += b'\0'
j['bufferViews'].append({'buffer': 0, 'byteOffset': len(bin_), 'byteLength': len(png)})
bin_ += png
while len(bin_) % 4:
    bin_ += b'\0'
j['images'] = [{'bufferView': len(j['bufferViews']) - 1, 'mimeType': 'image/png', 'name': 'atlas'}]
j['samplers'] = [{'magFilter': 9729, 'minFilter': 9987, 'wrapS': 33071, 'wrapT': 33071}]
j['textures'] = [{'sampler': 0, 'source': 0}]
j['materials'] = [{'name': 'atlas', 'pbrMetallicRoughness': {'baseColorTexture': {'index': 0}, 'metallicFactor': 0.0, 'roughnessFactor': 1.0}}]
j['buffers'] = [{'byteLength': len(bin_)}]
js = json.dumps(j, separators=(',', ':')).encode()
while len(js) % 4:
    js += b' '
out = b'glTF' + struct.pack('<II', 2, 12 + 8 + len(js) + 8 + len(bin_))
out += struct.pack('<II', len(js), 0x4E4F534A) + js + struct.pack('<II', len(bin_), 0x004E4942) + bytes(bin_)
open(args.dst, 'wb').write(out)
print(f'{args.dst}: {len(out)} bytes, atlas {size}x{size} ({len(png)} bytes PNG), materials merged: {sorted(set(done.values()))}')
