#   013 — nexus syntax: a delimiter-family framework

*Claude Opus 4.7 / 2026-04-24 · commissioned in response to the
IDE-highlighted §9 Open Questions in [012](012-nexus-vision.md).*

**Thesis.** Nexus should grow by **extending delimiter families**,
not by adding sigils or keywords. The five Tier-1 features, plus
Phase 2's rules and temporal queries, can all land without a
single new sigil — every new capability is a new shape at a free
position in an existing delimiter family. Aggregation operators
and pagination directives are spelled as **Pascal-named records**
(a `Limit`-typed record is a Limit the same way a `Point`-typed
record is a Point), not as reserved words. This preserves the
property that reviewers of report 012 called out as nexus's
strongest single trait: **first-token decidability with a
pronouncable grammar of <10 atoms.**

---

## 1 · Answering the eight open questions

Ordered as §9 of [012](012-nexus-vision.md) posed them. User's
pre-stated direction is quoted verbatim where given; my
research-backed position follows.

**Q1 — Tier-1 scope.** User: *all features.* Confirmed. All five
land in Phase 1 (MVP+). Subscriptions are the heaviest but they
drive the reactive use-cases criome depends on — deferring them
makes Phase 2 suddenly huge. See §6 for phase realism.

**Q2 — Pipeline syntax.** User: *not sure, look at this.*
**Proposal: no pipeline operator.** Drop both `|` and `->`.
Sequential application of record-operators via juxtaposition is
sufficient — see §3.3. Existing grammar already reads left-to-
right at query top level; a separator adds visual clutter
without disambiguation. (Reviewed against SQL, Kusto, Elixir,
F#, Datomic Pull — each needed a pipe because their expressions
weren't uniformly parenthesised. Nexus's are.)

**Q3 — Subscription delimiter.** User: *prefer a delimiter.
`<||>` mirrors the vector idea (a stream).* **Adopted.** `<| |>`
is the stream/subscription delimiter. The `<` family becomes
"ordered flow" — `< >` is finite sequence, `<| |>` is unbounded
stream. This is the keystone move that makes the rest of the
framework cohere; see §2.

**Q4 — Transaction failure.** User: *deep research.*
**Proposal: rollback-and-error is the default; partial-success
is a separately-delimited shape.** Research summary below.

> Comparative: Postgres/MySQL/Sqlite/Oracle all default to
> rollback on `ABORT`. Datomic transactions are atomic
> commit-or-fail. FoundationDB, Spanner, CockroachDB — all
> atomic. Event-sourcing systems (Kafka-as-ledger) are
> append-only so the concept is N/A. The only systems that
> default to partial-success are batch-update APIs (e.g., AWS
> DynamoDB `BatchWriteItem`) which explicitly return both
> committed and failed items — and those systems are notorious
> for subtle bugs where callers forget to check the per-item
> status. **Rollback is the non-surprising default for anything
> that calls itself a "transaction."**
>
> Partial-success has valid uses (bulk imports, idempotent
> upserts where items are independent), but those are a
> different semantic shape and deserve a different delimiter.
> My recommendation: introduce it later as `{#… #}` (distinct
> from `{|| ||}`) if and when the use case is real. For now,
> Phase 1 ships only atomic transactions. If the need surfaces,
> the `{#…#}` slot is unused and available.

**Q5 — Aggregation composition.** User: *deep research.*
**Proposal: aggregations nest freely because they are records.**
Aggregations are Pascal-named records (`Sum`, `Count`, `GroupBy`,
etc.). Records nest by definition in nota. `(GroupBy @h (Sum @v))`
is a GroupBy record whose body is a Sum record — no special
grammar. This makes composition a non-decision: it comes for
free the moment we commit to "operators are records."

> Comparative: SQL nests aggregations awkwardly via subqueries
> (`SELECT sum(count(x)) …` is actually illegal; must be
> `SELECT sum(c) FROM (SELECT count(x) AS c …)`). Kusto's
> `summarize` syntax allows nesting via parentheses. Datomic's
> pull API nests naturally because everything is a map. Nexus
> nests naturally because everything is a record. The
> record-as-operator choice collapses a dozen SQL corner cases
> (HAVING vs WHERE vs window functions) to uniform nesting.

**Q6 — Temporal default.** User: *deep research.*
**Proposal: query without time-scoping returns current state.**
Default to "now." Research:

> Every time-travel database I could find defaults to current:
> Datomic (`db` is always the latest db-value; `as-of` is
> explicit), XTDB (valid-time + transaction-time both default to
> now), Irmin (`Store.get` is HEAD; historical requires explicit
> commit hash), Git (HEAD unless `:<ref>`). The reason is that
> cross-time queries return a cross-product — almost never what
> the user wants. "All time" is a niche query mode, not a
> default.
>
> Proposed syntax for temporal scoping (Phase 2): a prefix
> delimiter pair that scopes the query to a time-view. See §3.7.

**Q7 — Criome timing.** User: *should be live before the
solstice, for sure. this will not be linear progress.* **Taken
as: summer solstice 2026-06-21, ~58 days from now.** Given the
MVP still has M2/M3/M4/M5/M6 ahead of it, "live before solstice"
means MVP-self-hosting demonstrated; Phase 1 Tier-1 features
land the quarter after. Criome itself remains a 2027+ thing —
see §6 for what's realistic per phase. (If the user meant winter
solstice 2026-12-21, shift everything ~6 months right.)

**Q8 — Rules deferral.** User: *phase 2.* Confirmed. Rules join
subscriptions in Phase 2. The delimiter is already reserved in
§2 below (`[|| ||]`) — so no syntax decision is deferred, only
the implementation.

---

## 2 · The delimiter-family framework

Nota + current-nexus has **7 delimiter pairs** and 4 sigils.
Every pair can be described as belonging to an **outer-character
family**, with an optional **pipe-escalation** (`| |` or `|| ||`)
inside that turns the pair into a more abstract semantic variant.
Once this pattern is named, the holes in the matrix become the
natural locations for new features.

### 2.1 The matrix

| Family | Bare | One-pipe (`\| \|`) | Two-pipe (`\|\| \|\|`) |
|---|---|---|---|
| `( )` — record / singular thing | record: `(Point 3.0 4.0)` | **pattern**: `(\| Point @h @v \|)` | **optional pattern** (new) |
| `{ }` — composite / structure | shape: `{ @h @v }` | **constrain** (conjunction): `{\| p1 p2 \|}` | **atomic txn** (new): `{\|\| m1 m2 \|\|}` |
| `[ ]` — string / container of bytes | inline string: `[text]` | multiline string: `[\| … \|]` | **rule** (new, Phase 2): `[\|\| head body \|\|]` |
| `< >` — ordered flow | sequence: `<1 2 3>` | **stream / subscription** (new): `<\| pat \|>` | windowed / paginated stream — reserved, see §3.3 |

Reading key: the **opening character** says what kind of
containment this is (record-ness, composite-ness, byte-ness,
ordered-flow-ness). The **`|` count inside** escalates
abstraction:

- **0 pipes** — concrete value of that family.
- **1 pipe** — *abstracted* form (matches, conjuncts, multiline,
  reactive — in each case, a more "symbolic" version of bare).
- **2 pipes** — *scoped commitment* form (may-not-match,
  all-or-nothing, derivation, windowed). Two-pipe means "this is
  a bounded region with its own transaction/scope semantics."

I claim this is a **discovered** pattern, not an imposed one —
nota+nexus already had `()/(||)` and `{}/{||}` and `[]/[||]`.
Adding `<>/<||>` completes a symmetric 4×3 matrix that can be
taught as one rule: *outer char picks family, pipe count picks
abstraction level.*

### 2.2 Why this beats sigil-per-feature

Report 012 proposed `?` for optional, `&` for subscription, and
`{|| ||}` for transactions. That's three unrelated syntactic
moves (two new sigils + one new delim). The matrix above uses
**zero new sigils** and four already-visually-related delimiters.

The cognitive difference: once the user learns "pipe count =
abstraction level," every new feature placed in the matrix is
**pre-learned**. A user who understands that `(Point 3.0 4.0)`
is a record and `(| Point @h @v |)` is a pattern will correctly
guess that `(|| Point @h @v ||)` is "*maybe* a pattern" before
seeing any documentation — because the escalation is semantically
uniform across families.

Report 012's sigil-per-feature path trends toward APL — each
new feature costs a new mark. The matrix path trends toward
Lisp — the same few delimiters cover everything.

### 2.3 Sigil scorecard

| Sigil | Role | After this proposal |
|---|---|---|
| `;;` | Line comment | unchanged |
| `#` | Byte-literal prefix | unchanged |
| `~` | Mutate marker | unchanged |
| `@` | Bind marker | unchanged |
| `!` | Negate marker | unchanged |
| `=` | Bind-alias (narrow, only in `@a=@b`) | unchanged |

**Net: zero sigils added.** All Tier-1 features and Phase 2
rules/temporal land via delimiter-family extensions.

---

## 3 · Feature-by-feature

### 3.1 Optional patterns — `(|| pattern ||)`

LEFT-JOIN semantics. Match if present; bound values become
`None` (or absent in the result tuple) if no match.

```nexus
(| Author @a |)
(|| Editor @a ||)       ;; optional — author may or may not be editor
```

Rationale for the shape: an optional pattern is *wrapping a
pattern in a scope where non-match is survivable*. Scope =
pipe-escalation in the record family (1 pipe → 2 pipes).

**Comparisons rejected:**
- `?(| … |)` (report 012's original proposal) — adds the `?`
  sigil to the global alphabet; breaks the zero-new-sigils
  discipline.
- `(Option (| Point @h @v |))` — conflates the *Option type in
  the schema* with the *query-time optionality of a match*.
  These are different semantics; collapsing them is subtle-bug
  bait.
- Keyword-at-position-0 like `(| Maybe Point @h @v |)` — `Maybe`
  would shadow any user type named `Maybe`.

**First-token decidability:** `(||` is distinct from `(|` at two
characters of lookahead (both start with `(`, but `(|` followed
by identifier vs `(||` followed by identifier differentiates at
position 2). Aski-style lexers tokenise `(||` as one
`OpenOptionalPattern` token, keeping single-token dispatch.

### 3.2 Aggregation — records inside shape

Aggregation operators are **Pascal-named records** defined in a
built-in module (call it `nexus::aggregate::*`). The shape
delimiter `{ }` accepts a mix of field-bind projections and
aggregation records.

```nexus
(| Order @customer @amount |) { @customer (Sum @amount) }

(| Order @customer @amount |)
  { (GroupBy @customer (Sum @amount) (Count)) }

(| Order @customer @amount |)
  { (Top 10 (GroupBy @customer (Sum @amount))) }
```

Defined operators (Phase 1):

| Record | Meaning |
|---|---|
| `(Count)` | Count of matches |
| `(Count @bind)` | Count of distinct values at `@bind` |
| `(Sum @bind)` | Sum of numeric bind |
| `(Min @bind)`, `(Max @bind)`, `(Avg @bind)` | Extrema / mean |
| `(GroupBy @bind …)` | Group by `@bind`, apply inner aggregations per group |
| `(Having pattern)` | Post-aggregation filter — pattern must match the grouped row |
| `(Top N op)` / `(Bottom N op)` | Top/bottom N by the enclosed aggregation |

**Nesting is free**: records nest. `(GroupBy @c (Sum @a) (Count))`
is one GroupBy containing two aggregations. `(Top 10 (GroupBy @c
(Sum @a)))` is Top-10 of the GroupBy output. Whatever SQL does
with `HAVING`, subqueries, and window functions is covered by
literal record nesting.

**No keywords.** `Sum` is a schema name like `Point`. It looks
up in the schema, finds the built-in `nexus::aggregate::Sum`
type, and dispatches. The same lexer handles it; the same
Pascal-first-char rule classifies it. No reserved-word list.

**Position-defines-meaning inside `{ }`:**

- `camelCase` (or `@camelCase`) → project that bind / field
- `PascalCase(…)` → aggregation / operator record
- Juxtaposition → concatenate the projection

This mirrors aski's "position defines meaning" — the same
delimiter `{ }` does projection (concrete) and aggregation
(operator) differentiated by first-character class.

### 3.3 Pagination — records, no pipeline

Proposal: `Limit`, `Offset`, `OrderBy`, `Before`, `After`,
`Reverse` are **records in `nexus::query::*`**, applied as
juxtaposed records at query top level.

```nexus
(| Point @h @v |)
  (OrderBy @h)
  (Limit 10)
  (Offset 20)
```

No `|` or `->` between. Juxtaposition = pipeline. Grammar rule
at query top-level: after a pattern/constrain, zero-or-more
record-operators apply in order.

**Why no pipeline operator:**

> SQL needs `|>` (Postgres 16+) because its SELECT/FROM/WHERE
> blocks are unparenthesised and whitespace-separated
> keywords — visually indistinguishable from the surrounding
> prose without a pipeline marker. Nexus has no keywords; every
> operator is a parenthesised record. The parens themselves mark
> the operator boundary. A pipeline operator would be redundant
> visual ink.
>
> Kusto (`summarize | extend | project`) needs pipes because its
> tabular operators are unparenthesised keywords. Same failure
> mode as SQL.
>
> F# / Elixir `|>` is meaningful because functions in those
> languages are whitespace-applied (`f x y`), so you need a
> marker to know which way data flows. Nexus's records are
> explicitly-typed nouns, not whitespace-applied verbs — no such
> ambiguity.

**Non-linear pipelines are explicit too.** If a consumer needs
to branch or join, they issue two messages and combine client-
side. Nexus is the wire format, not the workflow engine.

**Cursor-based pagination** for subscriptions uses the same
records:
```nexus
<| (| Order @id |) (After @lastCursor) (Limit 100) |>
```
The `<| |>` scope turns the query reactive; the operators inside
work the same way as in a one-shot query.

### 3.4 Atomic transactions — `{|| m1 m2 … ||}`

All mutations inside succeed or none.

```nexus
{||
  ~(Point 0.0 0.0)
  ~(Circle (Circle 5.0))
  !(Deprecated)
||}
```

**Failure behaviour:** Rollback + structured error. On any
failure (write-conflict, constraint violation, schema error),
the entire `{|| |}` block leaves no trace and the response is
an error message describing the first failing mutation. No
partial apply. No "best effort" — this is the whole point of
"atomic."

The response envelope: `(TxnError (FailedAt N) (Reason String))`
where `N` is the 0-based index of the failing mutation inside
the block.

**Nested transactions:** disallowed at the wire level. A
`{|| |}` block cannot contain another `{|| |}`. (If needed
later, savepoint semantics can join via a separately-delimited
`{# # }` shape — reserved.)

**First-token decidability:** `{||` distinct from `{|`, same
reasoning as `(||` vs `(|`.

### 3.5 Subscriptions — `<| pattern |>`

Reactive stream. The server sends diff messages each time a
record matching the pattern is asserted, mutated, or retracted.

```nexus
<| (| Order @customer @amount |) (Limit 100) |>
```

The server produces a sequence of diff messages:
```nexus
;; initial snapshot
(SubSnapshot <records…>)

;; thereafter, per matching change
(SubAssert   (| bindings… |))
(SubMutate   (| old… |) (| new… |))
(SubRetract  (| bindings… |))
```

**Why `<| |>` is the right family:** a subscription is an
*ordered flow of observations over time*. The `<` family
already owns "ordered flow" (sequences). One-pipe-escalation in
the `<` family is the exact right location for "reactive
sequence that doesn't end." Reading `<| … |>` as "stream of the
thing inside" is the natural interpretation.

**Reservation: `<|| … ||>` for windowed streams.** Phase 2 or
later. A windowed stream is a stream with an explicit temporal
or count-based window closure. Syntax sketch:
`<|| pattern (Window (Minutes 5)) ||>` — opens a tumbling
5-minute window, emits aggregate per window close. Does not
land in Phase 1; slot reserved.

### 3.6 Rules (Phase 2) — `[|| head body ||]`

Datalog-style derived facts. A rule says "if the body matches,
the head is asserted."

```nexus
[||
  (Ancestor @a @c)
  {|
    (| Parent @a @b |)
    (| Ancestor @b @c |)
  |}
||]
```

First element is the head (a record shape to assert), second is
the body (a constrain block). Rules are persistent: once
registered, the engine re-evaluates them on every write and
updates the derived facts.

**Why `[|| ||]`:** the `[` family owns "container of bytes /
evaluated content." `[| |]` is a multiline string (content that
evaluates only as text). `[|| ||]` escalates to "evaluated with
derivation semantics" — the body is code, the head is output.
Consistent with the family's "evaluation" flavour.

**Alternative considered: add a `:-` token.** Rejected. `:-`
would be a new multi-character token that doesn't fit any
existing lexical class, is visually foreign to nota's typography,
and forces a new parse state (rule-head vs rule-body) that
`[|| … ||]` gets for free from positional dispatch.

**Recursive rules** (the `Ancestor` example above is a classic)
work because the body can reference the head. The engine uses
semi-naïve evaluation internally.

### 3.7 Temporal queries (Phase 2) — scoping by time-view

Time-view as a prefix record applied to the whole query:

```nexus
(TimeAt #<hash-64>) (| Point @h @v |)    ;; pinned to a snapshot
(TimeBetween #<hash-a> #<hash-b>) (| Point @h @v |)   ;; range
(TimeAll) (| Point @h @v |)              ;; full history
```

`TimeAt`, `TimeBetween`, `TimeAll` are records in
`nexus::temporal::*`. No new sigil (`@time:` in report 012 was
replaced; `@time:` would force `@` to mean two things depending
on context).

**Default remains current state** — no time-record prefix means
"now."

**Composability:** temporal records compose with subscriptions
(`<| (TimeAt #…) (| pattern |) |>` streams from a starting
point), with pagination, with aggregation. All orthogonal.

---

## 4 · Grammar delta

Precise changes to the nota/nexus specs.

### 4.1 nota

**No changes.** `< >` sequence stays as-is. The `<| |>` family
is a *nexus* extension (nota remains the pure-data subset).

**Optional future nota change** (not in this proposal but
reserved): if nota ever needs a "streaming document" form,
`<| |>` is the natural slot. Phase 3+.

### 4.2 nexus

The six changes:

1. **New delimiter pair `<| |>`.** Surface lexer emits
   `OpenStream` / `CloseStream` tokens at `<|` and `|>`.
2. **New delimiter pair `(|| ||)`.** Lexer tokens
   `OpenOptionalPattern` / `CloseOptionalPattern`.
3. **New delimiter pair `{|| ||}`.** Lexer tokens `OpenTxn` /
   `CloseTxn`.
4. **New delimiter pair `[|| ||]`** (Phase 2). Lexer tokens
   `OpenRule` / `CloseRule`.
5. **Query top-level becomes record-operator-friendly.** After
   a pattern or constrain, zero-or-more operator records may
   appear as siblings; each applies to the accumulated result.
6. **Shape body accepts Pascal-named records** as aggregation
   directives, alongside camel-named field projections.

Lexer lookahead: `<`, `(`, `{`, `[` at position 0 followed by
`|` at position 1 followed by `|` at position 2 is a two-pipe
open. One `|` is one-pipe open (existing behaviour). No pipe is
bare (existing).

First-token decidability preserved: every new delimiter has a
unique two-or-three-character opening sequence.

### 4.3 nexus-schema / nexus-serde impact

- **nexus-schema** grows a new crate-internal module
  `nexus_schema::query` holding the operator records
  (`Limit`, `Offset`, `OrderBy`, `Count`, `Sum`, `GroupBy`,
  etc.) and `nexus_schema::temporal` (Phase 2).
- **nexus-serde** gains four new token pairs in the lexer.
  Each adds a `Token::*` variant. Ser/de dispatch adds arms for
  each new delim family; pattern at position "stream body" is
  routed differently from "record body."
- Duplication with nota-serde-core remains managed per
  [reports/011](011-code-sharing-and-deferred.md): the four new
  token variants are nexus-only and live in nexus-serde's ~140
  LoC of unique-to-nexus code.

### 4.4 nexus-cli / nexusd impact

- **Parser:** two new delimiter tokens (`<| |>` in Phase 1;
  `(|| ||)` + `{|| ||}` in Phase 1; `[|| ||]` in Phase 2;
  `TimeAt/TimeBetween/TimeAll` records in Phase 2 are schema
  additions, not grammar additions).
- **Dispatch:** query engine routes on first-token.
- **Wire:** unchanged — same text format, just more shapes
  recognised.

---

## 5 · Worked examples

### 5.1 One-shot query with all features

```nexus
(| Order @customer @amount |)
  (|| Customer @customer @tier ||)
  { @customer @tier (Sum @amount) (Count) }
  (OrderBy (Desc (Sum @amount)))
  (Limit 50)
```

Reads: match Orders, LEFT-JOIN to Customer, group by
customer+tier, sum and count, order by sum descending, top 50.

No keywords. No sigils added beyond `@` (bind) which was
already there. Eight delimiter pairs total used in this query:
`(| |)`, `(|| ||)`, `{ }`, `( )`.

### 5.2 Reactive query

```nexus
<|
  (| Order @customer @amount |)
  (After #<last-hash>)
  (Limit 100)
|>
```

Stream of orders since a cursor, batched 100 at a time.

### 5.3 Atomic transaction

```nexus
{||
  ~(Account (AccountId alice) 100)
  ~(Account (AccountId bob)   200)
  (Transfer (AccountId alice) (AccountId bob) 50)
||}
```

All three or none. Engine checks schema invariants (e.g., "no
account goes negative") and rolls back if any fail.

### 5.4 Rule (Phase 2)

```nexus
[||
  (TransitiveReport @employee @manager)
  {|
    (| Report @employee @manager |)
  |}
||]

[||
  (TransitiveReport @employee @top)
  {|
    (| Report @employee @intermediate |)
    (| TransitiveReport @intermediate @top |)
  |}
||]
```

Two-clause rule for transitive closure. Both heads are
`TransitiveReport`; engine unions the derivations.

### 5.5 Temporal query (Phase 2)

```nexus
(TimeAt #a3f2b1…) (| Account @id @balance |) { (Sum @balance) }
```

Total balance at a specific snapshot.

---

## 6 · Phases and solstice realism

**Given**: user wants "live before the solstice."

### Assumption

- **Solstice = summer, 2026-06-21.** 58 days from today
  (2026-04-24).
- If this is wrong and the user meant winter (2026-12-21),
  shift every phase ~6 months right.

### What's achievable

| Phase | Target window | Milestone | Concrete outcome |
|---|---|---|---|
| **0 — MVP self-hosting** | now → 2026-06-15 | M2/M3/M4/M5/M6 | rsc-generated binary edits its own DB and rebuilds; nota/nexus specs finalised at current surface; **no Tier-1 additions yet** |
| **Phase 1 — Tier-1 syntax** | Jun–Oct 2026 | optional + aggregation + pagination + atomic txn + subscriptions | all five delimiter-family additions land; nexusd query engine ships; first external consumer (horizon-rs) integrates |
| **Phase 2 — Rules + temporal** | Oct 2026 – Feb 2027 | rules (`[\|\| \|\|]`), temporal records, windowed streams | the remaining framework slots fill in |
| **Phase 3 — Criome / federation** | Feb 2027 – ? | criome MVP, cross-peer queries | federation; the report-012 §7 "Phase 3" compressed forward |

"Live before solstice" reads as **Phase 0 — MVP self-hosting
demonstrated**. Tier-1 lands in the quarter following. Criome
remains the outer goal.

### Why Phase 0 can hit 2026-06-15

- M2 method-body layer is ~400 LoC per
  [mentci-next-4jd](003-mvp-implementation-plan.md). Two
  focused sessions.
- M3 sema redb wrapper is a thin layer over redb + blake3 +
  rkyv; <500 LoC.
- M4 nexusd is a ractor actor + unix-socket framing layer;
  <500 LoC, mostly glue.
- M5 rsc projection is the hardest — it's one codegen rule per
  nexus-schema variant, iterating until output compiles.
  Rough estimate: 1000-1500 LoC, with the long-tail being
  diagnostic quality.
- M6 bootstrap is populating the DB (scripted) + running the
  loop (cargo invocation) + diffing outputs. <200 LoC.

Assumed productivity: **not linear.** User's own caveat. The
likely shape is a slow M2/M3 week, a breakthrough weekend on
rsc, and a scrambling M6 finish. The risk is rsc codegen fidelity
eating more time than expected.

### Blocking decisions for Phase 0

Not the syntax proposal. The syntax proposal is Phase 1. Phase 0
ships with current nexus grammar (no Tier-1 yet). The things
that block Phase 0 are the P1 per-repo bd items:

- [sema-5d3](../repos/sema/) — opus identity
- [mentci-next-ef3](003-mvp-implementation-plan.md) — capstone feature
- [nexus-schema-5rw / -wq3 / -tu8](../repos/nexus-schema/) — schema design

None of these need the Tier-1 syntax to be decided. Phase 0 can
proceed in parallel with Phase 1 design work (this document).

---

## 7 · Non-goals (what this proposal explicitly refuses)

Following aski v0.20's rigour of stating what's *Confirmed OUT*:

1. **A pipeline operator** (`|`, `->`, `|>`). Juxtaposition
   suffices. §3.3.
2. **New sigils.** Not for optional (`?`), not for subscription
   (`&`), not for anything. All Tier-1 + Phase 2 features use
   only existing sigils (`~ @ ! # =`) where needed.
3. **Keyword-based operators.** `Limit`, `Sum`, `GroupBy` are
   Pascal-named records, not reserved words. The schema
   distinguishes user types from built-in operator types by
   module path (`nexus::query::Limit` vs `user::Limit`).
4. **Pattern-body-level `|` alternation** (as in regex-style
   `A|B`). `!` + conjunction already suffices:
   `{| (| pat1 |) !(| pat2 |) |}` expresses "pat1 and not pat2."
   Regex-style alternation would need a new grammar state.
5. **Multi-character token additions** (`:=`, `:-`, `<->`, etc.).
   Every syntactic decision stays delimiter-based.
6. **Nested atomic transactions.** One `{|| ||}` block per
   message. Savepoints, if ever needed, go to `{# #}` (reserved
   but not specified).
7. **Mixing `<| |>` subscriptions with `{|| ||}` transactions
   inside each other.** They're separate message shapes; a
   client runs them as separate calls.

---

## 8 · What's still open

Even after this proposal, five questions remain:

1. **Subscription delivery guarantees.** At-least-once,
   at-most-once, exactly-once? Affects wire format for the
   diff messages, not the query syntax. Belongs to nexusd, not
   here.
2. **Rule termination** (Phase 2). Datalog is usually stratified
   or bounded to avoid non-termination. Need to pick a class
   (stratified / linear / full Datalog + bounded
   iterations?). Semantic not syntactic.
3. **Aggregation precision / overflow.** `Sum` over U64s that
   overflows — panic, saturate, widen? Same shape of decision
   as "integer overflow behaviour in Rust." Semantic.
4. **Cross-opus queries.** If a query references records from
   two opera, what's the join story? Report 012 §3 Tier-3. Out
   of scope here.
5. **`(TimeAt #hash)` vs `(TimeAt Kebab-tag)`.** Time-scoping
   might want human-readable tags in addition to hashes. Affects
   temporal syntax only, Phase 2 decision.

All five are bd-track candidates when their phase arrives. None
block the delimiter-family framework.

---

## 9 · Follow-through

If this proposal is accepted, concrete next steps:

**Spec changes (nota/nexus repos):**
- nota README: no change.
- nexus README: §Added by nexus tables grow to include
  `<| |>`, `(|| ||)`, `{|| ||}`, `[|| ||]` (last marked
  Phase 2). Examples added. Grammar section updated with
  first-token lookahead rule for the pipe-escalation pattern.

**bd tracking (per-repo):**
- Close [nexus-hsr](../repos/nexus/) — 8 open questions — after
  user confirmation of this doc; the answers are §1.
- Each of the five Tier-1 issues in nexus bd
  ([nexus-75v](../repos/nexus/), [nexus-7rq](../repos/nexus/),
  [nexus-jo1](../repos/nexus/), [nexus-mat](../repos/nexus/),
  [nexus-qyx](../repos/nexus/)) gets its syntax decision set
  to the matrix form above, tagged for Phase 1 after MVP.
- New bd (Phase 2): "rules delimiter `[|| ||]`" and "temporal
  records."

**mentci-next:**
- This doc lives at [reports/013](013-nexus-syntax-proposal.md).

**Implementation (Phase 1):**
- nexus-serde lexer: four new token pairs (~80 LoC of lexer
  additions + tests).
- nexus-serde de/ser dispatch: four new dispatch arms
  (~200 LoC).
- nexus-schema: `nexus_schema::query` module with operator
  records (~150 LoC).
- nexusd query engine: pattern matching with LEFT-JOIN
  (optional patterns) + aggregation + pagination. This is the
  heavy lift; estimate 1500-2500 LoC depending on optimisation
  ambition.

Tier-1 in nexusd is the work-heavy piece; the syntax half is
~400 LoC across nexus-serde + nexus-schema and should land in
a week once the MVP is out of the way.

---

*End of report 013. The framework stands or falls on one
question: does "outer char = family, pipe count = abstraction"
feel like a discovery or like scaffolding? If the former, it
will shape every future addition; if the latter, revert to
sigil-per-feature and budget accordingly.*
