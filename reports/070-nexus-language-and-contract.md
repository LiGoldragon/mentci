# 070 — nexus language design + signal contract

*Claude Opus 4.7 · 2026-04-25 · synthesis of four parallel
research agents (edit semantics, query semantics, correctness/
static-checks, prior-art-and-differentiators) plus the
signal contract that bridges nexusd ↔ criomed and carries
the full language. Per Li 2026-04-25: "first we need the logic
that will make nexus the greatest database edit-and-query
language ever made … and we need the contract that will create
this logic in rkyv messages into/from nexus (nexusd role), to
criomed."*

*This is a language-design exercise. The full eventual surface
is in scope; rung-1 subsetting is a later report's concern.*

---

## 1 · The thesis

Nexus is one language with two faces (edit + query) and a
single contract layer (signal). The language is grounded
in three structural choices that compound:

1. **Position-defines-meaning.** No keywords. Records like
   `Sum`, `GroupBy`, `Limit`, `OrderBy`, `Patch`, `Retract`
   are Pascal-named records like any other; their semantic
   role comes from grammatical position, not from being
   reserved.
2. **Delimiter-family matrix.** Outer character picks family
   (`()` records, `{}` composites, `[]` bytes/code, `<>`
   flow); inner pipe-count picks abstraction level (0
   concrete, 1 pattern/abstracted, 2 scoped/committed). One
   uniform rule makes every future feature pre-learnable.
3. **Records as operators.** Aggregation, pagination,
   temporal scoping, projection — all records, applied by
   juxtaposition. No pipeline operator (`|` or `->`); no
   keyword separator. Operators are first-class values.

Plus the architecture-level commitments: slot-refs (not
content-hashes) for cross-record references; sema-is-local
(no global database; instances negotiate); intrinsic
categories; per-kind change logs; capability tokens carried
in messages; criomed as the validation gate (the
hallucination wall).

---

## 2 · Edit semantics

### 2.1 · The edit primitives

Five core verbs:

- **`Assert`** — introduce a new record at a slot. Validator
  runs the six-step pipeline; appends `ChangeLogEntry(Assert,
  None, new_hash)`; updates `SlotBinding`; cascades fire.
- **`Mutate`** — whole-record replacement at an existing
  slot. Single audit entry `ChangeLogEntry(Mutate, old_hash,
  new_hash)`. Not Retract+Assert internally — it preserves
  slot continuity for subscriptions.
- **`Retract`** — remove a record. Validator checks
  outstanding slot-refs from other records; rejects with a
  Diagnostic naming dependents if any. Succeeds → tombstone
  bit on `SlotBinding`; cascades re-derive any
  rule-derived dependents.
- **`Patch`** — surgical field-level edit at a path inside a
  record. Path is a sequence of segments (field name, list
  index, variant selector). Validator type-checks the new
  value against the field's `TypeRef`. One ChangeLogEntry
  per Patch (treated as Mutate with a finer-grained diff).
- **`TxnBatch`** — atomic envelope wrapping a sequence of
  Asserts/Mutates/Retracts/Patches. All-or-nothing; one
  Revision; one redb transaction.

The `~` sigil is shorthand for `Mutate` at the syntax layer
(parsed by nexusd into `MutateOp`). The `!` sigil at top-level
record position is shorthand for `Retract`. Inside a pattern,
`!` is negation. Same character; position-defines-meaning.

### 2.2 · Atomicity model

`{|| ... ||}` is the atomic-transaction delimiter. Inside, all
ops commit together or roll back together. The first error
halts the batch; reply names the failed op index. No partial-
commit / best-effort mode.

```nexus
{||
  (Assert (Struct :slot 50 :name "Point" :fields [...]))
  (Assert (Fn :slot 51 :name "distance" :param-type 50 :body [...]))
||}
```

Cross-record edits (e.g., adding a `Fn` and updating a
`Module` to include it) are author-explicit as two ops in
one batch. Cascade rules (Phase D+) can derive related
updates, but the primary edit stays author-stated.

### 2.3 · Pattern-driven edits — client-side loop

A pattern matches many records; "mutate all matches" is
**client-side, not language-level**. The author runs a Query,
then issues N Mutates inside `{|| ... ||}`. Reasons:

- Mechanism stays transparent — each mutation is its own
  audit entry.
- Nexus is a messaging language, not a workflow engine.
- Atomicity is preserved by the batch wrapper.

If performance later demands server-side batch-mutate, that
lands as a Pascal-named record `(MutateMatching pattern
{ field-changes })` — no new sigil.

### 2.4 · Concurrency

**Optimistic with explicit CAS.** Default: last-write-wins.
Adding `:expected-rev N` to a Mutate/Retract/Patch makes the
op a compare-and-swap — validator rejects if the slot's
current revision doesn't match. Single-writer at criomed
makes this cheap.

```nexus
(Mutate :slot 42 :expected-rev 100 { :field "new-value" })
```

Time-travel edits (asserting "as if it had been at rev N")
are forbidden by design — would invalidate slot-refs and
break causality. Temporal queries are read-only.

### 2.5 · Cross-instance edits — outside nexus

Sema is local. Nexus targets one criomed at a time. Asserting
to a remote criomed is a transport-layer concern (the client
dials a different nexusd). Cross-instance *coordination*
(quorum-signed proposals, federated agreement) is part of
the architecture (Phase 3+), but it's not a nexus syntax
concern — it lives in signal as separate verbs that the
two criomeds exchange.

---

## 3 · Query semantics

### 3.1 · Pattern-and-pipeline shape

A query is a **pattern** followed by zero or more
**operator records**, optionally terminated by a
**projection shape**:

```nexus
<pattern> <op1> <op2> ... <opN> { <projection> }
```

- Pattern: `(| KindName :field-constraint @bind-name |)` —
  1-pipe parenthesis is the pattern abstraction level.
- Operators: Pascal-named records applied in sequence —
  `(Count)`, `(Sum @v)`, `(GroupBy @k (Sum @v))`,
  `(OrderBy (Desc @v))`, `(Limit 50)`, `(Distinct)`.
- Projection: `{ @bind-1 @bind-2 (Sum @v) }` — fields and
  aggregations to return.

Conjunction inside `{| ... |}` joins multiple patterns by
shared bind names (datalog-style):

```nexus
{|
  (| Fn @fn :module @mod |)
  (| Module @mod :visibility "public" |)
|}
{ @fn @mod }
```

`!` negates a pattern; `(|| ... ||)` is optional/LEFT-JOIN
(LEFT-JOIN that succeeds with `None` if no match).

### 3.2 · Operator catalogue

All Pascal-named records in `nexus::query::*` (or wherever
the schema places them):

- **Aggregation**: `Count`, `CountDistinct`, `Sum`, `Min`,
  `Max`, `Avg`.
- **Grouping**: `GroupBy { binds, inner-ops }`, `Having
  pattern`.
- **Ordering / pagination**: `OrderBy [(field, Asc|Desc)]`,
  `Limit N`, `Offset N`, `Top N op`, `Bottom N op`,
  `Before cursor`, `After cursor`.
- **Set operations**: `Distinct`, `DistinctBy @bind`,
  `Project [field-or-bind…]`.
- **Temporal** (Phase 2): `TimeAt rev-ref`, `TimeBetween
  rev-ref rev-ref`, `TimeAll`.
- **Cross-instance** (Phase 3 sketch): `RemoteInstance
  peer-id`.

All compose by nesting (records nest by definition):
`(Top 10 (GroupBy @module (Sum @bytes)))` is one nested
operator-record.

### 3.3 · Recursion + transitive closure

Two paths:

- **Phase 2 — rules** (`[|| head body ||]` form): user
  asserts a `Rule` record; criomed registers it; cascades
  derive transitive facts; subsequent queries pull derived
  records like any others. Stratified (per
  `KindDecl.stratum`) to guarantee termination.
- **Pre-Phase-2** — client-side iteration over forward
  closure. Acceptable for shallow chains.

### 3.4 · Streams / subscriptions — `<| ... |>`

Subscribe to a pattern; criomed streams diffs:

```nexus
<| (| Order @id @amount |) (After @cursor) (Limit 100) |>
```

Server delivers (over the wire) a sequence of:
`SubReady`, `SubSnapshot`, `SubAssert`, `SubMutate`,
`SubRetract`, `SubError`, `SubEnd`. Resumption from a known
revision; cursor-paginated. At-least-once semantics on the
default channel; at-most-once available with a flag (open
question).

### 3.5 · Introspection — schema-as-records

Sema's schema lives in sema as `KindDecl`, `FieldSpec`,
`CategoryDecl` records. Querying them needs no separate
language layer:

```nexus
(| KindDecl @kind :name "Fn" |) { @kind }
(| FieldSpec :kind @k :name "body" |) { @k }
```

This is the genesis-via-self property at the language level:
the language can describe itself in itself.

### 3.6 · Result shape

A query result is a stream of *bindings* (each match yields
a dict of `@name → value`). The `{ }` projection at the end
selects which fields/aggregations to return; if absent, all
binds are returned.

Nested results (for `GroupBy` etc.) are nested records:

```nexus
{ @customer { (Sum @amount) (Count) } }
```

Returns a stream of `{customer: ..., {Sum: ..., Count: ...}}`.

---

## 4 · Correctness story

### 4.1 · Three layers

| Layer | Where | What's caught | Failure mode |
|---|---|---|---|
| Parse | nota-serde-core in nexusd | syntax (delimiter balance, sigil budget, identifier shape, literal form) | `Reply::Rejected` (transient) |
| Validation | criomed's six-step pipeline | schema-mismatch, unresolved slot-refs, invariant violations, unauthorised actions, type errors | `Diagnostic` record in sema (durable) + `Reply::Rejected` summary |
| Execution | criomed's pattern matcher + cascade engine + rsc/rustc | non-linear pattern unification failures, cascade non-termination, rsc/rustc errors | `Diagnostic` record (durable); cascade-failure is diagnostic-only and does not reject the originating mutation |

Diagnostic codes E0000–E9999 are assigned per failure class
(E0000 parse, E0001 schema, E0002 ref, E0003 invariant,
E0004 unauthorised, E0005 expired-proposal, E0006
incomplete-quorum, E0007 invalid-signature, E9999 cascade).

### 4.2 · Schema-name resolution is validation-time

Parse-time has no sema access; the parser cannot know
whether `(Slot 3)` exists. Validation-time step 1 (schema-
check) looks up the kind by name in sema (or against
built-in Rust types during genesis). Mismatch → E0001.

### 4.3 · Reference correctness within transactions

Within `{|| op1 op2 ||}`, slot-refs in `op2` resolve against
*sema as it stood before the transaction*, not against what
`op1` would assert. **Intra-txn forward-refs are not
resolved.** If you need to assert a record and reference it
in the same call, split into two transactions:

```nexus
{|| (Assert (Point :slot 10 :x 1.0)) ||}
{|| (Assert (Circle :slot 11 :radius (Slot 10))) ||}
```

This is a deliberate constraint that simplifies the
validator and makes mechanism explainable. Open question:
relax later if real workflow demand surfaces.

### 4.4 · Pattern correctness

Patterns can be statically checked against `KindDecl` for
shape (does `:body` exist on `Fn`?). Lean: validate at
assertion-time of `Pattern` records (and `Rule` records
that contain patterns). Runtime pattern-evaluation only
emits Diagnostic on truly-dynamic problems (e.g.,
non-linear bind constraint failed during a particular
match).

Pattern linearity (repeated `@x` enforces equality) is a
runtime concern — the matcher unifies binds across
positions; mismatches fail the match silently (no emitted
record).

### 4.5 · Cascade-rule stratification

Stratification per `KindDecl.stratum` is a static check at
rule-assertion time: a rule may only read from strata ≤ its
head's stratum. Cycle detection at assertion time emits a
warning (not a rejection) for obvious cycles; runtime
infinite-loop detection emits `E9999` and times out the
cascade without rejecting the original assertion.

### 4.6 · Validate (dry-run) verb

A `Validate` request runs the full validator pipeline
without committing. Returns `Diagnostic`s + an optional
`ExecutionPlan` (for queries). Useful for editor/LSP
integration: pre-flight check before sending a real edit.

### 4.7 · Static-check tooling

- **Syntax-only**: `nexus-cli parse` (uses only nota-serde-
  core; no daemon).
- **Schema-aware**: `nexus-cli validate` against a cached
  schema snapshot.
- **Live**: `(Validate ...)` request to criomed.

---

## 5 · What nexus does that no one else does

Five distinguishing-by-design properties:

1. **Position-defines-meaning + delimiter-family matrix.** No
   keywords; one uniform abstraction-level rule. Future
   features extend without growing reserved-word lists or
   sigil budget. (SQL fails here; jq/Kusto succeed partially
   with `|`-pipelines but require keywords for operators.)

2. **Records as operators.** `(Sum @v)`, `(GroupBy @k)`,
   `(Limit 50)` are records — storable, queryable,
   serialisable like any other record. (GraphQL has typed
   schema but operators are syntax-keywords; EdgeQL is closer
   but still SQL-influenced.)

3. **Slot-refs separate identity from content.** Records
   reference each other by `Slot(u64)`; renames update the
   binding, not the hash; content edits don't ripple. (SQL
   foreign keys couple identity to PK fields; Datomic
   entity-id mixes identity with history.)

4. **Sema-is-local with rich cross-instance interaction.**
   No global database, no eventual-consistency illusion;
   instances communicate, agree, disagree, negotiate. Content-
   addressing makes record-sharing-by-hash tractable across
   machines. (Most distributed databases bury this in
   protocol layers; nexus exposes it as a first-class
   architectural concern with first-class language support
   coming in Phase 3.)

5. **Genesis-via-self.** The language can describe its own
   schema in itself (`KindDecl` records describe `KindDecl`).
   Bootstrap uses the same nexus-via-nexusd-via-criomed flow
   as any other input. No baked-in-rkyv shortcut, no internal-
   assert path. (Datomic's transaction-fn semantics are
   coded outside the database; SQL's system catalog is a
   separate language.)

---

## 6 · The signal contract

> *Naming note (2026-04-25).* This contract was originally
> drafted as *criome-msg*. Per Li's later naming decision, the
> rkyv-format messaging layer between nexusd and criomed is
> called **signal**. References are renamed throughout; the
> §6 type catalogue is the **signal envelope and payloads**.
> See [reports/077](077-nexus-and-signal.md) for the deep
> analysis.

The wire format that crosses nexusd ↔ criomed (and within
criomed → criomed cluster, sketched). The wire carries a stream
of `Frame` archives; both parties know the `Frame` rkyv schema,
so framing is intrinsic to the schema — the universal handshake
is the frame type itself, and there is no "raw bytes outside
rkyv" layer. Every field type below is rkyv-archivable.

### 6.1 · Frame envelope

```rust
pub struct Frame {
    pub correlation_id: u64,              // request/reply pairing
    pub principal_hint: Option<Slot>,     // who's making this request (Slot → Principal)
    pub auth_proof: Option<AuthProof>,    // signature; None during single-operator MVP
    pub body: Body,
}

pub enum Body { Request(Request), Reply(Reply) }

pub enum AuthProof {
    SingleOperator,                       // accepted only on Unix-socket peer-cred check
    BlsSig { sig: BlsG1, signer: Slot },  // post-MVP single-principal sig
    QuorumProof { committed: Slot },      // post-MVP; refers to CommittedMutation record
}
```

### 6.2 · Edit verbs

```rust
pub enum Request {
    // Edit
    Assert(AssertOp),
    Mutate(MutateOp),
    Retract(RetractOp),
    Patch(PatchOp),
    TxnBatch(TxnBatch),

    // Query
    Query(QueryOp),
    Subscribe(SubscribeOp),
    Unsubscribe { subscription_id: u64 },

    // Read-only
    Validate(ValidateOp),
}

pub struct AssertOp {
    pub record: RawRecord,                // wire form (kinds by name)
    pub assigned_slot: Option<Slot>,      // explicit during genesis; None → criomed mints
    pub expected_rev: Option<Revision>,   // CAS for slot non-existence
}

pub struct MutateOp {
    pub slot: Slot,
    pub new_record: RawRecord,
    pub expected_rev: Option<Revision>,
}

pub struct RetractOp {
    pub slot: Slot,
    pub expected_rev: Option<Revision>,
}

pub struct PatchOp {
    pub slot: Slot,
    pub field_path: Vec<RawSegment>,      // ["body", Index(0), "expr"]
    pub new_value: RawValue,
    pub expected_rev: Option<Revision>,
}

pub enum RawSegment {
    Field(String),
    Index(u32),
    Variant(String),                      // for sum types
}

pub struct TxnBatch { pub ops: Vec<TxnOp> }

pub enum TxnOp {
    Assert(AssertOp),
    Mutate(MutateOp),
    Retract(RetractOp),
    Patch(PatchOp),
}
```

### 6.3 · Query verbs

```rust
pub struct QueryOp {
    pub selection: Selection,
}

pub struct SubscribeOp {
    pub selection: Selection,
    pub from_revision: Option<Revision>,  // resume; None → start now
    pub initial_snapshot: bool,           // true → server emits current matches first
}

pub struct Selection {
    pub pattern: RawPattern,
    pub operators: Vec<RawOp>,            // applied left-to-right
    pub projection: Option<RawProjection>,
}

pub struct RawPattern {
    pub kind_name: String,                // unresolved at wire
    pub field_constraints: Vec<(String, RawConstraint)>,
    pub binds: Vec<(String, FieldPath)>,
    pub negations: Vec<String>,           // negated fields/sub-patterns
    pub list_pattern: Option<RawListPattern>,
    pub conjunctions: Vec<RawPattern>,    // {| | |} joins
}

pub enum RawConstraint {
    Eq(LiteralValue),
    StartsWith(String),
    EndsWith(String),
    Contains(LiteralValue),
    Range { min: Option<LiteralValue>, max: Option<LiteralValue> },
    Bind(String),                         // @h
    Negate(Box<RawConstraint>),
}

pub enum RawListPattern {
    HeadTail { head: Box<RawPattern>, tail: String /* @bind */ },
    Positional(Vec<RawPattern>),
    Anywhere(Box<RawPattern>),
}

pub enum RawOp {
    // Aggregation
    Count, CountDistinct(String), Sum(String), Min(String),
    Max(String), Avg(String),
    // Grouping
    GroupBy { binds: Vec<String>, inner: Vec<RawOp> },
    Having(RawPattern),
    // Ordering / pagination
    OrderBy(Vec<(String, SortOrder)>),
    Limit(u64), Offset(u64),
    Top(u64, Box<RawOp>), Bottom(u64, Box<RawOp>),
    Before(Cursor), After(Cursor),
    // Set
    Distinct, DistinctBy(String), Project(Vec<String>),
    // Temporal
    TimeAt(RevisionRef), TimeBetween(RevisionRef, RevisionRef), TimeAll,
    // Cross-instance (Phase 3 sketch)
    RemoteInstance(String /* peer-id */),
}

pub enum SortOrder { Asc, Desc }
pub struct Cursor(pub Vec<u8>);
pub enum RevisionRef { Rev(Revision), Hash(Blake3Hash) }

pub struct RawProjection { pub fields: Vec<RawProjField> }

pub enum RawProjField {
    Bind(String),
    Field(String),
    Aggregation(Box<RawOp>),
    Nested { key: String, inner: Box<RawProjection> },
}

pub struct ValidateOp {
    pub op: Box<TxnOp>,                   // dry-run
    pub explain: bool,                    // include ExecutionPlan
}
```

### 6.4 · Reply types

```rust
pub enum Reply {
    Ok(OkReply),
    Rejected(RejectedReply),
    QueryHit(QueryHitReply),

    // Subscription stream events (each frame carries one)
    SubReady    { subscription_id: u64 },
    SubSnapshot { subscription_id: u64, records: Vec<RawRecord> },
    SubAssert   { subscription_id: u64, slot: Slot, record: RawRecord },
    SubMutate   { subscription_id: u64, slot: Slot, old: RawRecord, new: RawRecord },
    SubRetract  { subscription_id: u64, slot: Slot, last: RawRecord },
    SubError    { subscription_id: u64, diagnostic: Diagnostic },
    SubEnd      { subscription_id: u64, reason: String },

    ValidateResult {
        passes: bool,
        diagnostics: Vec<Diagnostic>,
        plan: Option<ExecutionPlan>,
    },
}

pub struct OkReply {
    pub revision: Revision,
    pub effects: Vec<Effect>,
}

pub enum Effect {
    Asserted  { slot: Slot, content_hash: Blake3Hash },
    Mutated   { slot: Slot, old_hash: Blake3Hash, new_hash: Blake3Hash },
    Retracted { slot: Slot, last_hash: Blake3Hash },
    Patched   { slot: Slot, path: Vec<RawSegment>, new_hash: Blake3Hash },
}

pub struct RejectedReply {
    pub diagnostics: Vec<Diagnostic>,
    pub failed_at_op: Option<u32>,        // for TxnBatch: index in ops vec
    pub diagnostic_records: Vec<Slot>,    // slots where Diagnostic records were durably asserted
}

pub struct QueryHitReply {
    pub revision: Revision,               // sema snapshot revision
    pub bindings: Vec<Bindings>,          // one per match
    pub aggregation: Option<RawValue>,    // when query is purely aggregating
}

pub struct Bindings(pub Vec<(String, RawValue)>);

pub struct ExecutionPlan {
    pub steps: Vec<ExecutionStep>,
    pub estimated_cost: u64,
}

pub enum ExecutionStep {
    Scan      { kind_name: String, estimated_count: u64 },
    Filter    { constraints: Vec<String> },
    Join      { with_kind: String, via_field: String },
    Aggregate { op: RawOp },
    Sort      { by: Vec<(String, SortOrder)> },
    Limit(u64),
}
```

### 6.5 · Diagnostic + supporting types

```rust
pub struct Diagnostic {
    pub level: DiagnosticLevel,           // Error | Warning | Info
    pub code: String,                     // E0001..E9999
    pub message: String,
    pub primary_site: Option<DiagnosticSite>,
    pub context: Vec<(String, String)>,
    pub suggestions: Vec<DiagnosticSuggestion>,
    pub durable_record: Option<Slot>,     // if asserted as Diagnostic record in sema
}

pub enum DiagnosticLevel { Error, Warning, Info }

pub enum DiagnosticSite {
    Slot(Slot),
    SourceSpan { offset: u32, length: u32, source: String },
    OpInBatch(u32),
}

pub struct DiagnosticSuggestion {
    pub applicability: Applicability,
    pub replacement_text: String,
    pub site: Option<DiagnosticSite>,
}

pub enum Applicability { MachineApplicable, MaybeIncorrect, HasPlaceholders }
```

### 6.6 · The wire-record types (RawRecord / RawValue)

```rust
pub struct RawRecord {
    pub kind_name: String,                // resolved by criomed against KindDecl
    pub fields: Vec<(String, RawValue)>,
}

pub enum RawValue {
    Lit(LiteralValue),
    SlotRef(Slot),                        // bare integer in nexus → SlotRef when target field is Slot-typed
    List(Vec<RawValue>),
    Record(Box<RawRecord>),
    Bytes(Vec<u8>),                       // # byte-literal
}

pub enum LiteralValue {
    U64(u64), I64(i64), F64(f64), Bool(bool), String(String),
    Bytes(Vec<u8>), Blake3(Blake3Hash),
    Slot(Slot), Revision(Revision),
}

pub struct Slot(pub u64);
pub struct Revision(pub u64);
pub struct Blake3Hash(pub [u8; 32]);
pub struct BlsG1(pub [u8; 48]);

pub enum FieldPath {
    Direct(String),                       // top-level field
    Nested(Vec<RawSegment>),              // nested path
}
```

### 6.7 · What this contract commits to

- **Kind names are wire-strings, resolved at validation.**
  Criomed looks up `KindDecl` from `kind_name` at step 1. No
  pre-resolved type IDs on the wire — the contract stays
  schema-evolution-friendly.
- **Slot-refs are bare `Slot(u64)`** on the wire; criomed
  resolves them against `SlotBinding`. Wire never carries
  content hashes for cross-record references — those live
  on the sema side, not in messages.
- **Diagnostics can be both transient (wire-only) and
  durable (asserted as records).** `durable_record:
  Option<Slot>` on `Diagnostic` flags which.
- **Subscriptions are multi-frame.** A single subscribe
  request triggers a stream of reply frames sharing the
  `subscription_id`. The correlation_id maps to the
  `SubReady` only.
- **TxnBatch is one Frame.** All ops travel in a single
  rkyv-archived frame; criomed processes the whole batch
  atomically.

---

## 7 · Nexusd's role — text ↔ rkyv

Nexusd:

1. Accepts nexus text on a UDS socket.
2. Lexes and parses with `nota-serde-core` at
   `Dialect::Nexus`.
3. Maps the parsed AST to a `Frame`:
   - Top-level form `(Assert ...)` → `Request::Assert(AssertOp{...})`.
   - `~(...)` sigil → `Request::Mutate(...)`.
   - `!record-form` → `Request::Retract(...)`.
   - `(Patch slot path value)` → `Request::Patch(...)`.
   - `{|| ... ||}` → `Request::TxnBatch(...)`.
   - `(Query pattern op1 op2 ...)` → `Request::Query(...)`
     with `Selection { pattern, operators, projection }`.
   - `<| pattern ops |>` → `Request::Subscribe(...)`.
   - `(Validate op)` → `Request::Validate(...)`.
4. Sends the rkyv-archived `Frame` to criomed over UDS.
5. Reads reply frames; serialises back to nexus text using
   the inverse mapping.
6. For `Subscribe` requests, holds the connection open and
   relays each reply frame as it arrives.

Nexusd holds no sema state. It does **syntactic** mapping
only; all semantic resolution happens at criomed.

The mapping is mechanical and explainable per nexus form.
This is the contract Li asked for.

---

## 8 · Open questions

Carried forward from the agents' reports plus this synthesis:

### Q1 · Subscription delivery semantics

At-least-once / at-most-once / exactly-once. Default
candidate: at-least-once with explicit ack frames; client
deduplicates by `subscription_id × event-rev`. Confirm or
override.

### Q2 · Pattern-driven mutate — server-side or client-side?

Lean: client-side loop inside `{|| ... ||}` (mechanism is
honest; audit trail per record). Server-side variant
`(MutateMatching pattern { fields })` reserved for future
performance need. Confirm.

### Q3 · Intra-transaction forward-refs

Lean: not supported (split into two transactions). Confirm,
or relax for ergonomic workflows.

### Q4 · Cascade non-termination — reject vs diagnostic

Lean: emit E9999 diagnostic, time out cascade, do not
reject the originating mutation. Confirm.

### Q5 · Cross-instance verbs in this contract or separate?

The current contract sketches `RemoteInstance` as a query
operator and adds `QuorumProof` to `AuthProof`. The bigger
cross-criomed protocol (signed proposals, hash-shared
records, federated subscriptions) is intentionally not in
this contract — it belongs in a peer-to-peer criome-net
contract. Confirm scope: signal = local nexusd↔criomed
only; criome-net = criomed↔criomed peer.

### Q6 · Validate-verb's ExecutionPlan shape

The `ExecutionPlan` enum is a sketch. At what granularity do
we expose query plans? Lean: keep it coarse (Scan / Filter
/ Join / Aggregate / Sort / Limit) — enough for editor
hints without committing to optimiser internals.

---

*End report 070.*
