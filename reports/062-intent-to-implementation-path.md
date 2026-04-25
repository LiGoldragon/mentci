# 062 — intent → implementation path

*Claude Opus 4.7 · forward-looking multi-agent research
moving from intent (report/061) to concrete implementation.
Four parallel agents surveyed (1) what exists today across
active repos, (2) the critical path to a walking skeleton,
(3) the first record-kind end-to-end, and (4) the minimum
inter-daemon contracts plus transition bridges. Strict repo
allow-lists blocked aski and all retired-aski-adjacent
repos. This report presents the path and surfaces ten
concrete implementation questions.*

---

## 1 · Implementation front (state at 2026-04-25)

Where working code ends and speculative code begins.

### 1.1 · Operational today

| Crate / repo | LoC | Role |
|---|---|---|
| nota-serde-core | ~1,620 | Lexer + serde kernel; dialects Nota + Nexus; 127 tests ✓ |
| nota-serde | 28 | Thin façade for Nota dialect |
| nexus-serde | 74 | Thin façade for Nexus dialect; Bind/Mutate/Negate wrappers |
| horizon-rs | ~1,780 | `ClusterProposal → Horizon` projection; consumed by lojix |
| lojix (monolith) | ~820 | CriomOS deploy CLI; ractor pipeline; Li's daily tool |
| CriomOS | — | NixOS cluster, flake.lock pinned, running daily |

### 1.2 · Skeleton-as-design (types locked, `todo!()` bodies)

| Crate | LoC | Notes |
|---|---|---|
| lojix-store | ~430 | `StoreEntryHash`, `StoreReader`, `StoreWriter`, `BundleFromNix`, `IndexReader/Writer`, `BundlePolicy` |
| nexus-schema | ~500 | rkyv + serde derives on `Enum`, `Struct`, `Newtype`, `Const`, `Module`, `Program` — **no validation logic yet** |

### 1.3 · Stubs (< 30 LoC)

`nexusd`, `sema`, `rsc` — lib/main stubs, Cargo dependency lists, nothing else.

### 1.4 · Canon-missing (not yet a crate)

- `criome-msg` — nexusd ↔ criomed contract
- `lojix-msg` — criomed ↔ lojixd contract
- `criomed` — the sema engine itself
- `lojixd` — the effects daemon

### 1.5 · Gap magnitude

Roughly **3,000–4,000 LoC of new implementation** to close the minimum end-to-end loop (nexusd actor + criomed engine + sema redb schema + rsc projector + lojixd skeleton). Structure is clear; the engineering is substantial but not unprecedented.

---

## 2 · The walking skeleton (minimum end-to-end proof)

One request, end-to-end, ending at an executable in lojix-store:

> User types at `nexus-cli`:
> `(Assert (Fn :slot 1024 :name "id" :sig (Sig (Param x I32) I32) :body (Block (Var x))))`
>
> **nexusd** parses via nota-serde-core (`Dialect::Nexus`) → `criome_msg::Request::Assert { kind_id, content }` rkyv envelope.
>
> **criomed** validates shape against seed `KindDecl` for `Fn`, resolves refs, mints `SlotBinding { slot: 1024, content_hash, display_name: "id" }`, appends `ChangeLogEntry`, writes to **sema** (redb).
>
> User then: `(Compile (Opus :slot 0))`.
>
> **criomed** walks `OpusDep` closure → calls **rsc** in-process → rsc writes `src/lib.rs` + `Cargo.toml` + `flake.nix` to a scratch workdir → criomed sends `lojix_msg::RunNix { flake_ref, attr }` to **lojixd** → lojixd spawns `nix build` (crane + fenix).
>
> **lojix-store** `BundleFromNix` copies closure → `~/.lojix/store/<blake3>/` → returns `StoreEntryHash`.
>
> criomed asserts `CompiledBinary { opus, store_entry_hash }` → reply → binary is `exec`able at that path.

Closing this loop proves the architecture. Every later feature (cascades, rules, multi-machine interaction, machina-chk, world-fact records) extends from here.

---

## 3 · Critical path (ordered tasks)

Each task ≈ 15-word description + dependency arrows. Names follow current canonical lexicon (machina = code category; machina-chk = native checker; hacky-stack = current lojix/horizon-rs/CriomOS scaffolding).

1. **Lock `nexus-schema` v0.0.1 record kinds.** Pin rkyv shapes for `Fn`, `Block`, `Var`, `Sig`, `Param`, `Type::I32`, `SlotBinding`, `Opus`, `OpusDep`, `RustToolchainPin`, `CompiledBinary`, `KindDecl`, `ChangeLogEntry`. Only `Slot(u64)` references — no generics. *← depends on: nothing.*
2. **Create `criome-msg` v0.0.1.** Four request verbs (`Assert`, `Query`, `Retract`, `Compile`) + three replies (`Asserted {content_hash, rev}`, `QueryResult {records}`, `Rejected {diagnostic}`). *← 1.*
3. **Create `lojix-msg` v0.0.1.** Two verbs (`RunNix {flake_ref, attr, target}`, `BundleIntoLojixStore {nix_output_path, policy}`) + replies. Deferred: GC, materialise, deploy. *← 1.*
4. **Seed-loader + sema redb writer in `criomed`.** Baked `KindRegistry`; `SemaWriter::assert(bytes, kind_id, name) -> (Slot, Hash, Rev)`. Three tables: `index::*`, `changelog::*`, `rev_index`. No rules. *← 1.*
5. **nexus parser path in `nexusd`.** Reuse nota-serde-core at `Dialect::Nexus`; map `(Assert …)` → `Request::Assert`. Skip binds/mutates at v0. *← 2.*
6. **Compile plan path in criomed.** Walk `OpusDep` closure → `rsc::project(&records, &workdir)` → emit `lojix_msg::RunNix`. *← 2, 3, 4.*
7. **rsc::project for Fn/Block/Var/Sig/Param/Type::I32.** Writes `src/lib.rs`, fixed `Cargo.toml`, templated `flake.nix` (crane + fenix). No formatting cleverness. *← 1.*
8. **lojixd::run_nix.** Spawn `nix build <flake_ref>#<attr> --no-link --print-out-paths`, capture stdout. *← 3.*
9. **BundleFromNix → StoreEntryHash.** Replace `todo!()` in lojix-store's `bundle.rs`/`writer.rs`/`hash.rs::to_hex`: walk closure, normalise, blake3, copy, index. RPATH rewrite stubbable. *← 3, 8.*
10. **Close the loop.** On `Bundled {store_entry_hash}`, criomed asserts `CompiledBinary` → replies → nexusd serialises reply. *← 6, 9.*
11. **Smoke test.** Two-command `nexus-cli` script; verify `~/.lojix/store/<hash>/bin/id` runs. *← 5, 10.*

### Parallel tracks (off critical path)

- Real lojix-store polish: RPATH rewrite via `patchelf`, timestamp normalisation, `BundlePolicy::linux_default`, GC reachability index.
- Report/030 Phases B–C: real UDS transport (critical path collapses to in-process, see stubbing below).
- horizon-rs absorption sketch (design only; no implementation).
- Per-kind `ChangeLogEntry` + `rev_index` design per report/048.
- machina-chk Phase 1 (schema validity — implicit in step 4).
- Authoring seed `Opus` + `OpusDep` as rkyv literals in criomed source.

---

## 4 · First record-kind: two defensible choices

### 4.1 · `Const` (simplest; tests pipeline not codegen)

```nexus
(Assert
  (Const
    (name MAX_ATTEMPTS)
    (ty (Ref Type-Id-for-U32))
    (expr (Literal (U32 5)))
  )
)
```

```rust
// nexus-schema: canonical shape
pub struct Const {
    pub name: ConstName,
    pub ty: TypeId,
    pub expr: ExprId,
}
```

```rust
// rsc emission: one line of Rust
pub const MAX_ATTEMPTS: u32 = 5;
```

**Pro:** minimal surface; proves parse → validate → store → emit in one small bite.
**Con:** doesn't exercise rsc's hardest phase (expression/statement codegen).

### 4.2 · `Fn` (canonical v1 target; heavier)

```nexus
(Assert
  (Fn :slot 1024 :name "id"
    (Sig (Param x I32) I32)
    (Block (Var x))))
```

```rust
pub const fn id(x: i32) -> i32 { x }  // rsc emission
```

**Pro:** exercises body codegen, the load-bearing rsc phase; closer to real self-hosting.
**Con:** surfaces most edge cases late; body codegen unknowns are the largest.

### 4.3 · Recommendation

Land `Const` as the "pipeline heartbeat" (week 1 — proves the architecture); follow with `Fn` as "first real code" (weeks 2-4 — proves rsc codegen). This gives two end-to-end successes before the harder phases (cascades, multi-kind, machina-chk).

---

## 5 · Three contracts at v0.0.1 (sketches)

### 5.1 · `nexus-schema` record-kind list

```rust
pub enum RecordKind {
    // machina (code)
    Opus, Derivation, OpusDep,
    Module, Fn, Struct, Enum, Const,
    // operational
    CompiledBinary, DeployPlan, ChangeLogEntry, CapabilityToken,
    // schema/metadata
    SlotBinding, MemberEntry, KindDecl,
    // Rule — post-MVP
}

pub struct Slot(pub u64);
pub struct Rev(pub u64);
pub struct StoreEntryHash(pub [u8; 32]);  // blake3
```

### 5.2 · `criome-msg` v0.0.1 (nexusd ↔ criomed)

```rust
pub enum Request {
    Assert  { kind_id: KindId, content: Vec<u8> },
    Mutate  { slot: Slot, new_content: Vec<u8> },  // optional v0 (see Q8)
    Retract { slot: Slot },
    Query   { pattern: RawPattern },
    Compile { opus_slot: Slot },
    Subscribe { slot: Slot },                       // optional v0
}

pub enum Reply {
    Ok           { effect: Vec<SlotRef> },
    Rejected     { diagnostic: Diagnostic },
    QueryResult  { records: Vec<(Slot, Vec<u8>)> },
}
```

### 5.3 · `lojix-msg` v0.0.1 (criomed ↔ lojixd)

```rust
pub enum Verb {
    RunNix               { flake_ref: String, attr: String, overrides: Vec<(String, String)>, target: String },
    BundleIntoLojixStore { nix_closure_root: String, nix_closure_deps: Vec<String>, policy: BundlePolicy },
    RunNixosRebuild      { flake_ref: String, action: DeployAction },
    PutStoreEntry        { source_path: String, metadata: StoreMeta },
    GetStorePath         { hash: StoreEntryHash },
    MaterializeFiles     { entries: Vec<(StoreEntryHash, String)> },
}

pub enum Outcome {
    NixBuildComplete { store_hash: StoreEntryHash, wall_ms: u64 },
    Bundled          { hash: StoreEntryHash, narhash: String },
    DeployComplete   { action: DeployAction, exit_code: i32 },
    StorePathResolved(String),
    Materialized, Deleted,
}
```

### 5.4 · Transport + error model

UDS + length-prefixed rkyv frames. nota-serde-core's rkyv machinery is shared. Structured `Diagnostic` records (per report/060 §4) in replies; avoids `Result<T, Error>` minimalism at daemon boundaries where context matters.

```rust
pub struct Diagnostic {
    pub level: Level,                       // Error | Warning | Info
    pub code: String,                       // "SCHEMA_VIOLATION" | "REF_NOT_FOUND"
    pub message: String,
    pub context: Vec<(String, String)>,
    pub span: Option<SourceSpan>,
}
```

### 5.5 · Capability token (sketch)

```rust
pub struct CapabilityToken {
    pub operation:  CapOp,
    pub subject:    CapSubject,
    pub issued_at:  Rev,
    pub expires_at: Option<Rev>,
    pub bearer:     String,
    pub signature:  Vec<u8>,  // signed by criomed's keypair
}
```

Criomed signs; lojixd verifies against criomed's public key (hardcoded at daemon launch); no round-trip. Cross-machine tokens add machine identity to `bearer` and would move to BLS quorum (report/060 §2).

---

## 6 · Stubbing for v0 (proposals)

Each collapses a design surface for the first loop; each has an explicit un-stub trigger.

| # | Stub | Lost | Regained | Un-stub trigger |
|---|---|---|---|---|
| S1 | **Single process** (nexusd + criomed + lojixd as three modules) | Fault isolation; rkyv wire-format exercise | Real isolation; security boundary | Report/030 Phase C — real UDS |
| S2 | **Skip RPATH + timestamp canonicalisation** in lojix-store | Cross-machine hash reproducibility | Real content-addressing | Second machine wants to reproduce a hash |
| S3 | **Skip rules/cascades** | Re-derivation; the "evaluation, not storage" claim | The actual sema-as-engine property | Second record-kind that needs a derivation (likely `TraitImpl` coherence) |
| S4 | **Skip Mutate + Subscribe verbs** | In-place edits; slot continuity on rename | Edit UX of report/057 | First records-authored crate (report/051 Phase 2) |
| S5 | **Skip capability tokens; single hardcoded principal** | The whole authz layer | BLS quorum (report/060 §2) | Second principal exists — i.e., self-host close or later |

---

## 7 · Transition bridges (hacky-stack → proper-stack)

Absorption pattern: current in-process call in `lojix` → tomorrow's inter-daemon verb.

| Today (lojix monolith) | Tomorrow (proper stack) | Bridge |
|---|---|---|
| `lojix deploy` clap CLI → ractor actor pipeline | thin CLI → `lojix_msg::DeployRun` → lojixd | Report/030 Phase D: `--via-daemon` flag routes one actor; rest in-process |
| `ProposalReader` (ractor) | `lojixd::ProposalReader` | Phase D dual-mode → Phase E default flip |
| `HorizonProjector` (horizon-lib in-process) | `lojixd::HorizonProjector` | Move logic verbatim; wrap in lojix-msg envelope |
| `NixBuilder` (invokes nix; writes `/nix/store`) | `lojixd::NixBuilder` + `BundleFromNix` | `RunNix` + `BundleIntoLojixStore` verbs |
| horizon writes flake file directly | horizon records in sema; rsc emits flake.nix | Post-MVP: horizon-rs projection becomes sema rule |

**Invariant across every phase:** `lojix deploy …` never breaks from Li's shell workflow (report/030 §5).

---

## 8 · Prioritised question bank

Ten questions, ordered by how much design surface each unblocks.

### Q1 · Single-process collapse for v0 walking skeleton?

Option A: three daemons, UDS transport from day one.
Option B: single process, three modules, contracts real but transport in-process.
B is ~3× cheaper and closes the loop faster; A exercises the wire format from day one.

### Q2 · Order of attack for opening tasks?

Three candidates for "what to land first":
(a) lock nexus-schema types so everything else can type-check against them;
(b) scaffold daemon shells minimally so the topology is visible early;
(c) prototype rsc emission on hand-built records so codegen feasibility is proven first.
Plan above leads with (a). Confirm or propose a different opener?

### Q3 · First record-kind: Const first, Fn first, or Const→Fn sequenced?

Const proves the pipeline without body codegen; Fn proves rsc's hardest phase. Sequenced (Const week 1, Fn weeks 2-4) gives two end-to-end successes early.

### Q4 · Opus seed path: baked or asserted?

Walking skeleton needs *some* Opus to compile. Options:
(a) bake one seed Opus as an rkyv literal in criomed source;
(b) require the first Assert to construct it via nexus before Compile is callable.
(a) faster to first-loop success; (b) exercises the assert path on a more interesting record.

### Q5 · Sema bootstrap: in-memory or redb from day 1?

In-memory HashMap unblocks nexusd↔criomed wiring sooner; redb from day 1 is the real architecture but more scaffolding work before the first request lands.

### Q6 · Cascades at v0: minimal rule engine or defer entirely?

Rules are the reason sema is "evaluation, not storage." Skipping at v0 means manual re-asserts for derived records. Minimum viable rule engine = "match pattern; assert record." Include, or explicitly defer to post-first-loop?

### Q7 · criome-msg + lojix-msg crate timing

Report/030 Phase B leans "scaffold lojix-msg now as empty-commitment crate." Same question for criome-msg. Confirm scaffold both now (additive, breaks nothing), or defer both until the walking-skeleton first iteration needs them?

### Q8 · Mutate + Subscribe verbs at v0?

Walking skeleton works with just Assert/Query/Retract/Compile. Mutate = retract + assert (loses slot continuity). Subscribe = reconnect-to-query loop. Skip both at v0?

### Q9 · Expr record-kind scope for first-Const pipeline

Const depends on Expr. Options:
(a) implement the full Expr catalogue from report/004 before Const lands;
(b) minimal Expr (Literal atoms only) at v0, grow as Fn lands.
(b) is faster; (a) avoids piecemeal Expr growth.

### Q10 · Capability tokens at v0: skip, or minimal-shape from day 1?

Tokens are architectural but not critical-path for v0 correctness. Options:
(a) skip entirely — single hardcoded principal; lojixd accepts any caller;
(b) minimal token shape from day 1 so the signing surface exists and can later be filled with real keys + BLS quorum.

---

## 9 · Summary (one paragraph)

Today: ~6,700 LoC of real code across nota-serde-core, nexus-serde, horizon-rs, lojix (monolith), and CriomOS; ~930 LoC of skeleton-as-design in lojix-store and nexus-schema; three ~10-line stubs (nexusd, sema, rsc); four canon-missing components (criome-msg, lojix-msg, criomed, lojixd). Gap to walking skeleton: ~3,000–4,000 LoC. Critical path has 11 ordered tasks; five stubs for v0 collapse the work further. First record-kind choice is `Const` → `Fn` sequenced. Contracts at v0.0.1 are sketched in §5 and fit in one page each. The ten questions in §8 set the design surface Li's answers will narrow, and the first-loop work unblocks once Q1 (single-process v0), Q2 (order of attack), Q3 (first record-kind), and Q7 (contract-crate timing) are decided.

---

*End report 062.*
