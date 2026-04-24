# 018 — lojix positioning (v2)

*Claude Opus 4.7 / 2026-04-24 · synthesis of two research
passes (current-lojix inventory + positioning options). v1 of
this report proposed renaming the `lojix` crate to `horizond`;
Li corrected: "lojix was a play on nix — it is my take on an
expanded and more correct nix." The name is load-bearing. v2
treats `lojix` as a namespace, not a mislabel.*

## 1 · `lojix` is a namespace, not a single crate

"lojix" is Li's vision of an expanded, more-correct nix. It's
the umbrella, same way "nix" umbrellas `nix`, `nix-store`,
`nix-daemon`, `nix-build`, `nix-shell`, `nixos`, `nixpkgs`.

Current members of the `lojix-*` family:

| Name | Role |
|---|---|
| `lojix` (crate at `/home/li/git/lojix/`) | CriomOS deploy orchestrator — reads cluster proposals, projects horizons, invokes `nixos-rebuild` |
| `lojix-store` | content-addressed blob store (renamed from criome-store); holds compiled binaries + opaque blobs |
| `lojix-stored` | blob-store daemon; guards lojix-store, serves put/get over rkyv |
| `lojix-store-msg` | contract crate (wire types for blob-store traffic) |
| `~/.cache/lojix/` | on-disk cache path used by the current `lojix` CLI (deploy artifacts) |
| `lojix-archive` | older shelved aski build-dialect experiment; not active |

The namespace is healthy: every name has a specific, non-
overlapping meaning inside the umbrella "expanded nix."

Future `lojix-*` crates are expected. This report does not
prescribe what else joins the family — that emerges as the
engine grows.

## 2 · Current state of `lojix` (the deploy crate)

Inventory from research pass:

- **Shape**: single-binary Rust CLI, edition 2024, ~580 LoC,
  ractor-based. Status per README: "scaffold" — but actor
  skeleton is done and e2e test passes against goldragon/tiger.
- **Flow**: reads a cluster-proposal `.nota`, projects a
  `Horizon` in-process via `horizon-lib`, writes a content-
  addressed wrapper flake to `~/.cache/lojix/horizon/<cluster>/
  <node>/` and a parallel system flake to `~/.cache/lojix/
  system/<system>/`, shells out to `nix` / `nixos-rebuild` with
  `--override-input horizon …` and `--override-input system …`.
- **Actors**: `DeployCoordinator → {ProposalReader,
  HorizonProjector, HorizonArtifact, NixBuilder}`. Single-
  object-in / single-object-out at each boundary.
- **CLI surface**: `lojix {deploy | build | eval}` plus
  `--action {Eval | Build | Boot | Switch | Test}`.
- **Shell-out surface**: `nix hash path --type sha256 --sri`,
  `nix eval --raw …#drvPath`, `nix build --no-link
  --print-out-paths`, `nixos-rebuild {switch | boot | test}
  --flake`.
- **Typed newtypes** in `src/cluster.rs`: `FlakeRef`,
  `OverrideUri`, `NarHashSri`, `ProposalSource`, and
  `System`/`ClusterName`/`NodeName`/`Viewpoint`/`ClusterProposal`/
  `Horizon` re-exported from horizon-lib.
- **Dependents**: zero Rust library consumers. Operationally,
  CriomOS's `flake.nix` names lojix as its flake-input producer
  (`--override-input horizon` and `--override-input system` are
  exactly what lojix supplies).
- **13 open beads** in `/home/li/git/lojix/.beads/`. Priorities:
  P0 atomic-materialization + streaming-stdout; P1 root-check,
  upload target, error-path tests, concurrent-deploy safety;
  P2 timeouts, all-actions tests, `--target-host` passthrough;
  P3 watch-mode daemon.

## 3 · Where `lojix` sits in the architecture

**Layer 5 — clients + build helpers.** Peer to nexus-cli and
rsc (in the sema engine's sense) but unique in domain: it
bridges sema records (eventually) to nixos-rebuild. It runs as
a CLI from userland. It does not sit in the daemon graph
(nexusd / criomed / forged / lojix-stored).

**Relationship to `forged`**: peer, not child. Both daemons (or
in lojix's case, CLI-then-daemon) follow a `Coordinator →
{Reader, Projector, Artifact, Builder}` pattern:

- `forged` drives `rsc + cargo` against an `Opus`.
- `lojix` drives `horizon-lib + nixos-rebuild` against a
  `ClusterProposal`.

Nix and cargo concerns stay in separate crates; "one artifact
per repo" is preserved.

**Shared ractor-coordinator pattern — defer extraction.**
Discussed multiple times; still not worth extracting a shared
`ractor-coordinator` / `lojix-core` library. ~40 LoC of
boilerplate per coordinator is cheap to duplicate until both
crates exist and the pattern is demonstrably identical.

**HorizonProjector is already a specialized `Derivation`
builder.** A horizon flake produced by lojix is exactly what
[reports/017 §1](017-architecture-refinements.md)'s
`Derivation` type describes: a flake-output derivation with
override inputs and a `NarHashSri`. Once `Derivation` lands in
nexus-schema, lojix-the-crate's `HorizonArtifact` naturally
emits `Derivation` records.

**Shared newtypes lift into nexus-schema**:
- `NarHashSri` (already anticipated in
  [017 §1](017-architecture-refinements.md))
- `FlakeRef`
- `OverrideUri`
- `TargetTriple` / `System`

After the lift, lojix depends on nexus-schema for the
vocabulary; nexus-schema does not depend on lojix.

## 4 · Scoping — CLI now, daemon maybe later

Three framings of "should lojix be engine-integrated?":

- **(A) First-class nexus-driven.** `(Deploy (Cluster foo))`
  goes through nexusd → criomed → dispatches to a lojix daemon.
  Cluster proposals live as sema records; deploy history
  queryable. **Cost**: a new daemon, a `deploy-msg` contract
  crate, a migration of proposals from `.nota` files to sema
  records. Heavy surface for a not-yet-self-hosting MVP.

- **(B) Standalone tool.** lojix stays a CLI outside the daemon
  graph. Reads proposals from `.nota` files (today) or later
  from sema via nexusd. **Benefit**: zero MVP blast radius;
  Li's current workflow keeps working. **Cost**: two internal
  universes (engine vs ops).

- **(C) Hybrid — library + CLI now, daemon later.** Split lojix
  into a library (actor pipeline, `HorizonProjector`, etc.) +
  thin CLI. Post-MVP, a `lojix` daemon can be written atop the
  same library, exposing `(Deploy …)` via criomed.

**Recommendation: (C), paving toward (A) post-MVP.** Deploy is
genuinely adjacent to compile (same-shape pipelines) but it's
not on the critical path to self-hosting. Making it first-class
now doubles daemon count before the first daemon ships.

## 5 · Records a future deploy daemon would need

If/when lojix gains a daemon face (path (A) or post-MVP (C)),
these records land in nexus-schema:

- **`ClusterProposal`** — root declarative record; lift of
  horizon-lib's `ClusterProposal` with rkyv derives.
- **`ClusterNode`** — per-node proposal entry.
- **`Horizon`** — projected `(cluster, node)` view; carries
  `NarHashSri`.
- **`DeployRequest`** — wire type on `deploy-msg`.
- **`DeployOutcome`** — durable record of a deploy (request
  hash, exit status, horizon nar, toplevel drv, stdout/stderr
  hashes, wall time).

Concrete shapes land in a later report when the daemon actually
ships.

## 6 · Low-touch migration (no naming changes)

Crate does NOT rename. The only moves are:

1. **Lift shared newtypes** — `NarHashSri`, `FlakeRef`,
   `OverrideUri`, `TargetTriple` — from `lojix/src/cluster.rs`
   into `nexus-schema::names`. Update lojix to depend on
   nexus-schema.
2. **Add lojix to mentci-next's linkedRepos** so mentci-next's
   workspace can see it.
3. **Document the namespace** — architecture.md (done) notes
   that `lojix-*` is Li's expanded-nix family.
4. **No CriomOS flake.nix changes needed** — naming stays.
5. **13 open beads** in lojix's bd can be worked independently
   of the sema engine; none are blockers for the engine's MVP.

## 7 · Open questions for Li

**Q1 — Confirm scoping = (C)** (hybrid; CLI for MVP, path to
daemon post-MVP)? Or (B) pure-CLI indefinitely?

**Q2 — Type lift timing**: lift `NarHashSri` et al. into
nexus-schema **now** (touches lojix's Cargo.toml) or **later**
when `Derivation` actually lands?

**Q3 — Is `lojix` in the 18-repo count?** Today mentci-next
describes an 18-repo engine. lojix is engine-family by name
(lojix-store, lojix-stored) but deploy is adjacent to
sema-engine compile. Include in the count (→ 19) or keep
adjacent / CriomOS-specific?

**Q4 — Any of lojix's 13 open beads priority-block engine
work?** P0 atomic-materialization + streaming-stdout are
correctness improvements for lojix itself; they don't block
the engine. Confirm.

**Q5 — Are there other `lojix-*` crates Li has in mind?**
e.g., a `lojix-build` that does something nix-build-y? A
`lojix-hash` for content addressing primitives? Knowing the
namespace's intended shape helps future-proof.

---

## Superseded framings from v1

For the record — v1 of this report recommended renaming the
`lojix` crate to `horizond`. That recommendation is withdrawn.
"lojix" is not a misnomer; it is Li's expanded-nix namespace.
The `lojix` crate belongs in the family by its very name.

---

*End report 018 v2.*
