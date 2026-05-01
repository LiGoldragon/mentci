# 125 — Session handoff: end-of-day 2026-05-01

*Compact handoff for the next session. Captures where we are after
the 2026-04-30 → 2026-05-01 cleanup arc + where to pick up.
Lifetime: until the next session reads it and either supersedes
or deletes.*

---

## 0 · Read first (in order)

1. `lore/INTENTION.md` — what the project is for.
2. `criome/ARCHITECTURE.md` — what the engine IS.
3. `lore/AGENTS.md` — the workspace contract (now canonical for
   cross-project agent rules).
4. `workspace/AGENTS.md` — workspace-meta-repo carve-outs.

---

## 1 · What landed in this session

**Renames (github + local + path refs):**

- `tools-documentation` → `lore`
- `mentci` (the meta-repo) → `workspace`
- `lore/INTENTION.md` (was `mentci/INTENTION.md` — moved to lore root)

**Two new rules in `lore/AGENTS.md`:**

- *Cross-references to workspace files* — no deep github URLs;
  use prose ("criome's `ARCHITECTURE.md`") or `github:ligoldragon/<repo>`
  for repo-level pointers.
- *Positive framing only* — state what IS; rejected-framings
  sections, "don't do X" rules, and "we used to think A" history
  all live in git/jj history instead.

**criome/ARCHITECTURE.md §10 reframed positively:**

- §10.1 *Categories of records* (machina + open category names)
- §10.2 *Sema's string discipline* (slot ids; localization in a
  separate store)
- §10.3 *Bootstrap and runtime data flow* (criome's init builds
  bootstrap kinds; signal Frames carry domain records at runtime)
- §10.4 *Responsibilities table* (was 10.2)
- §10 Rules table reworded — every "no X" / "never X" entry
  rewritten as positive shape

**All canonical AGENTS.md files** trimmed to thin shim:

```
# Agent instructions — <repo>

You **MUST** read AGENTS.md at `github:ligoldragon/lore` —
the workspace contract.

## Repo role
<short>

## Carve-outs worth knowing
<short list>
```

**All canonical CLAUDE.md files**: `You **MUST** read AGENTS.md.`

**CriomOS-cluster meta-repo setup**: CriomOS now has the same
`repos/` symlink farm + multi-root `.code-workspace` pattern as
workspace. lore is symlinked at `CriomOS/repos/lore`; cluster
siblings (CriomOS-home, CriomOS-emacs, horizon-rs) and adjacent
repos (lojix-cli, brightness-ctl, clavifaber, goldragon) follow.
`devshell.nix` rebuilds the symlinks on entry.

**`mentci-tools` removed from canonical scope**: workspace/flake.nix
(input dropped), workspace/devshell.nix (`pkgs.beads` + `pkgs.dolt`
direct), workspace/flake.lock (5 nodes removed), CriomOS/devshell.nix,
CriomOS-home (cli-tools.nix comment), lore docs (annas / linkup /
substack basic-usage now reference "the home profile").

**bd audit across all canonical repos** — 6 stale issues closed
(2 superseded-because-repo-exists, 3 superseded-because-rejected-
nexus-files-pattern, 1 duplicate); 6 retitled to use the new
vocabulary / paths. See §3.

**Two reports survive the rollover:**

- `122-schema-bootstrap-architecture-2026-04-30.md` — polished
  current view of the schema-as-records architecture.
- `124-schema-architecture-brainstorm-raw-2026-05-01.md` — the
  live exploration with open questions.

This handoff is `125-…`.

---

## 2 · The live architectural question

**Localization-store ownership** — see `reports/124` §3.

Constraints (all firm):
- Sema is string-free at the schema layer.
- Nexus is messages, not storage.
- Compile-time-baked schema descriptors are the wrong shape for
  data.

The store's owner shape is open: separate criome-engine instance,
dedicated `localization-daemon`, library linked into nexus + mentci,
or option 4. This blocks mentci-egui label rendering, nexus per-
language text, and schema authoring tools.

When this lands, `reports/124` folds away into 122 / criome
ARCHITECTURE.md.

---

## 3 · bd state, post-audit

**Live, ready to start (P1):**

```
mentci-next-m5m  P1  Add Kind + Field + Variant + TypeExpression
                     + KindShape + Localization record kinds
                     to signal
mentci-next-4v6  P1  signal-derive direction post rejected-frames
                     (keep / repurpose / retire — open decision)
mentci-next-wd3  P1  Create process-manager crate
mentci-next-ef3  P1  Self-hosting "done" moment — concrete first
                     feature
```

**Live, P2 (downstream):**

```
mentci-next-149  P2  mentci-lib CompiledSchema queries sema for
                     schema records
mentci-next-7tv  P2  per-kind sema tables (replaces 1-byte
                     discriminator) — INFRASTRUCTURE for the
                     deterministic per-kind slot indexes
mentci-next-7dj  P2  Cross-repo wiring (flake input pattern)
mentci-next-0tj  P2  Implement prism records-to-Rust projection
mentci-next-zv3  P2  M6 bootstrap demonstration
mentci-next-4jd  P2  M2-remainder: method-body layer in signal
mentci-next-8ba  P2  M3: sema redb wrapper
```

Other repos (nexus, sema, lojix-cli, CriomOS, etc.) carry their
own bd issues — `bd list --status=open` from each for their state.

---

## 4 · Where to pick up

Two natural starting points:

1. **Decide the localization-store owner** (reports/124 §3).
   Unblocks the mentci-egui / nexus per-language work and lets
   the schema-authoring tooling land.
2. **Start `mentci-next-m5m`** — landing the bootstrap kinds in
   signal. This is structurally independent of the localization-
   store decision (Localization is one of the bootstrap kinds,
   but its *storage location* is separate from its *type
   definition*). Could proceed in parallel.

`mentci-next-7tv` (per-kind sema tables) is infrastructure for
m5m's deterministic slot-index requirement; may want to land
first.

---

## 5 · Where to look

Canonical docs:
- `lore/AGENTS.md` — workspace contract.
- `lore/INTENTION.md` — what the project is for.
- `criome/ARCHITECTURE.md` — engine architecture.
- `workspace/docs/workspace-manifest.md` — repo statuses.
- Per-repo `ARCHITECTURE.md` — repo niches.

Code:
- `signal/src/` — current hand-defined record kinds (Node, Edge,
  Graph, Principal, Theme, Layout, …) plus the Slot machinery.
  Eventually most of this becomes prism-emitted; the bootstrap
  set (Kind, Field, Variant, TypeExpression, KindShape, Primitive,
  Localization, Language) remains hand-written.
- `criome/src/engine.rs` — `State::handle_*` dispatch where the
  new bootstrap-kind constructors will land.
- `sema/src/` — the redb-backed records DB; per-kind tables are
  open work (`mentci-next-7tv`).

---

## 6 · One-paragraph state

The 2026-04-30 → 2026-05-01 arc settled the polished architecture
(reports/122) and a set of workspace-wide rules (lore/AGENTS.md +
criome §10 reframings). The single live decision is who owns the
localization store; everything downstream — UI label rendering,
per-language text, mentci's schema-authoring surface — depends on
that. Independent of it, `mentci-next-m5m` (bootstrap kinds in
signal) and `mentci-next-7tv` (per-kind sema tables) can
progress; the rest of the bd chain keys off those plus the
localization decision.

---

*End report 125.*
