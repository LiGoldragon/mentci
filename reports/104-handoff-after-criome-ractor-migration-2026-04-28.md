# 104 — Handoff after criome ractor migration

*Read this first. Then read §1's files in order, slowly. Then
re-read §§2-10 of this report. Then start work on the nexus
side of the ractor migration.*

This document is the post-context-reset entry point for the
session that follows. Reports/100 was deleted in this session
because it became stale; this file supersedes it.

The user is **Li**. The project is **criome**. The dev
environment is **mentci**. The conversation that produced
this report ran 2026-04-27 through 2026-04-28 and landed:

1. The full **M0 demo** working end-to-end (`(Node "User")` →
   `(Ok)`; `(\| Node @name \|)` → `[(Node "User")]`)
2. **Workspace-wide flake migration** to canonical crane+fenix
   (every CANON crate exposes `checks.default`; mentci
   aggregates 24+ flake checks)
3. **Three integration test suites** at the workspace level
   (`integration` monolithic; `scenario-chain` daemon-mode
   state-persistence; `roundtrip-chain` binary-stability
   per-daemon transformation)
4. **Daemon binaries renamed** to `nexus-daemon` /
   `criome-daemon` per the full-English `-daemon` suffix
   convention
5. **One-shot binaries** added: `nexus-parse`, `nexus-render`,
   `criome-handle-frame` (stdin/stdout filter wrappers around
   library code)
6. **Multi-agent style audit pass** (reports/101) — landed five
   small fixes; no deep abstraction issues
7. **Visual architecture report** (reports/102) — mermaid
   classDiagrams + sequenceDiagrams across the workspace
8. **Ractor migration design** (reports/103) — full design with
   7 open questions; all answered by Li 2026-04-28
9. **sema gained `reader_count` + `set_reader_count` API +
   `DEFAULT_READER_COUNT = 4`** — read-pool size is now
   configurable in the database itself (redb meta table)
10. **criome migration to ractor**: SHIPPED (5 actors —
    Daemon/Engine/Reader/Listener/Connection — replacing the
    sync Daemon noun + UDS Listener)
11. **Reports cleanup**: deleted 089, 091, 098, 099, 100;
    kept 074, 088, 101, 102, 103, 104

**The nexus ractor migration is the next work** — in-flight,
not started. See §8 for the precise plan.

---

## 1 · Required reading, in order — *Li wants you to read a lot*

Read **every** file listed below. Read in order. The order
matters — earlier files set up context for later ones. Spawn
an `Explore` agent for any file you can't fit in your own
context budget; the agent returns a compressed summary that
preserves structure. Don't skip files. Li explicitly asked for
thoroughness.

### 1a · Operational rules — load before doing anything

1. [`../AGENTS.md`](../AGENTS.md) — workspace operational
   rules. Substance-in-reports rule, jj workflow with
   blanket-auth always-push, naming rule with bad→good
   table + the "feels too verbose" anti-pattern,
   commit-message S-expression style, design-doc hygiene,
   no-version-history-in-designs, verify-each-parallel-tool
   -result, beauty-as-criterion section, **§Binary naming —
   `-daemon` suffix** and **§One-shot binaries —
   `<crate>-<verb>`** which are new in this session.
2. [`../repos/tools-documentation/AGENTS.md`](../repos/tools-documentation/AGENTS.md)
   — workspace-wide cross-project rules + pointers into
   per-tool docs.
3. [`../repos/tools-documentation/programming/beauty.md`](../repos/tools-documentation/programming/beauty.md)
   — **THE central principle.** Beauty is the criterion.
   **Read this twice.**
4. [`../repos/tools-documentation/programming/abstractions.md`](../repos/tools-documentation/programming/abstractions.md)
   — methods-on-types discipline.
5. [`../repos/tools-documentation/programming/beauty-research.md`](../repos/tools-documentation/programming/beauty-research.md)
   — deep philosophical case for beauty.
6. [`../repos/tools-documentation/programming/abstractions-research.md`](../repos/tools-documentation/programming/abstractions-research.md)
   — deep research for methods-on-types.
7. [`../repos/tools-documentation/programming/naming-research.md`](../repos/tools-documentation/programming/naming-research.md)
   — empirical case for full-English naming.
8. [`../repos/tools-documentation/rust/style.md`](../repos/tools-documentation/rust/style.md)
   — **Rust style guide.** §"Actors" specifies ractor as the
   project's actor framework with messages-typed,
   state-owned-not-shared, supervision-recursive discipline.
9. [`../repos/tools-documentation/rust/nix-packaging.md`](../repos/tools-documentation/rust/nix-packaging.md)
   — crane + fenix flake layout. Already executed across all
   7 CANON crates this session.
10. [`../repos/tools-documentation/nix/integration-tests.md`](../repos/tools-documentation/nix/integration-tests.md)
    — **NEW this session.** The chained-derivation pattern
    for nix integration tests. Documents the daemon-mode and
    binary-stability suites.
11. [`../repos/tools-documentation/jj/basic-usage.md`](../repos/tools-documentation/jj/basic-usage.md)
    — jj. **Use `jj commit` always, never `jj describe`** for
    normal commits.
12. [`../repos/tools-documentation/bd/basic-usage.md`](../repos/tools-documentation/bd/basic-usage.md)
    — beads (issue tracker).
13. [`../repos/tools-documentation/nix/basic-usage.md`](../repos/tools-documentation/nix/basic-usage.md)
    — nix. `nix run nixpkgs#<tool>`, never `cargo install` /
    `pip install` / `npm -g`.
14. [`../repos/tools-documentation/dolt/basic-usage.md`](../repos/tools-documentation/dolt/basic-usage.md)
    — dolt (the data store under bd).

### 1b · Project-wide architecture — load before designing

15. [`../repos/criome/ARCHITECTURE.md`](../repos/criome/ARCHITECTURE.md)
    — **THE canonical project doc.** Read in full. Specifically:
    - **§2 Invariants A / B / C / D** are load-bearing.
    - §1 macro philosophy — bootstrap-era allows authored
      macros.
    - §3 The request flow.
    - §10 Project-wide rules + the rejected-framings list.
16. [`../docs/workspace-manifest.md`](../docs/workspace-manifest.md)
    — repo inventory + statuses.
17. [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — workspace dev
    environment.

### 1c · Grammar — load before any code that touches text

18. [`../repos/nota/README.md`](../repos/nota/README.md) —
    base nota grammar.
19. [`../repos/nexus/spec/grammar.md`](../repos/nexus/spec/grammar.md)
    — full nexus grammar.
20. [`../repos/nexus/spec/examples/`](../repos/nexus/spec/examples/)
    — concrete `.nexus` files.

### 1d · The codec stack

21. [`../repos/nota-codec/ARCHITECTURE.md`](../repos/nota-codec/ARCHITECTURE.md)
22. [`../repos/nota-codec/src/lib.rs`](../repos/nota-codec/src/lib.rs)
23. [`../repos/nota-codec/src/error.rs`](../repos/nota-codec/src/error.rs)
    — **21 typed Error variants** (split into decoder errors
    + lexer errors); no `Custom(String)` arm.
24. [`../repos/nota-codec/src/lexer.rs`](../repos/nota-codec/src/lexer.rs)
    — pure tokenizer (525 LoC). `Token`, `Dialect`.
25. [`../repos/nota-codec/src/decoder.rs`](../repos/nota-codec/src/decoder.rs)
    — `Decoder<'input>` with the protocol methods derives call
    into. Pushback queue. `peek_token()` for top-level
    dispatchers.
26. [`../repos/nota-codec/src/encoder.rs`](../repos/nota-codec/src/encoder.rs)
    — `Encoder` with `needs_space` state.
27. [`../repos/nota-codec/src/traits.rs`](../repos/nota-codec/src/traits.rs)
    — `NotaEncode` + `NotaDecode` traits + blanket impls.
28. [`../repos/nota-codec/src/pattern_field.rs`](../repos/nota-codec/src/pattern_field.rs)
    — `PatternField<T>`.
29. [`../repos/nota-derive/ARCHITECTURE.md`](../repos/nota-derive/ARCHITECTURE.md)
30. [`../repos/nota-derive/src/lib.rs`](../repos/nota-derive/src/lib.rs)
    — six `#[proc_macro_derive]` entry points.
31. [`../repos/nota-derive/src/`](../repos/nota-derive/src/)
    — per-derive codegen files.

### 1e · Wire (signal)

32. [`../repos/signal/ARCHITECTURE.md`](../repos/signal/ARCHITECTURE.md)
33. [`../repos/signal/src/lib.rs`](../repos/signal/src/lib.rs)
34. [`../repos/signal/src/frame.rs`](../repos/signal/src/frame.rs)
    — `Frame`, `Body`, `FrameDecodeError`, `Frame::encode/decode`.
35. [`../repos/signal/src/handshake.rs`](../repos/signal/src/handshake.rs)
36. [`../repos/signal/src/auth.rs`](../repos/signal/src/auth.rs)
37. [`../repos/signal/src/request.rs`](../repos/signal/src/request.rs)
    — `Request` enum (rkyv-only at the wire; text dispatch
    happens in the nexus daemon's parser via sigil + delimiter
    routing).
38. [`../repos/signal/src/reply.rs`](../repos/signal/src/reply.rs)
    — `Reply` enum (rkyv-only; text rendering hand-written
    per-position in the nexus daemon's renderer).
39. [`../repos/signal/src/edit.rs`](../repos/signal/src/edit.rs)
    — `AssertOperation`, `MutateOperation`, `RetractOperation`,
    `AtomicBatch`, `BatchOperation` (last two rkyv-only).
40. [`../repos/signal/src/query.rs`](../repos/signal/src/query.rs)
    — `QueryOperation`.
41. [`../repos/signal/src/schema.rs`](../repos/signal/src/schema.rs)
    — `KindDecl`, `FieldDecl`, `Cardinality`.
42. [`../repos/signal/src/diagnostic.rs`](../repos/signal/src/diagnostic.rs)
    — `Diagnostic`, `DiagnosticLevel`, `DiagnosticSite`.
43. [`../repos/signal/src/slot.rs`](../repos/signal/src/slot.rs)
    — `Slot`, `Revision` (`NotaTransparent` u64 newtypes).
44. [`../repos/signal/src/flow.rs`](../repos/signal/src/flow.rs)
    — `Node`, `Edge`, `Graph`, `*Query`, `Ok`, `RelationKind`.

### 1f · Storage (sema)

45. [`../repos/sema/ARCHITECTURE.md`](../repos/sema/ARCHITECTURE.md)
46. [`../repos/sema/src/lib.rs`](../repos/sema/src/lib.rs) —
    `Sema::open / store / get / iter` + **NEW**
    `reader_count() / set_reader_count() / DEFAULT_READER_COUNT`.
    `Slot(u64)` private field.
47. [`../repos/sema/tests/sema.rs`](../repos/sema/tests/sema.rs)
    — 12 tests including reader_count persistence.

### 1g · The post-migration criome (DONE this session)

48. [`../repos/criome/ARCHITECTURE.md`](../repos/criome/ARCHITECTURE.md)
    — high-level. **NOTE: code map description still
    references the pre-migration shape.** Update needed; flagged
    in §10.
49. [`../repos/criome/src/lib.rs`](../repos/criome/src/lib.rs)
    — module structure. The supervision tree is documented in
    the doc comment.
50. [`../repos/criome/src/error.rs`](../repos/criome/src/error.rs)
    — `Error::ActorCall`, `Error::ActorSpawn` variants for the
    actor failure modes.
51. [`../repos/criome/src/daemon.rs`](../repos/criome/src/daemon.rs)
    — `Daemon` actor (root). Spawns `engine::Engine` +
    `reader::Reader` × N + `listener::Listener` linked. Has
    `Daemon::start()` for `main` to call.
52. [`../repos/criome/src/engine.rs`](../repos/criome/src/engine.rs)
    — `Engine` actor (writes + handshake + deferred verbs).
    `engine::State` is the noun with sync handler methods —
    `handle_frame` is the sync façade for `criome-handle-frame`
    + tests.
53. [`../repos/criome/src/reader.rs`](../repos/criome/src/reader.rs)
    — `Reader` actor (reads, query handler).
54. [`../repos/criome/src/listener.rs`](../repos/criome/src/listener.rs)
    — `Listener` actor (UDS accept loop; spawns Connection
    per accept).
55. [`../repos/criome/src/connection.rs`](../repos/criome/src/connection.rs)
    — `Connection` actor (per-client frame shuttle; decodes
    Frame and dispatches typed verb messages).
56. [`../repos/criome/src/kinds.rs`](../repos/criome/src/kinds.rs)
    — 1-byte kind discriminator scaffolding (M0; M1+ replaces
    with per-kind tables — bd `mentci-next-7tv`).
57. [`../repos/criome/src/main.rs`](../repos/criome/src/main.rs)
    — env config + `Daemon::start().await`.
58. [`../repos/criome/src/bin/handle_frame.rs`](../repos/criome/src/bin/handle_frame.rs)
    — uses `engine::State::handle_frame` directly (sync, no
    actor system).
59. [`../repos/criome/tests/engine.rs`](../repos/criome/tests/engine.rs)
    — 6 sync tests against `engine::State`.

### 1h · The pre-migration nexus (TARGET of migration)

60. [`../repos/nexus/ARCHITECTURE.md`](../repos/nexus/ARCHITECTURE.md)
61. [`../repos/nexus/src/lib.rs`](../repos/nexus/src/lib.rs)
62. [`../repos/nexus/src/error.rs`](../repos/nexus/src/error.rs)
63. [`../repos/nexus/src/daemon.rs`](../repos/nexus/src/daemon.rs)
    — pre-migration `Daemon` noun (config + bind + accept loop
    inline). **Becomes a ractor actor in the migration.**
64. [`../repos/nexus/src/connection.rs`](../repos/nexus/src/connection.rs)
    — pre-migration `Connection` noun (sync `shuttle()`
    method). **Becomes a ractor actor.**
65. [`../repos/nexus/src/criome_link.rs`](../repos/nexus/src/criome_link.rs)
    — `CriomeLink` (post-handshake signal connection).
    **Stays method-not-actor** per style.md §Actors carve-out.
66. [`../repos/nexus/src/parser.rs`](../repos/nexus/src/parser.rs)
    — `Parser` noun (text → `signal::Request`).
    **Stays a struct** (not an actor).
67. [`../repos/nexus/src/renderer.rs`](../repos/nexus/src/renderer.rs)
    — `Renderer` noun. **Stays a struct.**
68. [`../repos/nexus/src/main.rs`](../repos/nexus/src/main.rs)
    — pre-migration entry. Update to spawn the actor tree.
69. [`../repos/nexus/src/bin/parse.rs`](../repos/nexus/src/bin/parse.rs)
    — `nexus-parse` one-shot binary. **No change needed.**
70. [`../repos/nexus/src/bin/render.rs`](../repos/nexus/src/bin/render.rs)
    — `nexus-render` one-shot binary. **No change needed.**
71. [`../repos/nexus/tests/parser.rs`](../repos/nexus/tests/parser.rs)
    — 11 parser tests. **No change needed** (Parser stays a
    struct).
72. [`../repos/nexus/tests/renderer.rs`](../repos/nexus/tests/renderer.rs)
    — 9 renderer tests. **No change needed.**

### 1i · Other CANON repos

73. [`../repos/nexus-cli/ARCHITECTURE.md`](../repos/nexus-cli/ARCHITECTURE.md)
74. [`../repos/nexus-cli/src/`](../repos/nexus-cli/src/)
    — small text-shuttle client. No actor migration needed.

### 1j · Workspace integration tests

75. [`../checks/default.nix`](../checks/default.nix) — linkFarm
    of per-crate `checks.default`s. Auto-discovered by blueprint.
76. [`../checks/integration.nix`](../checks/integration.nix) —
    monolithic single-derivation shuttle.
77. [`../checks/scenario-assert-node.nix`](../checks/scenario-assert-node.nix),
    `scenario-query-nodes.nix`, `scenario-chain.nix` —
    daemon-mode chain (state.redb forwarded).
78. [`../checks/roundtrip-{assert,query}-{parse,handle,render}.nix`](../checks/) +
    `roundtrip-chain.nix` — binary-stability chain via
    one-shot binaries.
79. [`../lib/scenario.nix`](../lib/scenario.nix) — daemon-step
    builder; exposed as `flake.lib.scenario`.
80. [`../lib/default.nix`](../lib/default.nix) — exposes
    `flake.lib`.
81. [`../flake.nix`](../flake.nix) — workspace flake with 7
    CANON crate inputs each with nixpkgs+fenix+crane following.

### 1k · Active reports

82. [`074-portable-rkyv-discipline.md`](074-portable-rkyv-discipline.md)
    — pinned rkyv feature set. Cited from every Cargo.toml
    using rkyv.
83. [`088-closed-vs-open-schema-research.md`](088-closed-vs-open-schema-research.md)
    — research backing perfect-specificity (Invariant D).
84. [`101-style-audit-pass-2026-04-27.md`](101-style-audit-pass-2026-04-27.md)
    — multi-agent audit; zero deep findings; documents the 5
    fixes that landed.
85. [`102-visual-architecture-2026-04-27.md`](102-visual-architecture-2026-04-27.md)
    — mermaid diagrams across the workspace.
86. [`103-ractor-migration-design-2026-04-28.md`](103-ractor-migration-design-2026-04-28.md)
    — **THE design document for the ractor migration.** Read
    in full before continuing nexus migration. All 7 §8
    questions are answered (see §6 of this report).

### 1l · This report

87. **This file** — re-read after the others.

---

## 2 · Operational rules — the ones agents most often violate

Distilled from AGENTS.md + style.md + tools-documentation.

### 2a · Substance in reports, not chat

Anything that **explains, proposes, analyses, or summarises**
goes in a file under `../reports/` or in the right per-repo
documentation. The chat reply is one line pointing at it.

### 2b · Use `jj commit`, never `jj describe`

```
jj commit -m '<msg>'
jj bookmark set main -r @-
jj git push --bookmark main
```

**Always commit + push** after every meaningful change. Blanket
authorisation.

### 2c · Beauty is the criterion

When something feels ugly, slow down and find the structure
that makes it beautiful. The aesthetic discomfort *is* the
diagnostic reading.

### 2d · Full English words

`AssertOperation` not `AssertOp`. `nexus-daemon` not `nexusd`.
`Engine` not `Eng`. The "feels too verbose" objection is
training-data drift, not informed judgment.

### 2e · Methods on types, not free functions

Every reusable verb belongs to a noun. The bare-named-actor
pattern Li specified: actor file `engine.rs` contains an
`Engine` actor type (not `EngineActor`); the module gives the
qualifier.

### 2f · Wrapped field is private

`Slot(u64)` not `Slot(pub u64)`. Use `Slot::from(value)` and
`let value: u64 = slot.into()`.

### 2g · Tests live in separate files

Tests go in `tests/` at the crate root, not inline `#[cfg(test)]`.

### 2h · Verify each parallel-tool result

Scan every result block before any follow-up step. The bundle
returning is not the bundle succeeding.

### 2i · Authored macros — bootstrap-era policy

Per criome/ARCHITECTURE.md macro section: in the eventual
self-hosting state, sema-rules + rsc-projection replace
authored macros. **In the current bootstrap era, authored
macros are fine.**

### 2j · Design-doc hygiene — state criteria positively

Don't accumulate "do not use X" patterns. Excluded options are
silently omitted. The rejected-framings list in
[criome/ARCHITECTURE.md §10](../repos/criome/ARCHITECTURE.md)
is the *only* place wrong frames are named.

### 2k · Commit message style

Single-line nested-paren S-expression. First token = repo
name. `[...]` enumerates discrete bullets. `—` introduces
rationale. `((double parens))` mark direct quotes from Li.

### 2l · Reports are ephemeral

Per Li 2026-04-28: *"reports are ephemerous … if the points
are already made somewhere then only move data that is
extremely beneficial in terms of clarifying something."*
Default to deletion when a report's content is captured in
source/docs.

---

## 3 · Project shape in one paragraph

**Criome** is a typed binary record-graph engine. **Sema** is
its heart — content-addressed records, native binary form
(rkyv-encoded). Records reference each other by `Slot`.
**Signal** is the wire form (per-verb typed enums; no generic
record wrapper). **Nexus** is the text bridge — a daemon
parsing text into signal frames for criome, rendering signal
replies back to text. **Nota-codec** is the typed text codec
shared by both nota and nexus dialects; **nota-derive** is the
proc-macro pair providing six derives. **Nexus-cli** is a thin
text shuttle client. **Lojix** + family handle build/store/
deploy (M2+). The project is **self-hosting**: criome compiles
its own source from records in sema, via rsc + nix.

---

## 4 · Code state (what's working today)

| Crate | LoC | State | Tests |
|---|---|---|---|
| `nota-codec` | ~1500 | Production-ready for M0 verb scope. 21 typed Error variants. | 79 |
| `nota-derive` | ~600 | Six derives shipping. | 0 (compile-only) |
| `signal` | ~1100 | All `…Operation` types; `Slot`/`Revision`/`BlsG1` private fields. | 42 |
| `sema` | ~180 | M0 store + new `reader_count()`/`set_reader_count()` + `DEFAULT_READER_COUNT = 4`. | 12 |
| `nexus` | ~400 | Pre-migration: 5 nouns (`Daemon`/`Connection`/`CriomeLink`/`Parser`/`Renderer`) + 2 one-shot bins (`nexus-parse`/`nexus-render`). Daemon binary `nexus-daemon`. **Migration to ractor not started.** | 20 |
| `criome` | ~600 | **Post-migration: 5 actors** (`Daemon`/`Engine`/`Reader`/`Listener`/`Connection`) per the ractor design + 1 one-shot bin (`criome-handle-frame`). Daemon binary `criome-daemon`. `engine::State` carries the sync façade for the binary + tests. | 6 |
| `nexus-cli` | ~50 | `Client::shuttle()` sync byte shuttle. Binary `nexus`. | 0 |

All crates `cargo check` clean. Total ~4400+ LoC, **159+ unit
tests + 4 integration suites.**

**Workspace-level checks** (in mentci): `nix flake check` runs
all 7 per-crate `checks.default` plus the workspace-level
integration suites (`integration` monolithic; `scenario-chain`
daemon-mode state-persistence; `roundtrip-chain` binary-stability
per-daemon transformation). 24 flake checks total.

**Repos out of M0 scope** (M2+): lojix, lojix-cli, lojix-store,
rsc (TRANSITIONAL), horizon-rs, CriomOS cluster.

---

## 5 · The criome ractor migration — what shipped

Per Li 2026-04-28: *"I do want to use ractor. The timing is
right now."*

**5 actor types in criome**, all with bare-named module-derived
patterns (file `engine.rs` contains `Engine` actor type, not
`EngineActor`; the module path provides the qualifier):

```
Daemon (root)                  src/daemon.rs
  ├── Engine                   src/engine.rs   (writes + handshake + deferred verbs)
  ├── Reader × N               src/reader.rs   (reads, concurrent via redb MVCC)
  └── Listener                 src/listener.rs (UDS accept loop)
        ├── Connection × M     src/connection.rs (one per accepted UDS client)
        └── ...
```

**Per-actor file structure** — each actor file exports:
- `pub struct <Actor>;` — zero-sized actor marker
- `pub struct State { ... }` — the actual state with sync
  handler methods
- `pub struct Arguments { ... }` — what `pre_start` consumes
- `pub enum Message { ... }` — typed per-verb messages with
  `RpcReplyPort<T>` variants

**`engine::State` carries the sync façade** —
`engine::State::handle_frame(frame: Frame) -> Frame`
dispatches all verbs without going through ractor. The
`criome-handle-frame` one-shot binary uses this directly; so do
the 6 integration tests in `tests/engine.rs`. This keeps
per-verb handlers sync-testable and lets the binary stay
non-tokio.

**Reader pool size** — sema's `reader_count()` reads from the
redb meta table; default `DEFAULT_READER_COUNT = 4` if unset.
`Daemon::pre_start` spawns that many `Reader` actors at
startup, all sharing `Arc<Sema>` and answering queries
concurrently via redb's MVCC.

**Read-pool round-robin** — `Listener` holds an
`Arc<AtomicUsize>` cursor that's cloned into each spawned
`Connection`. Each query picks reader[cursor % readers.len()]
and bumps the cursor.

**Perfect-specificity at every boundary** — `Connection`
decodes the `Frame` and dispatches typed verb messages
(`Handshake { request, reply_port }` /
`Assert { operation, reply_port }` /
`Query { operation, reply_port }` /
`DeferredVerb { verb, milestone, reply_port }`) — no god
`HandleFrame` message. `RpcReplyPort<T>` is per-verb-typed.

**Cargo.toml requires the `async-trait` feature** —
`ractor = { version = "0.15", features = ["async-trait"] }`
to expose `ractor::async_trait` (it's gated behind a feature
flag in 0.15). The actor impl signatures use `#[ractor::async_trait]`.

**`std::result::Result<T, ActorProcessingErr>` is required
in actor-impl signatures** — the crate's `Result<T>` alias
shadows `Result` and is single-parameter, so writing
`Result<Self::State, ActorProcessingErr>` errors with "type
alias takes 1 generic argument but 2 supplied." Use the
fully-qualified `std::result::Result<...>` in the actor `impl`
blocks only.

**`ractor::rpc::CallResult<T>` unwrapping** — use a free
`call_into(result, label)` function in `connection.rs` (NOT a
trait extension method, which collides with method-resolution
ambiguity). Maps `Success(T)` to `Ok(T)`, `Timeout` /
`SenderError` to labelled `Error::ActorCall`.

**Files deleted** in the migration: `assert.rs`, `query.rs`,
`handshake.rs`, `dispatch.rs`, `uds.rs` — per-verb handlers
moved onto `engine::State` / `reader::State`; UDS accept logic
moved to `listener::Listener`; per-frame dispatch moved to
`connection::Connection`.

**Tests file renamed**: `tests/dispatch.rs` →
`tests/engine.rs`. The 6 sync tests use `engine::State`
directly.

**Commit hash on origin/main**: `f6e1102102bb` (the criome
rewrite commit). The ARCHITECTURE.md doc map description still
references the pre-migration shape — update flagged in §10.

---

## 6 · Settled architecture decisions — DO NOT relitigate

| Decision | Where it lives |
|---|---|
| Per-verb typed payloads (no generic wrapper enum) | criome/ARCH §2 Invariant D + signal/ARCH + 088 |
| `PatternField::Bind` carries no payload | nexus/spec/grammar.md §"The strict rule" + nota-codec/src/pattern_field.rs |
| `Slot` / `Revision` / `BlsG1` private fields with `From` traits | rust/style.md + signal/src/slot.rs |
| Bind names MUST equal schema field name at the position | nexus/spec/grammar.md §Binds + nota-derive `NexusPattern` codegen |
| Nexus daemon is in the path (CLI never speaks signal directly) | signal/ARCH + nexus/ARCH + nexus-cli/ARCH |
| Schema-as-data via `KindDecl`; closed Rust enum is rsc's projection | criome/ARCH §2 Invariant D + signal/src/schema.rs |
| Replies pair to requests by FIFO position; no correlation IDs | signal/ARCH + nexus/spec/grammar.md |
| All-rkyv discipline with pinned feature set | 074 + every Cargo.toml |
| nota-codec + nota-derive replace serde for nota+nexus text | nota-codec/ARCH + nota-derive/ARCH |
| `Option<T>` encoder always emits explicit `None`; decoder accepts both | nota-codec/src/traits.rs |
| `Decoder::read_string` accepts both quoted and bare-ident input | nota-codec/src/decoder.rs |
| `AtomicBatch` + `BatchOperation` are rkyv-only for M0 | signal/src/edit.rs |
| `Reply` / `Frame` / `Body` / `Request` / `Handshake*` / `Diagnostic` family rkyv-only | signal/src/{reply,frame,handshake,diagnostic}.rs |
| `BTreeMap` / `HashMap` wire form: `[(Entry key value) ...]` sorted | nota-codec/src/traits.rs |
| Tuples wire form: `(Tuple a b ...)` with explicit `Tuple` head | nota-codec/src/traits.rs |
| **Methods on types — criome restructured around actors with `<noun>::State` pattern** | criome/src/{daemon,engine,reader,listener,connection}.rs |
| Bootstrap-era allows authored macros | criome/ARCH §"Macro philosophy" |
| Beauty is the criterion | programming/beauty.md |
| **Daemon binaries carry the `-daemon` suffix** | AGENTS.md §"Binary naming" |
| **One-shot binaries** `<crate>-<verb>` | AGENTS.md §"One-shot binaries" |
| Workspace-wide flake migration to canonical crane+fenix | nix-packaging.md |
| Three integration suites: `integration` + `scenario-chain` + `roundtrip-chain` | mentci/checks/ |
| **ractor is the project's actor framework** with `<noun>` actor + `State` + `Arguments` + `Message` per file; per-verb typed messages with `RpcReplyPort` | rust/style.md §Actors + criome/src/{daemon,engine,reader,listener,connection}.rs |
| **sema reader_count config in redb meta table** (M2+ moves to typed `CriomeInstance` record) | sema/src/lib.rs |

### Li's answers to reports/103 §8 (durable):

1. **Read-pool now**, not deferred. Plus interest in
   multi-database / multi-version backends as a future
   direction (M2+).
2. **Per-crate Connection actors**, no shared abstraction —
   the inner shapes diverge once nexus gets subscriptions.
3. **Daemon IS an actor**, root of the supervision tree (not
   a thin façade). Already implemented for criome.
4. **Log-and-forget on connection panic**.
5. **One file per actor**, bare-named with module-derived
   qualifier (file `engine.rs` → `Engine` type, not
   `EngineActor`). State struct is `<module>::State`.
6. **Migration NOW**, before lojix-daemon arrives so lojix is
   born actor-shaped.
7. **Verb-specific messages** (perfect-specificity), not god
   `HandleFrame`. Confirmed.

---

## 7 · The lurking dangers — what trips agents

1. **`…Op` type names are deleted.** Use `AssertOperation` etc.
2. **`nexus/src/parse.rs` is deleted.** No more `QueryParser`.
3. **Slot has a private field.** Use `Slot::from(value)`.
4. **`Option<T>` wire form: explicit `None` always emitted;
   decoder accepts both.**
5. **`Decoder::read_string` accepts bare idents.**
6. **`peek_record_head` accepts both `(` and `(|`.**
7. **rkyv bytecheck doesn't catch type-punning.** criome
   prepends a 1-byte kind discriminator — M1+ replaces this
   with per-kind redb tables (bd `mentci-next-7tv`).
8. **`AtomicBatch` + `BatchOperation` are rkyv-only.**
9. **Authored macros are allowed in the bootstrap era.**
10. **`AtomicBatch.ops` is now `.operations`.**
    `ValidateOperation.op` is `.operation`.
    `DiagnosticSite::OpInBatch` is `OperationInBatch`.
    `AuthProof::BlsSig` is `BlsSignature`.
11. **Daemon binaries carry `-daemon` suffix.** CLI binary in
    nexus-cli is `nexus`.
12. **NEW — ractor 0.15 requires `features = ["async-trait"]`**
    in Cargo.toml to expose `#[ractor::async_trait]`. Without
    this feature, the macro path doesn't resolve.
13. **NEW — actor impl signatures need
    `std::result::Result<T, ActorProcessingErr>`** explicitly.
    The crate's `Result<T>` alias shadows and is
    single-parameter.
14. **NEW — `ractor::rpc::CallResult<T>`** is unwrapped via a
    free function (e.g. `call_into(result, label)`); a trait
    extension method shadows method resolution and produces
    confusing closure-type errors.
15. **NEW — engine has both an actor (`engine::Engine`) AND a
    sync state (`engine::State`).** The actor wraps the state
    for async use; sync use (one-shot binary, tests)
    constructs `engine::State::new(sema)` directly.
16. **NEW — reader pool size lives in sema's redb meta
    table** via `sema::Sema::reader_count()`. Default 4 if
    unset. `set_reader_count(n)` persists.
17. **NEW — actor State doesn't drop when handle returns.**
    Ractor wraps the exiting actor's State in a `BoxedState` and
    queues it to the supervisor as part of `SupervisionEvent::ActorTerminated`
    ([actor.rs:762](https://github.com/slawlor/ractor/blob/v0.15.6/ractor/src/actor.rs#L762)).
    The State doesn't drop until the supervisor's mailbox processes
    that event — which can be much later if the supervisor is
    inside an active `await` (mailbox priority only matters
    *between* iterations, not within an active handle). If the
    State holds a resource that something *external* is waiting
    on (a UnixStream the client is reading), close it explicitly
    inside `handle` before `myself.stop()`. Relying on `Drop`
    after the actor exits is wrong. Discovered via the nexus
    deadlock; see [reports/105 §10](105-nexus-ractor-migration-deep-review-2026-04-28.md).

---

## 8 · The nexus ractor migration — NEXT WORK

**Status:** NOT STARTED. The pre-migration nexus daemon body
(file 60-72 in §1h) is still in place. This section is the
plan.

### 8a · Goal

Mirror the criome migration: convert nexus's `Daemon` noun and
`Connection` noun into ractor actors. `CriomeLink` stays a
method-not-actor per style.md §Actors carve-out (single owner,
no concurrent shared state). `Parser` and `Renderer` stay as
plain structs (they're stateless transformers, not concurrent
components).

### 8b · Final shape

```
Daemon (root)         src/daemon.rs    spawns Listener; holds config
  └── Listener        src/listener.rs  UDS accept; spawns Connection per accept
        ├── Connection × M             src/connection.rs   per-client text shuttle
        └── ...
```

**Connection's job (M0)**: read text-to-EOF from client →
parse with `Parser` → for each Request, open a `CriomeLink`
(handshake) and forward → render Reply via `Renderer` →
write accumulated text → stop.

The shuttle is sequential (parse all, forward all, render
all, write all) so it could be a single-message lifecycle
in M0: `pre_start` → cast `Run` → handle `Run` does
everything → `myself.stop()`. This keeps the M0 simple while
matching the actor pattern. M2+ subscriptions expand this.

### 8c · Files to write

Each file follows the same pattern as criome (study
`criome/src/daemon.rs` etc. in §1g first):

- **`src/daemon.rs`** (REPLACE) — `Daemon` actor (root). Spawns
  `Listener` linked. Holds `socket_path`, `criome_socket_path`
  config. `pre_start` does the spawning. No user messages.
- **`src/listener.rs`** (NEW) — `Listener` actor. UDS
  `bind` → `accept` loop via self-cast `Accept`. Per accept,
  spawns `Connection` linked. Holds `criome_socket_path` to
  pass into each Connection.
- **`src/connection.rs`** (REPLACE) — `Connection` actor.
  `pre_start` casts `Run`. Handle `Run`: read text-to-EOF,
  parse loop, open `CriomeLink`, forward each request, render
  each reply, write back, stop self.

### 8d · Files to keep

- **`src/criome_link.rs`** — keep as struct, no actor. Used
  inline by Connection.
- **`src/parser.rs`** — keep as struct.
- **`src/renderer.rs`** — keep as struct.
- **`src/error.rs`** — extend with `ActorCall(String)` and
  `ActorSpawn(String)` variants like criome.
- **`src/main.rs`** — UPDATE to call `Daemon::start(args)` and
  await the join handle (mirror `criome/src/main.rs`).
- **`src/lib.rs`** — UPDATE re-exports to add `listener` module
  and remove `daemon::Connection` re-export (Connection is
  internal to the actor system now).
- **`src/bin/parse.rs`** + **`src/bin/render.rs`** — no change.

### 8e · Tests

- **`tests/parser.rs`** — no change (Parser stays a struct).
- **`tests/renderer.rs`** — no change.
- The `mentci/checks/integration.nix` test exercises nexus
  via UDS through nexus-cli — it doesn't change either, but
  is the load-bearing regression gate. **Verify it passes
  after the migration.**

### 8f · Cargo.toml additions

```toml
ractor = { version = "0.15", features = ["async-trait"] }
```

(Note the **`features = ["async-trait"]`** part — without it
`#[ractor::async_trait]` doesn't resolve.)

### 8g · Step-by-step

1. Read `criome/src/{daemon,engine,reader,listener,connection}.rs`
   in full to internalise the actor patterns.
2. Add ractor (with async-trait feature) to nexus/Cargo.toml.
3. Write `nexus/src/listener.rs` (mirror criome's structure).
4. Rewrite `nexus/src/daemon.rs` as a ractor actor.
5. Rewrite `nexus/src/connection.rs` as a ractor actor (the
   shuttle logic from the existing `Connection::shuttle`
   moves into the `handle(Run)` body).
6. Extend `nexus/src/error.rs` with `ActorCall` / `ActorSpawn`.
7. Update `nexus/src/main.rs` to spawn Daemon.
8. Update `nexus/src/lib.rs` re-exports.
9. `cargo build` — fix any ractor-related issues using the
   §7 lurking-dangers checklist (async-trait feature flag;
   `std::result::Result` explicit; `ractor::async_trait`
   path; `call_into` not trait method).
10. `cargo test` — the 20 parser+renderer tests should still
    pass.
11. Bump mentci's flake.lock for nexus + criome:
    `cd mentci && nix flake update nexus criome`.
12. `nix flake check` from mentci — confirm all 24+ checks
    pass, ESPECIALLY `integration` (the end-to-end UDS
    shuttle).
13. Smoke-test manually: start `criome-daemon` + `nexus-daemon`
    in two terminals, pipe `(Node "User")` through
    `nexus-cli`, expect `(Ok)`. Then query, expect
    `[(Node "User")]`.
14. Commit per repo with the standard S-expression style.
15. Update `nexus/ARCHITECTURE.md` code map for the new
    structure.
16. Update `criome/ARCHITECTURE.md` code map (still
    pre-migration; flag fix in this PR or as follow-up).

---

## 9 · Other open work (lower priority)

### 9a · M0 step 7 — genesis.nexus

bd `mentci-next-???` (ID created earlier this session).
Concrete: write `criome/genesis.nexus` with the bootstrap
KindDecls + glue in `criome/src/main.rs` that dispatches them
on first boot (empty sema). **Not blocking the demo** —
Node/Edge/Graph kinds are built into criome's M0 body.

### 9b · M1 — per-kind sema tables

bd `mentci-next-7tv`. Replaces the 1-byte kind discriminator
in `criome/src/kinds.rs` with per-kind redb tables. Unblocks
schema-based query optimisation, cleaner cross-version
compat, and the bootstrap loop.

### 9c · Encoder bare-ident emission for strings

The nota README §"Strings" says canonical form emits bare
when eligible. Encoder currently always quotes. Cosmetic;
defer.

### 9d · Open bd issues

Run `bd list --status=open` to see current state. Includes:
- `mentci-next-ef3` — self-hosting "done" moment
- `mentci-next-0tj` — rsc records-to-Rust projection
- `mentci-next-4jd` — M2-remainder method-body layer
- `mentci-next-7dj` — cross-repo flake input pattern
- `mentci-next-8ba` — M3 sema redb wrapper
- `mentci-next-rgs` — M4 nexus-daemon ractor migration
  (mostly satisfied by this work; close after nexus
  migration lands)
- `mentci-next-zv3` — M6 bootstrap demo
- `mentci-next-dqp` — rename rsc to a full English word
- `mentci-next-7tv` — M1 per-kind sema tables (this session)
- `mentci-next-???` — genesis.nexus (this session)

---

## 10 · Tooling cheat sheet

```
# session-close protocol — every meaningful change
jj commit -m '<msg>'      # NEVER jj describe
jj bookmark set main -r @-
jj git push --bookmark main

# session start (via bd hook)
bd ready                  # what's available to work on
bd memories <keyword>     # search project-scoped memories

# building / testing
cd <repo> && cargo test                   # quick
cd <repo> && nix flake check              # canonical (sandboxed)

# workspace-level — runs all 7 crate checks + 4 integration suites
cd mentci && nix flake check

# missing tool? never install
nix run nixpkgs#<pkg> -- <args>

# bd — short tracked items only; long-form goes in files
bd create --title="X" --description="Y" --type=task --priority=2
bd update <id> --claim
bd close <id>

# bumping mentci's nexus or criome inputs
cd mentci && nix flake update nexus criome
```

---

## 11 · The shape of a successful turn

A turn that goes well:

1. Read the user's message; understand the actual ask.
2. If non-trivial: read the relevant docs first.
3. Edit code or docs.
4. Run tests if code touched. **Check that pre-existing
   tests still pass.**
5. `jj commit + bookmark + push` per repo, per logical chunk.
6. Brief chat reply pointing at the change. If the change
   warrants explanation, the explanation lives in a doc.

A turn that goes badly: long chat replies, `jj describe`,
cryptic identifiers, propagating local dialect, half-finished
implementations sitting uncommitted, referencing deleted
artifacts.

---

## 12 · Reading priority if short on time

If you only have time to read FIVE files before starting work,
read these:

1. [`programming/beauty.md`](../repos/tools-documentation/programming/beauty.md)
2. [`programming/abstractions.md`](../repos/tools-documentation/programming/abstractions.md)
3. [`../AGENTS.md`](../AGENTS.md) — operational rules
4. [`../repos/criome/ARCHITECTURE.md`](../repos/criome/ARCHITECTURE.md) §2 Invariants A-D
5. **This report (104), at minimum §§5, 6, 7, 8** (the ractor
   migration state + nexus-side plan)

Then start. Then read more as you need it (per §1).

But Li explicitly asked for thoroughness. **Read all of §1
when you have the budget**, especially §1g (the post-migration
criome) and §1h (the pre-migration nexus that's about to be
migrated).

---

*End 104.*
