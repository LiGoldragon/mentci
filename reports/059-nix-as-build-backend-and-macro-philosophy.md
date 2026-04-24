# 059 — nix as the build backend; macro philosophy

*Claude Opus 4.7 · ratifies two decisions: (1) nix (crane +
fenix) is the build backend for the bootstrap era until lojix
replaces it; (2) we author no macros but freely call third-
party macros. Supersedes references to `RunCargo` as a primary
verb; adds `RunNix` as primary.*

---

## Decision 1 — nix as the build backend

### Rationale

Building Rust artifacts through nix gives us, for free:

- **Hermeticity**: nix sandboxes builds; no host leak, no
  network except fetched inputs.
- **Dep resolution + proc-macro compilation**: crane handles
  crates.io fetching, cdylib-building of proc-macro sub-
  crates, passing `--extern` correctly to rustc. We don't
  reimplement any of this in this era.
- **Ecosystem access**: serde, tokio, clap, thiserror — every
  proc-macro-using crate works out of the box.
- **Toolchain pinning**: fenix exposes stable/nightly/custom
  Rust toolchains with the components we need (rustc, cargo,
  clippy, rustfmt, rust-analyzer, rust-src). Already the
  canonical choice across every sema-ecosystem flake.
- **Store + linking**: `/nix/store` paths with RPATH stitched
  through; binaries run correctly wherever `/nix/store` is
  mounted.

Replacing any of this before we have the engine working would
be redundant with lojix's long-term replace-nix goal. Using
the reference implementation *is* preparation for building the
successor.

### Tooling choices (confirmed)

- **fenix** — nix-community toolchain pin. Already used in
  every canonical workspace repo: `sema`, `nexus-schema`,
  `nexusd`, `nexus-cli`, `rsc`, `nota-serde-core`. fenix
  exposes `stable.withComponents [...]`; crane consumes it.
- **crane** — ipetkov/crane. IFD-free Rust builder for nix
  flakes. Preferred over `rustPlatform.buildRustPackage` (too
  stodgy, tied to nixpkgs version) and naersk (abandoned).
- **Migration note**: `lojix/flake.nix` uses
  `oxalica/rust-overlay` (fenix's predecessor). Migrate to
  fenix when we next touch lojix.

### The compile flow concretely

1. User issues `(Compile (Opus :slot N))` to criomed.
2. criomed reads the Opus + transitive `OpusDep` + toolchain pin
   + features from sema.
3. criomed instructs rsc to project records to a scratch
   workdir. rsc emits:
   - `src/**.rs` (Rust code)
   - `Cargo.toml` (workspace + per-crate)
   - `flake.nix` (points at fenix for toolchain, crane for
     the build, imports `OpusDep`-linked flakes for crate
     deps where applicable)
   - `rust-toolchain.toml` (fenix pin)
4. criomed emits `RunNix { flake_ref: workdir, attr:
   "packages.<system>.<opus-name>", overrides, target }`
   to lojixd.
5. lojixd spawns `nix build` on the flake ref; nix/crane
   resolve deps, compile proc-macro sub-crates, invoke the
   fenix-pinned rustc, link, output to `/nix/store`.
6. lojixd returns `NixBuildOutcome { store_paths, narhashes,
   wall_ms, warnings }`.
7. criomed asserts `CompiledBinary { opus, narhash,
   store_path, toolchain_pin, produced_at_rev }` into sema.

### What lojix-msg carries

Primary verbs under this model:

- `RunNix { flake_ref, attr, overrides, target }` — compile +
  package builder for any Opus.
- `RunNixosRebuild { flake_ref, action, target_host,
  overrides }` — deploys (lojix's existing CriomOS path,
  unchanged).
- `PutStoreEntry`, `GetStorePath`, `MaterializeFiles`,
  `DeleteStoreEntry` — store-management verbs (future, once
  lojix-store is real).

`RunCargo` is **not** a primary verb; if it exists at all,
it's an internal optimisation inside lojixd for cases where
bypassing nix saves time (unlikely to be worth it).

`RunRustc` is deferred until we're replacing nix (years away).

### Linking implications

- Compiled binaries carry `RPATH` pointing into `/nix/store`.
  Fine on any host with `/nix/store` mounted.
- Cross-host distribution (if/when we need it): `nix-bundle`,
  `nix-portable`, or build static binaries. Not a Phase-0
  concern.
- Sema's `CompiledBinary` record references the **narhash**
  (stable, content-addressed identity) alongside the
  current-path. Narhash travels; paths don't.

### lojix-store status under this decision

Deferred further than previously planned:

- MVP and well beyond use `/nix/store` as the de-facto store.
- Sema records reference nix narhashes / store paths.
- `lojix-store` repo stays scaffolded (seed-only; AGENTS.md
  flags it so) — real implementation waits until we're
  replacing nix.
- Workspace-manifest should reflect this revised timeline.

---

## Decision 2 — macro philosophy

### We author no macros

- **No `macro_rules!` macros in our code.** Any syntactic
  sugar we need for nexus lives in the nexus grammar (the
  delimiter-family matrix + Pascal-named records). Rust-side
  sugar is replaced by sema-side rules.
- **No proc-macro crates authored by us.** Rich code-gen
  patterns (derive-equivalent, attribute-equivalent,
  function-like-equivalent) become sema rules.
- **Sema rules replace them structurally**: a rule's premise
  matches records; the head produces records. Running the
  rule emits fully-elaborated records. rsc projects those
  records to plain Rust with no `#[derive]` or `macro_rules!`
  in sight.

### Why sema rules beat Rust macros

- **Operate on name-resolved, type-aware, content-addressed
  records** — not on syntactic token trees.
- **Logic programming layer** (rules-as-records) — versioned,
  introspectable, permissioned.
- **Stratified** — cascade termination enforced by the rule
  engine, not by convention.
- **No phase ordering problem** — rules fire at sema-edit
  time, not at a special compile phase.
- **No privilege escalation** — running a sema rule is a
  cascade, not arbitrary Rust code executing in rustc.

### We freely call third-party macros

In the generated Rust, these appear verbatim:

- `#[derive(Serialize, Deserialize, Debug, Clone, Hash,
  PartialEq, Eq, thiserror::Error, ...)]`
- `#[tokio::main]`, `#[async_trait]`, other attribute macros
- `format!("{}", x)`, `vec![1, 2, 3]`, `println!("..."); eprintln!()`
- `sqlx::query!(...)`, other function-like macros

### How sema represents these

Rather than "a macro invocation" being opaque:

- `Opus` gets a `derives: Vec<TraitPath>` field (or
  equivalent; effectively a list that rsc serialises into
  `#[derive(...)]`).
- Attribute macros become a field on the documentable record
  (Fn, Struct, etc.): `attrs: Vec<AttrInvocation>`.
- Function-like macro calls are represented by a `MacroCall`
  record kind (`path: Path, tokens: MacroTokens`) or an
  `Expr::MacroCall` variant. rsc emits them as-is.

No parsing or expansion happens in sema. The macros are
opaque invocations to criomed — records carrying "call this
at rustc time."

### One subtle rule

If a third-party macro expands to *our-generated code pattern*
(e.g., if a derive produces an impl that mimics something we
also have a sema rule for), we have duplicates. Resolution:
prefer the third-party derive (we don't compete with
ecosystem crates). Our sema rules exist for patterns not
covered by the ecosystem.

---

## What changes across the workspace

### `docs/architecture.md`

Updated (this session):
- §1 engine-in-a-paragraph: lojixd spawns `nix` not `cargo`.
- §1: new paragraph on nix-backend + macro philosophy.
- §4 daemon diagram: `RunNix` as primary verb; nix/crane/fenix
  mention in lojixd internal.
- §5 lojix-store: `/nix/store` is the de-facto store;
  lojix-store impl deferred.
- §6 type families: `RunNix` primary in the lojix-msg verb
  list.
- §7 compile-loop data flow: rsc emits workdir; lojixd runs
  nix build.
- §10 rules: nix-is-backend rule + macro-philosophy rule.

### `reports/030` — lojix transition plan

Phase B's `lojix-msg` carries `RunNix` as the primary compile
verb. `RunCargo` was never a long-term verb. `RunRustc` is
post-nix-replacement (very deferred). The phase plan itself
is unchanged; just the verb set it produces.

### `docs/workspace-manifest.md`

`lojix-store` row updated: "scaffolded (seed code only); real
implementation deferred until nix-replacement phase. During
the bootstrap era, `/nix/store` serves this role." Effectively
moves lojix-store further down the priority queue.

### Sibling repos

- `lojix/flake.nix`: migrate `oxalica/rust-overlay` → fenix
  when next edited. Non-urgent; lojix's current flake works.
- Other flakes (sema, nexus-schema, etc.): already fenix-
  based; no change.
- rsc: when it actually emits package builds, it'll learn to
  write flakes that import crane + fenix.

---

## Open implementation details (not deciding now)

- **`OpusDep` → nix flake input translation**: how do we
  express "this opus depends on serde from crates.io" as a
  flake input? crane has a `cargoDeps` mechanism; we build on
  that. rsc emits the right pieces.
- **Cross-opus deps in our workspace**: each opus becomes a
  flake output; `OpusDep` to another opus becomes a flake
  input. Probably.
- **Incremental builds**: nix+crane incremental works at the
  dep-graph level; our sema knows the dep graph too. We let
  nix do incrementals; criomed's cascade handles the
  edit-to-plan side.
- **Profile resolution** (dev / release): Opus field →
  crane `CARGO_PROFILE` pass-through.
- **Cross-compilation**: crane supports it; we surface
  `target` in the `RunNix` verb.

These are operational details; not architectural open
questions.

---

*End report 059.*
