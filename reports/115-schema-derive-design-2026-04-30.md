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
| `Slot` | `SlotRef { of_kind: ??? }` | **needs annotation** — the *kind* the slot points at is semantic, not in the type |
| `Vec<T>` | `List { item: T's FieldType }` | recursive |
| `Option<T>` | `Optional { inner: T's FieldType }` | inferred |
| `RelationKind` (enum) | `Enum { variants: [...] }` | inferred — enum's own `Schema` derive emits the variant list |
| `IntentToken / GlyphToken / StrokeToken / ActionToken / SizeIntent` | `Enum { variants: [...] }` | same — every `NotaEnum` also derives `Schema` |
| nested struct (e.g. `KeybindEntry` inside `KeybindMap.bindings`) | `Record { kind: "KeybindEntry" }` | inferred from type name |

The annotation needed for slot fields:

```
   pub struct Edge {
       #[schema(refs = "Node")]  pub from: Slot,
       #[schema(refs = "Node")]  pub to:   Slot,
       pub kind: RelationKind,
   }

   pub struct Tweaks {
       #[schema(refs = "Principal")]  pub principal: Slot,
       #[schema(refs = "Theme")]      pub theme:     Slot,
       #[schema(refs = "Layout")]     pub layout:    Slot,
       #[schema(refs = "KeybindMap")] pub keybinds:  Slot,
   }
```

Without `#[schema(refs = "...")]`, the macro can't know that
`Edge.from` references a Node and not, say, a Graph. Every `Slot`
field gets exactly one annotation.

(`Vec<Slot>` like `Graph.nodes` follows the same pattern:
`#[schema(refs = "Node")]` on the field, item-kind inferred from the
annotation.)

---

## 2 · The hard parts — what the derive can NOT do alone

```
   ┌─────────────────────────────────────────────────────────┐
   │  (a) which kind a Slot points at                        │
   │       Slot is a u64 newtype — kind information is       │
   │       semantic, not type-level. Solution: per-field     │
   │       #[schema(refs = "Kind")] attribute (above).       │
   │                                                         │
   │  (b) which RelationKind variants are valid between      │
   │       a (source kind, target kind) pair                 │
   │       e.g. "an Edge of kind Contains makes sense        │
   │       Graph→Node but not Node→Graph". This is a         │
   │       semantic relation, not derivable from types.      │
   │       Solution: a hand-authored                         │
   │       valid_relations.rs table in signal,               │
   │       OR data records in sema once schema-in-sema       │
   │       lands. For the first version: hand-authored       │
   │       table; revisit when sema records the rule.        │
   │                                                         │
   │  (c) which fields are "user-editable" vs "derived"      │
   │       constructor flows want to ask the user only for   │
   │       editable fields. Today: every field is user-      │
   │       editable. Future kinds (e.g. an outcome-record    │
   │       with a hash field criome computes) need a         │
   │       #[schema(derived)] marker.                        │
   └─────────────────────────────────────────────────────────┘
```

Each is small and named. None blocks the first version.

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
