# Report 005 — resume state after bd backfill

**⚠ Superseded** — this is a point-in-time snapshot from early
April 2026, preserved for the decision-journey record. The
open questions it listed have since been re-distributed:

- P1/P2 design decisions → individual bd issues in each repo's
  bd (e.g. `sema-5d3` opus identity, `nexus-schema-5rw` cross-
  ref representation, `nexus-schema-wq3` Callable split,
  `mentci-next-ef3` capstone feature).
- Code-sharing direction → [reports/011](011-code-sharing-and-deferred.md).
- Nexus Tier-1 syntax → [reports/012](012-nexus-vision.md).
- Architectural layering → [reports/007](007-nota-nexus-layer-split.md).

Do not treat the task-list below as current work. Consult the
reports above or `bd list --status=open` in the relevant repo
for canonical live state.

---

Snapshot after the context reset. Supersedes the deleted
`SESSION-HANDOFF.md`. Delete or archive this report once the
decisions below are resolved.

---

## 1. Where we are

Infrastructure is ready. Eight repos on disk, all symlinked into
[`repos/`](../repos/) via [`devshell.nix`](../devshell.nix):

- Spec-only: `nexus` (grammar, [README](../repos/nexus/README.md))
- Rust libraries: `nexus-serde`, `nexus-schema`, `sema`
- Rust binaries: `nexusd`, `nexus-cli`, `rsc`
- Workspace: `mentci-next`, `tools-documentation`

Each has its own `.beads/` db (embedded-dolt). bd works. All
remotes point to `github.com:LiGoldragon/<name>`.

Design record: reports [001](001-migration-doc-reading.md)
(orientation), [002](002-sema-db-architecture.md) (database),
[003](003-mvp-implementation-plan.md) (MVP plan),
[004](004-sema-types-for-rust.md) (Rust types). §6 "Open
questions" sections stripped — those now live in per-repo bd.

Implementation: **M1 done, M2 partial.** nexus-schema contains
the data-definition surface (17 Name newtypes, 11 hash-ID
newtypes, 7 record modules, dual rkyv+serde derives, cycles
broken via Path-3 hash-ID indirection). **M2 remainder:
method-body layer.**

---

## 2. bd state — decisions waiting for you

Twelve design questions tracked across repos, priority-sorted:

**P1 — blockers / shape-setting:**

- `sema-5d3` — **Opus identity: hash, name, or both?** Blocks
  rsc export surface and nexusd addressing.
- `mentci-next-ef3` — **Self-hosting capstone: pick the first
  DB-edited feature.** Defines "done" for M6.

**P2 — needed before the next code lands:**

- `nexus-schema-5rw` — Cross-declaration refs: Name,
  NamedById, or both. Hits M2 method-body layer.
- `nexus-schema-wq3` — Callable split: three variants or one
  with flags. Hits M2 method-body layer.
- `nexus-schema-tu8` — Rust subset for self-hosting: must-have
  vs deferrable items (sizes rsc scope).
- `nexusd-chm` — Editing granularity API (field vs subtree).
- `nexusd-k6x` + `nexus-cli-tqg` — Wire format for the daemon
  socket (framing, errors, correlation).
- `rsc-cie` — rsc temp directory location + cleanup ownership.
- `mentci-next-7dj` — Cross-repo flake-input wiring pattern.

**P3 — tracked, re-evaluate later:**

- `sema-g2a` — String storage (inline vs companion table);
  current decision: inline, revisit when M3 has real data.
- `rsc-zxf` — Rust edition target.

MVP milestones (already in `mentci-next` bd, unchanged):
`4jd` (M2 method bodies), `8ba` (M3 sema wrapper),
`rgs` (M4 nexusd), `0xk` (nexus-serde), `0tj` (rsc),
`zv3` (M6 bootstrap).

---

## 3. Suggested order to resume

Two of the P1s unblock everything else. I'd tackle them first,
then M2-remainder in the same stretch since it depends on the
P2 nexus-schema questions.

1. **Decide opus identity** (`sema-5d3`). The content-addressed
   vs name-based split touches rsc, nexusd, and CLI; resolving
   it first shapes every surface that follows. My read: both,
   with name→hash resolution at query time, since pure content
   addressing loses discoverability and pure naming loses
   stability.
2. **Call the capstone feature** (`mentci-next-ef3`). Something
   small enough to isolate (e.g. add a new `list-opuses`
   subcommand to `nexus-cli`) but big enough that adding it via
   DB edits exercises rsc + rustc + re-run.
3. **Resolve the two nexus-schema M2 gatekeepers**
   (`5rw` cross-ref representation, `wq3` Callable split), then
   port the method-body layer (`mentci-next-4jd`).
4. **M3 — sema redb wrapper** (`mentci-next-8ba`). First real
   records-on-disk; strings-storage decision re-opens here with
   data to back it.
5. Wire format decision (`nexusd-k6x`) before M4 begins.

---

## 4. Tensions worth probing early

Not bd-tracked yet because they may dissolve once tested — but
worth keeping in the back of the mind:

- **rkyv zero-copy + schema evolution.** Zero-copy reads
  require exact stored bytes. Adding a field to a record type
  means old records don't have it. Likely constrains schema
  changes to append-only, or needs a migration layer outside
  the zero-copy path. Easy to check: write a Type record, add
  a field to `Type`, reread. Surface the failure mode before
  M3 locks in an assumption.
- **Content-addressing + mutable names.** "Current version of
  module Foo" has a moving hash. An opus-name registry solves
  it, but the registry itself is mutable shared state — where
  does it live and who writes to it? Ties directly to
  `sema-5d3`.
- **Single-writer redb vs ractor.** redb serializes writers;
  ractor expects many actors. Fine at MVP scale; worth a note
  if M4 shows message-queue backpressure at the writer actor.

---

## 5. What I did this session (for the record)

- Created the 12 design-question bd issues listed in §2.
- Stripped `§6 Open questions` from reports 002, 003, 004.
- Deleted `SESSION-HANDOFF.md`.
- Did not commit anything — left everything in working tree for
  your review.

Uncommitted changes across repos are either `.beads/` init
artifacts from prior sessions, or (in `mentci-next`) the above
report edits + this file.
