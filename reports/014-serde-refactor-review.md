# 014 — post-refactor review: nota-serde-core + nexus-serde

*Claude Opus 4.7 / 2026-04-24 · follow-up to [013](013-nexus-syntax-proposal.md).*

Deep review of the Dialect refactor ([nota-serde-core df962b37](https://github.com/LiGoldragon/nota-serde-core/commit/df962b37),
[nexus-serde 41e7c40b](https://github.com/LiGoldragon/nexus-serde/commit/41e7c40b)).
Covers what landed, what's latent, what's missing, and the decision
points that gate the next-phase work.

---

## 1 · TL;DR

**Refactor status**: complete, verified green. Both crates pass
`nix flake check`; clippy clean on nota-serde-core; one trivial
unused-imports warning in nexus-serde's test file. No regressions;
one regression *fixed* (u128 overflow handling was silently absent
in nexus-serde's fork, now restored by consolidation).

**Code shape**: nexus-serde's src/ shrank from ~1920 LoC to 74 LoC
(96% reduction). All duplication eliminated. Tier-1 delimiter
tokens (`<| |>`, `(|| ||)`, `{|| ||}`) lex correctly in nexus
mode; nota mode rejects them with a dialect-tagged error.

**What's unfinished**: the *lexer* half of Tier-1 is done but
the *Rust-type* half isn't. Pattern / Constrain / Shape
containers — and the three Tier-1 wrappers (Optional,
Subscription, Transaction) — still have no Rust representation.
Their design is the gating decision for nexusd and nexus-cli.

**Biggest recommended next action**: design the `PatternExpr`
type hierarchy in nexus-serde (or a new crate). Everything
downstream blocks on this.

---

## 2 · Verification

| Check | nota-serde-core | nexus-serde |
|---|---|---|
| `cargo build` | ✓ | ✓ |
| `cargo test` | 124 pass (91 edge + 30 nexus + 3 example) | 15 pass (nexus_wrappers + 1 doctest) |
| `cargo clippy --all-targets` | ✓ clean | 1 unused-imports warning in `char_tests` mod |
| `nix flake check` | ✓ | ✓ |
| Downstream nota-serde | unaffected (pinned to old rev) | N/A |
| Backward compat of public API | ✓ (`to_string`/`from_str` defaults to Nota) | ✓ (signatures unchanged for Bind/Mutate/Negate/to_string/from_str) |

Net test coverage **increased** for nexus functionality despite
the line-count drop: 30 new dedicated nexus-mode tests in
nota-serde-core + 15 façade-level integration tests in
nexus-serde ≈ 45 nexus-specific tests, vs. ~8 nexus-specific
cases in the pre-refactor fork (most of the fork's tests were
duplicates of nota-serde-core's).

---

## 3 · Specific findings

Ordered by severity.

### 3.1 Latent lexer gap — `<|| ||>` unreachable [medium]

The report-013 matrix reserved `<||` and `||>` for "windowed
stream" (Phase 2). The current lexer cannot produce those
tokens:

```
<||   →  LAnglePipe (from `<|`) + error on the trailing `|`
||>   →  error "unexpected `||` followed by `>`"
```

**Implication**: implementing the reserved slot later requires a
lexer patch, not just a Rust-type addition. The patch is small
(`read_left_angle` needs 2-byte lookahead for `<||`;
`read_pipe_close` needs an extra arm for `||>`) but it's a
non-obvious gotcha if a future session assumes "Tier-1 tokens
are all done."

**Proposed resolution**: either (a) land the two tokens now as
part of Phase 1 (ensures the grammar slot is reserved; ~15 LoC),
or (b) document in lexer.rs that `LAngleDouble`/`RAngleDouble`
are intentionally deferred with a pointer to this finding.

**Cost of (a)**: trivial. **Recommendation**: do (a); grammar
completeness favours landing all matrix slots together.

### 3.2 Clippy warning in nexus-serde tests [trivial]

```
tests/nexus_wrappers.rs:161:17
use serde::{Deserialize, Serialize};  // unused inside char_tests mod
```

Safe cleanup; 1-line fix. Not blocking.

### 3.3 Sentinel-dispatch edge cases — undocumented [low]

The sentinel pattern (`@NexusMutate`, `@NexusNegate`) works for
any inner `T: Serialize`, including semantically-dubious cases:

```rust
Mutate(None)          → "~None"         // retracting an absent Option?
Mutate(42)            → "~42"           // mutating a primitive?
Mutate(vec![1,2,3])   → "~<1 2 3>"      // mutating a sequence?
```

Structurally these all round-trip cleanly. Semantically they're
in nexus-grammar gray zones — the grammar spec assumes `~` precedes
a record or pattern. The serde layer cannot enforce that without
trait-bounded wrappers, which would constrain consumer code.

**Recommendation**: document the behaviour in nexus-serde's
module docs (one paragraph). No code change; the permissiveness
is by design.

### 3.4 `nota-serde` pinned to pre-refactor rev [low]

`nota-serde/Cargo.lock` still pins `nota-serde-core` at
`be4741df` (pre-Dialect). It works — the default Nota dialect is
behaviourally unchanged — but doesn't inherit:
- u128 overflow fix (it already had this; pre-refactor nota-serde-core was correct)
- new `to_string_nexus` / `from_str_nexus` fns (irrelevant; nota-serde doesn't use them)

**Recommendation**: defer the bump. No functional benefit to
nota-serde; it's just churn. Bump when the next real change to
nota-serde-core lands.

### 3.5 `Error::Custom(String)` dominates the error surface [low]

Most lexer and dispatch failures use `Error::Custom(String)`
rather than structured variants. The thiserror idiom recommends
per-case variants (e.g., `DialectMismatch { sentinel, dialect }`,
`UnexpectedToken { expected, got, offset }`) so callers can
pattern-match. Currently they'd have to substring-match error
strings — brittle.

**Recommendation**: land structured variants when a consumer
(nexusd error-message rendering, nexus-cli help text) needs them.
YAGNI now.

### 3.6 `=` token produced in Nota mode [negligible]

The lexer emits `Token::Equals` from `=` in both dialects. Nota
grammar doesn't use `=`. A stray `=` in a nota document
tokenises but fails at parse time as "unexpected Equals." The
failure mode is correct; only the *location* of the rejection
is suboptimal.

**Recommendation**: no change. Not worth the special-case.

### 3.7 Downstream style-violation in nexusd / nexus-cli [medium, not this crate]

`nexusd/Cargo.toml` and `nexus-cli/Cargo.toml` both depend on
`anyhow = "1"` and their `main()` returns `anyhow::Result<()>`.
[rust/style.md §Errors](../repos/tools-documentation/rust/style.md#errors-typed-enum-per-crate-via-thiserror)
explicitly forbids `anyhow`/`eyre` ("they erase error types at
boundaries").

**Recommendation**: fix when M4 (nexusd) work starts. Not
blocking the serde refactor but worth recording.

---

## 4 · Architectural review

### 4.1 The Dialect model — runtime enum vs type parameter

The refactor uses `Dialect::{Nota, Nexus}` as a runtime enum
field on Lexer/Serializer/Deserializer. Alternative: make
Dialect a zero-sized type parameter (trait-based, compile-time
specialisation).

**Runtime enum (chosen)**:
- Single code path; one binary has both dialects available.
- Users can parse nexus and fall back to nota dynamically.
- Every character-dispatch branch has a dialect check (predictable,
  branch-predictor friendly).
- The match statements stay flat and readable.

**Type parameter**:
- Two codegen paths; zero-cost abstraction.
- Compile-time guarantee: `Serializer<Nota>` can't serialise
  `Bind` (the sentinel would never dispatch).
- Twice the monomorphisation work; compile time increases.
- Generic bounds make the API more cluttered.

**Assessment**: the user's stated preference is elegance over
small performance gains. Runtime enum gives a flatter API and
trivially supports "parse as nexus, fall back to nota" if that
use case arises. Keep the runtime enum. The zero-cost type-param
alternative would only pay for itself if serialisation were in a
hot loop — and even then the enum dispatch is one branch per
char, noise-level.

### 4.2 Sentinel-name dispatch

The `@NexusBind` / `@NexusMutate` / `@NexusNegate` pattern uses
serde's `#[serde(rename = "...")]` to smuggle semantic intent
through a newtype-struct name. The serializer/deserializer
matches on the name and emits/consumes the nexus sigil.

**Strengths**:
- Pure library code; no custom derive needed.
- Wrapper types compose (`Mutate<Bind>` → `~@h` works).
- Sentinel names have the `@Nexus…` prefix — unlikely to
  collide with real user type names.

**Weaknesses**:
- String-based dispatch; no compile-time check that the
  rename-literal matches the constant. If `BIND_SENTINEL` and
  the `#[serde(rename)]` attribute drift, the wrapper silently
  stops dispatching.
- The constants are duplicated (a `&'static str` constant + a
  `#[serde(rename = "...")]` literal that must match).

**Mitigation**: export the sentinel constants (done —
`BIND_SENTINEL` / `MUTATE_SENTINEL` / `NEGATE_SENTINEL` are pub).
A test that cross-checks the literal against the constant could
prevent drift. Low priority.

**Alternative considered**: a proc-macro crate
`nexus-serde-derive` with `#[derive(NexusWrapper)]` that generates
both the `Serialize`/`Deserialize` and the sentinel binding.
Zero chance of drift. Cost: a new crate, proc-macro machinery,
longer compile times. **Verdict**: not worth it for 3 (soon 6-7)
wrapper types. Reconsider if the sentinel set grows past a dozen.

### 4.3 Placement — nota-serde-core hosting nexus knowledge

nota-serde-core now contains:
- All nexus-specific tokens (`Tilde`, `At`, `Bang`, etc.)
- Nexus sentinel constants (`BIND_SENTINEL`, etc.)
- Nexus dispatch logic in `serialize_newtype_struct` /
  `deserialize_newtype_struct`

Philosophically this muddies "nota-serde-core is nota's kernel."
In practice it's the shared kernel for both dialects — the name
is a historical accident from when it was extracted purely for
nota-serde. **Name is fine; purpose is documented in lib.rs**.

**Alternative**: rename the crate to `sema-serde-core` or
`nota-nexus-serde-core`. Cost: a crate rename + all dep updates.
**Verdict**: defer indefinitely. The name is slightly misleading
but not confusing in practice.

---

## 5 · The Pattern-type gap

This is the **biggest strategic question** and the gating
decision for all downstream work on nexusd / nexus-cli /
Tier-1 Rust wrappers.

### 5.1 What exists

Lexer produces these nexus-specific token pairs:

| Tokens | Grammar role |
|---|---|
| `LParenPipe` / `RParenPipe` | Pattern `(\| … \|)` |
| `LBracePipe` / `RBracePipe` | Constrain `{\| … \|}` |
| `LBrace` / `RBrace` | Shape `{ … }` |
| `LParenDouble` / `RParenDouble` | Optional pattern `(\|\| … \|\|)` |
| `LBraceDouble` / `RBraceDouble` | Atomic txn `{\|\| … \|\|}` |
| `LAnglePipe` / `RAnglePipe` | Stream / subscription `<\| … \|>` |
| (reserved, not yet in lexer) | Windowed stream `<\|\| … \|\|>` and rule `[\|\| … \|\|]` |

**No Rust type** consumes these tokens. The deserializer treats
them as "unexpected token" errors when a user tries to
deserialise a pattern-shaped message into a plain struct.

### 5.2 Design options

#### (a) Per-shape newtype wrappers

```rust
#[serde(rename = "@NexusPattern")]
pub struct Pattern<T>(pub T);      // → (| T |)

#[serde(rename = "@NexusOptional")]
pub struct Optional<T>(pub T);     // → (|| T ||)

#[serde(rename = "@NexusConstrain")]
pub struct Constrain<T>(pub T);    // → {| T |} (T usually Vec)

#[serde(rename = "@NexusShape")]
pub struct Shape<T>(pub T);        // → { T }

#[serde(rename = "@NexusStream")]
pub struct Stream<T>(pub T);       // → <| T |>

#[serde(rename = "@NexusTxn")]
pub struct Transaction<T>(pub T);  // → {|| T ||}
```

**Pros**: symmetric with Bind/Mutate/Negate. Same implementation
pattern. Flat type lattice.

**Cons**:
- **Heterogeneous inner problem**: `Constrain` contains a
  sequence of *mixed* patterns — some bare `Pattern`, some
  `Negate<Pattern>`, some `Optional<Pattern>`. The inner type
  has to be either `Vec<Box<dyn ErasedPattern>>` (dyn erasure) or
  a single enum that unifies them.
- **Seq-wrap problem**: Transaction's inner is `Vec<Op>`. Naïve
  `Transaction<Vec<Op>>` serialises as `{|| <op1 op2> ||}` — the
  inner `<>` from Vec is a visual bug. Requires either a custom
  serializer (strip the `<>`) or a non-Vec inner type.
- **Shape semantics**: a shape `{ @h @v (Sum @amount) }` is a
  list of projection-ish things (binds + aggregation records).
  Not `Shape<Vec<T>>` for any meaningful T — needs an enum.

#### (b) Unified `PatternExpr` enum

```rust
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternExpr {
    /// `(| TypeName @bind @bind |)`
    Match { record: Record, binds: Vec<PatternAtom> },
    /// `(|| PatternExpr ||)` — left-join
    Optional(Box<PatternExpr>),
    /// `!PatternExpr`
    Negate(Box<PatternExpr>),
    /// `{| PatternExpr… |}`
    Constrain(Vec<PatternExpr>),
    /// `<| PatternExpr (Op…) |>` — reactive stream
    Stream { body: Box<PatternExpr>, ops: Vec<QueryOp> },
}

pub enum PatternAtom {
    Bind(Bind),
    Wildcard,
    Literal(Literal),
    Nested(PatternExpr),
}

pub enum QueryOp {
    Limit(u64),
    Offset(u64),
    OrderBy(Bind),
    // aggregation operators…
    Count,
    Sum(Bind),
    GroupBy { key: Bind, inner: Vec<QueryOp> },
    // …
}
```

**Pros**:
- One type unifies all pattern shapes.
- The `untagged` serde enum dispatches on first-token at parse
  time (exactly what the grammar is designed for).
- Explicit representation of heterogeneity. No `dyn`, no strip
  hacks.
- Consumer-friendly: nexusd can pattern-match on `PatternExpr`
  without writing custom deserializers.
- Aligns with report-013's "operators are records" model (QueryOp
  is just records serialising via normal struct/variant rules).

**Cons**:
- Bigger type graph. ~6 enum types + ~4 struct types.
- `untagged` enums in serde require explicit try-each-variant
  logic — might need custom `Deserialize` to preserve error
  quality.
- `PatternExpr` is recursive and heap-allocates (`Box`).
  Unavoidable for a recursive grammar.

#### (c) Raw token-stream API

Expose the lexer publicly; let consumers (nexusd, nexus-cli)
build their own AST from the token stream, bypassing serde.

**Pros**: maximum flexibility; consumers don't inherit our
choices.

**Cons**: Every consumer reinvents the pattern AST. Fragmentation.

### 5.3 Recommendation

**Option (b), PatternExpr enum.** Reasoning:

1. The heterogeneous-inner problem in (a) forces an enum
   *somewhere*; putting it at the top-level is cleaner than
   hiding it inside each wrapper's generic T.
2. Nexus's grammar is already first-token-decidable — the exact
   property serde's `untagged` enum needs. The alignment is
   natural, not forced.
3. Consumers want to *pattern-match* on pattern shapes (what is
   this query? a match? an optional? a stream?). A flat enum is
   the Rust-idiomatic way to model that.
4. Report-013's "operators are records" approach maps cleanly —
   `QueryOp` is a serde-derived enum of built-in operator
   records, no special handling.

The work is ~400-600 LoC of types + custom deserializer for the
`untagged` dispatch. A single session of focused work.

### 5.4 Where the type lives

Three options:

- **In nexus-serde**: keeps the "thin façade" less thin (~700
  LoC instead of 74), but consistent with "nexus-serde is the
  nexus Rust-type home."
- **In nexus-schema**: nexus-schema already holds the data-type
  Rust types (Primitive, Type, Struct, etc.). Adding PatternExpr
  is consistent with "schema holds the types."
- **New crate `nexus-pattern`**: cleanest separation but adds a
  repo.

**Recommendation**: **nexus-schema**. It already owns the type
layer; PatternExpr is a message-layer *type* that references
schema types. Rule 1 (one artifact per repo) is preserved.

---

## 6 · Phase 1 completion plan

Given the design recommendation in §5, here's the full path to
Phase 1 done ("Tier-1 syntax lands; consumers can build
queries"):

### 6.1 Immediate — closes gaps in the just-done refactor

| Task | Est. | Blocker? |
|---|---|---|
| Land `LAngleDouble` / `RAngleDouble` tokens | 20 LoC | No — grammar completeness |
| Fix nexus-serde clippy warning | 1 line | No — trivial |
| Add Mutate-around-Vec semantics doc in lib.rs | 1 paragraph | No |

### 6.2 Short-term — PatternExpr in nexus-schema

| Task | Est. | Depends on |
|---|---|---|
| Decide: PatternExpr lives in nexus-schema | — | user yes/no |
| Add nexus-schema::query module with QueryOp + operator records (Limit, Sum, GroupBy, etc.) | ~200 LoC | decision above |
| Add nexus-schema::pattern module with PatternExpr + PatternAtom + Record + Literal | ~400 LoC | — |
| Custom Deserialize for PatternExpr's untagged dispatch | ~150 LoC | above |
| Tests: round-trip every shape in report-013's examples | ~200 LoC of tests | above |

### 6.3 Medium-term — Consumers

| Task | Depends on |
|---|---|
| [mentci-next-4jd](003-mvp-implementation-plan.md) method-body layer in nexus-schema | [nexus-schema-5rw / wq3 / tu8](../repos/nexus-schema/) decisions |
| [mentci-next-8ba](003-mvp-implementation-plan.md) M3 sema redb wrapper | [sema-5d3](../repos/sema/) opus-identity decision |
| Replace `anyhow` with `thiserror` in nexusd + nexus-cli | — |
| M4 nexusd — accepts nexus messages, dispatches to sema | Patterns + sema |

### 6.4 Phase 2 — deferred

- Rule delimiter `[|| ||]` — lexer + PatternExpr variant
- Windowed stream `<|| ||>` — lexer (if not landed per §6.1) +
  PatternExpr variant
- Temporal records `TimeAt` / `TimeBetween` / `TimeAll`
- Proc-macro `#[derive(NexusWrapper)]` (if wrapper set grows)

---

## 7 · Open questions

Numbered for bd tracking if the user wants per-question issues.

**Q1 — Land `<|| ||>` tokens now?** §3.1 flags that the reserved
windowed-stream slot currently can't be tokenised. Recommendation:
land the 2 tokens now (~20 LoC). Alternative: defer, document.

**Q2 — PatternExpr design — option (a) per-wrapper, option (b)
unified enum, or option (c) token-stream API?** §5. Recommendation:
(b) unified `PatternExpr` enum.

**Q3 — PatternExpr home — nexus-serde, nexus-schema, or new
crate?** §5.4. Recommendation: nexus-schema.

**Q4 — Mutate-around-non-record semantics.** §3.3. Document as
"permissive; grammar consumers are expected to constrain at the
type level" or add a `NexusRecord` trait bound?

**Q5 — Sentinel-drift protection.** §4.2. Add a test that
ensures `BIND_SENTINEL` matches the `#[serde(rename)]` on the
`Bind` struct, or accept the risk?

**Q6 — Bump nota-serde to current nota-serde-core rev?** §3.4.
Recommendation: defer until a nota-serde-facing change lands.

**Q7 — Structured Error variants — refactor now or later?** §3.5.
Recommendation: later. YAGNI.

**Q8 — nexusd/nexus-cli anyhow cleanup timing.** §3.7. Recommendation:
as part of M4 start, not earlier.

---

## 8 · Updated mental model

Where the 10-repo ecosystem sits after this refactor:

```
┌────────────────────────────────────────────────────────────────┐
│ nota-serde-core                                                │
│  Lexer + Ser + De with Dialect::{Nota, Nexus}                  │
│  All sigil/delim tokens including Tier-1 reside here           │
│  Sentinel-dispatch for @NexusBind / @NexusMutate / @NexusNegate│
└────────────┬──────────────────────────┬────────────────────────┘
             │                          │
      Dialect::Nota               Dialect::Nexus
             │                          │
             ▼                          ▼
     ┌─────────────┐            ┌─────────────────────┐
     │ nota-serde  │            │ nexus-serde         │
     │ (façade)    │            │ (façade) + 3 wrappers│
     └─────────────┘            │ Bind/Mutate/Negate   │
                                └──────────┬───────────┘
                                           │
                                  ┌────────▼────────┐
                                  │ nexus-schema    │
                                  │ Data types      │
                                  │ (+ PatternExpr  │
                                  │    once landed) │
                                  └────────┬────────┘
                                           │
                     ┌─────────────────────┼─────────────────────┐
                     ▼                     ▼                     ▼
                  sema                 nexusd              nexus-cli
               (DB backend)         (ractor daemon)      (thin client)
                     ▲                     │
                     └──────── rsc ────────┘
                       (records → .rs source)
```

The refactor moved the axis of duplication (nota-serde vs
nexus-serde's forked machinery) into the axis of *variation*
(Dialect enum in one crate). What used to be two parallel
codebases is now one codebase with a two-valued knob.

**Next axis to collapse**: the PatternExpr gap — §5. Then the
M2-remainder + M3 milestones open up, and Phase-0 MVP
(self-hosting) becomes the only remaining blocker to the
solstice-date target.

---

*End of report 014. The refactor landed clean. The next session
either picks up the Tier-1 token completion (§6.1, cheap) or
dives into PatternExpr design (§5, the real work).*
