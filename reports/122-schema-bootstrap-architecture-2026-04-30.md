# 122 — Schema as records: the architectural arc

*Captures the architecture that emerged from a long stepback
session on 2026-04-30. The originating question, the path
through it, the architecture that landed, and the open shapes
still pending Li's confirmation.*

---

## 0 · The originating question

Li's prompt, paraphrased: *"There seems to be a duplication of
logic with nexus and the 'descriptor' design. Nexus and mentci
both share the need to turn signal records into some kind of
strings. Step way back, look at the core of my intention and my
general approach to architecture design, and research how this
could turn into a eureka design moment, from a very high-level
POV, using visuals."*

The concern is concrete: every variant name like `Validates`
exists in many places — Rust ident, nexus text bytes, rkyv wire
discriminant, `KindDescriptor` const, sema `VariantDecl` record.
Every kind name like `Node` likewise. Every field name. The
descriptor system feels like a parallel mechanism to nexus's
codec, restating shape information that the codec already
walks.

This report follows the path that question opened, through
several corrections, to the architecture that fell out at the
end.

---

## 1 · The path through, briefly

The corrections that mattered, in order:

1. **Nexus and mentci are not separate concerns at the projection
   layer — both turn records into text-bearing structure.** Nexus
   produces flat text; mentci-egui produces text inside spatial
   widgets. The text content overwhelmingly overlaps. They differ
   in surrounding structure (linear vs spatial+interactive), not
   in the shape walk that drives them.
2. **Mentci-egui isn't a codec.** Codecs are symmetric (parse =
   inverse of render, with a round-trip property). Mentci-egui is
   a *renderer* (records → pixels) plus an *editor* (gestures →
   record edits, mapped via the shape walk). Different abstract
   shape from nexus.
3. **Shape and language are separate concerns.** Nexus and
   mentci both consume two distinct things: the *numerical
   description* of a kind (its structure, language-agnostic) and
   the *linguistic description* (per-language display names).
   Sema sees only numerical references; strings live exclusively
   in `Localization` records. Switching the user's language
   re-skins everything without touching any non-Localization
   record.
4. **The inversion.** Declaring types as Rust source creates
   tension because Rust source isn't the right home for the
   identity question (what numerical id does this kind have?).
   Capnp-style systems resolve this by declaring types in a
   different system that emits both Rust types and metadata. In
   our system, *that other system ought to be the engine itself*
   — nexus records describe kinds, prism projects them to Rust
   source, the resulting binary reads the same sema. The schema
   is data the engine stores; the Rust types are downstream
   artifacts.
5. **The slot-id problem.** Once schema lives as records, the
   author writing a new kind in text needs to refer to other
   records (the kind's fields refer to its kind, its type
   expressions refer to other kinds, etc.) before those records
   are asserted. Slots are assigned on Assert; the author writing
   text doesn't know them.
6. **Deterministic per-kind indexes solve it.** Each kind has its
   own slot index counting from zero. A canonical bootstrap file
   declares all kinds in fixed order; criome plays it before
   opening the UDS listener. The Nth assertion of kind K gets
   slot N in K's index — predictable from the file alone,
   including forward and self references.

The architecture below is what fell out of (6).

---

## 2 · The mechanism

Three combined properties solve it:

**(a) Each record kind has its own slot index.**

`Slot<Kind>(0)` and `Slot<Field>(0)` are distinct values in
distinct namespaces. Each kind keeps its own counter starting at
zero. The number space stays small per kind (a few dozen for
schema kinds; thousands for domain kinds; eventually millions for
data kinds — but always counted *per kind*).

```
   Kind index            Field index           TypeExpression index
   ──────────            ───────────           ────────────────────
   $0  Kind              $0  Kind.shape        $0  ref to KindShape
   $1  Field             $1  Kind.fields       $1  Option<Vec<Field>>
   $2  Variant           $2  Kind.variants     $2  Option<Vec<Variant>>
   $3  TypeExpression    $3  Field.of_kind     $3  ref to Kind
   $4  KindShape         $4  Field.position    $4  Integer
   $5  Localization      ...                   ...
   ...
```

**(b) A canonical bootstrap file declares all kinds in a fixed
order.**

The file is processed top-to-bottom. The Nth assertion of kind
K gets slot N in K's index. The order in the file is the only
thing determining slot ids — no hash, no name lookup, no
content-addressing.

**(c) Criome loads bootstrap before opening the UDS listener.**

Before any Frame from any front-end is processed, criome plays
the bootstrap file into the empty database. Per-kind counters
start at zero; the file produces a deterministic sequence; every
slot id that ends up in sema for schema records is predictable
from the file alone.

Combined, an author counting their declarations knows what slot
each record will get. Cross-references in the text are integer
slot literals. The *type* of each slot literal — which index it
lives in — is determined by the positional schema of the record
it appears inside.

---

## 3 · Boot sequence

```
                       bootstrap.nexus       kinds.nexus      data
                       (universal)           (project)        Frames
                             │                    │             │
   ┌────────────────┐        │                    │             │
   │ criome startup │        │                    │             │
   │                │        ▼                    │             │
   │ open redb      │   parse + assert            │             │
   │                │   top-to-bottom;            │             │
   │                │   per-kind counters         │             │
   │                │   advance from 0            │             │
   │                │        │                    │             │
   │                │◄───────┘                    │             │
   │                │                             ▼             │
   │                │                    parse + assert         │
   │                │                    (counters continue)    │
   │                │                             │             │
   │                │◄────────────────────────────┘             │
   │                │                                           │
   │ open UDS       │                                           │
   │ listener       │                                           │
   │                │                                           │
   │                │◄──────────────────────────────────────────┘
   │                │            front-ends connect;
   │                │            data assertions flow in
   └────────────────┘
```

`bootstrap.nexus` is universal across every sema instance —
its slots occupy reserved positions in each kind's index.
`kinds.nexus` is project-specific and extends the same indexes.
Both are deterministic in their respective scopes.

---

## 4 · How declarations look in text

The form below is *example data* showing what an author writes,
not new nexus syntax. Every line is a plain positional record
assertion using the existing nexus grammar. The `;;` lines are
nexus's existing line-comment form — the parser drops them.

```
   ;; bootstrap.nexus — the kinds that describe kinds.

   ;; Kind index slot 0 — Kind itself. Struct-shaped, has three
   ;; fields (declared below at Field index 0, 1, 2).
   (Kind Struct [$0 $1 $2] None)

   ;; Kind index slot 1 — Field. Struct-shaped, three fields.
   (Kind Struct [$3 $4 $5] None)

   ;; Kind index slot 2 — Variant. Struct-shaped, three fields.
   (Kind Struct [$6 $7 $8] None)

   ;; Kind index slot 3 — TypeExpression.
   (Kind Struct [$9 $10 $11 $12] None)

   ;; Kind index slot 4 — KindShape (an enum).
   (Kind Enum None [$0 $1])

   ...

   ;; Field index slot 0 — Kind.shape (position 0 of Kind).
   (Field $0 0 $0)
   ;;        ^   ^   ^
   ;;        │   │   └── slot in TypeExpression index — "ref to KindShape"
   ;;        │   └────── position 0
   ;;        └────────── slot in Kind index — "the Kind kind"

   ;; Field index slot 1 — Kind.fields (position 1).
   (Field $0 1 $1)

   ;; Field index slot 2 — Kind.variants (position 2).
   (Field $0 2 $2)
```

Each `$N` is just an integer slot literal. Which index it lives
in is determined by the position the literal appears at. The
first arg of `(Field …)` is `Slot<Kind>` per Field's schema, so
its `$0` is in the Kind index; the third arg is
`Slot<TypeExpression>`, so its `$0` is in the TypeExpression
index. The literals are written by the author counting
declarations.

---

## 5 · Self-reference and mutual recursion

The Kind kind has Fields that point back at it. Forward and
backward references work because all slots are deterministic the
moment the file is fixed.

```
                       Kind index                Field index
                       ──────────                ───────────

                       $0 Kind                   $0 shape
                          ├─ shape ──────────────  ├─ of_kind = $0 ────┐
                          ├─ fields ─→ [$0,$1,$2]  ├─ position = 0     │
                          └─ variants ─→ None      └─ type = $T_shape  │
                              │                                        │
                              │ contains slots in                      │
                              ▼ Field index                            │
                          [$0,$1,$2]                                   │
                              │                                        │
                              │ each Field's of_kind                   │
                              ▼ points back at Kind $0                 │
                                                                       │
                                                                       │
                                  ←──── self-loop closes here ─────────┘
```

No bootstrap problem: the cycle exists in the *records* but not
in the *parse order*. The parser sees Kind $0's `fields = [$0,$1,$2]`
list when it hasn't yet seen Field $0, $1, $2. That's fine — the
author wrote the integers because they know the order. Validation
happens once both ends are present.

---

## 6 · Strings, languages, and Localization

Sema is string-free except inside `Localization` records.
Localizations attach display names to slots, per language.

```
   Localization index
   ──────────────────
   $0  target=Kind:$0  language=English  text="Kind"
   $1  target=Kind:$0  language=French   text="Type"
   $2  target=Kind:$1  language=English  text="Field"
   $3  target=Field:$0 language=English  text="shape"
   $4  target=Field:$0 language=French   text="forme"
   ...
```

`target` carries a typed slot reference. A Localization for a
Kind targets a `Slot<Kind>`; for a Field, a `Slot<Field>`; etc.
(Whether `target` is `Slot<AnyKind>` or a typed enum is open —
see §9.)

Switching the user's language re-skins all UI labels and changes
nexus's surface text without touching any non-Localization record.

---

## 7 · Schema as data, prism as projector

The full self-hosting loop:

```
   ┌──────────────────────────────────────────────────────────────────┐
   │                                                                  │
   │   bootstrap.nexus       kinds.nexus                              │
   │   (universal)           (project)                                │
   │         │                   │                                    │
   │         └─────┬─────────────┘                                    │
   │               ▼                                                  │
   │     criome loads in order                                        │
   │               │                                                  │
   │               ▼                                                  │
   │   ┌──────────────────────────────┐                               │
   │   │  sema                        │                               │
   │   │  Kind / Field / Variant      │   ← schema records            │
   │   │  TypeExpression / KindShape  │     (no strings)              │
   │   │  Localization                │   ← strings live here only    │
   │   └──────────────┬───────────────┘                               │
   │                  │                                               │
   │                  ▼                                               │
   │              ┌───────┐                                           │
   │              │ prism │   reads schema records, emits             │
   │              │       │   Rust source for every domain kind       │
   │              └───┬───┘                                           │
   │                  │                                               │
   │                  ▼                                               │
   │            signal/src/                                           │
   │            (small hand-written bootstrap set                     │
   │             plus prism-emitted domain types)                     │
   │                  │                                               │
   │                  ▼                                               │
   │                rustc                                             │
   │                  │                                               │
   │                  ▼                                               │
   │             new binary                                           │
   │             reads same sema; knows new kinds                     │
   │                                                                  │
   └──────────────────────────────────────────────────────────────────┘
```

The system's type vocabulary is data the system itself stores;
the system's compiler-of-records-to-code is prism, which is
already in the architecture as a separate concern. The schema
layer is the same dog-fooding loop that the engine planned for
domain code — applied to itself.

---

## 8 · The hand-written bootstrap set

`signal/src/` keeps a small set of types written by hand. These
are the types whose definitions describe schema itself; without
them, prism has nothing to read.

```
   ┌─────────────────────────────────────────────────┐
   │  hand-written in signal/src/                    │
   │  (these types ARE the schema vocabulary)        │
   │                                                 │
   │   Kind                                          │
   │   Field                                         │
   │   Variant                                       │
   │   TypeExpression                                │
   │   KindShape                                     │
   │   Primitive                                     │
   │   Localization                                  │
   │   Language                                      │
   │   Slot<T>                                       │
   │                                                 │
   │  Plus the rkyv/nota-codec derives.              │
   │  Estimate: 150-200 lines total.                 │
   └─────────────────────────────────────────────────┘

   ┌─────────────────────────────────────────────────┐
   │  prism-emitted in signal/src/                   │
   │  (these types are projected from records)       │
   │                                                 │
   │   Node, Edge, Graph, Principal, Theme,          │
   │   Layout, Tweaks, Keybind, … and every          │
   │   future domain kind.                           │
   └─────────────────────────────────────────────────┘
```

Today's `signal/src/` mixes these — Node, Edge, Graph, etc. are
hand-written. The migration target moves all of them into the
prism-emitted set, leaving the hand-written set ≈ 150 lines.

---

## 9 · Open shapes

Decisions still pending — flagging in this report so they don't
get baked silently:

- **Bootstrap source: file vs compile-time constants.**
  - File-on-disk: criome reads `bootstrap.nexus` at init.
    Schema is data even at the bootstrap layer.
  - Compile-time constants: prism reads `bootstrap.nexus`
    *offline*, emits a static array of Records into a Rust
    constant; criome's init asserts that array. Author still
    writes nexus text, binary has no startup file dependency.
  - The compile-time-constants path likely wins on
    introspection-doesn't-suffer + atomic-with-binary, while
    keeping the authoring surface as nexus text.

- **`Kind` shape: single record-kind with optional fields, or
  split into `StructKind` + `EnumKind`.**
  Single matches Li's "struct with optional fields covering
  every possible kind of struct (and enums)" phrasing literally;
  cost is leaving "exactly one of fields/variants is set" to
  validation. Split makes that invariant type-enforced; cost is
  two record kinds where one was natural.

- **`TypeExpression` shape: same trade.**
  Single record-kind with `primitive | kind | constructor +
  arguments` as four optional fields, vs three separate kinds
  (`PrimitiveType`, `KindReferenceType`,
  `GenericApplicationType`).

- **`Localization.target` typing.**
  `Slot<AnyKind>` (type-erased; criome validates target kind at
  write time) vs a typed enum
  `Target = TargetKind(Slot<Kind>) | TargetField(Slot<Field>) |
  TargetVariant(Slot<Variant>)`. Type-erasure simpler;
  type-enforced safer.

- **Per-kind tables in sema.**
  Required to make this architecture work; matches bd issue
  `mentci-next-7tv` ("M1 — per-kind sema tables, replace 1-byte
  kind discriminator"). The deterministic-loading guarantee
  depends on each kind having its own counter, which is what
  per-kind tables give.

- **Two files vs one.**
  Bootstrap kinds (universal across every sema everywhere) and
  domain kinds (project-specific) probably want separate files
  so bootstrap stays stable. Both feed the same per-kind
  indexes; the boundary between them is "are these slots
  reserved for every sema, or for this project."

---

## 10 · One-paragraph summary

Each record kind has its own slot index counting from zero.
A canonical bootstrap file (universal kinds) and a project file
(domain kinds) declare records in a fixed top-to-bottom order,
processed before criome opens its UDS listener. The slot id
assigned to each assertion is determined by its position in the
file — so cross-references are integer slot literals the author
predicts from the file structure, including forward references
and self-references. The schema records that result live in
sema; prism reads them and emits Rust source; rustc compiles a
new binary that reads the same sema. The schema is data, the
projection is dog-fooded, and the hand-written set in
`signal/src/` shrinks to ≈ 150 lines covering Kind, Field,
Variant, TypeExpression, KindShape, Primitive, Localization,
Language, and `Slot<T>`.

---

*End report 122.*
