# Sema-ecosystem architecture

*Living document · last revision 2026-04-24 · canonical reference for the engine's shape*

---

## Scope rule (READ FIRST)

This file is **high-level concepts only**. Three layers of
documentation, strictly separated:

| Where | What | Example |
|---|---|---|
| `docs/architecture.md` | **Prose + diagrams only.** No code. High-level shape, invariants, relationships, rules. | "criomed owns sema; lojix-stored owns lojix-store; text crosses only at nexusd" |
| `reports/NNN-*.md` | **Concrete shapes + decision records.** Type sketches, record definitions, message enums, research syntheses, historical context. | `Opus { … }` full rkyv sketch |
| the repos themselves | **Implementation.** Rust code, tests, flakes, Cargo.toml. | `nexus-schema/src/opus.rs` |

**If a doc-layer rule is violated**, rewrite: move type sketches
out of `docs/architecture.md` into a report; move runnable code
out of reports into the appropriate repo. This file stays slim
so it remains readable in one pass.

When architecture changes, update this file first, then write a
new report explaining the change. Don't edit old reports —
they're decision-journey records.

---

## 1 · The engine in one paragraph

The sema ecosystem is a **records database** (`sema`, owned by
`criomed`) paired with a **content-addressed blob store**
(`lojix-store`, owned by `lojix-stored`), fronted by a **thin
nexus-text messenger** (`nexusd`) and served by a **compile
daemon** (`forged`). Humans and LLMs communicate in nexus text;
everything internal to the engine is rkyv. The MVP target is
**self-hosting**: the engine's own source lives as records in
sema; forged projects those records to Rust source and compiles
them; the resulting binary can re-edit its own records.

---

## 2 · The four daemons

```
     nexus text (humans, LLMs, nexus-cli)
        ▲ │
        │ ▼
     ┌─────────┐
     │ nexusd  │ messenger: text ↔ rkyv only; validates syntax +
     │         │ protocol version; forwards to criomed; serialises
     │         │ replies back to text. Stateless modulo in-flight
     │         │ correlations.
     └────┬────┘
          │ rkyv (criome-msg contract)
          ▼
     ┌─────────┐
     │ criomed │ guardian of sema-db; overlord of lojix-store.
     │         │ • owns the records database
     │         │ • runs pattern resolvers against sema snapshots
     │         │ • fires subscriptions on commits
     │         │ • dispatches compile requests to forged
     │         │ • signs capability tokens forged uses to Put
     │         │   directly into lojix-store
     │         │ • tracks what's in lojix-store (so it can direct
     │         │   GC); never moves binary bytes itself
     └────┬────┴────────────────────┐
          │                         │
  rkyv (compile-msg)     rkyv (lojix-store-msg)
          ▼                         ▼
     ┌─────────┐               ┌──────────────┐
     │ forged  │───capability  │ lojix-stored │
     │         │──token────────►              │
     │         │               │              │
     │ compile │ blake3 → bytes│    blob      │
     │ daemon  │               │   guardian   │
     │         │ • pulls records from criomed (read)
     │         │ • calls rsc (pure projection lib)
     │         │ • invokes cargo/rustc
     │         │ • Puts binary directly to lojix-stored via token
     │         │ • replies { binary-hash } to criomed
     └─────────┘
```

**Invariants**:

- Text crosses only at nexusd's boundary.
- No daemon-to-daemon path routes bulk data through criomed —
  forged and lojix-stored are connected directly, authorised by
  a criomed-signed capability token.
- Criomed never sees compiled binary bytes; it only records
  their hashes in sema.
- There is no `Launch` protocol message. Binaries are
  materialised to filesystem paths (nix-store style); you run
  them from a shell.

---

## 3 · The two stores

### sema — records database

- **Owner**: criomed.
- **Backend**: content-addressed records, keyed by the blake3
  of their canonical rkyv encoding. Storage engine is an
  embedded redb.
- **Holds**: every structural record (Struct, Enum, Module,
  Program, Opus, Derivation, Type, Origin, traits, …).
- **Writes**: single-writer through criomed's internal writer
  actor.
- **Reads**: parallel, MVCC semantics from the storage engine.
- **Identity of a workspace opus** tracked in a name→root-hash
  table (git-refs analogue).

### lojix-store — content-addressed blobs

- **Owner**: lojix-stored.
- **Backend**: append-only file plus a rebuildable hash-to-
  offset index.
- **Holds**: opaque bytes only (compiled binaries; user file
  attachments referenced by sema records; anything too large or
  too unstructured to belong in sema).
- **No typing**. No kind bytes. The type of a blob is known
  only through the sema record that references its hash.
- **Access control**: capability tokens, signed by criomed.

### Relationship

Sema records carry content-hash fields that reference blobs in
lojix-store. "Record says hash H; fetch H from lojix-stored."
Criomed keeps a reachability view (what hashes are live) and
can direct garbage collection; it never handles the bytes
themselves.

---

## 4 · Repo layout

18 code repos plus `tools-documentation` (docs).

- **Layer 0 — text grammars** — nota (spec), nota-serde-core
  (shared lexer+ser+de kernel), nota-serde (façade), nexus
  (spec), nexus-serde (façade).
- **Layer 1 — schema** — nexus-schema (record types, pattern
  types, contract-message types; the rkyv lingua franca).
- **Layer 2 — contract crates** — one per daemon↔daemon
  relation: criome-msg (nexusd↔criomed), compile-msg
  (criomed↔forged), lojix-store-msg (store traffic).
- **Layer 3 — storage** — sema, lojix-store.
- **Layer 4 — daemons** — criomed, forged, lojix-stored,
  nexusd.
- **Layer 5 — clients + build helpers** — nexus-cli
  (flag-less CLI), rsc (pure records-to-source library),
  horizond (CriomOS deploy CLI — currently named `lojix`, rename
  pending per [reports/018](../reports/018-lojix-positioning.md);
  reads cluster proposals, projects horizons via horizon-lib,
  invokes `nixos-rebuild`).

**Shelved**: `arbor` (prolly-tree versioning) — post-MVP.

Concrete record types and message enums live in
[reports/017](../reports/017-architecture-refinements.md) and
later reports. This file names the components; it does not
define their shapes.

---

## 5 · Key type families (named, not specified)

- **Opus** — a pure-Rust artifact specification. Nix-like and
  extremely explicit: toolchain pinned by derivation reference,
  outputs enumerated (bin / lib / both), features as plain
  strings, every build-affecting input a field so the record's
  hash captures the full closure.
- **Derivation** — the escape hatch for non-pure deps. Wraps a
  nix flake output (or, rarely, an inline nix expression) with
  a content-hash and named outputs (`out`, `lib`, `dev`, `bin`).
- **OpusDep** — an opus references either another opus
  (recursive Rust build) or a derivation (system lib, tool)
  with a link specification describing how cargo/rustc should
  consume the derivation's outputs.
- **RawPattern** — the wire form of a nexus pattern, carrying
  user-facing names (`StructName`, `FieldName`, `BindName`).
  Appears on criome-msg; never used inside criomed after
  resolution.
- **PatternExpr** — the resolved form, carrying schema IDs
  (`StructId`, `FieldId`). Pinned to a specific sema snapshot.
  Internal to criomed.
- **CriomeRequest / CriomeReply** — the nexusd↔criomed
  protocol verbs (lookup, query, assert, mutate, subscribe,
  compile, …).
- **CompileRequest / CompileReply** — the criomed↔forged
  protocol, carrying opus identity, sema snapshot, and a
  capability token.
- **LojixStoreRequest / LojixStoreReply** — put/get/contains,
  plus streaming variants for large blobs.

Concrete field lists live in
[reports/017 §1, §2](../reports/017-architecture-refinements.md)
and subsequent reports. If a type below needs to grow, update
its report (or write a new one); don't inline the shape here.

---

## 6 · Data flow

### Single query

```
 human nexus text
        ▼
  nexusd: lex + parse → RawPattern
        │ rkyv criome-msg (Query { pattern })
        ▼
  criomed: resolver(RawPattern, sema_snapshot) → PatternExpr
        │ matcher runs against records
        ▼
  rkyv reply (Records)
        ▼
  nexusd: serialize → nexus text
        ▼
 human
```

### Compile + self-host loop

```
 human: (Compile (Opus nexusd))
        ▼
 nexusd → criomed → forged (with capability token)
        │
        ▼ forged pulls records from criomed
        ▼ rsc projects records → in-memory crate
        ▼ cargo build
        ▼ Put binary bytes → lojix-stored (direct, via token)
        ▼ reply { binary-hash } → criomed
        ▼ criomed asserts a CompiledBinary record in sema
        ▼ reply flows back to human
 human: materialise binary to a path (nix-style), run from shell
        ▼ running binary connects back to nexusd
        ▼ Asserts new records
        ▼ next compile produces a different binary — LOOP CLOSES
```

---

## 7 · Grammar shape

Nota is a strict subset of nexus. A single lexer (in
nota-serde-core) handles both, gated by a dialect knob. The
grammar is organised as a **delimiter-family matrix** (see
[reports/013](../reports/013-nexus-syntax-proposal.md)):

- Outer character picks the family — records `( )`, composites
  `{ }`, evaluation `[ ]`, flow `< >`.
- Pipe count inside picks the abstraction level — none for
  concrete, one for abstracted/pattern, two for
  committed/scoped.

**Sigil budget is closed.** Six total: `;;` (comment), `#`
(byte-literal prefix), `~` (mutate), `@` (bind), `!` (negate),
`=` (bind-alias, narrow use). New features land as delimiter-
matrix slots or Pascal-named records — **never new sigils**.

---

## 8 · Project-wide rules

Foundational rules observed across sessions.

- **No ETAs.** Don't estimate time to complete work. Describe
  the work; don't schedule it.
- **No backward compat.** The engine is being born. Rename,
  move, restructure freely. Applies until Li declares a
  compatibility boundary.
- **Text only crosses nexusd.** Every internal daemon-to-daemon
  message is rkyv.
- **Schema is the documentation.** Patterns and types resolve
  against sema; hallucinated names are rejected early.
- **Criomed is the overlord.** Bulk data can flow directly
  between forged and lojix-stored, but criomed authorises it
  via capability tokens and retains the reachability view.
- **A binary is just a path.** No `Launch` message;
  materialisation is filesystem.
- **Sigils as last resort.** New features land in the matrix
  or as records. The sigil budget is frozen.
- **One artifact per repo.** rust/style.md rule 1.
- **Content-addressing is non-negotiable.** Record identity is
  the blake3 of its canonical encoding. Don't add mutable
  fields that would break identity.

---

## 9 · Reading order for a new session

1. **This file** — the canonical shape.
2. [reports/018](../reports/018-lojix-positioning.md) —
   where horizond (currently `lojix`) fits; rename rationale.
3. [reports/017](../reports/017-architecture-refinements.md) —
   latest refinements (Opus/Derivation, schema-bound patterns,
   no-Launch, no-kind-bytes, tokens).
3. [reports/013](../reports/013-nexus-syntax-proposal.md) —
   delimiter-family matrix (grammar canon).
4. [reports/015](../reports/015-architecture-landscape.md) v4 —
   full architecture synthesis (parts superseded by 017 — read
   after 017 so you know what's current).
5. [reports/016](../reports/016-tier-b-decisions.md) — open
   questions (most answered by 017).
6. `reports/014` — serde-refactor history.
7. `reports/004`, `reports/009-binds-and-patterns` — technical
   references.

Older reports have been deleted to prevent context poisoning.

---

## 10 · Update policy

When architecture changes:

1. Update this file first. Keep it prose + diagrams only.
2. Write a new report (`reports/NNN-whatever.md`) describing
   the decision, the alternatives considered, and any concrete
   shapes (types, enums).
3. Update implementation in the affected repos.

If an old report is superseded, **don't edit it** — it stays
as a decision-journey record. The current shape is wherever
this file points.

---

*End docs/architecture.md.*
