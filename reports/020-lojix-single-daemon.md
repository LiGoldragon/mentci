# 020 — lojix: one daemon, no CLI, single contract

*Claude Opus 4.7 / 2026-04-24 · supersedes report 019 §5
(rename table) and §6 (daemon graph) given Li's directions
that (1) the current `lojix` CLI is a temporary hack, (2) every
request should eventually flow through nexus, (3) `lojixd`
"does the evaluation."*

## 1 · Premises

- **Every request through nexus.** There is no dedicated
  `lojix-cli`. The only text-facing client is `nexus-cli`; lojix
  operations are nexus messages like any other.
- **Evaluation in `lojixd`.** Parallels criomed's role as
  hallucination-wall for sema patterns: criomed resolves
  schema-bound things (RawPattern → PatternExpr); lojixd
  resolves lojix-bound things (build specs → concrete plans).
- **nix-build analogy was misleading.** nix-build is a legacy
  per-verb binary; the real nix architecture is library-centric
  (`libnixexpr`, `libnixstore`) with a single `nix-daemon` that
  handles store ops + sandboxed builds. See §7 for the
  implications.

## 2 · The lojix family, revised

Six slots. No CLI. One daemon.

| Slot | Repo | Role |
|---|---|---|
| **spec** | `lojix` (README only) | Namespace spec; parallels `nexus` / `nota` spec repos |
| **schema** | `lojix-schema` | `Opus`, `Derivation`, `OpusDep`, `RustToolchainPin`, `NarHashSri`, `FlakeRef`, `TargetTriple`, `CompiledBinary`, `LojixStoreToken`. rkyv derives. |
| **contract** | `lojix-msg` | **Single** wire contract: Compile + Store + Deploy verbs. Replaces the previously-planned `lojix-forge-msg` + `lojix-store-msg` pair. |
| **store** | `lojix-store` | Append-only file + rebuildable index + a **reader library** for mmap blob access. Not a daemon. |
| **daemon** | `lojixd` | The single lojix daemon. Owns store writes, forge work, deploy coordination. Internal actor topology hidden behind `lojix-msg`. |
| **projection** | `rsc` | Pure records→Rust-source projector. Library. Used internally by `lojixd`. Keeps its unprefixed name — it's pure, not nix-scope. |

**Retirements from report 019**:
- `lojix-forged` — becomes `ForgeCoordinator` actor inside `lojixd`
- `lojix-stored` — becomes `StoreWriter` / `StoreReader` actors inside `lojixd`
- `lojix-forge` (planned lib) — no public library; logic lives in `lojixd`
- `lojix-deploy` (planned rename of current `lojix`) — deploy logic migrates into `lojixd` as internal actors (`DeployCoordinator`, `HorizonProjector`, `NixShellout`, etc.)
- `lojix-forge-msg` + `lojix-store-msg` — merge into `lojix-msg`

Net: workspace drops from ~19 → ~16 code repos. Simpler graph.

## 3 · Daemon graph — now three daemons

```
 humans / LLMs / tools
        │  nexus text
        ▼
   ┌──────────┐
   │nexus-cli │   only text client
   └────┬─────┘
        │ nexus text over UDS
        ▼
   ┌──────────┐
   │  nexusd  │   messenger: text ↔ rkyv
   └────┬─────┘
        │ rkyv (criome-msg)
        ▼
   ┌──────────┐
   │  criomed │   guardian of sema; overlord
   └────┬─────┘
        │ rkyv (lojix-msg) — single contract
        ▼
   ┌──────────┐   owns lojix-store directory
   │  lojixd  │   internal actors:
   │          │     ForgeCoordinator + CargoBuilder + …
   │          │     StoreWriter + StoreReaderPool
   │          │     DeployCoordinator + HorizonProjector + …
   │          │     NixShellout (transitional)
   └──────────┘
        │ mmap reads / append writes
        ▼
   lojix-store/   append-only file + hash→offset index
```

**Three daemons, three pillars**:
- nexusd → criome (the communication skin)
- criomed → criome (sema guardian, subscription hub, pattern resolver)
- lojixd → lojix (build, store, deploy — all lojix concerns in one process)

sema has no daemon of its own; it is *served by* criomed.

## 4 · Why single-daemon (not multi)

1. **Nix itself ships one daemon.** `nix-daemon` handles store + build + GC; internal concerns are actor-like, not process-like. Copying this structure is simpler and has precedent.
2. **One wire contract, not two.** criomed only talks to one peer via `lojix-msg`. Simpler error handling, simpler supervision.
3. **No intra-lojix capability tokens.** Forge worker writing a binary into the store is now an in-process function call, not a cross-daemon token flow. The token design from report 017 §5 applies only at criomed ↔ lojixd (external), not inside.
4. **Shared resources.** Forge needs the store; store needs forge's output. Keeping them in one process avoids the serialisation / handoff dance.
5. **Easier to test and operate.** One process to start, one log stream, one supervision tree.

The multi-daemon split in report 019 §5 was premature factoring. Fold it back.

## 5 · What `lojix` the repo is

**A spec repo, README-only** — like `nexus` and `nota`. Describes:
- The namespace's vision (Li's "expanded and more correct nix")
- The record types (pointing into `lojix-schema`)
- The message verbs (pointing into `lojix-msg`)
- The store semantics
- The capability-token model (criomed-signed)
- The nix-replacement direction (phases wrap → replace)

**No code.** The current `/home/li/git/lojix/` Rust crate (deploy
CLI + actor graph) retires — its actors migrate into `lojixd`,
its `[[bin]] name = "lojix"` declaration deletes, and the repo
slot is reused for the spec README.

**Alternative (pragmatic)**: skip creating the separate spec
repo during MVP; put the spec text in this `reports/` directory
until the shape stabilises, then promote it. Parallels how the
nexus grammar spec grew into its own README.

## 6 · Parallel to criomed's schema-bound pattern resolver

Report 017 §2 established: pattern text is parsed client-side
into `RawPattern`; criomed resolves it against a sema snapshot
into `PatternExpr`. criomed is the **hallucination wall** for
nexus patterns.

Lojix has the same shape:

- Build-spec text/records are parsed/assembled into a **raw
  Opus** (unresolved: toolchain = derivation-ref, deps =
  OpusIds, flake URLs unresolved).
- `lojixd` resolves: fetches records from criomed, resolves
  flake refs via nix, pins toolchain closure, computes build
  plan. Produces a **bound Opus** / concrete plan.
- `lojixd` is the **hallucination wall** for lojix build specs.

Two resolver boundaries, two daemons, two domains — mirroring
`libnixexpr` vs `libnixstore`, but with the twist that both
live in daemons (not libraries), because every request must
flow through the trusted runtime (criomed signs tokens;
lojixd holds privileged nixos-rebuild etc.).

## 7 · Why lojix's architecture *diverges* from nix

Nix puts evaluation in the **client** (via `libnixexpr` linked
into every CLI binary), not in the daemon. This is historical:
eval is CPU-heavy and user-supplied, so keep it at user
privilege; daemon stays small and root-privileged. Any client
can link `libnixexpr` and evaluate.

Lojix puts evaluation in `lojixd`. Why:

- **Schema-bound**: lojix evaluation requires fetching sema
  records (Opus, Derivation, schema-rev pinning). Only
  criomed's neighbour can do that cheaply. Embedding
  `libnixexpr`-equivalent in every client would require every
  client to speak `criome-msg` directly — defeating the
  universal-nexus rule.
- **Single source of truth**: caching toolchain closures,
  flake-lock states, derivation evaluations — all in lojixd
  means one cache, one invalidation story.
- **Capability-token simplicity**: lojixd holds the privilege
  for deploys (`nixos-rebuild switch` needs root); putting eval
  elsewhere would split privilege across processes.
- **Tooling unity**: clients never need to link a Rust "lojix
  eval library." They speak nexus. nexus-cli ships; every other
  client is a nexus client.

Trade-off acknowledged: headless evaluation (e.g. for CI
without a running daemon) requires either starting lojixd or
using a test double. Acceptable for MVP; revisit if the
headless case becomes urgent.

## 8 · What changes vs report 019

| Item | 019 position | 020 position |
|---|---|---|
| Daemons in lojix family | `lojix-forged` + `lojix-stored` (two) | `lojixd` (one) |
| Contract crates | `lojix-forge-msg` + `lojix-store-msg` | `lojix-msg` (merged) |
| `lojix-forge` library | Separate library crate | Does not exist; logic lives in lojixd |
| `lojix-deploy` crate | Rename of current `lojix` | Retires; deploy actors migrate into lojixd |
| `lojix` repo slot | The deploy CLI (→ lojix-deploy) | Spec repo (README only) |
| Workspace size | ~19 code repos | ~16 code repos |
| CLI for lojix | None; use nexus-cli | Same — unchanged |
| rsc | Pure lib, unprefixed | Same — unchanged |
| `lojix-schema` | New library crate | Same — unchanged |
| Opus/Derivation home | `lojix-schema` | Same — unchanged |

Everything else in report 019 stands: three-pillar framing,
two-axis per daemon (runtime = criome; family = lojix), the
11 nix problems lojix fixes, the 8 principles, the wrap→replace
migration phases.

## 9 · Open questions

**Q1 — Spec repo timing.** Create `lojix` spec repo now
(README only), or keep the vision in `mentci-next/reports/`
until the shape stabilises and promote later? Lean: keep in
reports/ until Phase B; promote when `lojixd` scaffolds.

**Q2 — `lojix-store` as a public reader library.** The
blob-store directory is mmap-safe for parallel readers. Do we
export a read-only API any process can link (read your own
binaries by hash without involving lojixd), or keep read access
daemon-only for capability-consistency? Nix's answer: read is
public (any user can read `/nix/store`); write is daemon-only.
Likely follow this.

**Q3 — lojixd privileges.** `nixos-rebuild switch` needs root.
Does `lojixd` run as root, drop privileges for eval+compile,
and re-raise for deploy? Or spawn a privileged helper? Nix runs
nix-daemon as root; the deploy path is the only one that
actually needs it. Defer to implementation time.

**Q4 — `lojix-msg` streaming.** Large binaries (tens of MB) on
the criomed ↔ lojixd wire. Chunked rkyv streaming, or
filesystem-path handoff (both daemons share `~/.lojix/`)?
Filesystem handoff is simpler; requires same-host deployment.
Lean path-handoff for MVP.

**Q5 — NixShellout retirement.** The transitional
`NixShellout` actor inside lojixd (Phase A → B wrapping of nix
commands) dies when `Derivation` records gain non-nix backends
(Phase C). Feature-flag the actor, or plain code-path that
later sessions delete? Lean: no feature flag; plain code path.

## 10 · docs/architecture.md updates needed

- §1 thesis: "four daemons" → "three daemons" (nexusd, criomed,
  lojixd).
- §2 daemon diagram: collapse `lojix-forged` + `lojix-stored`
  into `lojixd`; diagram drops one box; one contract arrow
  (`lojix-msg`) instead of two.
- §4 repo layout: Layer 4 daemons drops to three; Layer 2
  contracts drops `lojix-forge-msg` + `lojix-store-msg` and
  gains `lojix-msg`; Layer 3 storage keeps `lojix-store` as
  a library-plus-directory (no daemon); Layer 5 drops
  `lojix-forge` and `lojix-deploy` entries.
- §5 type families: `CompileRequest/Reply` and
  `LojixStoreRequest/Reply` both live in `lojix-msg`.
- §6 compile loop: all references to `lojix-forged` become
  `lojixd`; token flow to `lojix-stored` disappears (in-
  process).

---

*End report 020.*
