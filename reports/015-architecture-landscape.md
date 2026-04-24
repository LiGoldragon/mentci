# 015 — architecture landscape: wide-view synthesis (v4)

*Claude Opus 4.7 / 2026-04-24 · synthesis of 5 + 5 research
passes (two full rounds, 10 agents total). Supersedes v1 (aski
fiction) / v2 (nexusd-as-guardian) / v3 (single-store model).
Ground-truth architecture confirmed by Li: three logic daemons
plus a store daemon, rkyv everywhere internal, sema is the
records database.*

> **v4 correction from v3**
>
> - Four daemons, not two: **nexusd** (messenger), **criomed**
>   (guardian of sema-db), **forged** (compiler), **lojix-stored**
>   (blob guardian).
> - **sema keeps its redb-backed records DB** — not a view; Li
>   confirmed "sema is the database-side of the code."
> - **criome-store renames to lojix-store** (criome namespace was
>   over-extended) — owned by the new `lojix-stored` daemon.
> - **arbor is shelved** for MVP — post-self-hosting optimization.
> - **Three contract crates** (one per daemon↔daemon relation):
>   `criome-msg`, `compile-msg`, `lojix-store-msg`.
> - **Workspace grows 11 → 18 repos** (6 new + 1 rename).

Reading order: §1 thesis → §2 repo map → §3 the four daemons →
§4–§8 deep dives → §14+ tensions, wrongs, open questions.

---

## 1 · Thesis

**The system is a records database (`sema`, owned by
`criomed`), paired with a content-addressed blob store
(`lojix-store`, owned by `lojix-stored`), fronted by a
thin nexus-text messenger (`nexusd`) and served by a
compile daemon (`forged`) that bridges records to Rust
binaries.** Everything internal is rkyv; nexus text
appears only at nexusd's boundary. The MVP closes when
a hand-built binary can edit its own sema-db records via
nexus messages, forged recompiles with the edits, and
the resulting binary shows the new behaviour.

---

## 2 · Repo topology — 18 repos

```
    ┌────────────────────────────────────────────────────────────────────┐
    │                                                                    │
    │  Layer 0 — text surface                                            │
    │    nota (spec)         nexus (spec)                                │
    │        │                      │                                    │
    │        └────────┬─────────────┘                                    │
    │                 ▼                                                  │
    │        nota-serde-core (kernel)                                    │
    │          │          │                                              │
    │          ▼          ▼                                              │
    │      nota-serde  nexus-serde                                       │
    │                                                                    │
    │  Layer 1 — rkyv schema (the lingua franca)                         │
    │        nexus-schema                                                │
    │          • records (Struct, Enum, Module, Program, Opus [NEW])     │
    │          • PatternExpr, QueryOp, ShapeExpr                         │
    │          • Bind, Mutate<T>, Negate<T> (MOVED from nexus-serde)     │
    │          • AnyRecord, Delta, TxOp, Patch, Bindings                 │
    │                                                                    │
    │  Layer 2 — contract crates (per relation)                          │
    │    criome-msg (NEW)      compile-msg (NEW)   lojix-store-msg (NEW) │
    │    nexusd↔criomed        criomed↔forged      {criomed,forged}      │
    │                                              ↔ lojix-stored        │
    │                                                                    │
    │  Layer 3 — storage                                                 │
    │    sema                   lojix-store (RENAMED)                    │
    │    records DB             content-addressed bytes                  │
    │    redb + rkyv + blake3   append-only file + idx cache             │
    │                                                                    │
    │  Layer 4 — daemons                                                 │
    │    nexusd         criomed         forged        lojix-stored       │
    │    messenger      guardian of     compile       guardian of        │
    │    (nexus text    sema            daemon        lojix-store        │
    │    ↔ rkyv)        (records)                     (blobs)            │
    │                                                                    │
    │  Layer 5 — clients + projection + build                            │
    │    nexus-cli              rsc (LIB)         lojix (LIB)            │
    │    text client            records →         cargo/rustc            │
    │                           ProjectedCrate    orchestration          │
    │                           (pure; no I/O)                           │
    │                                                                    │
    │  [SHELVED for MVP]        arbor (prolly-tree versioning)           │
    │                                                                    │
    └────────────────────────────────────────────────────────────────────┘
```

### Full linkedRepos (18 entries + tools-documentation)

```nix
linkedRepos = [
  "tools-documentation"
  # Layer 0 — text
  "nota"  "nota-serde-core"  "nota-serde"
  "nexus" "nexus-serde"
  # Layer 1 — schema
  "nexus-schema"
  # Layer 2 — contracts (all NEW)
  "criome-msg"  "compile-msg"  "lojix-store-msg"
  # Layer 3 — storage
  "sema"  "lojix-store"          # renamed from criome-store
  # Layer 4 — daemons
  "criomed"                       # NEW
  "forged"                        # NEW
  "lojix-stored"                  # NEW
  "nexusd"
  # Layer 5 — clients + build
  "nexus-cli"
  "rsc"  "lojix"
];
```

### Repo-level changes

| Repo | Action | Role after change |
|---|---|---|
| `criomed` | **create** | guardian daemon; owns sema-db |
| `forged` | **create** | compile daemon; drives rsc + lojix + cargo |
| `lojix-stored` | **create** | blob daemon; owns lojix-store |
| `criome-msg` | **create** | rkyv contract crate (nexusd↔criomed) |
| `compile-msg` | **create** | rkyv contract crate (criomed↔forged) |
| `lojix-store-msg` | **create** | rkyv contract crate (store traffic) |
| `criome-store` | **rename** → `lojix-store` | content-addressed bytes; drop arbor dep |
| `nexusd` | shrink role | messenger only; drop `anyhow` |
| `rsc` | **reshape** | `[lib]` (pure projection); tiny `rsc-dump` bin for debug |
| `nexus-schema` | **extend** | add `Opus` record + `PatternExpr` + move `Bind/Mutate/Negate` in |
| `nexus-serde` | re-export | re-exports `Bind/Mutate/Negate` from nexus-schema |
| `sema` | confirm | keeps redb; is the records DB (not a view) |
| `lojix` | minor | stays a library; orchestration helpers that forged can opt into |

---

## 3 · The four daemons

**Text crosses only at nexusd. Everything else is rkyv.**

```
    ┌─────────────────────────────────────────────────────────────────┐
    │  human / LLM  ◄── nexus text ──►  nexusd  (messenger)           │
    │                                      │                          │
    │                                      │ rkyv (criome-msg)        │
    │                                      ▼                          │
    │                                   criomed  (guardian)           │
    │                                    │  │                         │
    │                      (owns sema-db)│  │                         │
    │                                    │  │                         │
    │         rkyv (compile-msg)         │  │ rkyv (lojix-store-msg)  │
    │       ┌────────────────────────────┘  └────────────┐            │
    │       ▼                                             ▼            │
    │     forged  (compiler)  ──── rkyv (lojix-store-msg) ───►        │
    │       │                                             ▼            │
    │       │ calls rsc (lib) + cargo                   lojix-stored  │
    │       │                                            (owns        │
    │       └─► binary bytes ─── rkyv StorePut ──────►   lojix-store) │
    │                                                                 │
    └─────────────────────────────────────────────────────────────────┘
```

### Per-daemon responsibilities

| Operation | nexusd | criomed | forged | lojix-stored |
|---|---|---|---|---|
| Byte-frame I/O to human | ✓ | | | |
| Lex nexus text | ✓ | | | |
| Parse tokens → CriomeRequest | ✓ | | | |
| Protocol-version check | ✓ | | | |
| Schema-shape check | | ✓ | | |
| Execute query / mutation / txn | | ✓ | | |
| Own sema-db (records) | | ✓ | | |
| Manage subscriptions | | ✓ | | |
| Dispatch compile requests | | ✓ | | |
| Receive compile request | | | ✓ | |
| Fetch records from criomed | | | ✓ | |
| Call rsc (pure projection) | | | ✓ | |
| Drive cargo/rustc | | | ✓ | |
| Own lojix-store (blobs) | | | | ✓ |
| Put/get content-addressed bytes | | | | ✓ |
| Return rkyv reply to criomed | | ✓ | ✓ | ✓ |
| Serialize reply to nexus text | ✓ | | | |

### Actor counts (minimum sets)

| Daemon | Actor classes | Why |
|---|---|---|
| nexusd | 4 (ClientListener, Connection, Forwarder, ReconnectSupervisor) | thin; no store |
| criomed | 7 (NexusLink, Router, SemaWriter, SemaReaderPool, SubHub+Watcher, ForgedLink, LojixStoreLink) | writer is single-writer over redb |
| forged | 6 (CompileCoord, SemaReader, SourceProjector, CrateWriter, CargoBuilder, BinaryStasher) | mirrors lojix's DeployCoordinator pattern |
| lojix-stored | 4 (SocketListener, Connection, StoreWriter, StoreReaderPool) | append-only file; single writer |

---

## 4 · Storage: two stores, clear roles

```
    ┌──────────────────────────────────────────────────────────────────┐
    │                                                                  │
    │  ┌─────────────────────────────┐  ┌────────────────────────────┐ │
    │  │  sema  (records DB)         │  │  lojix-store  (blobs)      │ │
    │  │                             │  │                            │ │
    │  │  Owner: criomed             │  │  Owner: lojix-stored       │ │
    │  │                             │  │                            │ │
    │  │  Backend: redb              │  │  Backend: append-only file │ │
    │  │                             │  │  at ~/.lojix/store/        │ │
    │  │  Tables:                    │  │    store.bin (data)        │ │
    │  │    records                  │  │    store.idx (hash→offset) │ │
    │  │    opus_roots               │  │                            │ │
    │  │    opus_history             │  │  Kind bytes (MVP):         │ │
    │  │    meta                     │  │    0x00..0x0F strings      │ │
    │  │                             │  │    0x20 compiled binaries  │ │
    │  │  Keys: blake3(rkyv(record)) │  │    0x21..0x3F reserved     │ │
    │  │  Values: [kind: u8][rkyv…]  │  │                            │ │
    │  │                             │  │  Streaming PutChunk/       │ │
    │  │  Ser: rkyv; identity:       │  │  GetChunk for large blobs  │ │
    │  │  blake3; content-addressed  │  │                            │ │
    │  └─────────────────────────────┘  └────────────────────────────┘ │
    │                                                                  │
    │  Relationship: sema records may carry a `ContentHash` field      │
    │  that references a blob in lojix-store. Criomed does NOT auto-   │
    │  inline blob fetches — clients fetch blobs separately. This      │
    │  keeps query cost legible.                                       │
    │                                                                  │
    └──────────────────────────────────────────────────────────────────┘
```

### sema redb schema

```
records      TableDefinition<&[u8;32], &[u8]>
             key   = blake3(stored_value)
             value = [kind: u8][rkyv_archive(record)...]

opus_roots   TableDefinition<&[u8], &[u8;32]>
             key   = OpusName.as_bytes()
             value = current ProgramId/ModuleId hash

opus_history TableDefinition<(&[u8], u64), &[u8;32]>
             key   = (OpusName, monotonic_seq)
             value = past root hashes (append-only)

meta         TableDefinition<&str, &[u8]>
             "schema_version" → u32
             "format_version" → u32
             "created_at"     → rkyv UnixMillis
```

### Kind-byte registry (MVP scope)

| Range | Owner | Purpose |
|---|---|---|
| `0x00..0x0F` | lojix-stored | transitional strings (pre-enumeration) |
| `0x10..0x1F` | sema (via redb's record table) | nexus-schema record kinds — Type (0x10), Struct (0x11), Enum (0x12), Newtype (0x13), Const (0x14), Module (0x15), Program (0x16), **Opus (0x17, NEW)**, GenericParam (0x18), Origin (0x19), TraitBound (0x1A), TraitDecl (0x1B), TraitImpl (0x1C) |
| `0x20` | lojix-stored | compiled binaries (from forged) |
| `0x21..0x2F` | lojix-stored (reserved) | source tarballs, build logs, cargo-lock |
| `0x30..0x3F` | lojix-stored (reserved) | criomed-addressed opaque blobs (file attachments referenced from sema records) |
| `0xA0`, `0xF0..F1` | (arbor) | shelved for MVP |

**Open**: who owns the kind-byte registry (§14 Q).

---

## 5 · The `Opus` record kind (blocks M5)

Blocks forged's ability to emit `Cargo.toml`. Proposed in `nexus-schema/src/opus.rs`:

```rust
pub struct Opus {
    pub name: OpusName,
    pub version: SemVer,
    pub edition: RustEdition,    // Edition2018 | 2021 | 2024
    pub toolchain: RustToolchain,
    pub root: ModuleId,
    pub deps: Vec<OpusDep>,
    pub emit: EmitKind,          // Binary { entry: ConstId } | Library | Both
    pub features: Vec<FeatureFlag>,
}

pub struct OpusDep {
    pub target: OpusId,
    pub as_name: OpusName,       // cargo's rename support
    pub features: Vec<FeatureFlag>,
    pub optional: bool,
}

pub struct RustToolchain {
    pub channel: ToolchainChannel,
    pub components: Vec<String>,
}

pub struct SemVer { pub major: u16, pub minor: u16, pub patch: u16 }
```

Add `OpusId` newtype in `names.rs`; assign kind byte `0x17`.

---

## 6 · Compilation pipeline

```
    criomed ── CompileRequest (rkyv) ──►  forged
                                            │
          ┌─────────────────────────────────┤
          │                                 │
          ▼                                 ▼
      SemaReader                      (one RPC pipeline per request)
      ─────────────
      reads records for opus + transitive deps from criomed
                                            │
                                            ▼
                                      SourceProjector
                                      ─────────────────
                                      calls rsc::project_closure(resolver, root)
                                      → Vec<ProjectedCrate>
                                            │
                                            ▼
                                      CrateWriter
                                      ──────────
                                      materializes .rs tree under
                                      ~/.cache/forged/<opus_hash>/<short>/
                                      path deps as sibling dirs
                                            │
                                            ▼
                                      CargoBuilder
                                      ──────────
                                      spawns cargo build --message-format=json
                                      captures structured diagnostics
                                            │
                                            ▼
                                      BinaryStasher
                                      ──────────
                                      StorePut(kind=0x20, binary_bytes)
                                      → BinaryHash
                                            │
                                            ▼
    criomed ◄── CompileReply { binary: BinaryHash, ... } ── forged
```

### rsc as pure library

```rust
// rsc/src/lib.rs
pub trait RecordResolver { /* fn opus/module/enum/struct/… */ }

pub struct ProjectedCrate {
    pub opus: OpusId,
    pub cargo_toml: String,
    pub files: BTreeMap<PathBuf, String>,
}

pub fn project<R: RecordResolver>(r: &R, root: OpusId) -> Result<ProjectedCrate>;
pub fn project_closure<R: RecordResolver>(r: &R, root: OpusId) -> Result<Vec<ProjectedCrate>>;
```

No I/O. A tiny `rsc-dump` binary (behind `required-features = ["cli"]`) stays for debug.

### lojix stays a library

Open verdict: forged **duplicates** lojix's ractor coordinator pattern rather than depending on it. `lojix` stays owned by the nix/deploy flow. When both crates stabilize, extract a `lojix-core` (or similar) with the shared `unwrap_call` + pipeline helpers. Premature today.

### Dependency resolution — path deps

```
~/.cache/forged/<root_opus_hash>/<short>/
  ├── <root_opus_name>/
  │   ├── Cargo.toml     [dependencies] y = { path = "../y" }
  │   └── src/…
  └── <dep_opus_name>/
      ├── Cargo.toml
      └── src/…
```

Synthesized workspace `Cargo.toml` at the outer dir lists all opera as members → shared `target/` across the closure.

---

## 7 · Self-hosting loop

```
  (0) BOOTSTRAP (one-shot, pre-M6)
      syn-based loader reads source of the 18 workspace repos,
      converts each syn::Item into nexus-schema records,
      writes directly to sema-db (bypassing nexusd).
      Writes file-like data into lojix-store by hash.
      Produces initial sema-db + lojix-store.

  (1) human: nexus-cli "(Compile (Opus nexusd))"
  (2) nexus-cli → nexusd (text)
  (3) nexusd → criomed (rkyv CriomeRequest::Compile(OpusId))
  (4) criomed → forged (rkyv CompileRequest)
  (5) forged:
       a. SemaReader fetches opus closure from criomed
       b. rsc::project_closure(…) → Vec<ProjectedCrate>
       c. CrateWriter writes .rs tree to cache dir
       d. CargoBuilder invokes cargo build
       e. BinaryStasher puts binary bytes into lojix-stored (kind 0x20)
  (6) forged → criomed (rkyv CompileReply::Ok { binary: hash })
  (7) criomed → nexusd → human ("(CompileOk h=…)")

  (8) human: nexus-cli "(Launch h=…)"
  (9) criomed fetches bytes from lojix-stored, exec's process
 (10) new process connects back to nexusd, sends Assert/Mutate
 (11) criomed writes edited records to sema-db
 (12) next Compile picks up new sema-rev → new binary — LOOP CLOSES
```

### Capstone feature (recommended)

**"Add a new subcommand to nexus-cli via nexus messages, recompile, observe it working."** Previous candidate (`list-opuses`) only demonstrated the read path; this exercises the mutation path end-to-end. Small (~40 LoC of generated Rust), objectively verifiable (old binary rejects unknown subcommand; new binary responds).

---

## 8 · Contract crate sketches

All three use the rkyv feature-set that matches nexus-schema:
`default-features = false, features = ["std", "bytecheck", "little_endian", "pointer_width_32", "unaligned"]`.

### criome-msg (nexusd ↔ criomed)

```rust
pub const PROTOCOL_VERSION: u32 = 1;

pub struct Envelope {
    pub version: ProtocolVersion,  // first field → first 4 bytes of frame
    pub id: MessageId,             // [u8; 16], correlation
    pub payload: Payload,
}

pub enum Payload {
    Request(CriomeRequest),
    Reply(CriomeReply),
    StreamItem { sub: SubId, delta: Delta },
}

pub enum CriomeRequest {
    Lookup { hash: Hash },
    Scan { kind: u8, limit: Option<u32> },
    Query { pattern: PatternExpr, limit: Option<u32> },
    Shape { pattern: PatternExpr, shape: ShapeExpr },
    Assert { record: AnyRecord },
    Retract { hash: Hash },
    Mutate { target: Hash, patch: Patch },
    Transaction(Vec<TxOp>),
    Subscribe { pattern: PatternExpr },
    Unsubscribe { sub: SubId },
    StorePut { kind: u8, data: Vec<u8> },
    StoreGet { hash: Hash },
    StoreScan { kind: u8 },
    Compile { opus: OpusId },
    Launch { binary: Hash, argv: Vec<String> },
    Ping, Shutdown,
}

pub enum CriomeReply { Ok, Record(AnyRecord), Records(Vec<AnyRecord>),
    Bindings(Vec<Bindings>), Shape(ShapeValue), Bytes(Vec<u8>),
    HashReply(Hash), SubConfirmed(SubId), CompileDone { opus, binary },
    CompileFailed { opus, error }, Launched { pid: u32 },
    Err(CriomeError) }
```

Depends on `nexus-schema` for `AnyRecord`, `PatternExpr`, `OpusId`, `Hash`,
`Bind`, `Mutate`, `Negate`, query operators.

### compile-msg (criomed ↔ forged)

```rust
pub const PROTOCOL_VERSION: u32 = 1;

pub struct CompileRequest {
    pub request_id: MessageId,
    pub opus: OpusId,
    pub sema_rev: Hash,
    pub profile: Profile,            // Dev | Release | ReleaseWithDebug
    pub target: Option<TargetTriple>,
    pub toolchain_override: Option<RustToolchain>,
    pub dep_mode: DepMode,           // PathDeps | GitDeps
    pub cache_mode: CacheMode,       // Shared | Isolated
}

pub enum CompileReply {
    Ok(CompileOutcome),
    Err(CompileError),
}

pub struct CompileOutcome {
    pub binary: BinaryHash,          // in lojix-store, kind 0x20
    pub store_kind: u8,              // 0x20 reserved
    pub cargo_lock_hash: Hash,
    pub crate_hashes: Vec<(OpusId, Hash)>,
    pub duration_ms: u64,
    pub diagnostics: Diagnostics,
}

pub enum CompileEvent {             // optional streaming progress
    Enqueued, ResolvingSema, Projecting,
    CrateWritten { crate_hash: Hash },
    DepsCompiled { done: u32, total: u32 },
    LinkingBinary, Done(CompileReply),
}
```

Depends on `nexus-schema` for `OpusId`, `Hash`.

### lojix-store-msg (store traffic)

```rust
pub const PROTOCOL_VERSION: u32 = 1;

pub enum LojixStoreRequest {
    Put { kind: u8, data: Vec<u8> },               // one-shot, cap 16 MiB
    PutBegin { kind: u8, total_len: u64 },          // → SessionId
    PutChunk { session: SessionId, bytes: Vec<u8> },
    PutCommit { session: SessionId, expected: Option<Hash> },
    PutAbort { session: SessionId },
    Get { hash: Hash },
    GetBegin { hash: Hash },
    Contains { hash: Hash },
    Scan { kind: u8, after: Option<Hash> },         // cursor-paginated
    Stats,
}

pub enum LojixStoreReply {
    HashReply(Hash), Bytes(Vec<u8>), Bool(bool),
    Hashes { page: Vec<Hash>, next: Option<Hash> },
    PutSession { session: SessionId },
    GetSession { session: SessionId, total_len: u64 },
    Chunk { session: SessionId, offset: u64, bytes: Vec<u8>, final_: bool },
    Stats { by_kind: Vec<(u8, u64)>, bin_bytes: u64 },
    Err(LojixStoreError),
}
```

**Standalone** — `rkyv` + `thiserror` only. No `nexus-schema` dep. Keeps the
store reusable without dragging the schema.

---

## 9 · nexus-schema grows

Per the contract-crate research, `nexus-schema` absorbs several types
currently living in nexus-serde:

| Type | From | To | Reason |
|---|---|---|---|
| `Bind` | nexus-serde | nexus-schema | wire-bearing (criome-msg needs rkyv) |
| `Mutate<T>` | nexus-serde | nexus-schema | same |
| `Negate<T>` | nexus-serde | nexus-schema | same |
| `PatternExpr` | new | nexus-schema | dual-derive (rkyv + serde) |
| `PatternAtom` | new | nexus-schema | |
| `QueryOp` | new | nexus-schema | Limit, Offset, OrderBy, Count, Sum, etc. |
| `ShapeExpr` | new | nexus-schema | |
| `TxOp`, `Patch`, `Delta`, `Bindings` | new | nexus-schema | |
| `AnyRecord` tagged union | new | nexus-schema | |
| `Opus`, `OpusDep`, `OpusId` | new | nexus-schema | §5 |

nexus-serde re-exports the wrappers so existing consumers (doctests, tests) don't break. Recommended: schema-agnostic deserialize — bind names stay as raw strings; field-identity resolution happens in criomed.

---

## 10 · The delimiter-family matrix (unchanged)

```
                    ┌──────┬──────┬──────┬──────────────┐
                    │ bare │  |   │  ||  │    notes     │
                    ├──────┼──────┼──────┼──────────────┤
     ( )  record    │  rec │ pat  │opt-  │ MATCHING     │
                    │      │      │ pat  │              │
     { }  composite │ shape│const-│atomic│ SCOPE        │
                    │      │rain  │ txn  │              │
     [ ]  evaluation│ str  │multi-│rule  │ CONTENT      │
                    │      │line  │(P2)  │              │
     < >  flow      │ seq  │stream│wind- │ ORDERED      │
                    │      │(sub) │ owed │              │
                    │      │      │(P2)  │              │
                    └──────┴──────┴──────┴──────────────┘

  IN LEXER NOW:  bare + | column complete for all 4 families
                 || column: ( || ), { || } done
                 [ || ] and < || > GAPS (report 014 §3.1)
```

Grammar freezes after Phase 2. Post-Phase-2 features are records and
sentinel wrappers, not new syntax.

---

## 11 · Workspace bootstrap sequence

**Critical path**: `rename store → contract crates → daemon scaffolds → Opus record → nexus-schema extensions → impl`.

| Phase | Tasks | Parallelism | Rough cost |
|---|---|---|---|
| 0 | rename criome-store → lojix-store; update mentci-next | serial | ~1h |
| 1 | create 3 contract-crate scaffolds (criome-msg, compile-msg, lojix-store-msg) | parallel | ~30m × 3 |
| 2 | create 3 daemon scaffolds (criomed, forged, lojix-stored) | parallel | ~45m × 3 |
| 3 | existing-repo updates (nexusd role, rsc → lib, lojix features, sema confirm, nexus-schema Opus + PatternExpr + wrappers move) | parallel | ~1d total |
| 4 | implementation milestones (M2 method bodies, M3 sema, forged pipeline, criomed actors, nexusd messenger, bootstrap loader, M6 capstone) | mostly serial by dependency | weeks |

**Tier-A cheap wins** (no design decisions needed):
- Land `<|| ||>` + `||>` lexer tokens (~20 LoC)
- Fix clippy warning in nexus-serde tests (1 line)
- Replace `anyhow` with typed `Error` in nexusd + nexus-cli (~30 LoC)
- Rename criome-store → lojix-store (disk move + one Cargo.toml edit)

---

## 12 · Stress-test catalogue

Can the architecture absorb these?

| Stress | Handling | Grammar cost |
|---|---|---:|
| Signatures on records | `Signed<T>` sentinel wrapper | 0 |
| Peer identity | `Peer` record | 0 |
| Cross-opus queries | `(Opus X (\| pat \|))` + federation actor | 0 |
| Time-travel | `TimeAt`/`TimeBetween`/`TimeAll` records | 0 |
| Large blob (>16 MiB) | PutChunk streaming in lojix-store-msg | 0 |
| Mutation path in self-hosting | Assert → sema-rev → Compile → new binary | 0 |
| New record kind | schema addition; assign new kind byte; rsc regenerates | 0 |
| Swap lojix-store backend | `Store` trait is the seam | 0 |
| Swap sema backend | same, for redb | 0 |
| **Grammar needs a keyword** | new sigil or delimiter family | HIGH |
| **Ambiguous lookahead** | breaks first-token-decidable | FATAL |
| **Mutable record identity** | breaks content addressing | FATAL |

---

## 13 · Tensions & contradictions caught

### T1 · forged ↔ lojix-stored direct or via criomed?

Forged produces a binary (10-100 MB). Options:
- (a) Forged has its own UDS to lojix-stored, pushes directly. Large blobs don't traverse criomed.
- (b) Forged replies to criomed with bytes; criomed relays the Put.
- (c) Forged returns a hash only; criomed separately fetches bytes from forged's temp dir.

Agents lean (a). Capability token issued by criomed when it dispatches the compile request. Confirmed as open question.

### T2 · Launch ownership

Who exec's the compiled binary?
- criomed (has client UDS, knows BinaryHash, fetches from lojix-stored) — lean
- forged (already produced the binary) — rejected: stretches forged's scope
- separate `launcher` daemon — overkill for MVP

Lean criomed. Open.

### T3 · kind-byte registry

Kinds span sema (0x10..0x1F) and lojix-stored (0x20+, 0xF0..). Who prevents collisions? Options:
- A constant table in a shared crate (e.g., lojix-store-msg)
- Each owner declares its range in its own crate
- A central `kind-bytes` tiny crate

Open.

### T4 · Bootstrap loader home

`sema/examples/bootstrap.rs`, `criomed/src/bin/bootstrap.rs`, or new
`sema-bootstrap` repo? Agents split. Lean: `criomed/src/bin/bootstrap.rs`
since criomed is the only process with write access anyway.

### T5 · lojix-extend vs duplicate

Prior research (round 1) recommended extending lojix with a compile
coordinator. Round 2 recommended forged duplicates the pattern in a new
repo. User already decided forged is its own daemon. Resolution: forged
is its own repo; lojix stays owned by nix-deploy; `lojix-core` (shared
ractor helpers) is a post-MVP refactor.

### T6 · `<|| ||>` / `||>` still unpatched

Report 014 §3.1 flagged this. No progress. Blocks Phase 2 windowed-stream
work. ~20 LoC fix. Ship in Tier-A.

---

## 14 · Things I probably have wrong

### W1 · PatternExpr custom-deserialize cost

Estimated 150 LoC. serde's untagged enum try-each-variant will produce
opaque errors; a hand-written `Deserialize` is likely 300+ LoC. Not a
blocker, but larger than stated.

### W2 · Single-writer at federation scale

criomed is single-writer over redb. Fine for a local node. For future
federation (many criomed nodes writing to many opera), the model needs
cross-peer coordination — unspecified.

### W3 · Cargo determinism

Agents recommend shared target-dir for MVP (fast, not hermetic). Same
`(opus, sema_rev, profile)` may produce different binaries across runs
(timestamps, codegen-units ordering). "Content-addressed binary" is
correct but "deterministic build" is not — document.

### W4 · Sentinel wrapper permissiveness

`Mutate(42)`, `Mutate(None)` serialize fine but are semantic gray zones.
nexus-schema's wrapper-type definitions don't enforce "inner must be a
record." Downstream types should self-constrain.

### W5 · Bootstrap loader covers all 18 repos

The syn loader has to parse ~5000 LoC of Rust across 18 repos. May hit
edge cases (macros, proc-macros, complex type bounds) that nexus-schema
can't yet represent. Might need a subset bootstrap first.

### W6 · Solstice timing

Still unconfirmed. Summer 2026 (2 months) or winter 2026 (8 months).

### W7 · Schema versioning across contract crates

Each contract crate has its own `PROTOCOL_VERSION`. Cross-bumping (e.g.,
adding a field to `AnyRecord` in nexus-schema should probably bump
criome-msg's version) is not automated — risks silent incompatibility.

### W8 · `sema` without arbor has no versioning primitive

Before arbor was shelved, arbor was going to handle versioning. Without
it, sema's `opus_history` table is manually managed — old roots stay
indefinitely unless a compact pass deletes them. Post-MVP concern but
worth naming.

### W9 · Large compile progress feedback

MVP `CompileReply` is one-shot. Cold builds can take minutes. No interim
progress to the human client. `CompileEvent` streaming proposed but not
wired. UX gap.

### W10 · Repo proliferation

18 repos is a lot. Contract-per-relation is user-directed but the jj
workflow cost per repo (bd init, flake.nix, rust-toolchain.toml,
Cargo.toml, README, LICENSE, .gitignore) is ~20 min × 6 new = 2 hours
just to scaffold. Real.

---

## 15 · Consolidated open questions

**Q1 — Solstice target** (W6).

**Q2 — T1 — forged ↔ lojix-stored direct (lean yes) or via criomed**.

**Q3 — T2 — Launch ownership (lean criomed)**.

**Q4 — T3 — kind-byte registry home** — shared crate vs per-owner.

**Q5 — T4 — Bootstrap loader home** — `criomed/src/bin/bootstrap.rs` lean.

**Q6 — PatternExpr hand-written Deserialize vs serde untagged** (W1). Lean hand-written for error quality.

**Q7 — Cargo determinism strategy** (W3) — shared target-dir MVP, hermetic later.

**Q8 — Subscription durability across criomed restart** — lean
re-subscribe (client responsibility).

**Q9 — Compile progress streaming** (W9) — lean add in Phase 1 not MVP.

**Q10 — Move `Bind`/`Mutate`/`Negate` into nexus-schema** — lean yes, for
rkyv availability; nexus-serde re-exports.

**Q11 — Capstone feature** — recommend "add nexus-cli subcommand via
edits" (mutation-path demo, not `list-opuses` read-only).

**Q12 — Protocol-version cross-bump coordination** (W7) — per-crate
independent; document policy.

**Q13 — Workspace bootstrap sequencing** (§11) — concrete phase plan
approved?

**Q14 — jj workflow for 6 new repos** — full scaffold each, or minimal
(Cargo.toml + lib.rs)?

**Q15 — `Opus` record kind** (§5) — approve the shape?

**Q16 — schema-agnostic pattern binds** — confirm raw `@h` strings?

**Q17 — rsc `[[bin]]` keep for debug** — yes/no?

**Q18 — large-blob chunking urgency** — MVP one-shot cap 16 MiB, chunk
later?

**Q19 — Lexer gap `<|| ||>` / `||>`** — land in Tier-A (recommended).

**Q20 — `arbor` timing** — confirmed shelved for MVP; when does it
return?

---

## 16 · Prioritized recommendations

### Tier A — immediate (≤2h, no design decisions)

1. **Rename** `criome-store` → `lojix-store` (disk + Cargo.toml)
2. **Land** `<|| ||>` + `||>` lexer tokens (~20 LoC)
3. **Fix** clippy warning in nexus-serde tests (1 line)
4. **Replace** `anyhow` with typed Error in nexusd + nexus-cli

### Tier B — Tier-A decisions before impl work

1. Confirm Q1 solstice
2. Confirm Q11 capstone
3. Approve Q15 Opus record shape
4. Approve §11 workspace sequencing plan
5. Confirm Q10 wrapper-type move into nexus-schema

### Tier C — parallel scaffold ~2h total

1. Create `criome-msg`, `compile-msg`, `lojix-store-msg` (contract crates)
2. Create `criomed`, `forged`, `lojix-stored` (daemon scaffolds)
3. Update mentci-next/devshell.nix + AGENTS.md
4. Reshape rsc as `[lib]`; keep `rsc-dump` debug bin
5. Add `Opus` record kind + `OpusId` newtype to nexus-schema

### Tier D — implementation (weeks)

1. M2 method-body layer in nexus-schema
2. PatternExpr + QueryOp + wrappers in nexus-schema (~700 LoC)
3. sema redb tables + opus_roots
4. lojix-stored file backend + actor topology
5. criomed actor topology
6. forged compilation coordinator
7. nexusd messenger (thin)
8. bootstrap loader
9. Capstone (M6)

### Suggested next session

Close Tier A (90 min). Then ask Li for Tier B decisions. Then Tier C can
parallelize over 2-3 sessions; Tier D is phase-work.

---

## 17 · Reading back

What this document is:
- A working map of the corrected 4-daemon architecture at 2026-04-24.
- Synthesis of 10 research passes (2 rounds × 5 agents).
- Explicit list of what I think I have wrong (§14).
- Catalogue of 20 open questions (§15) and tiered recommendations (§16).

What this document is not:
- A design specification. No decisions were made here.
- Final. The tensions in §13 and the "wrongness" in §14 will be
  rewritten as understanding deepens.

Redirect liberally.

---

*End report 015 v4 — four daemons: nexusd, criomed, forged, lojix-stored.
sema (records) + lojix-store (blobs) as peer stores. arbor shelved.
18 repos total; 6 to create, 1 to rename.*
