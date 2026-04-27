# 093 — Project-wide style review plan

*Per Li 2026-04-27 — prepare for a project-wide review of all
existing Rust code against two style rules: full-words naming
(AGENTS.md + rust/style.md) and methods-on-types over free
functions (rust/style.md, with the deeper "affordances, not
operations" rationale just landed). Free functions let the
agent skip creating types that should own behavior; this
review surfaces those missing types and creates them.*

---

## 1 · Why a review now

Two rules just hardened:

1. **Naming — full words by default** ([092](092-naming-research-and-rule.md))
   — landed in [AGENTS.md](../AGENTS.md) and
   [rust/style.md](../repos/tools-documentation/rust/style.md).
2. **Methods on types, not free functions** — already in
   rust/style.md, now extended with the *affordances vs
   operations* rationale.

Both rules apply globally. Existing code was written before
these rules were enforced. The question isn't whether to
review — it's how to scope and sequence it.

The deeper claim driving the review: **free functions absorb
behavior that should belong to typed entities, and missing
types compound over time**. A codebase that lets free
functions accumulate develops gaps in its model — verbs
without owning nouns, validators without input types,
parsers without parser types. The review surfaces those gaps
and fills them by promoting verbs into methods on the
nouns that should own them.

---

## 2 · Scope

Engine-relevant Rust crates only:

```
   crate              LoC    state                review priority
   ─────────────────────────────────────────────────────────────
   nota-serde-core    1808   real, mature         HIGH
   signal              932   real, recently       MEDIUM
                             rewritten — likely
                             clean but verify
   nexus-serde          93   façade               LOW
   nota-serde           29   façade               LOW
   nexus               ~250  parse just landed,   HIGH (verify)
                             body to come
   sema                ~200  just landed          MEDIUM (verify)
   criome             ~150  skeleton              LOW (most is
                             todo!())
   nexus-cli            21  stub                 LOW
   lojix-schema        112  typed stubs          LOW
```

Total: ~3600 LoC across 9 crates. Realistic review time:
1–2 hours per HIGH-priority crate, 30 min per MEDIUM,
10 min per LOW.

Skipped: lojix, lojix-cli, lojix-store, rsc, horizon-rs,
CriomOS family — all M2+ / out-of-MVP-scope.

---

## 3 · The two axes

### Axis A — Full-words naming

Per [AGENTS.md §Naming](../AGENTS.md) and the [bad → good
table](../AGENTS.md). For each file, grep for:

```
\b(lex|tok|ident|kd|pf|de|op|ctx|cfg|addr|buf|tmp|arr|obj|
   proc|calc|init|repr|gen|ser|deser|args|params|vars|
   sock|conn|req|resp|res|ret|val|var|num|str|len|idx|
   pos|cnt|cur|prev|next|fn)\b
```

Filter false positives (file names like `de.rs`, socket
paths like `/tmp/sock`, verb fragments inside longer words
like `deserialize`). Spell each remaining offender out per
the rule.

### Axis B — Methods on types, not free functions

For each `.rs` file, find every `pub fn` (and substantive
`fn`) that is NOT inside an `impl` block. Each is a candidate.

For each candidate, ask:

1. **What noun owns this verb?** If a clear answer exists,
   move the function into that type's `impl` block.
2. **If no noun exists yet, what type SHOULD exist?**
   Create it. The verb becomes a method on the new type.
3. **Is this a genuinely local helper inside one module?**
   Per the style guide carve-out, small private helpers
   may stay free (e.g. `is_pascal_case(&str)` — a one-line
   string predicate). The bar is "small + private + genuinely
   local + obviously not reusable."

Examples of good promotions:

```rust
// Before — verb floating
pub fn parse_query(text: &str) -> Result<QueryOp> { … }

// After — verb owned by the noun that should exist
pub struct QueryParser<'input> { lexer: Lexer<'input> }
impl<'input> QueryParser<'input> {
    pub fn new(input: &'input str) -> Self { … }
    pub fn into_query(self) -> Result<QueryOp> { … }
}
```

```rust
// Before — verb on a primitive type
pub fn relation_kind_from_variant_name(name: &str) -> Result<RelationKind> { … }

// After — verb owned by the type it constructs
impl FromStr for RelationKind {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> { … }
}
```

(Or `TryFrom<&str>` if that fits the calling pattern better.)

---

## 4 · Per-crate plan

### 4.1 nota-serde-core (~1808 LoC) — HIGH priority

Likely the biggest naming + free-fn pile. The lexer/ser/de
traditions push toward terse names (`tok`, `de`, `ser`).
Audit:
- `src/lexer.rs` — `Lexer` already a type with methods. Look
  for free helpers (`is_ident_start`, `is_ident_continue`,
  `hex_digit`, `dedent`, `parse_int_literal`, `utf8_char_len`)
  that might be private-helper exempt OR might want to live
  on a `Token` / `Lexer` / `Identifier` type.
- `src/de.rs` — `Deserializer` is a type. Free `int_to_i128`,
  `int_to_u128`, `float` look like helpers; consider whether
  they're methods on `Token` (e.g., `Token::expect_i128`).
- `src/ser.rs` — same pattern. `is_bare_string_eligible`,
  `is_valid_bind_name` look like they want a `BindName`
  type.
- `src/error.rs` — small; mostly fine.

Naming: identifiers like `tok`, `de` may appear; verify.
Method-vs-free: probably 5–10 functions worth promoting,
plus 2–3 missing types to introduce (`BindName`, maybe
`Identifier`).

**Estimated effort**: 2 hours.

### 4.2 signal (~932 LoC) — MEDIUM priority

Recently rewritten this turn with full-words names and a
typed enum-heavy design. Verify:
- Quick grep for cryptic identifiers — should turn up
  little.
- Free functions: `Frame::encode` and `Frame::decode` are
  methods. Other frame helpers are tests-only.
- The big enums (`Request`, `Reply`, `AssertOp`, `MutateOp`,
  `QueryOp`) are types with no behavior — pure data. That's
  appropriate; they're the IR.

**Estimated effort**: 30 min for verification + minor
fixes.

### 4.3 nexus-serde, nota-serde — LOW

Pure façades over nota-serde-core. The wrapper types
(`Bind`, `Mutate`, `Negate`, `Validate`, `Subscribe`,
`AtomicBatch`) are tuple structs with `serde` attributes —
no behavior to promote. Just verify naming.

**Estimated effort**: 10 min each.

### 4.4 nexus (~250 LoC) — HIGH priority (verify)

Just landed `QueryParser` per the methods-on-types rule.
Verify:
- `src/parse.rs` — has private free helpers
  (`relation_kind_from_variant_name`, `check_bind_name`,
  `is_pascal_case`, `is_lowercase_identifier`). Each
  candidate for promotion:
  - `relation_kind_from_variant_name` → `impl FromStr for
    RelationKind` in signal (cross-crate move; needs
    coordination).
  - `check_bind_name` → could stay free as a small private
    validation helper, OR become a method on `QueryParser`.
  - `is_pascal_case` / `is_lowercase_identifier` — small
    string predicates; probably fine as free, but consider
    whether an `IdentifierClass` type would be load-bearing
    (it would also live in nota-serde-core; cross-crate).
- `src/error.rs` — pure data enum.
- `src/lib.rs` — re-exports only.

**Estimated effort**: 1 hour. The cross-crate moves
(`RelationKind::FromStr` in signal, `IdentifierClass`
maybe in nota-serde-core) make this non-trivial.

### 4.5 sema (~200 LoC) — MEDIUM priority (verify)

Just landed. The `Sema` struct owns the redb DB and
exposes `open / store / get` as methods. Tests use a
`TempSema` fixture struct. Verify:
- No free public functions outside `main`-style.
- Constants `RECORDS`, `META`, `NEXT_SLOT_KEY`,
  `SEED_RANGE_END` are module-level `const` — fine
  (constants, not functions).
- The `Slot` newtype is a transparent wrapper.

**Estimated effort**: 15 min.

### 4.6 criome (~150 LoC, mostly stubs) — LOW

The validator pipeline modules (`schema.rs`, `refs.rs`,
etc.) currently have free `pub fn check()` and
`pub fn apply()` stubs. These are what the M0 step-3
implementation will rewrite. Re-do them per the rule —
each validator stage is a method on a `Validator` (or
per-stage `SchemaCheck`, `RefCheck`, etc.) type.

**Estimated effort**: handled by M0 step 3 implementation,
not a separate review pass.

### 4.7 nexus-cli (~21 LoC stub) — LOW

Just `main` + an Error enum. No promotion to do until
implementation lands.

**Estimated effort**: 5 min.

### 4.8 lojix-schema (~112 LoC typed stubs) — LOW

Verb / Spec / Outcome enums with `serde` + `rkyv`
derives. Pure data. Verify naming; nothing to promote
since there's no behavior yet.

**Estimated effort**: 10 min.

---

## 5 · Order

```
  Stage 1 — verify the recently-landed code (low risk,
  ────────────────────────────────────────────────────
  catches my own slips this session before they ossify):

    1. nexus            (~1 hour)   ← biggest verification
                                       target; new code
    2. signal           (~30 min)
    3. sema             (~15 min)

  Stage 2 — clean up the accumulated pile:

    4. nota-serde-core  (~2 hours)  ← largest target;
                                       deepest dialect inheritance
    5. nexus-serde      (~10 min)
    6. nota-serde       (~10 min)
    7. lojix-schema     (~10 min)
    8. nexus-cli        (~5 min)

  Stage 3 — rolled into M0 step 3:

    9. criome           (no separate pass; the M0
                         implementation lands per the
                         rules from day one)
```

Total: ~4–5 hours of focused work for stages 1+2.
Stage 3 is part of the M0 implementation effort.

---

## 6 · Per-promotion mechanics

For each free function being promoted to a method, the
template:

1. **Identify the noun.** What type owns this verb?
2. **If the noun is in another crate**, decide:
   - implement the method in the owning crate (preferred,
     long-term), OR
   - keep a small private helper here as a temporary expedient
     and add a follow-up note for the cross-crate move.
3. **Move the function body** into the appropriate `impl`
   block, adjusting `&self` / `Self` as needed.
4. **Update call sites** to method form
   (`type.method(arg)` instead of `function(type, arg)`).
5. **Re-run tests** for the affected crate.
6. **Commit per logical promotion** — don't batch unrelated
   moves in one commit.

For each cryptic name being spelled out:

1. **Read the function once** to confirm what the name
   actually refers to.
2. **Pick a full English word** per the rule. Use the
   bad → good table as a starting point.
3. **Rename** — IDE rename or `sed -i` carefully.
4. **Re-run tests**.
5. **Commit per logical rename pass** (e.g., one commit
   per file or one commit per renamed concept).

---

## 7 · What this review does NOT do

- Not a full architectural rewrite. Type signatures and
  module structures stay; we're just promoting verbs and
  spelling out names.
- Not a redesign of the IR. `signal`'s typed enums stay as
  designed.
- Not a refactor of the validator pipeline (that's M0
  step 3).
- Not adding new functionality. Behavior-preserving only.

---

## 8 · Where to start

If the user approves this plan:

1. Stage 1.1 — nexus self-review (catches my own slips
   from the parser landing earlier this turn). Likely
   surfaces 2–4 free helpers worth promoting and 0–2
   naming issues.
2. Then stage 1.2 — signal (verify clean).
3. Then stage 1.3 — sema (verify clean).
4. Then stage 2 in size order (nota-serde-core last
   because it's the biggest).

Each stage closes with a commit per crate (or per logical
group of changes within a crate). Tests pass before each
push.

---

## 9 · Open questions

- **Cross-crate method moves** (e.g., `RelationKind::
  FromStr` in signal): batch in one go, or do per-call-site
  as I encounter them? My instinct: batch.
- **Should the review include adding doc comments** that
  were missed? Or is that a separate pass? My instinct:
  separate; this pass is naming + methods only.
- **Should I land each rename + each promotion as a
  separate commit, or batch per-file**? My instinct:
  per-file commits, with the commit message listing the
  promotions / renames done.
- **Does signal's `Slot(pub u64)` violate the "wrapped
  field is private" rule** in
  [rust/style.md §Domain values are types, not primitives](../repos/tools-documentation/rust/style.md)?
  Probably yes — should it be `pub struct Slot(u64)` with
  accessors? Worth examining as part of the signal verify
  pass.

---

## 10 · What I'll do next (if approved)

1. Confirm the plan or adjust per your feedback.
2. Stage 1.1 — nexus self-review. Promote
   `relation_kind_from_variant_name` and
   `check_bind_name` (and decide on `is_pascal_case` /
   `is_lowercase_identifier`); spell out any naming
   slips.
3. Push, then move on to stage 1.2 (signal).
4. Continue through the stages.

If you'd rather I pause at any stage for review, say so.

---

*End 093.*
