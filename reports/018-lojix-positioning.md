# 018 — lojix positioning and the naming collision

*Claude Opus 4.7 / 2026-04-24 · synthesis of two research
passes (current-lojix inventory + positioning options).
Answers Li's "where is lojix in all this?" question with a
concrete rename + scope recommendation.*

## 1 · Current state of `lojix` (one paragraph)

`lojix` is a **single-binary Rust CLI** (~580 LoC, edition 2024,
ractor-based) that orchestrates CriomOS deploys. Given a cluster
proposal nota file and a `(cluster, node)` pair, it reads the
nota, projects a `Horizon` in-process via `horizon-lib`, writes
a content-addressed wrapper flake to `~/.cache/lojix/horizon/
<cluster>/<node>/` and a parallel system flake to `~/.cache/
lojix/system/<system>/`, then shells out to `nix` / `nixos-
rebuild` with those directories as `--override-input horizon …`
and `--override-input system …`. Five actors:
`DeployCoordinator → {ProposalReader, HorizonProjector,
HorizonArtifact, NixBuilder}`. The e2e test against
goldragon/tiger passes; the actor skeleton is done. Its sole
user today is CriomOS (which names `lojix` as its flake input
producer). 13 open beads track hardening: atomic materialization,
streaming subprocess I/O, upload target, root-check, concurrent-
deploy safety, watch-mode daemon, etc.

## 2 · The naming collision — three uses of "lojix"

| Use | Where | Meaning |
|---|---|---|
| `lojix` crate | `/home/li/git/lojix/` | CriomOS deploy orchestrator (current binary) |
| `lojix-store` | `docs/architecture.md` §3 | content-addressed blob store (renamed from criome-store) |
| `lojix-stored` | `docs/architecture.md` §2 | blob-store daemon |
| `~/.cache/lojix/` | `lojix/src/artifact.rs:39` | deploy-cache path |
| `lojix-archive` | `/home/li/git/lojix-archive/` | old shelved aski build-dialect design |

A new reader encountering "lojix" can reasonably think any of
five things. The deploy crate is the outlier — everywhere else,
"lojix" has converged on **content-addressed blob storage**
inside the sema engine.

## 3 · Recommended rename — `lojix` crate → `forged`

Wait — that's the compile daemon. Let me think again.

The rename candidates:

| Candidate | Reading | Verdict |
|---|---|---|
| `horizond` | "horizon-daemon" — but it's a CLI, not a daemon | Suggestive of future daemonization; slightly misleading today |
| `criodeploy` | explicit CriomOS-scope CLI | Clear; ties to CriomOS rather than being engine-agnostic |
| `zoned` | mirrors CriomOS's `crioZones.<cluster>.<node>` | Concise; neutral; daemon-ready |
| `viewpointer` | horizon-lib's `Viewpoint` projection | Too technical, no user-facing intuition |

**Recommendation: `horizond`.**

Reasoning: the artifact it produces is a horizon (per
horizon-lib's type). The suffix `-d` previews the eventual
daemon (CLI today; daemon post-MVP per §4 below). Naming after
the output is how most unix tools work (`httpd` serves http;
`horizond` produces horizons). `zoned` is a close second —
better if Li wants to stay cluster-topology-flavoured.

**Keep** `lojix-store` + `lojix-stored` as-is. They already own
the `lojix` noun in the engine; the deploy crate is the
collision source.

## 4 · Where horizond fits — scoping recommendation

**Three framings of the core question**:

- **(A) First-class nexus-driven.** `(Deploy (Cluster foo))`
  comes through nexusd → criomed → dispatches to horizond
  (daemon). Cluster proposals live as sema records; deploy
  history is queryable. **Cost**: new daemon, new contract
  crate (`deploy-msg`), migration of proposals from `.nota`
  files to sema records. Heavy surface for an MVP still
  figuring out self-hosting compile.

- **(B) Standalone tool.** horizond stays a CLI outside the
  daemon graph. Reads proposals from local `.nota` files (as
  today) or later from sema via nexusd once criomed is alive.
  **Cost**: two internal universes (engine vs ops). **Benefit**:
  zero blast radius on MVP; Li keeps `horizond deploy --cluster
  goldragon --node tiger` working uninterrupted.

- **(C) Hybrid — library + CLI now, daemon later.** Split
  horizond into a library (actor topology, `HorizonProjector`,
  `HorizonArtifact`, `NixBuilder`) and a thin CLI. Post-MVP, a
  deploy daemon can be written atop the same library, exposing
  `(Deploy …)` via criomed.

**Recommendation: (C) for MVP, paving toward (A) post-MVP.**

The deploy workflow is genuinely adjacent to compile:
proposal→horizon→nix-flake mirrors opus→records→cargo-build.
But deploy is **not** the critical path to self-hosting; making
it first-class now doubles the daemon count before the first
daemon ships.

## 5 · Relationship to the sema engine

### horizond is a peer of forged, not a child

Both daemons (or future-daemons) have similar shape: a
`Coordinator → {Reader, Projector, Artifact, Builder}` pipeline.
But they are **peer concerns**, not parent/child:

- `forged` drives `rsc + cargo` against an `Opus` record.
- `horizond` drives `horizon-lib + nixos-rebuild` against a
  `ClusterProposal`.

Mixing nix and cargo concerns in one daemon would violate the
"one artifact per repo" rule. They stay separate.

### Shared types belong in nexus-schema

These newtypes exist in lojix's `cluster.rs` today and should
lift into `nexus-schema::names`:

- `NarHashSri` — SRI content-hash of a nix store path. Already
  called out in [reports/017 §1](017-architecture-refinements.md)
  for reuse by `Derivation`.
- `FlakeRef` — nix flake URI.
- `OverrideUri` — `path:...` or `tarball+url?narHash=...` forms.
- `TargetTriple` / `System` — platform triples (already planned
  in 017 for `Opus`).

After the lift, horizond and forged both depend on nexus-schema
for these. Neither duplicates. Neither depends on the other.

### HorizonProjector is a specialized Derivation builder

The `Derivation` record type proposed in
[reports/017 §1](017-architecture-refinements.md) wraps any nix
build: `Derivation { builder: DerivationBuilder::FlakeOutput { … }, … }`.

A horizon flake produced by horizond today **is** a Derivation
— its `flake_url` points at CriomOS, its `overrides` map
`horizon` and `system` inputs to content-addressed local paths,
its `nar_hash` is the SRI hash.

So:
- horizond **consumes** the CriomOS `Derivation` (flake input).
- horizond **produces** the horizon `Derivation` (override
  input).
- Once these types live in nexus-schema, horizond's
  HorizonArtifact can emit `Derivation` records; criomed can
  store them; future nexus queries can answer "what horizon did
  we deploy on tiger last Tuesday?" via a `DeployOutcome` record.

The convergence is natural. No redesign needed — just a
dependency inversion (horizond → nexus-schema).

### Shared ractor-coordinator pattern — defer extraction

lojix and forged will share a 4-actor pipeline pattern. An
extracted `ractor-coordinator` or `lojix-core` crate was
discussed in [reports/015 §6](015-architecture-landscape.md).
Recommendation: **defer** until both crates exist and the
pattern is demonstrably identical. ~40 LoC of boilerplate per
coordinator is cheap to duplicate; extracting early risks
over-fitting to lojix's specific shape.

## 6 · Records a future `deployd` would need

If and when deploy becomes first-class (path (A) or the
post-MVP stage of (C)), these would land in nexus-schema under
a new `deploy` module:

- **`ClusterProposal`** — root declarative record; lift of
  horizon-lib's type with rkyv derives. Contains nodes, schema
  version, cluster name.
- **`ClusterNode`** — per-node proposal entry (system,
  hostname, role, module toggles).
- **`Horizon`** — projected view for one `(cluster, node)` pair;
  carries `NarHashSri` identity and a `System` field.
- **`DeployRequest`** — rkyv wire type on `deploy-msg`.
- **`DeployOutcome`** — stored in sema for history:
  `{ request_hash, exit_status, horizon_nar, toplevel_drv,
  stdout_hash, stderr_hash, … }`.

Concrete shapes go in a later report when the daemon actually
lands. Today: just the names.

## 7 · Repo layout after the rename

Rename changes `/home/li/git/lojix/` → `/home/li/git/horizond/`.
Update:

- `Cargo.toml` package name + bin name (`lojix` → `horizond`)
- `README.md`, `AGENTS.md`
- Cache path `~/.cache/lojix/` → `~/.cache/horizond/`
- CriomOS's `flake.nix` text references (cosmetic; no build-
  time coupling)
- `lojix/src/cluster.rs` path reference in
  [reports/017 §1](017-architecture-refinements.md) → update
  when lifting types

No Rust library dependents to worry about; nothing imports the
lojix crate.

Workspace `linkedRepos` in mentci-next gains `horizond` in
Layer 5 (clients + tools). Count stays at 18 code repos.

## 8 · Migration steps (no ETAs)

1. Rename the crate: `/home/li/git/lojix/` → `/home/li/git/horizond/`.
   Update package name, binary name, README.md, AGENTS.md. New
   cache path. Update one reference in reports/017 §1.
2. Update `/home/li/git/CriomOS/flake.nix:2, :25, :38` text.
3. Lift `NarHashSri`, `FlakeRef`, `OverrideUri`, `TargetTriple`
   into `nexus-schema::names`. horizond then depends on
   nexus-schema.
4. Add `horizond` to mentci-next's `devshell.nix` linkedRepos.
5. Update `docs/architecture.md` to mention horizond in Layer 5.
6. Keep horizond's CLI surface identical (`deploy | build | eval`
   + `--action`). No daemon work now.

**Post-MVP, optional**:
- Extract `horizond-core` (actor pipeline library).
- Write a `deployd` binary.
- Add `deploy-msg` contract crate.
- Add `Deploy` dispatch branch in criomed.
- Land `ClusterProposal` / `Horizon` / `DeployOutcome` records in
  nexus-schema.

## 9 · Open questions for Li

**Q1 — Rename to `horizond`?** Or prefer another name (`zoned`,
`criodeploy`, something else)? `horizond` is my recommendation
because the artifact is the horizon.

**Q2 — Confirm scoping = (C)** — keep as CLI for MVP, paving
toward daemon post-MVP? Or prefer (B) — CLI indefinitely, never
subsume into the engine?

**Q3 — Lift shared newtypes now, or when nexus-schema gains
`Derivation`?** Coupling horizond to nexus-schema could happen
in the same pass that lifts `NarHashSri` et al; or defer until
the `Derivation` record lands.

**Q4 — Does horizond belong in the 18-repo count or outside?**
Today the count is "18 code repos for the engine." If horizond
is a sema-ecosystem citizen (reads proposals from sema
eventually), include it. If it stays CriomOS-specific ops, keep
it adjacent.

**Q5 — 13 open beads on horizond** — are any of these priority
blockers for the rename, or can they land independently?

---

*End report 018.*
