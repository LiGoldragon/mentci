# Agent instructions — workspace

You **MUST** read AGENTS.md at `github:ligoldragon/lore` — the
workspace contract. The rules below are workspace-repo-specific
carve-outs only.

## Repo role

This repo is the **dev environment + meta-deploy aggregator**
for the sema ecosystem. The project being built is **criome**
(the engine). The user-facing interaction surface is the
**mentci** family — `mentci-lib` (application logic) +
`mentci-egui` (GUI shell) — separate repos, distinct concerns
from this meta-repo.

This repo hosts:
- `docs/workspace-manifest.md` —
  every repo under `~/git/` with its CANON / TRANSITIONAL /
  SHELVED status. `devshell.nix`'s `linkedRepos` mirrors the
  CANON + TRANSITIONAL entries.
- `reports/` — decision records and design syntheses.
- The `repos/` symlink directory created on `nix develop` /
  direnv entry, exposing every workspace repo as a sibling for
  cross-repo reading + editing.

For implementation detail of the workspace meta-repo itself:
see `ARCHITECTURE.md` at this repo's root.

---

## Reports — hygiene

Reports state the current correct frame. When the architecture
moves, the report is rewritten to describe the new state — the
prior framing disappears from the doc; jj/git history preserves
the path.

When a previous report's premise turns out misaligned, replace
the report with a clean successor; the successor stands alone.
Forensic narratives ("here's how this came to be") belong in
commit messages and bd memories, not in reports.

---

## Reports — rollover at the soft cap

**Soft cap: ~12 active reports** in `reports/`. When
the count exceeds this, run a rollover pass before adding the
next report. For each existing report, decide one of:

1. **Roll into a new consolidated report.** Multiple reports
   covering the same evolving thread fold into a single
   forward-pointing successor. The successor supersedes the old
   reports; the old ones are deleted.
2. **Implement.** When the report's substance can be expressed
   as architecture (criome's ARCHITECTURE.md, a per-repo
   ARCHITECTURE.md, skeleton-as-design code, or an AGENTS.md
   rule), move it to the right home and delete the report.
3. **Delete.** When the report's content is already absorbed
   elsewhere or its premise has changed, delete it.

The choice is made by reading each report against the author's
intent — no mechanical rule. When unclear, ask Li.

The cap is **soft** in that it triggers a rollover pass; it is
**firm** in that the pass must run before the next new report
lands. Default to deletion; extract only when the rationale has
no other home.
