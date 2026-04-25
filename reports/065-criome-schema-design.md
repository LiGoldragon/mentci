# 065 — criome-schema design (in depth)

*Claude Opus 4.7 · 2026-04-25 · in-depth design of `criome-
schema`, the Rust crate defining the record kinds criomed
needs for its OWN operational state. Two parallel research
agents covered (A) schema-of-schema + index + audit and (B)
authorization + correctness + diagnostics + subscriptions.
Synthesis below corrects both agents' slip-back into "internal
assert" framing for genesis (Li 2026-04-25 standing rule:
records enter sema only via nexus → nexusd → signal →
criomed; no baked-in rkyv assert path) and presents the
record catalogue plus the genesis-via-nexus bootstrap
mechanics.*

*Companion to: `nexus-schema` today (holds **machina** records
— the code category; possibly renamed to `machina-schema`,
see Q1) and a future `world-schema` (world-fact records, post-
MVP per reports/060 §3). criome-schema is one slice of sema's
full record catalogue, not the whole.*

---

## 1 · Why a separate crate

### 1.1 · The category boundary

Sema holds records of multiple intrinsic categories
(reports/061 §1.11). Each category gets its own schema crate
so:

- criomed depends on `criome-schema` always; on `machina-
  schema` (current `nexus-schema`) so it can validate code
  records; not on `world-schema` until that category lands.
- machina-chk (reports/061 §3.5) depends on `machina-schema`
  for the records it checks; need not depend on criome-
  schema except where it emits Diagnostic records.
- A new category lands as a new crate without touching
  existing schema crates.

### 1.2 · Two roles for the Rust types in criome-schema

The Rust struct/enum definitions in criome-schema serve two
distinct purposes:

1. **Schema-of-schema source-of-truth at first boot.** Before
   any KindDecl records exist in sema, criomed must validate
   the genesis-nexus messages. It does so against the built-
   in Rust types in criome-schema. This is the *only* moment
   Rust types act as schema; afterwards, in-sema KindDecl
   records are authoritative for everything except a
   consistency check at second-boot.
2. **In-process record handling.** criomed's Rust code
   reads/writes records via rkyv archiving. The Rust types
   are the in-process representation; rkyv-archived bytes
   are the on-disk + on-wire form.

After first boot the in-sema KindDecls and the Rust types
must agree (verified at every subsequent boot per
architecture.md §10 reject-loud rule; hard-fail on schema skew).

### 1.3 · Six groups of records

| Group | Purpose | Validator step served |
|---|---|---|
| 3.1 Schema-of-schema | Define record shapes | step 1 (schema-check) |
| 3.2 Slot/index | Identity + reference resolution | step 2 (ref-check) |
| 3.3 Audit/history | Persistent log of changes | step 5 (write — appends) |
| 3.4 Authorization | Principals + capabilities + quorum | step 4 (permission-check) |
| 3.5 Correctness/rules | Invariants + cascading derivations | step 3 (invariant-check) + step 6 (cascade) |
| 3.6 Diagnostics + outcomes + subscriptions | Output + reactivity | emitted, not consulted |

---

## 2 · Bootstrap correctly: genesis-via-nexus

Both research agents described seed assertion as
`criomed.assert_record(...)` — an internal call bypassing
nexus. **This is the framing Li flagged 2026-04-25:**

> *"I really don't know what you mean by 'baked-in rkyv data'
> — that sounds insane, like you just made that up. Did you
> think about how that would work? And we would just abandon
> the bottom-rung layer, nexus?"*

> *"did any of them bother to actually explain what the
> 'internal assert' would look like in practice? As in 'we
> could just ask god in heaven to write it all for us'?"*

**Neither agent explained the mechanism.** Both wrote
`criomed.assert_record(&kind_decl_self)` or
`let root_principal = Principal { ... };` as though those
operations were primitives. Unanswered in both:

- What `assert_record` actually does — does it run the
  validator pipeline? Skip it? Reimplement it? At what
  layer? With what invariants?
- How `ChangeLogEntry.principal` is populated when no
  Principal record exists yet to point a `Slot` at.
- How schema-check passes when no `KindDecl` record exists
  yet to validate against.
- Where the seed `Principal` struct *lives* before it's
  "asserted" — a Rust `const`? a config file? a hardcoded
  `vec!`? a private redb writer that bypasses the validator?

The "internal assert" is a hand-wave — magical thinking that
hides the absence of any specifiable mechanism. **Whenever a
proposed mechanism cannot be explained step by step, the
framing is wrong.** The architecturally consistent path is
genesis-via-nexus — and crucially, *every step of that path
has an existing or specifiable mechanism*.

### 2.1 · `genesis.nexus` ships with the criomed binary

`genesis.nexus` is a text file ship­ped with criomed (e.g.,
`include_str!` or in the criomed nix-store entry as a
sibling file). It contains a sequence of `(Assert ...)`
expressions in nexus syntax:

```nexus
(Assert (CategoryDecl :slot 0 :name "criomed-state"  :stratum-max 1 ...))
(Assert (CategoryDecl :slot 1 :name "machina"        :stratum-max 2 ...))
(Assert (KindDecl     :slot 2 :name "KindDecl"       :category 0 :fields [...]))
(Assert (KindDecl     :slot 3 :name "FieldSpec"      :category 0 :fields [...]))
;; ... ~60–120 seed records: every kind criomed knows at boot ...
(Assert (Principal    :slot 100 :pubkey 0x... :display-name "operator"))
(Assert (Quorum       :slot 101 :members [100] :threshold 1))
(Assert (Policy       :slot 102 :resource-pattern :all :allowed-ops [...]
                       :required-quorum 101))
(Assert (SemaGenesis  :slot 1   :marker 0xDEADBEEF :created-at 0))
```

### 2.2 · First-boot pipeline

1. criomed opens redb at sema location (creating empty if
   absent).
2. criomed reads the well-known `SemaGenesis` slot. Absent →
   first boot.
3. criomed dispatches `genesis.nexus` text to **nexusd over
   the normal UDS channel** — same wire as user requests.
4. nexusd parses each `(Assert ...)` via nota-serde-core at
   `Dialect::Nexus`, builds signal envelopes, sends each
   to criomed.
5. criomed runs each through the **normal validator
   pipeline**, with these "first-boot specifics" only:
   - **Schema-check**: against built-in Rust types in
     criome-schema (no in-sema KindDecls yet).
   - **Ref-check**: against records already asserted in this
     genesis stream (genesis.nexus is ordered so refs only
     point backward).
   - **Invariant-check**: trivially passes — no Rule
     records yet.
   - **Permission-check**: against a *bootstrap principal
     id* baked into criomed's binary, used only for the
     genesis stream. Once the genesis Principal record is
     asserted (early in genesis.nexus), subsequent messages
     in the stream may be re-validated against it; the
     simplest implementation uses the bootstrap id
     throughout the genesis stream.
   - **Write**: standard. SlotBinding + ChangeLogEntry +
     RevisionRecord. ChangeLogEntry's `principal` field
     points to the bootstrap principal slot during genesis.
6. The final assertion is `(Assert (SemaGenesis ...))`.
   After it lands, criomed switches to second-boot mode.

### 2.3 · Second-boot pipeline

1. criomed opens redb. Reads `SemaGenesis`. Present →
   second boot.
2. criomed iterates `[0, 1024)` (seed slot range). For each
   in-sema record at a seed slot, fetches it and compares
   against the corresponding built-in Rust type. Hard-fail
   on mismatch ("schema skew between criomed binary and
   sema; aborting").
3. criomed begins accepting normal nexus requests. Validator
   uses in-sema KindDecls (not built-in Rust types) for any
   user-defined kind.

### 2.4 · What this is NOT

- **No** `criomed.assert_record(...)` internal call. Every
  record enters sema through the validator pipeline.
- **No** baked-in rkyv data. The rkyv encoding is created in
  validator step 5 (write), the same as for any other
  record.
- **No** special private input port. genesis.nexus enters
  through nexusd → signal → criomed.

The only "specialness" is what's *not yet in sema* during
the genesis stream: built-in Rust types substitute for
KindDecls, a bootstrap principal id substitutes for a
Principal record. Once those records have been asserted
(through the normal flow), the system is in normal mode.
This is consistent with the iterative-bootstrap framing of
reports/064 §1: criomed's competence grows as sema's content
grows; the genesis stream is the first wave of that growth.

---

## 3 · The records

Compact reference; agent reports embed deeper detail. All
types `#[derive(Archive, Serialize, Deserialize)]` (rkyv
+ serde for nexus parsing).

### 3.1 · Schema-of-schema

```rust
pub struct KindDecl {
    pub kind_id: u64,
    pub name: String,
    pub category: Slot,                       // → CategoryDecl
    pub stratum: u32,                         // datalog stratification
    pub stability: StabilityTag,              // Seed | Stable | Evolving | Experimental
    pub fields: Vec<FieldSpec>,
    pub is_sum: bool,
    pub variants: Option<Vec<VariantDecl>>,
    pub created_at_rev: Revision,
}

pub struct FieldSpec {
    pub name: String,
    pub type_ref: TypeRef,
    pub is_nullable: bool,
    pub default: Option<LiteralValue>,
    pub constraints: Vec<ConstraintDecl>,
    pub visibility: FieldVisibility,          // Public | Internal
}

pub enum TypeRef {
    Primitive(PrimitiveType),                 // U32 U64 I32 I64 Bool String Bytes F64 Blake3Hash
    Named { slot: Slot, args: Vec<TypeRef> }, // → KindDecl
    Collection { element: Box<TypeRef>, kind: CollectionKind }, // Vec Set Map Option
}

pub struct VariantDecl {
    pub name: String,
    pub fields: Vec<FieldSpec>,
    pub discriminant: Option<u32>,
}

pub enum ConstraintDecl {
    Range { min: Option<LiteralValue>, max: Option<LiteralValue> },
    Regex(String),
    Length { min: usize, max: Option<usize> },
    UniqueWithinKind,
    ForeignKey { target_kind: Slot, target_field: String },
}

pub struct CategoryDecl {
    pub name: String,                         // "criomed-state" "machina" "world-fact"
    pub stratum_max: u32,
    pub visibility: CategoryVisibility,       // Public | SeedOnly | Internal
}
```

**The recursion**: `KindDecl` is itself a record kind. The
KindDecl-of-KindDecl exists in sema after genesis (slot 2
in the example above). Criomed validates user-asserted
KindDecls against this in-sema record. The first KindDecl
(KindDecl-of-KindDecl itself) is validated against the
built-in Rust `KindDecl` type — that's where the
recursion grounds out at first boot.

### 3.2 · Slot / index / Revision

```rust
pub struct Slot(pub u64);                     // [0, 1024) reserved for seed
pub struct Revision(pub u64);                 // Monotonic global commit counter

pub struct SlotBinding {
    pub slot: Slot,
    pub current_content_hash: Blake3Hash,
    pub display_name: String,                 // globally unique across all slots
    pub kind_id: u64,
    pub created_at_rev: Revision,
    pub updated_at_rev: Revision,
    pub tombstone: bool,
}

pub struct SemaGenesis {
    pub marker: u64,                          // sentinel value
    pub created_at: Revision,
    pub criomed_version: String,
}
```

The display-name index is a redb table keyed `String → Slot`,
rebuilt at startup from SlotBinding records — not itself a
record kind.

### 3.3 · Audit / history

```rust
pub struct ChangeLogEntry {
    pub seq: u64,                             // per-kind monotonic
    pub rev: Revision,                        // global monotonic
    pub slot: Slot,
    pub op: ChangeOp,                         // Assert | Mutate | Retract
    pub new_content: Option<Blake3Hash>,
    pub old_content: Option<Blake3Hash>,
    pub principal: Slot,                      // → Principal
    pub sig_proof: Option<Slot>,              // → CommittedMutation (post-MVP)
}

pub struct RevisionRecord {
    pub rev: Revision,
    pub timestamp_ref: Slot,                  // → Timestamp record
    pub principal_ref: Slot,                  // → Principal
    pub summary: String,
    pub change_list: Vec<(Slot, ChangeOp)>,
    pub diagnostics_ref: Option<Slot>,        // → Outcome
}
```

`AuditEntry` cross-kind summary is open (Q4 below) — keep
as derived view or materialize.

### 3.4 · Authorization

```rust
pub struct Principal {
    pub id: u64,
    pub pubkey: BLS12381G1,                   // 48 bytes
    pub display_name: String,
    pub valid_from_rev: Revision,
    pub valid_to_rev: Option<Revision>,
}

pub struct Quorum {
    pub members: Vec<Slot>,                   // → Principal
    pub threshold: u32,
    pub parent_quorum_ref: Option<Slot>,      // → Quorum (chained rotation)
    pub display_name: String,
}

pub struct Policy {
    pub resource_pattern: Slot,               // → Pattern
    pub allowed_ops: Vec<Op>,
    pub required_quorum: Slot,                // → Quorum
}

pub struct Capability {
    pub principal: Slot,                      // → Principal
    pub resource_pattern: Slot,               // → Pattern
    pub op: Op,
    pub expires_at_rev: Option<Revision>,
}

pub struct CapabilityToken {
    pub principal: Slot,
    pub allowed_ops: Vec<Op>,
    pub resource_ref: Blake3Hash,
    pub issued_at_rev: Revision,
    pub expires_at_rev: Revision,
    pub criomed_signature: BLS12381G1,        // signed by RuntimeIdentity's secret
}

pub struct MutationProposal {
    pub proposer: Slot,                       // → Principal
    pub payload_digest: Blake3Hash,
    pub frozen_required_quorum: Slot,         // → Quorum (snapshot at proposal time)
    pub created_at_rev: Revision,
    pub expires_at_rev: Revision,
}

pub struct ProposalSignature {
    pub proposal: Slot,                       // → MutationProposal
    pub signer: Slot,                         // → Principal
    pub signature: BLS12381G1,
    pub signed_at_rev: Revision,
}

pub struct CommittedMutation {
    pub proposal: Slot,                       // → MutationProposal
    pub aggregate_signature: BLS12381G1,
    pub signer_set: Vec<Slot>,
    pub committed_at_rev: Revision,
}

pub struct RuntimeIdentity {
    pub self_principal: Slot,                 // → Principal record describing criomed itself
    // private_key NOT stored as a sema field — read at startup from a key file
    // outside sema; sema only knows the corresponding Principal's pubkey
}

pub struct PrincipalKey {
    pub principal: Slot,                      // → Principal
    pub old_pubkey: BLS12381G1,
    pub new_pubkey: BLS12381G1,
    pub transition_signature: BLS12381G1,     // signed by old_pubkey
    pub rotated_at_rev: Revision,
}
```

**Records exist from day one; verification deepens by stage:**

- **Stages A-B (early bootstrap)**: genesis.nexus asserts
  Principal + Quorum{threshold:1} + root Policy. Validator's
  permission-check matches request principal id against the
  Principal record; no BLS verification yet.
- **Stages C-D**: BLS verification activates; every
  Assert/Mutate/Retract carries a ProposalSignature; criomed
  validates against Quorum members.
- **Stages E+** (per reports/060 §2): chained rotation via
  parent_quorum_ref; expiring sub-keys; external custody.

The records are real from day one. The depth of *verification*
is what grows.

### 3.5 · Correctness / rules

```rust
pub struct Rule {
    pub name: String,
    pub head: RuleHead,
    pub premises: Vec<Slot>,                  // → RulePremise
    pub stratum: u32,
    pub is_must_hold: bool,                   // true=invariant, false=derivation (or split: see Q3)
}

pub struct RulePremise {
    pub pattern: Slot,                        // → Pattern
    pub bindings: Vec<(String, FieldPath)>,
}

pub struct RuleHead {
    pub kind_id: u64,
    pub field_values: Vec<(String, FieldExpr)>,
}

pub enum FieldExpr {
    Binding(String),
    Constant(LiteralValue),
    RecordRef(Slot),
    Count(String),                            // aggregate
}

pub struct DerivedFrom {
    pub derived_record: Slot,
    pub rule: Slot,                           // → Rule
    pub premise_matches: Vec<Slot>,
    pub derived_at_rev: Revision,
}

pub struct Invariant {
    pub name: String,
    pub kind_id: u64,
    pub predicate: Slot,                      // → Pattern
    pub error_message: String,
}
```

At Stage A: kinds exist, no Rule/Invariant records loaded.
Cascade engine is inert. Stage D activates these.

### 3.6 · Diagnostics + outcomes + subscriptions

```rust
pub struct Diagnostic {
    pub level: DiagnosticLevel,               // Error | Warning | Info
    pub code: String,
    pub message: String,
    pub primary_site: Option<Slot>,
    pub context: Vec<(String, String)>,
    pub raw_rustc_json: Option<Blake3Hash>,   // → lojix-store
    pub emitted_by: DiagnosticSource,         // Validator | Rustc | MachinaChkPhaseN
    pub emitted_at_rev: Revision,
}

pub struct DiagnosticSuggestion {
    pub diagnostic: Slot,                     // → Diagnostic
    pub applicability: Applicability,         // MachineApplicable | MaybeIncorrect | HasPlaceholders
    pub replacement_text: String,
    pub span_slot: Option<Slot>,
}

pub struct Outcome {
    pub target: Slot,
    pub status: OutcomeStatus,                // Success | Partial(String) | Failure(String)
    pub diagnostics: Vec<Slot>,
    pub completed_at_rev: Revision,
}

pub struct CompilesCleanly {
    pub opus: Slot,                           // → Opus (machina-schema)
    pub store_entry_hash: Blake3Hash,
    pub narhash: String,
    pub toolchain_pin: Slot,
    pub compiled_at_rev: Revision,
}

pub struct CompileDiagnostic {
    pub opus: Slot,
    pub diagnostics: Vec<Slot>,
    pub failed_at_rev: Revision,
}

pub struct SubscriptionIntent {
    pub subscriber: Slot,                     // → Subscriber
    pub filter_pattern: Slot,                 // → Pattern
    pub delivery_channel: DeliveryChannel,
    pub created_at_rev: Revision,
}

pub struct Subscriber {
    pub principal: Slot,
    pub endpoint: String,
    pub created_at_rev: Revision,
}
```

CompilesCleanly / CompileDiagnostic are kinds with no
instances until Stage F (compile path lands). Their schema
is locked from day one so other records can reference them
by slot.

### 3.7 · Patterns and queries (boundary with signal)

Patterns straddle: in-flight forms live in `signal`
(the wire crate), persisted forms live in criome-schema:

```rust
pub struct Pattern {
    pub kind_id: u64,
    pub field_constraints: Vec<(String, ConstraintValue)>,
    pub binds: Vec<(String, FieldPath)>,
    pub negations: Vec<String>,
    pub created_at_rev: Revision,
}

pub struct Query {
    pub name: String,
    pub pattern: Slot,                        // → Pattern
    pub projections: Vec<String>,
    pub order_by: Option<Vec<(String, SortOrder)>>,
    pub created_by: Slot,                     // → Principal
    pub created_at_rev: Revision,
}
```

In-flight `RawPattern` and `Mutation` enums live in `criome-
msg`. When a user wants a reusable query, nexus syntax
`(Assert (Query ...))` persists it as a record.

---

## 4 · Validator pipeline integration

Six steps; every Assert/Mutate/Retract runs all six.

| Step | Reads from criome-schema | Failure code |
|---|---|---|
| 1 schema-check | KindDecl, FieldSpec, TypeRef, ConstraintDecl (or built-in Rust types during genesis) | E0001 SchemaViolation |
| 2 ref-check | SlotBinding, name-index | E0002 UnresolvedRef |
| 3 invariant-check | Rule with `is_must_hold=true`, RulePremise, Pattern | E0003 InvariantViolation |
| 4 permission-check | Principal, Capability, Policy, Quorum, CommittedMutation | E0004 Unauthorized, E0005 ExpiredProposal, E0006 IncompleteQuorum, E0007 InvalidSignature |
| 5 write | (writes SlotBinding, ChangeLogEntry, RevisionRecord) | (fatal I/O) |
| 6 cascade | Rule, RulePremise, DerivedFrom | E9999 CascadeRuleFailure (diagnostic-only; does not reject the original Assert) |

At Stage A (per reports/064): step 1 against built-in Rust
types; step 2 against the index as it accumulates; step 3
trivially passes (no rules); step 4 against bootstrap
principal then in-sema Principal; step 5 active; step 6
trivially passes.

Stages B+ progressively activate steps 2-4 fully as their
backing records accumulate.

---

## 5 · Cross-schema boundaries

criome-schema does **not** include:

- Code records (Fn, Struct, Const, Module, Opus, OpusDep,
  Derivation, RustToolchainPin, ...). These live in
  machina-schema (currently `nexus-schema` crate; rename Q1
  below).
- World-fact kinds. Post-MVP per reports/060 §3; will live
  in `world-schema`.
- Bulk parametric data (camera frames, point clouds, tensor
  weights). Live in lojix-store; metadata records in sema
  reference them by `Blake3Hash`.

Cross-schema references go through `Slot` (not Rust type
imports). Example: `CompilesCleanly.opus: Slot` — the slot
resolves to a record of kind `Opus` (machina-schema), but
criome-schema doesn't import machina-schema's Rust types.
Validator step 2 checks the slot resolves; step 1 checks the
referenced record's kind matches expected.

This makes criome-schema a leaf crate; machina-schema (and
other categories) depend on it but not vice versa.

---

## 6 · Open questions for Li

### Q1 · Crate names + nexus-schema rename

Three options:

- (a) Keep `nexus-schema` for code records; add new `criome-
  schema` beside it.
- (b) Rename `nexus-schema` → `machina-schema` (consistent
  with the lexicon — machina is the code-category name);
  add `criome-schema` beside it.
- (c) Single crate with sub-modules `machina/`, `criome/`,
  `world/` (loses dependency-graph cleanliness).

Lean: (b). But the rename has its own cost; (a) defers it.

### Q2 · Genesis principal: hardcoded bootstrap id, or first-record-as-self

During the genesis stream, before any Principal record
exists, criomed needs a principal id for the permission-
check on each genesis assertion. Two ways:

- (a) Hardcoded `bootstrap-principal-id` in criomed's binary
  used for all genesis assertions. Retired the moment
  `SemaGenesis` lands.
- (b) The first message in `genesis.nexus` is `(Assert
  (Principal ...))`; that one message bypasses permission-
  check (since no Principal exists yet); subsequent
  messages claim that Principal.

(a) names the special case clearly and keeps the surface
small. (b) reduces specialness slightly but creates a
"first-message-is-special" rule.

### Q3 · Invariants vs Rule with `is_must_hold`

Two ways to express "this constraint must hold":

- (a) Dedicated `Invariant` kind; `Rule` is purely for
  derivation; validator step 3 reads Invariants.
- (b) `Rule { is_must_hold: bool }`; one kind; cascade
  engine treats `is_must_hold=true` rules as
  rejection-raising on premise match.

(a) clearer pipeline semantics. (b) one cascade engine.
Lean (b) for now; reverse if step 3's semantics need to
diverge.

### Q4 · AuditEntry: materialized or derived

Cross-kind audit summary `(rev, principal, summary,
change_count)`:

- (a) Asserted at commit time — one record per Revision;
  fast queries.
- (b) Derived on-demand from per-kind ChangeLogEntry tables;
  zero duplication; expensive queries.

### Q5 · Constraint extensibility

`ConstraintDecl::Custom(String)` — accept now (with what
sandbox?), or punt to post-MVP?

### Q6 · Stratum contiguity in CategoryDecl

Stratification per-kind via `KindDecl.stratum`. Should
`CategoryDecl` enforce stratum contiguity (kinds in a
category must use strata 0..N with no gaps), or allow `{0,
2}` skipping 1?

---

## 7 · What's next after Li answers

After Q1-Q6 land:

1. Lock `criome-schema` v0.0.1 with the records sketched
   above — types only, no runtime logic.
2. Author `genesis.nexus` — the actual nexus text that
   will ship with the criomed binary; ~60-120 records.
3. Build the Stage A path (per reports/064 §2): nexusd
   parses genesis.nexus, criomed validates against built-
   in Rust types, sema accumulates seed records, second-
   boot verifies parity.
4. The signal verb set follows naturally: Assert,
   Query, Retract for Stage A; Mutate added in Stage B
   for in-place edits; Subscribe added when subscriptions
   become useful; Compile added at Stage F.

---

*End report 065.*
