# 115 — Schema derive: how mentci-lib learns signal's shapes

*Answer to "wiring `CompiledSchema` to signal's compile-time record
types — what would that look like? a proc-macro?" (Li, 2026-04-30).
Yes — proc-macro. The shape, the hard parts, and the path from
compile-time catalog to schema-in-sema. Lifetime: until the derive
lands and `mentci-lib::schema` reads it; then this folds into
[`nota-derive`](../repos/nota-derive/)'s `ARCHITECTURE.md` or gets
deleted.*

---

## 0 · The shape in one picture

```
   ┌─── signal/src/{flow,identity,style,layout,keybind}.rs ───┐
   │                                                          │
   │   each record-kind type already derives:                 │
   │     Archive · RkyvSerialize · RkyvDeserialize · NotaRecord
   │                                                          │
   │   add one more: Schema                                   │
   │                                                          │
   └────────────────┬─────────────────────────────────────────┘
                    │ proc-macro emits, at compile time:
                    │
                    ▼
   ┌─── nota-derive (existing proc-macro crate) ──────────────┐
   │                                                          │
   │   #[derive(Schema)] on Node emits a static               │
   │   KIND_DESCRIPTOR — name + fields + variant lists        │
   │                                                          │
   └────────────────┬─────────────────────────────────────────┘
                    │ exposed as:
                    │   trait Kind { const NAME; const FIELDS; … }
                    │
                    ▼
   ┌─── signal (re-exports) ──────────────────────────────────┐
   │                                                          │
   │   pub const ALL_KINDS: &[KindDescriptor] = &[            │
   │     Node::DESCRIPTOR,                                    │
   │     Edge::DESCRIPTOR,                                    │
   │     Graph::DESCRIPTOR,                                   │
   │     Principal::DESCRIPTOR,  Tweaks::DESCRIPTOR,          │
   │     Theme::DESCRIPTOR,      KindStyle::DESCRIPTOR, …     │
   │   ];                                                     │
   │                                                          │
   └────────────────┬─────────────────────────────────────────┘
                    │
                    ▼
   ┌─── mentci-lib/src/schema.rs ─────────────────────────────┐
   │                                                          │
   │   impl CompiledSchema for SignalCatalog:                 │
   │     kinds()           → walk ALL_KINDS                   │
   │     fields_of(name)   → look up in ALL_KINDS             │
   │     valid_relations() → from RelationKind's enum         │
   │                                                          │
   └────────────────┬─────────────────────────────────────────┘
                    │
                    ▼
   constructor flows surface real kind palettes,
   real field lists, real enum variant choices —
   no more hardcoded `["Node"]`
```

The derive is the bridge. Everything else falls out of it.

---

## 1 · Type → FieldType mapping

What the proc-macro infers automatically from a Rust field declaration,
and what it cannot:

| Rust field | FieldType emitted | source of truth |
|---|---|---|
| `String` | `Text` | inferred from type name |
| `bool` | `Bool` | inferred |
| `u8 / u16 / u32 / u64 / i64` | `Integer` | inferred |
| `f32 / f64` | `Float` | inferred |
| `Slot<T>` | `SlotRef { of_kind: T's name }` | inferred — `T` carries the kind |
| `Vec<T>` | `List { item: T's FieldType }` | recursive |
| `Option<T>` | `Optional { inner: T's FieldType }` | inferred |
| `RelationKind` (enum) | `Enum { variants: [...] }` | inferred — enum's own `Schema` derive emits the variant list |
| `IntentToken / GlyphToken / StrokeToken / ActionToken / SizeIntent` | `Enum { variants: [...] }` | same — every `NotaEnum` also derives `Schema` |
| nested struct (e.g. `KeybindEntry` inside `KeybindMap.bindings`) | `Record { kind: "KeybindEntry" }` | inferred from type name |

The shape that carries the kind in the type:

```
   Edge                            Tweaks
   ────────────────────────        ────────────────────────────
   from : Slot<Node>               principal : Slot<Principal>
   to   : Slot<Node>               theme     : Slot<Theme>
   kind : RelationKind             layout    : Slot<Layout>
                                   keybinds  : Slot<KeybindMap>

   Graph
   ────────────────────────────
   title     : String
   nodes     : Vec<Slot<Node>>
   edges     : Vec<Slot<Edge>>
   subgraphs : Vec<Slot<Graph>>
```

`Slot<T>` is a phantom-typed newtype — the wire format is still a
plain u64 (the `T` is compile-time only), but the type system carries
the kind. Every reading of a Slot field tells the type system, the
macro, and the human reader the same thing. No annotation; the type
*is* the annotation.

This eliminates the (a) hard part below at the type-system level —
see §2.1.

---

## 2 · The hard parts — what the derive can NOT do alone

A bit of context for a reader new to the engine before the three
hard parts make sense:

- Every record stored in the database (**sema**) has an identity
  called a **Slot** — a stable u64 number that always points at
  *this specific record* even when the record's contents change.
  Records reference each other by Slot.
- An **Edge** is a record kind with three fields: `from: Slot`,
  `to: Slot`, and `kind: RelationKind`. The two slots are the
  endpoints; the relation says what kind of connection it is.
- The user shapes records through **constructor flows** — modal
  dialogs in the workbench where you pick a kind, fill in fields,
  and hit Commit. The schema layer tells the modal *which fields
  exist, what types they have, and what valid choices are*. The
  schema layer is the thing this whole report is about.

The proc-macro can mechanically derive a lot from a Rust struct
definition (string fields become text inputs, enum fields become
dropdowns, integer fields become number inputs, etc.). But three
pieces of information sit *outside* the type system — they live in
the meaning of the design, not in any type signature — so the macro
can't see them. Each is small and named.

### 2.1 (a) Which kind a Slot points at — solved by phantom-typing the Slot

**The situation as it exists today.** `Slot` in signal is a bare u64
newtype. From the type system's view, every Slot looks identical —
there is no compile-time difference between "a slot pointing at a
Node" and "a slot pointing at a Theme." That difference matters
semantically: an Edge's `from` slot must point at a Node; a Tweaks
record's `theme` slot must point at a Theme record.

```
   today's type signature — every Slot looks the same:

       Edge
       ─────────────────────────────────────
       from : Slot         ← which kind?
       to   : Slot         ← which kind?
       kind : RelationKind
```

**The concrete cost.** The user drag-wires between two canvas
elements to create an Edge. The constructor needs to populate the
"from" picker with valid candidates. With every Slot looking
identical, the picker has two bad options: show *every* slot in
sema (overwhelming and mostly nonsense) or silently fall back to
"any slot," which lets the user create valid-looking but
semantically broken Edges.

```
   today (Slot is bare u64):

   "from" picker offers ALL slots:

     ▢ Theme "Sunset"
     ▢ Layout "compact"
     ▢ Node "ticks"           ✓ what the user actually wants
     ▢ Principal "operator"
     ▢ KeybindMap "default"
     ▢ Node "double"          ✓
     …
```

**The fix is to make Slot carry the kind.** Phantom-type the
newtype: `Slot<T>` instead of `Slot`. The `T` is compile-time only;
the wire format stays a plain u64. But every place a Slot appears,
the type names what it points at:

```
   tomorrow (Slot phantom-typed):

   Edge { from: Slot<Node>, to: Slot<Node>, kind: RelationKind }

       the macro reads `Slot<Node>` and emits
       FieldType::SlotRef { of_kind: "Node" } directly.
       no annotation needed.
```

Three things fall out of this change:

```
   ┌── before (Slot is bare u64) ──┐  ┌── after (Slot<T>) ─────────────────┐
   │                                │  │                                    │
   │  picker has to be told the     │  │  picker reads `Slot<Node>` and     │
   │  kind via annotation           │  │  knows the kind directly           │
   │                                │  │                                    │
   │  passing a Theme-slot where    │  │  passing a Theme-slot where        │
   │  a Node-slot is expected       │  │  a Node-slot is expected is a      │
   │  compiles — silent bug         │  │  type error — caught at compile    │
   │                                │  │                                    │
   │  the kind information lives    │  │  the kind information lives        │
   │  in attribute metadata —       │  │  inside the type — single source   │
   │  parallel to the type system   │  │  of truth                          │
   │                                │  │                                    │
   └────────────────────────────────┘  └────────────────────────────────────┘
```

The wire format does not change — `Slot<T>` archives identically
to `Slot` (one u64; the phantom is purely compile-time). nota-codec's
`NotaTransparent` derive renders `Slot<T>` as a bare integer in
nexus text, same as today. The change is *internal* to Rust.

**What this means for the schema derive.** The proc-macro reads
field types from the syntax tree. `Slot<Node>` parses as a typed
path with `Node` as a generic argument; the macro extracts "Node"
mechanically. No `#[schema(refs = "...")]` attribute, no
hand-maintained parallel list. The kind information is in exactly
one place (the type), and every consumer — the type checker, the
schema derive, the human reader — reads it from there.

**Cost.** This is a signal-level type-design change. It touches:

```
   ┌── signal/src/slot.rs ─────────────────────────────────────────┐
   │  Slot becomes Slot<T> with PhantomData<fn() -> T>             │
   │  NotaTransparent derive still renders as bare u64             │
   └───────────────────────────────────────────────────────────────┘
   ┌── signal/src/{flow,identity,style,layout,keybind,tweaks}.rs ──┐
   │  every `: Slot` field becomes `: Slot<Kind>`                  │
   │  every `Vec<Slot>` becomes `Vec<Slot<Kind>>`                  │
   └───────────────────────────────────────────────────────────────┘
   ┌── signal::Records variants ───────────────────────────────────┐
   │  Records::Node(Vec<(Slot, Node)>) becomes                     │
   │    Records::Node(Vec<(Slot<Node>, Node)>) — the type          │
   │    parameter is now redundant with the variant, which is      │
   │    fine: the type system rechecks the consistency for free    │
   └───────────────────────────────────────────────────────────────┘
   ┌── criome's reader.rs decode_kind<T> ──────────────────────────┐
   │  the conversion sema::Slot → signal::Slot<T> is monomorphic   │
   │  per kind — each find_nodes / find_edges / find_graphs        │
   │  knows its T. drop-in change.                                 │
   └───────────────────────────────────────────────────────────────┘
   ┌── mentci-lib's WorkbenchState + cache + canvas ──────────────-┐
   │  WorkbenchState.principal: Slot becomes Slot<Principal>       │
   │  ModelCache fields stay homogeneous-per-kind                  │
   │  RenderedEdge.from / .to become Slot<Node>                    │
   └───────────────────────────────────────────────────────────────┘
```

A genuinely heterogeneous Slot list (e.g., a future "pinned items"
record holding slots of mixed kinds) is handled by either type
erasure into a `RawSlot` newtype, or — per the perfect-specificity
invariant — a typed enum naming the kinds it can hold. Both shapes
exist; both are clean. The need is hypothetical today.

**The honest accounting.** This was the right answer the first
time and the report was wrong to reach for the annotation pattern.
Annotation lists are how you do it when you don't control the
underlying type (ORM frameworks over `i64` foreign keys, for
example). Here we own `Slot`; phantom-typing it is the answer the
type system was already asking for.

### 2.2 (b) Which RelationKind variants make sense between which kinds

**The situation.** Every Edge carries a `kind: RelationKind` — a
closed enum of nine values:

```
   RelationKind
   ─────────────
   Flow · DependsOn · Contains · References ·
   Produces · Consumes · Calls · Implements · IsA
```

Not every relation makes sense between every pair of node kinds. A
Graph *contains* Nodes (`Contains` is sensible Graph→Node); a Node
does not contain a Graph (`Contains` is nonsense Node→Graph).
`Implements` makes sense between two Nodes representing an
interface and an implementation; it does not make sense between a
Theme and a Layout.

**Concrete example.** The user drags a wire from a Graph onto a
Node and the kind picker pops up. Without a valid-relations table,
the picker shows all nine variants. The user might pick `IsA`,
producing an Edge that says "this Graph IS-A Node." That Edge is
well-typed — it serialises, it round-trips on the wire, criome
stores it without complaint — but it is *semantically* wrong, and
nothing in the engine catches it.

```
   from-kind: Graph,  to-kind: Node      from-kind: Node,  to-kind: Node
   ────────────────────────────────      ──────────────────────────────

   no table — every variant offered:     no table — every variant offered:

     Flow         ✓ sensible               Flow         ✓ sensible
     DependsOn    ✓ sensible               DependsOn    ✓ sensible
     Contains     ✓ sensible               Contains    nonsense
     References   ✓ sensible               References   ✓ sensible
     Produces    nonsense                  Produces     ✓ sensible
     Consumes    nonsense                  Consumes     ✓ sensible
     Calls       nonsense                  Calls        ✓ sensible
     Implements  nonsense                  Implements   ✓ sensible
     IsA         nonsense                  IsA          ✓ sensible

   with table — only sensible offered:    with table — only sensible offered:

     Flow                                   Flow
     DependsOn                              DependsOn
     Contains                               References
     References                             Produces
                                            Consumes
                                            Calls
                                            Implements
                                            IsA
```

**Why the macro can't see it.** The information is a relation
between *three* things — source-kind, target-kind, and
relation-kind. None of those three sit on `RelationKind`'s enum
definition. The macro can list the nine variants automatically (it
already does, by deriving `Schema` on the enum); it cannot infer
"Contains is invalid Node→Graph" from any type-level signal.

**The resolution.** A small hand-authored table — one entry per
sensible (source-kind, target-kind, RelationKind) triple. The
constructor flow consults the table when picking which variants to
offer. The table lives in signal (where both the kinds and the
RelationKind enum live) and grows when a new kind or relation
lands.

The longer-term shape: each (source, target, relation) triple
becomes its own record in sema (something like
`RelationKindRule { … }`), and the schema layer queries the records
instead of reading a hand-authored table. That folds into the same
schema-in-sema path as §4.

### 2.3 (c) Which fields are user-editable vs engine-computed

**The situation.** Every record kind has fields. Today, every field
is something the user fills in — Node has a `name`, Edge has its
endpoints and relation, Graph has a `title`. The constructor flow
asks for each field and lets the user supply a value.

But the engine will grow record kinds whose fields are *computed*,
not entered. The constructor flow has to know the difference, or it
will prompt the user for things the user has no way to know.

**Concrete example.** When the build flow lands
([criome ARCH §7.3](../repos/criome/ARCHITECTURE.md#73-build-post-mvp--the-milestone-flow)),
the user requests a build of a Graph; forge compiles it; criome
asserts a `CompiledBinary` record describing the result:

```
   CompiledBinary
   ────────────────────────────────────────────────────────────────
   graph     : Slot   ← user-meaningful (the Graph that was built —
                        comes from the user's BuildRequest)
   arca_hash : Hash   ← engine-computed (arca-daemon's blake3 of
                        the actual binary on disk)
   narhash   : Hash   ← engine-computed (nix's store-path hash)
   wall_ms   : u64    ← engine-measured (how long forge spent
                        building, in milliseconds)
```

If the constructor flow surfaced "Create a new CompiledBinary
record" and asked the user for all four fields:

- the user has no way to type the right `arca_hash` — forge produces
  it during the build, after the user's input is gone
- typing a wrong hash means the record points at content that
  doesn't match (the workbench shows the artifact's name; the
  filesystem stores a different blob; reads silently break)
- typing `wall_ms` is meaningless — there's no event being timed

The right shape: don't surface CompiledBinary as a user-creatable
kind in the generic "+ new record" picker at all. It only ever
appears as the *outcome* of a specific verb (BuildRequest), and the
constructor for *that* verb asks the user only for the user-meaningful
inputs (which Graph to build).

**Why the macro can't see it.** The struct definition gives no
hint: from the type system's view, `CompiledBinary` is just a record
with four fields, all of them serialisable. Whether each field is
"the user supplies this" or "the engine computes this" is a property
of the build pipeline (forge produces hashes; the user does not),
not a property of the field's type.

**The resolution.** Two complementary attributes, used per field
or per kind:

- `#[schema(derived)]` on a field — this field is engine-computed.
  The constructor hides it (or shows it read-only with a "computed
  by …" hint). The user cannot supply a value.
- `#[schema(only_via = "BuildRequest")]` on a kind — this whole
  record kind is asserted only as the outcome of a named verb.
  The generic "+ new record" picker hides the kind entirely; the
  kind shows up as the *reply* to executing the verb.

For the record kinds wired today (Node, Edge, Graph, Principal,
Theme, Layout, NodePlacement, KeybindMap, KindStyle,
RelationKindStyle, Tweaks) every field is user-editable, so neither
attribute is needed yet. They land alongside the first engine-
computed kind — most likely `CompiledBinary` when the build flow
goes in.

---

Each of (a), (b), (c) is small, named, and visible in the design.
None blocks the first version: a derive macro that handles only the
mechanically-derivable cases is already enough to replace the
hardcoded `["Node"]` palette with a real catalog. The semantic
attributes get added field-by-field as each missing piece surfaces.

---

## 3 · Where the macro lives

Two options — Li's call:

```
   ┌──────────────────────────────────┬───────────────────────────────┐
   │  Option A — extend nota-derive   │  Option B — new signal-derive │
   ├──────────────────────────────────┼───────────────────────────────┤
   │  + crate already exists, already │  + perfect-component-isolation│
   │    sees every record kind        │    schema introspection has   │
   │  + every record already derives  │    different consumers from   │
   │    NotaRecord; one more derive   │    text codec (mentci-lib's   │
   │    next to it is uniform         │    UI vs nexus-daemon)        │
   │  + faster to ship                │  − new repo + flake + tests   │
   │                                  │    setup cost                 │
   │  − couples schema introspection  │                               │
   │    to the codec crate's release  │                               │
   │    cadence                       │                               │
   └──────────────────────────────────┴───────────────────────────────┘
```

My read: **Option A**. nota-derive already owns derives over signal's
record types; schema introspection is naturally adjacent to "knows the
shape well enough to encode/decode." The micro-components rule isn't
fundamentally violated — derives over Rust types are one capability
("emit metadata about typed records"), nota-derive is the noun for
that capability.

---

## 4 · Bootstrap path — proc-macro today, schema-in-sema tomorrow

```
   today
   ─────

   compile-time:
     #[derive(Schema)] on each record kind
       │
       ▼
     pub const ALL_KINDS: &[KindDescriptor]
       │
       ▼
     mentci-lib::SignalCatalog implements CompiledSchema
       by walking ALL_KINDS
       │
       ▼
     constructor flows narrow to real kinds + real fields


   medium-term ([criome ARCH §11 "Open shapes"](../repos/criome/ARCHITECTURE.md#11--open-shapes))
   ───────────

   build-time:
     #[derive(Schema)] still emits ALL_KINDS
       │
       ▼
   first-run boot:
     helm reads ALL_KINDS, formats a `kinds.nexus` seed:
       (Assert (KindDecl name:"Node" fields:[(FieldDecl …)]))
       (Assert (KindDecl name:"Edge" fields:[(FieldDecl …)]))
       …
       │
       ▼
     piped through nexus-cli into criome (same path as
     genesis.nexus, see [reports/114 §4.2](114-mentci-stack-supervisor-draft-2026-04-30.md))
       │
       ▼
   runtime:
     mentci-lib's CompiledSchema impl now queries sema for
     KindDecl records instead of reading the compile-time
     catalog. Same trait, different implementation.
       │
       ▼
   the engine knows its own schema as data. Adding a kind at
   the user's discretion (without a recompile) becomes
   plausible.
```

The proc-macro is not a detour — it's the *seed* that boots
schema-in-sema. The compile-time catalog becomes the bootstrap data
for the runtime catalog.

---

## 5 · One question

**Q1 — Option A or Option B in §3?** I'd default to A (extend
nota-derive). Confirm or correct?

(All other choices — the `#[schema(refs = "...")]` attribute, the
per-field annotation discipline, the staged path to schema-in-sema —
flow from the §0 picture and don't need answers before skeleton lands.)

---

*End report 115.*
