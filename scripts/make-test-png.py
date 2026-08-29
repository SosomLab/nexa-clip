# 의존 0 — 256x256 PNG 한 장을 표준 라이브러리만으로 만든다(치수 판정 검증용).
import zlib, struct, sys
W = H = 256
raw = b"".join(b"\x00" + bytes([(x * 255) // W, (y * 255) // H, 128] * 1)
                for y in range(H) for x in range(W))
# 위 표현은 행 필터 바이트가 픽셀마다 들어가므로 다시 제대로 만든다.
rows = []
for y in range(H):
    rows.append(b"\x00" + b"".join(bytes([(x * 255) // W, (y * 255) // H, 128]) for x in range(W)))
raw = b"".join(rows)
def chunk(t, d):
    c = t + d
    return struct.pack(">I", len(d)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)
png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 2, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 6))
       + chunk(b"IEND", b""))
open(sys.argv[1], "wb").write(png)
