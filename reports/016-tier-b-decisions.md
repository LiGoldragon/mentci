# 016 — Tier-B decisions pending before workspace bootstrap

*Claude Opus 4.7 / 2026-04-24 · companion to
[015 v4](015-architecture-landscape.md). Tier-A cheap wins are
landed this session (see §Tier-A at bottom); Tier-B items below
need Li input before they can be actioned.*

Report 015 v4 consolidated 20 open questions across 10 research
passes. This document lifts the ones that **block workspace
bootstrap** to the top, strips the rest to "note and defer,"
and makes each decision pointed enough to answer in a sentence.

---

## Gate A — timing & scope

### Q1 · Solstice target: summer or winter 2026?

Report 015 v4 §1, W6. Hard-constrains every phase plan below.

- **Summer (2026-06-21, ~58 days)**: MVP cuts corners aggressively;
  Phase-1 Tier-1 slips to Q3. `forged` compiles opera with the
  minimum-viable feature set, no determinism work, no streaming
  progress.
- **Winter (2026-12-21, ~8 months)**: Phase-1 can co-land with
  MVP; PatternExpr, Opus record, all three contract crates ship
  together; cargo determinism + compile progress fit.

**Recommendation**: needed.

---

## Gate B — workspace shape

These block actually creating the 6 new repos.

### Q2 · `Opus` record-kind shape in nexus-schema

§5 of 015 v4 proposes:

```rust
pub struct Opus {
    name: OpusName, version: SemVer, edition: RustEdition,
    toolchain: RustToolchain, root: ModuleId,
    deps: Vec<OpusDep>, emit: EmitKind, features: Vec<FeatureFlag>,
}
pub enum EmitKind {
    Binary { entry: ConstId }, Library, Both { entry: ConstId },
}
```

Assign kind byte `0x17`. **Needs: approve / amend / defer.**

### Q3 · Move `Bind` / `Mutate<T>` / `Negate<T>` into nexus-schema?

Currently in nexus-serde. criome-msg needs them rkyv-archived;
only nexus-schema is upstream of both consumers. **Recommendation**:
move in, re-export from nexus-serde for back-compat.

### Q4 · PatternExpr: schema-agnostic strings, or schema-resolved FieldRefs?

```rust
pub enum PatternExpr {
    Match { record: Record, binds: Vec<PatternAtom> },
    Optional(Box<PatternExpr>),
    Negate(Box<PatternExpr>),
    Constrain(Vec<PatternExpr>),
    Stream { body: Box<PatternExpr>, ops: Vec<QueryOp> },
}
```

- **Schema-agnostic (recommended)**: `Bind(String)` — resolve to
  field identity in criomed at match time.
- **Schema-bound**: `Deserialize` requires schema handle, binds
  carry resolved `FieldRef`s.

Schema-agnostic keeps the type serde-context-free; validation
shifts to criomed.

### Q5 · Capstone feature

- `nexus-cli list-opuses` — reads only. Previously recommended.
- **"Add a subcommand to nexus-cli via nexus edits, recompile,
  observe it working"** (recommended now) — exercises mutation
  path, provably closes the self-hosting loop.

---

## Gate C — protocol & topology

### Q6 · forged ↔ lojix-stored direct push, or via criomed?

A binary is 10–100 MB. Three options:

- **(a) Forged talks directly to lojix-stored** with a capability
  token issued by criomed. Large blobs skip criomed.
- (b) Forged returns bytes to criomed; criomed writes.
- (c) Forged returns a hash only; criomed fetches from forged's
  temp dir.

**Recommendation**: (a) — keeps the big-bytes path off criomed
while preserving criomed as the compile authoriser.

### Q7 · Launch ownership

Who exec's a compiled binary (matching `CriomeRequest::Launch`)?

- **criomed (recommended)**: has the client UDS, fetches bytes
  from lojix-stored, exec's.
- forged: already produced it — but stretches forged's role.
- separate `launcher` daemon: overkill.

### Q8 · Kind-byte registry home

Kinds span sema (`0x10..0x1F`) and lojix-stored (`0x20+`, `0xF0..`
if arbor ever returns). Registry should live in:

- a shared tiny `kind-bytes` crate (clean, one more repo)
- each owner declares its range in its own crate (simple, risks
  collision)
- a constant module in `lojix-store-msg` (pragmatic)

**Recommendation**: constants in `lojix-store-msg` for bytes the
store sees; mirror constants in sema's code for the record kinds
it owns. Cross-check with a CI test.

---

## Gate D — implementation-detail decisions (defer to Phase C)

These don't block scaffolding and can wait until implementation.

### Q9 · Cargo determinism (shared target-dir vs hermetic)

MVP: shared target-dir at `~/.cache/forged/target/`. Fast,
non-bit-deterministic. Hermetic option for later: sandboxed
`$HOME`, `SOURCE_DATE_EPOCH`, `codegen-units=1`.

### Q10 · Subscription durability across criomed restart

Lean: force re-subscribe on reconnect. Persistent subscription
table can land when multi-minute subscribers matter.

### Q11 · Compile progress streaming

Cold builds take minutes; one-shot `CompileReply` gives no
interim feedback. Defer `CompileEvent` streaming; ship MVP
one-shot.

### Q12 · PatternExpr custom `Deserialize`

serde's `untagged` enum dispatch has opaque errors. A hand-
written `Deserialize` (~300 LoC) gives real error quality.
**Recommendation**: hand-written, but not until PatternExpr has
consumers — defer to Phase C.

### Q13 · Large-blob chunking

`LojixStoreMessage::Put` caps at 16 MiB in MVP. `PutBegin/Chunk/
Commit/Abort` defined in the schema from day one; wire up when
forged produces a binary > 16 MiB.

### Q14 · Bootstrap loader home

`criomed/src/bin/bootstrap.rs` — criomed is the only process
with write access to sema, so parking the loader there is tidy.

### Q15 · rsc debug binary

Keep `rsc-dump` under `[[bin]] required-features = ["cli"]` for
human-readable projection dumps. Library is default.

### Q16 · Rustc toolchain pinning

MVP: accept `rustup` channel string; trust user's rustup install.
Hermetic rustc-in-lojix-store is post-MVP.

### Q17 · forged long-lived vs one-shot per request

Lean long-lived with per-request isolated target dirs. Keeps
cargo's incremental cache warm (big win for the edit-compile
self-hosting loop).

---

## Gate E — workspace-bootstrap execution plan

Assuming Gate A–C answers are in, the bootstrap sequence is §11
of 015 v4. Summary:

| Phase | Tasks | Cost |
|---|---|---|
| 0 | rename criome-store → lojix-store; mentci-next linkedRepos + AGENTS.md | ~1h |
| 1 | 3 contract-crate scaffolds (criome-msg, compile-msg, lojix-store-msg) | ~90 min parallel |
| 2 | 3 daemon scaffolds (criomed, forged, lojix-stored) | ~2h parallel |
| 3 | existing-repo updates (nexusd role, rsc → lib, sema confirm, nexus-schema Opus + PatternExpr + wrappers move) | ~1d total |
| 4 | implementation milestones (M2 → M6) | weeks |

### Q18 · Phase-0 execution

The rename + linkedRepos + AGENTS.md update can happen in one
session once Gate B is settled. Currently nothing blocks it.

### Q19 · Phase-1 contract-crate scaffolding

Each crate is `Cargo.toml` + `flake.nix` + `rust-toolchain.toml`
+ `src/lib.rs` stub (~20 LoC of types). Parallelizable; blocked
only by Q2–Q4.

### Q20 · Phase-2 daemon scaffolding

Blocked by Phase-1. Each daemon's `Cargo.toml` pins its own
contract crate as a git dep with `outputHashes` in flake.nix
(pattern matches nota-serde → nota-serde-core).

---

## Tier-A landed this session

Minimum list; everything here is committed.

| Item | Where | Lines | Status |
|---|---|---|---|
| Lexer `<\|\|` (`LAngleDouble`) + `\|\|>` (`RAngleDouble`) tokens | `nota-serde-core/src/lexer.rs` | ~15 | ✓ + 2 tests |
| Nota-serde-core full test pass | `nota-serde-core/tests/` | 127 tests | ✓ |
| Clippy warning | `nexus-serde/tests/nexus_wrappers.rs` | -6 | ✓ |
| anyhow → typed Error | `nexusd/src/` | +15 | ✓ |
| anyhow → typed Error | `nexus-cli/src/` | +15 | ✓ |
| Stale `aski` comment | `nexus-schema/src/domain.rs` | 2 | ✓ |
| Nexusd description update | `nexusd/Cargo.toml` | 1 | ✓ |

---

## Summary: what needs Li input

**Must answer before Phase-0 (workspace bootstrap)**:

- **Q1** — solstice date
- **Q2** — Opus record shape approval
- **Q3** — move wrappers into nexus-schema (lean yes)
- **Q5** — capstone feature (lean "add subcommand via edits")

**Must answer before Phase-1 (contract crates)**:

- **Q4** — PatternExpr schema-agnostic (lean yes)
- **Q6** — forged ↔ lojix-stored direct (lean yes)

**Must answer before Phase-2 (daemon scaffolds)**:

- **Q7** — Launch ownership (lean criomed)
- **Q8** — kind-byte registry (lean lojix-store-msg + sema mirror)

**Defer to implementation**:

- Q9–Q17 — shape-of-impl decisions; can change during Phase 3–4.

**Unblocked right now**:

- None until Q1 is answered (anything past Tier-A stops on
  schedule clarity).

---

## Reading-back checklist

1. Architecture in 015 v4 still correct?
2. Tier-A landed — any regressions I missed?
3. Q1 answer, which unfreezes Phase-0 planning.
4. Gate B answers (Q2–Q5), which unfreeze Phase-1 crate creation.

---

*End report 016 — 8 Tier-B questions gating workspace bootstrap;
9 Tier-D questions that defer to implementation; Tier-A landed.*
