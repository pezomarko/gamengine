#!/usr/bin/env python3
"""Join the two bakes of scripts/dev/blender-bake-patch.py into the patch's source PNG
(CONTENT.md 3.3): the colour where the mask says cloth hung over the skin, alpha 255 there
and 0 elsewhere, the colour pulled 4 texels out past the edge (the block's endpoints and
the bilinear filter at the edge see cloth, not the cleared black). Prints the coverage.

    scripts/dev/patch-join.py PREFIX OUT.png     (reads PREFIX_color.png, PREFIX_mask.png)
"""
import sys
import numpy as np
from PIL import Image

prefix, dst = sys.argv[1], sys.argv[2]
color = np.asarray(Image.open(f'{prefix}_color.png').convert('RGBA')).astype(np.float32)
mask = np.asarray(Image.open(f'{prefix}_mask.png').convert('RGBA'))[..., 0] > 127
rgb = color[..., :3].copy()
cov = mask.copy()
for _ in range(4):
    for dy, dx in [(0, 1), (0, -1), (1, 0), (-1, 0)]:
        shifted = np.roll(cov, (dy, dx), (0, 1))
        srgb = np.roll(rgb, (dy, dx), (0, 1))
        fill = shifted & ~cov
        rgb[fill] = srgb[fill]
        cov |= fill
alpha = np.where(mask, 255, 0).astype(np.uint8)
out = np.concatenate([rgb.clip(0, 255).astype(np.uint8), alpha[..., None]], -1)
Image.fromarray(out).save(dst, optimize=True)
h, w = mask.shape
blocks = mask.reshape(h // 4, 4, w // 4, 4).any(axis=(1, 3))
print(f'{dst}: {w}x{h}, coverage {mask.mean():.3f}, blocks touched {int(blocks.sum())} of {blocks.size}')
