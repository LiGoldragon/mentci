# Report 012 — nexus as the greatest database messaging system

Research-driven vision document. What would it take for nexus to
be the best-in-class messaging protocol for typed,
content-addressed, locally-first databases? Current state is
~70% of MVP; this report lays out the remaining ~30% and the
decade-plus direction.

Source: agent-assisted survey of SQL, SPARQL, Cypher, GraphQL,
Datalog/Prolog, Datomic Pull API, Pathom/EQL, Gremlin, AQL,
miniKanren, Kafka, Materialize, GraphQL Subscriptions, IPFS,
Hypercore, Scuttlebutt, CRDTs, event sourcing, Irmin.

---

## 1. Thesis

"Greatest" is niche-relative. Nexus isn't trying to beat
Postgres at shared-mutable tables, or Datomic at attribute-value
datoms, or GraphQL at API-layer resolvers. Nexus's niche is
specific:

> **Typed, content-addressed, locally-first databases with
> schema-driven semantics.**

In that niche, nexus has a shot at dominance because the niche
itself is small, underserved, and growing (local-first software,
CRDTs, content-addressed systems, distributed knowledge bases).

Nexus wins in that niche by being **radically minimal yet
sufficient** — 7 delimiter pairs, 4 sigils, one token, five
message shapes. Smaller than any competitor's grammar; equally
expressive for its domain.

---

## 2. Lineage — what nexus owes and what it rejects

Four lineages shape nexus:

**Query.** Nexus inherits pattern-matching from Cypher/Datalog,
shape-based projection from GraphQL/EQL, bind aliasing from
unification (Datalog/miniKanren). Rejects SQL's
JOIN-ON-column-name (schema-driven positional eliminates it),
Cypher's ASCII-art edges (nota delimiters are cleaner), Pathom's
dynamic map reliance (nexus stays schema-driven).

**Write.** Nexus inherits immutability from event sourcing + Git,
append-only logs from Kafka/Hypercore, content addressing from
IPFS/Git. Rejects SQL DML's in-place mutation semantics, CRDTs'
requirement to pre-commit to a merge function (nexus defers
conflict resolution to the app/criome layer).

**Reactivity.** Inherits materialised-view incremental recompute
from Materialize, subscription notifications from GraphQL.
Rejects Kafka's opaque-blob payload (nexus messages are
schema-typed).

**Distribution.** Inherits content addressing from IPFS, per-peer
logs from Hypercore, federation from SPARQL. Rejects single-log
assumption of Hypercore (criome will need cross-log queries).

---

## 3. Current state — honest critique

### Strong

- **First-token-decidability** across the entire grammar. Every
  construct identifiable from the opening character. Rare among
  messaging protocols.
- **Schema-driven binding with auto-naming** (`@fieldname`
  derives the bind name from the schema). Chef's kiss — no
  field names in text, no ambiguity.
- **Records as the fundamental unit.** Positional + Rust ADTs
  makes wire format extraordinarily terse (`(Point 3.0 4.0)` vs
  JSON's `{"x": 3.0, "y": 4.0}` — 40% the bytes).
- **Nota's bare-string form.** `nota-serde` instead of
  `[nota-serde]` keeps configs readable.
- **Immutability + content-addressing** as the foundation.
  Every record has a blake3 hash; history is free; time travel
  is cheap.
- **Constraint conjunction** (`{| pat1 pat2 |}`) with shared
  bind unification is compact and powerful.

### Missing — tiered

**Tier 1 (MVP-blocker for horizon-rs-quality consumer):**

1. **Optional patterns** — LEFT-JOIN semantics. Real queries
   want "give me X and maybe Y."
2. **Aggregation** — count/sum/group-by. Without it you fetch
   everything and aggregate client-side.
3. **Pagination / limits** — unbounded result sets are
   unusable at scale.
4. **Atomic transactions** — multi-mutate-or-none.
5. **Subscriptions** — reactive queries. Without them the
   protocol is unusable for real-time / collaborative systems.

**Tier 2 (mature-nexus credibility):**

6. Datalog-style rules (derived facts).
7. Path expressions / transitive closure.
8. Temporal queries (`@time:<hash-or-date>`).
9. Conditional writes (CAS).
10. UNION / alternation.
11. Provenance (author, timestamp, signature on records).

**Tier 3 (differentiation):**

12. Schema evolution / migration records.
13. Cross-opus queries.
14. Streaming / partial results.

**Tier 4 (explicitly out of scope — see §6):**

User-defined functions, implicit joins, in-protocol ACLs,
distributed transactions, graph algorithms.

---

## 4. Nexus's unique wedge — three things only nexus can do easily

### 4.1 Immutable record versioning without explicit branching

Every record has a blake3 hash. "Undoing" a mutation means
asserting the previous record's hash. Time travel is a
query-time choice (`@time:HASH`), not a schema choice.

- SQL: UNDO requires triggers / transaction logs / extensions.
- Datomic: immutable datoms, but querying at-time requires
  explicit `as-of`.
- Git: content-addressed but optimised for trees, not records.

### 4.2 Append-only replication without explicit sync

Nexus messages append to an immutable record log per peer. That
log replicates trivially: "send all records I've seen";
deduplicate by content hash.

- SQL: replication requires primary keys, row versioning,
  conflict resolution.
- CRDTs: require pre-committing to a merge function.
- Event sourcing: replicate events, but event format is
  per-domain.

### 4.3 Schema-driven compactness + transparency

Positional records + schema-driven binding give a wire format
that's simultaneously compact (bytes), transparent (human-
readable), and unambiguous (no field-name ambiguity).

- SQL: text, but verbose (field names repeated).
- Protobuf: compact binary, opaque without `.proto`.
- GraphQL: transparent but verbose.

---

## 5. Proposed Tier-1 syntax additions

Each needs concrete syntax that preserves first-token-
decidability, doesn't collide, and fits the existing grammar.

### 5.1 Optional patterns — `?(| pattern |)`

```nexus
?(| Point @horizontal @vertical |)
```

Match if present; if no match, binds resolve to `None` (or the
result tuple is empty for that pattern). `?` is unused in the
current grammar; no collision.

### 5.2 Aggregation — extend shape

```nexus
(| Point @h @v |) { count }
(| Point @h @v |) { sum @v }
(| Point @h @v |) { group-by @h sum @v }
```

Aggregation keywords live inside `{ }` (shape delimiter). The
parser recognises `count`, `sum`, `min`, `max`, `avg`,
`group-by` as aggregation forms. No new delimiter needed.

### 5.3 Pagination — query pipeline

Two proposals. Both valid, pick one:

- **(a)** Reuse `|` as pipeline separator at top level:
  `(| Point @h @v |) | limit 10 | offset 20 | order-by @h`.
  Risk: `|` already appears inside delimiter pairs; at top
  level might confuse.
- **(b)** Arrow `->`: `(| Point @h @v |) -> limit 10 -> offset 20`.
  Cleaner separation; new token.

**Lean (b).** `->` is distinctive and unambiguous.

### 5.4 Atomic transactions — `{|| ... ||}`

```nexus
{||
  ~(Point 0.0 0.0)
  ~(Circle 5.0)
||}
```

Double-brace delimiter (parallel to existing `{| |}` but
distinct). All mutations succeed or none. Failure semantics:
rollback, no partial apply.

First-token decidability: yes — `{||` is distinct from `{|`.

### 5.5 Subscriptions — `&(| pattern |)`

```nexus
&(| Point @h @v |)
```

Opens a reactive stream. Nexusd sends a diff message each time
a record matching the pattern is asserted / mutated / retracted.
`&` is currently unused; no collision (though it's also my
proposed sigil for file-include in report 011; pick one).

**Conflict flag:** `&` is proposed for both subscriptions and
file-include. Pick one use per sigil. My vote: `&` for file-
include in nota (appears more commonly in configs); subscriptions
use something else like `??(| … |)` or a new sigil. Or swap:
subscriptions use `&`, file-include uses a compound form like
`[& path &]`.

---

## 6. What nexus should NOT do

Explicit non-goals — what nexus refuses to become, to preserve
minimalism:

1. **Arbitrary user-defined functions.** Computation belongs in
   the schema / verifier layer (sema), not messaging. If a query
   needs custom logic, encode it as a derived type in the schema.
2. **Heuristic / implicit joins.** Schema-driven explicit
   matching (bind aliasing, constraint conjunction) is correct
   by construction. Implicit joins are subtle-bug factories.
3. **ACLs / encryption in the protocol.** Sign the author, check
   permissions at the app layer. Don't burden the core.
4. **Distributed transactions across peers.** Coordination
   belongs in criome (higher layer). Nexus stays append-only +
   local-first.
5. **Built-in graph algorithms.** Shortest path, PageRank,
   community detection — domain-specific, belongs in query
   engines, not protocols.

Saying "no" preserves the grammar's radical minimalism. Every
"yes" adds delimiters, sigils, cognitive load.

---

## 7. Roadmap

| Phase | Window | Adds | Blocking on |
|---|---|---|---|
| 0 MVP | now → 2026-Q3 | Finalise 5 shapes; ship nexusd + query engine; horizon-rs first consumer | implementation time |
| 1 MVP+ | Q3 → Q4 2026 | Optional; aggregation; pagination; atomic txns | Phase 0 |
| 2 Mature | Q4 2026 → Q2 2027 | Subscriptions; temporal queries; conditional writes; maybe rules | Phase 1 + push infra |
| 3 Federation | Q2 → Q4 2027 | Criome MVP; cross-peer queries; signatures; conflict resolution | Phase 2 + criome |
| 4 Polish | 2028+ | Datalog rules; path expressions; schema evolution; graph algos as needed | Phase 3 |

None blocked by design problems. All blocked by implementation
time. The design is stable.

---

## 8. The pitch — FAQ for a skeptical engineer

**"Why not Postgres / SQL?"** Nexus's niche requires immutability
+ content-addressing + local-first replication. SQL assumes
mutable shared tables; those are a fundamentally different
model. Use Postgres for traditional web apps; use nexus for
collaborative / distributed / time-travelling systems.

**"Why not Datomic?"** Datomic queries datoms (entity-attribute-
value triples); nexus queries records. Records are richer —
they inherit Rust's algebraic data types (sum + product + newtype
discrimination) directly. Datomic is also a full database (with
licensing); nexus is a protocol (wire-level), usable with any
storage backend.

**"Why not GraphQL?"** GraphQL lives at the application layer —
it's a resolver abstraction over existing databases. Nexus lives
at the wire layer — it's what the database speaks. No impedance
mismatch between "query language" and "storage format."

**"Why not CRDTs?"** CRDTs solve merge semantics; they don't give
you a query language. Nexus + CRDTs (+ criome) is a coherent
stack: nexus is query/message, CRDTs handle concurrent-edit
merge, criome handles replication.

**"Performance?"** Honest answer: single-machine nexusd is
probably 2-5x slower than Postgres for complex queries today.
Postgres has 50 years of query optimisation. Nexus is new. But:
for the features nexus offers for free (time travel, replication,
audit, content-addressing), comparable implementations on top of
Postgres would cost more. The tradeoff is domain-specific.

**"Learning curve?"** Grammar has 7 delimiters, 4 sigils, one
token. Smaller than SQL's keyword count. A Rust engineer can
learn it in a day. A non-Rust engineer needs more ramp to grok
positional records + schema-driven binding, but the grammar
itself is tiny.

**"Production-ready?"** Not yet (2026-Q2). MVP ships Q3 2026;
credible production is Q2 2027 at earliest. Adopting now means
betting on the direction. For early/bold teams in the right
niche, the bet is good.

---

## 9. Open questions for you

1. **Tier-1 scope:** all 5 features for MVP+? Or drop
   subscriptions to Tier 2? (Subscriptions are the heaviest to
   implement — push infrastructure + incremental recompute.)
2. **Pipeline syntax:** `|` reuse (conflicts with delimiter
   interiors) or `->` (my vote)?
3. **Subscription sigil:** `&` (but conflicts with my file-
   include proposal) or something else? See report 011 §5 for
   the collision.
4. **Transaction failure:** rollback-and-error (my vote), or
   partial-success-with-report?
5. **Aggregation composition:** can aggregations nest
   (`{ group-by @x { sum @y } }`)? Or flat-only?
6. **Temporal default:** query without `@time:` returns current
   state (my vote) or all time?
7. **Criome timing:** Phase 3 is 2027. Realistic? Too fast / too
   slow?
8. **Rules deferral:** Datalog-style rules in Phase 2 (alongside
   subscriptions) or Phase 4 (after everything else)?

Any answer narrows the design space; many can be deferred until
Phase 1 implementation forces them. But decisions 1-3 are
needed before Phase 1 code starts.
