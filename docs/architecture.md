# Sema-ecosystem architecture

*Canonical reference for the engine's shape. Edited with extreme care.*

---

## Scope rule (READ FIRST)

This file is **high-level concepts only**. Three layers of
documentation, strictly separated:

| Where | What | Example |
|---|---|---|
| `docs/architecture.md` | **Prose + diagrams only.** No code. High-level shape, invariants, relationships, rules. | "criomed owns sema; lojixd owns lojix-store; text crosses only at nexusd" |
| `reports/NNN-*.md` | **Concrete shapes + decision records.** Type sketches, record definitions, message enums, research syntheses, historical context. | `Opus { … }` full rkyv sketch |
| the repos themselves | **Implementation.** Rust code, tests, flakes, Cargo.toml. | `nexus-schema/src/opus.rs` |

If a doc-layer rule is violated, rewrite: move type sketches
out of `docs/architecture.md` into a report; move runnable code
out of reports into the appropriate repo. This file stays slim
so it remains readable in one pass.

When architecture changes, update this file first, then write a
new report describing the change. Per Li's rule ("delete wrong
reports, don't banner them"), superseded reports are deleted —
they do not stay as banner-wrapped relics.

---

## 1 · The engine in one paragraph

**Sema is all we are concerned with.** Sema is the records —
the canonical, content-addressed, evaluated state of the
engine. Every concept the engine reasons about (code, schema,
rules, plans, authz, history, world data) is expressed as
records in sema. The records are stored in rkyv, content-
addressed by blake3. The rest of the engine exists to serve
sema:

- **criomed** is sema's engine. It receives every request,
  validates it (schema, references, permissions, invariants),
  and applies the change to sema. Rules and derivations are
  themselves records; cascades settle inside sema. Nothing
  "lives above" sema holding derived values.
- **nexusd** is the translator. Nexus is a text request
  language — structured, controlled, permissioned — used
  because humans and LLMs can't hand-type rkyv. nexusd parses
  nexus text into `criome-msg` rkyv envelopes (`Assert`,
  `Mutate`, `Retract`, `Query`, `Compile`, …) and serialises
  replies back.
- **lojixd** is the hands. It performs effects sema can't
  (spawning `nix` subprocesses; reading and writing
  filesystem paths; materialising files). Inputs are plan
  records read from sema; outputs become outcome records
  written back.
- **rsc** projects sema → `.rs` + `Cargo.toml` + `flake.nix`
  for nix to consume. One-way emission.
- **lojix-store** is a content-addressed filesystem (nix-store
  analogue) holding real unix files, referenced from sema by
  hash. Canonical from day one — see §5 for how it relates to
  `/nix/store` during the bootstrap era.

**Build backend for this era**: **nix via crane + fenix**.
fenix pins the Rust toolchain; crane builds packages. rsc
emits the workdir that these consume. Direct `rustc`
orchestration is a post-nix-replacement concern.

**Macro philosophy**: we **author no macros** ourselves (no
`macro_rules!`, no proc-macro crates). Our internal code-gen
patterns live as sema rules that run before rsc emission. We
**freely call** third-party macros — `#[derive(Serialize)]`,
`#[tokio::main]`, `format!`, `println!`, etc. — and rsc emits
those invocations verbatim for rustc to expand.

**The code category in sema is named *machina*** — the subset
of records that compiles to Rust in v1. The native checker
over machina records is *machina-chk*. World-fact records,
operational-state records, and authz records are separate
categories.

**Bootstrap is rung by rung.** The engine bootstraps using
its own primitives starting from rung 0. There is no "before
the engine runs" mode; criomed runs from the first instant,
sema starts empty, nexus messages populate it. Each rung's
capability comes from the data already loaded; that
capability is what populates the next rung. See §10.

---

## 2 · Three invariants

These are load-bearing. Everything downstream depends on them.

### Invariant A — Rust is only an output

Sema changes **only** in response to nexus requests. There is
**no** `.rs` → sema parsing path. No ingester. rsc projects
sema → `.rs` one-way for rustc/cargo; nothing in the engine
ever reads that text back. External tools may do whatever they
want in user-space, but only nexus requests reach the engine.

### Invariant B — Nexus is a language, not a record format

Sema is rkyv (binary, content-addressed). **Nexus is a request
language** (text) used to talk to criomed. Parsing nexus
produces `criome-msg` rkyv envelopes; it does not produce sema
directly. There are no "nexus records." There is sema (rkyv),
and there are nexus messages (text requests). The analogy is
SQL-and-a-DB: SQL is a request language; stored rows are in
the DB's on-disk format. No one calls a row a "SQL record."

### Invariant C — Sema is the concern; everything orbits

If a component does not serve sema directly, it is not core.
criomed = sema's engine / guardian. nexusd = sema's
text-request translator. lojixd = executor for effects sema
can't perform directly — outcomes return as sema. rsc = sema →
`.rs` projector. lojix-store = artifact files, referenced
*from* sema.

---

## 3 · The request flow

```
  user writes nexus text
      │
      ▼
  nexusd ─────── parses text → criome-msg (rkyv)
      │           (CriomeRequest::Assert / Mutate / Retract /
      │            Query / Compile / Subscribe / …)
      ▼
  criomed ─────── validates:
      │            • schema conformance
      │            • reference resolution (slot-refs exist)
      │            • invariant preservation (Rule records with `is_must_hold`)
      │            • authorization (capability tokens; BLS quorum post-MVP)
      │
      │          if valid → apply to sema; otherwise → reject
      │
      ▼
  criomed replies via criome-msg rkyv
      │
      ▼
  nexusd ─────── rkyv → nexus text
      │
      ▼
  user reads reply
```

**Every edit is a request.** criomed is the arbiter; assertions,
mutations, retractions can all be rejected. This is the
hallucination wall: unknown names, broken references,
schema-invalid shapes, unauthorised actions all fail here.

**Genesis runs the same flow.** At first boot, criomed
dispatches a `genesis.nexus` text file (shipping with the
criomed binary) through the same path: nexusd parses it,
criome-msg envelopes flow to criomed, the validator runs,
records land in sema. The first messages validate against
the built-in Rust types in `criome-schema` (no in-sema
KindDecls yet); subsequent ones validate against records
the genesis stream has already asserted. Once the
`SemaGenesis` marker lands, normal mode begins.

---

## 4 · The three daemons (expanded)

```
     nexus text (humans, LLMs, nexus-cli)
        ▲ │
        │ ▼
     ┌─────────┐
     │ nexusd  │ messenger: text ↔ rkyv only; validates syntax +
     │         │ protocol version; forwards requests to criomed;
     │         │ serialises replies back to text. Stateless modulo
     │         │ in-flight request correlations.
     └────┬────┘
          │ rkyv (criome-msg contract)
          ▼
     ┌─────────┐
     │ criomed │ sema's engine — validates, applies, cascades.
     │         │ • receives every request; checks validity
     │         │ • writes accepted mutations to sema
     │         │ • rules cascade as records update (nothing
     │         │   lives outside sema)
     │         │ • resolves RawPattern → PatternExpr
     │         │ • fires subscriptions on commits
     │         │ • reads plan records from sema; dispatches
     │         │   execution verbs to lojixd
     │         │ • signs capability tokens; tracks reachability
     │         │   for lojix-store GC
     │         │ • never touches binary bytes itself
     └────┬────┘
          │ rkyv (lojix-msg — concrete "do this" verbs)
          ▼
     ┌──────────┐   owns lojix-store directory
     │  lojixd  │   (lojix family; thin executor; no evaluation)
     │          │ internal actors:
     │          │   • NixRunner (spawns nix/nixos-rebuild;
     │          │     cargo runs inside via crane, not directly)
     │          │   • StoreWriter + StoreReaderPool (store-entry
     │          │     placement + path lookup + index updates)
     │          │   • FileMaterialiser (store entries → workdir)
     │          │ • receives concrete plans: RunNix (primary
     │          │   compile + build), RunNixosRebuild (deploy),
     │          │   PutStoreEntry, GetStorePath, MaterializeFiles, …
     │          │ • invokes nix (crane + fenix) against the workdir
     │          │   rsc emitted; output lands in /nix/store during
     │          │   the bootstrap era
     │          │ • replies {output-hash, warnings, wall_ms}
     └──────────┘
```

**Invariants**:

- Text crosses only at nexusd's boundary. Internal daemon-
  to-daemon messages are rkyv.
- No daemon-to-daemon path routes bulk data through criomed —
  when forge work inside lojixd writes to lojix-store, it does
  so in-process under a criomed-signed capability token; no
  bytes ever cross criomed.
- Criomed never sees compiled binary bytes; it only records
  their hashes (as slot-refs resolved to blake3 via sema) in
  sema.
- There is no `Launch` protocol verb. Store entries are real
  files at hash-derived paths; you `exec` them from a shell.

---

## 5 · The two stores

### sema — records database

- **Owner**: criomed.
- **Backend**: redb-backed, content-addressed records keyed
  by blake3 of their canonical rkyv encoding.
- **Reference model**: records store **slot-refs** (`Slot(u64)`),
  not content hashes. Sema's index maps each slot to its
  current content hash plus a bitemporal display-name binding
  (`SlotBinding` records). Content edits update the slot's
  current-hash (no ripple-rehash of dependents). Renames
  update the slot's display-name (no record rewrites
  anywhere). Display-name is global — one name per slot; rsc
  projections pick it up everywhere.
- **Change log**: per-kind. Each record-kind has its own redb
  table keyed by `(Slot, seq)` carrying `ChangeLogEntry`
  records (rev, op, content hashes, principal, sig-proof for
  quorum-authored changes). Per-kind logs are ground truth;
  per-kind index tables and a global revision index are
  derivable views.
- **Scope**: slots are **global** (not opus-scoped); one name
  per slot, globally consistent.

### lojix-store — canonical artifact store (built on nix)

lojix-store is the **canonical artifact store from day one**.
It's an analogue to the nix-store, hashed by blake3. It holds
**actual unix files and directory trees**, not blobs. A
compiled binary lives at a hash-derived path; you `exec` it
directly.

nix produces artifacts into `/nix/store` during the build.
lojixd immediately bundles them into `~/.lojix/store/` (copy
closure with RPATH rewrite) and returns the lojix-store hash.
**sema records reference lojix-store hashes as canonical
identity** — `/nix/store` is a transient build-intermediate,
not a destination.

Why not defer lojix-store: dogfooding the real interface now
reveals what it actually needs; deferred implementations rot.
The gradualist path "nix builds; lojix-store stores; loosen
dep on nix over time" is strictly safer than "nix forever
until Big Bang replace."

- **Owner**: lojixd.
- **Layout**: hash-keyed subdirectory per store entry, close
  to nix's `/nix/store/<hash>-<name>/` tree.
- **Index DB**: lojixd-owned redb table mapping
  `blake3 → { path, metadata, reachability }`. The index does
  not contain the files; it maps to them.
- **Holds**: compiled binaries and their runtime trees;
  user file attachments referenced by sema. Always real files
  on disk.
- **No typing**. The type of a store entry is known only
  through the sema record that references its hash.
- **Access control**: capability tokens, signed by criomed.

### Relationship

Sema records carry `StoreEntryRef` (blake3) fields pointing at
lojix-store entries. Criomed maintains the reachability view
and drives GC; lojixd resolves hashes to filesystem paths;
binaries are `exec`'d directly from their store path (no
extraction, no copy, no `Launch` verb).

---

## 6 · Key type families (named, not specified)

Concrete field lists live in reports; this file only names.

- **Opus** — pure-Rust artifact specification. User-authored
  sema record. Toolchain pinned by derivation reference,
  outputs enumerated, every build-affecting input a field so
  the record's hash captures the full closure.
- **Derivation** — escape hatch for non-pure deps. Wraps a nix
  flake output or inline nix expression.
- **OpusDep** — opus → {opus | derivation} link.
- **Slot** — `u64` content-agnostic identity. Counter-minted
  by criomed with freelist-reuse. Seed range `[0, 1024)`
  reserved.
- **SlotBinding** — slot-keyed binding to current content
  hash and global display name. Bitemporal; slot-reuse is
  safe for historical queries.
- **MemberEntry** — opus-membership record declaring which
  slots an opus contributes and at what visibility.
- **RawPattern** — wire form of a nexus pattern, carrying
  user-facing names. Transient on criome-msg.
- **PatternExpr** — resolved form, carrying slot-refs. Pinned
  to a sema snapshot. Internal to criomed.
- **CriomeRequest / CriomeReply** — nexusd↔criomed protocol
  verbs.
- **lojix-msg verbs** — concrete execution in criomed→lojixd
  direction: **RunNix** (primary compile + package builder,
  via crane + fenix), **BundleIntoLojixStore** (copy /nix/store
  output into lojix-store with RPATH rewrite, returns blake3
  hash), RunNixosRebuild (deploy), PutStoreEntry, GetStorePath,
  MaterializeFiles, DeleteStoreEntry. No `CompileRequest {
  opus: OpusId }` — criomed plans; lojixd executes.

---

## 7 · Data flow

### Single query

```
 human nexus text: (Query (Fn :name :resolve_pattern))
        ▼
  nexusd parses → RawPattern; wraps as criome-msg::Query
        ▼
  criomed validates; resolver(RawPattern, sema snapshot) → PatternExpr
        ▼
  matcher runs; records returned
        ▼
  criomed replies via rkyv
        ▼
  nexusd serialises reply to nexus text
        ▼
 human
```

### Mutation request (validation + apply)

```
 user: (Mutate (Fn :slot 42 :body (Block …)))
        ▼
 nexusd → criomed (criome-msg::Mutate)
        ▼
 criomed validates:
   • kind well-formed?
   • all slot-refs in the body resolve to existing slots?
   • author authorised? (caps / BLS post-MVP)
   • rule engine permits? (e.g., not mutating a seed-protected
     record)
        ▼ (if any check fails → reject with Diagnostic)
 criomed writes new content to sema:
   • per-kind ChangeLogEntry appended
   • SlotBinding updated with new current-hash
   • subscriptions on slot 42 fire → downstream cascades
     re-derive
        ▼
 criomed replies success
```

### Compile + self-host loop

Edit-time (requests accumulate):
- User issues nexus requests (Assert / Mutate / Patch) that
  change code records in sema. Each is validated; cascades
  settle; sema reflects the new state.

Run-time (plan dispatch):
- User issues `(Compile (Opus :slot N))`.
- criomed reads the Opus + transitive OpusDeps from sema.
- rsc projects records → scratch workdir containing `.rs` +
  `Cargo.toml` + `flake.nix` (crane + fenix call).
- criomed emits `RunNix { flake_ref, attr, overrides, target }`
  to lojixd.
- lojixd invokes `nix build`; nix/crane run cargo + rustc with
  the fenix-pinned toolchain; proc-macros expand in rustc;
  output lands in `/nix/store`.
- lojixd runs `BundleIntoLojixStore` on the nix output: copy-
  closure, RPATH rewrite via patchelf, deterministic bundle,
  blake3 hash, write tree under `~/.lojix/store/<blake3>/`.
- lojixd replies with `{ store_entry_hash, narhash,
  wall_ms }`.
- criomed asserts `CompiledBinary { opus, store_entry_hash,
  narhash, toolchain_pin, … }` to sema. The canonical identity
  is `store_entry_hash`; narhash is kept for nix cache lookup.

Self-host close:
- User runs the new binary directly from its lojix-store path.
- New binary connects to nexusd; asserts records; cascades fire
  against the live sema. Loop closes.

---

## 8 · Repo layout

Canonical list lives in [`docs/workspace-manifest.md`](workspace-manifest.md);
this section is the architectural roles.

- **Layer 0 — text grammars**: nota (spec), nota-serde-core
  (shared lexer+ser+de kernel), nota-serde (façade),
  nexus (spec), nexus-serde (façade).
- **Layer 1 — schema vocabulary**: nexus-schema (record-kind
  declarations: Fn, Struct, Opus, SlotBinding, MemberEntry,
  Rule, ChangeLogEntry, …).
- **Layer 2 — contract crates**: criome-msg (nexusd↔criomed;
  requests + replies), lojix-msg (criomed↔lojixd; execution
  verbs).
- **Layer 3 — storage**: sema (records DB — redb-backed;
  owned by criomed), lojix-store (content-addressed
  filesystem — owned by lojixd; includes a reader library).
- **Layer 4 — daemons**: nexusd (translator), criomed (sema's
  engine), lojixd (executor).
- **Layer 5 — clients + projectors**: nexus-cli (the text
  client), rsc (sema → `.rs` projector; linked by lojixd).
- **Spec-only (terminal state)**: lojix (namespace README).

Currently `criome-msg`, `lojix-msg`, `criomed`, `lojixd` are
CANON-MISSING — not yet scaffolded. See
`docs/workspace-manifest.md` for status.

> Some repos in this layout are not yet at terminal shape;
> see `docs/workspace-manifest.md` for current vs. terminal
> status (e.g., `lojix` is currently a working monolith and
> must not be rewritten — its own AGENTS.md carries the
> binding warning).

### Three-pillar framing

- **criome** — the runtime (nexusd, criomed, lojixd; the
  daemon graph).
- **sema** — the records.
- **lojix** — the artifacts pillar (build, compile, store,
  deploy).

criome ⊇ {sema, lojix}. nexus is the communication skin
spanning all of criome; not a fourth pillar.

**Lojix family membership** is orthogonal to layer. A crate is
lojix-family iff it participates in the content-addressed
typed build/store/deploy pipeline. `lojixd` is the only
current lojix-family daemon.

**Shelved**: `arbor` (prolly-tree versioning) — post-MVP.

---

## 9 · Grammar shape

Nota is a strict subset of nexus. A single lexer (in
nota-serde-core) handles both, gated by a dialect knob. The
grammar is organised as a **delimiter-family matrix**:

- Outer character picks the family — records `( )`, composites
  `{ }`, evaluation `[ ]`, flow `< >`.
- Pipe count inside picks the abstraction level — none for
  concrete, one for abstracted/pattern, two for
  committed/scoped.

**Every top-level nexus expression is a request.** The head of
a top-level `( )`-form is a request verb (`Assert`, `Mutate`,
`Retract`, `Query`, `Compile`, `Subscribe`, …). Nested
expressions are record constructions that the request refers
to. Parsing rejects top-level expressions that aren't requests.

**Sigil budget is closed.** Six total: `;;` (comment), `#`
(byte-literal prefix), `~` (mutate), `@` (bind), `!` (negate),
`=` (bind-alias, narrow use). New features land as delimiter-
matrix slots or Pascal-named records — **never new sigils**.

---

## 10 · Project-wide rules

Foundational rules. Every session follows these.

- **Rust is only an output.** No `.rs` → sema parsing. rsc
  emits one-way.
- **Nix is the build backend until we replace it.** Compile
  plans become `RunNix` invocations (crane + fenix); lojixd
  spawns `nix build`. Direct rustc orchestration is a post-
  nix-replacement concern. rsc emits `.rs` + `Cargo.toml` +
  `flake.nix`; nix drives the rest.
- **We author no macros.** No `macro_rules!`, no proc-macro
  crates. Our code-gen patterns are sema rules. We freely
  **call** third-party macros (derive, attribute, function-
  like) and rsc emits the invocations.
- **Skeleton-as-design.** New concrete design starts as
  compiled skeleton code (types + trait signatures + `todo!()`
  bodies) in the relevant repo. Reports are for WHY
  (philosophy, invariants, decision-journey); skeleton code
  is for WHAT (types, traits, enums, verbs). rustc checks
  consistency; prose can't drift. Example: `lojix-store/src/`.
- **AGENTS.md/CLAUDE.md shim.** In every canonical repo:
  `AGENTS.md` holds real content; `CLAUDE.md` is a one-line
  shim (`See [AGENTS.md](AGENTS.md).`). Codex reads
  AGENTS.md; Claude Code reads CLAUDE.md; both converge.
- **Delete wrong reports; don't banner.** When a report's
  thesis is wrong or the content is absorbed elsewhere,
  delete it. Banners invite agents to relitigate. Keep the
  report tree small.
- **Nexus is a request language.** Sema is rkyv. There are no
  "nexus records."
- **Sema is all we are concerned with.** Everything else
  orbits sema.
- **Text only crosses nexusd.** All internal traffic is rkyv.
- **Every edit is a request.** criomed validates; requests can
  be rejected; this is the hallucination wall.
- **Bootstrap rung by rung.** The engine bootstraps using its
  own primitives, starting from rung 0. There is no "before
  the engine runs" mode; criomed runs from the first instant,
  with sema initially empty. Nexus messages populate the
  initial versions of the database — including seed records
  via `genesis.nexus`. Each rung's capability comes from the
  data already loaded; that capability is what populates the
  next rung. No internal-assert paths, no baked-in-rkyv
  shortcuts, no special bootstrap inputs that bypass nexus.
  If a proposed mechanism cannot be explained step by step,
  the framing is wrong.
- **References are slot-refs.** Records store `Slot(u64)`;
  the index resolves slot → current hash + display name.
- **Content-addressing is non-negotiable.** Record identity is
  the blake3 of its canonical rkyv encoding.
- **A binary is just a path.** No `Launch` verb; store entries
  are real files.
- **Criomed is the overlord** of lojix-store. Tracks
  reachability; signs tokens; directs GC.
- **lojixd is for effects sema can't do.** Its inputs are plan
  records; its outputs are outcome records. It never sees an
  Opus directly.
- **No backward compat.** The engine is being born. Rename,
  move, restructure freely until Li declares a compatibility
  boundary.
- **No ETAs.** Describe the work; don't schedule it.
- **Sigils as last resort.** New features are delimiter-matrix
  slots or Pascal-named records.
- **One artifact per repo** (per rust/style.md rule 1).

### Rejected framings (reject-loud)

Agents repeatedly rediscover wrong framings when the docs
say only what is true. These explicit rejections block
recurrence. Add to this list when Li rejects a new framing.

- **Aski is retired.** mentci-next / sema-ecosystem does not
  treat aski as a design input. Do not reason from aski
  axioms (II-L, v0.21 syntax, synth.md, compile-pipeline
  framing) to current sema architecture. Shared surface
  features (delimiter-family matrix, case rules) are
  coincidence, not lineage.
- **Scope is world-supersession, not personal-scale.** CriomOS
  aims to supersede proprietary operating systems and
  computing stacks globally. Framings like "personal-scale,"
  "craftsperson workshop," or "self-hosted-self" underestimate
  the project.
- **Sema is local; reality is subjective.** There is no global
  sema, no federated-global database, no single logical truth.
  Each criomed holds a subjective view; instances communicate,
  agree, disagree, and negotiate to reach agreement. "Global
  database," "global blockchain," and "federated global sema"
  are wrong framings.
- **Categories are intrinsic.** Code records and world-fact
  records cannot share a category — the separation is a fact
  of reality, not a schema choice. The code category is named
  **machina** (the subset of sema that compiles to Rust in
  v1). The native checker over machina records is
  **machina-chk** (not "semachk" — the check is not over all
  of sema). Names for world-fact, operational, and authz
  categories are still open.
- **Self-hosting close is normal software engineering.** The
  engine works correctly, canonical crates authored as
  records. Bit-for-bit identity with the bootstrap version is
  not a bar — new rustc versions aren't byte-identical to
  predecessors either.
- **Nexus is the agent interface.** "Legibility to agents" is
  not a separate design axis. Nexus is how agents (LLMs,
  humans, scripts) interact with criome; text in, criomed-
  validated records out.

### Reject-loud rule

When a framing is considered and rejected, state the
rejection here — not just the acceptance elsewhere. Past
recurring wrong frames: aski-as-input, personal-scale,
global-database, federation, boundary-as-tension,
bit-for-bit-identity, legibility-axis, sema-as-data-store,
four-daemon topology, ingester-for-Rust, lojix-store-as-
blob-DB, banner-wrong-reports.

---

## 11 · Update policy

This file is the golden document. Edits are deliberate and
surgical.

1. **No report links here.** Cross-references go *into* this
   file from reports, not *out of* this file to reports.
   Reading lists, decision histories, type-spec details all
   live in reports or in `docs/workspace-manifest.md` —
   never inline here.
2. **Prose + diagrams only.** Type sketches, field lists,
   enum variants belong in skeleton code (compiler-checked)
   or in reports.
3. **Update this file first**, then update implementation
   in the affected repos, then write a report only if the
   decision carries a journey worth recording.
4. **If a framing is rejected, name the rejection in §10
   "Rejected framings."** Stating only the acceptance lets
   agents rediscover the wrong frame.
5. **If a report is superseded, delete it.** Don't banner.
6. **Skeleton-as-design over prose-as-design.** Prefer
   compiler-checked types in the relevant repo over prose
   here.

---

*End docs/architecture.md.*
