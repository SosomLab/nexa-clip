# 32 · `nexa-beep` 전달문 — nexa-clip에서 확인한 공유 항목

> **이 문서는 beep 쪽에 그대로 건네기 위한 것이다.** 추적용 원장은 [22 전달 원장](22-upstream-beep-liaison.md)이고,
> 여기는 **beep 담당자가 읽고 바로 판단할 수 있게** 항목별로 *무엇 · 왜 · beep 어디 · 무엇을 정해야 하나*를 자립적으로 적는다.
>
> - 작성: 2026-09-05 · nexa-clip(SosomLab) → nexa-beep
> - 대조한 beep 판: **v0.2.13** · 커밋 `5537f5c` (로컬 `../nexa-beep`)
> - ⚠️ **beep 저장소는 고치지 않았다.** 다른 프로젝트라 판단·적용은 beep 쪽 몫이다([22 §4](22-upstream-beep-liaison.md)).

---

## 0. 한 장 요약

| # | 항목 | 구분 | beep이 할 일 |
|:--:|---|:--:|---|
| **A-1** | 공용 크레이트 `nexa-conf` — 설정 파일이 **umask 권한**(0644/0664)으로 쓰인다 | 🟡 공유 | 비밀을 담는지 보고 **0600 적용 여부 결정** |
| **A-2** | 공용 크레이트 `nexa-conf` — 미지 키를 무조건 재방출해 **같은 키가 두 줄** 남는다 | 🟡 공유 | 키를 새로 등재할 계획이면 **함께 적용** |
| **B-1** | 랑데부 **도메인 문자열 · Noise prologue** = 앱 식별자 | 🟡 공유 | *"임의로 바꾸지 않는다"* 를 beep 문서에 명기 |
| **B-2** | 동시 `Open`(**glare**) 타이브레이크 규칙이 릴레이에 없다 | 🟡 공유 | 같은 규칙 채택 권함 |
| **B-3** | `nbeep-relay` **결합 방식**(사본 vs 공유 크레이트) | 🟡 공유 | 사본이 갈라지지 않도록 **동기 고지** |
| **C-1** | 공유 URID(한 RID에 기기 N대) | 🔴 반영 | 지금은 불필요 — clip이 기기별 RID로 회피 중 |
| **C-2** | 오프라인 큐(릴레이가 버퍼 역할) | 🔴 반영 | beep도 미구현 — 필요해지면 beep 결정이 선행 |
| **C-3** | 앱/버전 태그를 와이어에 싣기 | 🔴 반영 | 지금은 불필요 |

**A 계열이 이번에 새로 확인된 것**이고, 나머지는 기존 원장 항목의 요약이다.

---

## A. 공용 크레이트 `nexa-conf` — 사본이 양쪽에 있다

`nexa-conf`는 **nexa 계열 공용 설정 크레이트**로, clip과 beep이 **각자 사본**을 갖고 있다
(clip `crates/nexa-conf/` · beep `crates/nexa-conf/`). clip이 2026-09-05 Linux 점검에서 두 결함을 찾아 고쳤고,
**beep 사본에도 같은 코드가 그대로** 있어 알린다.

### A-1. 설정 파일이 umask 권한으로 쓰인다 → 0600 권함

**증상**: `write_atomic`이 temp 파일을 `fs::File::create`로 만든다. 그러면 모드가 **umask 기본**(대개 0644/0664)이 되고,
`rename`이 그 모드를 그대로 최종 파일에 나른다. 설정에 비밀이 실리면 **같은 PC의 다른 계정·백업 도구·동기화 클라이언트에 그대로 노출**된다.

**beep 위치**: `crates/nexa-conf/src/lib.rs:106` `pub fn write_atomic` 안 (`fs::File::create(&tmp)`).

**beep 실측**(2026-09-05 · 이 PC의 `~/.config/nexa-beep/`):

| 파일 | 권한 |
|---|:--:|
| `settings.cfg` | **664** |
| `profile.sec` | **664** |
| `server.pin` | **664** |
| `trust.seg` | **664** |
| `keys.seg` | **664** |
| `identity.key` | 600 (이미 안전 — 다른 경로로 쓰인다) |

> `identity.key`만 0600이다. 이름으로 보아 `profile.sec`·`server.pin`·`trust.seg`·`keys.seg`가 민감해 보이는데,
> 그 파일들이 `write_atomic`을 타는지는 **beep 쪽에서 확인**해야 한다.

**clip에서 왜 문제였나**: clip은 `settings.cfg`에 **페어링 패스프레이즈를 평문으로** 담는다. 실측 664였다.

**clip이 적용한 수정**(그대로 이식 가능):

```rust
// crates/nexa-conf/src/lib.rs — write_atomic 안, temp 생성 부분
let write = (|| -> io::Result<()> {
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    // ★ 소유자만 — 설정에는 비밀이 실릴 수 있다. umask 기본(0644/0664)은
    //   같은 PC의 다른 계정·백업 도구에 그대로 노출된다. rename이 temp의 모드를 나른다.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(contents.as_bytes())?;
    f.sync_all()
})();
```

회귀 테스트도 함께 넣었다:

```rust
/// 설정 파일은 소유자만 읽는다(비밀이 실릴 수 있다).
#[cfg(unix)]
#[test]
fn written_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;
    let d = tmpdir("perm");
    let p = d.join("settings.cfg");
    write_atomic(&p, "_schema=1\n").unwrap();
    let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "mode {mode:o}");
    let _ = fs::remove_dir_all(d);
}
```

**적용 범위 주의**: `#[cfg(unix)]`라 **macOS도 함께 바뀐다**(종전 644 → 600). Windows는 사용자 프로필 ACL이 같은 역할을 하므로 변경 없음.
**기존 파일은 바뀌지 않는다** — 다음 저장 때부터 0600이다. 이미 만들어진 파일까지 죄려면 별도 1회 `set_permissions`가 필요하다(clip은 넣지 않았다).

**beep이 정할 것**: 위 파일들이 비밀을 담는가? 담으면 이식(권함). 담지 않아도 손해는 없다(0600이 기능을 막지 않는다).

### A-2. 미지 키를 무조건 재방출해 같은 키가 두 줄 남는다

**증상**: `serialize`가 `known`을 쓴 뒤 `unknown`을 **조건 없이** 이어 쓴다.
어떤 키가 **나중에 등재되면**(구버전이 미지 키로 보존해 둔 것 → 신버전이 known으로 씀) 한 파일에 **같은 키가 두 줄** 남는다.
파서가 마지막 줄을 채택하므로 값 자체는 무해하지만, **파일이 계속 자라고** 진단할 때 사람을 헷갈리게 한다.

**beep 위치**: `crates/nexa-conf/src/lib.rs:71` `pub fn serialize` 끝부분 (`for (k, v) in unknown { line(k, v); }`).

**clip에서 어떻게 드러났나**: 설정 창 위치 키 5개(`ui.set_x/y/w/h/mon`)를 뒤늦게 등재했더니, 실제 사용자 설정 파일에서
**같은 키가 두 줄씩** 나왔다(실측 5키 중복 · 재현 후 수정). 재시작 때 복원이 안 되던 별개 버그와 함께 잡았다.

**clip이 적용한 수정**:

```rust
for (k, v) in known {
    line(k, v);
}
// ★ 미지 키가 나중에 아는 키가 되면(등재·런타임 set) known이 이긴다 — 같은 키 두 줄 금지.
for (k, v) in unknown {
    if known.iter().any(|(kk, _)| *kk == k.as_str()) {
        continue;
    }
    line(k, v);
}
```

```rust
/// 미지 키로 보존된 것이 나중에 known으로 오면 한 줄만(known 값) 남는다.
#[test]
fn known_wins_over_stale_unknown() {
    let text = serialize(
        &[("a", "new")],
        &[("a".to_string(), "old".to_string()), ("z".to_string(), "keep".to_string())],
    );
    assert_eq!(text, "_schema=1\na=new\nz=keep\n");
}
```

**beep이 정할 것**: 앞으로 설정 키를 새로 등재할 계획이 있으면 함께 적용을 권한다(없으면 증상이 안 난다).

---

## B. 어긋나면 조용히 깨지는 공유 규약

> 서버 코드가 바뀌지 않아 **아무도 신경 쓰지 않는데, 한쪽이 값을 바꾸면 조용히 못 만나게 되는** 부류다.

### B-1. 랑데부 도메인 문자열 · Noise prologue = 앱 식별자

clip은 RID 파생 도메인을 `"nclip-rid-v1"`, 종단 prologue를 `"nexa-clip/1"`로 쓴다(2026-09-03 구현 · 서버 세션엔 미적용).
beep은 `"nbeep-rid-v1"`을 유지한다. 이 분리가 **beep과 clip이 서로를 못 보게** 하는 유일한 장치다.

**beep에 부탁**: *"이 문자열은 앱 식별자다 — 임의로 바꾸지 않는다"* 를 beep 쪽 결정 기록/ADR에 남겨 달라.
beep이 모른 채 바꾸면 **격리가 깨지거나 충돌**한다.

근거: clip [07 §3-4](07-device-rendezvous.md) · [DR-23](10-decision-record.md).

### B-2. 동시 `Open`(glare) 타이브레이크

양쪽이 동시에 `Open`을 걸면 누가 initiator인지 정하는 규칙이 필요하다.
**실코드 확인 범위에서 beep 릴레이에는 규칙이 없다** — beep도 같은 문제를 갖는다.

clip이 채택한 규칙: **`PeerId` 바이트 사전순으로 작은 쪽이 initiator**(추가 왕복 0).
같은 규칙을 쓰면 서로 붙을 때도 안전하다.

근거: clip [07 §4-3](07-device-rendezvous.md) · D-15.

### B-3. `nbeep-relay` 결합 방식 — 사본이 갈라지면 통신이 깨진다

UI 계층(`nbeep-gfx`·`nbeep-ctl`)은 clip이 **포크로 흡수**했다([DR-17](10-decision-record.md)) — 갈라져도 화면만 다를 뿐이다.
**릴레이는 다르다. 와이어라서 사본이 갈라지는 순간 통신이 깨진다.**

clip은 CI 단독 빌드 때문에 path 의존을 못 써 **사본을 채택**했고(2026-09-03), `nclip-sync/*` 머리말에
*"와이어 규약 공유 · beep과 동기 필수"* 를 명기했다.

**beep에 부탁**: beep 쪽 `nbeep-relay`에도 같은 고지를 남겨 달라 — *"이 크레이트는 clip과 와이어를 공유한다.
메시지·상수·포트를 바꾸면 clip에 알린다."* 장기적으로는 **공유 크레이트 승격**([DR-18](10-decision-record.md)) 여부를 함께 정하는 것이 낫다.

---

## C. 지금은 불필요 — 서버 변경이 필요해지면

> 셋 다 **서버(`nexa-beepd`) 변경**이 필요한 항목이고, clip은 현재 우회하거나 쓰지 않는다. 정보 공유용이다.

| # | 항목 | 지금 상태 |
|:--:|---|---|
| **C-1** | **공유 URID** — 한 RID에 기기 N대 등록 | `nexa-beepd`의 `rids: HashMap<Rid, ConnId>`가 **1:1**이라 뒤 등록이 앞을 덮는다. 채택하려면 `HashMap<Rid, Vec<ConnId>>` + `Open` 팬아웃 필요. **clip은 기기별 RID로 회피** → 서버 변경 없음 |
| **C-2** | **오프라인 큐** | 릴레이는 *"버퍼가 아니라 파이프"* 라 **양쪽 동시 접속일 때만** 흐른다. 꺼진 기기가 나중에 받으려면 컨텐츠 서버 모드가 필요한데 **beep에서도 미구현**이다. clip이 먼저 필요해지면 **beep의 결정이 선행**돼야 한다 |
| **C-3** | **앱/버전 태그를 와이어에** | 서버가 앱별 통계·상한을 갖게 하려면 `Register`에 태그가 필요하다. 지금은 불필요 |

---

## D. 회신이 필요한 것

1. **A-1 / A-2** — `nexa-conf` 사본에 이식할지 여부(권함). 이식하면 clip과 사본이 다시 맞춰진다.
2. **B-1 / B-2 / B-3** — beep 문서에 규약 고지를 남겨 줄 수 있는지.
3. C 계열은 회신 불필요(정보 공유).

회신 결과는 clip 쪽 [22 원장](22-upstream-beep-liaison.md) 상태를 **[전달] → [반영]**으로 바꾸고 beep 커밋 해시를 적는다.
