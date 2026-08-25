//! `nclip-store` — 영속. **DB 엔진을 링크하지 않는다**([docs/06](../../../docs/06-storage-design.md) · DP-2).
//!
//! ```text
//! data/
//! ├─ index/ seg-NNNNNN.idx (append-only · AEAD) + manifest
//! ├─ blob/  3f/3fa9c1…      (blob_id = H(암호문))
//! └─ keys                   (래핑된 마스터 키)
//! ```
//!
//! 접근 패턴에 join도 트랜잭션 경합도 복잡한 질의도 없다 — 쓰기는 끝에 붙이고, 삭제는 앞에서
//! 자르고, 검색은 훑는다. **DB가 잘하는 문제가 아니라 로그가 잘하는 문제**다.
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]
