# 085 — Session handoff (2026-04-26)

*Compaction-resilient checkpoint of where the design and code stand at the end of a long working session. A fresh agent can use this report plus the per-repo `ARCHITECTURE.md` files to pick up where things left off without needing the conversation transcript.*

---

## Project at a glance

**Criome** — a typed binary record-graph engine. Sema (the records) is by definition computer-cognizable: the bytes ARE the meaning. Criome is sema's engine; it validates incoming changes and stores them. Nexus is a text dialect humans (and LLMs) use to author records; the nexus daemon translates text to / from signal (the rkyv binary form criome speaks). Lojix is the build/store side (post-MVP for the engine itself).

**Repo layout** — see `mentci/docs/workspace-manifest.md` for the canonical list. Sema-ecosystem repos live as siblings under `~/git/<repo>/` and are exposed inside mentci via symlinks at `mentci/repos/<repo>/` (created by devshell entry) plus a multi-root VSCode workspace at `mentci/mentci.code-workspace`. Submodules were tried and reverted on 2026-04-25 (see commit `fd9f9c9`).

---

## Settled design (everything load-bearing is in `ARCHITECTURE.md`)

For the durable canon, read these in order:

1. [`criome/ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) — project-wide. **§2 Invariants A/B/C** are load-bearing.
2. [`nota/README.md`](https://github.com/LiGoldragon/nota/blob/main/README.md) — base text data format.
3. [`nexus/spec/grammar.md`](https://github.com/LiGoldragon/nexus/blob/main/spec/grammar.md) — locked v3 nexus grammar.
4. [`nexus/spec/examples/`](https://github.com/LiGoldragon/nexus/tree/main/spec/examples/) — `flow-graph.nexus` + `patterns-and-edits.nexus`.
5. [`signal/ARCHITECTURE.md`](https://github.com/LiGoldragon/signal/blob/main/ARCHITECTURE.md) — wire format + IR + flow-graph kinds.
6. [`nexus/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus/blob/main/ARCHITECTURE.md) — text-translator daemon.
7. [`sema/ARCHITECTURE.md`](https://github.com/LiGoldragon/sema/blob/main/ARCHITECTURE.md) — the records DB.
8. [`sema/reference/Vision.md`](https://github.com/LiGoldragon/sema/blob/main/reference/Vision.md) — plain-language aspirational doc.

### Principles distilled during this session (all in arch docs above)

| # | Principle | Where it lives |
|---|---|---|
| 1 | **Sema is binary by definition.** Records' bytes ARE their meaning. Signal is sema's *native form* on the wire — not a perf optimization. | `signal/ARCHITECTURE.md` opening, `criome/ARCHITECTURE.md` Invariant B |
| 2 | **Records are self-contained.** Identity, kind, data — all in the record's bytes. No envelope-level record metadata. | `criome/ARCHITECTURE.md` Invariant B |
| 3 | **Criome speaks signal only.** Text never crosses criome's boundary. Failure modes involving text streams live at the nexus daemon. | `criome/ARCHITECTURE.md` Invariant B |
| 4 | **Nexus is a request language, not a programming language.** No variables, scoping, evaluation, or cross-request state. Pattern binds (`@x`) are matching-only. Dependent edits are client-side orchestration. | `criome/ARCHITECTURE.md` Invariant B |
| 5 | **Comments carry no load-bearing data.** Information has a typed home in the schema. | `nota/README.md` ## Sigils |
| 6 | **"No keywords" applies to the parser, not the schema.** No reserved words like `SELECT` / `IF`; the verb system is sigil × delimiter composition. **Schema-level typed enums are encouraged** — `RelationKind { DependsOn, Contains, … }`, `OutcomeMessage { Ok, Diagnostic }` etc. | `nota/README.md`, `signal/ARCHITECTURE.md` ## Schema discipline, `criome/ARCHITECTURE.md` Invariant B |
| 7 | **Slots are user-facing identity, hashes are version-locking.** Records reference each other by `Slot` (mutable identity). Hashes exist for cases where the client wants to lock onto a specific version. `Edge.from: Slot` is correct. | `criome/ARCHITECTURE.md` Invariant B (last paragraph) |
| 8 | **Replies are typed messages.** `(Ok)` (success record kind in `signal::flow`) and `(Diagnostic …)` (existing kind). Multi-element edits return sequences `[(Ok) (Ok) (Diagnostic …) ...]`. Queries return record sequences. | `nexus/spec/grammar.md` ## Reply semantics |
| 9 | **Positional pairing, no correlation IDs.** N-th reply pairs with N-th request. Strict FIFO per connection. | `signal/ARCHITECTURE.md`, `nexus/spec/grammar.md` |
| 10 | **One subscription per connection.** Subscriptions stream events from now-onward (no initial snapshot — issue a Query first if needed). Close the socket to end. | `nexus/spec/grammar.md` ## Subscriptions |
| 11 | **Message-as-record.** Every reply is a delimited record. Even success — `(Ok)`, not bare `Ok`. | `nexus/spec/grammar.md` ## Reply semantics |
| 12 | **Three-family delimiter algebra.** `( )` / `(\| \|)`, `[ ]` / `[\| \|]`, `{ }` / `{\| \|}`. Strings via `" "` / `""" """`. `< >` reserved for comparison operators (deferred). 7 sigils. | `nexus/spec/grammar.md`, `nota/README.md` |
| 13 | **No version-history narration in design/vision docs.** Architecture describes what IS; history goes in commit messages and reports. | feedback memory |
| 14 | **ARCHITECTURE.md durable, reports non-durable.** Load-bearing claims in arch docs. | feedback memory |
| 15 | **Vision docs use plain language.** No library names, file paths, milestone IDs in body. | `sema/reference/Vision.md` is the worked example |

---

## Schema state (signal)

```
signal/src/
├── flow.rs
│     pub struct Node    { name: String }                        // identity is the slot
│     pub struct Edge    { from: Slot, to: Slot, kind: RelationKind }
│     pub struct Graph   { title: String, nodes: Vec<Slot>,
│                          edges: Vec<Slot>, subgraphs: Vec<Slot> }
│     pub struct Ok      {}                                       // success message kind
│     pub enum   RelationKind {
│         Flow, DependsOn, Contains, References, Produces,
│         Consumes, Calls, Implements, IsA,
│     }
│     pub const  KNOWN_KINDS: &[&str] = &["Node", "Edge", "Graph"];
│
├── edit.rs
│     pub struct AssertOp     { record: RawRecord }
│     pub struct MutateOp     { slot: Slot, new_record: RawRecord, expected_rev: Option<Revision> }
│     pub struct RetractOp    { slot: Slot, expected_rev: Option<Revision> }
│     pub struct AtomicBatch  { ops: Vec<BatchOp> }
│     pub enum   BatchOp      { Assert, Mutate, Retract }
│
├── request.rs
│     pub enum Request {
│         Handshake, Assert, Mutate, Retract, AtomicBatch,
│         Query, Subscribe, Validate
│     }
│
├── reply.rs
│     pub enum Reply {
│         HandshakeAccepted, HandshakeRejected,
│         Outcome(OutcomeMessage),     // single-element edit
│         Outcomes(Vec<OutcomeMessage>), // multi-element edit
│         Records(Vec<RawRecord>),     // query
│     }
│     pub enum OutcomeMessage { Ok(Ok), Diagnostic(Diagnostic) }
│
├── frame.rs   — Frame { principal_hint, auth_proof, body }   // NO correlation_id
├── handshake.rs — ProtocolVersion 0.1.0
├── auth.rs   — AuthProof (SingleOperator MVP)
├── value.rs  — RawRecord, RawValue, RawLiteral, FieldPath
├── pattern.rs — RawPattern, FieldConstraint
├── query.rs  — Selection, RawOp, RawProjection
├── diagnostic.rs — Diagnostic, DiagnosticLevel, DiagnosticSite
├── slot.rs   — Slot, Revision (u64 newtypes)
└── hash.rs   — Hash ([u8; 32], Blake3)
```

`cargo check` + `cargo test` green (3 round-trip tests on Frame).

---

## Code state (other repos)

| Repo | Status | Notes |
|---|---|---|
| nota | spec only | README locked at v3 grammar |
| nota-serde-core | implementation complete | full delimiter rewrite landed; 158 tests pass under `nix flake check` |
| nota-serde | façade | 8 tests + 1 doctest pass |
| nexus-serde | façade + 6 wrappers | `Bind` / `Mutate` / `Negate` / `Validate` / `Subscribe` / `AtomicBatch`; 30 tests + 1 doctest pass |
| nexus | spec + daemon stub | grammar.md locked; src/main.rs is `Ok(())`; client_msg/ deleted |
| nexus-cli | scaffold | src/main.rs todo!() — M0 client to be written |
| criome | scaffold | validator pipeline modules exist as `todo!()` stubs; src/main.rs todo!() — M0 daemon to be written |
| sema | scaffold | src/lib.rs is just a docstring — needs the redb-backed record graph implementation |
| lojix / lojix-cli / lojix-store / lojix-schema | scaffolds | M2+ work |
| rsc | wiped | stub for "records-to-source projector" |
| horizon-rs | stable | unrelated to engine MVP |
| CriomOS / CriomOS-emacs / CriomOS-home | host OS | unrelated to engine MVP |

---

## Implementation roadmap (M0)

The smallest end-to-end loop where `nexus-cli` asserts a `Node` and sees `(Ok)` back is **~260 LoC** across 5 missing pieces:

1. **`sema/src/lib.rs`** — open redb; `store(kind_name, bytes) → Slot`; `get(Slot) → bytes`. ~50 LoC.
2. **`criome/src/validator/schema.rs`** body — match `RawRecord.kind_name` against `signal::KNOWN_KINDS`. ~20 LoC.
3. **`criome/src/validator/write.rs`** body — call `sema::store`; assign Slot. ~30 LoC.
4. **`criome/src/uds.rs`** + `main.rs` — UDS bind on `/tmp/criome.sock`, accept loop, length-prefixed Frame decode/encode, dispatch on Request variant. ~80 LoC.
5. **`nexus-cli/src/main.rs`** — read `.nexus` text, parse via nexus-serde to typed Records, wrap as `Request::Assert(AssertOp)`, encode Frame, write to UDS, read Frame reply, render to text. ~80 LoC.

**Skipped for M0** (stay `todo!()`): validator stages 2/3/4/6 (refs / invariants / permissions / cascade), Mutate / Retract / Patch / Query / Subscribe verb processing, the nexus daemon binary (CLI talks signal directly to criome), lojix integration.

**Open design questions for the implementation** (none blocking):
- The nexus daemon binary is needed for text-only clients; for M0 nexus-cli has nexus-serde linked so it speaks signal directly.
- Length-prefix frame format is settled (4-byte big-endian per `signal/ARCHITECTURE.md` ## Wire format).

---

## Memory index (~/.claude/projects/-home-li-git-mentci/memory/)

The feedback memories distilled this session. Read these for the operational rules:

- `project_criomev3.md` — bit-rot markers (aski / `:keyword` / `(Assert …)` etc.)
- `feedback_no_negative_context.md`
- `feedback_no_version_history_in_designs.md`
- `feedback_vision_docs_plain_language.md`
- `feedback_multiline_string_indent.md`
- `feedback_verify_parallel_writes.md`
- `feedback_signal_is_native_not_optimization.md`
- `feedback_criome_speaks_signal_only.md`
- `feedback_nexus_is_not_a_programming_language.md`
- `feedback_arch_docs_durable_reports_not.md`
- `feedback_no_parser_keywords_not_no_schema_enums.md`
- `reference_workspace_layout.md` — symlinks + multi-root, NOT submodules
- `feedback_commit_style.md`, `workflow_jj.md` — process

---

## Reports — what survives this cleanup

- `074-portable-rkyv-discipline.md` — pinned-features reference, actively cited from signal source + arch.
- `085-session-handoff-2026-04-26.md` — this report.

All other reports retired in this cleanup pass (070, 077, 078, 079, 080, 081, 082, 083, 084) — their load-bearing content lives in arch docs; their working-notes content has expired.

---

## Where to start a new session

1. Run `bd prime` (auto-runs at session start in claude code).
2. Read [criome/ARCHITECTURE.md](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) for project shape.
3. Read [nexus/spec/grammar.md](https://github.com/LiGoldragon/nexus/blob/main/spec/grammar.md) for the locked text language.
4. Read this report (085) for current state + roadmap.
5. Open MEMORY.md for the operational rules.
6. `bd ready` for any task tracking; otherwise the M0 roadmap above is the next concrete work.
