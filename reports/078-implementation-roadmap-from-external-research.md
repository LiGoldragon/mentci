---
title: 078 — implementation roadmap from external research (multi-angle review)
date: 2026-04-25
anchor: Li 2026-04-25 "research how to implement our design from what has already been done and has a similar function, within the more-correct constraints of the criome project"
feeds: criome/ARCHITECTURE.md (canonical); reports/070 (language); reports/074 (rkyv); reports/076 (forward agenda); reports/077 (signal naming)
status: actionable plan; supersedes 076 §4 Tier-1/Tier-2 once landed
---

# 078 — implementation roadmap (multi-angle research)

Four parallel research agents studied existing systems with similar function and recommended how to implement criome's design within its stricter constraints. This report synthesises the findings into an ordered roadmap from where we are today (skeleton-as-design across all canonical repos) to **Stage A** — `nexus-cli '(| KindDecl :name "KindDecl" |)'` returning the bound seed record.

## 1. Research findings (one paragraph each)

### 1.1 Validator pipeline + datalog

Datomic's transactor matches criome's six-step ordering exactly (schema → refs → invariants → permissions → write → cascade). RDFox and Postgres MVCC commit pipelines confirm the pattern. For the cascade engine specifically, **`datafrog` 0.4** is the closest Rust crate match (semi-naive fixpoint, stratified, deterministic, no-async); but for v0.0.1 a hand-rolled stratified fixpoint over redb iterators (~200-300 lines) is clearer and avoids the API-learning tax. **Crepe** is rejected because its `#[datalog]` macros violate the project's no-macros rule. **Differential Dataflow** is overkill for v0.0.1 but a strong candidate when streaming cascades become hot. Pattern resolution (`RawPattern → PatternExpr`) is a static check at assertion-time; runtime unification only emits diagnostics on truly-dynamic problems.

### 1.2 Content-addressed store

Nix-store's NAR canonical-encoding is heavier than we need. **Sorted-tree blake3** suffices: walk depth-first, sort entries by path, normalise timestamps, hash the `(path, mode, size, content_hash)` tuple stream. **Patchelf** for RPATH rewrite (shell out via `std::process::Command`; do *not* use the `goblin` crate — it parses but doesn't rewrite). Heterogeneous closures (non-ELF entries) handled by try-and-warn. **GC is criome-driven, lojix-executed**: criome marks reachability via sema records; lojix polls + sweeps after a grace period (default 7 days). Crates: `walkdir 2.4+`, `tempfile 3.8+`, `redb 4.x` (already in sema), `blake3 1.5+` (already in lojix-store).

### 1.3 Binary protocol over UDS

The framing question (where one frame ends on the byte stream when "the schema is the framing") is resolved by adopting **`tokio_util::codec::LengthDelimitedCodec`** — battle-tested, async-native, isolates framing from business logic. 4-byte big-endian length prefix per frame. Wayland's size-in-header and D-Bus's length-prefix designs validate the choice. Auth: **SO_PEERCRED check at accept time** skipping SASL EXTERNAL text negotiation; UID-based operator allowlist for MVP. Subscription multiplexing already correctly modeled by signal's `subscription_id`. rkyv validation overhead is 5-15% CPU — negligible vs UDS I/O; defer optimization.

### 1.4 End-to-end sequencing

A `(| KindDecl :name "KindDecl" |)` round-trip touches every layer: nexus-cli → client-msg → nexus daemon → signal → criome → sema → reply path back. **The bootstrap blocker**: querying for `KindDecl` requires `KindDecl` to exist as a record in sema, but sema starts empty. Resolution: genesis-via-nexus per architecture.md §10. **Pre-Stage-A milestones M0-M5 are sequenced below.** Total estimate ~9-10 days of focused body-fill work given current skeleton.

## 2. The schema-of-schema question

Four research agents implicitly assumed `criome-schema` exists as a separate crate (per the original [reports/065](065-criome-schema-design.md) design). It does not currently exist — and shouldn't, in MVP. **Recommendation: add the schema-of-schema types to `signal`.**

Per Li 2026-04-25: *"signal is where this goes. nexus depends on signal. anything criome is signal. nexus is just a frontend to it."*

```
signal/src/
├── ... (existing envelope: Frame, Body, Request, Reply, handshake, auth)
└── kind.rs    ← NEW: KindDecl, FieldSpec, TypeRef, VariantDecl, CategoryDecl
```

Why signal, not nexus-schema:

- **Anything criome is signal.** Schema-of-schema records describe records; they are criome data; therefore signal.
- **nexus depends on signal.** Putting kind types in nexus-schema would invert the dependency direction. nexus-schema (and its IR types: `RawPattern`, `RawOp`, etc.) currently live alongside Rust-source records — but per the signal-as-everything-criome rule, those IR types likely also belong in signal. See open question Q-N1 below.
- **No new crate.** signal already exists, just absorbs the new module.

### Q-N1 — RESOLVED 2026-04-25

Li: *"it's true that nexus-schema might now be redundant. we should probably shelf it."*

Action taken: All of `nexus-schema/src/*` moved into `signal/src/*` as additional modules. `nexus-schema` is now SHELVED (per `mentci/docs/workspace-manifest.md`); a deprecation README points migration users at signal. signal cargo check + criome cargo check + lojix-schema cargo check all green after the move.

Migration completed:

| Was | Now |
|---|---|
| `nexus_schema::{domain,module,names,origin,primitive,program,ty}` | `signal::{domain,module,names,origin,primitive,program,ty}` |
| `nexus_schema::{slot,value,pattern,query,edit,diagnostic}` | `signal::{slot,value,pattern,query,edit,diagnostic}` |

signal now holds three concentric layers: wire envelope (Frame, handshake, request/reply), language IR (RawPattern, RawOp, AssertOp, RawRecord, …), and sema record kinds (KindDecl pending M0; Rust-source records absorbed). Anything criome is signal.

## 3. Milestones M0 → M5

Each milestone is independently shippable and testable. Parallel opportunities marked.

### M0 — Schema-of-schema + genesis text (~1-2 days)

**Files**:
- `signal/src/kind.rs` (~80 LoC): `KindDecl`, `FieldSpec`, `TypeRef`, `VariantDecl`, `CategoryDecl` with rkyv canonical derives. Per Li's signal-as-everything-criome rule.
- `criome/genesis.nexus` (~30 lines of `(Assert (KindDecl ...))` etc.): the seed records, in actual nexus syntax (positional record form per `nexus/spec/grammar.md`).
- `criome/src/lib.rs`: `include_str!("../genesis.nexus")` to embed at compile-time.

**Output**: signal cargo check green; criome compiles with embedded genesis text (not yet dispatched).

### M1 — UDS listener + framing (~2 days)

**Parallel-safe across criome and nexus.**

**Files**:
- `criome/src/uds.rs` body fill (~70 LoC): `tokio::net::UnixListener` + `tokio_util::codec::LengthDelimitedCodec` + handshake check + dispatch stub.
- `nexus/src/main.rs` UDS bind + accept loop (~40 LoC): listens for client-msg on operator socket; opens persistent UDS to criome.
- Add to `criome/Cargo.toml`: `tokio = "1.35"`, `tokio-util = "0.7"`, `bytes = "1.5"`, `futures = "0.3"`, `nix = "0.27"` (for `getpeereid`).
- Same deps in `nexus/Cargo.toml`.

**Output**: criome and nexus daemons start, listen, handshake, log each frame received. No business logic yet.

### M2 — sema redb storage (~2 days)

**Parallel-safe with M1.**

**Files**:
- `sema/src/lib.rs` body fill (~100 LoC): open/close redb file; per-kind table schema (`(Slot, seq) → ChangeLogEntry`); `SlotBinding` table; helpers `lookup_slot`, `query_kind`, `next_revision`, `mint_slot`.
- `sema/Cargo.toml` deps: `redb = "4"`, `nexus-schema = { path = "../nexus-schema" }` (or git).

**Output**: sema crate compiles + has integration test that creates a redb file, writes one ChangeLogEntry, reads it back.

### M3 — Validator pipeline body fills (~2-3 days)

**Depends on M0 + M2.**

**Files**:
- `criome/src/validator/schema.rs` body (~80 LoC): for genesis Asserts, validate against `nexus_schema::kind::*` Rust types; for post-genesis, look up `KindDecl` from sema. Emit `E0001` Diagnostic on mismatch.
- `criome/src/validator/refs.rs` body (~40 LoC): walk record fields; for each `RawValue::SlotRef(slot)`, call `sema.lookup_slot(slot)`; emit `E0002` if missing.
- `criome/src/validator/permissions.rs` body (~30 LoC): MVP — accept any request whose `auth_proof` is `SingleOperator` (the SO_PEERCRED check at the UDS layer is the actual security boundary). Emit `E0004` otherwise.
- `criome/src/validator/write.rs` body (~60 LoC): `redb::WriteTransaction`; append ChangeLogEntry to per-kind table; update SlotBinding; commit.
- `criome/src/validator/cascade.rs` body: stays `todo!()` for Stage A (no rules in seed).
- `criome/src/validator/invariants.rs` body: stays `todo!()` for Stage A (no `is_must_hold` rules).

**Output**: `validate_request(request, sema) -> Result<Reply>` works end-to-end for Assert + Query verbs. Stage A's seed records can land via the validator (still without nexus parsing).

### M4 — Nexus text ↔ signal translation (~2 days)

**Parallel with M3.**

**Files**:
- `nexus/src/main.rs` parse + map (~80 LoC): call `nota_serde_core::parse(text, Dialect::Nexus)` → AST; case-match top-level form to build `signal::Request::{Assert, Mutate, Retract, Patch, Query, Subscribe, Validate}`.
- `nexus/src/main.rs` reply path (~60 LoC): inverse — `signal::Reply::*` → nexus text via `nota_serde_core::serialize`.
- Add to `nexus/Cargo.toml`: `nota-serde-core = { path = "../nota-serde-core" }` and `signal = { path = "../signal" }`.

**Output**: nexus daemon reads nexus text on its operator socket, forwards as signal frames to criome over the persistent UDS, returns rendered text.

### M5 — nexus-cli wiring (~1 day)

**Files**:
- `nexus-cli/src/main.rs` (~50 LoC): clap argv parse; build `client_msg::Request::Send { nexus_text }`; encode rkyv frame; connect to nexus daemon UDS; write frame; read reply frame; print reply text.

**Output**: `nexus-cli '(| KindDecl :name "KindDecl" |)'` (or whatever exact pattern syntax the nexus grammar settles on — see `nexus/spec/`) returns the bound seed `KindDecl` record. **Stage A complete.**

## 4. Dependency graph

```
        M0 (kind types + genesis.nexus)
         │
         ├──────────┐
         │          │
         ▼          ▼
        M1         M2
   (UDS listener) (sema redb)
         │          │
         └─────┬────┘
               │
               ▼
              M3 (validator pipeline)
               │
       ┌───────┴───────┐
       │               │
       ▼               ▼
      M4              M5
   (nexus parse)   (nexus-cli)
       │               │
       └───────┬───────┘
               │
               ▼
          Stage A demo
```

M0 is the only true blocker. M1 + M2 land in parallel. M3 gates M4 and M5.

## 5. Cargo dependency additions

| Crate | New deps |
|---|---|
| `signal` | none (`kind.rs` module added; canonical rkyv features already present) |
| `nexus-schema` | none for M0; pending Q-N1 (possibly move IR types to signal) |
| `sema` | `redb = "4"`, `nexus-schema` (path) |
| `criome` | `tokio = "1.35"`, `tokio-util = "0.7"`, `bytes = "1.5"`, `futures = "0.3"`, `nix = "0.27"`, `sema` (path), `tracing = "0.1"` |
| `nexus` | `tokio-util = "0.7"`, `bytes = "1.5"`, `futures = "0.3"`, `nix = "0.27"`, `signal` (path), `nota-serde-core` (path) |
| `nexus-cli` | `tokio = "1.35"`, `nexus` (path; for client-msg types only — reuse the lib half) |
| `lojix-store` | `walkdir = "2.4"`, `tempfile = "3.8"`, `redb = "4"`, `tracing = "0.1"` |
| `lojix` | `lojix-store` (path), `lojix-schema` (path), `walkdir = "2.4"`, `tempfile = "3.8"`, `tokio-util = "0.7"`, `bytes = "1.5"`, `futures = "0.3"`, `tracing = "0.1"` |

Path deps within mentci's symlinked workspace are cleaner than git deps for development; switch to git deps before publishing or for cross-repo CI.

## 6. Open questions surfaced

- **Q-V1: Validator backpressure.** When a TxnBatch fails at op N, do we partially apply ops 1..N-1 (no per-arch.md "all-or-nothing")? Confirmed: all-or-nothing per [reports/070 §2.2](070-nexus-language-and-contract.md). Single redb transaction covers the batch.
- **Q-V2: Genesis principal mechanism (G1 from 076).** Validator step 4 (permission-check) needs a principal-id during genesis when no Principal record exists yet. Lean: hardcoded bootstrap principal `Slot(0)`; permission-check accepts it during `SemaGenesis`-not-yet-asserted phase.
- **Q-S1 from [077](077-nexus-and-signal.md)**: still open (architectural peer status of signal). Not a blocker for Stage A.
- **Q-L1: lojix-store grace period default.** 7 days suggested by Agent 2; configurable; defer until lojix daemon body-fills land (post-Stage-A).
- **Q-W1: Length-delimit max frame size.** `LengthDelimitedCodec` defaults to 8 MiB. Signal frames are typically small but a single TxnBatch could carry many records. Recommend 64 MiB cap.

## 7. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Recursive rkyv bound failures on new types | Follow nexus-schema/src/value.rs pattern (`#[rkyv(omit_bounds)]` + serialize/deserialize/bytecheck bounds) |
| Cargo cycle nexus-schema ↔ sema | nexus-schema is a leaf; sema imports nexus-schema; criome imports sema + signal + lojix-schema; no cycle |
| Nexus parser missing dialect knob | nota-serde-core already exposes `Dialect::Nexus`; verify by reading [nota-serde-core/src/lib.rs](https://github.com/LiGoldragon/nota-serde-core) before M4 |
| First-boot detection fragility | Use a well-known `SemaGenesis` slot (e.g., `Slot(1)`); criome reads at boot; absent → first-boot → dispatch genesis.nexus |
| Genesis permission-check infinite loop | Bootstrap principal hardcoded as `Slot(0)`; permission-check has a `is_genesis_phase` flag set true until SemaGenesis asserted |

## 8. The smallest committable milestone (M1.5)

Even before all of M1, the absolute smallest committable progress: **criome and nexus daemons each `cargo run` and bind their UDS sockets, log "listening on …", accept one connection, log "got frame", reply with handshake-rejected, exit cleanly.** ~50 LoC across the two daemons. Demonstrates the runtime shape; unblocks M2 and M3.

## 9. What this report does *not* recommend

- **Don't scaffold criome-schema as a separate crate yet.** Add `kind.rs` to signal. (Schema-of-schema is criome data; signal is the home for criome data per the signal-as-everything-criome rule.)
- **Don't implement cascade in M3.** Stage A has no rules. Cascade body-fill is post-Stage-A (bootstrap stage D per arch.md §10).
- **Don't optimize rkyv read paths.** 5-15% CPU is below the I/O floor. Profile after Stage A.
- **Don't write integration tests against a live UDS yet.** Use in-process `Frame::encode` ↔ `Frame::decode` round-trips for M0-M3. UDS integration test lands at M5.

## 10. Source acknowledgements

External systems consulted: Datomic, RDFox, Postgres MVCC, FoundationDB, datafrog, differential-dataflow, Crepe, nix-store, patchelf, git GC, Wayland, D-Bus, gRPC/tonic, tokio-util, zbus.

---

*End report 078.*
