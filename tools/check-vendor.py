#!/usr/bin/env python3
"""vendor 무결성 검증 — U-1(복사본 불변)을 기계로 강제한다.

`crates/vendor/*` 는 nexa-beep에서 **무수정 복사**한 것이고, 확장은 우리 크레이트에서만
한다(docs/13 §5). 규칙을 문서에만 두면 언젠가 깨지므로 **해시로 잠근다**.

사용:
    python tools/check-vendor.py                 검증(CI·커밋 전)
    python tools/check-vendor.py --update <리비전>  upstream 동기화 후 잠금 재생성(U-3)
"""

import datetime
import hashlib
import os
import sys

ROOT = "crates/vendor"
LOCK = os.path.join(ROOT, "VENDOR.lock")

HEADER_TMPL = [
    "# vendor 무결성 잠금 — U-1(복사본 불변) 기계 강제",
    "#",
    "# 출처   : SosomLab/nexa-beep  crates/nbeep-gfx · crates/nbeep-ctl",
    "# 리비전 : {rev}",
    "# 복사일 : {date}",
    "#",
    "# ★ 이 파일이 U-1을 '규칙'에서 '검증'으로 바꾼다.",
    "#   검증: python tools/check-vendor.py",
    "#   의도적 갱신(upstream 동기화)이라면 --update 로 다시 굽고",
    "#   journal에 diff 요약을 남긴다(docs/13 §5 U-3).",
]


def scan():
    """vendor 파일 → sha256 (VENDOR.lock 자신은 제외)."""
    out = {}
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames.sort()
        for fn in sorted(filenames):
            if fn == "VENDOR.lock":
                continue
            path = os.path.join(dirpath, fn)
            rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
            with open(path, "rb") as fh:
                out[rel] = hashlib.sha256(fh.read()).hexdigest()
    return out


def update(rev):
    files = scan()
    lines = [
        line.format(rev=rev, date=datetime.date.today().isoformat())
        for line in HEADER_TMPL
    ]
    lines += ["{}  {}".format(h, rel) for rel, h in sorted(files.items())]
    with open(LOCK, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")
    print("잠금 재생성: {}개 파일 @ {}".format(len(files), rev))
    return 0


def verify():
    if not os.path.exists(LOCK):
        print("ERROR: {} 이 없다 — --update 로 먼저 굽는다".format(LOCK), file=sys.stderr)
        return 2

    expected = {}
    with open(LOCK, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            digest, _, rel = line.partition("  ")
            expected[rel] = digest

    actual = scan()
    changed = sorted(r for r in expected.keys() & actual.keys() if expected[r] != actual[r])
    removed = sorted(expected.keys() - actual.keys())
    added = sorted(actual.keys() - expected.keys())

    if not (changed or removed or added):
        print("OK: vendor {}개 파일 무수정 (U-1 준수)".format(len(actual)))
        return 0

    print("U-1 위반 — vendor 복사본이 변경됐다:", file=sys.stderr)
    for rel in changed:
        print("  변경: " + rel, file=sys.stderr)
    for rel in removed:
        print("  삭제: " + rel, file=sys.stderr)
    for rel in added:
        print("  추가: " + rel, file=sys.stderr)
    print(
        "\n확장은 nclip-ctl 등 우리 크레이트에서 한다(docs/13 §5 U-1).\n"
        "upstream 동기화가 의도라면 --update 로 잠금을 다시 굽고 journal에 요약을 남긴다(U-3).",
        file=sys.stderr,
    )
    return 1


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--update":
        return update(sys.argv[2] if len(sys.argv) > 2 else "unknown")
    return verify()


if __name__ == "__main__":
    raise SystemExit(main())
