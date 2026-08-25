import zlib, struct, sys

W = 1024
OUT = int(sys.argv[1]) if len(sys.argv) > 1 else 256
NAME = sys.argv[2] if len(sys.argv) > 2 else 'out.png'
SS = W // OUT

def rr(x, y, rx, ry, w, h, r):
    if x < rx or y < ry or x > rx + w or y > ry + h: return False
    cx = min(max(x, rx + r), rx + w - r); cy = min(max(y, ry + r), ry + h - r)
    dx, dy = x - cx, y - cy
    return dx*dx + dy*dy <= r*r

def hexc(s):
    s = s.lstrip('#'); return (int(s[0:2],16), int(s[2:4],16), int(s[4:6],16))
def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i]-a[i])*t)) for i in range(3))

BG0, BG1 = hexc('#22C3D6'), hexc('#0B7FA6')
BD0, BD1 = hexc('#FFFFFF'), hexc('#E6F8FF')
ACC, HOLE = hexc('#0B7FA6'), hexc('#E6F8FF')
solid = lambda c: (lambda x, y: c)

shapes = [
    (392, 284, 432, 492, 48, solid((255,255,255)), 0.38),
    (344, 266, 432, 528, 52, solid((255,255,255)), 0.68),
    (296, 248, 432, 560, 56, lambda x, y: lerp(BD0, BD1, (y-248)/560), 1.0),
    (424, 192, 176, 112, 36, solid(ACC), 1.0),
    (470, 150,  84,  84, 30, solid(ACC), 1.0),
    (494, 176,  36,  36, 18, solid(HOLE), 1.0),
    (364, 424, 296,  44, 22, solid(ACC), 1.0),
    (364, 522, 224,  44, 22, solid(ACC), 1.0),
    (364, 620, 272,  44, 22, solid(ACC), 1.0),
]

buf = bytearray(W*W*4)
for py in range(W):
    yy = py + 0.5; row = py*W*4
    for px in range(W):
        xx = px + 0.5
        if not rr(xx, yy, 0, 0, 1024, 1024, 232): continue
        r, g, b = lerp(BG0, BG1, yy/1024.0)
        for (sx, sy, sw, sh, sr, cf, a) in shapes:
            if rr(xx, yy, sx, sy, sw, sh, sr):
                cr, cg, cb = cf(xx, yy)
                r = int(round(r+(cr-r)*a)); g = int(round(g+(cg-g)*a)); b = int(round(b+(cb-b)*a))
        o = row+px*4; buf[o]=r; buf[o+1]=g; buf[o+2]=b; buf[o+3]=255

out = bytearray()
for oy in range(OUT):
    out += b'\x00'
    for ox in range(OUT):
        rs=gs=bs=as_=0
        for dy in range(SS):
            base = ((oy*SS+dy)*W + ox*SS)*4
            for dx in range(SS):
                o = base+dx*4; al = buf[o+3]
                rs += buf[o]*al; gs += buf[o+1]*al; bs += buf[o+2]*al; as_ += al
        n = SS*SS
        out += bytes((0,0,0,0)) if as_ == 0 else bytes((rs//as_, gs//as_, bs//as_, as_//n))

def ck(t, d):
    return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t+d) & 0xffffffff)
png = (b'\x89PNG\r\n\x1a\n' + ck(b'IHDR', struct.pack('>IIBBBBB', OUT, OUT, 8, 6, 0, 0, 0))
       + ck(b'IDAT', zlib.compress(bytes(out), 9)) + ck(b'IEND', b''))
open(NAME, 'wb').write(png)
print('ok', NAME, OUT)
