# 119 — Schema-in-sema: descriptors live in the database, not the binary

*Course-correction in response to Li 2026-04-30 ("this looks like
it should be data in sema — a kind of data that needs different
authorization to edit. It looks really clumsy like this, hardcoding
it in the runtime."). Reports/115 + 118 framed the proc-macro's
output as the runtime catalogue. That framing was off — the
runtime authority is sema; the proc-macro is one bootstrap path
into sema, not the catalogue itself. This report names the
shift and the implementation consequences. Lifetime: until the
schema-in-sema records land and this folds into criome's
ARCHITECTURE.md or signal's ARCHITECTURE.md.*

---

## 0 · TL;DR

```
   ┌── the framing reports/115 + 118 carried (wrong) ─────────────┐
   │                                                              │
   │   compile-time:  signal's KindDescriptor const is THE        │
   │                  catalogue                                   │
   │   runtime:       mentci-lib reads ALL_KINDS at runtime via   │
   │                  Rust path                                   │
   │   consequence:   every binary that touches schema knows it   │
   │                  by virtue of being compiled against signal  │
   │                                                              │
   └──────────────────────────────────────────────────────────────┘

   ┌── the framing this report corrects to ───────────────────────┐
   │                                                              │
   │   sema authoritative:  schema descriptors are records in     │
   │                        sema — `KindDecl`, `FieldDecl`        │
   │   bootstrap:           the proc-macro emits compile-time     │
   │                        descriptors that get PROJECTED into   │
   │                        sema as records on engine boot —      │
   │                        the macro's role is the seed, not    │
   │                        the catalogue                         │
   │   runtime:             every consumer (mentci-lib, the       │
   │                        future nexus renderer, agents, the    │
   │                        constructor flow) reads schema by    │
   │                        QUERYING sema for KindDecl records   │
   │   access control:               KindDecl records are system-only-    │
   │                        write — normal users have read-only  │
   │                        access; only privileged genesis +     │
   │                        future schema-evolution flows can    │
   │                        write                                 │
   │                                                              │
   └──────────────────────────────────────────────────────────────┘
```

The proc-macro doesn't disappear. Its role narrows: it's the
build-time projection mechanism that turns Rust type definitions
into schema records, analogous to how prism turns sema records
into Rust source. Both move data across one boundary; neither is
the runtime authority for what they project.

---

## 1 · Why the binary-resident descriptor is wrong

Three problems with the shape reports/115 and 118 carried:

```
   ┌── 1. authority is in the wrong place ───────────────────────┐
   │                                                             │
   │  "what kinds of records exist in this engine?" is a fact   │
   │  about the running engine — exactly the kind of fact sema   │
   │  is for. Compiling it into every consumer binary makes     │
   │  the schema a property of "which version of signal you     │
   │  link" rather than "what's in this database."              │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘

   ┌── 2. introspection is broken ───────────────────────────────┐
   │                                                             │
   │  per [INTENTION.md](../INTENTION.md): "the engine reveals  │
   │  itself to those participating in its development."         │
   │  Schema is the most introspectable layer there is — it     │
   │  describes the shape of every record. If the schema lives  │
   │  in compiled-in consts, the workbench can show kinds but   │
   │  not edit them, can't watch them change, can't apply the   │
   │  same wire-pane / inspector / change-log treatment to them │
   │  it applies to every other record. The schema becomes      │
   │  invisible to the engine that depends on it.               │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘

   ┌── 3. access control is a category error ─────────────────────────────┐
   │                                                             │
   │  schema needs different access control than user records:  │
   │  read-only by default, write only via explicit privileged  │
   │  paths. The compiled-in const can't carry access control at all —   │
   │  it's just bytes in the binary. Records-in-sema get access control  │
   │  from the same machinery every other record gets it from   │
   │  (capability tokens, signed proofs, etc.), with the        │
   │  distinguishing dimension just being "which Principal can  │
   │  write this kind."                                         │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘
```

---

## 2 · The shape of what changes

Sema-native — every cross-record reference is a `Slot<KindDecl>`,
not a string lookup. Names exist exactly once, on the canonical
record they identify. Same discipline as `Edge.from: Slot<Node>`.

```
   KindDecl
   ─────────────────────────────────────────────────────────
   name  : String              ← the kind's English name
   shape : KindShapeDecl       ← tag only: Record or Enum

   KindShapeDecl  (closed enum, no payload)
   ─────────────────────────────────────────────────────────
   Record
   Enum

   FieldDecl
   ─────────────────────────────────────────────────────────
   kind        : Slot<KindDecl>   ← which kind this field belongs to
   position    : u32              ← declaration order
   name        : String           ← field name (the only string here)
   field_type  : FieldTypeDecl
   is_optional : bool
   is_list     : bool

   VariantDecl
   ─────────────────────────────────────────────────────────
   kind     : Slot<KindDecl>      ← which Enum-shape kind this is
                                   a variant of
   position : u32                  ← declaration order
   name     : String              ← variant name (the only string)

   FieldTypeDecl  (closed enum)
   ─────────────────────────────────────────────────────────
   Text                                       ← String-typed field
   Bool                                        ← bool-typed
   Integer                                     ← any integer primitive
   Float                                       ← any float primitive
   AnyKind                                     ← Slot<AnyKind>
                                                (type-erased)
   SlotRef { of_kind: Slot<KindDecl> }        ← Slot<NamedKind>
   Record  { kind:    Slot<KindDecl> }        ← reference to another kind
```

The relational shape:

```
   "what is the kind named 'Edge'?"
       → Query KindDecl where name = "Edge" → kind_slot
       → read shape tag: Record

   "what are Edge's fields?"
       → Query FieldDecl where kind = kind_slot, ORDER BY position
       → returns three FieldDecls: from, to, kind

   "the 'kind' field of Edge — what's its type?"
       → field_type = Record { kind: relation_kind_slot }
       → Query KindDecl by slot relation_kind_slot → "RelationKind"
       → read shape tag: Enum

   "what are RelationKind's variants?"
       → Query VariantDecl where kind = relation_kind_slot,
         ORDER BY position
       → returns nine VariantDecls: Flow, DependsOn, …
```

Three things follow from this shape:

- **Names appear once**, on the record they identify. References
  elsewhere are `Slot<KindDecl>`. Renaming a kind means mutating
  one record; every reference keeps working.
- **No cycles in the seed order.** KindDecls assert first (they
  reference nothing). FieldDecls + VariantDecls assert next
  (they reference KindDecls, which exist). FieldTypeDecl::Record
  fields reference KindDecls — also fine because KindDecls
  exist before FieldDecls do. No `Mutate` required.
- **The recursion lands clean.** KindDecl describes itself: there's
  a KindDecl record named "KindDecl" with shape Record, and three
  FieldDecls whose `kind` field points at that KindDecl's own
  slot. Self-reference at the data level, not in dependency order.

### 2.1 The flow, before vs after

```
   BEFORE (binary-resident)

   signal source        signal-derive          signal binary
   ────────────         ─────────────          ─────────────
   #[derive(Schema)]      ┌──>             impl Kind for Node {
   pub struct Node {      │                  const DESCRIPTOR:
       name: String,      │                    KindDescriptor = …
   }                      │               }
                          │                       │
                          │                       │  baked into
                          │                       │  every binary
                          ▼                       ▼
                                    mentci-lib reads compile-time
                                    catalogue via Rust path

   AFTER (sema-resident)

   signal source        signal-derive          ALL_KINDS
                                              (compile-time const
                                               — bootstrap source,
                                               not runtime authority)
   #[derive(Schema)]      ┌──>             ┌──────────────────────┐
   pub struct Node {      │                │ Vec<KindDescriptor>  │
       name: String,      │                │  used ONCE at boot   │
   }                      │                │  to build seed       │
                          │                └──────────┬───────────┘
                          │                           │ projection
                          ▼                           ▼
                                            kinds.nexus stream
                                            (Assert KindDecl …)
                                            (Assert FieldDecl …)
                                                      │
                                                      ▼
                                            criome stores the
                                            records in sema with
                                            system-write access control
                                                      │
                                                      ▼
                                            mentci-lib at runtime
                                            queries sema for
                                            KindDecl records;
                                            same path every other
                                            record-read uses
```

---

## 2.1 · Are we re-implementing parts of nexus?

Per Li 2026-04-30: *"are we re-implementing parts of nexus with
this? I feel like strings are creeping in from everywhere. How
does nexus 'store' the strings for variants?"*

**Short answer: no, we're not re-implementing the codec.** What's
happening is that the *same logical truth* (the Rust type
definitions in signal) gets projected into two different forms
for two different consumers:

```
   the Rust source                        the truth
   in signal/ — the                       ─────────
   single source of truth                 ┌─────────────────────┐
                                          │  signal record kind │
                                          │  definitions        │
                                          │  (Node, Edge, …)    │
                                          └──────────┬──────────┘
                                                     │
                       ┌─────────────────────────────┼─────────────────────────────┐
                       │ projected into              │             projected into  │
                       │ codec impls at              │             schema records  │
                       │ compile time                │             at boot time    │
                       │ (nota-derive)               │             (signal-derive  │
                       │                             │              + seed step)   │
                       ▼                             │                             ▼
   ┌─────────────────────────────────┐               │             ┌─────────────────────────────┐
   │ NotaEncode + NotaDecode for     │               │             │ KindDecl, FieldDecl,        │
   │ wire form — knows variant       │               │             │ VariantDecl records in sema │
   │ names + integer tags            │               │             │ — knows variant names too,  │
   │                                 │               │             │ but as data not code        │
   │ Consumer: nexus daemon when     │               │             │                             │
   │   parsing/rendering text        │               │             │ Consumers: mentci-lib's     │
   │ Consumer: rkyv when             │               │             │   constructor flow,         │
   │   encoding/decoding bytes       │               │             │   nexus-daemon's renderer,  │
   │                                 │               │             │   future agents asking      │
   │ Form: compile-time generated    │               │             │   "what kinds exist?"       │
   │   match-on-variant code         │               │             │                             │
   └─────────────────────────────────┘               │             └─────────────────────────────┘
                                                     │
   How nexus "stores" variant strings today          │
   ───────────────────────────────────────           │
   It doesn't, at runtime. The strings appear        │
   in nexus TEXT (e.g. `Flow` in `(Edge … Flow)`).   │
   The wire form stores an integer tag chosen by     │
   the NotaEnum derive. The mapping between          │
   them lives in the codec's compile-time            │
   generated code — never persisted.                 │
                                                     │
                                          one source of truth
```

The strings *do* exist in two places — but they're not
duplicating each other. The codec needs them to encode/decode
text; sema needs them as data so consumers can introspect at
runtime. Neither reads from the other; both come from the same
source (the Rust type defs), and the proc-macros are projection
mechanisms.

The same shape holds at scale: prism (when wired) projects sema
records to Rust source; the projection is one-way; the canonical
truth lives in sema; the .rs files are downstream artefacts. We
already accept that pattern. signal-derive + nota-derive are a
similar pair: both are projections out of Rust source, neither
is the other's authority.

The only thing that *would* be re-implementation is if we
asked sema to be the authority for codec encoding/decoding too —
i.e. if nota-codec started reading variant-tag mappings from
sema instead of having them baked in. We're not doing that. The
codec stays compile-time; sema stays the authority for
introspection.

## 3 · The proc-macro's revised role

`signal-derive` doesn't disappear and isn't a stop-gap — its role
changes from "emit the catalogue" to "emit the seed projector."
The output:

```
   today's emission (per reports/118)         tomorrow's emission

   impl ::signal::Kind for Node {              same — still useful as
       const DESCRIPTOR: KindDescriptor =       compile-time check; the
           KindDescriptor { … };                seed projector reads it
   }                                            as ITS input
                                              + a function or method
                                                that turns a
                                                `KindDescriptor` const
                                                into a sequence of
                                                Assert frames the
                                                process-manager seed
                                                step pipes through
                                                nexus-cli
```

Compile-time tests on `Node::DESCRIPTOR` still work — they verify
the macro's lowering rules. The change is downstream:
**mentci-lib stops reading `ALL_KINDS` directly**. Schema queries
go through sema like any other query.

This matches prism's role for code: prism doesn't replace
`rustc`; it generates the input rustc consumes. Likewise the
proc-macro doesn't replace sema; it generates the input sema
consumes.

---

## 4 · Authz: read-only normally, system-edit only

The access control model that distinguishes `KindDecl` records from user
records:

```
   ┌── normal user (any Principal) ──────────────────────────────┐
   │                                                              │
   │   Query KindDecl                       allowed (read-only)   │
   │   Subscribe to KindDecl changes        allowed                │
   │   Assert KindDecl                      REJECTED — diagnostic │
   │   Mutate KindDecl                      REJECTED — diagnostic │
   │   Retract KindDecl                     REJECTED — diagnostic │
   │                                                              │
   └──────────────────────────────────────────────────────────────┘

   ┌── system path (genesis + future schema-evolution flows) ────┐
   │                                                              │
   │   Assert KindDecl                      allowed                │
   │   Mutate KindDecl                      allowed (with care —  │
   │                                         schema evolution is  │
   │                                         a real thing; M1+    │
   │                                         work)                │
   │   Retract KindDecl                     allowed                │
   │                                                              │
   └──────────────────────────────────────────────────────────────┘
```

The mechanism: criome's validator (currently `todo!()` skeleton)
checks the writing Principal's access control against the record kind
being written. KindDecl writes require a special "system write"
capability that only the genesis-bootstrap path holds.

This is the criome ARCH §10.2 responsibilities applied to schema
records: criome is the gatekeeper. The capability is a token
criome signs for itself during bootstrap (no external signer).

For the first cut: the validator-pipeline isn't wired (per
[reports/113 §3.3](113-architecture-deep-map-2026-04-29.md#33-status-by-verb)).
KindDecl writes via Assert are accepted today the same as Node
writes. Tightening the check is downstream of the auth slice
landing — not blocking on this report.

---

## 5 · What this changes in the implementation plan

Compared with [reports/117 §5](117-implementation-gap-2026-04-30.md#5--sequence-to-engine-works-end-to-end):

```
   step                                         status / change
   ──────────────────────────────────────────  ─────────────────

   1  process-manager skeleton                 unchanged
   2  genesis.nexus written                    UPDATED — now also
                                               carries the KindDecl /
                                               FieldDecl seed
                                               (generated from
                                                signal::ALL_KINDS at
                                                build time)
   3  process-manager seed step                UPDATED — pipes both
                                               kinds.nexus + genesis
                                               .nexus on empty-sema
   4  nix run .#up spawns full stack           unchanged

   ────── above this line: the engine is working ──────

   5  Slot<T> migration                        DONE
   6  signal-derive crate +                    PARTIALLY DONE — derive
      mentci-lib's CompiledSchema reads          + ALL_KINDS landed
      ALL_KINDS                                  but the consumer-side
                                                 read should query
                                                 sema, not ALL_KINDS
   6.5 NEW — KindDecl + FieldDecl + …          add to signal as record
        record kinds                            kinds with their own
                                                #[derive(Schema)]
                                                (recursive: KindDecl
                                                describes itself)
   6.6 NEW — kinds.nexus generator              small one-shot binary
                                                in signal that emits
                                                Assert KindDecl
                                                frames from ALL_KINDS
   6.7 mentci-lib's CompiledSchema reads       replaces step 6's
        sema instead of ALL_KINDS                 in-process catalogue
                                                 read
   7  NewEdge constructor commit                unchanged
   8  mentci-egui handlers                      unchanged
   9  per-user identity                         unchanged

   ────── M1 ──────

   10 criome Mutate / Retract / AtomicBatch     unchanged
   11 Subscribe push delta / sub-id              unchanged
   12 NEW — KindDecl access control                      tighten so normal
                                                 Asserts of KindDecl
                                                 are rejected; only
                                                 the genesis path
                                                 writes
```

Steps 1–4 still hit the "engine working end-to-end" milestone.
The change from this report doesn't push that milestone further —
it replaces the in-process schema catalogue (which mentci-lib's
constructor flow would have used) with sema-resident schema
records (which the constructor flow will use instead). The
constructor-flow's kind palette doesn't unblock until 6.7 lands;
that gates step 7+.

---

## 6 · The recursion completes

There's a beautiful recursion that this direction enables:

```
   KindDecl is itself a record kind described by a KindDecl
   (the one named "KindDecl"). Same for FieldDecl and
   FieldTypeDecl.

   The seed step asserts:
     • a KindDecl named "KindDecl"
     • a KindDecl named "FieldDecl"
     • a KindDecl named "FieldTypeDecl"
     • a KindDecl named "Node"
     • a KindDecl named "Edge"
     • … (one per signal record kind)

   At runtime, mentci-lib can ask "what kinds exist?" and get
   back records describing every kind including KindDecl
   itself. The workbench can paint the schema as records, edit
   them through the same constructor flow it uses for user
   records (with access control catching the writes), watch them change
   through the same Subscribe push, render them through the
   same nexus renderer.
```

Schema becomes another shape inside sema. Per [criome ARCH §11
"Open shapes"](../repos/criome/ARCHITECTURE.md#11--open-shapes)
this is what schema-in-sema means; this report is just naming
that the workspace should land there from the first runtime, not
"someday."

---

## 7 · Open shapes

**Q1 — `KindDecl::shape`: tag-only or carry the children?**
Resolved as a tag-only enum after the §2 redesign — the children
(FieldDecls / VariantDecls) point back at their KindDecl rather
than the KindDecl listing them. Cleaner and avoids the cycle in
the seed order. Confirm that's the right call?

**Q2 — Access-control carrier.** Today's MVP is
`AuthProof::SingleOperator`. The KindDecl-write capability is
what gates schema writes. Two shapes:

- a special "genesis context" flag in criome that's only true
  during boot
- a real capability token signed by criome's signing key
  (per [reports/114 §10.1 Q8](114-mentci-stack-supervisor-draft-2026-04-30.md))

The second is the durable shape (per the no-stop-gaps rule).
The first might be acceptable as a transient piece of the boot
sequence that's clearly demarcated. I lean the second; flagging
the choice.

**Q3 — One Assert per FieldDecl, or AtomicBatch?** Each KindDecl
references multiple `Slot<FieldDecl>`s. Asserting them one-by-one
means each FieldDecl gets its slot at assert time, but then the
KindDecl's `fields:` list has to reference those slots — same
ordering question as in [reports/116 §5](116-genesis-seed-as-design-graph-2026-04-30.md#5--slot-ordering).
Easiest: assert all FieldDecls first, then the KindDecl
referencing them. Wraps cleanly into AtomicBatch when that
verb lands.

---

*End report 119.*
