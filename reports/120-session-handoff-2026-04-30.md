# 120 — Session handoff: end-of-day 2026-04-30

*Compact handoff for the next session. The previous handoff
([reports/112](112-session-handoff-2026-04-29.md)) was at the end
of 2026-04-29; this one captures the 2026-04-30 dance and where
the work paused. Lifetime: until the next session reads it and
either supersedes or deletes.*

---

## 0 · Read first

1. [`INTENTION.md`](../INTENTION.md) — clarity > correctness >
   introspection > beauty; no time estimates; no MVP framings;
   no stop-gaps.
2. [`AGENTS.md`](../AGENTS.md) — process rules. Recent additions
   relevant to this session:
   - §"No stop-gaps — INTENTION applied"
   - §"Design reports — visuals, not code"
   - §"Commit message style" (rewritten — short single-line, no
     S-expression nesting)
3. [criome ARCHITECTURE.md](../repos/criome/ARCHITECTURE.md) —
   the canonical engine description.
4. The reports below, in order: 113 → 114 → 115 → 116 → 117 →
   118 → 119 (this session's arc).

---

## 1 · What landed in code this session

```
   repo            HEAD                 what changed
   ──────────      ────────────         ────────────────────────────────
   sema            ed238815             SEED_RANGE_END reservation
                                        removed; counter starts at 0
                                        (Li 2026-04-30: didn't earn
                                         its keep)

   signal          b2814862             1) Slot<T> phantom-typed (every
                                           record file + manual rkyv
                                           Archive impls)
                                        2) RetractOperation refactored
                                           to per-kind enum
                                        3) AnyKind / CommittedMutation /
                                           CriomeDaemonInstance markers
                                        4) schema.rs added: Kind trait
                                           + ALL_KINDS const + 19 kinds
                                           with #[derive(Schema)]
                                        5) signal-derive dep added
                                        40 tests pass

   signal-derive   6f7b380e (NEW)       new repo. proc-macro emits
                                        impl signal::Kind for T with
                                        const DESCRIPTOR. Wired but
                                        flagged as wrong-runtime-shape
                                        (see §3 below)

   criome          ad7b22f7             reader.rs threads Slot<T>
                                        through decode_kind. 6 tests
                                        pass.

   mentci-lib      f823edfc             every Slot field across
                                        state/event/constructor/
                                        canvas/inspector/view/
                                        diagnostics/theme/layout typed
                                        with the right kind parameter

   mentci-egui     467ddacc             default_principal returns
                                        Slot<Principal>; doc updated
                                        to drop the [0,1024) reference

   mentci          most recent ~10      reports 113-119 + AGENTS.md
                   commits              edits + workspace-manifest +
                                        devshell registration of
                                        signal-derive
```

All ~85 tests across the workspace pass.

---

## 2 · Reports written this session

| # | Title |
|---|---|
| [113](113-architecture-deep-map-2026-04-29.md) | Architecture deep map (verified directly against source) |
| [114](114-mentci-stack-supervisor-draft-2026-04-30.md) | Mentci stack supervisor (process-manager) — design |
| [115](115-schema-derive-design-2026-04-30.md) | Schema-derive design (originally framed; corrected by 119) |
| [116](116-genesis-seed-as-design-graph-2026-04-30.md) | Genesis seed = the project's design as a flow-graph |
| [117](117-implementation-gap-2026-04-30.md) | Implementation gap (code vs design) |
| [118](118-signal-derive-emitted-code-2026-04-30.md) | What signal-derive emits (banner: read 119) |
| [119](119-schema-in-sema-corrected-direction-2026-04-30.md) | Schema-in-sema — descriptors live in the database |
| 120 | this report |

Soft cap is ~12 active reports; we're at 8 — under cap, no
rollover needed.

---

## 3 · Where the work paused

**Partway through the §5 sequence in [reports/117](117-implementation-gap-2026-04-30.md).**
The Slot<T> migration (step 5) and signal-derive crate (step 6)
both landed — but [reports/119](119-schema-in-sema-corrected-direction-2026-04-30.md)
corrected the direction mid-stream:

> *Li 2026-04-30: "to me, this looks like it should be data in
> sema — a kind of data that needs different authorization to
> edit (read-only except for unsafe system edits). It looks
> really clumsy like this, hardcoding it in the runtime like this."*

The `impl Kind for T { const DESCRIPTOR }` form is still useful
as the **bootstrap projection source**, but its current
*consumer-side framing* (mentci-lib walks `ALL_KINDS` directly)
is wrong. The correct shape: schema lives in sema as `KindDecl`
/ `FieldDecl` / `VariantDecl` records, with `Slot<KindDecl>`
references for cross-record identity (no string lookups). The
proc-macro stays; mentci-lib's read path changes.

Tracked-as-known-wrong: beads `mentci-next-lvg`.

---

## 4 · Next steps (with bd issue IDs)

The first-cut goal — "engine running end-to-end with the design
graph painted on first launch" — is unchanged. Order:

```
   ┌── ready to start ───────────────────────────────────────────┐
   │                                                             │
   │  mentci-next-m5m  P1  Add KindDecl + FieldDecl +            │
   │                       VariantDecl + KindShapeDecl +         │
   │                       FieldTypeDecl record kinds to         │
   │                       signal (with #[derive(Schema)] —      │
   │                       the recursion lands clean)            │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘
   ┌── unblocks once the above lands ────────────────────────────┐
   │                                                             │
   │  mentci-next-4g9  P1  Build kinds.nexus seed projector      │
   │                       (one-shot binary in signal that       │
   │                        reads ALL_KINDS at boot time and     │
   │                        emits Assert KindDecl/FieldDecl/     │
   │                        VariantDecl statements as nexus      │
   │                        text for piping through nexus-cli)   │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘
   ┌── unblocks once the above lands ────────────────────────────┐
   │                                                             │
   │  mentci-next-wd3  P1  Create process-manager crate          │
   │                       (first-cut scope from 114 §10.3:      │
   │                        config + spawn + readiness +         │
   │                        seed [kinds.nexus then               │
   │                        genesis.nexus] + respawn on crash    │
   │                        + tear-down. NO swap. NO watch       │
   │                        mode. New repo on github;            │
   │                        Li green-lit creating without        │
   │                        asking)                              │
   │                                                             │
   │  mentci-next-xpl  P1  Write mentci/seeds/genesis.nexus      │
   │                       (the design graph per reports/116:    │
   │                        19 component Nodes + ~28 Edges)      │
   │                                                             │
   │  mentci-next-149  P2  mentci-lib CompiledSchema queries     │
   │                       sema (replacing the current todo!()   │
   │                       bodies; do NOT read ALL_KINDS         │
   │                       directly per beads mentci-next-lvg)   │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘
```

After all of the above land, the engine boots, asserts the
schema records via kinds.nexus, asserts the design graph via
genesis.nexus, and mentci-egui paints the design on first
launch. That's the "engine working end-to-end" milestone.

Subsequent work (steps 7-11 from [117 §5](117-implementation-gap-2026-04-30.md#5--sequence-to-engine-works-end-to-end))
not yet in bd: NewEdge constructor body, mentci-egui drag-wire
handlers, per-user identity (mentci-keygen one-shot + BLS key
ceremony), then M1 verbs (Mutate / Retract / AtomicBatch),
Subscribe push delta, KindDecl access-control tightening.

---

## 5 · Resolved questions Li answered this session

| | answer |
|---|---|
| Bootstrap-rung-by-rung slot reservation | Removed. Beauty rule: didn't make anything more beautiful, elegant or correct. |
| CompiledSchema stop-gap (path A vs path B) | Withdrawn. Stop-gaps violate INTENTION; AGENTS gained §"No stop-gaps". |
| Mentci key management | Mentci's domain. UI for creating/managing user keys. Future: separate key daemon for HW enclaves. |
| Process-manager name | `process-manager`. Says directly what it is. |
| Process-manager scope | First cut: supervise/respawn/seed. Swap deferred. |
| Process-manager terminal home | OS-embedded eventually (CriomOS service module). Not a user-facing CLI shim. |
| Genesis seed contents | The project's own design as a flow-graph (per [116](116-genesis-seed-as-design-graph-2026-04-30.md)). |
| Genesis.nexus location | `mentci/seeds/genesis.nexus`. |
| Auto-select graph on first paint | No. Keep simple. |
| Slot<T> shape | Phantom-typed. The kind information lives inside the type. |
| signal-derive crate location | New crate, not extension of nota-derive (wrong-noun trap). |
| Schema authority | Sema-resident, not binary-resident. |
| Re-implementing nexus? | No. The codec stays; sema gets the same shape information for a different consumer. Variant names in nexus text are stored literally as PascalCase identifiers (per `nota-derive/src/nota_enum.rs`); rkyv binary uses discriminant integers. |

---

## 6 · Open questions Li hasn't answered yet

These are flagged in the relevant reports' §-final-questions
sections; not blocking the next steps but worth raising again
when you reach them:

- **[119 §7 Q2 — Access-control carrier for schema writes.**
  Transient "genesis context" flag in criome (small) vs real
  capability token criome signs for itself (durable, pulls in
  BLS signing infra). Default: durable; flagging because of
  scope.
- **[114 §10.2 Q5 — Reconnect after intentional swap.** UX
  shape proposed (auto-reconnect on intentional swap, chip-click
  for crashes, control-message flagging intent). Implementation
  deferred since swap is later iteration. Confirm UX when swap
  lands.
- **[116 §4 — Open shapes** for Supervises RelationKind variant,
  prism→signal Produces edge, mentci-as-workspace-umbrella,
  Graph title choice.

---

## 7 · Where to look in code

```
   sema/src/lib.rs                 SEED_RANGE_END removed;
                                   counter starts at 0
   signal/src/slot.rs              Slot<T> with manual rkyv +
                                   nota-codec impls
   signal/src/schema.rs            Kind trait + ALL_KINDS const
                                   (bootstrap source — see §3)
   signal/src/{flow,identity,
              tweaks,layout,
              keybind,style}.rs    every record gets
                                   #[derive(Schema)]
   signal/src/edit.rs              MutateOperation per-kind
                                   variants typed; RetractOperation
                                   refactored from struct to enum
   signal/tests/schema.rs          5 tests verifying derive output
   signal-derive/src/lib.rs        proc-macro emitting impl
                                   signal::Kind
   criome/src/reader.rs            decode_kind<T> threads typed
                                   slots through
   mentci-lib/src/schema.rs        CompiledSchema with todo!()
                                   bodies; docstring says queries
                                   sema (NOT ALL_KINDS)
   mentci-lib/src/state.rs         WorkbenchState.principal:
                                   Slot<Principal> + cache fields
                                   typed
   mentci-egui/src/main.rs         default_principal returns
                                   Slot<Principal>
   mentci/AGENTS.md                "No stop-gaps" + "Design reports
                                   — visuals, not code" +
                                   "Commit message style" (short)
```

Skeletons (todo!() bodies — design-as-code, not yet
implementation): `forge/src/`, `arca/src/`, `signal-forge/src/`,
parts of `criome/src/validator/`.

---

## 8 · One-paragraph state

The Slot<T> migration is in. signal-derive is in but the
consumer-side direction was corrected mid-stream — schema lives
in sema, not in compiled binaries. The proc-macro stays as the
bootstrap projection source. Five bd issues (mentci-next-m5m →
4g9 → wd3 → xpl + 149) chain into the corrected sequence; once
they land the engine boots end-to-end with the design graph
painted on first launch. mentci-next-lvg tracks the in-flight
correction for future agents who'd otherwise extend the
binary-resident path. Reports 119 + 120 are the load-bearing
docs for resuming.

---

*End report 120.*
