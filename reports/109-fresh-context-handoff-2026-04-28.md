# 109 — Fresh-context handoff snapshot 2026-04-28 (post-rename + design)

*Ephemeral. Replaces the deleted `reports/107`. Delete when consumed —
the workspace's target state is `reports/` containing only the active
design report (currently 108).*

## 0 · TL;DR

- **M0 demo working end-to-end.** `(Node "User")` → `(Ok)` and
  `(| Node @name |)` → `[(Node "User")]` shuttle through
  `criome-daemon` + `nexus-daemon` via `nexus-cli`, verified by
  `mentci-integration` in `nix flake check`. All 14 derivations
  pass.
- **Active design captured in [`reports/108`](108-flow-graph-three-projections-2026-04-28.md)** —
  flow-graph-as-shared-substrate with three projections (nexus
  text shipping, prism code-emission for runtime creation, mentci
  UI for live render+edit). Of the 12 §8 questions, **6 resolved
  2026-04-28 (Q1/Q3/Q4/Q5/Q9/Q12)** — see §6. The remaining 6
  (Q2/Q6/Q7/Q8/Q10/Q11) are tactical, deferrable to M1 work.
- **prism (renamed from rsc, 2026-04-28)** is the code-emission
  piece of `lojix-daemon`'s runtime-creation flow; lojix-daemon
  links prism and runs the surrounding work internally —
  workdir assembly, nix-via-crane-and-fenix compile, bundle
  into lojix-store. Full flow:
  [`criome/ARCHITECTURE.md` §7 — Compile + self-host loop](../../criome/ARCHITECTURE.md).
  Stub today; body lands once criome's record supply is wide
  enough + lojix-daemon arrives.
- **Path A on KindDecl** (2026-04-28): `KindDecl` + `FieldDecl` +
  `Cardinality` + `KindDeclQuery` dropped from signal entirely.
  The closed Rust enum in signal is the **authoritative type
  system today**. Schema-as-data scaffolding will be re-added when
  `prism` or mentci has a real reader.

## 1 · Required reading order

A fresh agent reads, in this order:

1. [`mentci/AGENTS.md`](../AGENTS.md) — workspace conventions
   (jj+always-push, naming, S-expression commits, restate-to-refute
   prohibition, report rollover, §Style docs are patterns not
   snapshots).
2. [`mentci/docs/workspace-manifest.md`](../docs/workspace-manifest.md) —
   repo inventory + status (CANON / TRANSITIONAL / SHELVED).
3. `criome/ARCHITECTURE.md` — the project-wide canonical doc
   (Invariants A–D, the request flow, the three-daemon shape, the
   two-stores split, the lojix-daemon-as-runtime-creator framing).
4. [`tools-documentation/programming/{beauty,abstractions}.md`](https://github.com/LiGoldragon/tools-documentation) —
   discipline (beauty as the criterion, methods on types,
   "free functions are incorrectly specified verbs").
5. [`tools-documentation/rust/{style,ractor,rkyv}.md`](https://github.com/LiGoldragon/tools-documentation/tree/main/rust) —
   Rust patterns (the No-ZST-method-holders rule, ractor 0.15
   four-piece-template + mailbox semantics, rkyv portable
   feature-set).
6. **[`reports/108`](108-flow-graph-three-projections-2026-04-28.md)** —
   the active design. Read in full; the 11 §8 questions are the
   gating decisions for code work.
7. Per-repo `AGENTS.md` + `ARCHITECTURE.md` when working in a
   specific repo.

## 2 · Where to look

| Question | File |
|---|---|
| Workspace conventions | `mentci/AGENTS.md` |
| Repo inventory + status | `mentci/docs/workspace-manifest.md` |
| Project-wide architecture (canonical) | `criome/ARCHITECTURE.md` |
| Per-repo orientation | `<repo>/AGENTS.md` |
| Per-repo bird's-eye | `<repo>/ARCHITECTURE.md` |
| Beauty + methods-on-types + free-fns-are-incorrectly-specified-verbs | `tools-documentation/programming/{beauty,abstractions}.md` |
| Rust style + No-ZST-method-holders + branch-pin policy | `tools-documentation/rust/style.md` |
| Ractor 0.15 patterns | `tools-documentation/rust/ractor.md` |
| Rkyv 0.8 discipline | `tools-documentation/rust/rkyv.md` |
| Crane + fenix flake layout | `tools-documentation/rust/nix-packaging.md` |
| Nix integration-test patterns | `tools-documentation/nix/integration-tests.md` |
| **Active design idea** | `reports/108` |

## 3 · State per repo

- **CANON, M0 working** — `criome`, `nexus`, `nexus-cli`, `signal`,
  `sema`, `nota`, `nota-codec`, `nota-derive`, `tools-documentation`.
  All cross-repo Cargo deps use `branch = "main"`. Lockfiles pin
  by sha. Workspace `nix flake check` from mentci passes all 14
  derivations including `mentci-integration` end-to-end.
- **CANON, skeleton (M2+)** — `lojix-store`. Stub with valid
  ARCH+AGENTS docs.
- **TRANSITIONAL** — `lojix`, `lojix-cli`, `prism` (renamed from
  rsc 2026-04-28; the code-emission subcomponent of lojix-daemon's
  pipeline; stub).
- **CANON cluster (CriomOS)** — `CriomOS`, `horizon-rs`,
  `CriomOS-emacs`, `CriomOS-home`. Ancillary; not part of the
  M0 daemon graph.

## 4 · Recent decisions worth carrying forward

- **branch=main for sibling Cargo deps.** Lockfiles pin by sha for
  reproducibility; Cargo.toml stays stable while upstream evolves.
  Switch a dep to `rev = "..."` only when it stabilises.
- **`signal::Diagnostic::error(code, message)`** — canonical
  constructor for daemon-emitted Error-level diagnostics. Replaces
  the prior free `criome::engine::diagnostic` function.
- **`Daemon::start` was a ZST-method-holder violation, removed.**
  Actor markers (`pub struct Daemon;`) carry only `impl Actor for
  Daemon` (trait impl). The spawn lives inline in `main.rs`.
- **`CriomedInstance` → `CriomeDaemonInstance`** (daemon-suffix
  naming convention).
- **Reports/074 absorbed into `tools-documentation/rust/rkyv.md`**;
  9 cross-citations across 4 repos repointed at the durable home.
- **Style docs are patterns, not snapshots** — no embedded
  specifics from current downstream repos (no exact actor counts,
  no file-path listings, no concrete function or message-variant
  names that will rot, no cross-citations to a downstream
  ARCHITECTURE.md to backstop a claim).
- **Free functions are incorrectly specified verbs** — when you
  reach for one, slow down and find the noun. If no obvious noun
  exists, the model is incomplete; the missing type is what the
  verb is asking you to declare.
- **Reports are ephemeral.** Default to deletion. Extract to
  architecture docs or tools-documentation only when the rationale
  has no other durable home.
- **lojix-daemon orchestrates runtime creation.** prism emits
  `.rs` only; lojix-daemon links prism and runs the surrounding
  work internally — workdir assembly, nix-via-crane-and-fenix
  compile, bundle into lojix-store. Full flow lives in
  criome/ARCHITECTURE.md §7; the exact internal orchestration
  shape inside lojix-daemon is open until lojix-daemon is built
  (today: skeleton-as-design).
- **criome speaks only signal.** Signal is the messaging system
  of the whole sema-ecosystem. nexus is one front-end (text↔signal
  gateway); mentci will be another (gestures↔signal). Any future
  client connects the same way — by speaking signal directly.
  Documented first-class in criome/ARCHITECTURE.md §1 as of
  2026-04-28.
- **mentci is two things at once** — the workspace umbrella (this
  repo: dev shell, design corpus, agent rules, reports) and the
  concept goalpost (the eventual LLM-agent-assisted editor). The
  actual GUI implementation will land in a **separate future
  repo**; "mentci" is the working name in design docs until that
  repo is created.
- **Path A on KindDecl** — schema-as-data scaffolding dropped
  entirely until prism or mentci earns it (see §0 + reports/108
  §2 + §8 Q12).
- **Push, never pull** (2026-04-29). Producers push, consumers
  subscribe; no polling fallback, ever. Documented in
  [`tools-documentation/programming/push-not-pull.md`](https://github.com/LiGoldragon/tools-documentation/blob/main/programming/push-not-pull.md).
  Resolves reports/108 §8 Q6 + sets the design rule for all
  future change-feeds in the workspace.
- **`mentci-lib` is its own crate/repo** (2026-04-29). Holds
  signal-speaking logic (gesture → signal envelope translation,
  criome-link, reply demux). Consumed by the future GUI repo
  and any alternative UIs (mobile, etc.). One Rust crate per
  repo per `tools-documentation/rust/style.md`. Resolves
  reports/108 §8 Q7.
- **Composite gestures are atomic** (2026-04-29). The UI wraps
  multi-mutation gestures in `AtomicBatch([…])`, all-or-nothing.
  No "atomic mode" toggle. Resolves reports/108 §8 Q11.
- **Supervisor is a control-plane node kind** (2026-04-29). When
  the Supervisor variant lands in signal, its edges to children
  are control-plane relations (`Supervises`, `EscalatesTo`),
  not data edges. Most graphs won't declare an explicit
  Supervisor since supervision is recursive in ractor; the kind
  is for explicit fault-isolation boundaries. Refines reports/108
  §8 Q1.

## 5 · bd state

10 open issues, 0 in-progress, 0 blocked — all forward-looking work.

| ID | P | Title |
|---|---|---|
| `mentci-next-ef3` | P1 | Self-hosting "done" moment — concrete first feature |
| `mentci-next-d3b` | P2 | M0 step 7 — genesis.nexus + bootstrap glue |
| `mentci-next-7tv` | P2 | M1 — per-kind sema tables (replace 1-byte kind discriminator) |
| `mentci-next-8ba` | P2 | M3 — sema redb wrapper (database library) |
| `mentci-next-4jd` | P2 | M2-remainder: method-body layer in signal |
| `mentci-next-0tj` | P2 | Implement prism records-to-Rust projection |
| `mentci-next-7dj` | P2 | Cross-repo wiring — flake input pattern for local dev |
| `mentci-next-9jv` | P2 | Create mentci-lib repo for signal-speaking logic |
| `mentci-next-txz` | P2 | Create GUI repo for mentci's flow-graph editor (depends on 9jv) |
| `mentci-next-zv3` | P2 | M6 — bootstrap demo (DB → prism → compile → demonstrate) |

(`mentci-next-dqp` "Rename rsc to a full English word" was closed
2026-04-28 with the prism rename. `mentci-next-9jv` and
`mentci-next-txz` were added 2026-04-28 / 2026-04-29 capturing the
future repos for the gesture→signal library and the GUI editor.)

## 6 · Open work — all §8 questions answered

The 12 questions in [`reports/108` §8](108-flow-graph-three-projections-2026-04-28.md#8--open-questions)
are now resolved (11) or deferred (1).

**Resolved 2026-04-28**:

- **Q1** — closed set of 5 first node kinds: **Source /
  Transformer / Sink / Junction / Supervisor** (extending Li's
  tentative trio per Akka-Streams + OTP convergence research).
- **Q3** — prism shape: **library** (Rust).
- **Q4** — mentci UI tech: **egui** (top of three; iced #2, gpui
  #3). Linux desktop, arbitrary 2D + rotation transforms for
  interactive wheels + future astrological charts.
- **Q5** — mentci ↔ criome: **direct UDS, mentci speaks signal**.
- **Q9** — main repo name: **mentci**, current repo is workspace
  umbrella + concept goalpost; future GUI is separate repo.
- **Q12** — Path A on KindDecl (resolved earlier 2026-04-28).

**Resolved 2026-04-29**:

- **Q1 refinement** — Supervisor described accurately: control-
  plane node with child registry + restart strategy; edges to
  children are control-plane (`Supervises`, `EscalatesTo`), not
  data edges. Most graphs won't declare an explicit Supervisor
  since supervision is recursive in ractor (per
  `tools-documentation/rust/style.md` §Actors); Supervisor as
  a kind is for explicit fault-isolation boundaries.
- **Q2** — smallest demo: encode criome's M0 request flow as
  records, prism-emit it, run `mentci-integration` against the
  prism-emitted binary (M6 close, `bd mentci-next-zv3`).
- **Q6** — **push never pull**. No polling, ever. mentci's UI
  launches after `Subscribe` ships (M2). Principle documented
  in
  [`tools-documentation/programming/push-not-pull.md`](https://github.com/LiGoldragon/tools-documentation/blob/main/programming/push-not-pull.md).
- **Q7** — `mentci-lib` as a **separate crate** (own repo per
  `tools-documentation/rust/style.md` §"One Rust crate per
  repo"). Holds gesture→signal translation + criome-link logic.
  Consumed by the future GUI repo and any alternative UIs
  (mobile, etc.).
- **Q8** — visible rejection display; specific styling deferred
  (post-prototype).
- **Q11** — **atomic**. Composite gestures wrap in
  `AtomicBatch([…])`. No "atomic mode" toggle.

**Deferred**:

- **Q10** — recursive rendering long-term. Out of scope until a
  running prototype exists. Re-open when M3+ mentci UI is
  working against real graphs.

**Concrete next code work is unblocked**: `bd mentci-next-7tv`
(per-kind sema tables for the 5 kinds), then `bd mentci-next-0tj`
(prism's records-to-Rust projection per the resolved taxonomy +
library shape), then `bd mentci-next-txz` (GUI repo) + the new
bd for `mentci-lib`. None gated on further design questions.

## 7 · Lurking dangers (context worth keeping)

- **Lojix family carries the ZST-method-holder pattern** that
  criome + nexus had before the 2026-04-28 audit; deferred per
  Li's "leave lojix alone for now." When lojix is next worked on,
  audit `lojix/src/{uds,actors/*.rs}` and
  `lojix-cli/src/{build,deploy,proposal,project,artifact}.rs` for
  inherent impls on `pub struct Foo;` and apply the same fix
  shape (move work into `main` or onto a real noun with fields).
- **`horizon-rs` flake** uses blueprint with a Cargo crate at
  `lib/`; blueprint tries to import `lib/default.nix` which
  doesn't exist, so `nix flake check` from horizon-rs fails.
  Direct `nix build .#packages.x86_64-linux.default` works.
  Pre-existing structural issue, unrelated to recent cleanups.
- **prism is a stub.** `src/main.rs` is `fn main() {}`. Body lands
  when criome's record supply is wide enough to project AND
  lojix-daemon arrives to host the runtime-creation pipeline.
  Don't refactor prism speculatively.
- **per-kind sema tables** (`bd mentci-next-7tv`) need to land for
  prism to read structured per-kind storage. The 1-byte kind
  discriminator in `criome/src/kinds.rs` is the M0 stop-gap.
- **Subscribe is M2+.** mentci's UI launches before subscribes
  exist; will poll until then per reports/108 §8 Q6.

## 8 · Lifetime

This file is a snapshot. Delete when consumed.
[`reports/108`](108-flow-graph-three-projections-2026-04-28.md)
remains in the active-design sense — until the design is
concretised into `criome/ARCHITECTURE.md` + per-repo
`ARCHITECTURE.md` + code in prism + mentci. When all three
exist, both 108 and 109 are deleted; `reports/` returns to its
target empty shape.
