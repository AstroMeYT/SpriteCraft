#!/usr/bin/env python3
"""Regenerates assets/spritecraft.png and assets/spritecraft.ico from the logo definition."""
import struct, zlib, os

SQ = [(12,12,0xef7d57),(20,20,0xffcd75),(28,28,0xa7f070),(36,36,0x41a6f6),(44,44,0xb13e53),(28,12,0x73eff7),(12,44,0x5d275d),(44,12,0x3b5dc9)]
BG = (35,35,38)

def sample(u, v):
    r = 12.0
    cx = min(max(u, r), 64 - r); cy = min(max(v, r), 64 - r)
    if (u-cx)**2 + (v-cy)**2 > r*r:
        return None
    for x, y, c in SQ:
        if x <= u < x+8 and y <= v < y+8:
            return ((c>>16)&255, (c>>8)&255, c&255)
    return BG

def render(n, ss=4):
    rows = []
    for y in range(n):
        row = bytearray()
        for x in range(n):
            a = r = g = b = 0
            for sy in range(ss):
                for sx in range(ss):
                    s = sample((x+(sx+.5)/ss)/n*64, (y+(sy+.5)/ss)/n*64)
                    if s:
                        a += 1; r += s[0]; g += s[1]; b += s[2]
            t = ss*ss
            if a == 0:
                row += bytes((0,0,0,0))
            else:
                row += bytes((r//a, g//a, b//a, a*255//t))
        rows.append(bytes(row))
    return rows

def png(n):
    rows = render(n)
    raw = b''.join(b'\x00' + r for r in rows)
    def chunk(t, d):
        c = struct.pack('>I', len(d)) + t + d
        return c + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', n, n, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b'')

os.makedirs('assets', exist_ok=True)
open('assets/spritecraft.png', 'wb').write(png(512))
sizes = [16, 32, 48, 64, 256]
imgs = [png(s) for s in sizes]
ico = struct.pack('<HHH', 0, 1, len(sizes))
off = 6 + 16*len(sizes)
for s, d in zip(sizes, imgs):
    ico += struct.pack('<BBBBHHII', 0 if s == 256 else s, 0 if s == 256 else s, 0, 0, 1, 32, len(d), off)
    off += len(d)
ico += b''.join(imgs)
open('assets/spritecraft.ico', 'wb').write(ico)
print('ok')
