# 080 — Code-age audit 2026-04-25

> Flags any code older than 2 days (created 2026-04-23 or earlier) for
> review per Li 2026-04-25: *"anything older than 3 days is highly
> suspect. anything over a week is probably wrong."*

## Method note (read first)

`git log --reverse --diff-filter=A` returns the date a path **first
appeared** in the repo, not the date the current content of that
path was written. Several repos in this audit (sema, lojix-store,
horizon-rs) carry forward path names from prior incarnations that
have since been completely rewritten. To avoid false alarms, the
"OLDER" column below counts files whose **most recent commit on the
path** is also 2026-04-22 or earlier. Where the most-recent commit
is 2026-04-23+, the file is counted in the bucket of its most-recent
commit and called out in prose if its semantics are stale despite
the timestamp.

## Summary

| Repo | TODAY (04-25) | YESTERDAY (04-24) | 2 DAYS (04-23) | OLDER | Verdict |
|---|---|---|---|---|---|
| nexus | 17 | 1 | 4 (scaffold-only edited later) | 0 | clean — every 04-23 file has been edited 04-24 or 04-25 |
| nexus-cli | 1 | 1 | 7 | 0 | clean — scaffold; src/main.rs is 9-line stub |
| nexus-serde | 1 | 0 | 8 | 0 | clean — src/lib.rs rewritten 04-24 as thin façade |
| nota | 1 | 0 | 4 | 0 | minimal — README/flake/LICENSE only; no src |
| nota-serde | 1 | 1 | 7 | 0 | clean — src/lib.rs rewritten 04-24 as thin façade |
| nota-serde-core | 1 | 14 | 0 | 0 | clean — kernel crate, all from 04-24 |
| lojix-cli | 1 | 16 | 0 | 0 | clean |
| lojix-store | 5 | 1 | 1 (Cargo.toml stale-frame) | 5 (paths only — content rewritten 04-23+) | mixed — see §3 |
| sema | 0 | 1 | 5 | 3 (path lineage 2019/2020; content rewritten 04-23) | flag Vision.md content |
| rsc | 0 | 1 | 6 | 0 | scaffold-only; 9-line stub main; uses anyhow (style violation) |
| horizon-rs | 1 | 1 | 14 | 12 (path lineage 2025-03 to 2025-06; current bodies from 04-23) | mixed — see §3 |
| CriomOS | 8 | 5 | 38 | 0 | first commit 2026-04-23 — repo is born-fresh |
| CriomOS-emacs | 1 | 1 | 11 | 0 | first commit 2026-04-23 — clean |
| CriomOS-home | 1 | 1 | 28 | 0 | first commit 2026-04-23 — clean |
| tools-documentation | 1 | 0 | 10 | 1 (README.md 04-22) | clean — docs |

Overall: **no repo contains pre-2026-04-22 source code that survived**.
Every Rust source file currently in HEAD was either authored or fully
rewritten on or after 2026-04-23. The 2019/2020/2025 first-add dates
in sema, lojix-store, and horizon-rs are git-history artifacts of
repos that were repurposed-in-place rather than re-initialized.

## Suspect content (flagged for review despite recent commit dates)

### `/home/li/git/sema/reference/Vision.md` — 2026-04-23 (143 LoC)

- One-sentence guess: aspirational "Sema future state" doc moved
  during the 04-23 sema-repurpose; references the retired aski
  pipeline (corec, askicc, askic, veric, domainc, semac).
- Suggested action: **review**. The pipeline names are stale per
  the post-rename architecture (the active pipeline is nexus →
  signal → criomed; lojix is the compiler; corec/askicc/etc. don't
  exist as repos any more). Either rewrite to use current names or
  delete and let `criome/ARCHITECTURE.md` carry the vision.

### `/home/li/git/sema/Cargo.toml` + `flake.nix` + `src/lib.rs` — path-lineage 2019; content 2026-04-23

- One-sentence guess: 10-line stub `lib.rs` doc-comment + redb +
  rkyv + blake3 + thiserror deps; commit message says "scaffold —
  pseudo-sema for now".
- Suggested action: **keep**. Content is fresh (2026-04-23), the
  ancient first-add date is misleading. Body is a stub awaiting
  real implementation.

### `/home/li/git/rsc/src/main.rs` + `Cargo.toml` — 2026-04-23 scaffold

- One-sentence guess: 9-line stub `fn main() -> anyhow::Result<()>`
  + Cargo.toml depending on `anyhow` and `clap` derive.
- Suggested action: **review**. Two issues, neither old-code:
  (1) `anyhow` is a `~/git/tools-documentation/rust/style.md`
  violation (lojix-cli, nexusd, nexus-cli all moved off anyhow on
  2026-04-24); (2) clap 4 derive may or may not be wanted given
  rsc is described as a projector library + capstone tool.
  Same-cohort scaffolds (nexus, nexus-cli) already had their
  anyhow → thiserror conversion on 04-24; rsc was missed.

### `/home/li/git/lojix-store/Cargo.toml` — 2026-03-21 first-add, last-touched 2026-04-25

- The file path has continuous lineage from the original
  criome-store crate (2026-03-21). Current content is fresh: deps
  swept on 2026-04-24, bodies-fill on 2026-04-25.
- Suggested action: **keep**. Path-age is artifact only.

### `/home/li/git/horizon-rs/lib/src/lib.rs` + 12 sibling source files — path-lineage 2025-04-18, content 2026-04-23

- One-sentence guess: schema crate scaffolded in April 2025;
  current modules (address, error, io, machine, magnitude, name,
  proposal, pub_key, species + cluster/horizon/node/user) were
  *added on 2026-04-23* in commit "horizon-rs scaffold horizon-lib
  + horizon-cli phase 1". Older path-history is from a different
  shape that was wiped.
- Suggested action: **keep**. Per its commit log, that 2026-04-23
  scaffold "deletes legacy stub modules (cluster/criosphere/
  horizon/nix/node/request/user); now produces enriched horizon
  TOML matching DESIGN.md schema". Then 2026-04-23/24 sweeps moved
  TOML → nota and added JSON output. Content is fresh.

### `/home/li/git/tools-documentation/README.md` — 2026-04-22

- One-sentence guess: top-level README for the tools-documentation
  repo; one day older than the other docs.
- Suggested action: **keep**. Off-scope (documentation-only).

## Per-repo detail

### nexus  (CANON daemon, renamed from nexusd 2026-04-25)

All 22 source files. The 4 files dating 2026-04-23 (Cargo.toml,
flake.nix, README.md, rust-toolchain.toml) were all touched
2026-04-25 in the rename sweep. src/main.rs (04-23 first-add) was
touched 04-25 in the same sweep (`use nexusd::error::Result` →
`use nexus::error::Result`). src/error.rs added 04-24. New
client_msg/* tree all 04-25. spec/* all 04-25 (newly consolidated
from the former nexus-spec repo). **Verdict: clean**.

### nexus-cli  (CANON CLI, renamed from lojix-cli's old role)

9-file scaffold. src/main.rs (04-23) is a 12-line stub with
correct module-doc; src/error.rs added 04-24. **Verdict: clean
scaffold awaiting body-fill**.

### nexus-serde  (CANON serde façade, Nexus dialect)

src/lib.rs first-added 04-23, but rewritten on 04-24 as a
~70-LoC thin façade over nota-serde-core (per "nexus-serde
refactor" commit; replaced 1920 LoC of forked lexer/ser/de). Tests
in tests/nexus_wrappers.rs (04-23) preserved as consumer-API
guard. **Verdict: clean — current content all 04-24**.

### nota  (CANON family-namespace umbrella)

Only README/flake/LICENSE/.beads/ARCHITECTURE — no Rust source.
**Verdict: clean docs-only repo**.

### nota-serde  (CANON serde façade, Nota dialect)

Same shape as nexus-serde. src/lib.rs first-added 04-23, rewritten
04-24 as thin façade. tests/smoke.rs added 04-24. **Verdict:
clean**.

### nota-serde-core  (CANON shared kernel)

All 14 files born 2026-04-24. ARCHITECTURE.md added 04-25.
**Verdict: clean — newest crate in the family**.

### lojix-cli  (CANON deploy CLI, renamed from lojix 2026-04-25)

All 16 source files born 2026-04-24 in a clean scaffold (was the
package-rename of "lojix" → "lojix-cli"). rust-toolchain.toml +
ARCHITECTURE.md from 04-25. **Verdict: clean**.

### lojix-store  (CANON content-addressed FS)

The four "scaffold" Rust source files (lib/error/hash/Cargo.toml)
have first-add dates ranging 2026-03-21 to 2026-04-06 due to the
criome-store → lojix-store rename and several "reset" commits.
Current content is from the 2026-04-25 "skeleton — types + traits +
todo!() bodies" commit followed by the 04-25 body-fills for
hash.rs and layout.rs. Non-source files (CLAUDE.md 04-22, flake.nix
04-06, AGENTS.md 04-24) all sit in the modern half of the sweep.
**Verdict: clean — path-age is git-rename artifact**.

### sema  (CANON record DB)

Path lineage to 2019. Full repurpose on 2026-04-23 (single commit
"sema repurpose: retire old aggregator role; scaffold library").
src/lib.rs is a 10-line doc-comment stub (current content 04-23).
Cargo.toml + flake.nix + rust-toolchain.toml all repurposed 04-23.
**One real flag**: `reference/Vision.md` (143 LoC, 04-23) describes
a retired pipeline — corec, askicc, askic, veric, domainc, semac —
that no longer maps to the current architecture. See suspect-list
above.

### rsc  (CANON projector)

7-file scaffold born 04-23. src/main.rs is a 9-line stub. Two
flags: (1) **anyhow dep is a style violation** that the same-cohort
scaffolds (nexus, nexus-cli) already corrected on 04-24 — rsc was
missed; (2) clap derive 4 was added before any CLI surface is
known. Otherwise clean. **Verdict: review the dep list**.

### horizon-rs  (CANON typed schema + projector)

Path lineage to 2025-03. Full content rewrite landed in single
2026-04-23 commit "horizon-lib + horizon-cli phase 1" which deleted
the 2025-vintage stub modules and built the current address/cluster/
horizon/io/machine/magnitude/name/node/proposal/pub_key/species/user
shape. Subsequent 04-23/04-24 sweeps moved wire format from TOML
to nota and added JSON output. Cargo.toml/lib/Cargo.toml have
2025-03/2025-04 first-add but current content is post-2026-04-23.
**Verdict: keep — path-age is artifact**.

### CriomOS  (off-scope OS distro)

68 files. First commit on this repo was 2026-04-23; no pre-04-23
content exists. New, growing actively. **Verdict: clean**.

### CriomOS-emacs / CriomOS-home  (off-scope user environments)

Both born 2026-04-23. **Verdict: clean**.

### tools-documentation  (off-scope tool docs)

12 docs, all 04-23 except README.md 04-22 and rust/nix-packaging.md
04-25. **Verdict: clean**.

## Cross-cutting observations

1. **No surviving abandoned code**. Every concerning first-add
   timestamp resolved to a path-rename artifact rather than a
   forgotten-implementation artifact. The 2026-04-23 "scaffold
   day" generated all the canonical Rust crates simultaneously,
   and most have already been touched again on 04-24 or 04-25.

2. **rsc is the lone scaffold-day file that fell off the
   anyhow-removal sweep.** On 2026-04-24 the cohort lojix-cli +
   nexusd (now nexus) + nexus-cli all got `anyhow → thiserror`
   conversions per the rust/style.md decree. rsc still has
   `anyhow = "1"` and `clap = { version = "4", features =
   ["derive"] }` in Cargo.toml. Either rsc's 9-line stub gets the
   same treatment now, or it stays the canary (decide explicitly).

3. **sema/reference/Vision.md is the lone genuine stale-content
   item.** It mentions corec/askicc/askic/veric/domainc/semac — a
   pipeline that no longer maps to the current architecture
   (lojix is the compiler infrastructure; nexus is the bridge;
   criome is the engine). Either rewrite the vision against the
   current names or delete and lean on `criome/ARCHITECTURE.md`.

4. **The signal-absorption sweep already known.** Per the audit
   handoff, signal/src/{domain, module, names, origin, primitive,
   program, ty}.rs were absorbed from nexus-schema on 2026-04-23
   and need deletion review. Confirmed on the file listing in the
   main agent's notes; not re-derived here.

5. **lojix-store, sema, horizon-rs are the three repos where
   `--diff-filter=A` lies about content age** because they were
   *rebooted in place* via single "repurpose / scaffold" commits.
   For future audits, prefer
   `git log -1 --format=%ad --date=short -- <file>` (last-touched)
   over the diff-filter form when triaging suspect files.

## Action checklist (for Li's review)

- [ ] **rsc** — drop `anyhow`, swap to `thiserror` + local `Error`
      enum to match the 2026-04-24 cohort sweep (or decide rsc
      should stay anyhow-shaped given its capstone role).
- [ ] **sema/reference/Vision.md** — review against current
      architecture; either rewrite (corec/askicc/askic/veric/
      domainc/semac → real current names) or delete.
- [ ] **signal** — delete the 7 absorbed nexus-schema modules
      (already flagged by main agent; tracked separately).
