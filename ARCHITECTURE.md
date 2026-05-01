# ARCHITECTURE — workspace

The development environment for building criome.
Holds the workspace conventions, design corpus (`reports/`),
agent rules, and tooling integrations; symlinks every canonical
sema-ecosystem repo under `repos/` for unified navigation.

> **Read this file first.** Then read criome's `ARCHITECTURE.md`
> for the project being built.

## What this repo is

A development environment. Its concrete responsibilities:

- **Workspace orchestration.** `devshell.nix`'s
  `linkedRepos` symlinks every canonical sema-ecosystem repo
  into `repos/` on `nix develop` / direnv entry.
  Agents working here see the entire ecosystem at one path.
- **Design corpus.** `reports/` holds the evolving
  decision-trail (soft cap at ~12 per `AGENTS.md`
  "Report rollover").
- **Workspace manifest.** `docs/workspace-manifest.md`
  tracks every repo in `~/git/` with its status (CANON,
  TRANSITIONAL, CANON-MISSING, RETIRED, ARCHIVED, SHELVED,
  OFF-SCOPE). `devshell.nix`'s `linkedRepos` mirrors the
  CANON + TRANSITIONAL entries.
- **Tooling integration.** `bd` (issue tracker) state lives
  here; jj is the version-control surface; nix flakes provide
  reproducible toolchains.
- **Deployment aggregation.** This repo's `flake.nix` composes
  the canonical-crate flake inputs into NixOS modules + service
  specs that deploy the daemons (criome, nexus, forge,
  arca-daemon) and the static binaries (nexus-cli, lojix-cli)
  onto a CriomOS host.

## Layout

```
workspace/
├── ARCHITECTURE.md            ← this file
├── AGENTS.md                  ← workspace-repo-specific carve-outs
├── CLAUDE.md                  ← Claude Code shim → AGENTS.md
├── README.md
├── devshell.nix               ← workspace symlinks + tooling
├── flake.nix                  ← deployment aggregation
├── docs/
│   └── workspace-manifest.md  ← every repo's status
├── reports/                   ← decision-trail (soft cap 12)
├── checks/                    ← workspace-level nix checks
├── lib/                       ← shared nix helpers
├── repos/                     ← symlinks to canonical repos
└── .beads/                    ← bd issue tracker state
```

## Deployment

The sema-ecosystem deploys via nix flakes pinned in this repo:
each canonical crate is a flake input here; this repo's
`flake.nix` defines NixOS modules + container/service specs
that compose the daemons and static binaries into a
reproducible runtime.

`nix develop` opens the dev shell. `nix build .#packages.<sys>.<crate>`
builds any individual crate as a flake artifact.
`nixos-rebuild --flake workspace#<host>` deploys onto a CriomOS
host (lojix-cli covers this path during the transitional
phase; eventually criome itself drives deploys via signal-forge
verbs — see criome's ARCHITECTURE.md §10 phases).

The deploy spec lives in this repo because this is the
meta-repo: the place that knows the full set of canonical
crates and their flake URLs. Individual crates publish their
own flakes; this repo composes.

## How agents work here

Per `AGENTS.md` and the workspace-wide contract at
`github:ligoldragon/lore`:

1. **Read this file** for the dev environment shape.
2. **Read criome's `ARCHITECTURE.md`** for the project's design.
3. Each canonical repo carries its own `ARCHITECTURE.md` at
   root (matklad pattern). Open the relevant one when working
   inside a specific repo.
4. Decision history lives in `reports/`. Trim aggressively;
   replace cleanly.

## Status

**Active dev environment.** No fixed schedule for any of its
components; work proceeds against criome's milestones in
criome's ARCHITECTURE.md.
