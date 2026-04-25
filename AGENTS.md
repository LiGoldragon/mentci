# Agent instructions

Tool references live in [`repos/tools-documentation/`](repos/tools-documentation/) — a symlink to `~/git/tools-documentation/` created on `nix develop` / direnv entry.

Start there for: cross-project rules (jj workflow, always-push, Rust style — see [rust/style.md](repos/tools-documentation/rust/style.md)) in [`repos/tools-documentation/AGENTS.md`](repos/tools-documentation/AGENTS.md), and curated daily-use docs for jj, bd, dolt, nix under [`repos/tools-documentation/<tool>/basic-usage.md`](repos/tools-documentation/).

## Architecture

Canonical architecture: [`docs/architecture.md`](docs/architecture.md). Read it first. Design history and decision records are in [`reports/`](reports/).

**Workspace manifest**: [`docs/workspace-manifest.md`](docs/workspace-manifest.md) lists every repo under `~/git/` with its status. `devshell.nix`'s `linkedRepos` mirrors the CANON + TRANSITIONAL entries.

### Documentation layers — strict separation

| Where | What | Example |
|---|---|---|
| [`docs/architecture.md`](docs/architecture.md) | **Prose + diagrams only.** No code. High-level shape, invariants, relationships, rules. | "criomed owns sema; lojixd owns lojix-store; text crosses only at nexusd" |
| [`reports/NNN-*.md`](reports/) | **Concrete shapes + decision records.** Type sketches, record definitions, message enums, research syntheses, historical context. | `Opus { … }` full rkyv sketch |
| the repos themselves | **Implementation.** Rust code, tests, flakes, Cargo.toml. | `nexus-schema/src/opus.rs` |

If a layer rule is violated, rewrite: move type sketches out of `docs/architecture.md` into a report; move runnable code out of reports into the appropriate repo. The architecture stays slim so it remains readable in one pass.

**No report links inside `docs/architecture.md`.** Cross-references go *into* architecture from reports, not *out of* architecture to reports. Reading lists, decision histories, type-spec details all live in reports or in `docs/workspace-manifest.md` — never inline in architecture.

When architecture changes, update `docs/architecture.md` first, then update the affected repos, then write a report only if the decision carries a journey worth recording. Per the project rule "delete wrong reports, don't banner them," superseded reports are deleted — they do not stay as banner-wrapped relics.

### Inclusion/exclusion rule — HARD

**If a repo is not listed as CANON or TRANSITIONAL in the workspace manifest, do not edit its source or docs.** Agents that drift outside the manifest corrupt repos that are either superseded, archived, or outside scope. To add a new canonical repo: update the manifest and `devshell.nix`, write a report, commit.

### AGENTS.md / CLAUDE.md pattern

Across all canonical repos we follow: **`AGENTS.md` holds the real content; `CLAUDE.md` is a one-line shim reading "See [AGENTS.md](AGENTS.md)."** This way Codex (which reads `AGENTS.md`) and Claude Code (which reads `CLAUDE.md`) converge on a single source of truth. When creating or restructuring a repo, keep this pattern.

MVP goal: **self-hosting** — write the system's own source as records in the sema database; rsc projects those records to `.rs` files; rustc compiles them; the new binary reads and extends its own database.

An **opus** is the database's compilation-unit term — one opus compiles to one artifact (library or binary). Corresponds to one Rust crate on the filesystem side.

Write discoveries in [`reports/`](reports/) or in tools-documentation as topic files, don't scatter them across the repo root.

## Session-response style — substance goes in reports

If the agent's final-session response would be more than very minimal (a few lines), write the substance as a report (in [`reports/`](reports/)) and keep the chat reply minimal — a one-line pointer at the report. Two reasons: (1) the Claude Code UI is a poor reading interface; files are easier; (2) the author reviews responses asynchronously while the agent moves to next work, so the substance must be in a stable, scrollable, file-backed place.

Small reports are fine — the report doesn't have to be large. Acknowledgements, tool-result summaries, "done; pushed" confirmations don't need reports. Anything that explains, proposes, analyses, or summarises does.

## Tooling

`bd` (beads) tracks short items (issues, tasks, workflow). Designs and reports go in files. See [reference_bd_vs_files](repos/tools-documentation/bd/basic-usage.md#bd-vs-files--when-each-is-the-right-home).

`bd prime` auto-runs at session start and gives current state.
