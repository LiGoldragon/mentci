# Agent instructions

Tool references live in [`repos/tools-documentation/`](repos/tools-documentation/) — a symlink to `~/git/tools-documentation/` created on `nix develop` / direnv entry.

Start there for: cross-project rules (jj workflow, always-push, Rust style — see [rust/style.md](repos/tools-documentation/rust/style.md)) in [`repos/tools-documentation/AGENTS.md`](repos/tools-documentation/AGENTS.md), and curated daily-use docs for jj, bd, dolt, nix under [`repos/tools-documentation/<tool>/basic-usage.md`](repos/tools-documentation/).

## Architecture

This repo (mentci) is the **dev environment**. The project being built is **criome**.

1. Read [`ARCHITECTURE.md`](ARCHITECTURE.md) at this repo's root for the dev-environment shape.
2. Then read [criome's `ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) — the canonical reference for the engine being built (sema, nexus, lojix, criome, nexus, lojix, rsc, lojix-store, signal, …).

Design history and decision records are in [`reports/`](reports/).

**Workspace manifest**: [`docs/workspace-manifest.md`](docs/workspace-manifest.md) lists every repo under `~/git/` with its status. `devshell.nix`'s `linkedRepos` mirrors the CANON + TRANSITIONAL entries.

### Documentation layers — strict separation

| Where | What | Example |
|---|---|---|
| [`criome/ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md) | **Project-wide canonical.** Prose + diagrams only. No code. High-level shape, invariants, relationships, rules of the engine being built. | "criome owns sema; lojix owns lojix-store; text crosses only at nexus" |
| [`mentci/ARCHITECTURE.md`](ARCHITECTURE.md) | **This dev environment.** Workspace conventions, role, layout. Points at criome for the project itself. | "mentci is the dev workshop; long-term it becomes the universal UI" |
| `<repo>/ARCHITECTURE.md` | **Per-repo bird's-eye view.** This repo's role, boundaries (owns / does not own), code map, status. Points at criome for cross-cutting context — does *not* duplicate it. Per the matklad ARCHITECTURE.md convention. | `lojix-store/ARCHITECTURE.md` "owns the `~/.lojix/store/` layout + index DB" |
| [`reports/NNN-*.md`](reports/) | **Concrete shapes + decision records.** Type sketches, record definitions, message enums, research syntheses, historical context. | `Opus { … }` full rkyv sketch |
| the repos themselves | **Implementation.** Rust code, tests, flakes, Cargo.toml. | `nexus-schema/src/opus.rs` |

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

MVP goal: **self-hosting** — write the system's own source as records in the sema database; rsc projects those records to `.rs` files; rustc compiles them; the new binary reads and extends its own database.

An **opus** is the database's compilation-unit term — one opus compiles to one artifact (library or binary). Corresponds to one Rust crate on the filesystem side.

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

The cap is **soft** in that it triggers a rollover pass, not an instant rejection; it is **firm** in that the pass must run before the next new report lands. Trim passes have happened on 2026-04-25 (18 → 6 reports across three sub-passes); precedent for what each decision looks like is in `reports/076` §5 trim ledger.

## Session-response style — substance goes in reports

If the agent's final-session response would be more than very minimal (a few lines), write the substance as a report (in [`reports/`](reports/)) and keep the chat reply minimal — a one-line pointer at the report. Two reasons: (1) the Claude Code UI is a poor reading interface; files are easier; (2) the author reviews responses asynchronously while the agent moves to next work, so the substance must be in a stable, scrollable, file-backed place.

Small reports are fine — the report doesn't have to be large. Acknowledgements, tool-result summaries, "done; pushed" confirmations don't need reports. Anything that explains, proposes, analyses, or summarises does.

## Tooling

`bd` (beads) tracks short items (issues, tasks, workflow). Designs and reports go in files. See [reference_bd_vs_files](repos/tools-documentation/bd/basic-usage.md#bd-vs-files--when-each-is-the-right-home).

`bd prime` auto-runs at session start and gives current state.
