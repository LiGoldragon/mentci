# 094 — Explicit handoff for a fresh agent

*Read this first. Then read the files in the order given.
Then read the other reports. Then start work.*

This document is the post-context-reset entry point. A fresh
agent (or future-me after compaction) lands here, reads the
files in §1, internalises the rules in §2, understands the
state in §3 + §4, and proceeds with the M0 plan in §5.

The user is **Li**. The project is **criome**. The dev
environment is **mentci**. The conversation that produced this
report ran 2026-04-26 → 2026-04-27 and landed substantial
code + every architectural decision below.

---

## 1 · Required reading, in order

Read all of these. The order matters — earlier ones set up
context for later ones. **Spawn an `Explore` agent for any
file you can't fit in your own context budget**; the agents
return summaries that compress without losing structure.

### 1a · Operational rules (load before doing anything)

1. [`../AGENTS.md`](../AGENTS.md) — workspace operational
   rules. Substance-in-reports rule, jj workflow with
   blanket-auth always-push, naming rule with bad→good table,
   commit-message S-expression style, design-doc hygiene
   (state criteria positively), no-version-history-in-designs,
   verify-each-parallel-tool-result, design-doc layer table.
2. [`../repos/tools-documentation/AGENTS.md`](../repos/tools-documentation/AGENTS.md)
   — workspace-wide cross-project rules.
3. [`../repos/tools-documentation/rust/style.md`](../repos/tools-documentation/rust/style.md)
   — Rust style guide. The two load-bearing sections are:
   - **§Methods on types, not free functions** + the
     "Why this matters — affordances, not operations"
     subsection (free functions let the agent skip creating
     the noun-type that should own the verb).
   - **§Naming — full words by default** + 6-class
     exception list + bad→good table.
4. [`../repos/tools-documentation/jj/basic-usage.md`](../repos/tools-documentation/jj/basic-usage.md)
   — jj. **Use `jj commit` always, never `jj describe`** for
   normal commits.
5. [`../repos/tools-documentation/bd/basic-usage.md`](../repos/tools-documentation/bd/basic-usage.md)
   — beads (issue tracker). bd-vs-files distinction.
6. [`../repos/tools-documentation/nix/basic-usage.md`](../repos/tools-documentation/nix/basic-usage.md)
   — nix. `nix run nixpkgs#<tool>`, never `cargo install` /
   `pip install` / `npm -g`.
7. [`../repos/tools-documentation/dolt/basic-usage.md`](../repos/tools-documentation/dolt/basic-usage.md)
   — dolt (the data store under bd).

### 1b · Project-wide architecture (load before designing)

8. [`../repos/criome/ARCHITECTURE.md`](../repos/criome/ARCHITECTURE.md)
   — **THE canonical project doc.** Read in full. Specifically:
   - **§2 Invariants A / B / C / D** are load-bearing.
     - A: Rust is only an output (no `.rs` → sema parsing).
     - B: Nexus is a language, not a record format. (Plus
       the four sub-rules: criome-speaks-signal-only, nexus-
       is-not-a-programming-language, no-parser-keywords-≠-
       no-schema-enums, slots-are-user-facing-identity.)
     - C: Sema is the concern; everything orbits.
     - **D: Perfect specificity.** Every typed boundary
       names exactly what flows through it. No wrapper
       enums. No string-tagged dispatch. Per-verb payload
       types. Query-as-kind. KindDecl-as-data + typed-
       Rust-as-projection.
   - §3 The request flow.
   - §10 Project-wide rules — the operational rules list +
     the rejected-framings list (the only place wrong frames
     are named).

### 1c · Grammar (load before any code that touches text)

9. [`../repos/nota/README.md`](../repos/nota/README.md) — the
   base nota grammar. PascalCase / camelCase / kebab-case
   identifier classes; bare-identifier strings;
   `#[serde(transparent)]` opt-in for primitive newtypes.
10. [`../repos/nexus/spec/grammar.md`](../repos/nexus/spec/grammar.md)
    — full nexus grammar. Verb table; **§Binds** with the
    strict auto-name-from-schema rule; reply semantics.
11. [`../repos/nexus/spec/examples/`](../repos/nexus/spec/examples/)
    — concrete `.nexus` files. flow-graph.nexus +
    patterns-and-edits.nexus.

### 1d · Per-repo architecture (load before touching that repo)

12. [`../repos/signal/ARCHITECTURE.md`](../repos/signal/ARCHITECTURE.md)
    — wire format + per-verb typed IR + perfect-specificity-
    at-the-wire (companion to Invariant D).
13. [`../repos/nexus/ARCHITECTURE.md`](../repos/nexus/ARCHITECTURE.md)
    — text translator daemon + QueryParser.
14. [`../repos/sema/ARCHITECTURE.md`](../repos/sema/ARCHITECTURE.md)
    — record store + "stored by precise kind."
15. [`../repos/sema/reference/Vision.md`](../repos/sema/reference/Vision.md)
    — aspirational vision in plain language. Read after
    arch docs, not before — the arch is canonical, the
    vision is forward-looking.
16. [`../repos/nexus-cli/ARCHITECTURE.md`](../repos/nexus-cli/ARCHITECTURE.md)
    — text shuttle client.
17. [`../repos/lojix-schema/ARCHITECTURE.md`](../repos/lojix-schema/ARCHITECTURE.md)
    — lojix verb payload types (M2+ pipeline).
18. [`./mentci-arch.md` — actually `../ARCHITECTURE.md`](../ARCHITECTURE.md)
    — workspace dev environment.
19. [`../docs/workspace-manifest.md`](../docs/workspace-manifest.md)
    — repo inventory + statuses.

### 1e · Reports (load when working on the topic each covers)

Six active reports survive the cleanup. Each has a specific
purpose:

20. [`074-portable-rkyv-discipline.md`](074-portable-rkyv-discipline.md)
    — the rkyv 0.8 pinned-feature-set reference. Cited from
    every Cargo.toml that uses rkyv. Durable.
21. [`088-closed-vs-open-schema-research.md`](088-closed-vs-open-schema-research.md)
    — the deep research that produced the perfect-
    specificity decision. Read this if you ever question
    why per-verb typed payloads vs a generic record wrapper.
22. [`091-pattern-rethink.md`](091-pattern-rethink.md) — the
    bind-name-from-schema rule + corrected `PatternField`
    shape (no String payload). Implemented in
    [`../repos/nexus/src/parse.rs`](../repos/nexus/src/parse.rs).
23. [`../repos/tools-documentation/programming/naming-research.md`](../repos/tools-documentation/programming/naming-research.md)
    — full research backing the "spell every identifier as
    full English words" rule. The rule itself is in
    AGENTS.md; this is the *why*. (Also see sibling
    [`abstractions-research.md`](../repos/tools-documentation/programming/abstractions-research.md)
    and [`beauty-research.md`](../repos/tools-documentation/programming/beauty-research.md)
    for the methods-on-types and beauty rules.)
24. [`093-project-wide-style-review-plan.md`](093-project-wide-style-review-plan.md)
    — plan for verifying every Rust crate against both new
    style rules. Stage 1.1 (nexus) is done; stage 1.2/1.3
    + stage 2 are pending.
25. [`089-m0-implementation-plan-step-3-onwards.md`](089-m0-implementation-plan-step-3-onwards.md)
    — the M0 implementation plan. Steps 1 + 2 + parser are
    done; **step 3 (criome body), step 5 (nexus daemon body),
    step 6 (nexus-cli), step 7 (genesis.nexus) remain**.
    Read in full before starting M0 implementation.

### 1f · This report

26. **This file** — [`094-handoff-explicit.md`](094-handoff-explicit.md).
    Re-read it after the others.

---

## 2 · Operational rules — the ones agents most often violate

Distilled from AGENTS.md + the rust/style.md + the workspace
tools docs. Internalise these as muscle memory.

### 2a · Substance in reports, not chat

The Claude Code UI is a poor reading interface. The user
reviews asynchronously while you move on. Anything that
**explains, proposes, analyses, or summarises** goes in a
file under [`../reports/`](../reports/), and the chat reply
is one line pointing at the file. Acknowledgements and
"done; pushed" confirmations don't need reports.

If your final-session response would be more than a few
lines, you've already failed. Write the report instead.

### 2b · Use `jj commit`, never `jj describe`

`jj describe` names `@` without advancing it. It looks like
it works but creates surprising states (empty trailing
commits, mis-described commits, subsequent edits silently
joining the pushed change).

Always:
```
jj commit -m '<msg>'    # or -m "<msg>" if msg has apostrophes
jj bookmark set main -r @-
jj git push --bookmark main
```

`-r @-` because `jj commit` advances `@` to a new empty
change; the commit you just made is its parent.

**Always commit + push** after every meaningful change.
Blanket authorisation. This overrides Claude Code's default
"ask before pushing." Unpushed work is invisible to nix
builds and downstream flake consumers.

### 2c · Full English words

Per [`naming-research.md`](../repos/tools-documentation/programming/naming-research.md) and the rule in
[`../AGENTS.md`](../AGENTS.md): spell every identifier as a
full English word. `lex` → `lexer`, `tok` → `token`,
`ident` → `identifier`, `op` → `operation`, `de` →
`deserializer`, `kd` → `kind_decl`, `pf` → `pattern_field`,
`ctx` → `context`, `cfg` → `configuration`, `sock` →
`socket`, `args` → `arguments`, `params` → `parameters`,
etc.

Six exception classes (loop counters in tight scope, math
contexts, generic type parameters, general-English acronyms,
std-inherited names, kind-names documented in
ARCHITECTURE.md). Everything outside spells out.

When generating new code that lives next to existing cryptic
code: **break the local-dialect pattern**. Use full words
even if surrounding code is cryptic. Don't propagate.

### 2d · Methods on types, not free functions

Per [`../repos/tools-documentation/rust/style.md`](../repos/tools-documentation/rust/style.md)
§Methods on types. The rule isn't aesthetic — it forces you
to ask "what type owns this verb?" If you can't name the
type, you haven't found the right model yet.

Free functions let you skip creating the noun-type that
should own the verb. The codebase then develops gaps:
verbs without owning nouns, missing structural types.

Carve-out: small private helpers genuinely local to one
module are fine.

### 2e · Memory is deprecated

The auto-memory system at `~/.claude/projects/<project>/memory/`
is deprecated for this workspace. **Don't write new memory
files.** The single remaining stub
([`memory_deprecated.md`](memory/memory_deprecated.md)) tells
you where each kind of content goes:

- Architecture facts → ARCHITECTURE.md (project-wide or per-
  repo)
- Operational rules → AGENTS.md
- Tool usage → tools-documentation/<tool>/basic-usage.md
- Rust style → tools-documentation/rust/style.md
- Decision narratives → reports/
- Short tracked items → `bd remember` / `bd memories`

If you would have written `bd remember`, do that instead.
Project files are visible to Li; memory is not.

### 2f · Relative paths in reports

Reports live in [`../reports/`](../reports/). Sibling repos
are reachable via the workspace symlinks at
[`../repos/<name>/`](../repos/). Always use these relative
paths in report links, not GitHub URLs — Li reads in Codium
and clicks links to open files locally.

### 2g · Verify each parallel-tool result

When batching `Write` / `Edit` / `Bash` calls, scan every
result block for errors before any follow-up step. Failed
`Write` calls (typically the "must Read first" guard) cascade
silently. The bundle returning is not the bundle succeeding.

### 2h · Design-doc hygiene

State criteria **positively**. Don't accumulate "do not use
X" / "X was ruled out" patterns in design docs — they pollute
context for every future agent. Excluded options are
silently omitted. The `criome/ARCHITECTURE.md §10 Rejected
framings` list is the *only* place wrong frames are named,
and only as one-line entries.

### 2i · No version history in design docs

Vision / design / architecture docs describe what the system
IS, not the lineage of failed attempts. No "this is the
third try," no "we abandoned X to get here." git/jj log
preserves history; docs preserve current truth.

### 2j · Commit message style

Single-line nested-paren S-expression. First token is repo
name. `[...]` for notes, `—` for rationale, `((...))` for
direct user quotes. See AGENTS.md §"Commit message style"
for templates + examples.

---

## 3 · Project shape in one paragraph

**Criome** is a typed binary record-graph engine. **Sema** is
its heart — content-addressed records, native binary form
(rkyv-encoded), each kind a Rust struct generated by **rsc**
from `KindDecl` records in sema itself. Records reference
each other by `Slot` (mutable identity). **Signal** is the
wire form (per-verb typed enums; no generic record wrapper).
**Nexus** is the text bridge — a daemon parsing text into
signal frames for criome, rendering signal replies back to
text for clients. **Nexus-cli** is a thin text shuttle.
**Lojix** + family handle the build/store/deploy pillar
(M2+). The project is **self-hosting**: criome compiles its
own source from records in sema, via rsc + nix. The user
exposes everything as text through nexus; LLM agents author
nexus text; criome validates and stores.

---

## 4 · Code state (what's working today)

| Crate | LoC | State | Tests |
|---|---|---|---|
| [`../repos/nota-serde-core/`](../repos/nota-serde-core/) | 1808 | Working — lexer + ser + de kernel; supports nota + nexus dialects; PascalCase enforcement at record/variant heads; transparent newtype support; `is_pascal_case` + `is_lowercase_identifier` public helpers | 174 |
| [`../repos/signal/`](../repos/signal/) | ~1000 | Working — per-verb typed enums (AssertOp / MutateOp / RetractOp / AtomicBatch / QueryOp / Records); `Frame` + handshake + auth; `KindDecl` + flow-graph kinds (Node / Edge / Graph + paired *Query types); `RelationKind` with `::ALL`, `::from_variant_name`, `::variant_name` methods; `PatternField<T>` no-payload Bind; `Slot`/`Revision` `#[serde(transparent)]`; `Hash` BLAKE3 alias | 18 |
| [`../repos/nota-serde/`](../repos/nota-serde/) | 29 | Façade re-exporting nota-serde-core (Nota dialect) | 0 |
| [`../repos/nexus-serde/`](../repos/nexus-serde/) | 93 | Façade re-exporting nota-serde-core (Nexus dialect) + 6 sentinel wrappers (Bind / Mutate / Negate / Validate / Subscribe / AtomicBatch) | 0 |
| [`../repos/nexus/`](../repos/nexus/) | ~280 | `QueryParser` in [`src/parse.rs`](../repos/nexus/src/parse.rs) — text `(\| Kind ... \|)` → typed `signal::QueryOp`; per-kind PatternField parsing; bind-name validated against schema field name; daemon body (`src/main.rs`) is a stub | 24 |
| [`../repos/sema/`](../repos/sema/) | ~200 | `Sema::open` / `store(&[u8]) → Slot` / `get(Slot) → Option<Vec<u8>>`; redb-backed; monotone slot allocation starting at `SEED_RANGE_END = 1024`; persistent across reopens | 7 |
| [`../repos/criome/`](../repos/criome/) | 156 | Skeleton — six validator pipeline modules + `UdsListener` stub + `main.rs` stub; M0 body to come (step 3 of 089) | 0 |
| [`../repos/nexus-cli/`](../repos/nexus-cli/) | 21 | Stub returning `Ok(())`; M0 text shuttle to come (step 6 of 089) | 0 |
| [`../repos/lojix-schema/`](../repos/lojix-schema/) | 112 | Typed scaffold — `LojixRequest` / `LojixReply` enums + 3 spec types + 3 outcome types | 0 |

All crates `cargo check` clean. Total ~3700 LoC, 223 tests
passing.

**Repos out of M0 scope** (M2+): lojix, lojix-cli, lojix-store,
rsc, horizon-rs, CriomOS, CriomOS-emacs, CriomOS-home,
arbor (shelved).

---

## 5 · M0 implementation plan (what's left)

Read [`089`](089-m0-implementation-plan-step-3-onwards.md)
in full. Summary of what's done vs to-do:

**Done:**
- Step 1 — signal rewrite to per-verb typed payloads (per
  [`088`](088-closed-vs-open-schema-research.md)).
- Step 2 — sema body (`Sema::open / store / get`).
- Parser (was step 4) — `QueryParser` in nexus per
  [`091`](091-pattern-rethink.md).

**To do:**
- **Step 3 — criome body** (~150 LoC):
  [`../repos/criome/src/`](../repos/criome/src/)
  - `uds.rs` — tokio `UnixListener` bind on
    `/tmp/criome.sock`, accept loop.
  - `main.rs` — open sema, run listener.
  - `dispatch.rs` (new) — match `Request` → `Reply`.
  - `assert.rs` (new) — `AssertOp` → encode + sema.store.
  - `query.rs` (new) — `QueryOp` → iterate sema, decode,
    filter by `PatternField`, return typed `Records`.
  - 4 integration tests.
  - **Verb scope**: Handshake + Assert + Query for M0;
    Mutate / Retract / AtomicBatch / Subscribe / Validate
    return `Diagnostic E0099 "verb not implemented in M0"`.
- **Step 5 — nexus daemon body** (~130 LoC):
  [`../repos/nexus/src/`](../repos/nexus/src/)
  - `main.rs` — bind `/tmp/nexus.sock`, accept loop.
  - `handler.rs` (new) — per-connection text shuttle +
    paired criome connection + handshake.
  - Reply rendering via `nota-serde-core::to_string_nexus`
    (NO HARDCODING — per Invariant D).
- **Step 6 — nexus-cli text shuttle** (~30 LoC):
  [`../repos/nexus-cli/src/main.rs`](../repos/nexus-cli/src/main.rs)
  - Read file or stdin → connect `/tmp/nexus.sock` →
    write → read → stdout. No tokio needed.
- **Step 7 — `genesis.nexus` + bootstrap** (~50 LoC):
  - Text file at `../repos/criome/genesis.nexus` with the
    bootstrap `KindDecl` records (Node / Edge / Graph /
    KindDecl itself).
  - Bootstrap glue in `criome/main.rs` — on first boot
    (empty sema), parse genesis text and dispatch each
    KindDecl through the normal Assert path.

End-to-end demo on completion: `nexus-cli example.nexus`
where example.nexus contains `(Node "User")` and
`(| Node @name |)`, with daemon + criome running, returns:

```
(Ok)
[(Node User)]
```

---

## 6 · Settled architecture decisions — DO NOT relitigate

These are settled. If a future session brings them up
again, point at the relevant doc.

| Decision | Where it lives |
|---|---|
| Per-verb typed payloads (no `KnownRecord` wrapper enum) | criome/ARCH §2 Invariant D + signal/ARCH §"Perfect specificity at the wire" + 088 |
| `PatternField::Bind` carries no payload (name implicit from `*Query` field position) | nexus/spec/grammar.md §"The strict rule" + signal/src/pattern.rs + 091 |
| `Slot` and `Revision` use `#[serde(transparent)]` — bare integers in nexus text | nota/README.md §Newtype structs + signal/src/slot.rs |
| Bind names MUST equal schema field name at the position they appear | nexus/spec/grammar.md §Binds + nexus/src/parse.rs check_bind_name |
| PascalCase enforced for record/variant head identifiers at parse time | nota/README.md §Identifiers + nota-serde-core deserialiser |
| `@` sigil required for binds in pattern position; bare lowercase = bare-string literal | nexus/spec/grammar.md §"must carry @ sigil" |
| Nexus daemon is in the path (CLI never speaks signal directly) | signal/ARCH ASCII diagram + nexus/ARCH + nexus-cli/ARCH |
| Schema-as-data via `KindDecl`; closed Rust enum is rsc's projection (rsc lands M2+) | criome/ARCH §2 Invariant D + signal/src/schema.rs + 088 §9 |
| Replies pair to requests by FIFO position; no correlation IDs | signal/ARCH §"Reply protocol" + nexus/spec/grammar.md §"Reply semantics" |
| One subscription per connection; close socket to end | nexus/spec/grammar.md §Subscriptions |
| All-rkyv discipline with pinned feature set | 074 + criome/ARCH §10 + every Cargo.toml |
| Memory deprecated; project files + bd are the alternatives | memory/memory_deprecated.md + this doc §2e |

**Rejected framings** (named in
[`../repos/criome/ARCHITECTURE.md`](../repos/criome/ARCHITECTURE.md)
§10 reject-loud list) — see that section for the canonical
list. Don't propose any of: aski-as-input, personal-scale
framing, global-database, federation, bit-for-bit-identity,
legibility-axis, sema-as-data-store, four-daemon topology,
ingester-for-Rust, lojix-store-as-blob-DB,
banner-wrong-reports.

---

## 7 · The lurking dangers — what trips agents

Concrete things that have caused churn in prior sessions:

1. **Cryptic naming pattern-matching** — agents see `lex`
   in surrounding code and continue with `lex`. The naming
   rule explicitly says break this pattern; spell out new
   identifiers regardless of local dialect.
2. **`jj describe` instead of `jj commit`** — looks like
   it works; creates surprising states. See §2b.
3. **Dramatising trivial parser problems** — the
   `PatternField` dispatch was framed as a "Path A vs B"
   debate over multiple reports before the answer turned
   out to be ~50 lines of straight recursive descent. If
   you find yourself proposing multiple paths, check
   whether the problem is actually hard.
4. **Long chat replies** — substance goes in reports;
   chat is one-line pointers. See §2a.
5. **Writing new memory files** — don't. See §2e.
6. **Referencing deleted reports** — only 074, 088, 089,
   091, 093, 094, 095 exist as reports. Research syntheses
   for naming / methods-on-types / beauty have been re-homed
   to [`../repos/tools-documentation/programming/`](../repos/tools-documentation/programming/)
   as `naming-research.md` / `abstractions-research.md` /
   `beauty-research.md` (long-lived public docs, no longer
   subject to report rollover).
7. **Referencing deleted/renamed concepts**:
   - `nexus-schema` (absorbed into signal)
   - `client_msg` (deleted)
   - `Patch` / `TxnBatch` (renamed AtomicBatch)
   - `Reply::Rejected` (renamed `OutcomeMessage::Diagnostic`)
   - `Resume` (no such verb)
   - `RawRecord` / `RawValue` / `RawPattern` (deleted)
   - `KNOWN_KINDS` (deleted)
   - `criomed` (renamed `criome`)
   - `nexusd` (renamed `nexus`)
   - `criome-schema` (never existed)
   - `aski` / `v1` / `v2` / `criomev3` / `mentci-next`
     (abandoned naming)
   - `:keyword` / `(Assert …)` / `[ ]`-strings / `< >`-flow-
     delimiters / `{|| ||}`-atomic-batch (old grammar)
   - `correlation_id` on frames (removed)
8. **Compile verb** — listed in criome/ARCH §3 / §7 / §9
   as a planned post-MVP verb. NOT in `signal::Request`
   today. Don't propose it as current.
9. **The redundant empty commit** on nexus main from a
   bad jj-describe pattern earlier this session — harmless,
   left in place.

---

## 8 · Reports inventory

| # | File | Purpose | Lifetime |
|---|---|---|---|
| 074 | [pinned-features rkyv discipline](074-portable-rkyv-discipline.md) | Cited from every Cargo.toml using rkyv | DURABLE — keep |
| 088 | [closed-vs-open schema research](088-closed-vs-open-schema-research.md) | Research backing perfect specificity (Invariant D) | DURABLE — keep |
| 089 | [M0 implementation plan steps 3+](089-m0-implementation-plan-step-3-onwards.md) | Active M0 plan (criome / daemon / cli / genesis to come) | ACTIVE — work-in-progress |
| 091 | [pattern parser rethink](091-pattern-rethink.md) | Implemented; the auto-name-from-schema rule + corrected `PatternField` shape | DURABLE — keep as backing |
| 093 | [project-wide style review plan](093-project-wide-style-review-plan.md) | Active plan; nexus stage 1.1 done, signal/sema/etc to verify | ACTIVE — pending |
| 094 | this report | Post-context-reset entry point | DURABLE — keep, refresh as state evolves |
| 095 | [project-wide rust style audit](095-style-audit-2026-04-27.md) | Active style-fix plan with Q1–Q4 decisions | ACTIVE — fix-up pending |

**Re-homed research syntheses** (now in `tools-documentation/programming/`,
no longer subject to report rollover):

- [`naming-research.md`](../repos/tools-documentation/programming/naming-research.md) (was reports/092)
- [`abstractions-research.md`](../repos/tools-documentation/programming/abstractions-research.md) (was reports/096)
- [`beauty-research.md`](../repos/tools-documentation/programming/beauty-research.md) (was reports/097)

Reports policy: per
[`../AGENTS.md`](../AGENTS.md) report-hygiene rules. Soft cap
~12 reports; trim discipline runs when needed. Wrong-premise
reports get **deleted** (no banners). Working notes that have
been absorbed into ARCHITECTURE.md or AGENTS.md get **deleted**.

---

## 9 · Tooling cheat sheet

```
# session-close protocol — every meaningful change
jj commit -m '<msg>'      # NEVER jj describe
jj bookmark set main -r @-
jj git push --bookmark main

# session start (via bd hook)
bd ready                  # what's available to work on
bd memories <keyword>     # search project-scoped memories

# building / testing
cd <repo> && cargo test                   # quick
cd <repo> && nix flake check              # canonical (sandboxed)

# missing tool? never install
nix run nixpkgs#<pkg> -- <args>

# bd — short tracked items only; long-form goes in files
bd create --title="X" --description="Y" --type=task --priority=2
bd update <id> --claim
bd close <id>
bd dolt push  # at session end
```

---

## 10 · Where to start work

If the user gives you a specific task, do that. Otherwise,
the natural next chunk of work is **M0 step 3 — criome body**
per [`089 §2`](089-m0-implementation-plan-step-3-onwards.md).
Read 089 in full, propose any clarifying questions, then
start.

For style polish work, [`093`](093-project-wide-style-review-plan.md)
stage 1.2 (signal verify) and stage 2 (nota-serde-core
audit) are pending.

For doc work, the audit pass landed this turn finished the
known-stale items. If new staleness is found, fix it as part
of normal work; don't create a separate doc-cleanup task.

---

## 11 · The shape of a successful turn

A turn that goes well looks like:

1. Read the user's message; understand the actual ask.
2. If the task is non-trivial: read the relevant docs first
   (don't guess; verify against current code).
3. If multiple steps: track via `bd` only if the user wants
   tracked work; otherwise proceed without TodoWrite-style
   ceremony.
4. Edit code or docs.
5. Run tests if code touched.
6. `jj commit + bookmark + push` (per repo, per logical
   chunk). The S-expression commit message captures what
   changed AND why AND any direct user quote that
   justifies it.
7. Brief chat reply pointing at the change. If the change
   warrants explanation, the explanation lives in a report,
   not in chat.

A turn that goes badly looks like: long chat replies,
`jj describe`, cryptic identifiers in new code, propagating
local dialect from surrounding code, half-finished
implementations sitting uncommitted at end-of-turn.

---

*End 094.*
