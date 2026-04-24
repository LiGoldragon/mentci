# 027 — adversarial review of 026 (sema-is-code-as-logic)

*Claude Opus 4.7 / 2026-04-24 · red-team pass on
[reports/026](026-sema-is-code-as-logic.md). Not a rewrite —
a catalog of weaknesses, unverified claims, and
under-specified bits. Where 026 is genuinely shaky, I say so.*

---

## Scope and method

026 is a synthesis report that corrects the text-layer
contamination from 023/024/025 by asserting: **sema holds code
as already-parsed, already-name-resolved logical records; text
is never an input; a whole class of rustc errors vanishes
because references are content-hash IDs validated at mutation
time.** This review pressure-tests that claim against the
current `nexus-schema` code, the Rust language's actual shape,
and the engineering tasks 026 hand-waves away.

Findings are grouped under the 10 questions in the task, plus
three additional concerns the review surfaced.

---

## 1 · Identity and mutability — the core paradox

**Concrete problem.** 026 insists that *"A reference is a hash;
if the hash points at a record of the right kind, it's
resolved"* (§2, 023-correction bullet). But sema also hosts
editable code: `(Mutate (Fn resolve_pattern …))` replaces the
`Fn`. Under content addressing, mutation doesn't edit — it
creates a *new* record with a *new* hash. The old `Fn` still
exists under its old hash. Every caller that previously stored
the old `FnId` now points at a stale-but-valid record.

The report acknowledges this indirectly in §3 ("the current-state
pointer for that `FnId`'s name swings") but never reconciles two
irreconcilable claims:

- **Claim A**: identity is the hash of content.
- **Claim B**: mutation is edit; callers see the change.

These require a **mutable name→current-hash layer** on top of
the content-addressed layer. That layer does exist in
`docs/architecture.md §3` ("name→root-hash table, git-refs
analogue") — but 026 never threads it through the "references
are content-hash IDs" slogan. So there is in fact a second kind
of reference in sema: *name-refs* (resolved via the mutable
table) alongside *hash-refs* (direct content pointers). 026
papers this over.

**Worse**, the current `nexus-schema` code makes the situation
concrete: `Type::Named(TypeName)`, `TypeApplication.constructor:
TypeName`, `TraitBound.trait_name: TraitName`, and
`Import.names: Vec<TypeName>` all store *string names*, not
hashes. The code as it stands does not satisfy 026's invariant.
Either 026 is describing an aspirational future schema, or it's
wrong about what's implemented. Either way, the report should
not be read as "we're already there."

**Mutually-recursive `f` ↔ `g`.** 026 simply does not address
this. Content-hashing a pair of records that cite each other
requires either (a) breaking the cycle via a name/symbol (then
it isn't "references are hashes" anymore), or (b) computing a
fixed-point hash (a research problem — Unison solves it by
typing the SCC as a unit and hashing the whole component; then
individual function identity is a *projection* of the component
hash, not a standalone hash). The Unison precedent 026 cites
(§2) is the right direction but 026 does not adopt the
machinery — cycles are simply never mentioned.

**Best-guess answer.** Sema has two reference modes: (1)
*structural-hash refs* for leaves and DAGs (types, Fn bodies,
Exprs — already written this way in the code); (2) *symbolic
name refs* resolved against a mutable opus-level symbol table,
for call sites, type references, and anything cyclic. The "whole
class of rustc errors vanishes" claim is therefore *weaker than
026 states*: only refs of mode (1) are validated at mutation
time. Mode (2) can be unresolved or stale, exactly like in rustc.

**Confidence in 026's current answer: low.** This is the single
biggest hole. The report oversells what the invariant buys us.

---

## 2 · Ingester correctness — "weekend tool" is wishful

**Concrete problem.** 026 §2 (024-correction) describes the
ingester as: *"walks the workspace's .rs files. For each file,
it uses syn (or rustc_parse) to parse the text to a Rust AST in
the tool's memory. It translates that AST to nexus-schema
records, resolving names to content-hash IDs during the
translation."* The load-bearing verb is *translates*. That verb
is doing enormous work.

- **Name resolution is not a translator's job; it is half of
  rustc's frontend.** `foo::Bar::baz()` could be a module path,
  an associated function, a trait method via UFCS, a qualified
  path with type ascription, or shadowed by a local binding.
  Resolving requires implementing rustc's name-resolution
  algorithm including visibility, `use` statements, prelude
  injection, glob imports, `_` imports, macro-expanded imports,
  and shadowing rules. rust-analyzer's `hir-def` is ~40 KLOC and
  still has known bugs.

- **Macros.** The report says (§6, Q4) *"Users write
  already-expanded nexus syntax"* and macros expand only at
  ingest. But the workspace itself uses `vec!`, `format!`,
  `matches!`, `derive(Serialize)`, `cfg_attr` — all of which
  must be expanded before records can be emitted. `macro_rules!`
  expansion is implementable (100s-1000s of lines; rust-analyzer
  does it). Proc macros are worse: they require spawning a
  proc-macro host process (rustc does this; rust-analyzer does
  this; it is nontrivial), or accepting that ingestion skips them.

- **External crates.** `use serde::Deserialize;` — does the
  ingester recursively ingest `serde`? Every transitive
  dependency? All of `std`? If not, and name-res needs to know
  whether `Deserialize` is a trait (to type-check later), where
  does that knowledge live? 026 mentions no `ExternalCrate`
  record kind. The answer is probably *"external references
  stay as `Named(TypeName)` with no hash"*, but that
  reintroduces mode-(2) refs (see §1) and silently concedes
  that the "can't have bad references" invariant doesn't hold
  across crate boundaries.

**Best-guess answer.** The ingester for self-hosting specifically
is feasible because the engine's own code is small, macro-light,
and its deps are pinned — but only for *that* scope. As a
general "bring in external Rust" tool, it is a multi-month
project and is not pre-MVP scope. The report should say so.

**Confidence: medium-low.** 026 is too glib about ingest.

---

## 3 · Edit UX — nexus syntax for a 500-record function body

**Concrete problem.** 026 says the user edits by sending
`(Mutate (Fn resolve_pattern { body: (Block …)}))`. Yes, the
body is a record tree. But a function with 30 statements, 5
nested match arms, closures, and method chains decomposes into
hundreds of `Expr` / `Statement` / `Pattern` records. A typical
`resolve_pattern` body might be ~80 records. The user is not
going to hand-type that. So *how does the edit surface actually
work?*

Options the report does not explore:

- **a) Only-mutate-at-top**: users always `(Mutate (Fn name
  { body: <full-body> }))` — they type or paste the whole new
  body as a nexus record tree. Works for small changes via text
  paste from an editor that round-trips, poorly for
  hand-editing.
- **b) Path-based patch**: users mutate a sub-record via
  `(Mutate (Path (Fn resolve_pattern)/body/stmt[3]/rhs)
  <new-expr>)`. 026 mentions no such verb. The nexus grammar in
  013 doesn't define one either.
- **c) Edit via projection**: user asks rsc to project to `.rs`,
  edits text, re-ingests. This is exactly the rust-analyzer
  model that 026 rejects — but it's the only plausible
  developer UX for non-trivial edits today. The report does not
  concede that hybrid flow even as an MVP concession.

**Comments, formatting, doc-strings.** 026 never addresses
them. rsc "projects records → text"; if records don't carry
comments, the projection loses every comment the user ever
wrote, irreversibly. If records *do* carry them, that's a new
record kind (`Doc`, `LineComment`, `BlockComment`) and a schema
extension the report neither enumerates nor specs. Same for
formatting preferences (line width, attr placement).

**LLMs.** 026 implies LLMs emit nexus record trees directly.
Today's LLMs are trained on Rust text. Asking an LLM to produce
"fully-expanded, name-resolved nexus records" for a non-trivial
Rust change is not in distribution. In practice LLMs will
produce Rust text and ingestion will happen — which means
ingest must be robust in the loop, not just at bootstrap.

**Best-guess answer.** MVP edit UX is the hybrid: users edit
text (in their editor, via LLM, etc.), rsc projects and the
ingester re-ingests on save. Nexus record-level edits are for
programmatic callers (other daemons, surgical refactors). 026
should say so instead of implying the record-level edit is the
primary flow.

**Confidence: low.** The report's edit-flow story is the most
under-specified part.

---

## 4 · Diagnostic spans — span tables don't cover rustc's shape

**Concrete problem.** 026 §4f step 9 says criomed joins rustc's
JSON diagnostics with rsc's span table to produce
`Diagnostic { site: RecordId }`. Real rustc diagnostics have:

- A **primary span** (one rustc `Span`).
- **Zero or more secondary labels** each with a span and a
  note.
- **Notes** that may or may not have spans ("consider adding a
  `use` statement").
- **Suggestions** with *rewrite spans* and replacement text
  (rustfix consumes these).
- **Macro backtrace** — the chain of expansions that led to
  the final token.
- **Compound spans** that cross record boundaries (e.g., a
  mismatched-types error pointing at both the function's
  declared return type AND the `return` expression body).

Each of those spans must translate to a `RecordId`. rsc's span
table is `(record_id → byte_range)`. Rustc emits `(file,
byte_range)`. A single diagnostic has up to a dozen spans. None
of this is mentioned in 026; the section says "each diagnostic
carries a source-span" (singular), which is factually wrong
about rustc's JSON shape.

**Worse**, "expected `(`" style messages reference *source
text* the user never typed. A `Diagnostic` pointing at the
`Call` expression's `callee` RecordId is informative only if
the user understands that `callee` is the field at fault. That
requires translating rustc's *message prose* as well as its
spans, which is a much harder NLP problem and is entirely not
in scope for any MVP.

**Suggestions (rustfix)** are even worse: rustfix's
rewrite-spans often cross record boundaries (insert a `.await`
between an expression and a method call — one byte range, two
records). Translating these into record-level mutations is a
research project.

**Best-guess answer.** MVP accepts `Diagnostic` as a
**pass-through string-plus-span-list** with best-effort
RecordId attribution for primary spans only. Secondary labels,
suggestions, and macro backtraces stay as opaque strings
referencing byte ranges in the ephemeral rsc output. Users read
diagnostics in editors with rsc-materialised text loaded. The
"Diagnostic.site is a RecordId" slogan is aspirational.

**Confidence: low.** 026 oversells what span tables buy.

---

## 5 · Non-Rust stuff in the workspace — totally unanswered

**Concrete problem.** The workspace includes, minimum:

- `Cargo.toml` (workspace + per-crate)
- `Cargo.lock`
- `flake.nix`, `flake.lock`
- `rust-toolchain.toml`
- `.gitignore`, `.mailmap`, etc.
- `build.rs` scripts (proc-macro crates frequently use them)
- `tests/`, `examples/`, `benches/` (integration tests — each
  its own crate root)
- Doctests in `///` comments
- `README.md`, `LICENSE.md`, architecture docs
- Non-Rust assets (GoldenFiles, test fixtures, images)

026 mentions *none* of these except a nod to `Opus` covering
"pure-Rust artifact specification." `Opus` in
`docs/architecture.md §5` has toolchain pins and features, but
it was designed as a nix-like spec, not a replacement for
`Cargo.toml`'s full surface (`[workspace]`, `[patch]`,
`[features]`, `[profile.release.package.foo]`,
`[target.'cfg(unix)'.dependencies]`, env overrides). Making
Opus record-shape complete enough to replace `Cargo.toml` is a
substantial schema exercise that is orthogonal to 026 and
invisible in it.

**Doctests** deserve a specific call-out: they are Rust source
inside doc comments that rustc compiles and runs. If records
don't hold doc comments (026 never says they do), doctests
don't survive round-tripping.

**README/docs.** If rsc emits `.rs` files but not `.md`, the
workspace after ingest/project cycle is missing every `.md`
file. The self-host loop in 026 §5 implicitly assumes the
workspace is just `.rs`.

**Best-guess answer.** Sema needs a `FileAttachment` /
`OpaqueWorkspaceFile` record kind (blob-hash + path) for
everything that isn't structured Rust. rsc writes these
back during projection. 025 §8 gestures at this ("non-Rust
worlds — blobs, file attachments"), but 026 drops it.

**Confidence: low-to-medium.** The report simply doesn't
engage with this surface.

---

## 6 · Cascade cost — no bound, no guardrail

**Concrete problem.** 026's cascade story is a two-paragraph
handwave: "the cascade re-derives analyses that reference that
`FnId`." The report never quantifies:

- **Fan-out of a field rename.** Rename `foo.bar` → `foo.baz`.
  Every `FieldAccess` record with `field: FieldName("bar")`
  *that resolves to the renamed field* must mutate. Since the
  current schema resolves by name (not hash), every access site
  in every function body in the opus must be scanned, typed,
  and selectively rewritten. That's a full-opus walk per
  rename.

- **Coherence is opus-global.** Adding a new `impl Trait for
  Type` invalidates every obligation resolution that previously
  concluded "no impl found" and was ambiguous. In rustc this
  re-runs coherence over the whole crate graph. 026 says nothing
  about how sema bounds this.

- **Incremental correctness under retract.** `Retract` of a
  record may falsify derived analyses. What re-runs? Everything
  transitively? The cascade machinery is a research project
  (DBSP/differential dataflow, per 022) but 026 just says "it
  cascades."

- **Live subscriptions amplify.** 025 §5 describes
  subscriptions that fire on every change. A pathological
  editor tool can hold 1000 subscriptions; a single Fn mutation
  can fire all of them. No rate-limiting, no coalescing story.

**Best-guess answer.** MVP implements cascades as *full
re-run of the affected analyses, keyed by the changed record's
upstream hash*. Invalidation is coarse: any analysis whose
inputs include the changed record re-runs. Fine-grained
differential evaluation is a post-MVP project. 026 should name
this trade-off.

**Confidence: low.** 026 claims "incremental" but does not
prove or bound it.

---

## 7 · Rules as records vs rules compiled-in — the bootstrap paradox

**Concrete problem.** 024 (and per 026's §2 nod, still) says
"genesis rules are hardcoded in criomed." 026 also says (§5)
post-MVP semachk's rules are records in sema, "extensible at
runtime without recompiling criomed." These are inconsistent
unless the report specifies *a migration path* from hardcoded
to record-based — which it doesn't.

Specific holes:

- **Can a user retract a rule?** If rules are records, `Retract`
  must apply to them. 026 does not say whether genesis rules
  have a special "protected" bit. If they don't, a malicious or
  buggy mutation can brick the engine. If they do, rule-records
  and other records are not symmetric — the records-as-rules
  slogan is partially false.

- **Version skew between compiled-in seed and stored rules.**
  Restart criomed v2 over a sema DB populated by criomed v1.
  v1's rules are now in the DB. v2's seed rules differ. What
  happens? Report 026 doesn't address this; §8 Q5 ("semachk and
  chicken-and-egg") gestures at the compiler-in-the-loop story
  but not the rule-in-the-DB story.

- **Rule-set content-hashing.** If the full rule-set should be
  reproducible, we need a hash of the rule-set as of a given
  sema rev, tied to criomed version. 026 has nothing here. This
  is load-bearing for any "CompilesCleanly" cache invalidation.

**Best-guess answer.** MVP: rules are compiled-in, rule IDs are
opaque, no user `Retract` of rules, no rule records in sema.
Post-MVP: rules move to records behind a protected-namespace
convention. The path is exactly what semachk-is-a-subagent
implies — but 026 doesn't lay it out.

**Confidence: medium-low.** Plausible resolution; 026 does not
reach it.

---

## 8 · Performance reality — unquantified optimism

**Concrete problem.** 026 says nothing about performance.
Work-the-numbers check on its own premises:

- **Record count.** The self-host loop mentions the engine's own
  source as records. Current `mentci-next` workspace is ~10
  crates, growing. If a medium crate is 20 modules × 10 items ×
  (avg 50 sub-records each) = 10k records per crate, the
  engine's own DB is 100k–500k records at self-host time.
  Larger workspaces (a distro? a company monorepo?) go to 10M+.

- **Redb under blake3 key.** Every record read is a blake3-keyed
  B-tree lookup. mmap-zero-copy via rkyv helps reads; writes
  are the other direction. A single `Mutate` of a deep
  expression tree allocates N new records (every ancestor on
  the path mutates), all blake3-hashed and stored. No
  measurement, no budget.

- **Dedup realism.** 026 implicitly assumes
  structural-identical sub-expressions dedup. In practice, Rust
  bodies differ in every detail (different bindings, different
  types inferred); the *pure-tree* sub-expression dedup rate is
  probably <5% outside of type records and trait refs. The
  "buys us a lot" claim in report 004 is overstated.

- **rkyv write path.** rkyv is optimised for read; writes go
  through the `serialize` path which is fine but not free.
  Compared with bincode/postcard the ratio is closer than 026
  would suggest.

**Best-guess answer.** MVP's performance is an open question;
the report should say "TBD, measure on self-host" rather than
"zero-copy reads throughout and content-hash dedup."

**Confidence: low.** No numbers, no measurement.

---

## 9 · Version skew — hard error is not a plan

**Concrete problem.** 026 §6/§8 gesture at version-skew:
"hard error." That's not an answer, it's a failure mode.
Concrete cases:

- **Schema skew.** criomed v2 has a new `Fn` variant (say,
  `async_fn: bool`). v1's stored `Fn` records lack it. rkyv's
  archived layout depends on the struct — adding a field
  changes hash and layout. Either v2 migrates the DB on open,
  or it refuses to open. 026 never names the migration story.
  The `docs/architecture.md §8` rule "no backward compat" is
  only viable at pre-MVP; once a user has records they care
  about, this rule bites.

- **Toolchain pin skew.** rsc projects records → `.rs` in a
  syntax. Rust 2024 edition emits `let_chains` differently from
  2021. If the Opus's `RustToolchainPin` changes, rsc's output
  must change. Does rsc take a toolchain parameter? `Opus`
  currently has `RustToolchainPin` (architecture.md §5), so
  yes — but rsc needs to dispatch on it. 026 doesn't mention
  this.

- **Cache-hash stability.** `CompilesCleanly(opus,
  input_closure_hash)` is a cache. If the rule-set changes, the
  cache is invalid. If rsc's projection changes, the cache is
  invalid. If rustc version changes, the cache is invalid.
  What's actually in the cache key? 026 says `input_closure_hash`
  but doesn't define closure.

**Best-guess answer.** Cache key must include: opus-hash,
rule-set-version, rsc-version, rustc-toolchain-pin,
criomed-version (insofar as criomed contains native typeck
logic). The cache-key surface is as complex as Bazel's. 026
treats it as a one-field compound.

**Confidence: low.** 026 is too brief here.

---

## 10 · The "semachk" subsystem — a hand-wave named

**Concrete problem.** 026 §4f says semachk is "a subagent
inside criomed" that reimplements type/trait/borrow checking
against sema records and whose rules are records. Elsewhere
(§8 Q5) it says semachk is itself written in Rust and
self-hosted. Let's stack that up:

- Reimplementing rust-analyzer's `hir-ty` + chalk + polonius
  against sema records is several team-years of work. 023 Part
  2 says this explicitly; 026 nods at it then declares semachk
  a natural extension.

- Rules-as-records for type inference means the unification
  algorithm reads unification rules from sema at runtime.
  That's essentially a *metacircular evaluator* — the rules
  describing Rust's type system live in the same store as the
  Rust code being checked. If the code being checked *is* the
  type-checker, we have a bootstrapping problem of the
  chicken-and-egg kind that 026 §8 Q5 waves at but does not
  solve.

- "Extensible at runtime without recompiling criomed" is
  plausibly a research goal; it is not MVP-compatible. The
  report would benefit from flagging semachk as a multi-year
  arc, not a post-MVP item.

**Best-guess answer.** Semachk MVP is *nothing*; rustc is the
checker. Semachk v1 is a tiny subset: schema-validity + ref-
validity + very-limited trait resolution against a fixed,
compiled-in clause set. Rules-as-records is a research aim.

**Confidence: medium.** 026 is directionally sensible,
practically under-scoped.

---

## 11 · Additional concern — the ingester's place in the
architecture

026 §3 "Layer 5" says the ingester is a "bootstrap-only"
sibling binary. But §6 concedes it runs whenever external code
comes in. And §3 under 024-correction concedes LLMs emit text
(implicitly) so ingest happens on every AI workflow. Therefore
the ingester is not bootstrap-only — it is a **continuous
text→records translator on the hot path**. That makes it a
daemon-class concern, not a tool-class one. The report should
probably name it `ingestd` (or fold into nexusd) and own it at
the architecture layer, not treat it as a weekend utility.

**Confidence in 026's framing: low.** It underestimates the
ingester's footprint.

---

## 12 · Additional concern — rsc as a lossy inverse

026 §1 says *"rsc (records → .rs projector; lossy direction:
records carry more than text; text can always be regenerated)"*.
That is probably backwards. Records *today* in `nexus-schema`
carry no comments, no docstrings, no formatting hints, no
originalnames-before-resolution. Rust text carries all of
those. Round-tripping `text → records → text` loses comments
and formatting. The lossy direction is *into records*, not out.

This matters for the self-host loop: if ingestion loses
comments, the engine's own source degrades every time it
round-trips. Over N self-host cycles, all documentation evaporates
unless the ingester preserves it — which the report does not
say it does.

**Best-guess answer.** Records must carry opaque trivia (doc,
comment, whitespace) as side-channel annotations. This is a
non-trivial schema extension. 026 should either specify it or
concede the loop-degrades-comments problem.

**Confidence: low.** Directionally wrong in 026.

---

## 13 · Additional concern — code-at-rest vs code-in-edit
symmetry

026 frames the engine as always having "fully-specified logic."
But during edit, a user's record tree is in-flight and may be
ill-formed (half-typed, referencing a not-yet-created `TypeId`).
What is the state model here?

- Option a: mutations are atomic. A mutation is either fully
  well-formed or rejected. No "draft" state.
- Option b: there's a draft / working-set concept, diverging
  from committed sema.
- Option c: criomed accepts partial records with placeholders
  and re-runs checks as more records arrive.

Option (a) makes interactive editing essentially impossible
(you can't have a "typing in progress" state); option (b) means
sema has a two-layer identity model (draft vs committed);
option (c) reintroduces "unresolved references" that 026
specifically claims don't exist.

026 does not pick an option. `docs/architecture.md §5` mentions
`Revision / Assertion / Commit` in passing (via 025 §4). Without
a concrete edit-session model, the "sema is the truth" slogan
is incomplete.

**Confidence: low.** Report doesn't address this at all.

---

## Summary verdict

026 is **directionally sharp** — "records are logic, not text,
text crosses at nexusd and rsc only" is a clean invariant that
unblocks much of the downstream thinking. It correctly supersedes
023/024/025's text-contamination.

But on the load-bearing specifics, the report is **consistently
optimistic**:

| Area | Verdict |
|---|---|
| §1 Identity/mutability | Genuine paradox, unresolved |
| §2 Ingester | Underestimates by 1–2 orders of magnitude |
| §3 Edit UX | Primary flow unspecified |
| §4 Diagnostics | Overpromises what span tables deliver |
| §5 Non-Rust | Blind spot |
| §6 Cascade cost | Unquantified |
| §7 Rules-as-records | Migration path missing |
| §8 Performance | Not engaged |
| §9 Version skew | Under-specified cache key |
| §10 semachk | Research project presented as post-MVP |
| §11 Ingester placement | Architecturally mis-assigned |
| §12 rsc lossiness | Likely inverted |
| §13 Edit-session state | Missing |

The most urgent correction: **acknowledge that sema has two
reference modes** (hash-refs and name-refs) and redraft the
"whole class of rustc errors vanishes" claim to apply only to
hash-refs. The current `nexus-schema` code uses name-refs
heavily, so the claim as written is not only under-specified
but contradicted by the codebase.

The second-most-urgent: **the ingester is not a bootstrap
tool**; it is a continuous-mode component and needs daemon-
class thinking.

The third: **edit UX**. Without a usable edit surface, "sema is
the truth" is a slogan, not a user-facing system.

026 is worth keeping as a framing document; it is not yet a
design that has survived contact with the implementation.

---

*End report 027.*
