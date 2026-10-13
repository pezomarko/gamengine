#!/usr/bin/env python3
"""Retarget a .glb for the standard rig (MODELS.md 2) without Blender: rename nodes, unlink
subtrees, and scale the whole model in world space, per axis, keeping the skin consistent.

    scripts/dev/glb-retarget.py IN.glb OUT.glb [--rename OLD=NEW ...] [--drop NODE ...]
                                [--fold PATTERN ...] [--scale S | --scale SX SY SZ] [--keep-anims]

--fold folds the joints whose names match a shell pattern (`index_*`, `ball_leaf_?`), and
every joint under them, into their nearest kept ancestor: the vertices they drove are given
to that ancestor (weights merged), they leave every skin's joint list, and their nodes are
unlinked, so a 68-joint pack rig (fingers, leaf bones) comes under the source budget of 32
(budgets.toml) while the model deforms as before at the standard bones (MODELS.md 2).

glTF axes: X right, Y up, Z forward (the face). A scale moves every node's world translation
(rotations kept), recomputes each inverse bind matrix as the inverse of the joint's new world
matrix, scales skinned vertices in skeleton space and re-expresses rigid meshes under nodes so
their world positions scale; so a non-uniform scale (a body 10% thinner front to back) is
exact. Animations are stripped unless --keep-anims (an upload carries none, MODELS.md 3).
Needs numpy. CONTENT.md 7 and LICENSES.md say which files were made with it.
"""
import argparse, fnmatch, json, struct, sys
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument('src'); ap.add_argument('dst')
ap.add_argument('--rename', nargs='*', default=[], metavar='OLD=NEW')
ap.add_argument('--drop', nargs='*', default=[], metavar='NODE')
ap.add_argument('--fold', nargs='*', default=[], metavar='PATTERN')
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

CTYPES = {5121: ('B', 1), 5123: ('H', 2), 5125: ('I', 4), 5126: ('f', 4)}

def acc(ai):
    a = j['accessors'][ai]; bv = j['bufferViews'][a['bufferView']]
    n = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4, 'MAT4': 16}[a['type']]
    fmt, size = CTYPES[a.get('componentType', 5126)]
    st = bv.get('byteStride', size * n)
    base = bv.get('byteOffset', 0) + a.get('byteOffset', 0)
    return a, base, st, n

def readi(ai):
    a, base, st, n = acc(ai)
    fmt = CTYPES[a['componentType']][0]
    return [struct.unpack_from('<%d%s' % (n, fmt), bin_, base + k * st) for k in range(a['count'])]

def writei(ai, rows):
    a, base, st, n = acc(ai)
    fmt = CTYPES[a['componentType']][0]
    for k, row in enumerate(rows):
        struct.pack_into('<%d%s' % (n, fmt), bin_, base + k * st, *row)

def readv(ai):
    a, base, st, n = acc(ai)
    return np.array([struct.unpack_from('<%df' % n, bin_, base + k * st) for k in range(a['count'])])

def writev(ai, arr, bounds=True):
    a, base, st, n = acc(ai)
    for k, row in enumerate(arr):
        struct.pack_into('<%df' % n, bin_, base + k * st, *row)
    if bounds:
        a['min'] = arr.min(axis=0).tolist(); a['max'] = arr.max(axis=0).tolist()

if args.fold:
    parent = {}
    for i, n in enumerate(nodes):
        for c in n.get('children', []):
            parent[c] = i
    folded = set()
    for i, n in enumerate(nodes):
        if any(fnmatch.fnmatch(n.get('name', ''), pat) for pat in args.fold):
            folded.add(i)
    # Everything under a folded node folds with it.
    changed = True
    while changed:
        changed = False
        for i in range(len(nodes)):
            if i not in folded and i in parent and parent[i] in folded:
                folded.add(i); changed = True
    def keeper(i):
        while i in folded:
            i = parent[i]
        return i
    print('folding %d joints:' % len(folded), sorted(nodes[i].get('name', '?') for i in folded)[:12], '...')
    for sk in j.get('skins', []):
        old_joints = sk['joints']
        new_joints = [jn for jn in old_joints if jn not in folded]
        remap = {}
        for k, jn in enumerate(old_joints):
            remap[k] = new_joints.index(keeper(jn))
        # The inverse bind matrices of the kept joints, packed in place; the count shrinks.
        a, base, st, n = acc(sk['inverseBindMatrices'])
        kept = [struct.unpack_from('<16f', bin_, base + k * st) for k, jn in enumerate(old_joints) if jn not in folded]
        for k, m in enumerate(kept):
            struct.pack_into('<16f', bin_, base + k * st, *m)
        a['count'] = len(kept)
        sk['joints'] = new_joints
        sk['_remap'] = remap
    done = set()
    for i, n in enumerate(nodes):
        if 'mesh' not in n or 'skin' not in n:
            continue
        remap = j['skins'][n['skin']]['_remap']
        for pr in j['meshes'][n['mesh']]['primitives']:
            at = pr['attributes']
            if 'JOINTS_0' not in at or at['JOINTS_0'] in done:
                continue
            done.add(at['JOINTS_0'])
            J = readi(at['JOINTS_0']); Wt = readv(at['WEIGHTS_0'])
            J2, W2 = [], []
            for jr, wr in zip(J, Wt):
                merged = {}
                for jj, ww in zip(jr, wr):
                    if ww > 0:
                        merged[remap[jj]] = merged.get(remap[jj], 0.0) + float(ww)
                top = sorted(merged.items(), key=lambda kv: -kv[1])[:4]
                top += [(0, 0.0)] * (4 - len(top))
                J2.append([t[0] for t in top]); W2.append([t[1] for t in top])
            writei(at['JOINTS_0'], J2)
            writev(at['WEIGHTS_0'], np.array(W2), bounds=False)
    for sk in j.get('skins', []):
        sk.pop('_remap', None)
    for n in nodes:
        if 'children' in n:
            n['children'] = [c for c in n['children'] if c not in folded]
    for sc in j.get('scenes', []):
        sc['nodes'] = [c for c in sc['nodes'] if c not in folded]

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
