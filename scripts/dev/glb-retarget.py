#!/usr/bin/env python3
"""Retarget a .glb for the standard rig (MODELS.md 2) without Blender: rename nodes, unlink
subtrees, and scale the whole model in world space, per axis, keeping the skin consistent.

    scripts/dev/glb-retarget.py IN.glb OUT.glb [--rename OLD=NEW ...] [--drop NODE ...]
                                [--scale S | --scale SX SY SZ] [--keep-anims]

glTF axes: X right, Y up, Z forward (the face). A scale moves every node's world translation
(rotations kept), recomputes each inverse bind matrix as the inverse of the joint's new world
matrix, scales skinned vertices in skeleton space and re-expresses rigid meshes under nodes so
their world positions scale; so a non-uniform scale (a body 10% thinner front to back) is
exact. Animations are stripped unless --keep-anims (an upload carries none, MODELS.md 3).
Needs numpy. CONTENT.md 7 and LICENSES.md say which files were made with it.
"""
import argparse, json, struct, sys
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument('src'); ap.add_argument('dst')
ap.add_argument('--rename', nargs='*', default=[], metavar='OLD=NEW')
ap.add_argument('--drop', nargs='*', default=[], metavar='NODE')
ap.add_argument('--scale', nargs='+', type=float, default=[1.0])
ap.add_argument('--keep-anims', action='store_true')
args = ap.parse_args()
if len(args.scale) == 1:
    args.scale *= 3
if len(args.scale) != 3:
    sys.exit('--scale takes one number or three')

data = open(args.src, 'rb').read()
if data[:4] != b'glTF':
    sys.exit('not a .glb')
jlen = struct.unpack_from('<I', data, 12)[0]
j = json.loads(data[20:20 + jlen])
off = 20 + jlen
blen = struct.unpack_from('<I', data, off)[0]
bin_ = bytearray(data[off + 8:off + 8 + blen])
nodes = j['nodes']

# Renames and drops (a dropped node is unlinked; readers walk the scene, so it is gone).
ren = dict(x.split('=', 1) for x in args.rename)
for n in nodes:
    if n.get('name') in ren:
        n['name'] = ren[n['name']]
byname = {n.get('name'): i for i, n in enumerate(nodes)}
missing = [d for d in args.drop if d not in byname]
if missing:
    print('warning: not in the file:', missing)
drop = {byname[d] for d in args.drop if d in byname}
for n in nodes:
    if 'children' in n:
        n['children'] = [c for c in n['children'] if c not in drop]
for s in j.get('scenes', []):
    s['nodes'] = [c for c in s['nodes'] if c not in drop]
if not args.keep_anims:
    j.pop('animations', None)

def quat_to_mat(q):
    x, y, z, w = q
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])

def mat_to_quat(R):
    t = np.trace(R)
    if t > 0:
        s = np.sqrt(t + 1) * 2; w = 0.25 * s; x = (R[2, 1] - R[1, 2]) / s; y = (R[0, 2] - R[2, 0]) / s; z = (R[1, 0] - R[0, 1]) / s
    elif R[0, 0] > R[1, 1] and R[0, 0] > R[2, 2]:
        s = np.sqrt(1 + R[0, 0] - R[1, 1] - R[2, 2]) * 2; w = (R[2, 1] - R[1, 2]) / s; x = 0.25 * s; y = (R[0, 1] + R[1, 0]) / s; z = (R[0, 2] + R[2, 0]) / s
    elif R[1, 1] > R[2, 2]:
        s = np.sqrt(1 + R[1, 1] - R[0, 0] - R[2, 2]) * 2; w = (R[0, 2] - R[2, 0]) / s; x = (R[0, 1] + R[1, 0]) / s; y = 0.25 * s; z = (R[1, 2] + R[2, 1]) / s
    else:
        s = np.sqrt(1 + R[2, 2] - R[0, 0] - R[1, 1]) * 2; w = (R[1, 0] - R[0, 1]) / s; x = (R[0, 2] + R[2, 0]) / s; y = (R[1, 2] + R[2, 1]) / s; z = 0.25 * s
    q = np.array([x, y, z, w])
    return (q / np.linalg.norm(q)).tolist()

def local(n):
    if 'matrix' in n:
        return np.array(n['matrix']).reshape(4, 4).T
    T = np.eye(4); T[:3, 3] = n.get('translation', [0, 0, 0])
    R = np.eye(4); R[:3, :3] = quat_to_mat(n.get('rotation', [0, 0, 0, 1]))
    Sc = np.diag(list(n.get('scale', [1, 1, 1])) + [1])
    return T @ R @ Sc

def acc(ai):
    a = j['accessors'][ai]; bv = j['bufferViews'][a['bufferView']]
    n = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4, 'MAT4': 16}[a['type']]
    st = bv.get('byteStride', 4 * n)
    base = bv.get('byteOffset', 0) + a.get('byteOffset', 0)
    return a, base, st, n

def readv(ai):
    a, base, st, n = acc(ai)
    return np.array([struct.unpack_from('<%df' % n, bin_, base + k * st) for k in range(a['count'])])

def writev(ai, arr, bounds=True):
    a, base, st, n = acc(ai)
    for k, row in enumerate(arr):
        struct.pack_into('<%df' % n, bin_, base + k * st, *row)
    if bounds:
        a['min'] = arr.min(axis=0).tolist(); a['max'] = arr.max(axis=0).tolist()

if args.scale != [1.0, 1.0, 1.0]:
    S = np.diag(args.scale + [1.0])
    parent = {}
    for i, n in enumerate(nodes):
        for c in n.get('children', []):
            parent[c] = i
    world = {}
    def W(i):
        if i not in world:
            m = local(nodes[i])
            world[i] = (W(parent[i]) @ m) if i in parent else m
        return world[i]
    for i in range(len(nodes)):
        W(i)
    neww = {}
    for i, m in world.items():
        m2 = m.copy(); m2[:3, 3] = (S @ np.append(m[:3, 3], 1))[:3]; neww[i] = m2
    done = set()
    for i, n in enumerate(nodes):
        if 'mesh' not in n:
            continue
        for pr in j['meshes'][n['mesh']]['primitives']:
            ai = pr['attributes']['POSITION']
            if ai in done:
                continue
            done.add(ai)
            P = readv(ai); P4 = np.c_[P, np.ones(len(P))]
            M = S if 'skin' in n else np.linalg.inv(neww[i]) @ S @ world[i]
            writev(ai, (M @ P4.T).T[:, :3])
            if 'NORMAL' in pr['attributes']:
                ni = pr['attributes']['NORMAL']; N = readv(ni)
                Nn = np.linalg.inv(M[:3, :3]).T
                N2 = (Nn @ N.T).T; N2 /= np.linalg.norm(N2, axis=1, keepdims=True)
                writev(ni, N2, bounds=False)
    for sk in j.get('skins', []):
        a, base, st, n = acc(sk['inverseBindMatrices'])
        for k, jn in enumerate(sk['joints']):
            ibm = np.linalg.inv(neww[jn])
            struct.pack_into('<16f', bin_, base + k * st, *ibm.T.flatten())
    for i, n in enumerate(nodes):
        L = (np.linalg.inv(neww[parent[i]]) @ neww[i]) if i in parent else neww[i]
        n.pop('matrix', None)
        sc = np.linalg.norm(L[:3, :3], axis=0); R = L[:3, :3] / sc
        n['translation'] = L[:3, 3].tolist(); n['rotation'] = mat_to_quat(R); n['scale'] = sc.tolist()

js = json.dumps(j, separators=(',', ':')).encode()
js += b' ' * ((4 - len(js) % 4) % 4)
bin_ = bytes(bin_) + b'\0' * ((4 - len(bin_) % 4) % 4)
out = (b'glTF' + struct.pack('<II', 2, 12 + 8 + len(js) + 8 + len(bin_))
       + struct.pack('<I', len(js)) + b'JSON' + js + struct.pack('<I', len(bin_)) + b'BIN\0' + bin_)
open(args.dst, 'wb').write(out)
print('wrote', args.dst, len(out), 'bytes')
