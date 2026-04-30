# 118 — Signal-derive: what the macro emits

*Answer to "what kind of code does signal-derive produce?" — a
visual of the codegen pipeline plus worked examples for each
shape the macro handles. Lifetime: until the derive's lowering
rules drift enough that the examples stop matching reality; then
this folds into signal-derive's `ARCHITECTURE.md` or gets deleted.*

---

## 0 · TL;DR

`#[derive(Schema)]` emits one `impl signal::Kind for T` block per
deriving type. The block carries a single `const DESCRIPTOR:
KindDescriptor` describing the type's shape — name, fields, field
types, enum variants — at compile time. No runtime work; just a
const that consumers walk.

The shape mirrors the source mechanically: every Rust field
becomes a `FieldDescriptor`, every variant becomes a string in a
variants list. Compound types collapse: `Vec<T>` sets
`is_list: true`, `Option<T>` sets `is_optional: true`, `Slot<T>`
becomes `SlotRef { of_kind: "T" }`. Anything not explicitly
recognised becomes `Record { kind_name: "T" }` — deferred
resolution at the consumer side via `ALL_KINDS`.

---

## 1 · The pipeline in one picture

```
   source                       signal-derive (proc-macro)              emitted at derive site
   ──────                       ───────────────────────────              ─────────────────────

   #[derive(Schema, …)]          ┌─ parse_macro_input as DeriveInput
   pub struct Node {             ├─ dispatch on Data::{Struct, Enum}
       pub name: String,         ├─ struct_shape walks named fields
   }                             ├─ lower_field_type per field:
                                 │    Option<T> wrap → is_optional
                                 │    Vec<T>    wrap → is_list
                                 │    primitives  → FieldType variants
                                 │    Slot<Kind>  → SlotRef{of_kind}
                                 │    other named → Record{kind_name}
                                 │
                                 ▼
                                 ┌──────────────────────────────────┐
                                 │  TokenStream of                  │
                                 │  impl ::signal::Kind for Node {  │
                                 │      const DESCRIPTOR: KindDescriptor
                                 │          = KindDescriptor { … }; │
                                 │  }                               │
                                 └──────────────────────────────────┘
                                                 │ injected at
                                                 │ derive expansion site
                                                 ▼
                                          impl ::signal::Kind for Node {
                                              const DESCRIPTOR = …;
                                          }
```

The macro never depends on `signal` itself (proc-macro crates
can't depend on a crate that depends on them). It emits absolute
paths like `::signal::FieldType::Text` and trusts that `signal`
re-exports the schema types. `signal/src/lib.rs` includes
`extern crate self as signal;` so those paths resolve from inside
signal too.

---

## 2 · Field-type lowering — at a glance

| Rust shape | emitted FieldDescriptor parts |
|---|---|
| `name: String` | `field_type: FieldType::Text`, `is_optional: false`, `is_list: false` |
| `flag: bool` | `field_type: FieldType::Bool`, both flags false |
| `n: u8 / u16 / u32 / u64 / i8 / i16 / i32 / i64` | `field_type: FieldType::Integer` |
| `x: f32 / f64` | `field_type: FieldType::Float` |
| `from: Slot<Node>` | `field_type: FieldType::SlotRef { of_kind: "Node" }` |
| `slot: Slot<AnyKind>` | `field_type: FieldType::AnyKind` |
| `kind: RelationKind` (any non-built-in named type) | `field_type: FieldType::Record { kind_name: "RelationKind" }` |
| `nodes: Vec<Slot<Node>>` | `field_type: SlotRef { of_kind: "Node" }`, `is_list: true` |
| `note: Option<String>` | `field_type: Text`, `is_optional: true` |
| `tags: Vec<Option<String>>` | `field_type: Text`, `is_list: true`, `is_optional: true` |

The `is_optional` and `is_list` flags compose: `Vec<Option<T>>`
flattens to both flags true on T's mapping. `Option<Vec<T>>`
collapses the same way (the macro recurses through both wrappers).

---

## 3 · Worked examples

### 3.1 Simplest — `Node`

Source:

```rust
#[derive(Schema, /* …other derives… */)]
pub struct Node {
    pub name: String,
}
```

Emitted:

```rust
impl ::signal::Kind for Node {
    const DESCRIPTOR: ::signal::KindDescriptor = ::signal::KindDescriptor {
        name: "Node",
        shape: ::signal::KindShape::Record {
            fields: &[
                ::signal::FieldDescriptor {
                    name: "name",
                    field_type: ::signal::FieldType::Text,
                    is_optional: false,
                    is_list: false,
                },
            ],
        },
    };
}
```

One field, one descriptor, primitive `String` → `Text`.

### 3.2 With `Slot<Kind>` references — `Edge`

Source:

```rust
#[derive(Schema, /* … */)]
pub struct Edge {
    pub from: Slot<Node>,
    pub to: Slot<Node>,
    pub kind: RelationKind,
}
```

Emitted:

```rust
impl ::signal::Kind for Edge {
    const DESCRIPTOR: ::signal::KindDescriptor = ::signal::KindDescriptor {
        name: "Edge",
        shape: ::signal::KindShape::Record {
            fields: &[
                ::signal::FieldDescriptor {
                    name: "from",
                    field_type: ::signal::FieldType::SlotRef { of_kind: "Node" },
                    is_optional: false,
                    is_list: false,
                },
                ::signal::FieldDescriptor {
                    name: "to",
                    field_type: ::signal::FieldType::SlotRef { of_kind: "Node" },
                    is_optional: false,
                    is_list: false,
                },
                ::signal::FieldDescriptor {
                    name: "kind",
                    field_type: ::signal::FieldType::Record {
                        kind_name: "RelationKind",
                    },
                    is_optional: false,
                    is_list: false,
                },
            ],
        },
    };
}
```

The macro reads `Slot<Node>` and writes `of_kind: "Node"` directly
— no annotation, no parallel list. `RelationKind` is unrecognised
as a primitive, so it becomes `Record { kind_name: "RelationKind" }`;
the consumer resolves what RelationKind actually IS by looking
it up in `ALL_KINDS`.

### 3.3 With `Vec<Slot<Kind>>` — `Graph`

Source:

```rust
#[derive(Schema, /* … */)]
pub struct Graph {
    pub title: String,
    pub nodes: Vec<Slot<Node>>,
    pub edges: Vec<Slot<Edge>>,
    pub subgraphs: Vec<Slot<Graph>>,
}
```

Emitted (abbreviated — `KindDescriptor` framing identical to the
above):

```rust
fields: &[
    FieldDescriptor {
        name: "title",
        field_type: FieldType::Text,
        is_optional: false,
        is_list: false,
    },
    FieldDescriptor {
        name: "nodes",
        field_type: FieldType::SlotRef { of_kind: "Node" },
        is_optional: false,
        is_list: true,                                  // ← Vec<…>
    },
    FieldDescriptor {
        name: "edges",
        field_type: FieldType::SlotRef { of_kind: "Edge" },
        is_optional: false,
        is_list: true,
    },
    FieldDescriptor {
        name: "subgraphs",
        field_type: FieldType::SlotRef { of_kind: "Graph" },
        is_optional: false,
        is_list: true,
    },
],
```

The `Vec<>` collapses into `is_list: true`; the inner `Slot<Kind>`
keeps its kind information.

### 3.4 Enum with unit variants — `RelationKind`

Source:

```rust
#[derive(Schema, /* … */)]
pub enum RelationKind {
    Flow,
    DependsOn,
    Contains,
    References,
    Produces,
    Consumes,
    Calls,
    Implements,
    IsA,
}
```

Emitted:

```rust
impl ::signal::Kind for RelationKind {
    const DESCRIPTOR: ::signal::KindDescriptor = ::signal::KindDescriptor {
        name: "RelationKind",
        shape: ::signal::KindShape::Enum {
            variants: &[
                "Flow",
                "DependsOn",
                "Contains",
                "References",
                "Produces",
                "Consumes",
                "Calls",
                "Implements",
                "IsA",
            ],
        },
    };
}
```

Just the variant names, in source order. Data-carrying variants
fail compilation with a clear error — the closed-vocabulary shape
forbids them.

---

## 4 · Hypothetical: `Vec<Option<T>>`

No record kind in signal today has this shape, but it's
mechanically supported. Hypothetical:

```rust
#[derive(Schema)]
pub struct Hypothetical {
    pub maybe_nodes: Vec<Option<Slot<Node>>>,
}
```

Emitted descriptor field:

```rust
FieldDescriptor {
    name: "maybe_nodes",
    field_type: FieldType::SlotRef { of_kind: "Node" },
    is_optional: true,           // inner Option<…>
    is_list: true,                // outer Vec<…>
},
```

Both flags compose; the consumer reads "a list whose elements
may individually be absent."

---

## 5 · How consumers use the descriptor

The schema layer in mentci-lib (when wired) walks `ALL_KINDS`
and per-kind descriptors:

```rust
// enumerate every kind in the vocabulary:
let kind_names: Vec<&str> = signal::ALL_KINDS
    .iter()
    .map(|k| k.name)
    .collect();
// → ["Node", "Edge", "Graph", "RelationKind", "Ok",
//    "Principal", "Tweaks", "Theme", "IntentToken", …]

// inspect a specific kind:
let edge = <signal::Edge as signal::Kind>::DESCRIPTOR;
match edge.shape {
    signal::KindShape::Record { fields } => {
        for f in fields {
            // "from : SlotRef { of_kind: \"Node\" }"
            // "to : SlotRef { of_kind: \"Node\" }"
            // "kind : Record { kind_name: \"RelationKind\" }"
            println!("{} : {:?}", f.name, f.field_type);
        }
    }
    signal::KindShape::Enum { variants: _ } => unreachable!(),
}

// resolve a Record reference to the kind it names:
let relation_kind = signal::ALL_KINDS
    .iter()
    .find(|k| k.name == "RelationKind")
    .expect("RelationKind in catalogue");
match relation_kind.shape {
    signal::KindShape::Enum { variants } => {
        // ["Flow", "DependsOn", "Contains", …]
    }
    _ => unreachable!(),
}
```

The two-pass resolution — `Edge.kind` says
`Record { kind_name: "RelationKind" }`, the consumer looks
"RelationKind" up in `ALL_KINDS` to learn it's an enum with nine
variants — keeps the per-type descriptor self-contained while
letting the catalogue do the cross-referencing.

The constructor flow's kind picker, when wired, will use exactly
this pattern: walk `ALL_KINDS` to populate the kind dropdown,
then walk the chosen kind's `FieldDescriptor` list to surface a
field-by-field input form, with each field's `field_type` driving
the input widget (text input, integer input, slot picker filtered
by `of_kind`, enum dropdown via the referenced kind's variants).

---

## 6 · What the macro doesn't emit

By design:

- **No runtime work.** Everything is a `const`; no allocation,
  no lazy initialisation, no atomics. The descriptor is baked
  into the binary at compile time.
- **No reflection over private fields.** Tuple structs are
  rejected with a compile error; the macro requires named fields
  for record-shaped kinds.
- **No data-carrying enum variants.** Closed-vocabulary semantics
  only — variants must be unit. The compile error names the rule.
- **No semantic validation.** "Is this `RelationKind` valid
  Graph→Node?" is not the macro's concern (per [reports/115 §2.2](115-schema-derive-design-2026-04-30.md#22-b-which-relationkind-variants-make-sense-between-which-kinds)
  — the valid-relations table lives elsewhere). The descriptor
  carries shape; meaning lives outside.

---

*End report 118.*
