#!/usr/bin/env bash
# mac-dev-cert.sh — 개발용 코드 서명 신원 1회 생성(09-04 사용자 실기 "재설치마다 손쉬운 사용 권한이 풀림"):
#   macOS TCC(권한 DB)는 앱을 **서명**으로 식별한다. 애드혹 서명은 빌드마다 cdhash가 달라
#   dev-install 때마다 권한이 무효가 된다(토글은 켜져 보이지만 AXIsProcessTrusted = false).
#   자체 서명 인증서 하나로 서명하면 신원이 고정돼 권한이 유지된다(공증과 무관 · 배포에는 안 쓴다).
#
# 사용:  scripts/mac-dev-cert.sh            # 로그인 키체인에 "Nexa Clip Dev" 생성(이미 있으면 no-op)
#        → 이후 scripts/dev-install-mac.sh 가 자동으로 이 신원으로 서명한다.
# 주의:  신뢰 등록 단계에서 macOS가 **로그인 암호를 한 번 묻는다**(GUI 대화창).
#        신원을 바꾼 첫 설치 뒤 손쉬운 사용 토글을 한 번 껐다 켜야 한다(그 다음부터 유지).

set -euo pipefail
NAME="${NEXA_SIGN_IDENTITY:-Nexa Clip Dev}"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning 2>/dev/null | grep -q "$NAME"; then
    echo "이미 있음: $NAME"
    exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/cfg" <<CFG
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
basicConstraints = critical, CA:false
CFG
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -keyout "$tmp/key.pem" -out "$tmp/cert.pem" -config "$tmp/cfg" 2>/dev/null
# ★ macOS `security import`는 OpenSSL 3 기본 PKCS12(AES-256 · SHA-256 MAC)를 못 읽는다
#   ("MAC verification failed" — 실측 09-04). 구형 알고리즘(3DES · SHA1)을 명시하면
#   OpenSSL 3·LibreSSL(/usr/bin/openssl) 어느 쪽이 잡혀도 임포트된다.
openssl pkcs12 -export -inkey "$tmp/key.pem" -in "$tmp/cert.pem" \
    -out "$tmp/id.p12" -passout pass:nexa-dev \
    -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1
# -T /usr/bin/codesign: codesign이 키를 쓸 때 매번 "허용" 대화창이 뜨지 않게 미리 허가.
security import "$tmp/id.p12" -k "$KEYCHAIN" -P nexa-dev -T /usr/bin/codesign >/dev/null
# 코드 서명 용도로 신뢰 — 여기서 로그인 암호 대화창이 한 번 뜬다.
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$tmp/cert.pem"
echo "생성 완료: $NAME — 이제 dev-install-mac.sh 가 이 신원으로 서명합니다"
security find-identity -v -p codesigning | grep "$NAME"
