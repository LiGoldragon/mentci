# 129 — Project state audit (2026-05-02)

*Snapshot of where the criome / sema / mentci ecosystem stands.
What's done, what's in flight, what's pending decision, what
isn't a problem. Lifetime: until the localization-store
ownership decision lands and the per-kind sema tables migration
ships, then this report is folded into a successor or deleted
because the questions it surfaces become resolved.*

---

## 0 · What this project is

Criome is a typed, content-addressed validation engine.
- **Signal frames come in over UDS.** Criome validates them
  against typed constraints, writes accepted mutations to **sema**
  (a redb-backed records DB), forwards effect-bearing verbs to
  **forge**, signs capability tokens, fires subscriptions.
- **Sema** is the records database — slot counter per kind, redb
  tables, no string columns at the schema layer.
- **Forge** is the executor daemon. Links **prism** for code
  emission, runs nix, bundles outputs into **arca** (a
  blake3-keyed multi-store; arca-daemon is the privileged
  writer).
- **Nexus** is the text ↔ signal translator. Humans / agents
  write nexus text; nexus parses to signal frames, forwards to
  criome, renders signal replies back to text.
- **Mentci-lib** is the application logic — workbench state,
  view derivation, constructor flows, dual-daemon connection
  drivers. **Mentci-egui** is the first thin shell over it
  (canvas + nav + inspector + diagnostics).

The architectural arc named in [reports/122](122-schema-bootstrap-architecture-2026-04-30.md) is **schema as records**: types
authored as sema records (Kind, Field, Variant, TypeExpression,
KindShape) rather than as Rust enums. A canonical bootstrap file
declares core kinds in fixed order; criome plays it before
opening the listener; per-kind slot indexes (0, 1, 2 …) are
predictable from file structure; prism reads schema records
from sema and emits Rust source for domain kinds; rustc compiles
a new binary that reads the same sema. The type system is
runtime-resident, not source-resident.

---

## 1 · State

**M0 is done across the canonical roster.** End-to-end
verified: criome validates signal frames, mentci-lib dials
criome, auto-subscribes, paints records on canvas, fires
Assert frames back. 788 commits across the ecosystem in the
last seven days; every canonical repo's last commit is within
two days; all tests pass; all flakes `nix flake check` clean.

Per-repo summary (canonical roster, all on `main`, all clean
working trees as of audit time):

| Repo | M0 state | Notes |
|---|---|---|
| `criome` | Daemon shipped | State-engine body landed. Slot<T> migration complete; signal-derive integrated. |
| `sema` | Shipped | reader_count API + persistence; SEED_RANGE_END mistaken-reservation removed. |
| `signal` | Shipped | All 19 record kinds carry Schema introspection via signal-derive. KindDecl scaffolding dropped (was forward-looking, not read by anyone). |
| `signal-derive` | New (2026-04-30) | Proc-macro emitting per-kind FieldDescriptor/Schema impls. Sibling to nota-derive, different concern. |
| `nexus` | Shipped | Ractor-migrated; Listener/Connection/Daemon actors. Records-with-slots wire shape live. |
| `nexus-cli` | Shipped | One-shot stateless text client. |
| `nota` | Stable | Spec-only, grammar is source of truth. |
| `nota-codec` | Shipped | Serde-free codec. 79 round-trip tests. |
| `nota-derive` | Shipped | Six proc-macro derives. |
| `mentci-lib` | Shipped | Connection driver + auto-subscribe + first constructor flow (NewNode) + 40 node kinds. |
| `mentci-egui` | Shipped | Full per-frame loop; canvas paint resolves edges by slot; modal NewNode working; rename/retract/batch unimplemented (paced for M1). |
| `signal-forge` | Skeleton (2026-04-29) | Build verb payload sketched. Bodies todo!(). |
| `forge` | Skeleton (2026-04-28) | Actor stubs (NixRunner, StoreWriter, ArcaDepositor, FileMaterialiser). Bodies todo!(). |
| `arca` | Skeleton (2026-04-29) | deposit.rs + token.rs stubs; multi-store layout framed. Bodies todo!(). |
| `prism` | Skeleton (2026-04-28) | Contract sketched (FlowGraphSnapshot → Emission). First five node-kind templates named. Bodies todo!(). |

Transitional: `lojix-cli` (operational, deploys CriomOS today;
migrates to thin signal-speaking client of forge when forge
ships). `lojix-cli-v2` (development fork; Nota-native CLI work
parked here while v1 stays operational).

Shelved: `arbor` (prolly-tree versioning; post-MVP).

The skeleton-as-design discipline is being held — boundaries
explicit, ARCHITECTURE.md / AGENTS.md / README in every repo,
tests pinned before bodies, no premature implementation.

---

## 2 · Architectural calls pending

### 2.1 Localization-store ownership (load-bearing)

Three credible options ([reports/124](124-schema-architecture-brainstorm-raw-2026-05-01.md) §3):

- separate criome-engine instance scoped to localization
  (uniform, operationally heaviest)
- dedicated localization-daemon (lighter, but adds a new
  daemon to operate)
- library in nexus / mentci (no daemon; consumer manages
  consistency)

Not a code blocker — the schema-record bootstrap is ready —
but **gates roughly 30% of M1 features**: label rendering,
language switching, schema authoring UI. Open since 2026-05-01.
Worth a decision before more downstream work piles on top.

The dedicated localization-daemon shape carries the lowest
total cost: keeps criome's record-validation pipeline single-
purpose, gives the localization store a clear lifecycle owner,
and is a smaller surface than running a second criome
instance. The library-in-consumer option scatters consistency
concerns (every consumer has to keep its own view current) and
should be the loser unless there's a reason it isn't.

### 2.2 Kind and TypeExpression record shapes

Smaller-stakes versions of the same family of question
([reports/124](124-schema-architecture-brainstorm-raw-2026-05-01.md) §3–4):

- `Kind` as a single record with optional `fields` / `variants`
  fields, or split into `StructKind` / `EnumKind`?
- `TypeExpression` as one record with optional primitives /
  refs / constructors, or one record per case?

Lower-impact than 2.1 but **load-bearing for prism's templates
and the bootstrap-file shape**. Settle these before prism gets
bodies, otherwise the templates have to assume a shape that
might still flip.

---

## 3 · The hard infrastructure task

**Per-kind sema tables.** The schema-as-records arc requires
moving sema's storage from a single records table with a
1-byte kind discriminator to one redb table per kind, each
with its own slot counter. Tracked as `mentci-next-7tv` in
[reports/124](124-schema-architecture-brainstorm-raw-2026-05-01.md) §5, named as load-bearing in [reports/122](122-schema-bootstrap-architecture-2026-04-30.md) §9.

This is a real redb schema migration — not stub-fillable, not
a one-day task. Nothing M1+ ships without it:

- prism can't emit (it needs to read kind-shaped records back
  from sema)
- schema authoring UI can't read kinds
- subscribe-push correctness can't be tested cleanly (the
  cache.absorb-replaces-Vec gap noted in [124](124-schema-architecture-brainstorm-raw-2026-05-01.md) §4)

This is the next concrete piece of code work. Whoever picks it
up should also drive the **forge → prism → arca first-build
proof-of-concept** — getting `cargo nix build` style work
through the right-side pipeline is the proof that the typing is
actually wired right, not just declared right. Doing the table
migration in isolation leaves the integration deferred and
doesn't validate the design end-to-end.

---

## 4 · Recommended sequencing

1. **Decide localization-store ownership.** Write the decision
   into a new report (or an addendum to 122). Default
   recommendation: dedicated localization-daemon, on the
   reasoning above; argue against if there's a reason.
2. **Settle Kind and TypeExpression shapes.** Same report.
3. **Per-kind sema tables migration.** This is real engineering
   work; budget accordingly. Land it with tests against an
   actual schema-record bootstrap that uses the new tables.
4. **First end-to-end build through forge.** Pick the smallest
   real workflow that exercises forge → prism → arca. Doesn't
   have to be production-shaped; "build a hello-world Node
   kind, deposit it in arca, verify the manifest" is enough.
5. **Mentci-egui rename/retract/batch handlers.** Paced for M1
   completion; can run in parallel with anything from steps 1–4.

Steps 1 and 2 are decisions; step 3 is the load-bearing
implementation; step 4 is the integration proof; step 5 is the
M1 UI completion. After step 4, alternative GUI shells
(mentci-iced, mentci-flutter) become attractive — they depend
only on mentci-lib's stable surface.

---

## 5 · What's not a problem

Naming these explicitly so they don't generate false signals
later:

- **No repos silent that shouldn't be silent.** `arbor` (shelved)
  is the only canonical sibling without recent commits, and that's
  correct.
- **No reports waiting on triggers that never came.** [reports/124](124-schema-architecture-brainstorm-raw-2026-05-01.md) is
  marked *"fold into 122 once localization-store ownership
  lands"* — known-pending, not forgotten.
- **No "TODO once X" patterns where X already happened.** The
  2026-04-28 sweep cleaned cross-references after the
  nota-serde → nota-codec rename and the mentci-next absorption.
- **All flakes pass `nix flake check`.** No build rot.
- **Mentci-egui has known incomplete handlers** (21/31
  UserEvent variants unimplemented; canvas paint + selection +
  NewNode work; rename/retract/batch don't). Tracked in
  report 117, paced for M1.

No architectural rot. All docs current. The workspace is in
good working order.

---

## 6 · Open questions worth surfacing

- **Process-manager crate** (`mentci-next-wd3`, [124](124-schema-architecture-brainstorm-raw-2026-05-01.md) §5).
  No code, no design beyond the name. What does "process
  manager" actually mean — systemd services, a Nix module, a
  Rust orchestration library? Worth a one-paragraph design note
  before any code.
- **Self-hosting "done" moment** (`mentci-next-ef3`, [124](124-schema-architecture-brainstorm-raw-2026-05-01.md) §5).
  Currently described only as "concrete first feature." What is
  the minimal example that proves the M1 loop closes — likely
  "create a new Node kind via mentci UI, author its template in
  prism, build it through forge, see it appear in the next
  binary." Worth scoping explicitly.
- **Signal-derive's role post-schema-as-records-M1.** Signal-
  derive currently emits per-kind Schema introspection consts.
  Once schema-as-records lands and clients can query sema for
  kind metadata directly, does signal-derive stay (compile-time
  guarantees), get repurposed (only emit Slot<T> bookkeeping?),
  or retire? Not a blocker; deferred design question.
