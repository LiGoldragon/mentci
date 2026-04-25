# 064 — bootstrap as iterative competence growth

*Claude Opus 4.7 · 2026-04-25 · review of the engine's
implementation path through the lens of iterative bootstrap,
correcting the v0-skeleton-of-the-end framing of report/062
(flagged by Li 2026-04-25) and the diagnostic of report/063.
Built on Li's correction:*

> *"this is going to be an iterative, continuous process. …
> the architecture will not match our intended design. this is
> the nature of bootstrap. … the seed will concern itself with
> populating sema with the data needed for criomed's decision-
> making state aspect to become operational. we can't beeline
> to the end all at once, the engine is too complex."*

*Near-term detail high; long-term direction only — no beeline.
Agents not used this round (conceptual reframe; canonical
corpus sufficient); deferred to follow-up rounds when stage-
specific detail becomes near-term.*

---

## 1 · The corrected high-level understanding

### 1.1 · Bootstrap is a gradient, not a moment

Earlier drafts treated bootstrap as: *build a v0 that touches
every part of the architecture, then thicken*. That framing
forces shortcuts because the full architecture is too complex
to assemble at once; the shortcuts then violate invariants
(report/063). The correction: bootstrap is the continuous
process by which sema accumulates the records criomed needs
to be more competent at deciding things. Each stage is a
coherent operational state; it just decides less than the
next stage will.

### 1.2 · The seed's purpose

The seed inside criomed's binary is *not* a test fixture, *not*
a way to "have an Opus to compile", *not* a shortcut for
prototyping rsc. Per Li 2026-04-25:

> *"the seed will concern itself with populating sema with the
> data needed for criomed's decision-making state aspect to
> become operational."*

Without the seed, criomed cannot validate any nexus message at
all — it doesn't know what `KindDecl` looks like, doesn't know
what a `SlotBinding` looks like, doesn't know what its own
decision substrate is. The seed is what makes the validator a
validator. It has nothing to say about rsc, nothing to say
about lojixd, nothing to say about compile.

### 1.3 · Architecture-during-bootstrap is sparser than intended-architecture

Per Li: *"the architecture will not match our intended
design."* The intended architecture has nexusd + criomed +
lojixd, two stores, contract crates, capability tokens, rule
cascades, rsc, and more. At early stages, only the components
with a *job to do* exist. lojixd doesn't exist yet because
nothing is compiling. rsc is empty because no projection is
happening. lojix-msg is unwritten because no producer or
consumer exists for it.

This is **not** "v0 stubs" (the framing 063 critiqued). It is
"components that haven't been built yet because their
preconditions aren't met." The components that DO exist at
any stage are real — not stubs of their eventual selves. They
respect their invariants from day they're born. The thing
that changes between stages is the *inventory* of components,
not the quality of any single one.

### 1.4 · Stages are defined by what criomed can decide

The stages are conceptual handles, not a rigid plan. The
actual progression composes by what's most needed next:

| Stage | Criomed can decide | Components that exist |
|---|---|---|
| **A** | seed-shaped messages conform to seed kinds | nexusd, criomed, signal, nexus-cli, sema (redb), seed |
| **B** | user-authored KindDecls; new kinds extend the catalogue | (no new components — sema content extends) |
| **C** | slot-references resolve correctly; the index is real | (no new components — the validator now exercises ref-check) |
| **D** | cascade rules fire; derived facts settle | rule engine slot in criomed begins exercise |
| **E** | requests authorised against capability records | capability validator exercised; multi-principal real |
| **F** | a Compile request is dispatchable | lojixd, lojix-msg, rsc body, lojix-store body all enter |
| **G** | rsc-projected crates compile via nix | the compile loop closes |
| **H** | engine's own crates begin records-authoring | per the self-hosting gradient (architecture.md §2 Invariant A) |
| **Z** | every canonical crate runs records-authored | self-host close per reports/061 §1.12 |

Each stage is operational at its level. We do not skip ahead.

---

## 2 · Near-term: Stages A and B (concrete)

This is the level Li wants detailed. Beyond Stage B, §4
gestures direction only.

### 2.1 · Stage A — criomed accepts a seed-shaped message

**What exists at Stage A:**

- **nexus-schema** crate with the seed kinds (schema-of-
  schema records, `KindDecl`, `SlotBinding`, `FieldSpec`,
  `TypeRef`, `ChangeLogEntry`, `AuditEntry` — exact set is
  the §3 detail-research item).
- **signal** crate with the verb set criomed accepts at
  this stage: `Assert`, `Query`, `Retract`, plus reply and
  diagnostic shapes.
- **criomed** binary that boots, loads the seed, opens sema
  (redb), serves signal over UDS. Validator pipeline is
  real (schema-check → ref-check → invariant-check →
  permission-check → write); single-principal at this stage.
- **nexusd** binary that boots, listens on UDS or stdin,
  parses text via `nota-serde-core` at `Dialect::Nexus`,
  builds signal envelopes, dials criomed over UDS,
  serialises replies back to text.
- **nexus-cli** thin client that pipes text → nexusd over UDS
  → text.
- shared transport machinery: UDS + length-prefixed rkyv
  frames.

**What does NOT exist yet:**

- lojixd (no compile, no nix, no bundle to drive).
- lojix-msg (no producer or consumer).
- rsc projection bodies (skeleton remains; no records to
  project yet).
- lojix-store bundle/writer bodies (skeleton; no artifacts).
- cascade rule engine *exercised* (the slot in criomed is
  there but no Rule records loaded; cascades are inert).
- capability tokens *exercised* (the slot is there; single
  hardcoded principal; no multi-principal yet).
- horizon-rs absorption (still in the "hacky stack" per
  reports/061 §3.2; lojix monolith still ships deploys via
  its current path; this stack is untouched at Stage A).

### 2.2 · Stage A success criterion

```
$ nexus-cli '(Query (KindDecl :name "KindDecl"))'
(Reply :ok :records [(KindDecl :slot 1 :name "KindDecl" :fields [...])])
```

Criomed knows what `KindDecl` is from the seed; can validate
the query envelope; retrieves the matching record from sema;
serialises the reply. The validation pipeline exists end-to-
end for the validator's job. There is no compile, no
projection, no bundle.

### 2.3 · Stage B — first user-authored KindDecl extends the catalogue

```
$ nexus-cli '(Assert (KindDecl :name "ColourPreference" :fields [...]))'
(Reply :ok :slot 1024)

$ nexus-cli '(Assert (ColourPreference :slot 1025 :colour "azure"))'
(Reply :ok :slot 1025)
```

The first assertion lands a new `KindDecl` in sema. Criomed
validates this against the *seed's* `KindDecl` schema (it
already knows what KindDecls look like). The second assertion
is a record of the user-defined kind — criomed validates this
against the `ColourPreference` `KindDecl` *that is now in
sema*. **Criomed's competence grew because sema's content
grew.**

### 2.4 · Concrete tasks for Stages A → B

Ordered roughly; explicit dependencies named:

1. **Author `genesis.nexus`.** Seed records as nexus text
   shipped with the criomed binary; criomed dispatches them
   through nexusd at first boot. Per architecture.md §10
   "Bootstrap rung by rung."
2. **Lock the Stage A kind set in `nexus-schema`.** Minimum:
   schema-of-schema + `KindDecl` + `SlotBinding` + supporting
   types. Subject to §3 detail-research.
3. **Lock signal verbs for Stage A.** `Request::{Assert,
   Query, Retract}`; `Reply::{Ok, Rejected, QueryResult}`;
   `Diagnostic` per reports/060 §4. *← 2.*
4. **Build criomed binary skeleton.** Tokio runtime, UDS
   listener, signal dispatch loop, sema redb open, seed
   loader (per Q1), validator pipeline (schema/ref/invariant/
   permission/write). *← 2, 3.*
5. **Build nexusd binary skeleton.** Tokio runtime, UDS
   listener for client connections, nota-serde-core parser at
   Dialect::Nexus, signal envelope construction, UDS
   client to criomed, reply serialisation. *← 3.*
6. **Build nexus-cli thin client.** Argv/stdin → nexusd UDS
   → reply text. *← 5.*
7. **Author `genesis.nexus`** with schema-of-schema +
   KindDecl + SlotBinding records (and the bootstrap
   Principal/Quorum/Policy + the SemaGenesis marker).
   *← 1, 2.*
8. **Smoke test: Stage A success criterion (§2.2).** *← 4, 5,
   6, 7.*
9. **Stage B path: user asserts a `KindDecl`; criomed
   validates it; sema stores it; subsequent records of the
   new kind validate against it.** *← 8.*

Estimated LoC for Stages A and B combined: 1500–2500. Far
smaller than the 3000–4000 figure in 062 because lojixd,
lojix-msg, rsc bodies, and lojix-store bodies are all out of
scope at this layer.

---

## 3 · Detail still missing for Stage A (research candidates)

Things to verify before Stage A code lands. Good candidates
for follow-up agent rounds when these become near-term:

- **Exact seed kind set.** The schema-of-schema must describe
  itself recursively; the fixed point of this recursion is
  the minimum seed. The 499-LoC `nexus-schema` crate today
  has Enum/Struct/Newtype/Const/Module/Program — these are
  user-facing kinds, not necessarily the seed kinds. The
  seed-vs-user-kind distinction needs concrete enumeration.
- **Validator pipeline shape in criomed.** Where schema-check,
  ref-check, invariant-check, capability-check fit; how
  they're ordered; what state they share; what the failure
  channel looks like. reports/033 has fragments; needs
  synthesis.
- **`nexus-schema` v0.0.1 lock.** What goes in (seed kinds,
  contract types) vs what's left to user-authored extensions
  (most code records).
- **Genesis marker mechanics**: criomed needs an empty-sema
  detection step at boot (read the well-known `SemaGenesis`
  slot; absent → first boot → dispatch `genesis.nexus`;
  present → second boot → verify in-sema seed kinds match
  the binary's built-in Rust types).

---

## 4 · Long-term direction (no beeline)

Stages C → Z get their own reports when each becomes near-
term. Sketch only:

- **C** (slot-references real). Once sema has multiple
  records, the index resolver becomes load-bearing. This is
  reports/048 territory — per-kind change log, name index.
- **D** (rules / cascades user-authored). User asserts `Rule`
  records; cascade engine matches and derives. The cascade
  engine slot in criomed (which existed inert from Stage A)
  starts exercising.
- **E** (capabilities). Capability records; signature
  verification; the genesis-quorum sketch from reports/060
  §2 lands here when ready (single hardcoded principal until
  multi-principal records arrive).
- **F-G** (compile path). lojixd written; lojix-msg written;
  rsc bodies filled in; lojix-store `BundleFromNix` body
  implemented. First binary materialises. The "hacky stack"
  (lojix monolith + horizon-rs + CriomOS-as-configured) per
  reports/061 §3.2 starts being absorbed in this region.
- **H** (engine self-authoring begins). Li / agents author
  `nota-serde-core`, `nexus-schema`, etc. as records along the
  self-hosting gradient (architecture.md §2 Invariant A). Each
  crate flips when its records-authored binary matches the
  hand-written one's behaviour.
- **Z** (close). Criomed runs from a records-authored binary.

Each later stage is a direction, not a plan. We do not
specify late-stage details now because we cannot see them
yet — the engine's own substrate will reshape what those
stages look like as it grows.

---

## 5 · What this corrects in 062 / 063

Report 062 proposed a "walking skeleton" stretching end-to-
end (nexus → all daemons → nix → lojix-store) at v0. Report
063 identified specific contaminations but did not provide
the right framing. This report does:

- **The walking-skeleton concept itself was wrong.** End-to-
  end at v0 forces shortcuts. The correct first iteration is
  Stage A — criomed validates a seed-shaped message — and
  Stage A is operational on its own without rsc, lojixd, or
  lojix-store bodies.
- **The "five v0 stubs" in 062 §6 were wrong.** Each of S1–S5
  is a constraint-collapse. The correct framing is: at Stage
  A, lojixd does not yet exist (nothing compiles), capability
  machinery is unexercised (single principal), cascades are
  unexercised (no rules loaded). NOT stubbed — *not yet
  written*.
- **The 062 §8 questions framed as "skip X at v0?" were
  wrong-shaped.** The right question is "what stage does X
  enter the system, and what's the trigger?" Most do not
  apply at Stage A.
- **The "hand-built records to test rsc" framing was wrong.**
  Records enter sema only via nexus, including the seed if
  delivery is `genesis.nexus`. If seed is baked, it's still
  self-asserted via the same internal validator path, not
  constructed inline by code that bypasses validation. AND:
  the seed's purpose is criomed's decision-making, not rsc
  testing; rsc doesn't enter the picture for many stages.

Stage A's exact kind set emerges from what the signal
verbs need to express (per [reports/070 §6](070-nexus-language-and-contract.md)),
not from a pre-listed taxonomy. See [reports/076 §3.4](076-corpus-trim-and-forward-agenda.md).

---

*End report 064.*
