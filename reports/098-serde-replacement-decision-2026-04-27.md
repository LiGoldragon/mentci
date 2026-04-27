# 098 — Serde for the nexus text layer: build our own, staged

*Per Li 2026-04-27: "Why? I didn't make that decision. I want
to look into this in detail. Why keep serde? Have you looked
at what implementing our own de/serialization logic could give
us, given our elegant and correct async/actor approach? It's
worth considering now."*

This report supersedes the unilateral "keep serde" answer
[reports/095 §4a Q1](095-style-audit-2026-04-27.md) gave
without proper investigation. After deep research on what serde
actually gives vs costs us in this codebase, the answer flips.

**Decision: build our own — staged.** Keep serde for M0 (no
live callers yet, no benefit to forking mid-flight). At the
M0→M1 boundary, replace the serde-driven text path with a
hand-written `Decoder` / `Encoder` framework that aligns with
our perfect-specificity invariant, methods-on-types discipline,
and beauty-as-criterion test.

---

## 1 · The reversal in one paragraph

The earlier "keep serde" answer treated serde's free-function
shape (`from_str`/`to_string`) and string-tagged enum dispatch
as minor cosmetic concessions. They are not minor. The
sentinel-wrapper pattern (Bind / Mutate / Negate / Validate /
Subscribe / AtomicBatch) is **pure serde-fitting ceremony** —
six types that exist only so the serde derive has somewhere to
hang `#[serde(rename = "@NexusBind")]` attributes. The
[QueryParser carve-out in nexus/src/parse.rs](../repos/nexus/src/parse.rs)
is **structural, not incidental** — it will recur every time
we want syntactic dispatch that depends on schema (not Rust
type) identity. List-pattern matching (M1+), constraint forms
(`{| |}`), and any future delimiter family will all need their
own carve-outs. Each carve-out is, per
[programming/beauty.md](../repos/tools-documentation/programming/beauty.md),
evidence that the underlying problem is unsolved.

The right structure — one `Decoder` type with one method per
nexus verb — collapses every special case into the normal case
(the Torvalds linked-list test).

---

## 2 · What serde actually gives us today

Serde gives the project four things, of which only two are
used end-to-end and only one is load-bearing.

### 2.1 Used and load-bearing

`Serialize` / `Deserialize` derive macros that synthesize ~30–50
lines of visitor-driven trait code per record kind. For our M0
kinds (Node, Edge, Graph, RelationKind, KindDecl, Ok, plus the
four `*Query` kinds), that's ~300–500 LoC of free reflection we
don't have to write or maintain. The derive output drives
`Serializer::serialize_struct` to emit `(Node "User")` and
`Deserializer::deserialize_struct` to read it back.

This is the actual win — and it's only a win because of
**volume**: the more kinds we add, the more derive saves.

### 2.2 Used as façade convention

`nota_serde_core::to_string_nexus` / `from_str_nexus`. They
round-trip the simple positional-record cases plus the six
sentinel newtype wrappers in
[nexus-serde/src/lib.rs](../repos/nexus-serde/src/lib.rs)
which use `#[serde(rename = "@NexusBind")]` etc. so the
deserializer's `deserialize_newtype_struct` arm can pattern-
match on the static name and emit/recognize a sigil.

### 2.3 Available but unused in the daemons

`from_str` for query patterns: nexus already wrote a
hand-written [`QueryParser`](../repos/nexus/src/parse.rs)
because `(| Foo @x |)` cannot be expressed in serde's data
model. The `nexus` crate today imports only `Lexer` and
`Token` from `nota-serde-core` — *not* the `Deserializer`. The
serde dispatch path is currently dead code in the daemon graph.

### 2.4 Available but disqualified

`serde_json` / `serde_yaml` interop: useless to us — we never
round-trip via JSON.

### 2.5 The actual ratio

By line count of [nota-serde-core/src/{de,ser}.rs](../repos/nota-serde-core/src/):

- ~30% is actual format logic (lexer-driven token consumption,
  sigil emit, bare-string eligibility, float canonicalization,
  sorted-map output).
- ~70% is **serde-trait scaffolding to fit our format into
  serde's 29-method `Deserializer` trait + visitor pattern +
  `SeqAccess`/`MapAccess`/`EnumAccess`/`VariantAccess`/
  `DeserializeSeed` machinery.**

We are renting space in someone else's data model.

---

## 3 · Where serde fights us

Serde's design centre is "any data model can be matched
against any data structure via a self-describing universal
IR." That centre is exactly what perfect specificity
([criome/ARCHITECTURE.md §2 Invariant D](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md))
refuses.

### 3.1 String dispatch on enum variants

Serde matches `AssertOp::Node(Node)` by reading the string
`"Node"` and calling `variant_seed`. Invariant D explicitly
forbids string-tagged dispatch. Today we get away with it
because the derive emits the variant strings statically, but
the dispatch site inside `de.rs:559-561` reads a token, takes
its text, and asks serde to translate that text back into a
variant id. That's stringly-typed at the trait boundary.

### 3.2 The sentinel-wrapper smell

`Bind`, `Mutate`, etc., exist *only* to give the derive a
`#[serde(rename = "@NexusBind")]` hook so the deserializer can
dispatch on the wrapper name and emit a sigil. The wrappers
carry no semantic information that the verb-typed enums in
[signal/src/edit.rs](../repos/signal/src/edit.rs) don't already
carry. They are pure serde-fitting ceremony — anti-Brooks
"uncoordinated good-but-independent ideas," and they've already
had to grow from 3 to 6 sentinels.

### 3.3 The QueryParser carve-out is structural

The reason `(| Node @name |)` can't be expressed in serde is
that **the field's Rust type is not the field's pattern-position
type**:

- `Node.name` is `String`
- `NodeQuery.name` is `PatternField<String>`
- The bind name `@name` is validated against the *schema field
  name*, not the Rust deserialization machinery.

Serde's `deserialize_struct` can't see schema field names — it
only sees positional `&'static str` field-name slots from the
derive. **This carve-out will recur every time we want
syntactic dispatch that depends on schema, not Rust type
identity.** List-pattern matching (M1+) will hit it. Constraint
forms `{| |}` will hit it. Any future delimiter family hits it.

### 3.4 ADT model mismatch

Serde's data model has 29 types organized as primitives /
sequences / tuples / maps / structs / enums / options / units.
Our model has *records* (positional, head-tagged, schema-typed),
*patterns* (delimited, schema-validated bind names), *requests*
(verb-prefixed), *replies* (typed-per-query), and a closed set
of *sigils*. The mapping is lossy in both directions — serde's
units don't exist for us; multi-field tuple structs don't exist
for us; `deserialize_any` doesn't exist for us. The pattern of
`Err(Error::Custom("…not supported"))` arms in `de.rs:166-168`,
`433-435`, `628-632` is structural — not bugs, but tells.

### 3.5 `from_str` / `to_string` as free functions

The "well-known-libraries" carve-out from the methods-on-types
rule is honest, but it tells us we are paying *aesthetic* cost
to look like serde-ecosystem code that no caller in our
codebase ever sees as serde-ecosystem code. The consumer is
one daemon. The producer is one daemon. Neither shells out to
JSON.

---

## 4 · Cost of replacing it

### 4.1 The choice space — broader than first framed

*Per Li 2026-04-27 correction: there is no "no-macros"
constraint in the current bootstrap era. The
[criome/ARCHITECTURE.md macro section](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)
describes the eventual self-hosting state (sema-rules +
rsc-projection replacing authored macros); during the
bootstrap era, we may author macros where useful, accepting
that they're transitional code that will be replaced by
sema-projection later.*

So the actual implementation choices for Stage 2 are:

1. **Hand-written per-kind methods.** ~30 LoC per kind ×
   ~7 kinds = ~270 LoC. Direct projection target for rsc
   later. No new macro to maintain.
2. **Author our own derive macro.** `#[derive(NexusKind)]`
   that emits per-kind methods on `Decoder` / `Encoder`,
   dispatching on a closed `Token` enum. Roughly equivalent
   ergonomics to `#[derive(Serialize, Deserialize)]`, but
   speaks our data model natively. Transitional by design —
   gets replaced by rsc-projection in the eventual state.
3. **Keep serde, hide the carve-outs.** Status quo +
   cosmetic shims. Doesn't address the structural problems
   in §3.

(1) and (2) both align with the staged plan. Decide between
them at Stage 2 based on volume — if M0→M1 keeps adding
kinds, the derive becomes more attractive; if the kind set
stabilises, hand-written stays direct.

Lighter ecosystem alternatives (miniserde / nanoserde /
serde_lite / borsh-derive) all ship derives that fit
*their* data models, not ours. They share serde's structural
mismatches (variant-name-as-string dispatch, no native
sigils, no schema-typed pattern positions). Adopting any of
them would solve volume cost without solving the
architectural fit problem — false economy.

### 4.2 The cost numbers

For Option 1 (hand-written):

- **Per record kind**: a `Decoder::node(&mut self) -> Result<Node>` and `Encoder::node(&mut self, n: &Node)` pair = ~15 LoC each = ~30 LoC per kind.
- **For M0's seven kinds** (Node, Edge, Graph, KindDecl, NodeQuery, EdgeQuery, GraphQuery, KindDeclQuery, RelationKind): ~270 LoC.
- **Plus the per-verb dispatcher**: one method that reads the first sigil/delimiter and dispatches to the right `decode_*` — ~80–120 LoC, replacing the ~250 LoC of serde trait machinery in `de.rs:161-436`.
- **Lexer stays.** [nota-serde-core/src/lexer.rs](../repos/nota-serde-core/src/lexer.rs) (525 LoC) is pure format logic.

**Net: ~915 LoC vs. today's ~1750 LoC** (de + ser + lexer + nexus-serde façade), and the new code is uniformly *our verb vocabulary* rather than half-format / half-trait-fitting.

For Option 2 (custom derive):

- The derive crate itself: a few hundred LoC of proc-macro
  code, written once.
- Per-kind cost: zero — `#[derive(NexusKind)]` on the struct.
- Total: ~700 LoC including the derive and the lexer.

The derive option *adds* a maintenance burden (the proc-macro
crate) but *removes* the per-kind cost. Worth it if we expect
the kind count to grow significantly post-M0; not worth it if
M0+M1 is the steady-state.

Either option deletes serde, `nota-serde-core` (in current
shape), and `nexus-serde` from the dependency graph. The
resulting crate is one library with the lexer + `Decoder` +
`Encoder` types.

### 4.3 What we cannot determine without running code

Serde-derive's monomorphized output may compile to faster code
than a hand-written matched dispatch in some shapes, and
slower in others. The Wenhe Li WebAssembly post and miniserde's
design notes both observe serde's monomorphization can be a
binary-size and code-bloat liability; the
[rust_serialization_benchmark](https://github.com/djkoloski/rust_serialization_benchmark)
shows hand-written postcard-style codecs are competitive at
runtime. For our text format the dispatch is dominated by the
lexer, not the per-field call.

---

## 5 · What replacing it gives us

### 5.1 Native sigil dispatch

The `Decoder` reads a `Token`, inspects whether it's
`LParenPipe` / `Tilde` / `LBracketPipe`, and dispatches into
the right typed method. **No sentinel newtypes, no
`#[serde(rename = …)]`, no `reject_sentinel_in_nota` gating.**
The dispatch becomes a `match` on a closed `Token` enum —
exactly what perfect specificity wants.

### 5.2 The QueryParser is no longer a carve-out

It becomes one method (`Decoder::node_query`) on the same
`Decoder` type that holds `Decoder::node`. The split between
"asserts go through serde, queries go through hand-written
code" disappears — they are sibling methods on the same noun.
This is the Hoare "make it so simple there are obviously no
deficiencies" win.

### 5.3 Methods-on-types throughout

`Decoder::nexus(text).into_request()` replaces
`from_str_nexus::<Request>(text)`. Every verb is a method on
`Decoder` or `Encoder`. No free functions, no carve-out.

### 5.4 Schema-as-data alignment

When rsc lands ([criome/ARCHITECTURE.md §7 self-host loop](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)),
the generation target is one `decode_<kind>` and one
`encode_<kind>` method per `KindDecl`, projected straight from
the field list. With serde, rsc has to emit struct + derive +
serde rename attributes + the surrounding visitor protocol
contract — a much wider projection surface to maintain.

### 5.5 Typed errors, no `Error::Custom(String)`

Today every `de.rs` error is an `Error::Custom(format!(...))`
because serde's `de::Error` trait demands `Error: serde::de::Error`,
which forces a `custom(impl Display)` constructor. The
structured information vanishes into a string. Our own
framework can carry
`Error::ExpectedToken { expected: Token, got: Token, position: ByteOffset }`
natively.

### 5.6 No format-version coupling to serde 1.x

Serde 2.0 has been discussed for years. We don't want a
wire-format-affecting upstream we don't drive.

### 5.7 Decoupling the wire from the text contract

Today every signal type derives both `rkyv::Archive` and
`serde::Serialize/Deserialize`. The serde derive bloats compile
time and binary size for no benefit at the wire boundary
(which is rkyv). After replacement: signal types derive only
rkyv; the text-encode/decode methods live on `Decoder` /
`Encoder` in the nexus-side text crate.

---

## 6 · The staged plan

### Stage 1 — now, through M0

**Keep serde unchanged.** The 089 plan has ~7 kinds; the
existing serde derives + nota-serde-core round-trip them.
Don't fork mid-flight.

But: **stop adding sentinel wrappers.** The current six
(Bind / Mutate / Negate / Validate / Subscribe / AtomicBatch)
are the floor, not the floor of an active expansion. New
delimiters / sigils that show up during M0 should be flagged
as "Stage 2 work," not added to the sentinel pile.

### Stage 2 — M0 → M1 boundary

Write the `Decoder` and `Encoder` in a new crate (working
name `nexus-codec`, owned by the nexus daemon). Reuses
[`nota-serde-core::Lexer`](../repos/nota-serde-core/src/lexer.rs)
only.

**Implementation choice (decide at Stage 2):** hand-written
per-kind methods (Option 1) or our own
`#[derive(NexusKind)]` proc-macro (Option 2) — see §4.1. Both
align with the rest of the plan.

Steps:

1. New crate `nexus-codec` with `Decoder` and `Encoder` types
   and one method per nexus verb / kind.
2. Port the seven M0 kinds: `decode_node` / `encode_node` etc.
3. Fold `QueryParser`'s functionality in as
   `Decoder::node_query` / `Decoder::edge_query` etc.
4. Delete `nexus-serde` and the sentinel-newtype machinery.
5. Drop the serde derive from `signal` types — they now derive
   only `rkyv`.
6. `nota-serde-core` becomes `nota-lexer` — a 525-LoC
   tokenizer crate.

Work estimate: ~2 days of focused work for Option 1; ~3 days
for Option 2 (the proc-macro crate is the extra cost).
~hundreds of LoC written, ~thousands deleted. No behaviour
changes; tests stay green throughout.

### Stage 3 — rsc lands (M2+)

rsc projects KindDecl → Rust struct + the matching
`Decoder::<kind>` / `Encoder::<kind>` methods. The methods are
mechanical; the projection is small. Adding a kind = asserting
a KindDecl + recompiling — the existing self-host loop, with
one less projection target (no serde derive emission to
maintain).

---

## 7 · Implications for reports/095 style fix-up

### 7.1 Q1 is now a directive, not a discussion

Update [reports/095 §4a](095-style-audit-2026-04-27.md) Q1 to
reflect the staged plan. The carve-out for `from_str` /
`to_string` becomes "tolerated through M0; gone in Stage 2."

### 7.2 Q3 (rename de.rs / ser.rs) — DEFER

Renaming files that will be deleted in Stage 2 is throwaway
work. **Skip Q3 entirely.** The code in `nota-serde-core/src/{de,ser}.rs`
keeps its current names through M0; the files (and most of
the code in them) cease to exist at Stage 2.

The local-variable / struct-field / visitor-parameter renames
inside those files (the ~50 sites of `let mut de = …` /
`SeqSerializer { ser: … }` / `(self, v: V)`) are also throwaway.
Skip those too.

**Net effect on the 095 fix-up plan**: ~150 line touches dropped.
The remaining work is Q2 (`…Op` → `…Operation` rename across
signal + call sites) + Q4 (Slot/Revision/wrapper privacy + From
traits) + R12 (move inline tests to `tests/`).

### 7.3 The `nota-serde-core` style audit — DEFER

The crate gets stripped down to just the lexer at Stage 2. Doing
a style audit on de.rs/ser.rs now is polishing soon-deleted
code. Stage 2 will deliver clean code by construction.

### 7.4 Q2 (`…Op` → `…Operation`) — STILL APPLIES

These types (AssertOp / MutateOp / etc.) live in `signal`,
which survives Stage 2 (just stops deriving serde). The
rename is on long-lived types and remains in scope.

### 7.5 Q4 (`Slot`/`Revision`/wrapper privacy + From traits) — STILL APPLIES

Same reasoning. These types are in signal/sema; they're
long-lived.

### 7.6 R2 in nexus-serde (six sentinel wrappers) — DEFER

These six wrappers will be **deleted entirely** at Stage 2.
Don't fix their pub-field violation now. They are doomed code.

### 7.7 R12 inline tests — STILL APPLIES

`signal/src/frame.rs` and `sema/src/lib.rs` test-relocations
still apply (those crates survive Stage 2 with no serde
involvement).

---

## 8 · Open questions for the Stage 2 boundary

These are decisions the Stage 2 work surfaces; not blockers
for committing to the staged plan.

1. **Crate name.** `nexus-codec` is one option; `nexus-text`
   would emphasise the human-text↔signal direction;
   `signal-text` would emphasise that it's the text companion
   to signal. Decide at Stage 2.

2. **Rename `nota-serde-core` → `nota-lexer`.** Likely yes;
   confirm at Stage 2.

3. **Keep `serde::Serialize`/`Deserialize` derive on `signal`
   types as an *external* convenience?** If we ever want
   debugging tools / Python bindings / ad-hoc JSON dumps for
   diagnostic purposes, `#[derive(Serialize, Deserialize)]`
   is one line per type. Don't pay for it now; revisit if a
   real consumer appears.

4. **`Decoder` / `Encoder` actor wrap.** At criome / ractor-
   integration time, wrap the `Decoder` and `Encoder` as
   actor-state-owning components if their lifecycle warrants
   it; otherwise they stay as plain types. Decide at the
   ractor-integration boundary.

5. **Borrowed deserialization / zero-copy.** Serde handles
   this via complex lifetime gymnastics (`'de` lifetime).
   Our `Decoder` can either borrow from the input or own
   `String`s. Our M0 use case (small request texts, parsed
   eagerly) doesn't need zero-copy. Decide at Stage 2 based
   on actual measurements.

---

## 9 · Why this is the right time to commit

The codebase has **no production callers** of the serde path
yet. Nexus daemon has not been written. The cost of changing
direction post-M0 is the cost of Stage 2 above (~hundreds of
LoC, bounded). The cost of changing direction post-M2 is the
cost of unwiring rsc's serde-derive emission, which is much
higher.

The earlier "keep serde" answer was wrong because it weighed
"continuing convention" against "real architectural fit" and
chose convention. Per [beauty.md](../repos/tools-documentation/programming/beauty.md):
the discomfort with carve-outs (six and growing) is the
diagnostic reading. The right structure — one `Decoder` type
with one method per nexus verb — collapses every special case
into the normal case. **That structure is the one we were
missing.**

---

## 10 · Citations

### Serde and alternatives

- [Serde — Overview](https://serde.rs/)
- [Serde — Data Model](https://serde.rs/data-model.html)
- [Serde — Enum Representations](https://serde.rs/enum-representations.html)
- [Serde — Deserializer Lifetimes](https://serde.rs/lifetimes.html)
- [Deserialize trait docs](https://docs.rs/serde/latest/serde/trait.Deserialize.html)
- [Forward-compatible enum deserialization (issue #1388)](https://github.com/serde-rs/serde/issues/1388)
- [miniserde docs](https://docs.rs/miniserde) — non-recursive, trait-object-based
- [nanoserde — zero-deps alternative](https://github.com/not-fl3/nanoserde)
- [serde_lite docs](https://docs.rs/serde-lite/latest/serde_lite/)

### Performance / size

- [Avoiding Serde in Rust WebAssembly When Performance Matters — Wenhe Li](https://medium.com/@wl1508/avoiding-using-serde-and-deserde-in-rust-webassembly-c1e4640970ca)
- [rust_serialization_benchmark — djkoloski](https://github.com/djkoloski/rust_serialization_benchmark)

### Schema-as-data precedent

- [Datomic + EDN reference](https://docs.datomic.com/reference/edn.html)
- [edn-format spec — tagged literals + extensible reader](https://github.com/edn-format/edn)

### rkyv (the wire format that stays)

- [rkyv — Zero-copy deserialization](https://rkyv.org/zero-copy-deserialization.html)
- [Manish Goregaokar — Not a Yoking Matter (Zero-Copy #1)](https://manishearth.github.io/blog/2022/08/03/zero-copy-1-not-a-yoking-matter/)

### Project files (load-bearing)

- [`../repos/nota-serde-core/src/de.rs`](../repos/nota-serde-core/src/de.rs) — 651 LoC; ~70% serde-trait scaffolding
- [`../repos/nota-serde-core/src/ser.rs`](../repos/nota-serde-core/src/ser.rs) — 580 LoC; same ratio
- [`../repos/nota-serde-core/src/lexer.rs`](../repos/nota-serde-core/src/lexer.rs) — 525 LoC; pure format logic, the only piece nexus actually imports today
- [`../repos/nexus-serde/src/lib.rs`](../repos/nexus-serde/src/lib.rs) — six sentinel wrappers + façade; 94 LoC, all of it would be deleted
- [`../repos/nexus/src/parse.rs`](../repos/nexus/src/parse.rs) — 240 LoC hand-written QueryParser; would become methods on `Decoder` in the new crate
- [`../repos/signal/src/flow.rs`](../repos/signal/src/flow.rs) — every type currently derives both rkyv and serde; serde derives drop after Stage 2

---

*End 098.*
