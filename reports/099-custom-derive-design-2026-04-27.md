# 099 — Custom-derive design: replacing serde for nota and nexus

*Per Li 2026-04-27: "do a deep exploration of 2" + "always
pick elegance, correctness, and beauty — if in doubt, rethink
the system until it is elegant, and without 'special cases'
and 'hacks'." This report is the result of that pass; the
prior version's open-question section is gone because the
elegance test resolved each question.*

The full staged plan + reasons-to-replace-serde live in
[reports/098](098-serde-replacement-decision-2026-04-27.md).
This report assumes that decision and lays out the elegant
implementation.

---

## 1 · Scope — nota *and* nexus, at the kernel

The replacement is not "nexus-only." Today the shared kernel
[`nota-serde-core`](../repos/nota-serde-core/src/lib.rs)
hosts the lexer + ser/de impls for **both** dialects (gated
by a `Dialect::Nota` / `Dialect::Nexus` knob in the lexer);
[`nota-serde`](../repos/nota-serde/src/lib.rs) and
[`nexus-serde`](../repos/nexus-serde/src/lib.rs) are
thin façades. **The replacement collapses the whole stack:**
serde leaves the dependency graph; both façades are deleted;
the kernel becomes a single dialect-aware codec.

After the replacement, the file-grammar layer is **two
crates**:

| Crate | Role |
|---|---|
| `nota-codec` | runtime — Lexer + Decoder + Encoder + traits + blanket impls. Speaks both dialects via the existing `Dialect` knob. |
| `nota-derive` | proc-macros — re-exported through `nota-codec` so users only depend on `nota-codec`. |

Crates that go away: `nota-serde-core`, `nota-serde`,
`nexus-serde`. The hand-written QueryParser at
[`nexus/src/parse.rs`](../repos/nexus/src/parse.rs) goes
away too — its job is the `NexusPattern` derive's job.

---

## 2 · Naming — foundation prefix carries semantic weight

`Nota` prefix on universal pieces (work in both dialects).
`Nexus` prefix on extensions that need nexus-only features.
Reading the derive name tells you which dialect you're
committing to.

| Symbol | Dialect | Meaning |
|---|---|---|
| `nota-codec` (crate) | both | the runtime |
| `nota-derive` (crate) | both | the proc-macros |
| `NotaEncode` (trait) | both | encode any value to nota or nexus text |
| `NotaDecode` (trait) | both | decode any value from nota or nexus text |
| `#[derive(NotaRecord)]` | both | `(Foo a b c)` form |
| `#[derive(NotaEnum)]` | both | unit-variant enum dispatched on PascalCase identifier |
| `#[derive(NotaTransparent)]` | both | newtype-of-primitive — emits the inner value bare (`Slot(42)` → `42`) |
| `#[derive(NexusPattern)]` | nexus-only | `(\| Foo a b c \|)` form with `PatternField<T>` semantics |
| `#[derive(NexusVerb)]` | nexus-only | closed-kind enum dispatched on head identifier |

A type that uses only `Nota*` derives round-trips in either
dialect; a type with `NexusPattern` or `NexusVerb` is
implicitly nexus-only and the encoder errors when asked to
emit it in nota mode.

---

## 3 · The trait surface

```rust
pub trait NotaEncode {
    fn encode(&self, encoder: &mut Encoder) -> Result<()>;
}

pub trait NotaDecode: Sized {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self>;
}
```

One `Result` type (`nota_codec::Result<T> = std::result::Result<T, Error>`)
across the whole crate. No associated error types — the cost
of generality outweighs the benefit when every error is a
nota-text-format error.

Blanket impls for the standard primitives + containers live
in `nota-codec`:

- `u64`, `i64`, `f64`, `bool`, `String`, `&str` (encode-only)
- `Vec<T> where T: NotaEncode + NotaDecode`
- `Option<T> where T: NotaEncode + NotaDecode` — see §6

---

## 4 · The derives — what each one emits

### 4.1 `NotaRecord`

```rust
#[derive(NotaRecord)]
pub struct Edge {
    pub from: Slot,
    pub to: Slot,
    pub kind: RelationKind,
}

// Generated:
impl ::nota_codec::NotaEncode for Edge {
    fn encode(&self, encoder: &mut ::nota_codec::Encoder) -> ::nota_codec::Result<()> {
        encoder.start_record("Edge")?;
        self.from.encode(encoder)?;
        self.to.encode(encoder)?;
        self.kind.encode(encoder)?;
        encoder.end_record()
    }
}

impl ::nota_codec::NotaDecode for Edge {
    fn decode(decoder: &mut ::nota_codec::Decoder<'_>) -> ::nota_codec::Result<Self> {
        decoder.expect_record_head("Edge")?;
        let from = <Slot as ::nota_codec::NotaDecode>::decode(decoder)?;
        let to = <Slot as ::nota_codec::NotaDecode>::decode(decoder)?;
        let kind = <RelationKind as ::nota_codec::NotaDecode>::decode(decoder)?;
        decoder.expect_record_end()?;
        Ok(Self { from, to, kind })
    }
}
```

Empty records (`pub struct Ok {}`) emit `(Ok)`; the derive
handles the empty-fields case.

### 4.2 `NotaEnum`

```rust
#[derive(NotaEnum)]
pub enum Cardinality { One, Many, Optional }

// Generated:
impl ::nota_codec::NotaEncode for Cardinality { /* match → write_pascal_identifier */ }
impl ::nota_codec::NotaDecode for Cardinality { /* read_pascal_identifier → match */ }
```

This subsumes the existing hand-written
`RelationKind::from_variant_name` and `::variant_name`
methods in [signal/src/flow.rs:96–126](../repos/signal/src/flow.rs#L96)
— they fold into the derive emission and the methods can
delete.

### 4.3 `NotaTransparent`

```rust
#[derive(NotaTransparent)]
pub struct Slot(u64);

// Generated:
impl ::nota_codec::NotaEncode for Slot {
    fn encode(&self, encoder: &mut ::nota_codec::Encoder) -> ::nota_codec::Result<()> {
        self.0.encode(encoder)  // emits "42" not "(Slot 42)"
    }
}

impl ::nota_codec::NotaDecode for Slot {
    fn decode(decoder: &mut ::nota_codec::Decoder<'_>) -> ::nota_codec::Result<Self> {
        Ok(Self(<u64 as ::nota_codec::NotaDecode>::decode(decoder)?))
    }
}

// Plus, for ergonomics, the standard newtype trait pair:
impl From<u64> for Slot { fn from(v: u64) -> Self { Self(v) } }
impl From<Slot> for u64 { fn from(s: Slot) -> u64 { s.0 } }
```

This replaces the current `#[serde(transparent)]` attribute
hack and the `pub` field in `Slot(pub u64)`. The wrapped
field becomes private; access goes through the auto-emitted
`From` traits. Same target on `Revision`, `Hash`, `BlsG1`.

### 4.4 `NexusPattern`

```rust
#[derive(NexusPattern)]
#[nota(queries = "Edge")]
pub struct EdgeQuery {
    pub from: PatternField<Slot>,
    pub to: PatternField<Slot>,
    pub kind: PatternField<RelationKind>,
}

// Generated:
impl ::nota_codec::NotaEncode for EdgeQuery {
    fn encode(&self, encoder: &mut ::nota_codec::Encoder) -> ::nota_codec::Result<()> {
        encoder.start_pattern_record("Edge")?;
        encoder.encode_pattern_field(&self.from, "from")?;
        encoder.encode_pattern_field(&self.to, "to")?;
        encoder.encode_pattern_field(&self.kind, "kind")?;
        encoder.end_pattern_record()
    }
}

impl ::nota_codec::NotaDecode for EdgeQuery {
    fn decode(decoder: &mut ::nota_codec::Decoder<'_>) -> ::nota_codec::Result<Self> {
        decoder.expect_pattern_record_head("Edge")?;
        let from = decoder.decode_pattern_field::<Slot>("from")?;
        let to = decoder.decode_pattern_field::<Slot>("to")?;
        let kind = decoder.decode_pattern_field::<RelationKind>("kind")?;
        decoder.expect_pattern_record_end()?;
        Ok(Self { from, to, kind })
    }
}
```

Three load-bearing properties:

1. The wire-form record name comes from the explicit
   `#[nota(queries = "Edge")]` attribute, **not** from
   stripping `Query` off the type name. Explicit > convention:
   the relationship between `EdgeQuery` and `Edge` is a real
   schema fact, and making it textual avoids any "what if a
   query type doesn't end in Query" edge case.
2. `decode_pattern_field` carries the schema field name. When
   the input contains `@from`, the Decoder validates that the
   bind name `"from"` matches the schema field name passed at
   this position — same check as today's
   [`nexus/src/parse.rs::check_bind_name`](../repos/nexus/src/parse.rs#L231),
   automated by derive emission. Mismatched bind names produce
   typed `Error::WrongBindName { expected, got }`.
3. `PatternField<T>` requires `T: NotaEncode + NotaDecode`.
   The derive doesn't know this — it's a trait bound on
   `decode_pattern_field` itself.

### 4.5 `NexusVerb`

```rust
#[derive(NexusVerb)]
pub enum AssertOperation {
    Node(Node),
    Edge(Edge),
    Graph(Graph),
    KindDecl(KindDecl),
}

// Generated:
impl ::nota_codec::NotaEncode for AssertOperation {
    fn encode(&self, encoder: &mut ::nota_codec::Encoder) -> ::nota_codec::Result<()> {
        match self {
            Self::Node(value)     => value.encode(encoder),
            Self::Edge(value)     => value.encode(encoder),
            Self::Graph(value)    => value.encode(encoder),
            Self::KindDecl(value) => value.encode(encoder),
        }
    }
}

impl ::nota_codec::NotaDecode for AssertOperation {
    fn decode(decoder: &mut ::nota_codec::Decoder<'_>) -> ::nota_codec::Result<Self> {
        let head = decoder.peek_record_head()?;
        match head.as_str() {
            "Node"     => Ok(Self::Node(Node::decode(decoder)?)),
            "Edge"     => Ok(Self::Edge(Edge::decode(decoder)?)),
            "Graph"    => Ok(Self::Graph(Graph::decode(decoder)?)),
            "KindDecl" => Ok(Self::KindDecl(KindDecl::decode(decoder)?)),
            other => Err(Error::UnknownKindForVerb {
                verb: "Assert",
                got: other.to_string(),
            }),
        }
    }
}
```

The string match is bounded by the closed enum's variant set
— adding a kind = adding a variant + recompiling. Exactly
the [Invariant D §"closed enums at the wire"](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)
discipline made mechanical.

For struct-variant verbs (`MutateOperation`), the derive emits
positional encode/decode of the struct fields. The field names
in the source are recovered by position in the decode binding;
they don't appear in the wire form.

---

## 5 · The runtime — `Decoder` and `Encoder`

```rust
pub struct Decoder<'input> {
    lexer: Lexer<'input>,
}

impl<'input> Decoder<'input> {
    pub fn nexus(input: &'input str) -> Self { ... }
    pub fn nota(input: &'input str) -> Self { ... }

    pub fn decode<T: NotaDecode>(&mut self) -> Result<T> {
        T::decode(self)
    }

    pub fn next_request(&mut self) -> Result<Request> {
        match self.peek_token()? {
            Token::LParen       => Ok(Request::Assert(AssertOperation::decode(self)?)),
            Token::Tilde        => { self.consume(Token::Tilde)?; Ok(Request::Mutate(MutateOperation::decode(self)?)) }
            Token::Bang         => { self.consume(Token::Bang)?;  Ok(Request::Retract(RetractOperation::decode(self)?)) }
            Token::Question     => { self.consume(Token::Question)?; Ok(Request::Validate(BatchOperation::decode(self)?)) }
            Token::Star         => { self.consume(Token::Star)?; Ok(Request::Subscribe(QueryOperation::decode(self)?)) }
            Token::LParenPipe   => Ok(Request::Query(QueryOperation::decode(self)?)),
            Token::LBracketPipe => Ok(Request::AtomicBatch(AtomicBatch::decode(self)?)),
            other => Err(Error::UnexpectedToken { expected: "request sigil or delimiter", got: other.clone() }),
        }
    }

    // Methods invoked by derived impls:
    pub fn expect_record_head(&mut self, name: &str) -> Result<()>;
    pub fn expect_record_end(&mut self) -> Result<()>;
    pub fn expect_pattern_record_head(&mut self, name: &str) -> Result<()>;
    pub fn expect_pattern_record_end(&mut self) -> Result<()>;
    pub fn peek_record_head(&mut self) -> Result<String>;
    pub fn read_pascal_identifier(&mut self) -> Result<String>;
    pub fn decode_pattern_field<T: NotaDecode>(
        &mut self,
        expected_bind_name: &str,
    ) -> Result<PatternField<T>>;
    fn peek_token(&mut self) -> Result<&Token>;
    fn consume(&mut self, expected: Token) -> Result<()>;
}

pub struct Encoder {
    output: String,
    dialect: Dialect,
}

impl Encoder {
    pub fn nexus() -> Self { ... }
    pub fn nota() -> Self { ... }
    pub fn into_string(self) -> String { self.output }

    pub fn encode<T: NotaEncode>(&mut self, value: &T) -> Result<()> {
        value.encode(self)
    }

    // Methods invoked by derived impls (mirror the Decoder side).
}
```

The runtime defines the *protocol* the derives speak. The
runtime is small and stable; derives are the volume.

---

## 6 · Wire-format choices — settled

### 6.1 `Option<T>` — trailing-omission

`Option<T>` fields appear at the end of records. None means
the field is omitted from the wire; Some means the field is
present.

```rust
pub enum MutateOperation {
    Node { slot: Slot, new: Node, expected_rev: Option<Revision> },
    ...
}
```

Wire forms:
- None: `~(Node 100 (Node "User"))` — three positions
- Some(5): `~(Node 100 (Node "User") 5)` — four positions

The derive enforces a compile-time rule: in any record
deriving `NotaRecord` or `NexusVerb`, all `Option<T>` fields
must come after all non-`Option` fields. This is good Rust
style anyway; the derive just makes it required.

The decoder peeks for record-end after each optional field
position. Trivial loop, no special parser state.

### 6.2 Struct-variant verbs — positional

`MutateOperation::Node { slot, new, expected_rev }` encodes as
`(Node 100 (Node "User") 5)` — three positions in declaration
order. The struct-variant field names are NOT in the wire
form; they're recovered by position in the decode-binding.

### 6.3 Sigils are owned by the request-level dispatcher

Verb sigils (`~`, `!`, `?`, `*`) and pattern delimiters
(`(|`, `|)`) are read by `Decoder::next_request` (§5), NOT by
individual derives. The verb-payload derive (`NexusVerb`)
just dispatches by head identifier; the sigil told the
caller which verb-payload to ask for.

Consequence: the six sentinel newtypes in
[nexus-serde](../repos/nexus-serde/src/lib.rs)
(`Bind`, `Mutate`, `Negate`, `Validate`, `Subscribe`,
`AtomicBatch`) all delete. They existed only to hang
`#[serde(rename = "@NexusBind")]` attributes for serde's
dispatch — replaced here by Token-enum dispatch in
`next_request`.

### 6.4 Errors

Single `nota_codec::Error` type with structured variants:

```rust
pub enum Error {
    UnexpectedToken { expected: &'static str, got: Token },
    ExpectedRecordHead { expected: &'static str, got: String },
    WrongBindName { expected: &'static str, got: String },
    UnknownVariant { enum_name: &'static str, got: String },
    UnknownKindForVerb { verb: &'static str, got: String },
    LexerError(LexerError),
    // …
}
```

No `Error::Custom(String)` arm. Every error carries typed
context.

---

## 7 · The proc-macro implementation

`nota-derive` crate (proc-macro = true). Five `#[proc_macro_derive]`
entry points; one file per derive plus shared utilities.

```
nota-derive/
├── Cargo.toml      # proc-macro = true; deps: syn, quote, proc-macro2
└── src/
    ├── lib.rs           # 5 entry points + dispatch
    ├── nota_record.rs
    ├── nota_enum.rs
    ├── nota_transparent.rs
    ├── nexus_pattern.rs
    ├── nexus_verb.rs
    └── shared.rs        # field/variant introspection helpers
```

`nota-codec` re-exports the derives so users depend on one
crate:

```rust
// nota-codec/src/lib.rs
pub use nota_derive::{NotaRecord, NotaEnum, NotaTransparent,
                     NexusPattern, NexusVerb};
```

Estimated sizes:

| Module | LoC |
|---|---|
| `nota-derive` (proc-macros) | ~450 |
| `nota-codec` runtime (Lexer + Decoder + Encoder + traits + blanket impls + Error) | ~1100 |
| **Total** | **~1550** |

Today's footprint that gets deleted:

| Crate / file | LoC |
|---|---|
| `nota-serde-core` (`de.rs` + `ser.rs` + `lib.rs`; lexer moves) | ~1200 |
| `nota-serde` façade | 29 |
| `nexus-serde` façade | 94 |
| `nexus/src/parse.rs` (QueryParser, replaced by `NexusPattern` derive) | 240 |
| **Total** | **~1565** |

Net: roughly even on visible LoC, with thousands of
invisible serde-derive expansion lines deleted as a bonus.
The new code is uniformly *our verb vocabulary*.

---

## 8 · Migration — single rip-and-replace, no parallel period

Currently nothing consumes the serde-derived path at runtime
— criome and the nexus daemon are stubs. So no parallel-derives
window is needed; it's a clean atomic swap.

Steps in dependency order:

1. **Create repos** `LiGoldragon/nota-codec` and
   `LiGoldragon/nota-derive`. Seed each with the canonical
   crane + fenix flake.nix per
   [rust/nix-packaging.md](../repos/tools-documentation/rust/nix-packaging.md).
2. **Implement `nota-codec`**: copy `Lexer` from
   nota-serde-core; write `Decoder` + `Encoder` + traits +
   blanket impls + `Error`. Integration tests with internal
   test-fixture types covering every derive case.
3. **Implement `nota-derive`**: five `proc_macro_derive`
   entry points. trybuild compile-fail tests for malformed
   input.
4. **Wire `nota-codec` to re-export derives** via
   `pub use nota_derive::*;`.
5. **Update `signal` Cargo.toml**: drop serde dep; add
   nota-codec dep.
6. **Update signal source**: replace
   `#[derive(Serialize, Deserialize)]` lines with the
   appropriate Nota/Nexus derive. Also rename `…Op` →
   `…Operation` (per [reports/095 §4a Q2](095-style-audit-2026-04-27.md)),
   `Slot(pub u64)` → `Slot(u64)` with `NotaTransparent` derive.
7. **Delete `nexus/src/parse.rs`**, rewrite
   `nexus`'s tests as integration tests against `nota-codec`'s
   `Decoder`.
8. **Delete `nota-serde`**, `nexus-serde`, `nota-serde-core`
   — and remove their entries from
   [`mentci/devshell.nix linkedRepos`](../devshell.nix),
   [`mentci/docs/workspace-manifest.md`](../docs/workspace-manifest.md),
   and [`mentci.code-workspace`](../mentci.code-workspace).
9. **Add `nota-codec` and `nota-derive`** to the same three
   files.
10. **Run `nix flake check`** in each touched repo. Push.

Estimated work: ~3 days end to end.

---

## 9 · What I need to start

Four things:

1. **Confirm timing: now, before M0 step 3 (criome body).**
   This was originally proposed for the M0→M1 boundary; doing
   it first means criome never depends on serde.
2. **Confirm naming.** §2's table — `nota-codec` /
   `nota-derive` / traits `NotaEncode`/`NotaDecode` / derives
   `NotaRecord` / `NotaEnum` / `NotaTransparent` /
   `NexusPattern` / `NexusVerb`.
3. **Confirm `gh repo create LiGoldragon/nota-codec --public`
   + same for `nota-derive`** is the right way to create the
   repos. (Alternative: you create them in the GitHub UI;
   I clone and seed.)
4. **Confirm rsc rename is out of scope for this work.** Per
   the side-note in the previous turn, `rsc` is up for rename
   per the full-words rule, but it's not load-bearing for
   this codec replacement. I'll leave it for a separate pass
   unless you want to fold it in.

That's all. Once those are confirmed, I begin at step 1 of §8.

---

*End 099.*
