---
title: 079 — relation-types research (edge semantics across established systems)
date: 2026-04-25
anchor: Li 2026-04-25 "we should have *types of relations* and such things. do some research in this field."
feeds: signal/src/flow.rs (M0 record kinds); reports/078 §M0; criome/ARCHITECTURE.md §3 validator pipeline
status: research synthesis with concrete decision points for Li
---

# 079 — relation-types research

`signal/src/flow.rs` today carries `Edge { from: Slot, to: Slot, label: Option<String> }`. The `label` is free-form
text — useful as a human caption, but not a *type*. Real systems differentiate the **kind** of relation
(depends-on, contains, produces, calls, implements, …) from any free-text annotation: the kind drives validation,
constraint-checking, projection, and reasoning; the label is just decoration on top.

This report surveys ten established systems, draws cross-cutting patterns, and proposes a concrete schema for
criome's M0 flow-graph kinds — staying inside the project's "logic only, validator-checked, content-addressed,
no-macros" constraints. It ends with explicit decision points (A/B/C choices) Li can answer to nail down M0.

---

## 1 · Survey

### 1.1 Property graphs (Neo4j / Cypher)

Neo4j enforces a hard invariant: **every relationship has exactly one relationship type, and exactly one** —
relationships *cannot* carry multiple labels the way nodes can. The relationship type is a string identifier
established at relationship creation. ([Neo4j Graph Database Concepts](https://neo4j.com/docs/getting-started/appendix/graphdb-concepts/),
[CREATE clause](https://neo4j.com/docs/cypher-manual/current/clauses/create/))

For decades this type was an open vocabulary: Cypher accepts any new `:KNOWS` or `:DEPENDS_ON` at write time.
Schemas were soft — index existence, uniqueness, property-existence constraints layered around the open type
namespace.

In **Neo4j 2026.02 (preview)** the picture changes: the new `GRAPH TYPE` feature lets a schema declare
*relationship element types* with **strictly defined valid source and target node labels** — i.e. domain/range
constraints land in the engine, not just in convention. ([GRAPH TYPE schema enforcement preview](https://neo4j.com/blog/developer/graph-type-schema-enforcement-made-easy-preview/),
[Cypher graph types](https://neo4j.com/docs/cypher-manual/current/schema/graph-types/set-graph-types/))

Constraints in modern Neo4j ([Constraints reference](https://neo4j.com/docs/cypher-manual/current/schema/constraints/)):

- **Property uniqueness** on a relationship type
- **Property type** (this property must be `STRING`, `INTEGER`, …)
- **Property existence** (this property must be present)
- **Key constraints** (combined uniqueness + existence)
- **Source/target node-label binding** (new in 2026 GRAPH TYPE)

What it *doesn't* model directly: subtype hierarchies on relationship types, transitivity, inverses. Those live
in user-side query patterns (`(:A)-[:DEPENDS_ON*]->(:B)`) rather than in the schema.

**Take-away for criome**: a single string-typed `relationship_type` per edge is empirically sufficient for an
enormous range of practical modelling. The lift from "open soft schema" to "closed checked schema with
domain/range" is the major recent move and validates the criome instinct to put domain/range checks in the
validator.

### 1.2 RDF / OWL

RDF takes the most extreme position: **the predicate IS the relation type**, denoted by a URI. There is no
separate "type" attached to an edge — every triple `(subject, predicate, object)` is itself an edge of *kind
predicate*. The vocabulary is intrinsically open via URIs; closure happens by convention (use this prefix, this
ontology). ([RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/),
[Semantic triple](https://en.wikipedia.org/wiki/Semantic_triple))

OWL adds a rich type theory over predicates:

- **`owl:ObjectProperty`** — relates instances to instances (i.e. a "real edge")
- **`owl:DatatypeProperty`** — relates an instance to a literal (i.e. an attribute)
- **`rdfs:domain` / `rdfs:range`** — type-of-source and type-of-target
- **`rdfs:subPropertyOf`** — predicate hierarchies (`:fatherOf` ⊑ `:parentOf`)
- **`owl:inverseOf`** — bidirectional pairing (`:hasParent` inverse-of `:hasChild`)
- **`owl:TransitiveProperty`** — `P(x,y) ∧ P(y,z) ⇒ P(x,z)` (e.g. `:ancestorOf`)
- **`owl:SymmetricProperty`** / **`owl:AsymmetricProperty`**
- **`owl:FunctionalProperty`** / **`owl:InverseFunctionalProperty`** — cardinality flavours
- **`owl:ReflexiveProperty`** / **`owl:IrreflexiveProperty`**

([OWL Reference](https://www.w3.org/TR/owl-ref/), [OWL 2 Quick Reference](https://www.w3.org/2007/OWL/refcardLetter))

The crucial design move: **constraints live on the relation type itself**, not on the edges. A reasoner reads
the property axioms once; the cost is amortised over every triple using that predicate. The trade is intricacy:
OWL's reasoning is undecidable in the general case, decidable in fragments (DL, EL, RL, QL).

**Take-away**: OWL is the most expressive lattice we'll encounter. We should not ship most of it for v0.0.1, but
the *categories* it surfaces — domain, range, hierarchy, inverse, transitivity, cardinality — are the canonical
slot-list to consider when defining a `RelationKind` record later.

### 1.3 Datomic

Datomic's design move is striking: **attributes ARE relations**. There's no separate "edges" notion.
`:artist/country` is an attribute of an Artist entity whose value is a *reference* (`:db.type/ref`) to a Country
entity. The "edge" is implicit — the attribute *is* the edge type. ([Schema Reference](https://docs.datomic.com/schema/schema-reference.html),
[Schema Modeling](https://docs.datomic.com/schema/schema-modeling.html))

Each attribute carries:

- **`:db/valueType`** — `:db.type/ref` (an entity reference, i.e. an edge), or a primitive (`:db.type/string`,
  `:db.type/long`, …) for non-edge attributes. **Cannot be changed** after the attribute is created.
- **`:db/cardinality`** — `:db.cardinality/one` or `:db.cardinality/many`. The single uniformity gate for
  arity.
- **`:db/isComponent`** — only meaningful when `:db/valueType` is `:db.type/ref`. Marks the target as a
  *sub-component* of the source: retracting the source retracts the target. (UML composition, in DB form.)
- **`:db/unique`** — `:db.unique/value` or `:db.unique/identity` (uniqueness flavours).
- **`:db/index`** — soft hint, not a relation-type semantic.

([Component Entities](https://blog.datomic.com/2013/06/component-entities.html))

What Datomic distinguishes cleanly: the **kind of relation** (which attribute it is) is identity; the **data on
the relation** is the attribute's metadata (cardinality, isComponent). There's no separate "edge object" with
its own properties — if you need that, you create a reified entity that the attribute points to.

What Datomic doesn't have natively: domain/range (any entity can have any attribute), subPropertyOf, transitivity.
Those are query-pattern concerns in Datalog.

**Take-away**: Datomic's "relation type IS the field name" framing maps cleanly onto criome — a field of a sema
record whose `valueType` is `Slot` (a ref) IS a relation. We're already doing this in nexus-schema's
`signal::ty::TypeRef` machinery. The flow-graph `Edge` is just the explicit object-shaped form of what could
also be modelled as a `Slot` field on the source node — a question worth asking explicitly (decision point D2
below).

### 1.4 TerminusDB / SHACL

TerminusDB merges OWL + SHACL into a single closed-world JSON-LD schema language. ([TerminusDB Schema Reference](https://terminusdb.org/docs/schema-reference-guide/),
[DFRNT comparison](https://dfrnt.com/blog/2023-06-10-exploring-tradeoffs-rdf-owl-sparql-shacl-terminusdb-make-an-informed-decision))

Where TerminusDB diverges from OWL: **closed-world** — what isn't asserted is false; SHACL-style validation is
strict and at-write-time. This matches criome's "every edit is a request; criome validates; rejection is the
hallucination wall" framing exactly.

Cardinality in SHACL ([SHACL spec](https://www.w3.org/TR/shacl/)):

- **`sh:minCount`** and **`sh:maxCount`** on property shapes — the workhorses
- **`sh:qualifiedMinCount`** / **`sh:qualifiedMaxCount`** — *of values that match a class*
- Default cardinality is `{0, unbounded}` (open)

TerminusDB uses `Set` as its primary collection-cardinality construct (the older `Cardinality` is deprecated).
Property shapes also carry a target class — the SHACL equivalent of OWL's `rdfs:domain`.

**Take-away**: SHACL is the sweet spot between "no schema" (RDF) and "too much schema" (OWL DL). criome's
`invariants` validator step is the natural home for SHACL-flavour constraints — *but* SHACL itself is not the
shape we want; we want native sema records that play the same role.

### 1.5 UML class/component diagrams

UML offers exactly **six** relationship kinds, and the choice of six (not three, not twelve) is itself
load-bearing — these capture distinctions that arose from decades of object-modelling practice.
([Class diagram (Wikipedia)](https://en.wikipedia.org/wiki/Class_diagram),
[UML Relationships overview](https://www.umlboard.com/docs/relations/),
[Visual Paradigm: aggregation vs composition](https://www.visual-paradigm.com/guide/uml-unified-modeling-language/uml-aggregation-vs-composition/))

| Relation | Symbol | Semantic distinction |
|---|---|---|
| **Association** | `——` solid line | A has a structural link to B; either can exist independently |
| **Aggregation** | `◇——` hollow diamond | Whole-part, but parts outlive the whole (collection-of) |
| **Composition** | `◆——` filled diamond | Whole-part, parts cannot exist without the whole (Datomic's `:db/isComponent`) |
| **Dependency** | `╌╌▷` dashed open | A uses B; weaker than association; transient (e.g. parameter) |
| **Generalization** | `——▷` solid hollow triangle | A is-a B (subtyping / inheritance) |
| **Realization** | `╌╌▷` dashed hollow triangle | A implements interface B (behavioural conformance) |

The *reason* there are six: each is a different **lifetime/identity coupling** between source and target.
Composition couples lifetimes; aggregation couples identity but not lifetime; association couples reference
but not identity; dependency couples nothing but a transient mention; generalization couples *types* (not
instances); realization couples *contracts* (not types).

Plus **stereotypes** (`«interface»`, `«abstract»`, `«enum»`) — UML's escape hatch for refining a node's *type*
without inventing new node kinds. ([UML Stereotype](https://en.wikipedia.org/wiki/Stereotype_(UML)))

**Take-away**: the lifetime/coupling axis is real, but the six-way split is a domain-specific choice (OO design).
For criome's flow-graphs (architecture diagrams), we will not reuse all six — but the discipline of carving the
coupling axis at *meaningful* joints, not arbitrary ones, is the lesson.

### 1.6 PROV-O (W3C provenance ontology)

PROV-O is a successful **small fixed vocabulary** for provenance. Its core relation set is intentionally tiny:

| Relation | Source kind | Target kind | Meaning |
|---|---|---|---|
| `prov:wasGeneratedBy` | Entity | Activity | this thing was produced by that activity |
| `prov:used` | Activity | Entity | that activity consumed this thing |
| `prov:wasInformedBy` | Activity | Activity | one activity triggered another |
| `prov:wasDerivedFrom` | Entity | Entity | this thing came from that thing |
| `prov:wasAttributedTo` | Entity | Agent | this thing is credited to that agent |
| `prov:wasAssociatedWith` | Activity | Agent | that agent ran this activity |
| `prov:actedOnBehalfOf` | Agent | Agent | delegation chain |

([PROV-O spec](https://www.w3.org/TR/prov-o/))

PROV-O distinguishes **"starting point" terms** (the seven above, plus a few more) from **"expanded" terms**
(qualified relationships that add roles, plans, time-stamps via reification). Most users live entirely in the
starting point set. ([Qualified relations github discussion](https://github.com/w3c/dxwg/wiki/Qualified-relations))

The *qualified* form solves the binary-only-edges limitation: when a relation needs more than `(from, to)`
(e.g. "A used B *with role* C *at time* T"), PROV-O reifies it into a `prov:Usage` instance and attaches
properties. ([CEUR: Reification in OWL](https://ceur-ws.org/Vol-573/paper_19.pdf),
[W3C: N-ary Relations](https://www.w3.org/TR/swbp-n-aryRelations/))

**Take-away — most important data-point in this entire survey**: a *closed* vocabulary of ~7 well-chosen
relations covers the vast majority of provenance modelling. People use the open RDF/OWL substrate, but
discipline themselves to the small fixed set. **For criome's flow-graph M0, a small fixed enum is almost
certainly the right move.** The PROV-O qualified-relationship pattern (reify when binary isn't enough) is the
escape hatch.

### 1.7 Mermaid

Mermaid's choice of relation kinds is the closest direct precedent for criome's flow-graphs (the user explicitly
took inspiration from Mermaid). What Mermaid distinguishes by *diagram dialect*:

**Class diagrams** ([Mermaid class diagram syntax](https://mermaid.ai/open-source/syntax/classDiagram.html)):

| Mermaid syntax | UML kind |
|---|---|
| `<|--` | Inheritance |
| `*--` | Composition |
| `o--` | Aggregation |
| `-->` | Association |
| `..>` | Dependency |
| `<|..` | Realization |
| `--` | Link (solid, untyped) |
| `..` | Link (dashed, untyped) |

These six (plus two untyped fallbacks) map 1:1 to UML's six. They live at the **logical** layer — the
relationship *kind* changes the meaning, not just the styling.

**Flowcharts** ([Mermaid flowchart syntax](https://mermaid.ai/open-source/syntax/flowchart.html)):

| Syntax | Meaning |
|---|---|
| `-->` | Generic directed edge |
| `---` | Generic undirected edge |
| `==>` | Thick edge (cosmetic) |
| `-.->` | Dotted edge (cosmetic) |
| `--o` / `--x` | Circle / cross endpoints (cosmetic) |
| `\|text\|` | Edge label (free text) |

Flowcharts have **essentially one logical relation type** — directed-edge — plus visual variants that don't
carry semantic weight. The label is the only payload.

**Take-away**: criome's flow-graphs sit between Mermaid's two dialects. Architecture diagrams *are* mostly
flowchart-shaped (one generic relation kind suffices for many diagrams) — but architecture also benefits from
the class-diagram distinctions: "depends on" vs "contains" vs "implements" are *logically* different. The right
move for criome is probably "Mermaid flowchart's single-kind base + a small explicit relation-kind enum on top
when the user wants it" — not the full UML six, but more than the empty-string flowchart default.

### 1.8 Conceptual / ER modelling

ER modelling (Chen 1976 onward) gives the cleanest historical decomposition of relation properties:

- **Cardinality** — 1:1, 1:N, N:M, plus arbitrary `(min, max)` pairs ([Wikipedia: cardinality (data modeling)](https://en.wikipedia.org/wiki/Cardinality_(data_modeling)))
- **Optionality / participation** — must this entity participate in this relation? ("Total" vs "partial"
  participation in Chen's terms.) ([Engineering LibreTexts: relationship types](https://eng.libretexts.org/Courses/Delta_College/Introduction_to_Database_Systems/04:_Integrity_Rules_Constraints_and_Functional_Dependencies/4.03:_Relationship_Types))
- **Identifying vs non-identifying** — does the foreign key participate in the primary key of the dependent
  entity? Identifying makes the dependent's identity *include* the parent's — strong existence dependency
  (close cousin to UML composition). Non-identifying is a soft reference.
- **Degree** — binary, ternary, n-ary

ER notations make these choices visible at a glance — crow's-foot, ring/dash, etc. The substance of an ER
"relation type" is the *combination* of these dials.

**Take-away**: cardinality and optionality are first-class. Identifying-vs-non-identifying maps to UML
composition-vs-association. For criome v0.0.1 we can defer most of this — `Edge` is implicitly N:M (any node can
be the source or target of arbitrary edges) and there are no "must participate" constraints yet.

### 1.9 Hypergraphs / n-ary relations

When arity > 2, the binary-edge model breaks. Approaches:

- **Reification** — turn the n-ary relation into an entity with binary edges to each participant (PROV-O
  qualified terms; OWL pattern). Lossy in semantics; requires careful query rewriting.
- **Native hyperedges** — an edge connects ≥2 entities directly. ([Wikipedia: Hypergraph](https://en.wikipedia.org/wiki/Hypergraph))
- **Knowledge hypergraphs** — recent ML literature uses these for richer multi-entity reasoning.
  ([JMLR: Knowledge Hypergraph Embedding](https://www.jmlr.org/papers/volume24/22-063/22-063.pdf),
  [HyperGraphRAG](https://arxiv.org/html/2503.21322v1))

For architecture flow-graphs specifically, n-ary relations are rare — most edges are honestly binary
("criome calls lojix"). When they aren't, the reification pattern is sufficient: introduce an intermediate
"Junction" or "Coordination" node with binary edges to each participant.

**Take-away**: criome should not natively model hyperedges in v0.0.1. The reification pattern via subgraphs or
intermediate nodes is sufficient and consistent with how PROV-O extends.

### 1.10 Categorical / morphism-based models

Category theory frames a relation as a **morphism** with **domain** (source object) and **codomain** (target
object), composable when the codomain of one matches the domain of the next. ([Wikipedia: Morphism](https://en.wikipedia.org/wiki/Morphism),
[MIT 18.S996 Chapter 4](https://ocw.mit.edu/courses/18-s996-category-theory-for-scientists-spring-2013/b84f9d8840db0c745c75ab23c89851bb_MIT18_S996S13_chapter4.pdf))

For data, a **categorical schema** is a finite category: objects are entity types, morphisms are functional
relations between them, with composition expressing chains. Functorial data migration (David Spivak's work)
treats schema mappings as functors.

This is heavyweight for an MVP, but the framing reinforces:

- **Every relation has a directed (domain, codomain) pair** — not optional (RDF/OWL agree, Datomic agrees,
  Neo4j agrees).
- **Composition matters** — chain of edges is itself a meaningful relation. Path queries (Cypher's `*`,
  Datalog's recursive rules, OWL's transitivity) are how this surfaces in practice.
- **Identity morphism** per object — the implicit "self" relation. (Reflexive properties in OWL.)

**Take-away**: the directed-edge framing in `Edge { from, to }` is already morphism-shaped. Path / chain
reasoning is something to support eventually but not in M0.

---

## 2 · Cross-cutting patterns

Pulling out the shared patterns from §1:

### 2.1 Vocabulary openness

Three positions, with examples:

| Position | Examples | When it works |
|---|---|---|
| **Open** (any string / URI) | RDF, Cypher (pre-2026), early Datomic | Vocabularies are external & many; users own the namespace |
| **Closed enum** (fixed set) | UML (6), PROV-O starting points (~7), Mermaid class | The relation set is small and well-known in the domain |
| **User-asserted records** (closed-but-extensible: vocabulary is data) | Datomic attributes; OWL `owl:ObjectProperty` instances; Neo4j 2026 GRAPH TYPE; SHACL | The system is itself the source of truth — the vocabulary lives in the data |

Criome's "everything is a record" framing strongly nudges toward the third — **a `RelationKind` sema record
type that user assertions define, with edges holding a `Slot` reference to it**. That mirrors how `KindDecl`
will eventually describe record kinds. But for v0.0.1, the **closed enum** is cheaper, faster, and
empirically validated (PROV-O works fine for many people with a closed enum; Mermaid does too).

### 2.2 Where constraints live

Three locations:

| Constraint | Lives on | Examples |
|---|---|---|
| **Domain / range** (source-kind, target-kind) | Relation-type definition | OWL `rdfs:domain` / `rdfs:range`; Neo4j 2026 GRAPH TYPE |
| **Cardinality** (1:N, max-1, etc.) | Relation-type definition | Datomic `:db/cardinality`; SHACL `sh:maxCount`; UML multiplicity |
| **Inverse** (this relation pairs with that) | Relation-type definition | OWL `owl:inverseOf` |
| **Transitivity / symmetry / reflexivity** | Relation-type definition | OWL transitive/symmetric/reflexive properties |
| **Per-edge data** (timestamps, weights, qualifications) | The edge itself, or a reified "qualification" record | PROV-O qualified terms; Neo4j relationship properties |

The clean rule: **if it's the same for every edge of this kind, it lives on the kind. If it varies per edge,
it lives on the edge.** The cleanest way to honour this is to keep the `Edge` record minimal (`from`, `to`,
plus *just enough* type-info to dispatch validation), and put domain/range/cardinality on a separate
`RelationKind` record (or a Rust enum, for v0.0.1 closed-set).

In criome's pipeline, **domain/range/cardinality go in `invariants`** (per criome/ARCHITECTURE.md §3 the
validator runs `schema → refs → invariants → permissions → write → cascade`). Schema-step proves the record's
shape is well-formed; refs-step proves slot-refs resolve; invariants-step is where "this DependsOn edge's
source must be a Module-shaped node" lives. v0.0.1 can hardcode invariants; later they become `Rule` records.

### 2.3 Node typing — beyond `label`

UML stereotypes (`«interface»`, `«abstract»`), Mermaid class-diagram annotations, Neo4j multi-labels, OWL
classes — **every serious system gives nodes a *kind* beyond the human label.** A free-string `label` is
display; a *kind* drives validation.

For flow-graph nodes: do we want `Node { id, label, kind: NodeKind }` where `NodeKind` is e.g. `Daemon |
Library | Store | Subsystem | External | …`? This unlocks domain/range checks: a `DependsOn` edge from a
`Daemon` to a `Daemon` is fine; from a `Daemon` to a free-form text is not.

The minimal version (v0.0.1) might be no node-kinds at all — keep it as flexible as Mermaid flowchart, type
the *edges* only. Then introduce node-kinds when domain/range constraints become valuable. (The question is
explicit in decision points D3 below.)

### 2.4 What does Graph carry semantically beyond `title`?

The diagram-as-record question. Survey:

- **Mermaid** — diagram type tag (flowchart / class / sequence / state / ER / gantt / …) + direction (TD/LR/…)
  + theme. The diagram type is the load-bearing logical fact.
- **UML** — diagram name + diagram kind (class, component, deployment, sequence, …). Diagrams are *views* into
  a shared model, not standalone artefacts.
- **PROV-O** — provenance statements have no enclosing "diagram" concept; the graph is the union of all
  asserted triples scoped to a `prov:Bundle` if needed.
- **Neo4j** — no "graph" record per se; the graph IS the database. Subgraphs are query results.

For criome, `Graph` is a **named view** — a user-asserted bundle of nodes+edges with a title. What it might
also carry:

- **`purpose` or `scope`** — a free-text field describing what the diagram models (architecture, sequence,
  data flow, deployment topology). Useful for humans browsing sema, irrelevant to validation. *Defer.*
- **`layer` / `dialect`** — `Architecture | Sequence | StateMachine | DataFlow | …` analogous to Mermaid's
  diagram kind. Drives different validator invariants per kind. *Possibly worth M0.*
- **`participants` / `roles`** — sequence-diagram specific. *Defer until sequence diagrams are needed.*

The minimum-viable Graph stays at `{ title, nodes, edges, subgraphs }`. Adding `kind: GraphKind` could be
tomorrow's increment when more diagram dialects land.

---

## 3 · Recommendation for criome's M0 schema

### 3.1 Design constraints, restated

- **Logic only.** No styling, no rendering, no theme.
- **Validator-checked.** Whatever we put in the type must be validatable cheaply and deterministically.
- **Content-addressed.** Records are blake3-hashed; type changes are migrations later.
- **No macros.** All rkyv/serde derives are third-party-macro calls (allowed); we do not author macros.
- **Skeleton-as-design.** The Rust types are the schema for v0.0.1 (per signal/src/flow.rs's docstring).
- **Tractable for v0.0.1.** Future-friendly but not future-paying.
- **One artifact per repo.** `signal/src/flow.rs` is the only home for these.

### 3.2 Proposed schema — Option A (closed enum, recommended for M0)

```rust
// signal/src/flow.rs

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use crate::slot::Slot;

/// A node in a flow-graph.
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Serialize, Deserialize,
         Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node {
    pub id: String,
    pub label: String,
}

/// The kind of relation an edge represents — closed vocabulary for v0.0.1.
///
/// PROV-O-style discipline: a small fixed set covers the vast majority of
/// architecture-diagram modelling. New kinds are added by amending this enum
/// (a versioning event, with rkyv migration). When the closed set proves
/// insufficient, the path forward is `RelationKind::User(Slot)` pointing at
/// a user-asserted RelationKind record — but that's post-M0.
///
/// Inspired by UML class-diagram relations + PROV-O starting points + the
/// architecture-diagram patterns observable in sema-ecosystem reports.
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Serialize, Deserialize,
         Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationKind {
    /// Generic directed edge with no further commitment. The Mermaid-flowchart
    /// default. Use when no other variant fits and free-text label suffices.
    Flow,

    /// "A depends on B" — A's correct functioning requires B. UML dependency,
    /// Cargo dep, OpusDep in sema, every architecture diagram's bread and butter.
    DependsOn,

    /// "A contains B" with strong lifetime — B cannot exist without A.
    /// UML composition; Datomic :db/isComponent. e.g. a daemon contains
    /// internal actors.
    Contains,

    /// "A references B" with weak lifetime — B can exist independently.
    /// UML aggregation. e.g. a sema record references a lojix-store entry by
    /// hash.
    References,

    /// "A produces B" — A is the source, B the output. PROV-O wasGeneratedBy
    /// (with arrow inverted to source→target convention).
    Produces,

    /// "A consumes B" — A reads/uses B as input. PROV-O used.
    Consumes,

    /// "A calls B" — control-flow / RPC / function call. Distinct from
    /// DependsOn (which is structural) — a daemon depends on a library statically
    /// but calls another daemon dynamically.
    Calls,

    /// "A implements B" — A satisfies the contract specified by B.
    /// UML realization. e.g. nexus-cli implements the client-msg protocol.
    Implements,

    /// "A is-a B" — A is a specialisation/subtype of B. UML generalization.
    IsA,
}

/// A directed edge from one node to another, typed by RelationKind.
///
/// `label` remains for free-text annotation (mirroring Mermaid's `|text|`).
/// The kind drives validation (domain/range checks live in validator's
/// `invariants` step); the label is human-readable colour.
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Serialize, Deserialize,
         Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: Slot,
    pub to: Slot,
    pub kind: RelationKind,
    pub label: Option<String>,
}

/// A flow-graph: a titled bundle of nodes and edges.
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Serialize, Deserialize,
         Debug, Clone, PartialEq, Eq, Hash)]
pub struct Graph {
    pub title: String,
    pub nodes: Vec<Slot>,
    pub edges: Vec<Slot>,
    pub subgraphs: Vec<Slot>,
}

/// The kind names criomed accepts at v0.0.1.
pub const KNOWN_KINDS: &[&str] = &["Node", "Edge", "Graph"];
```

**Rationale**:

- **Nine variants is not arbitrary** — they map onto observed dialects: PROV-O (`Produces`, `Consumes`), UML
  (`Contains`, `References`, `Implements`, `IsA`), Cargo/Mermaid (`DependsOn`, `Flow`), control-flow
  diagrams (`Calls`).
- **`Flow` is the escape hatch** — when the user doesn't know or care, `Flow` + a label works exactly like
  Mermaid flowchart's generic edge. Migration path for any current `Edge { label: Some(s) }` is to set
  `kind: Flow` and keep the label.
- **The label stays.** Free-text annotation is genuinely useful and orthogonal to type — a `DependsOn` edge
  with label `"only at compile time"` is still a depends-on, just qualified.
- **No domain/range constraints in the type** — those are validator invariants (post-M0 if needed). For M0,
  any `Node` can be the source or target of any kind; if Li wants stricter, that's a specific decision (D3
  below).
- **No cardinality constraints in the type** — graph-level invariants ("a node has at most one `IsA` parent")
  belong to the validator's invariant step, not to the record shape.
- **No `inverse` annotation** — keep it simple; if someone writes both `(A -DependsOn-> B)` and `(B -ProvidesFor-> A)`,
  that's redundancy the validator can warn on later. v0.0.1 doesn't auto-derive inverses.

### 3.3 Proposed schema — Option B (string-typed, Neo4j-style)

```rust
pub struct Edge {
    pub from: Slot,
    pub to: Slot,
    pub relation_type: String,   // open vocabulary, conventional names
    pub label: Option<String>,
}
```

**Argument for**: maximum flexibility, deferred commitment, matches Cypher's pre-2026 model. Everything in the
ecosystem already revolves around strings (slot display-names, kind names…) so this is consistent.

**Argument against**: no compile-time safety; every consumer string-matches; typos like `"depnds_on"` slip
through the validator unless we add a registry; loses the PROV-O lesson that small fixed sets work.

### 3.4 Proposed schema — Option C (slot-ref to RelationKind record, Datomic-style)

```rust
pub struct Edge {
    pub from: Slot,
    pub to: Slot,
    pub kind: Slot,   // -> RelationKind record
    pub label: Option<String>,
}

pub struct RelationKind {
    pub name: String,
    pub source_kind: Option<Slot>,    // -> NodeKind (when typed)
    pub target_kind: Option<Slot>,
    pub cardinality: Cardinality,
    pub inverse: Option<Slot>,        // -> RelationKind
    pub is_transitive: bool,
    pub is_symmetric: bool,
}

pub enum Cardinality { OneToOne, OneToMany, ManyToOne, ManyToMany }
```

**Argument for**: this IS the eventual destination — relation kinds become first-class records, queryable,
versionable, content-addressed; new vocabulary lands as data without a schema migration. Mirrors Datomic and
OWL.

**Argument against**: the schema-of-schema tar-pit — needs `RelationKind` itself to bootstrap, which means
genesis has to assert the basic relation kinds before any Edge can land. Deferred per [reports/078 §M0
"machina + KindDecl/FieldSpec/etc. wait until the engine has been used to design them via flow-graph records
first"](078-implementation-roadmap-from-external-research.md).

### 3.5 Migration path between options

These are not mutually exclusive over time:

```
Today          M0           ?              future
-----          --           -              ------
Edge.label  →  Option A  →  Option A   →  Option C
(string)       (enum +       + a User(Slot)   (RelationKind
               label)        escape variant)   records,
                                               KindDecl-shaped)
```

Going **A → C** is a straightforward rkyv migration (the variant `User(Slot)` lands on the enum first;
existing closed-set edges keep working). Going **A → B** (enum → string) is conceptually a *retreat* — losing
the type discipline — and probably never wanted.

### 3.6 Proposed validator invariants (M3, not M0)

When the validator's `invariants` step is fleshed out (post-M0 per [reports/078 §M3](078-implementation-roadmap-from-external-research.md):
"`criome/src/validator/invariants.rs` body: stays `todo!()` for Stage A"), candidate flow-graph invariants
based on the surveyed systems:

- **`Edge.from` and `Edge.to` resolve to `Node` records** (refs-step already catches missing slots; this
  proves the *kind* of the slot's binding).
- **No self-loops for `IsA`** (irreflexive — every type is its own super-type, so the explicit edge is
  redundant; OWL's irreflexive constraint).
- **No cycles in `IsA`** (acyclic). Cheap DFS at write time.
- **No cycles in `Contains`** (composition can't be circular). Cheap DFS.
- **`Implements` target should be a node intended-as-interface** — needs node-kinds (decision point D3).
- **Subgraph edges reference nodes within the graph or its ancestors** — scoping invariant.

These are five-line checks each; not the bottleneck.

### 3.7 What to defer explicitly

Cut to keep v0.0.1 tractable:

1. **Subproperty hierarchies.** OWL's `rdfs:subPropertyOf`. Useful, but adds a transitive-closure check at
   query time. Defer until there's demand.
2. **Inverse-property axioms.** `owl:inverseOf`. Useful for query rewriting; not load-bearing for asserting +
   storing.
3. **Transitive / symmetric / reflexive flags on relation types.** Defer; a few specific transitive rules can
   be hardcoded.
4. **Cardinality constraints encoded in records.** SHACL `sh:minCount` / `sh:maxCount`. Defer; M0 has no
   cardinality-checked invariants.
5. **Hyperedges / n-ary relations.** Reify via subgraphs or junction nodes if the need arises; do not extend
   `Edge`.
6. **Node kinds.** See decision point D3 — possibly defer entirely from M0.
7. **Graph dialect tag (`Architecture | Sequence | …`).** Single dialect (architecture flow) for M0; expand
   when sequence diagrams or state machines come in.
8. **Open vocabulary escape (`RelationKind::User(Slot)`).** Land it when the closed set bites; not before.
9. **Edge-on-edge** ("this edge is annotated by that edge"). Almost never needed for architecture diagrams;
   reify if needed.
10. **Per-edge timestamps / authorship.** Already covered by the per-kind `ChangeLogEntry` (per
    criome/ARCHITECTURE.md §5); does not need to live on `Edge`.

---

## 4 · Decision points for Li

These are the specific A/B/C choices to land before M0 ships. Each has a default lean noted, but the call is
Li's.

### D1 — Open vs closed vocabulary for relation type

**Question**: should `Edge` carry an open-string `relation_type`, a closed Rust enum, or a slot-ref to a
user-asserted record?

- **Option A** (closed Rust enum, ~9 variants — `Flow`, `DependsOn`, `Contains`, `References`, `Produces`,
  `Consumes`, `Calls`, `Implements`, `IsA`). PROV-O / UML / Mermaid-class style. *Lean — simplest, most
  validator-friendly, fastest to ship; covers every diagram in the corpus that currently uses
  `Edge.label`.*
- **Option B** (open string `relation_type: String`). Cypher pre-2026 / RDF-without-OWL. Most flexible, no
  compile-time discipline.
- **Option C** (`relation_type: Slot` → user-asserted `RelationKind` record). Datomic / OWL / Neo4j-2026 style.
  Most powerful, but pulls schema-of-schema forward into M0 — currently deferred per reports/078 §M0.

### D2 — Field-as-relation vs explicit Edge record

**Question**: when modelling "criome depends on lojix-schema", is the natural shape (i) an `Edge` record
linking the two nodes, or (ii) a `dependencies: Vec<Slot>` field on the `Daemon` record itself (Datomic-style:
the field IS the relation)?

- **Option A** (explicit Edge records — current shape). Generic, queryable uniformly, matches Mermaid /
  Neo4j / RDF. *Lean — already what flow-graph M0 commits to; consistent with arch-diagrams-as-data.*
- **Option B** (relation-as-field). More Datomic-flavoured; more compact for "every node has these fields";
  loses uniformity (each node-kind would have its own relation-fields, no generic Edge query).
- **Option C** (both: keep Edge as the generic substrate, but allow specific record kinds to also carry
  ref-fields when convenient). Pragmatic — but adds complexity now.

### D3 — Node typing beyond label

**Question**: should `Node` also carry a `kind` field (e.g. `NodeKind::{Daemon, Library, Store, Subsystem,
External, Concept}`) so domain/range constraints on edges become checkable?

- **Option A** (no node-kind in M0; `Node { id, label }` stays as-is). Mermaid-flowchart minimalism. Most
  flexible, defers domain/range until they prove valuable. *Lean — matches "tractable for v0.0.1"; can add
  later as a non-breaking field if rkyv migration is handled.*
- **Option B** (closed `NodeKind` enum from day one; ~6-8 variants for architecture diagrams). Enables
  validator invariants like "DependsOn must go between two Daemons or a Daemon and a Library". UML stereotype
  / OWL class style.
- **Option C** (free-string `node_kind: Option<String>`). Cheapest middle ground; no validator help but
  human-readable.

### D4 — Free-text label retention on Edge

**Question**: with `kind` carrying typed semantics, does the optional `label: Option<String>` still earn its
keep?

- **Option A** (keep `label: Option<String>` alongside `kind`). Free-text qualification; e.g. a `DependsOn`
  with label `"compile-time only"`. Mermaid + OWL annotation properties both go this way. *Lean — orthogonal
  to type; cheap.*
- **Option B** (drop label; rely entirely on kind). Forces every distinction into the type system; cleaner
  but less human-friendly.
- **Option C** (replace label with a structured `qualifier: Option<EdgeQualifier>` enum like `OnlyAtCompileTime
  | Async | Bidirectional | …`). PROV-O qualified-relation flavour. Probably premature for M0.

### D5 — Graph-kind tag

**Question**: should `Graph` carry a `kind` field distinguishing dialects (architecture flow vs sequence
diagram vs state machine vs data flow)?

- **Option A** (no kind in M0; everything is an architecture flow-graph implicitly). Single-dialect
  simplicity. *Lean — matches reports/078 §M0 "first criomed should handle flow-graph records"; expand when
  more dialects land.*
- **Option B** (closed `GraphKind` enum from day one — `Architecture | Sequence | StateMachine | DataFlow |
  EntityRelationship`). Mermaid-style multi-dialect. Adds validator-invariant divergence per kind.
- **Option C** (free-string `dialect: Option<String>`). Cheapest middle ground.

### D6 — Where `RelationKind` lives in the codebase

If D1 picks **Option A** (closed Rust enum), the enum lives in `signal/src/flow.rs` next to `Edge` —
straightforward. If D1 picks **Option C** (slot-ref to record), where does the `RelationKind` *record kind*
declaration go?

- **Option A** (`signal/src/flow.rs` adds a `RelationKind` struct; `KNOWN_KINDS` becomes
  `["Node", "Edge", "Graph", "RelationKind"]`). Self-contained in the flow-graph subset. *Lean if D1 picks
  C.*
- **Option B** (`signal/src/kind.rs` per the deferred schema-of-schema design from reports/078 §2). Pulls
  schema-of-schema partially forward — possibly the right move, but a larger commit.
- **Option C** (separate crate `signal-relations`). Probably overkill.

---

## 5 · Open questions surfaced (for later, not blocking M0)

- **Q-R1: Inverse edges.** When `(A -DependsOn-> B)` is asserted, is there an implicit inverse edge
  `(B -DependedOnBy-> A)` queryable? RDF/OWL says yes (with `owl:inverseOf`); Neo4j requires explicit traversal
  direction. Lean: queryable both directions natively (the `from`/`to` slots are both indexed); no implicit
  reverse edge created. Defer.
- **Q-R2: Bidirectional edges.** `Edge` is directed (from→to). Bidirectional relations (UML association,
  Mermaid `<-->`) — how represent? Two edges; or a single edge with `bidirectional: bool`. Lean: two edges;
  matches RDF/Datomic. Defer.
- **Q-R3: Multi-graphs.** Can two edges with the same `(from, to, kind)` exist? Lean: yes (different `Slot`
  identities; ChangeLogEntry differentiates). The validator can warn but not reject by default. Defer.
- **Q-R4: Edge-on-edge / qualified relations.** PROV-O reifies via `prov:Usage`. For criome, the natural
  shape is making the `Edge` itself a slot you can reference from another `Annotation` record. Defer.

---

## 6 · Summary

The dominant pattern across ten surveyed systems is: **edges carry one type, the type drives validation, and
constraints (domain, range, cardinality, inverse) live on the type definition rather than per-edge.** Open
vocabulary is the historical default (RDF, Cypher) but the recent direction (Neo4j 2026, TerminusDB, Datomic,
OWL-with-discipline, PROV-O practice) is toward a closed-but-extensible type set with constraints in the
schema layer.

For criome's M0 — which exists to start designing the project's own architecture *as data in sema* — the lean
recommendation is **Option A** of §3.2: a closed Rust enum `RelationKind` with ~9 variants drawn from
PROV-O + UML + Mermaid-class precedent, keeping the free-text `label` for human-readable qualification, with
domain/range invariants deferred to the validator's `invariants` step (already a no-op for v0.0.1 per
reports/078 §M3) and node-kinds + graph-kinds deferred to later milestones.

The decision points (§4 D1-D6) are the explicit forks. The rest of the design follows once those land.

---

## Sources

Primary sources consulted:

- [Neo4j Cypher Manual: Constraints](https://neo4j.com/docs/cypher-manual/current/schema/constraints/)
- [Neo4j Cypher Manual: Graph Types](https://neo4j.com/docs/cypher-manual/current/schema/graph-types/set-graph-types/)
- [Neo4j Blog: GRAPH TYPE schema enforcement (Preview, 2026.02)](https://neo4j.com/blog/developer/graph-type-schema-enforcement-made-easy-preview/)
- [Neo4j Graph Database Concepts](https://neo4j.com/docs/getting-started/appendix/graphdb-concepts/)
- [W3C: OWL Web Ontology Language Reference](https://www.w3.org/TR/owl-ref/)
- [W3C: OWL 2 Web Ontology Language Quick Reference Guide](https://www.w3.org/2007/OWL/refcardLetter)
- [W3C: RDF 1.1 Concepts and Abstract Syntax](https://www.w3.org/TR/rdf11-concepts/)
- [W3C: PROV-O — The PROV Ontology](https://www.w3.org/TR/prov-o/)
- [W3C: Defining N-ary Relations on the Semantic Web](https://www.w3.org/TR/swbp-n-aryRelations/)
- [W3C: SHACL — Shapes Constraint Language](https://www.w3.org/TR/shacl/)
- [Wikipedia: Semantic triple](https://en.wikipedia.org/wiki/Semantic_triple)
- [Wikipedia: Class diagram](https://en.wikipedia.org/wiki/Class_diagram)
- [Wikipedia: Stereotype (UML)](https://en.wikipedia.org/wiki/Stereotype_(UML))
- [Wikipedia: Hypergraph](https://en.wikipedia.org/wiki/Hypergraph)
- [Wikipedia: Morphism](https://en.wikipedia.org/wiki/Morphism)
- [Wikipedia: Cardinality (data modeling)](https://en.wikipedia.org/wiki/Cardinality_(data_modeling))
- [Datomic: Schema Reference](https://docs.datomic.com/schema/schema-reference.html)
- [Datomic: Schema Modeling](https://docs.datomic.com/schema/schema-modeling.html)
- [Datomic Blog: Component Entities](https://blog.datomic.com/2013/06/component-entities.html)
- [TerminusDB: Schema Reference Guide](https://terminusdb.org/docs/schema-reference-guide/)
- [DFRNT: OWL/SHACL/SPARQL vs TerminusDB tradeoffs](https://dfrnt.com/blog/2023-06-10-exploring-tradeoffs-rdf-owl-sparql-shacl-terminusdb-make-an-informed-decision)
- [Mermaid: Class diagrams](https://mermaid.ai/open-source/syntax/classDiagram.html)
- [Mermaid: Flowchart syntax](https://mermaid.ai/open-source/syntax/flowchart.html)
- [Visual Paradigm: UML Aggregation vs Composition](https://www.visual-paradigm.com/guide/uml-unified-modeling-language/uml-aggregation-vs-composition/)
- [UMLBoard: UML Relationships](https://www.umlboard.com/docs/relations/)
- [Engineering LibreTexts: Relationship Types](https://eng.libretexts.org/Courses/Delta_College/Introduction_to_Database_Systems/04:_Integrity_Rules_Constraints_and_Functional_Dependencies/4.03:_Relationship_Types)
- [Owlready2: Properties](https://owlready2.readthedocs.io/en/latest/properties.html)
- [JMLR: Knowledge Hypergraph Embedding (vol. 24)](https://www.jmlr.org/papers/volume24/22-063/22-063.pdf)
- [arXiv: HyperGraphRAG](https://arxiv.org/html/2503.21322v1)
- [MIT 18.S996 Category Theory for Scientists, Chapter 4](https://ocw.mit.edu/courses/18-s996-category-theory-for-scientists-spring-2013/b84f9d8840db0c745c75ab23c89851bb_MIT18_S996S13_chapter4.pdf)
- [CEUR: Guiding Reification in OWL through Aggregation](https://ceur-ws.org/Vol-573/paper_19.pdf)
- [W3C DXWG wiki: Qualified relations](https://github.com/w3c/dxwg/wiki/Qualified-relations)

---

*End report 079.*
