# 123 — lojix-cli-v2 repo creation and work survey

## What changed

A new sibling repo, `~/git/lojix-cli-v2`, now exists as a fresh
jj-colocated repo copied from the current `lojix-cli` source tree
without inherited VCS state, build artefacts, or bead state.

The workspace now treats it as a first-class TRANSITIONAL repo:

- added to `docs/workspace-manifest.md`
- added to `devshell.nix` `linkedRepos`
- added to `workspace.code-workspace`

The repo itself has its own:

- `AGENTS.md`
- `CLAUDE.md`
- `ARCHITECTURE.md`
- `.beads/` database

The binary/package identity is distinct from the live tool:

- Cargo package: `lojix-cli-v2`
- binary: `lojix-cli-v2`
- flake main program: `lojix-cli-v2`

That separation matters operationally: the new repo can evolve
aggressively without shadowing the current operator binary.

## Why this repo exists

The workspace manifest already says the current `lojix-cli` is the
working deploy tool and should not be rewritten in place. CriomOS
report `0038` now carries changes that are architectural, not just
incremental flags:

- Nota-native invocation
- request-file loading
- typed target generalization beyond system only
- local home deployment semantics

Those changes touch the CLI contract, request model, build target
selection, and activation logic simultaneously. They are exactly the
kind of work that should happen in a forked repo while the original
tool stays stable for real deployments.

## Current copied shape

The copied code still reflects the original monolith:

- `src/main.rs` is Clap-first and emits one `DeployRequest`
- `src/deploy.rs` coordinates proposal read → project → artifact →
  build → copy → activate
- `src/build.rs` hardcodes
  `nixosConfigurations.target.config.system.build.toplevel`
- `src/activate.rs` knows only system activation semantics
- tests assert the current argv and builder-validation behavior

This is good starting material because the projection and artifact
phases are already reusable. The hardcoded target attr and
system-only activation path are the actual constraints.

## Work survey

### 1. Replace Clap-first entry with Nota-first request decoding

The new canonical entry should be:

- inline Nota when first argv begins with `(`
- otherwise a request file path
- otherwise the default request path

Compatibility subcommands can remain temporarily, but they should map
into one typed request model internally instead of remaining the
architectural center.

Immediate code impact:

- `src/main.rs`
- typed request definitions, likely in a new request/config module
- tests for inline-nota vs file-path dispatch

### 2. Split request target from system action

Today `BuildAction` mixes "what phase do we run" with a hardcoded
assumption that the target is always the system toplevel.

V2 needs a typed target model such as:

- system target
- home target with `UserName`

and a separate home mode:

- build
- profile
- activate

Immediate code impact:

- `src/build.rs`
- `src/deploy.rs`
- request decoding and validation

### 3. Generalize build attr selection

`src/build.rs` currently emits only:

`<flake>#nixosConfigurations.target.config.system.build.toplevel`

V2 needs target-derived attr selection, especially:

`nixosConfigurations.target.config.home-manager.users.<user>.home.activationPackage`

This is the center of the v2 redesign. Once the build target is typed,
the rest of the home flow becomes ordinary orchestration instead of a
special case bolted onto a system-only pipeline.

### 4. Add local home activation as a separate domain

`src/activate.rs` is system-specific: root SSH, system profile path,
`switch-to-configuration`, EFI reconciliation.

Local home deploy needs a separate activation object with user-scoped
behavior:

- profile set at `~/.local/state/nix/profiles/home-manager`
- optional `activate`
- no system profile writes
- no `switch-to-configuration`
- no EFI semantics

This should be a sibling activation path, not an overloaded extension
of `SystemActivation`.

### 5. Validate home requests at the horizon boundary

Per `0038`, home deploy should fail before Nix when the requested user
is absent from projected `horizon.users`.

That validation belongs in the same stage that currently resolves and
validates `--builder`.

### 6. Decide whether a separate defaults/alias file still earns its keep

If the Nota-native request path is pleasant enough, the first useful
config file may just be "a saved request".

A richer alias/default layer should only land if repeated use shows
that saved requests are too weak. V2 should avoid inventing a second
user-facing grammar without evidence it is needed.

### 7. Keep remote home deploy out of the first cut

The codebase already has enough moving pieces for:

- local projection
- system builds
- optional remote builders
- root-targeted system activation

Remote home deploy adds a different SSH principal, different profile
path ownership, and possibly `systemd-run --user` detachment
semantics. It should stay explicitly second-phase work.

## Recommended sequence

1. Introduce the typed v2 request model and Nota-first dispatch.
2. Generalize build target selection to support `System` and `Home`.
3. Add local home `build/profile/activate`.
4. Add pre-Nix user validation.
5. Revisit whether a separate alias/default config layer is still
   justified.
6. Design remote home deploy only after local home flows are verified.

## Initial bead candidates

The new repo should track at least these short items:

- Nota-first request decoding
- typed build target generalization
- local home activation path
- pre-Nix horizon user validation
- request-file/default-path loading
