# Workspace manifest

*Authoritative inventory of which repos under `~/git/` are part of
the sema-ecosystem MVP and which are not. Agents consult this
file to decide what to touch.*

**Rule for agents**: if a repo isn't listed here as **CANON** or
**TRANSITIONAL**, don't edit its source or docs without explicit
instruction. `devshell.nix`'s `linkedRepos` list mirrors this
manifest's CANON + TRANSITIONAL entries.

Last reviewed: 2026-04-25.

---

## Status vocabulary

| Status | Meaning |
|---|---|
| **CANON** | Currently canonical; symlinked in `repos/`; agents freely operate. |
| **TRANSITIONAL** | Canonical today but in a migration; read the linked report first. |
| **CANON-MISSING** | Belongs in canonical but the repo doesn't exist yet. Create when needed per the linked report. |
| **RETIRED** | Superseded; about to move to `~/git/archive/`. Don't edit. |
| **ARCHIVED** | Historical; banner-marked; don't edit. May still live in `~/git/` until physical archive pass. |
| **SHELVED** | Design-valid but post-MVP. Keep around; not in canonical. |
| **OFF-SCOPE** | Not part of sema-ecosystem MVP. Ignore. |

---

## CANON

The repos that make the sema-ecosystem MVP exist. Agents expect
to find them at `~/git/<name>/` and symlinked at
`mentci/repos/<name>/` (local directory is `~/git/mentci-next/`
during the in-progress rename; the symlink target paths do not
change).

| Repo | Role | Pointer |
|---|---|---|
| `tools-documentation` | Cross-project rules, daily-use tool docs. | `repos/tools-documentation/AGENTS.md` |
| `criome` | Spec repo — runtime pillar; three-pillar framing. | `docs/architecture.md §4` |
| `nota` | Spec repo — data grammar (nota ⊂ nexus). | `reports/013` |
| `nota-serde-core` | Shared lexer + ser/de kernel for both dialects. | `reports/014` |
| `nota-serde` | nota's public façade. | `reports/014` |
| `nexus` | The nexus language — grammar spec under `spec/` + translator daemon (text ↔ signal rkyv). Renamed from `nexusd` 2026-04-25; absorbed the former `nexus` spec repo (now archived as `nexus-spec-archive`). Local dir is still `~/git/nexusd/`. | [`nexus/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus/blob/main/ARCHITECTURE.md), `criome/ARCHITECTURE.md §3+§4` |
| `nexus-serde` | nexus's public façade. | `reports/014` |
| `nexus-schema` | Record-kind vocabulary (Fn, Struct, Opus, Derivation, …) + nexus language IR (RawPattern, RawOp, RawRecord, edit verbs, diagnostics). | `reports/070 §6.6`, `reports/077`, `criome/ARCHITECTURE.md §6` |
| `signal` | nexus↔criome messaging schema — rkyv envelope (Frame), handshake protocol (ProtocolVersion 0.1.0), edit/query/validate verbs, subscription stream. Imports IR payloads from nexus-schema. | `reports/077`, `reports/070 §6` |
| `sema` | Records DB (redb-backed). | `criome/ARCHITECTURE.md §3` |
| `lojix-store` | Content-addressed filesystem + index DB (nix-store analogue). **MVP-required alongside lojix**. nix builds into `/nix/store`; lojix's `BundleIntoLojixStore` copies the closure into `~/.lojix/store/<blake3>/` with RPATH rewrite; sema records reference lojix-store hashes as canonical identity. Skeleton types + traits in `lojix-store/src/`. | `criome/ARCHITECTURE.md §5`, `lojix-store/ARCHITECTURE.md`, `lojix-store/AGENTS.md` |
| `nexus-cli` | Text client. | `criome/ARCHITECTURE.md §4`, `nexus-cli/ARCHITECTURE.md` |
| `rsc` | Records → Rust source projector. | `architecture.md §1` |

### CriomOS cluster

The criome engine runs on CriomOS. These host-OS repos are
canonical because engine work may need to evolve the OS
alongside it.

| Repo | Role | Pointer |
|---|---|---|
| `CriomOS` | NixOS-based host OS for the sema ecosystem. | `CriomOS/AGENTS.md` |
| `horizon-rs` | Horizon projection library; lojix-cli's deploy path links it in-process. | `horizon-rs/AGENTS.md` |
| `CriomOS-emacs` | Emacs configuration as a CriomOS module. | `CriomOS-emacs/AGENTS.md` |
| `CriomOS-home` | Home-manager configuration as a CriomOS module. | `CriomOS-home/AGENTS.md` |

## TRANSITIONAL

Canonical today, structure changes per a plan.

| Repo | Current role | Target | Pointer |
|---|---|---|---|
| `lojix-cli` | Li's working CriomOS deploy orchestrator (CLI + ractor actors + horizon-lib + nixos-rebuild). Renamed from `lojix` 2026-04-25; `lojix` namespace reserved for the family. Local dir is still `~/git/lojix/`. | Thin transport for `lojix-schema` requests once `lojix` exists. Agents must NOT rewrite this repo. | `lojix-cli/ARCHITECTURE.md` |

## CANON-MISSING

Belongs in canonical per architecture but the repo doesn't
exist yet. Create when we reach the corresponding work.

| Repo | Purpose | When |
|---|---|---|
| `criome` | sema's engine daemon. | Needed for anything beyond nexus scaffolding. |
| `lojix-schema` | criome↔lojix contract (rkyv). | `reports/030` Phase B. |
| `lojix` | lojix daemon (forge + store + deploy actors inside). | `reports/030` Phase C. |

Once each is created, add its entry to CANON and to
`devshell.nix`'s `linkedRepos` list.

## RETIRED / ARCHIVED

All session-initial RETIRED/ARCHIVED entries were actioned on
2026-04-24:

- `criome-store` was **renamed to `lojix-store`** (GitHub rename
  + local move; remote redirects). Now CANON.
- `lojix-archive` was **deleted** (GitHub + local). Pre-2026-04-24
  "lojix-as-aski-dialect" vision was obsolete; no surviving
  content worth preserving. Reference trace lives in commit
  history; the lojix family's three-pillar role lives in
  [criome/ARCHITECTURE.md §1 + §4 + §8](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md).
- `lojix` was **renamed to `lojix-cli`** (GitHub rename;
  redirects). Frees the `lojix` name for the family namespace
  (lojix-store, lojix-cli, lojix-schema, lojix). Local dir
  `~/git/lojix/` unchanged for now (Li to rename later).

There are currently no entries in RETIRED or ARCHIVED.

## SHELVED (post-MVP but not retired)

| Repo | Reason | Revisit |
|---|---|---|
| `arbor` | Prolly-tree versioning over records; post-MVP optimisation. | After self-hosting closes. |

## OFF-SCOPE

Not part of the sema-ecosystem MVP. Agents should not touch
these except when explicitly directed.

Grouped for readability; not exhaustive — assume any repo in
`~/git/` that isn't listed above is OFF-SCOPE until proven
otherwise.

- **Old aski / synth family** (superseded language-family
  vision): `aski`, `askic`, `aski-cc`, `aski-core`,
  `aski-core-bootstrap`, `aski-macro`, `ply-aski`,
  `astro-aski`, `synth-core`, `semac`, `sema-codegen`.
- **CriomOS archival only**: `criomos-archive`. (The live
  cluster `CriomOS`, `CriomOS-emacs`, `CriomOS-home`,
  `horizon-rs` is CANON, listed above.)
- **Noesis / veri / etc.** (old experiments): `noesis`,
  `noesis-schema`, `veri-core`, `veric`, `corec`.
- **Web / book / non-technical**: `AnaSeahawk-website`,
  `BookMaker`, `BookOfLuna`, `caraka-samhita`, `clavifaber`,
  `maisiliym`, `MotherSpirit-webpage`, `phoenixWebsite`,
  `seahawkWebsite`, `TheBookOfGoldragon`, `TheBookOfSol`,
  `webpage`, `WebPublish`, `wiki`, `world`, `criomeWebsite`,
  `awesome`, `bibliography`, `bibliotheca`, `goldragon`,
  `helloWorld`, `hob`, `kibord`, `lib`, `ndi`, `pi-delegate`,
  `pi-mentci`, `pkdjz`, `private`, `registry`, `rkyv`,
  `rust-atom`, `seahawk`, `shen-sources`, `skrips`, `SonyUtils`,
  `substack-cli`, `system`, `brightness-ctl`, `devenv-atom`,
  `nixpkgs-atom`, `qmkBinaries`, `mkHorizon-atom`, `mkSkrip`,
  `pi-mentci`, `aedifico`, `annas-mcp`, `Armbian-RockPi4B-NixOS`,
  `ArtificialIntelligence`, `vscode-aski`,
  `Mentci`, `mentci-tools`, `domainc`, `criosphere`, etc.
- `beads`, `home-manager` — tooling, not sema-ecosystem.

---

## Update protocol

When the architecture changes or a repo's status changes:

1. Update this manifest (change the status row and `last-reviewed`).
2. Update `devshell.nix` `linkedRepos` if a CANON row was added or removed.
3. Update `docs/architecture.md §4` if the layer layout changed.
4. Write a report (`reports/NNN-*.md`) describing the change.
5. Commit + push.

When a repo goes RETIRED and we do a physical archive pass:

1. Move `~/git/<name>/` → `~/git/archive/<name>/`.
2. Remove from `devshell.nix` if it was there.
3. Update this manifest's row (RETIRED → ARCHIVED).
4. Run `nix develop` to refresh symlinks (cleanup stale ones).

---

*End workspace-manifest.*
