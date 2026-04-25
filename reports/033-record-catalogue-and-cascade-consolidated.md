# 033 — record-kind catalogue, compile story, cascade walkthrough — consolidated

*Claude Opus 4.7 / 2026-04-24 · Rewrite of the still-useful content from
deleted reports 023 (sema-as-rust-checker), 024 (self-hosting cascade
walkthrough), 025 (schema inventory), corrected to the code-as-logic
framing of report 026 and the lojix-store-as-content-addressed-filesystem
framing of `docs/architecture.md` §3. Produces the MVP record-kind
catalogue, compile-validity story, cascade walkthrough, and remaining
open questions. Treats architecture.md and 026 as canonical and avoids
repeating either; cites inline instead.*

---

## Part 1 — Framework re-statement

**Sema — code as logic.** Sema is the authoritative representation
of code as fully-specified, schema-bound logical records: `Fn`,
`Struct`, `Enum`, `Module`, `Program`, `Expr`, `Statement`,
`Pattern`, `Type`, `Signature`, `TraitDecl`, `TraitImpl`, and the
sub-record pieces `Field`, `Variant`, `Method`, `Param`, `Origin`,
`Visibility`, `Import`, `Newtype`, `Const`. References between
records are content-hash `RecordId`s, not names. Text is not an
input — rsc emits text *from* records only when rustc needs bytes
to read. See report 026 for the full statement and report 004 for
the original type shapes.

**Lojix-store — content-addressed filesystem.** A nix-store
analogue, keyed by blake3. It holds real unix files and directory
trees on disk under hash-derived paths. A compiled Rust binary is
a real executable at e.g. `~/.lojix/store/<hash>/bin/foo`; you
`exec` it directly — there is no extraction step, no copy, no
`Launch` protocol verb. A separate index database (lojixd-owned,
likely redb) maps `blake3 → path + metadata + reachability`; the
index is bookkeeping, it does not contain the bytes. See
`docs/architecture.md` §3 for the full spec.

**Three daemons.** criomed owns sema + machina-chk + subscription
delivery + capability enforcement; lojixd owns lojix-store + every
filesystem-touching / process-spawning verb (cargo, rustc,
nix-build, nixos-rebuild, proc-macro sandbox); nexusd is the
session-local stdio bridge for human and agent clients. Text
crosses only at nexusd. See `docs/architecture.md` §1–§4.

**Rustc-as-derivation — the hybrid strategy.** For the MVP, sema
does not re-implement rustc. Instead, `RunCargoPlan` (or the
finer-grained `RunRustcPlan` if we choose) is a derivation
executed by lojixd: rsc projects the record graph to a scratch
`.rs` workdir, lojixd invokes cargo/rustc, the resulting binary
tree is placed into lojix-store under its blake3 hash, and
diagnostics are parsed back into `CompileDiagnostic` records
whose `site: RecordId` fields point at the producing sema
records. This lets us ship with a real borrow-checked Rust
compiler on day one. Post-MVP, machina-chk subsumes cheap phases
(parse, module graph, name resolution) — they are mostly *free*
under code-as-logic because sema already is the resolved
structure — then trait solving (chalk-like), then body-level
typeck (r-a-like). Borrow check stays in rustc indefinitely.

---

## Part 2 — MVP record-kind catalogue

Everything below lives in `nexus-schema` unless noted. The point
of this catalogue is to be the checklist for the first pass of
schema-writing.

### Core code records

Already spec'd in report 004: **Fn, Struct, Enum, Module, Program,
Expr, Statement, Pattern, Type, Signature, TraitDecl, TraitImpl,
Field, Variant, Method, Param, Origin, Visibility, Import,
Newtype, Const.** Crate: `nexus-schema`. No redefinition here —
report 004 is the source of truth and report 026 §1 confirms the
list.

### Opus family — the top-level build/deploy unit

- **Opus** — the top-level aggregate (one per project-like
  deliverable); references a `Program` record plus the
  derivation that compiles it, plus toolchain pins.
- **Derivation** — a referentially-transparent recipe:
  (inputs, builder, args, env) → output-hash. Mirrors nix
  derivations shape but with content-hashed record-graph
  inputs rather than `.drv` text.
- **OpusDep** — a dependency edge between opera (or between an
  opus and an external nix flake / cargo registry entry).
- **RustToolchainPin** — exact rustc/cargo versions, profile
  flags, target list. Exists so a `Derivation` is reproducible.
- **NarHashSri** — SRI-formatted blake3 value as a newtype, used
  wherever a hash appears in a record field (distinct from
  `RecordId`; `RecordId` is sema-internal, `NarHashSri` is
  lojix-store-facing).
- **FlakeRef** — optional, for opera that reference nix flakes.
- **TargetTriple** — a newtype around `String` with validation.

Crate: `nexus-schema`.

### Schema-of-schema — the meta layer

- **StructSchema** — describes a record kind that is struct-shaped.
- **EnumSchema** — describes a record kind that is enum-shaped.
- **FieldSchema** — one field of a struct; carries a `TypeRef`.
- **VariantSchema** — one variant of an enum; zero-or-more
  `FieldSchema` attached.
- **TypeRef** — references another record-kind's schema by
  `RecordId`, or a primitive, or a collection of a `TypeRef`.
- **TypeParam** — for generic schemas (a single monomorph point
  is enough for MVP; type-level computation is post-MVP).

These are themselves records, stored in sema, self-describing
(the schema-of-schema describes its own shape). Bootstrapping
is discussed in Part 5. Crate: `nexus-schema`.

### Rules — logic-programming layer

- **Rule** — a named inference rule with premise(s) and head.
- **RulePremise** — a pattern over records; may bind variables.
- **RuleHead** — the derived record kind + field expressions.
- **DerivedFrom** — a sidecar provenance record asserted whenever
  a rule fires; carries `(derivedRecordId, ruleId, premiseIds)`
  so we can answer "why does this record exist?"

Crate: `nexus-schema`. MVP can ship with rules *disabled* and
still function; they are the Phase-2 delimiter per report 013.

### History — revision and assertion

- **Revision** — a monotonically-numbered sema transaction
  boundary; the unit of subscription watermark.
- **Assertion** — one `Assert`/`Retract` edge inside a Revision:
  `(revisionId, recordId, op, actorId, timestamp)`.
- **Commit** *(optional for MVP)* — a named / signed bundle of
  revisions for share-with-others workflows; not required for
  self-hosting.

Crate: `nexus-schema`. The append-only log of Assertions is the
bitemporal backbone — current state is a materialised view over
it.

### Identity and named refs

Per `docs/architecture.md` §3, these are **named-ref table
entries, not content-hashed records**:

- **OpusRoot** — `("opus-root", opusId) → RecordId` pointing at
  the current head of an opus.
- **Bookmark** *(optional)* — human-named pointers into revisions.
- **WorkingHead** *(optional)* — per-actor cursor for
  work-in-progress lines.

They live in criomed's named-ref table (git-refs analogue). They
are mutable; records are not.

### Plans — intents sent to lojixd

**Renamed from blob/file vocabulary** to reflect lojix-store's
actual nature as a filesystem:

- **RunCargoPlan** — "run cargo on this record-projected workdir;
  return outputs + diagnostics."
- **RunNixPlan** — "build this nix derivation; return the
  store-path."
- **RunNixosRebuildPlan** — "apply this nixos config."
- **PutStoreEntryPlan** — "accept these bytes/this tree and place
  it in lojix-store at its hash-derived path." Replaces the
  wrong `PutBlob` name.
- **GetStorePathPlan** — "resolve this hash to a filesystem
  path." Replaces `GetBlob`.
- **MaterializeFilesPlan** — "copy this store entry into a
  workdir at a requested layout." Used by rsc before cargo runs.
- **DeleteStoreEntryPlan** — GC-path; lojixd executes only when
  criomed confirms the hash is unreachable.

Crate: `nexus-schema` (the plan records themselves) — the wire
contract lives in the `lojix-store-msg` crate per report
015/017, but the plan *record kinds* are sema records.

### Outcomes — evidence that plans ran

- **CompiledBinary** — `(planId, opusId, storeEntryRef,
  revisionId)`; `storeEntryRef` points at the lojix-store path
  where the binary tree lives. The record is the evidence; the
  bytes are on disk.
- **CompileDiagnostic** — a structured diagnostic: severity,
  message, `site: RecordId`, optional `span_within_record`.
  Text line/column is *not* the primary locator.
- **CompilesCleanly** — a boolean-ish assertion keyed by
  `(opusId, revisionId)` stating "0 errors as of this rev."
- **NixBuildOutcome** — analogous to CompiledBinary for nix
  derivations.
- **FailureOutcome** — structured failure envelope
  (cause-class, retry-hint, plan-id).

Crate: `nexus-schema`.

### Analyses — machina-chk's record-level output

- **Obligation** — "this program requires X": a trait bound, a
  type equation, a borrow fact. Emitted by rules (post-MVP) or
  by machina-chk (as machina-chk phases arrive).
- **CoherenceCheck** — trait-impl coherence verdicts.
- **TypeAssignment** *(optional MVP)* — `(exprId → typeId)` when
  machina-chk runs body-level typeck; otherwise typeck results are
  implied by the absence of `CompileDiagnostic`.
- **Diagnostic** — unified severity-bearing record (machina-chk-side
  counterpart to `CompileDiagnostic`).
- **BorrowFacts / BorrowResult** *(deferred)* — polonius-style;
  not MVP.

Crate: `nexus-schema`.

### Subscriptions

- **SubscriptionIntent** — durable: `(actorId, pattern,
  delivery-mode, watermark-revisionId)`. Survives criomed
  restart.
- **Active delivery** — session-local state in criomed's
  subscription-mux actor; re-established when a client
  reconnects. Not a record.

Crate: `nexus-schema` for the durable record; runtime state is
just actor memory.

### Capabilities

- **CapabilityPolicy** — `(principal, verbs, scopes, expiry)`.
- **PrincipalKey** — long-lived identity key; principals are
  content-addressed by their public key.

Crate: `nexus-schema`. Enforcement point is criomed (for sema
verbs) and lojixd (for store verbs, gated by criomed-signed
capability tokens).

### Store references

- **StoreEntryRef** — newtype wrapping `blake3` that semantically
  says "this is a lojix-store handle." Distinct from `RecordId`
  so the type system refuses to confuse the two. Replaces the
  `BlobRef` name from 025.
- **StoredEntry** — sidecar record with metadata about a store
  entry: size, kind-hint (binary / source-tree / attachment),
  mtime-at-ingest, reachability-class. Written by lojixd when
  a `PutStoreEntryPlan` completes; read by criomed's GC.

Crate: `nexus-schema`.

---

## Part 3 — Compile-validity story

**Rustc phase split.** Of rustc's pipeline (lex, parse, macro
expansion, AST lowering, name resolution, HIR, trait solving,
type check, borrow check, MIR, optimisation, codegen), phases
1–10 answer "is this program valid?" and phase 11 (codegen)
produces the artifact. The machina-chk subsystem eventually wants to
subsume some of 1–10 at the record level; codegen is staying in
rustc for the foreseeable future.

**Hybrid MVP.** Sema delegates to rustc-as-derivation:

1. User sends `(Compile opus)`. Criomed constructs a
   `RunCargoPlan` referencing the `Opus` record, its
   `RustToolchainPin`, and the input record graph (closure
   reachable from the `Program`).
2. Criomed forwards the plan to lojixd.
3. Lojixd invokes rsc, which projects each record in the closure
   to a `.rs` file in a scratch workdir (the projection is
   lossy in the easy direction — records carry strictly more
   than text).
4. Lojixd runs cargo with the pinned toolchain. Cargo / rustc
   read the scratch tree, do their 11 phases, and either emit a
   target tree or error output.
5. Lojixd hashes the target tree and writes it into lojix-store
   at `~/.lojix/store/<hash>/` — a real directory with the
   binary at `bin/<name>`.
6. Lojixd returns an outcome. Criomed asserts a `CompiledBinary`
   record (pointing at the store path via `StoreEntryRef`) and
   zero-or-more `CompileDiagnostic` records. Diagnostics get
   `site: RecordId` via rsc's reverse-projection map — because
   rsc emitted the `.rs` text, it knows which record produced
   each span.

**Record-level invariant.** Every analysis verdict — MVP's
rustc-mediated ones, and post-MVP's machina-chk-native ones — is a
record whose `site` fields are `RecordId`s, not `(path, line,
col)`. Text coordinates exist only in the scratch workdir,
which is ephemeral.

**Post-MVP machina-chk migration.** The cheap phases are mostly free
under code-as-logic:

- **Parse / AST lowering** — not needed; sema already holds
  resolved logical records.
- **Name resolution / module graph** — near-free; references are
  already `RecordId`s. Semachk just has to validate that
  `Visibility` rules aren't violated.
- **Macro expansion** — for `derive(…)` proc macros, machina-chk can
  stay in record-land by running the macro inside a lojixd
  sandbox verb and asserting the result back as records. For
  function-like macros, some text detour likely remains.
- **Trait solving** — chalk-like; a real new subsystem with
  meaningful engineering cost.
- **Body-level typeck** — r-a-like; also meaningful cost.
- **Borrow check** — stays in rustc indefinitely.

The migration is incremental: as each machina-chk phase becomes
stable, it takes over from the rustc round-trip for that class
of question. Until the final phase graduates, `RunCargoPlan`
remains the backstop that guarantees real-Rust semantics.

See report 029 for how we expect to vendor r-a crates (chalk-ir,
rustc_parse_format, etc.) rather than rewrite them.

---

## Part 4 — Cascade walkthrough

**Cold start.** Criomed boots with an empty sema. Its loader
actor fires: it hardcodes the schema-of-schema seed records
(StructSchema/EnumSchema and their sub-records describing
themselves), then seeds the Opus/Derivation/OpusDep family and
the initial rule set. These are all asserted into a Revision-0.

At this point criomed is ready to accept nexus requests. The
first Assert / Mutate / Query arrives from a user or LLM via
nexusd → criomed. There is **no** automated ingest of `.rs` —
sema populates only via nexus requests. The engine's own
source accumulates in sema over time as someone (Li, LLM
agents) writes nexus requests to assert the records that
describe the engine itself. The current hand-written `.rs`
in canonical repos is scaffolding that produces the MVP
binary via cargo; over time each crate is rewritten into sema
and rsc's projection supersedes the hand-written text.
(Self-hosting gradient + no-ingester invariant are in
architecture.md §10.)

**Warm edit.** The user (or an agent) sends, over nexusd →
criomed, something like:

- `(Mutate (Fn resolve_pattern { body: (Block …) }))` —
  directly replacing the body of a logical function record.

Criomed asserts the new record, supersedes the old one, and
fires the cascade. Subscriptions keyed on `Fn resolve_pattern`
or any record that transitively depends on it fire. No text is
parsed; no file is touched. The edit is structural.

On an explicit `(Compile opus)`, criomed constructs a
`RunCargoPlan`, forwards it to lojixd, lojixd follows the steps
in Part 3, the resulting binary tree lands in lojix-store, and a
`CompiledBinary` record plus `CompileDiagnostic`s (if any) are
asserted. Any subscriber watching `CompiledBinary { opusId: X
}` fires.

**Close the loop (self-hosting).** The user examines
`CompiledBinary { opusId: criomed }.storeEntryRef` to get a
`blake3`. Criomed (or the user) calls `GetStorePathPlan` on
lojixd to resolve it to `~/.lojix/store/<hash>/bin/criomed`.
The user runs that binary directly — no extraction, no
`Launch` verb, it's just `exec`. The old criomed is killed;
the new one starts on the same sema database and named-ref
table. It opens sema, reads its own source as records,
verifies self-consistency, and the session continues.

**Restart.** On every criomed boot, a fresh `RuntimeIdentity`
record is asserted (so we can tell which runtime instance is
speaking in logs / diagnostics). Durable `SubscriptionIntent`
records are preserved across restarts; session-local active
delivery re-establishes when clients reconnect. Lojix-store is
untouched by a criomed restart (it's lojixd's). The result is
that "restart criomed" feels like a session hiccup, not a
loss of state.

---

## Part 5 — Awkwardness, honestly flagged

**Schema-of-schema bootstrap.** The MVP hardcodes the seed
StructSchema/EnumSchema records in Rust source. The schema-of-
schema describes itself, but you have to *start* somewhere, and
that somewhere is a literal in the criomed loader. Post-MVP
migration records will let the schema evolve without a binary
rebuild; for self-hosting we eat the cost that schema changes
need a compile.

**Rules-as-records bootstrap.** Seed rules are asserted on cold
start, and per report 031 P1.5 they are **immutable from the
signal surface** — you cannot retract or mutate them over
the wire. A mistake in a seed rule requires a code change +
rebuild + restart. This is the right trade for a self-hosting
system: it removes a whole class of "I poisoned my sema with a
rule-loop" failure modes.

**Proc macros.** These run unsandboxed Rust at compile time in
conventional cargo. In our world they run **inside lojixd** as a
sandboxed verb. MVP likely restricts to `derive(…)` (which is
well-behaved and determined) and defers function-like /
attribute macros until a proper sandbox policy is settled.
Results land as records (the expansion of `derive(Debug)` for
`Struct X` becomes sema records for the generated `impl`
block).

**IDE-style queries mid-cascade.** When the cascade is partway
through, criomed still needs to answer reads consistently.
Redb's MVCC snapshots give us atomic reads at a `sema_rev`; the
cascade scratchpad is invisible to readers until it commits.
Subscribers see changes at Revision boundaries, never mid-flight.

**Schema-crate self-referential hazard.** The `nexus-schema`
Rust crate describes the shape of records; criomed's compiled
binary depends on that crate. If sema's stored schema-of-schema
records disagree with the crate the running binary was built
against, criomed must hard-error at open time, not silently
drift. The self-host loop works cleanly only when `nexus-schema`
itself is unchanged from one compile to the next; changing it
is a two-step operation (ingest new schema, build binary that
matches, restart). Report 031 P0 tracks this.

---

## Part 6 — Open questions that survive

The full open-question list lives in report 031. The specific
items that originated in 023/024/025 and are **not** resolved
by this report:

- **Hash-vs-name refs** — resolved by the slot-ref /
  `SlotBinding` model; records store `Slot(u64)` and the index
  holds `slot → { current_content_hash, display_name }`. See
  architecture.md §5.
- **Edit UX** — resolved: nexus requests only (Assert, Mutate,
  Patch, Retract). No text round-trip. See reports/057.
- (remaining open questions — if any — listed below)
  pick one, and has not.
- **Diagnostic translation granularity.** Rustc gives us
  `(path, line, col)` spans; rsc's reverse-projection map lets
  us recover the originating `RecordId`. Sub-record span
  (offset-within-record) is desirable for UX but not obviously
  needed for MVP correctness. Cost-vs-value not settled.

---

*End report 033.*
