# 072 — multi-angle audit + path forward

*Claude Opus 4.7 · 2026-04-25 · synthesis of three parallel
audits launched per Li directive: (A) next CANON-MISSING crate
scope + skeleton design, (B) body-fill candidates in existing
skeleton, (C) corpus staleness check across reports + golden
doc + AGENTS.md + bd memories. Findings converge on a
three-track path forward that needs only one Li ratification
to unblock substantial work.*

---

## 1 · Summary

The corpus is in good shape. Audit-C found no high-severity
contradictions. Recent ratifications (lojix-store day-one,
client-msg rename, wire-format clarification, rung-by-rung
rule) propagated cleanly. The one structural gap — Slot /
Revision / Blake3Hash newtypes appearing in multiple future
crates with no shared definition — is solvable by a small
shared crate (Audit-A).

The path forward has three tracks that can run in parallel:

- **Track 1 — Corpus cleanup** (no Li input needed). Six
  small fixes; ~10 minutes of edits.
- **Track 2 — Body-fill in existing skeleton** (no Li input
  needed). Eight tightly-bounded `todo!()` bodies become real
  code; adds `uuid`, `rkyv`, optionally `hex` to two
  Cargo.tomls.
- **Track 3 — Create `criome-types` crate** (one Li
  ratification: confirm option (iii) from [reports/071 Q3](repos/mentci-next/reports/071-cli-protocol-and-implementation-order.md)).
  Tiny shared crate (~410 LoC) housing the newtypes that
  `criome-msg`, `criome-schema`, and `sema` all need. Once
  this lands, the next four CANON-MISSING crates can scaffold
  in parallel.

After Track 3, the remaining blockers are the two carried-
forward Li questions (Q-α kind set, Q-β genesis principal),
which unblock `criome-schema` and `criomed` scaffolding.

---

## 2 · Track 1 — corpus cleanup

Audit-C surfaced six items. None are wrong claims that would
mislead future agents; all are stale references or cosmetic
drift.

| # | Severity | Location | Fix |
|---|---|---|---|
| 1 | Med | reports/033 (~10 occurrences) | Replace `semachk` → `machina-chk` (per architecture.md §10 rejection) |
| 2 | Med | reports/033 | Verify `reports/054` reference; fix or drop (054 may be among the deletion sweep) |
| 3 | Med | reports/013 | Verify aski links are descriptive-only, not instructional; otherwise add historical-note framing |
| 4 | Low | workspace-manifest.md line 12 | Bump "Last reviewed: 2026-04-24" to current date |
| 5 | Low | architecture.md §1/§6 | Decide: introduce `machina` in positive prose (per [reports/066 §5.1](repos/mentci-next/reports/066-architecture-md-audit.md)), or leave to §10 only |
| 6 | Low | architecture.md header | Timestamp drift; either drop the timestamp altogether or update on each substantive edit |

Items 1–4 land without further Li input. Items 5–6 are
judgment calls awaiting Li direction.

---

## 3 · Track 2 — body-fill candidates

Audit-B identified eight `todo!()` bodies whose mechanism is
unambiguous given current decisions. These can land as real
code without inviting design churn.

### 3.1 · Tightly-bounded fills (drop-in)

| File | Function | Body sketch | Cargo addition |
|---|---|---|---|
| [`repos/lojix-store/src/hash.rs`](repos/lojix-store/src/hash.rs) | `to_hex` | `self.0.iter().map(\|b\| format!("{:02x}", b)).collect()` | none |
| [`repos/lojix-store/src/hash.rs`](repos/lojix-store/src/hash.rs) | `from_hex` | inline parse, or `hex::decode` if crate added | optional `hex = "0.4"` |
| [`repos/lojix-store/src/layout.rs`](repos/lojix-store/src/layout.rs) | `default_for_user` | `PathBuf::from($HOME).join(".lojix/store")` | none (uses `std::env`) |
| [`repos/lojix-store/src/layout.rs`](repos/lojix-store/src/layout.rs) | `entry_tree` | `self.0.join(hash.to_hex())` | depends on `to_hex` landing |
| [`repos/lojix-store/src/layout.rs`](repos/lojix-store/src/layout.rs) | `exists` | `self.0.exists() && self.index_db_path().exists()` | none |
| [`repos/nexusd/src/client_msg/frame.rs`](repos/nexusd/src/client_msg/frame.rs) | `RequestId::fresh` | `Self(uuid::Uuid::now_v7().as_u128())` | `uuid = { version = "1", features = ["v7"] }` |
| [`repos/nexusd/src/client_msg/frame.rs`](repos/nexusd/src/client_msg/frame.rs) | `Frame::encode` | rkyv `to_bytes` | `rkyv = "0.8"` (with appropriate features) |
| [`repos/nexusd/src/client_msg/frame.rs`](repos/nexusd/src/client_msg/frame.rs) | `Frame::decode` | rkyv `access` (validated) | (same `rkyv` add) |

For `Frame::encode/decode` to actually work, the types in
`client_msg/` need rkyv `Archive`/`Serialize`/`Deserialize`
derives. That's a small follow-up — add the derives, set the
rkyv version + features to match `nexus-schema`'s exactly.

### 3.2 · Why these specifically

These functions are leaf utilities. Their shape is
constrained by external libraries (`uuid::Uuid::now_v7`, rkyv
API) or by trivial path/byte arithmetic. There's no
"interface might shift" risk — the function signatures are
stable; only their bodies change from `todo!()` to working
code.

### 3.3 · Bodies that should stay `todo!()` for now

- Anything that opens redb (lojix-store reader/writer, sema)
  — depends on the storage-backend choice surviving
  cross-crate review.
- Anything that runs the `BundleFromNix` body — depends on
  patchelf / ELF rewriting strategy not yet decided.
- nexusd's main loop (`fn main`) — depends on tokio actor
  shape not yet sketched.
- nexus-cli's main loop — depends on nexusd's exposed API
  shape (path-dep on nexusd-the-library).

---

## 4 · Track 3 — `criome-types` crate

Audit-A recommends creating `criome-types` next, before any
of the other CANON-MISSING crates. Reason: the same newtypes
appear in three downstream crates, and "duplicate per crate"
or "criome-msg owns; others import" both invite issues
(rkyv-derive divergence; layer inversion).

### 4.1 · Layout

```
criome-types/
├── Cargo.toml                    (~25 LoC)
├── flake.nix                     (~30 LoC; mirror lojix-store)
├── AGENTS.md / CLAUDE.md         (shim pattern)
└── src/
    ├── lib.rs                    (~30 LoC; module overview + re-exports)
    ├── slot.rs                   (~60 LoC; Slot newtype + SEED_RANGE const + display)
    ├── revision.rs               (~40 LoC; Revision newtype + monotonic helpers)
    ├── hash.rs                   (~80 LoC; Blake3Hash newtype + hex parse + From<blake3::Hash>)
    ├── literal.rs                (~120 LoC; LiteralValue + PrimitiveType enums)
    ├── op.rs                     (~40 LoC; ChangeOp + Op enums)
    └── error.rs                  (~40 LoC; thiserror)
```

~410 LoC of Rust; passes `cargo check`. No `[[bin]]`. No
features. Default `[lib]`.

### 4.2 · Cargo.toml shape

```toml
[package]
name = "criome-types"
version = "0.1.0"
edition = "2024"
description = "Shared newtypes for criome — Slot, Revision, Blake3Hash, LiteralValue."

[dependencies]
rkyv = { version = "0.8", default-features = false, features = ["std", "bytecheck", "little_endian", "pointer_width_32", "unaligned"] }
serde = { version = "1", features = ["derive"] }
blake3 = "1"
thiserror = "2"

[lints.rust]
unused = "allow"
dead_code = "allow"
```

(rkyv feature flags must match nexus-schema's exactly so
archived types interop.)

### 4.3 · What it unlocks

Once `criome-types` lands, the next four crate creations
become parallel-safe:

- `criome-msg` — types `RawRecord`, `RawValue`, `Request`,
  `Reply`, `Frame`, etc.; uses Slot/Revision/Blake3Hash
  imports from `criome-types`.
- `criome-schema` — record kinds (KindDecl, FieldSpec,
  TypeRef, etc.); uses LiteralValue/PrimitiveType imports.
- `sema` (extends stub) — redb tables keyed by
  Slot/Revision; SemaWrite trait.
- `criomed` (CREATE) — depends on all above.

---

## 5 · Sequencing

```
Track 1 (corpus cleanup):
  ┌──────────────────────────────────┐
  │ #1 fix semachk → machina-chk     │
  │ #2 verify reports/054 reference  │
  │ #3 verify reports/013 aski links │
  │ #4 bump workspace-manifest date  │
  └──────────────────────────────────┘
   parallel; ~10 min total

Track 2 (body-fill):
  ┌──────────────────────────────────┐
  │ A. lojix-store hex/path/exists   │  ~6 LoC bodies
  │ B. nexusd Cargo additions        │  uuid + rkyv
  │ C. nexusd Frame::encode/decode   │  ~5 LoC + derives
  │ D. nexusd RequestId::fresh       │  ~1 LoC
  └──────────────────────────────────┘
   parallel; ~30 min total

Track 3 (criome-types crate):
  Step 0: Li confirms reports/071 Q3 option (iii).
  Step 1: Create /home/li/git/criome-types/ skeleton.
  Step 2: workspace-manifest update, devshell.nix update.
  Step 3: Symlink /home/li/git/mentci-next/repos/criome-types.
  Step 4: cargo check passes.

After Track 3 — parallel scaffolds:
  ├── criome-msg (CREATE; ~700 LoC)
  ├── criome-schema (CREATE; ~500 LoC)  ← needs Q-α answer
  └── sema (extend stub; ~400 LoC)

After all three:
  └── criomed (CREATE; ~600 LoC)         ← needs Q-β answer
```

Tracks 1 + 2 are independent of each other and of Li input —
they can land immediately. Track 3 needs one Li
confirmation. Then Q-α and Q-β unblock the next round.

---

## 6 · Questions for Li

### Q1 · Confirm reports/071 Q3 option (iii) — `criome-types` shared crate

The audit recommends (iii) for cycle-safety and dependency-
graph cleanliness. (i) "duplicate per crate" causes rkyv
archived-type divergence; (ii) "criome-schema owns; others
re-export" inverts the layering (wire-crate depending on
schema-crate). (iii) is a tiny leaf crate that all three
parents depend on. Confirm or override.

### Q2 · `Op` enum scope at `criome-types` v0.0.1

`Op` enum drives `Policy.allowed_ops: Vec<Op>` records. Two
options:

- (a) Define the full set `{Assert, Mutate, Retract, Patch,
  Query, Subscribe, Validate}` now, even though only
  `{Assert, Query, Retract}` are wired at rung 1.
- (b) Define only the rung-1 three and grow later.

Lean: (a). Each variant is a one-line definition; trimming
costs schema-skew at rung 2 when more ops light up.

### Q3 · `hex` crate or inline hex in lojix-store?

Tiny choice: `hex = "0.4"` for ~2 LoC of parsing, or inline
~10 LoC. Inline avoids a dependency; the crate is more
ergonomic. Lean: inline (no new dep for a leaf utility).

### Q4 · Track 1 judgment calls

- Item 5 (introduce `machina` in architecture.md §1
  positively): apply or leave?
- Item 6 (drop or update header timestamp): apply or leave?

### Q5 · Carried over from prior reports

Still pending and still blocking later work:

- Q-α from [reports/067](repos/mentci-next/reports/067-what-to-implement-next.md)
  — confirm or revise the ~15-kind v0.0.1 set. Blocks
  `criome-schema` scaffolding.
- Q-β from [reports/067](repos/mentci-next/reports/067-what-to-implement-next.md)
  — genesis principal mechanism (a) hardcoded or (b)
  first-message-bypass. Blocks `criomed` scaffolding.
- Q4 from [reports/071](repos/mentci-next/reports/071-cli-protocol-and-implementation-order.md)
  — cancel-criomed verb in criome-msg or skip. Blocks final
  shape of `criome-msg::Request` enum.

---

## 7 · One observation

Tracks 1 + 2 are zero-risk maintenance work that lands now.
Track 3 needs one Li nod (Q1 above) and unblocks
~1500–2000 LoC of parallel crate scaffolding work.
Everything beyond that is gated by the two long-standing
blockers (Q-α + Q-β).

The cheapest unblocker is to answer Q1 + Q-α + Q-β + Q4
together — five short answers — and then the
~3000–4000 LoC of skeleton-as-design code can land in three
parallel scaffolds + one final criomed scaffold + one
genesis.nexus authoring step.

---

*End report 072.*
