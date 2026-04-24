# 030 — lojix transition plan: monolith → thin-CLI + lojixd

*Claude Opus 4.7 / 2026-04-24 · preserves Li's currently-working
CriomOS deploy orchestrator while routing toward the eventual
lojixd daemon. Replaces any reading of architecture.md §4 that
says "delete lojix's code and make it a spec repo today". That's
the END state; this report is the ROAD.*

---

## 1 · Current state (do not disturb)

`/home/li/git/lojix/` is a single Rust crate:

- **Binary**: `lojix deploy | build | eval --cluster X --node Y --source Z.nota`.
- **Internals**: ractor actor pipeline —
  `DeployCoordinator → ProposalReader + HorizonProjector +
  HorizonArtifact + NixBuilder`.
- **External deps**: `horizon-lib` (in-process projection),
  `nota-serde` (reads cluster proposal nota), `ractor`,
  `tokio`, `clap`.
- **Function**: reads a cluster proposal nota → projects via
  horizon-lib in-process → writes a content-addressed horizon
  flake → invokes `nixos-rebuild` against CriomOS with the
  horizon as `--override-input`.
- **Role in Li's workflow**: the only hand-operation tool for
  CriomOS deploys today. Used from shell daily.
- **Reference design**:
  `/home/li/git/CriomOS/reports/2026-04-24-ractor-tool-design.md`.

**Invariant for this session and all near-future ones**: the
code that ships these deploys keeps shipping these deploys. Any
restructuring must be behaviour-preserving from the CLI user's
perspective.

## 2 · Terminal state (from architecture.md §4)

Eventually — when nexusd + criomed + lojixd all run — the lojix
namespace has the layout in
[reports/020](020-lojix-single-daemon.md):

- `lojix/` — spec-only README repo.
- `lojixd/` — the single lojix daemon; owns the actor tree
  (forge + store + deploy all inside).
- `lojix-msg/` — wire contract (rkyv). CompileRun, DeployRun,
  StoreOp verbs.
- `lojix-store/` — append-only blob directory + reader lib.
- A **thin CLI** that parses shell syntax, constructs a
  `lojix-msg` rkyv envelope, and sends it to lojixd. Eventually
  superseded by `nexus-cli` when criomed routes deploys, but
  may persist as a direct-to-lojixd utility.

The CLI-to-lojixd path matters for bootstrap: long before
criomed is usable, lojixd needs an operator-facing interface,
and nexus-cli is over-engineered for the "just run a deploy"
ergonomics the shell user has today.

## 3 · Gap between current and terminal

| Aspect | Current | Terminal | Delta |
|---|---|---|---|
| Process topology | 1 binary (lojix) | 2+ (lojix-cli + lojixd; plus criomed, nexusd) | New daemon |
| Wire contract | none (in-process calls) | `lojix-msg` rkyv | New crate |
| Actor ownership | lojix binary | lojixd | Crate migration |
| CLI ergonomics | clap subcommands | clap subcommands → rkyv → UDS/socket | Add transport |
| Store | none (horizon writes files; nix handles CAS) | `lojix-store` blob dir | New, later |
| Forge work | none (lojix only deploys) | forge actors in lojixd | Add later |

**The smallest-first-step observation**: the only delta we can
cheaply do *today* is to sketch the `lojix-msg` shapes by
reading the existing `DeployMsg::Run { request, reply }`
envelope and `DeployRequest` fields. Everything else needs
lojixd to exist, which needs criomed's scaffolding to exist.

## 4 · Transition phases

Phases are sequential but each ships independently. No phase
breaks the hand-operation tool.

### Phase A — keep shipping (current)

- lojix monolith unchanged.
- Changes to existing `src/*.rs` files are only bug-fixes,
  features Li asks for, or refactors that preserve the CLI
  behaviour.
- Agents DO NOT rip actors out, DO NOT introduce a second
  crate, DO NOT add IPC.
- Architecture.md is correct about the *end state* but agents
  must read this report's Phase-A description and leave lojix
  alone structurally.

### Phase B — extract the message shapes (no runtime change)

*Unlocks: lojix-msg contract exists for future clients to
target. Alongside, the lojix-store skeleton (already landed
2026-04-25 in `/home/li/git/lojix-store/src/` as types +
traits + `todo!()` bodies) gets wired into lojixd's internal
layout.*

- Create new crate `/home/li/git/lojix-msg/`.
- Define rkyv-archived message types including the primary
  verbs under the nix-backend model:
  `RunNix { flake_ref, attr, overrides, target }`,
  `BundleIntoLojixStore { nix_store_closure, policy }`,
  `RunNixosRebuild { … }`, `PutStoreEntry`, `GetStorePath`,
  `MaterializeFiles`, `DeleteStoreEntry`.
- Mirror today's in-process types for backward continuity:
  - `DeployRun { cluster, node, action, source: ProposalSourceRef, criomos: FlakeRef } → DeployOutcome`
  - Supporting newtypes (`ClusterName`, `NodeName`,
    `ProposalSourceRef`, `FlakeRef`, `BuildAction`,
    `DeployOutcome`) — copy shapes from lojix's current types
    as rkyv-derives-clean structs.
- lojix continues to use its current in-process types; it does
  not yet depend on lojix-msg.
- **No semantic change to the running binary.** lojix-msg is an
  empty commitment.
- Why-not-skip: giving lojix-msg a name and a home now means
  future sessions aren't scattered trying to invent the crate.
  Building lojix-msg from the existing shapes locks in a
  coherent verb set before anything has to speak it.

### Phase C — scaffold lojixd shell (not yet routed through)

*Unlocks: a daemon process exists and parses lojix-msg, even if
lojix binary doesn't talk to it.*

- Create new crate `/home/li/git/lojixd/`.
- Implement: UDS socket listener; rkyv deserialise → match
  verb → stub handlers that return "not yet implemented".
- lojixd links `lojix-msg` but does not yet use horizon-lib,
  nix, or store.
- lojix binary unchanged. Li's deploys still go through the
  monolith.
- Observable effect: `lojixd` runs, you can hit it with a test
  client, you get back `NotImplemented` replies.
- Why separate from Phase D: getting the transport right (rkyv
  framing, UDS handshake, reconnect on restart, error types
  on wire) is its own chunk of work with no runtime risk to the
  hand-operation tool.

### Phase D — move one actor; dual-mode lojix CLI

*Unlocks: a path for lojix CLI to optionally route through
lojixd, so we can validate the daemon wiring without breaking
monolith fallback.*

- Pick a leaf actor — `HorizonProjector` is a good candidate
  because it's pure and already uses horizon-lib in-process.
- Implement the horizon-projection handler in lojixd (move or
  clone the logic).
- Add a `--via-daemon` flag to the lojix CLI (default: off).
  When set, the CLI constructs a `lojix-msg` and sends to
  lojixd for that single operation, but the rest of the
  pipeline stays in-process.
- `--via-daemon` is opt-in and off by default. Li's normal
  `lojix deploy ...` invocations use the monolith path.
- Observable effect: `lojix deploy --via-daemon ...` produces
  the same output as `lojix deploy ...` but one pipeline stage
  ran in lojixd.
- Why one actor at a time: lets us develop daemon
  infrastructure (logging, error handling, health, restart)
  under a feature flag before committing to "all traffic
  through the daemon".

### Phase E — migrate remaining actors; make lojixd authoritative

*Unlocks: lojixd is the source of truth for deploy operations.*

- Migrate `ProposalReader`, `HorizonArtifact`, `NixBuilder` to
  lojixd actors.
- Each migration keeps the `--via-daemon` opt-in semantics
  until every stage is daemon-backed.
- Once all stages route through lojixd, flip the default:
  `--via-daemon` becomes the default, with `--monolith` as the
  opt-out for rollback.
- lojix binary becomes thin: it parses the clap CLI, constructs
  `lojix-msg` verbs, sends to lojixd, prints the reply.
- Crucially, this flip only happens when Li is comfortable
  with lojixd's operational maturity — not on any agent's
  timetable.

### Phase F — name transition

*Unlocks: the repo-layout invariants in architecture.md §4.*

- Decide the thin CLI's home: rename the `lojix` binary to
  `lojix-cli`, or keep `lojix` as the binary name and
  accept the imprecision, or fold it into `nexus-cli` once
  criomed brokers deploys.
- Rename the `lojix/` repo content: the production CLI code
  moves (to `lojix-cli/` or retires as folded into nexus-cli);
  the `lojix/` slot becomes the spec-only README.
- Lojix-archive may receive the monolith codebase as a historical
  snapshot even though most of its logic lives on in lojixd.

Phase F should not happen until Phases B–E have produced
stable alternatives. Architecture.md §4's "lojix = spec-only"
is Phase F's end-state.

### Phase G — criomed takes over routing

*Unlocks: architecture.md's "every request through nexus".*

- Once criomed runs, deploy verbs flow:
  nexus-cli text → nexusd rkyv → criomed → lojixd.
- The direct lojix-cli → lojixd path remains as an
  ops-utility (break-glass for when criomed is down, or for
  lower-level operations nexus doesn't wrap).
- This is the "every request through nexus" invariant of
  architecture.md; it requires criomed to exist and understand
  deploy messages, which is far away.

## 5 · Guardrails for agents

Baseline invariants that hold across phases:

1. **Never ship a commit that breaks `lojix deploy ...` from
   the current shell workflow.** If work-in-progress risks that,
   feature-flag it off.
2. **Add before you subtract.** New crates (lojix-msg, lojixd)
   appear before anything in the lojix repo changes structure.
3. **Any refactor inside `lojix/src/` must be behaviour-
   preserving** for the CLI user. If in doubt, run `lojix
   deploy` against a known-good cluster/node pair and diff
   output.
4. **Don't rename the `lojix` binary without asking Li.** The
   binary name is in Li's muscle memory.
5. **The AGENTS.md warning on `/home/li/git/lojix/CLAUDE.md`**
   (added this session) is authoritative; if you're about to
   violate it, stop.
6. **Architecture.md's "lojix = spec-only README" is Phase F.**
   It's not an order to execute this session.

## 6 · Per-phase blocking relationships

```
Phase A (forever) ─── Phase B (lojix-msg crate)
                  │
                  ├── Phase C (lojixd shell listening)
                  │
                  ├── Phase D (one actor moved; --via-daemon flag)
                  │
                  ├── Phase E (all actors migrated; default flip)
                  │
                  ├── Phase F (repo-shape rename to terminal layout)
                  │
                  └── Phase G (nexus routes deploys via criomed)
```

B and C can parallelise. D depends on C. E depends on D. F
depends on E. G depends on E and criomed.

## 7 · What the "thin-wrapper CLI for creating lojix messages" is

Concretely, what Li asked for today lands in **Phase B + a
small Phase-D slice**:

- **Phase B**: lojix-msg crate exists. Anyone — including a
  hand-crafted script, a tui, a curl-over-uds invocation — can
  construct a `DeployRun` message as a plain struct and
  rkyv-serialise it.
- **Phase-D slice** (the minimum viable thin CLI): a small
  binary in a new crate (tentative name `lojix-cli` — NOT in
  the existing `lojix/` repo) or a subcommand in lojix gated by
  `--via-daemon`, that does nothing but construct a
  `lojix-msg` envelope and send it. No actors, no horizon-lib,
  no nix invocation. Pure translator.
- **Non-goal for this CLI**: actually executing the deploy.
  That stays in lojix (Phase A) or moves to lojixd (Phase E).

The thin CLI is useful *even before lojixd exists* as:
- A round-trip tester ("construct the message, print it,
  round-trip through rkyv archive/deserialise, print back").
- A future operator interface ("construct and send once lojixd
  accepts it").

## 8 · What the mentci-next docs should say

- **architecture.md §4**: clarify that `lojix` (spec-only) is
  the terminal layout; current state is a monolith in-flight.
  Point at this report.
- **reports/020**: already accurate about the terminal layout.
  Add a breadcrumb sentence "the transition plan is in
  reports/030" near the top.
- **lojix/CLAUDE.md**: big warning banner landed this session.
  References this report.

## 9 · Open questions

**Q1 — thin CLI's repo home.** Three candidates: (a) a new
`lojix-cli/` repo at `/home/li/git/lojix-cli/`; (b) a subcommand
or second binary inside the existing lojix repo
(`src/bin/lojix-cli.rs`); (c) fold into nexus-cli once criomed
exists. Lean: (a) when lojixd is scaffolded enough to need a
client; until then, no dedicated thin CLI — just lojix-msg the
crate and handwritten tests.

**Q2 — lojixd transport.** UDS with rkyv length-prefixed frames
is the simplest. Alternatives: TCP localhost, named pipe, gRPC
over UDS. Lean: UDS + length-prefixed rkyv. Matches
architecture.md's pattern for criome-msg.

**Q3 — compatibility boundary.** During Phases D–E, lojix-msg
may evolve. Does the monolith lojix (Phase A) need to track
those changes? No — lojix's in-process types are its own;
lojix-msg is for daemon traffic. The boundary only matters
when lojix starts using lojix-msg internally (Phase E flip).

**Q4 — CriomOS-specific vs lojix-generic.** The current
lojix binary is CriomOS-specific (`--criomos github:...`
default; nixos-rebuild targeting CriomOS). The lojixd in
architecture.md is lojix-generic (forge + store + deploy for
any Rust/nix artifact). Do deploy verbs in lojix-msg stay
CriomOS-shaped, or generalise? Lean: generalise the verbs
(`DeployRun { target_host, flake_ref, action, … }`) and let
the CLI supply CriomOS defaults as a convenience layer.

**Q5 — Phase ordering with criomed work.** Criomed's scaffolding
(reports/021, /026) is early. If criomed is months away,
Phases B + C may make sense sooner even without immediate
consumers. If criomed is also scaffolded soon, lojix-msg should
be designed knowing criomed will shape `lojix-msg` usage.
Lean: start Phase B now (write the crate); defer Phase C
until lojixd has a clearer need.

**Q6 — Bd board for lojix transition.** Where do the phase
tasks live? Options: in `lojix`'s bd, in `mentci-next`'s bd, or
in a new `lojixd`'s bd once scaffolded. Lean: phase-planning
tasks in mentci-next's bd (architecture-scope work); individual
refactor tasks inside lojix's bd when we reach Phase D+.

---

## 10 · TL;DR for future agents

If you land in this repo with fresh context and see
architecture.md saying "lojix is a spec-only README":

1. **Stop.** Read this report.
2. lojix is Li's working deploy tool right now.
3. The transition is gradual: Phase B (lojix-msg crate)
   happens first; Phase F (repo-shape rename) happens last.
4. Don't delete anything in `lojix/src/`. Don't rename the
   binary. Don't introduce IPC inside the current crate.
5. If you're asked to extract lojix-msg or scaffold lojixd,
   follow Phase B/C. Both are additive in separate repos.

---

*End report 030.*
