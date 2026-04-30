# Agent instructions

Tool references live in [`repos/tools-documentation/`](repos/tools-documentation/) — a symlink to `~/git/tools-documentation/` created on `nix develop` / direnv entry.

Start there for: cross-project rules (jj workflow, always-push, Rust style — see [rust/style.md](repos/tools-documentation/rust/style.md)) in [`repos/tools-documentation/AGENTS.md`](repos/tools-documentation/AGENTS.md), and curated daily-use docs for jj, bd, dolt, nix under [`repos/tools-documentation/<tool>/basic-usage.md`](repos/tools-documentation/).

## Architecture

This repo (mentci) is the **dev environment + meta-deploy aggregator**. The project being built is **criome**.

> **🚨 [`INTENTION.md`](INTENTION.md) is REQUIRED READING — read it first. 🚨**
>
> It states *why* the project exists and what is being optimised for. It is upstream of every other doc; if a downstream doc conflicts with it, the downstream doc is wrong. Among other invariants: agents do not estimate, do not pre-decide on cost, and do not frame work as "MVP" / "ship fast" / "time-to-market."

> **🚨 [criome's `ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) is REQUIRED READING for every agent and human working in any sema-ecosystem repo. 🚨**
>
> Read it in full before touching any component (criome, signal, signal-forge, signal-arca, sema, nexus, nexus-cli, forge, arca, prism, mentci-lib, lojix-cli, the GUI repo, or any future canonical crate). Per-repo `ARCHITECTURE.md` files describe each repo's niche; criome's describes how the niches fit. Both are needed; criome's is the apex.

1. Read [`INTENTION.md`](INTENTION.md) — the project's intent and the invariants that shape every decision.
2. Read [criome's `ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) — the canonical reference for the engine being built (sema, nexus, forge, criome, prism, arca, signal, signal-forge, signal-arca, …).
3. Read [`ARCHITECTURE.md`](ARCHITECTURE.md) at this repo's root for the dev-environment + deployment shape.

Design history and decision records are in [`reports/`](reports/).

**Workspace manifest**: [`docs/workspace-manifest.md`](docs/workspace-manifest.md) lists every repo under `~/git/` with its status. `devshell.nix`'s `linkedRepos` mirrors the CANON + TRANSITIONAL entries.

### Documentation layers — strict separation

| Where | What | Example |
|---|---|---|
| [`criome/ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) | **Project-wide canonical.** Prose + diagrams only. No code. High-level shape, invariants, relationships, rules of the engine being built. | "criome owns sema; forge owns arca; text crosses only at nexus" |
| [`mentci/ARCHITECTURE.md`](ARCHITECTURE.md) | **This dev environment.** Workspace conventions, role, layout. Points at criome for the project itself. | "mentci is the dev workshop; long-term it becomes the universal UI" |
| `<repo>/ARCHITECTURE.md` | **Per-repo bird's-eye view.** This repo's role, boundaries (owns / does not own), code map, status. Points at criome for cross-cutting context — does *not* duplicate it. Per the matklad ARCHITECTURE.md convention. | `arca/ARCHITECTURE.md` "owns the `~/.arca/` layout + index DB" |
| [`reports/NNN-*.md`](reports/) | **Decision records + design syntheses.** Prose + visuals (diagrams, tables, swimlanes, flowcharts). No implementation code — type sketches and code go in skeleton code in the relevant repo. | "the build-flow swimlane in [reports/110](reports/110-engine-architecture-snapshot-end-to-end-build-2026-04-29.md)" |
| the repos themselves | **Implementation.** Rust code, tests, flakes, Cargo.toml. Type sketches as compiler-checked skeletons live here. | `signal/src/flow.rs` |

If a layer rule is violated, rewrite: move type sketches out of `criome/ARCHITECTURE.md` into a report or skeleton code; move runnable code out of reports into the appropriate repo. The architecture stays slim so it remains readable in one pass.

**No report links inside `criome/ARCHITECTURE.md`.** Cross-references go *into* architecture from reports, not *out of* architecture to reports. Reading lists, decision histories, type-spec details all live in reports or in `docs/workspace-manifest.md` — never inline in criome's architecture.

When architecture changes, update `criome/ARCHITECTURE.md` first, then update the affected repos, then write a report only if the decision carries a journey worth recording. Per the project rule "delete wrong reports, don't banner them," superseded reports are deleted — they do not stay as banner-wrapped relics.

### Inclusion/exclusion rule — HARD

**If a repo is not listed as CANON or TRANSITIONAL in the workspace manifest, do not edit its source or docs.** Agents that drift outside the manifest corrupt repos that are either superseded, archived, or outside scope. To add a new canonical repo: update the manifest and `devshell.nix`, write a report, commit.

### AGENTS.md / CLAUDE.md pattern

Across all canonical repos we follow: **`AGENTS.md` holds the real content; `CLAUDE.md` is a one-line shim reading "See [AGENTS.md](AGENTS.md)."** This way Codex (which reads `AGENTS.md`) and Claude Code (which reads `CLAUDE.md`) converge on a single source of truth. When creating or restructuring a repo, keep this pattern.

### Per-repo `ARCHITECTURE.md` at root

Every canonical repo carries an `ARCHITECTURE.md` at its root (matklad convention). The file is short — typically 50-150 lines — and answers: *what does this repo do, where do things live in it, and how does it fit into the wider sema-ecosystem.* Standard sections: role, boundaries (owns / does not own), code map, invariants, status, cross-cutting context (link to [criome's `ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) and any relevant report).

Per-repo `ARCHITECTURE.md` does **not** duplicate criome's `ARCHITECTURE.md`. It points. Project-wide invariants live once, in criome; per-repo files describe their own niche only. When a repo's role changes, edit that repo's `ARCHITECTURE.md` and (if the change is system-level) criome's `ARCHITECTURE.md`.

When creating a new canonical repo: write `ARCHITECTURE.md` at root before the first commit.

MVP goal: **self-hosting** — write the system's own source as records in the sema database; prism projects those records to `.rs` files (one phase of forge-daemon's runtime-creation pipeline); rustc compiles them; the new binary reads and extends its own database.

A **`Graph`** record is the database's compilation unit — a flow-graph of `Node` records (joined by `Edge` records carrying `RelationKind::Contains` for membership and `DependsOn` for cross-graph dependencies) that compiles to one artifact (library or binary). The flow-graph IS the program. Corresponds to one Rust crate on the filesystem side.

Write discoveries in [`reports/`](reports/) or in tools-documentation as topic files, don't scatter them across the repo root.

## Report hygiene — don't restate-to-refute

When a frame has been **decisively rejected** (architecture.md §10 "Rejected framings", a bd memory, or a chat correction): do not re-present it as a candidate in subsequent reports just to refute it. State only the correct frame.

When a previous report's premise is **wrong**: delete it and write a clean successor that states only the correct view. Do not append corrections, do not banner, do not restate-to-refute.

The rejected-framings list in [criome's `ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) §10 is the *only* place wrong frames are named, and only as one-line entries. Forensic narratives ("here's how this contamination crept in") are not reports — their lessons land in §10 as one-liners and in bd memories; the forensic narrative itself goes too.

## Report rollover at the soft cap

**Soft cap: ~12 active reports** in [`reports/`](reports/). When the count exceeds this, run a rollover pass before adding the next report. For each existing report, decide one of:

1. **Roll into a new consolidated report.** Multiple reports covering the same evolving thread fold into a single forward-pointing successor. The successor supersedes the old reports; the old ones are deleted (no banner).

2. **Implement.** If the report's substance can be expressed as architecture ([criome's `ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)), as a per-repo `ARCHITECTURE.md`, as code (skeleton-as-design in the relevant repo), or as an `AGENTS.md` rule, move it to the right home and delete the report.

3. **Delete.** If the report's content is already absorbed elsewhere or its premise has been refuted, delete it.

The choice is made by reading each report against the author's intent — no mechanical rule. When unclear, ask Li.

The cap is **soft** in that it triggers a rollover pass, not an instant rejection; it is **firm** in that the pass must run before the next new report lands. The 2026-04-28 cleanup left `reports/` empty — every active report had been absorbed into `tools-documentation/` topic files, per-repo `ARCHITECTURE.md`s, or code, and the rest were deleted. Default to deletion; extract only when the rationale has no other home.

## No stop-gaps — INTENTION applied

[`INTENTION.md`](INTENTION.md) explicitly rejects "minimum viable," "ship fast," "iterate later," and "time to market" as design-shaping framings. **The right shape now is worth more than a wrong shape sooner; unbuilding a wrong shape costs more than the speed it bought.**

This rules out a specific kind of agent suggestion: *"ship X as a stop-gap; we'll replace it with Y when Y lands."* That suggestion is the failure mode INTENTION names directly — it ships a known-wrong shape and accepts the cost of unbuilding it later in exchange for an immediate appearance of progress. Both the wrong shape AND the unbuild cost survive. The "speed" it bought is illusory.

**The recognition heuristic.** When you find yourself proposing two paths where one is "small / hand-written / quick" and the other is "the durable shape," that pairing IS the antipattern. The durable shape is the answer; the small/quick path is what to *not* propose. If the durable shape isn't ready yet, the work that's gated by it waits — that's a feature of the system, not a bug to route around.

**Worked example.** A hand-written `CompiledSchema` impl in `mentci-lib` (matching every signal record kind by name in a giant `match`) was proposed as a "stop-gap to unblock the workbench today, replaced when `signal-derive` lands." That proposal is wrong: the right shape is `signal-derive`. The constructor flow's kind palette waits on that crate. Building the stop-gap means writing code that gets thrown away, plus carrying its bugs and divergences from `signal-derive`'s eventual semantics in the meantime. **What looks like progress is regression** the moment you measure against INTENTION's "right shape now."

When this rule conflicts with a deadline pressure, the deadline pressure is wrong, not the rule (per INTENTION §"What we are not"). When this rule conflicts with another rule, INTENTION wins (per INTENTION §"The relationship with rules").

## Design reports — visuals, not code

Reports in [`reports/`](reports/) explain, propose, analyse, or summarise. Their medium is **prose + visuals** — ASCII diagrams, swimlanes, flowcharts, tables, dependency graphs. Implementation code (Rust `impl` blocks, function bodies, struct definitions with methods, full Nix derivations) belongs in skeleton code in the relevant repo, not in a report.

**Why:** code in a design doc goes stale the moment it lands and the real type drifts; readers can't tell whether the report's snippet or the repo's actual type is authoritative; visual shapes carry the same information without that freshness trap. A two-column "field name | type | meaning" table reads better than a struct definition and won't go silently wrong when the code evolves. A flow diagram reads better than a code walkthrough.

**Test:** if the report has more than a couple of lines that look like Rust / Python / Nix implementation, refactor those into a visual.

**The narrow allowance:** a few-line *sample* of the surface the design talks about — a snippet of a config file showing its shape, a one-line CLI invocation, a single field declaration to anchor a name — is fine. The rule is about implementation blocks, not about showing the shape of the thing the design is about. If the sample needs more than a few lines, it has stopped being a sample and become an implementation; move it.

This rule pairs with the [Skeleton-as-design over prose-as-design rule in criome/ARCHITECTURE §13.6](repos/criome/ARCHITECTURE.md#13--update-policy) — both push compiler-checkable shapes into the repos where the type system enforces them, leaving reports free to do what reports are good at: tracing decisions, mapping interactions, recording journeys.

When this rule is violated in an existing report: don't backfill all reports at once; fix it the next time a report is touched, or note it as a follow-up.

## Session-response style — substance goes in reports

If the agent's final-session response would be more than very minimal (a few lines), write the substance as a report (in [`reports/`](reports/)) and keep the chat reply minimal — a one-line pointer at the report. Two reasons: (1) the Claude Code UI is a poor reading interface; files are easier; (2) the author reviews responses asynchronously while the agent moves to next work, so the substance must be in a stable, scrollable, file-backed place.

Small reports are fine — the report doesn't have to be large. Acknowledgements, tool-result summaries, "done; pushed" confirmations don't need reports. Anything that explains, proposes, analyses, or summarises does.

**Use relative paths in reports.** When a report references files in sibling repos, link via [`../repos/<name>/...`](../repos/) (the workspace symlinks), not via GitHub URLs. The author reads in Codium and clicks links to open files locally; GitHub URLs break that flow. Absolute paths to `~/git/` also don't open in the editor.

## Components, not monoliths

The workspace is composed of **micro-components** — one capability per crate, per repo, per protocol. **sema** is the database. **criome** is the state-engine around it. **forge** is the executor (nix, filesystem, deploy). **prism** is the code emitter. **signal** is the wire protocol. **nexus** is the text front-end. Each lives in its own repo with its own `Cargo.toml`, `flake.nix`, and tests; each fits in a single LLM context window; each speaks to its neighbors only through typed protocols.

**Adding a feature defaults to a new crate, not editing an existing one.** The burden of proof is on the contributor (human or agent) who wants to grow a crate. They must justify why the new behavior is part of the *same capability* — not a new one. The default answer is "new crate."

**criome communicates; it never runs.** It does not spawn subprocesses, write files outside sema, invoke external tools, or link code-emission libraries. Effect-bearing work (nix builds, file writes, code emission, deployment) is dispatched as typed verbs to dedicated components. Bundling executor work into criome — or any communicator component — is the failure mode this rule closes.

The full case (historical canon from McIlroy's Unix philosophy through Parnas's information-hiding to Erlang's actor isolation; the catastrophic record of monolith collapse — Twitter, Facebook, Healthcare.gov, government COBOL; the modern LLM-context-window argument) lives in [`repos/tools-documentation/programming/micro-components.md`](repos/tools-documentation/programming/micro-components.md). Read it before pushing back on a "new crate, not new mod" requirement as overhead. The cost of plumbing is minutes; the cost of bundling is months or years of friction no agent and no team will resolve cleanly.

## Beauty is the criterion

Read [`repos/tools-documentation/programming/beauty.md`](repos/tools-documentation/programming/beauty.md) before pushing back on any rule below as "verbose" or "ceremonial." Beauty is not a luxury — it is the test of correctness. Ugly code is evidence that the underlying problem is unsolved.

The aesthetic discomfort *is* the diagnostic reading. When something feels ugly, slow down and find the structure that makes it beautiful — that structure is the one you were missing. Per Li (2026-04-27): *"Fuck ugliness and non-conciseness. Who knows how many people were put to death and tortured because someone wasn't concise and explicit enough."*

## Thinking discipline — every reusable verb belongs to a noun

Read [`repos/tools-documentation/programming/abstractions.md`](repos/tools-documentation/programming/abstractions.md) before writing free functions. The discipline applies to any language with method dispatch: behavior lives on types, not as floating verbs. The rule's purpose is to force the question "what type owns this verb?" — when the answer isn't obvious, the model of the problem isn't fully formed yet, and slowing down to find the noun is the load-bearing cognitive event.

Especially load-bearing for LLM-generated code, which lacks the tactile friction that makes humans economize on type creation: declaring a `struct` and declaring a `fn` cost the same number of tokens, so without the rule, agents default to the shorter shape and the noun never gets named.

The rule has a sharper version: the verb belongs to **the right** noun, not just any nearby noun. Adjacency of *types* is not the same as adjacency of *concerns* — see [`repos/tools-documentation/programming/abstractions.md`](repos/tools-documentation/programming/abstractions.md) §"The wrong-noun trap."

## Naming — full words by default

Identifiers are read far more than they are written. Cryptic abbreviations optimize for the writer (a few keystrokes saved) at the reader's expense (one mental lookup per occurrence). Canonical rule + full table of common offenders: [`repos/tools-documentation/programming/naming.md`](repos/tools-documentation/programming/naming.md).

**Default: spell every identifier as full English words.**

Examples (bad → good):

| bad | good |
|---|---|
| `lex` | `lexer` |
| `tok` | `token` |
| `ident` | `identifier` |
| `op` | `operation` (or specific: `assert_op`) |
| `de` | `deserializer` |
| `pf` | `pattern_field` |
| `ctx` | `context` (or specific: `parse_context`) |
| `cfg` | `config` (or `configuration`) |
| `addr` | `address` |
| `buf` | `buffer` |
| `tmp` | `temporary` (or — better — name what it holds) |
| `arr` | `array` (or — better — what it contains) |
| `obj` | (name what it actually is) |
| `params` | `parameters` |
| `args` | `arguments` |
| `vars` | `variables` |
| `proc` | `procedure` or `process` |
| `calc` | `calculate` |
| `init` | `initialize` |
| `repr` | `representation` |
| `gen` | `generate` or `generator` |
| `ser` / `deser` | `serialize` / `deserialize` |
| `fn` (in identifier) | `function` (the `fn` *keyword* is fine) |
| `impl` (in identifier) | `implementation` (the `impl` *keyword* is fine) |

### Permitted exceptions — tight, named, no others

1. **Loop counters in tight scopes (<10 lines).** `for i in 0..n { ... }` is fine. Beyond ~10 lines or nested, use descriptive names.
2. **Mathematical contexts** where the math itself uses the symbol. `x`, `y`, `z`, `theta`, `phi`, `lambda`, `n` for sample size, `p` for probability — only when the surrounding code or comment establishes the math context.
3. **Generic type parameters.** `T`, `U`, `V`, `K`, `E`. Use a descriptive name when the parameter has non-trivial semantic content.
4. **Acronyms that have passed into general English.** `id`, `url`, `http`, `json`, `uuid`, `db`, `os`, `cpu`, `ram`, `io`, `ui`, `tcp`, `udp`, `dns`. Spell them when ambiguous in context.
5. **Names inherited from std / well-known libraries.** `Vec`, `HashMap`, `Arc`, `Rc`, `Box`, `Cell`, `RefCell`, `Mutex`, `mpsc`, `regex`. Do not rename these; do *not* extend the abbreviation pattern to your own types.
6. **Domain-standard short names already documented in an ARCHITECTURE.md.** `slot`, `graph`, `node`, `edge`, `frame` are full words and need no exception. If a true short form is load-bearing in the schema, name it in ARCHITECTURE.md so the exception is explicit; otherwise spell it out.

### Rule of thumb (Martin / Linus, combined)

**Name length proportional to scope.** A 3-line loop counter can be `i`. A module-level type that appears across the codebase must spell itself out. A function parameter that lives for 50 lines must read as English.

### What this rule is NOT

- Not "verbose names everywhere" — `calculate_the_total_amount_of_items` is worse than `total_items`. The goal is *clear*, not *long*.
- Not "no acronyms ever" — see exception 4.
- Not "rewrite std" — see exception 5.

### How to apply when generating code

When generating new code: **spell identifiers as full English words by default.** When the surrounding code uses cryptic identifiers: do not propagate them into new code. Either rename (if rename is in scope) or use the full form for new identifiers and flag the inconsistency as a follow-up. Pattern-matching the local dialect is exactly the failure mode this rule exists to break.

### The "feels too verbose" anti-pattern

When a spelled-out name (`AssertOperation`, `Deserializer`, `PatternField`, `RelationKind`) "feels needlessly verbose" — that feeling is **not** a signal to shorten the name. It is a signal that the writer (human or agent) has been **taught wrong** by training data drawn from a 60-year-old culture of card-deck and 80-column-line constraints that no longer apply.

The full word reads as English. The abbreviation reads as ceremony to be decoded. The cost of mis-naming is paid every time the name is read; the benefit of saving three keystrokes is paid once. There is no contest.

Per Li (2026-04-27): *"You were taught wrong. We did a research on this, use it to make the docs convincing so agents don't slip back into writing all this fucking slop garbage-naming bullshit. I REALLY FUCKING HATE IT — IT'S SOOO UGLY!"*

When you catch yourself thinking "this name feels too long" or "this is unnecessarily ceremonial":

1. **Question the feeling.** It is almost certainly inherited prejudice, not informed judgment.
2. **Re-read the name as English.** Does `AssertOperation` read as English? (Yes.) Does `AssertOp` read as English? (No — it requires expansion.)
3. **Apply the rule.** The full English form wins unless the name falls in one of the six named exception classes above.

There is no exception class for "feels verbose." That feeling is the bug, not the criterion.

## Binary naming — `-daemon` suffix, full English

Long-running daemon binaries carry the `-daemon` suffix:
`nexus-daemon`, `criome-daemon`, `forge-daemon`. The library
half of the same crate keeps the bare name (`nexus`,
`criome`, `forge`) — `[lib] name = "nexus"` and `[[bin]] name = "nexus-daemon"`
in the same `Cargo.toml`. CLI binaries that the user
invokes interactively keep bare names (`nexus`, `criome`).

**Why:** `nexusd` / `criomed` are unix-folklore
abbreviations carrying no information beyond the suffix;
the empirical naming-research case applies. `nexus-daemon`
reads as English; `nexusd` requires expansion. It also
disambiguates the daemon binary from the CLI binary at PATH
level (`nexus` the CLI vs `nexus-daemon` the long-running
process).

**How to apply:** when introducing a new daemon, name its
binary `<name>-daemon`. The library half (if any) stays as
the bare name. Per-process stderr log tags also use the
suffix (`nexus-daemon: ready`).

## One-shot binaries — `<crate>-<verb>` for stdin/stdout glue

Beyond the daemon, several crates ship thin one-shot
binaries that wrap a single verb of the library code as a
stdin/stdout filter — useful for test pipelines, agent
harnesses, and debug scripts that need a parser/encoder
without running the daemon.

Naming convention: `<crate>-<verb>`, ~30 LoC each.

| Binary | What it does |
|---|---|
| `nexus-parse` | reads nexus text from stdin, writes length-prefixed signal Frames to stdout |
| `nexus-render` | reads length-prefixed Frames from stdin, writes rendered text to stdout |
| `criome-handle-frame` | reads a Frame, dispatches via `Daemon::handle_frame` against `$SEMA_PATH`, writes reply Frame |

**Why these are not test-only:** they expose useful
affordances of the library code (parser, renderer, handler)
as standalone tools — same shape as `gcc -E` (preprocess
only) shipping alongside `gcc`. Agents and scripts that
manipulate Frames or sema state directly, without a UDS
roundtrip, use them. They live as additional `[[bin]]`
entries in the existing crate, not in a separate test-tools
crate.

**When to add a new one:** when a verb of the library code
is genuinely useful as a filter primitive. Don't create
trivial passthrough binaries.

## Design-doc hygiene — state criteria positively

Avoid polluting design context with "do not use X" patterns. When a candidate is excluded — by constraint, preference, or past decision — **omit it silently** from forward-going context. Don't leave "X is bad because Y," "we ruled out X," or "not X" breadcrumbs across files.

**Why:** design docs and these instructions get loaded into every future agent's context. Every negative statement ("don't use X") costs context budget forever for zero ongoing value — X isn't going to be used; naming it just keeps it alive as a dead option. The accumulated negatives make docs unreadable and teach agents to think about what's excluded instead of what's being built.

Failure mode this closes: agent discovers a constraint ("must be Rust"), accumulates an elimination list ("so not Dolt, not Datomic, not XTDB, not …"), and every future doc re-states the elimination list. Within a few sessions the design context is mostly gravestones.

**How to apply:**
- State criteria **positively**: "must be Rust," not "Go is excluded because …"
- List candidates that **satisfy** the criteria. Silently drop the rest.
- When correcting an earlier wrong direction, the retraction is a one-time edit. Do not leave the retracted direction in the doc "for historical record" — git/jj log preserves it.
- If an excluded option keeps getting re-proposed by agents, that's a signal to **add a positive criterion that silently excludes it**, not to add a rule naming it.
- Applies to: reports, design docs, README-style docs, ARCHITECTURE.md files, AGENTS.md, CLAUDE.md.

The **rejected-framings** list in [criome/ARCHITECTURE.md §10](repos/criome/ARCHITECTURE.md) is the *only* place wrong frames are named, and only as one-line entries. Forensic narratives ("here's how this contamination crept in") do not become reports — their lessons land in §10 as one-liners.

## No version history in vision / design / architecture docs

Vision, design, and architecture docs describe what the system IS and what it's heading toward. They do NOT narrate prior abandoned approaches, scaffold history, or "why we restarted." That framing is a self-deprecation reflex that pollutes the doc and dates poorly.

A doc that opens with "this is the third try" is already weaker than a doc that just describes the system. Future readers don't need the lineage; current readers don't either. If a piece of context truly matters (e.g. "the validator pipeline is Datomic-inspired"), state the fact and cite the inspiration — don't frame it as "we abandoned X to get here."

How to apply:
- When rewriting a stale doc, extract the durable ideas + recast in current terms. Drop *all* meta-commentary about the rewrite itself.
- The same rule applies to commit messages of substance — describe the change, not the history of failed attempts.
- Project framing inside docs is "criome" — there is no `criomev3` / `v1` / `v2` distinction in published docs. Version markers are bookkeeping for agents, never for readers.

## Verify each parallel-tool result

When batching tool calls (parallel `Write`, `Edit`, `Bash`), scan each result block for errors **before** any follow-up step that depends on the results. The bundle returning is not the same as the bundle succeeding.

Failure mode: failed `Write` calls (typically the "must Read first" guard) don't show up in subsequent state until something else reads the file. By then the failure has cascaded — a wrong commit, a misled subagent, a published doc that doesn't match what the code reflects.

How to apply:
- After any parallel batch, look at every `<result>` block. If any says "File has not been read yet" or "Exit code N" or `<tool_use_error>`, fix that *before* moving on.
- Double-check by reading the file or running `git status` / `ls` if a write was meant to land.
- Especially load-bearing when the next step is committing, pushing, or spawning an agent that consumes the written file.
- When a `Write` fails on the must-Read-first guard, do the Read then redo the Write before continuing.

## Architectural defaults agents keep getting wrong

Three frames that agents (this one included) repeatedly slip into
when designing schema, bootstrap, or localization. All three are
rejected — see [criome/ARCHITECTURE.md §10.1](repos/criome/ARCHITECTURE.md)
for the canonical statement. Restated here as agent-facing rules so
the failure mode is visible during design:

- **Don't propose nexus files as a storage or seed mechanism.**
  Nexus is the text request language; its parser produces signal
  envelopes for criome. Files of nexus assertions
  (`bootstrap.nexus`, `genesis.nexus`, `kinds.nexus`, …) are a
  hack — a human typing assertions into a file and feeding them
  through a tool. Don't repeat the pattern for new concerns.
  Schema bootstrap is hand-coded in criome's init; domain data
  enters sema through Assert frames at runtime (mentci-egui,
  agents, scripts), never by criome reading a `.nexus` file off
  disk.
- **Don't put schema-identity strings in sema.** Kind, field, and
  variant identifiers are slot ids in per-kind indexes — never
  strings. Display names, per-language translations, and other
  human-facing text labels for schema records live in a separate
  localization store, distinct from sema and arca. Sema can hold
  user-data text content (e.g. a `Label` field on a domain
  record), but not schema-identity strings.
- **Don't default to "compile-time baked" for data.** When the
  architecture treats X as data (schema is data, kinds are data,
  localization is data), proposing "X lives as compile-time tables
  in the binary" is regressing to a config-style pattern that
  contradicts the principle. Data lives in a database (sema, the
  localization store, or a future store with its own bounded
  context); not in a const array or generated source file.

If the design you are sketching pulls toward any of these,
question it before proceeding. The defaults exist because they're
agent-natural (training data taught them); the architecture rules
out all three.

## Commit message style

Single line. Short. Lead with the repo name, then a short verb + scope. Optionally one short clause naming the change. No nested parens, no em-dashes, no Li-quote markers, no enumerated bullets. Detail belongs in the diff and the report, not the commit message.

Examples:

- `signal Slot<T> migration`
- `mentci report add 119`
- `criome reader for typed slots`
- `mentci AGENTS commit-style shortened`

If a single change touches multiple repos, each repo gets its own short commit. Don't roll them into one long compound message.

## Tooling

`bd` (beads) tracks short items (issues, tasks, workflow). Designs and reports go in files. See [reference_bd_vs_files](repos/tools-documentation/bd/basic-usage.md#bd-vs-files--when-each-is-the-right-home).

`bd prime` auto-runs at session start and gives current state.
