# 029 — rust-analyzer, chalk, polonius: structural lessons after stripping the text layer

*Claude Opus 4.7 / 2026-04-24 · design-research follow-up to
reports/023, /025, /026. Previous reports treated r-a/chalk/
polonius as loose analogies ("r-a-style cache", "chalk-like
datalog"). Li's correction (reports/026): sema is the canonical
representation of code, not a cache over text. This report
re-examines what each precedent actually teaches at the
structural level once the text-derivation framing is removed.*

---

## Part 1 — rust-analyzer's true architecture below the text layer

rust-analyzer has two halves that are architecturally distinct
and usually conflated. The **front half** (`syntax`, `parser`,
`mbe`, `hir-expand`, the `base-db` file-loader plumbing) exists
because r-a's input is unstructured text that changes character
by character in an editor buffer. The **back half** (`hir-def`,
`hir-ty`, `hir`, the diagnostics crates, the IDE crates) operates
on structural IDs and typed queries; its inputs are already
lowered. The text-layer contamination we were worried about all
lives in the front half.

The front-half → back-half transition is visible in two crates:

- `hir-expand` takes `SyntaxNode` (a red-tree over rowan) plus a
  macro-expansion context and produces an `ItemTree` plus a
  `DefMap`. `ItemTree` is the first *item-structural* form; it
  strips trivia and carries `FileItemTreeId`s. `DefMap` is a
  module tree with resolved names keyed by `ModuleId` /
  `ItemTreeId`.
- `hir-def` then takes `ItemTree` + `DefMap` and produces
  *definition bodies* as typed structures (`Body`, `Expr`,
  `Pat`, `Statement`), all addressed by `DefId`-style IDs
  (`FunctionId`, `AdtId`, `ConstId`, `StaticId`, `TraitId`,
  `ImplId`, `TypeAliasId`, `ExprId` local to a body). These IDs
  are interned into salsa; they are not span-derived.

Once `hir-def` has run, the back half almost never looks at text
again. `hir-ty` operates on the `Body` expression tree using
`ExprId` as the addressable unit. Inference returns
`InferenceResult` keyed by `ExprId` + `PatId`. The method
resolver, the chalk-ish trait solver (`rustc_trait_selection`
port), and the diagnostic machinery all consume IDs.

### Granularity of incremental invalidation at the HIR level

r-a's invalidation is not "the file changed, re-parse". It's
layered:

1. **Parse-level** (front half): a syntax tree is reparsed for
   the edited file. This is text-only concern; ignore.
2. **ItemTree-level**: the `item_tree_query` recomputes the
   item tree for the file. If the edit is inside a function
   body, `ItemTree` is *unchanged* (bodies are late-lowered).
   This is the famous r-a optimisation: editing a body does not
   invalidate the item structure of the file.
3. **DefMap-level**: name resolution is recomputed for the
   *crate* when items are added/removed. Adding a `use`
   invalidates `DefMap`. Editing a body does not.
4. **Body-level**: `body_query(FunctionId)` recomputes the
   `Body` for one function. Only that function's inference is
   redone. Downstream queries (type-of-expr, method resolution
   for that body) are invalidated; trait solver queries that
   depended on *this body's* inferred types invalidate; trait
   solver queries against *the function's signature* do not,
   because the signature lives in `ItemTree` / `hir-def`
   metadata, not in the body.

This is the piece that transfers directly. Whether the input is
text or records, the structural insight is: **item structure and
body structure invalidate at different granularities**, and a
correctly factored engine keeps them separate. Sema already has
this factoring — `Fn` (signature) is a different record kind
from `Block` / `Expr` (body). A body-mutation cascade in sema
should look almost identical to a `body_query` recompute in r-a:
re-run inference on this body, re-run trait obligations raised
by this body, don't touch the signature or the module's name
resolution.

### The text-specific vs structure-specific line

The clean cut: anything keyed by `SourceFile` / `SyntaxNode` /
`TextRange` is front half; anything keyed by a `*Id` (interned
through salsa) is back half. `parser`, `syntax`, `mbe`,
`hir-expand`, `base-db` are front; `hir-def` is the boundary;
`hir-ty`, `hir`, and the IDE crates are back; `rust-analyzer`
itself is LSP glue on top.

Delete the front-half crates and we lose `.rs` ingest, macro
expansion, proc-macro support, and span-indexed diagnostics.
We keep: `hir-def`'s item/body model, `hir-ty`'s inference,
the trait solver, method resolution, coercion, the
borrow-check interface, and name resolution over `DefMap`.
That remainder is a reasonable skeleton for semachk. The only
structural gap is *persistence*: `hir-def`'s stores are
salsa-interned and process-local; sema wants durable records
with content-hash IDs. That swap is a shape change but not a
structural one — see Part 4.

## Part 2 — chalk's role and limits as a precedent

Chalk encodes Rust's trait system as Horn-ish logic programming.
A chalk program consists of **program clauses** (facts and
rules) and **goals** (queries). A program clause for a trait
impl looks structurally like:

- *head*: `Implemented(T: Trait<A1, …>)`
- *conditions* (the "if"): a conjunction of sub-goals —
  `Implemented(T: Bound1)`, `Normalize(<T as Trait>::Assoc ->
  U)`, well-formedness conditions, lifetime conditions.

Chalk's solver does SLG-style resolution with canonicalisation
and universes. Four design artefacts matter for sema:

1. **Goals and clauses are data**, not code. A `ProgramClause`
   and a `Goal` are structurally well-defined terms; solving is
   a computation over them — the rules-as-records shape sema
   targets.
2. **Canonicalisation** normalises inference variables so
   structurally-equivalent goals share a cache key. Any
   caching trait-solver *must* canonicalise; otherwise the
   cache is useless.
3. **Universes** distinguish environment-quantified from fresh
   skolem variables. `TraitObligation` records need this field.
4. **Solver results** are `Unique(substitution, subgoals)` /
   `Ambiguous` / `NoSolution` — record this trichotomy, not a
   boolean.

### What chalk's encoding suggests for sema

`TraitImpl` in sema already exists (reports 025, 026). Chalk
teaches that each `TraitImpl` should materialise a derived
`ProgramClause` record at derivation time — head is the impl's
predicate, conditions are the where-clauses lowered to goal
form. Trait resolution becomes a query against a relation of
`ProgramClause`s, not a walk of `TraitImpl`s. Sema's
solver-facing record inventory: `TraitImpl` (user-facing),
`ProgramClause` (derived, solver-facing), `TraitObligation`
(pending goal raised by a body's inference), `TraitResolution`
(solver result).

### Incrementality and limits

Chalk itself is per-query. r-a wraps it with salsa so
`trait_solve_query(canonical_goal)` is memoised — because chalk
canonicalises first, the cache is goal-structural. Sema
transfer: cache `TraitResolution` by canonicalised
`TraitObligation` ID. When a `TraitImpl` mutates and its
`ProgramClause` changes, only the `TraitResolution`s whose goal
could match that clause's head invalidate. That "could match"
check is standard discrimination-tree / orphan-rules work;
inherit it conceptually.

Chalk's limits: it is an oracle, not a storage engine;
evolution across revisions is out of scope. It doesn't address
lifetime inference or borrow checking. Its solving is
semi-decidable — pathological impl trees loop. Lift chalk's
`Overflow` category into `TraitResolution`.

## Part 3 — polonius's role and limits as a precedent

Polonius restates NLL borrow checking as a datalog program over
input facts derived from MIR. Inputs include `loan_issued_at`,
`loan_killed_at`, `cfg_edge`, `var_used_at`, `var_defined_at`,
`path_moved_at`. Outputs include `loan_live_at`,
`errors(point, loan)`, `move_errors(point, path)`.

### Structural vs source-span basis

Polonius's input facts are **keyed by MIR points** (`Point` is
a basic-block-id × statement-index pair) and **MIR paths**
(`Path` is a projection over a MIR local). MIR is already
post-HIR, post-type-check; spans have been attached to MIR
statements but the identity of a point / local does not depend
on the source span. Polonius could in principle run on any IR
that supplies the input-fact relations — the polonius engine
itself is generic datalog (originally `datafrog`, now optionally
souffle-compiled).

**Conclusion: polonius is already structural.** The spans it
associates with errors are for display; the analysis is over
IDs. That makes it a good precedent for sema: borrow-check
cascades produce `BorrowError` records keyed by
`Function × Point × Path` (structural), and the span is a
sidecar displayed by whatever surface renders the diagnostic.

### Where borrow-check results live in sema

Two options. **MVP**: keep borrow check inside
rustc-as-derivation; the `cargo check` outcome record subsumes
borrow errors in its diagnostic stream. **Post-MVP semachk**:
polonius-on-sema — materialise `MirBody` records (derived from
`Fn.body`), derive input-fact records (`LoanIssuedAt`,
`PathMovedAt`, `CfgEdge`), run a datalog solver as a derivation
emitting `BorrowResult` records.

Recommendation: defer `loan_live_at`-style intermediate facts to
semachk. MVP sema holds the *obligation* (body to be
borrow-checked) and the *outcome* (yes/no/errors); intermediate
datalog relations are cache, not canon. Promoting them to canon
is a small move later: add record kinds, add a derivation rule.

**Limits:** polonius is research-grade; rustc hasn't fully
adopted it in stable codegen and its performance on large
programs is known-rough. Borrowing polonius's *schemas* is
fine; treating polonius-the-solver as a drop-in dependency for
a daemon with deterministic-latency goals is optimistic. Plan
to own the datalog runtime.

## Part 4 — salsa vs records-as-state

Salsa's model: `#[salsa::tracked]` functions. The framework
records dependencies by intercepting calls to other tracked
functions. Invalidation is by input-version bumps; recompute
is demand-driven; the cache is in-memory, process-lifetime.

Our model: **derivations are records**. A `DerivedFrom`
sidecar on the output record names the inputs. Invalidation is
by input record's hash changing (content-addressed, so any
change ⇒ new ID ⇒ dependent derivations are trivially stale).
Recompute is still demand-driven (criomed runs a rule when a
consumer asks for the output) but the result is a record
written to redb, not a memo entry.

### Can the r-a dependency graph be encoded as records?

Yes, shape-for-shape. Every salsa tracked function becomes a
*derivation rule record*; every salsa input becomes a
*primitive-mutation record* (or a record ingested from outside);
every tracked function's inputs become `DerivedFrom` edges.
r-a's `body_query(FunctionId) -> Body` becomes a rule
"body-of(Fn) → Block" whose output is a `Block` record with a
`DerivedFrom: [Fn's content hash, …]` sidecar.

One structural difference worth naming: salsa tracks
*query-function* dependencies (which function called which with
which args). Our records track *data* dependencies (which input
record IDs produced which output record ID). Because
content-addressed IDs change on any value change, the two are
equivalent up to granularity — but sema is coarser when a
derivation uses only part of a big input record (e.g., only the
signature of a `Fn`, not its body). That coarseness causes
spurious invalidation unless we split the record ("signature" is
a separate content-addressed record from "body"). This is a
recurring sema design pressure, not a bug: wherever salsa would
have a narrow query, sema wants a narrower record.

### Performance and rehydration

Salsa is in-memory hash-map lookup (hundreds of nanoseconds);
redb with `mmap` is a B-tree descent on a warm page cache (low
microseconds). 10–100× constant-factor gap at the lookup.

That gap matters less than it sounds. r-a's user-facing
latency is bounded by *recompute* cost, not lookup cost; on a
warm cache the recompute set after an edit is small, and
that's true for sema too. The headline difference is
**rehydration**: a fresh criomed on a populated sema directory
starts already as fast as a warm r-a process, because the
"warm" state is persisted — no reindex, no macro re-expansion,
no `cargo metadata` round-trip; `Fn`, `TraitImpl`,
`InferenceResult`, `TraitResolution` records are already
there. r-a explicitly isn't built for this; salsa persistence
has been experimented with (`salsa-persist`, the "persistent
caches" thread) but nothing shipped.

## Part 5 — what sema+semachk should borrow structurally

**From r-a:** the `hir-def`/`hir-ty` split (item structure,
name resolution, body inference are three invalidation layers);
the `Body` + `ExprId` local-addressing pattern (sub-records
inside a function body need IDs); the "body edit doesn't
invalidate signature" rule, wired explicitly into sema's
`DerivedFrom` dependency logic; `InferenceResult`-shaped
records (per-body maps from expr-id to resolved type, from
method-call-expr to resolved impl, from path-expr to resolved
def).

**From chalk:** `ProgramClause` as a derived record kind
generated from `TraitImpl` at derivation time; canonicalisation
of `TraitObligation` before caching; the
`Unique`/`Ambiguous`/`NoSolution`/`Overflow` tetrachotomy in
`TraitResolution`; universes / skolem discipline in obligation
records.

**From polonius:** input-facts-as-data (`LoanIssuedAt`,
`PathMovedAt`) as a *post-MVP* semachk subsystem, not MVP sema;
datalog-as-derivation framing (borrow check is rules over input
facts producing output facts); MIR-point-keyed addressing with
spans as sidecars.

**From salsa:** demand-driven recompute discipline; the
*firewall* pattern (intermediate derived records that stabilise
cheap-to-recompute aggregates so upstream churn doesn't
propagate — in records terms, `FnSignature` is a separate
record from `Fn` because many consumers only need the
signature). We do *not* borrow salsa's runtime: Rust-specific,
in-memory, and tied to macro-generated query structs. Concepts
transfer; code does not.

## Part 6 — what sema+semachk should not try to borrow

**Text-level from r-a:** `SyntaxNode`, `SourceFile`, rowan
red-trees, `TextRange`, `TextSize`. Sema never sees these;
they live behind the rsc projection at most (rsc emits a
throwaway span table so rustc diagnostics can be retranslated).
Macro expansion as an r-a subsystem: nexus users write
already-expanded records; a `.rs` ingester — if we build one —
does expansion via `syn` + proc-macro-server at the ingest
boundary and emits records. No `MacroFile` / `HirFileId` /
`MacroCallId` in sema. File-change invalidation: sema has no
files; record-hash equality is the only invalidation primitive.

**LSP-shaped from r-a:** Range-indexed diagnostics — sema
diagnostics reference record IDs, spans attach at the rsc
boundary. Editor-responsiveness tradeoffs — r-a returns
`Ambiguous` / partial results to keep completion latency
bounded; criomed's obligation is reproducibility, so "partial"
becomes `TraitResolution { status: Overflow }`, not silent
best-effort. The `analysis::Cancelled` cooperative-cancellation
model: criomed cascades run to fixpoint; they're not racing a
typing human.

**Cargo-build-graph:** `Cargo.lock` as input-of-record — sema's
record of a build is `LockSet`; `Cargo.lock` is an rsc
projection. `target/` as oracle — MVP uses rustc-as-derivation
*outcomes* as sema records; artefacts are blobs in lojixd or
discarded scratch. Feature-resolution as a whole-workspace
pass — that's cargo's artefact; sema holds feature selections
per `CompileRequest` record.

## Part 7 — the sharp question: is sema+semachk "r-a without the text layer"?

Honest answer: **structurally, semachk is substantially
"r-a's back half with content-hash IDs replacing salsa-interned
IDs, and redb replacing the in-memory query cache"**. That's a
narrower and more defensible claim than "sema is r-a without
text".

Three things are *not* r-a-shaped:

1. **Sema's schema is primary.** In r-a, `hir-def` types are
   Rust structs; they have no cross-process identity. In sema,
   record schemas are themselves records in `nexus-schema`;
   they have identity and evolve with migrations. This is
   closer to Datomic than to r-a.
2. **Sema is multi-language-ready.** The Rust back-half in r-a
   is Rust-specific because HIR is Rust-specific. Sema's Rust
   records are *one surface* of a substrate intended to also
   host nexus's own records, criomed's own records, lojixd's
   records. The substrate is not Rust-shaped; the Rust *skin*
   over it is.
3. **Sema's derivations are first-class records too.** In r-a,
   queries are Rust functions; their *existence* is not data.
   In sema, rules are records in the store; a new rule is
   added by writing a record, not by recompiling the daemon.
   This is the real departure from both salsa and chalk.

With those three caveats, the implementation strategy "reuse
r-a's `hir-def` + `hir-ty` machinery behind an adapter that
swaps interned-ID for content-hash-ID and swaps salsa for our
`DerivedFrom`-based cache" is plausible for semachk. It would
not give us sema — sema is the storage substrate, which r-a
has no analogue for — but it would give us the *type-checking
subsystem of semachk* on a far shorter schedule than writing
name resolution + trait solving + method resolution + coercion
+ inference from scratch.

Counter-argument: `hir-ty` has deep assumptions about salsa's
cycle detection and about `chalk-ir` being the term language.
Swapping the cache is doable; swapping the term language means
rewriting `hir-ty`. So the decision: design nexus-schema's
`TraitObligation` / `TraitResolution` / `ProgramClause`
records at `chalk-ir`'s shape, content-addressed. That keeps
the eventual adapter thin. Each sema-specific divergence from
`chalk-ir` pays for itself in adapter cost; write down each
one deliberately.

---

*End report 029.*
