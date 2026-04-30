# 114 — Mentci stack supervisor: design draft

*A draft sketch of how a `mentci up` / `mentci down` pair would assemble
the running stack. The supervisor itself is a small new crate;
configuration is a single nota record; the nix flake builds every
daemon + the supervisor + a wrapper that runs them as one foreground
unit. Lifetime: until Li answers the §10 questions and this report
either rolls into a successor or the design lands as skeleton code in
the new crate.*

---

## 0 · What we want, stated positively

A user typing one command at a fresh checkout gets the full mentci
workbench running:

```
$ nix run .#up
   ↳ builds every daemon if needed (cached after first time)
   ↳ creates the per-user state directories (sema.redb, arca root, run dir)
   ↳ launches each daemon with the right socket paths + env
   ↳ launches mentci-egui pointed at those sockets
   ↳ supervises everything in the foreground
   ↳ respawns crashed daemons with bounded backoff
   ↳ Ctrl-C tears the whole stack down cleanly
```

A second command swaps a daemon for an upgraded build without taking
the workbench down:

```
$ nix run .#reload criome     # rebuild criome, swap the running daemon
```

A third gives an explicit teardown for cases where the supervisor is
running detached:

```
$ nix run .#down              # graceful shutdown via the supervisor's
                              #  control socket
```

These three are the user-facing surface. Everything else is the
supervisor's internal concern.

---

## 1 · Topology — what the supervisor is supervising

### 1.1 Today (criome + nexus + mentci-egui)

```
                       ┌────────────────────────┐
                       │      mentci-egui       │   GUI process
                       │ tokio-runtime + driver │   (user-facing)
                       └─────┬────────────┬─────┘
                             │            │
                /run/user/$UID/mentci/    │
                       criome.sock        │
                             │            │  /run/user/$UID/mentci/nexus.sock
                             ▼            ▼
                       ┌──────────┐  ┌──────────┐
                       │ criome-  │  │ nexus-   │   long-running daemons
                       │ daemon   │  │ daemon   │
                       └────┬─────┘  └──────────┘
                            │
                ┌───────────▼────────────────┐
                │  ~/.local/share/mentci/    │   on-disk state
                │       sema.redb            │   (criome owns)
                └────────────────────────────┘
```

The above three processes plus the on-disk state are everything that
needs orchestrating today.

### 1.2 What lands later (forge + arca)

```
                       ┌────────────────────────┐
                       │      mentci-egui       │
                       └─────┬────────────┬─────┘
                             │            │
                       ┌─────▼─────┐ ┌────▼─────┐
                       │ criome-   │ │ nexus-   │
                       │ daemon    │ │ daemon   │
                       └─────┬─────┘ └──────────┘
                             │ signal-forge
                             ▼
                       ┌──────────┐
                       │  forge-  │  links prism; runs nix build;
                       │  daemon  │  bundles into ~/.arca/_staging/
                       └─────┬────┘
                             │ signal-arca
                             ▼
                       ┌──────────┐
                       │  arca-   │  verifies token; computes blake3;
                       │  daemon  │  atomic-moves into <store>/<blake3>/
                       └─────┬────┘
                             │
                       ┌─────▼────────┐
                       │  ~/.arca/    │  content-addressed FS
                       │   <store>/   │  (arca-daemon owns)
                       │   <blake3>/  │
                       └──────────────┘
```

The supervisor design has to anticipate this. Adding forge and arca
later should not require rewriting the supervisor — only adding two
entries to the config and naming two more sockets.

### 1.3 The directory layout the supervisor creates

```
   ${XDG_RUNTIME_DIR}/mentci/         # ephemeral; tmpfs on Linux
       criome.sock                    # signal      (criome's listener)
       nexus.sock                     # signal      (nexus's listener)
       forge.sock                     # signal-forge (forge's listener)
       arca.sock                      # signal-arca (arca's listener)
       supervisor.sock                # supervisor's control socket
       supervisor.pid                 # for `mentci down` from another shell

   ${XDG_DATA_HOME}/mentci/           # persistent
       sema.redb                      # criome owns
       criome/                        # criome's capability-signing
           signing.bls                #   key (chmod 0600); criome
                                      #   mints on first run
       principal.bls                  # this user's identity key
                                      # (chmod 0600); mentci-egui's
                                      # bootstrap (or a small
                                      # mentci-keygen one-shot)
                                      # mints on first run

   ${HOME}/.arca/                     # persistent, criome ARCH §5
       _staging/<deposit-id>/         # write-only by writers
       <store-name>/<blake3>/         # written only by arca-daemon
       <store-name>/index.redb        # arca-daemon owns
```

`XDG_RUNTIME_DIR` is per-user, per-session, tmpfs-backed on Linux —
exactly the right place for sockets. `XDG_DATA_HOME` is the persistent
store. Both fall back to sensible defaults when unset.

---

## 2 · The supervisor crate

Per [tools-documentation/programming/micro-components.md](../repos/tools-documentation/programming/micro-components.md):
adding a feature defaults to a new crate. This *is* a new capability —
process supervision over the sema-ecosystem daemons — so it lives in
its own repo with its own `Cargo.toml` + `flake.nix` + tests.

**Name: `process-manager`.** Reads as English; says directly what the
crate is.

### 2.1 What process-manager owns

```
   ┌────────────────────────────────────────────────────────────────┐
   │                       process-manager                          │
   ├────────────────────────────────────────────────────────────────┤
   │  reads:    Config (nota text → typed Rust via nota-codec)      │
   │                                                                 │
   │  spawns:   one child OS process per (Daemon ...) entry         │
   │            via std::process::Command + tokio::process::Child   │
   │                                                                 │
   │  watches:  child exits via tokio's `wait()`                    │
   │            file-system events on watched src/ dirs (optional)  │
   │            its own control socket (`reload`, `stop`,           │
   │              `swap-binary <name>`)                             │
   │                                                                 │
   │  policies: RestartPolicy per daemon                             │
   │              { Always | OnFailure | Never }                     │
   │            ExponentialBackoff with cap                          │
   │            HealthProbe: Handshake-on-socket-or-fail (per-      │
   │              daemon, before declaring it ready)                 │
   │                                                                 │
   │  surfaces: stdout/stderr per daemon, prefixed by name           │
   │            its own structured log line per lifecycle event     │
   │                                                                 │
   │  shuts down: one SIGTERM per child, then SIGKILL after grace   │
   │              period. Reverse spawn order so dependents go      │
   │              first (mentci-egui before criome before nexus).   │
   └────────────────────────────────────────────────────────────────┘
```

### 2.2 What process-manager explicitly does NOT do

Per the criome-runs-nothing rule applied analogically:

- **No nix invocations.** process-manager never calls `nix build`. Upgrades go
  through `nix run .#reload <name>` which rebuilds with nix and then
  asks the running process-manager to swap the path.
- **No record awareness.** process-manager does not parse signal frames, does not
  speak to criome, does not know what's in sema. Its only knowledge of
  the daemons is "this binary path at this socket path with this env."
- **No log aggregation database.** Lines are passed through to the
  controlling terminal (or to journald when running under systemd).
  Persistent log analysis is a different capability for a different
  crate.
- **No multi-tenant supervision.** One process-manager per user session.
  System-wide criome lives in NixOS service modules
  ([criome/ARCHITECTURE.md §8](../repos/criome/ARCHITECTURE.md)), not
  in process-manager.

### 2.3 The shape of process-manager's types

The nota config deserialises into a closed `Config` record;
process-manager's run loop holds a `Supervisor` whose methods own the
verbs. Field-level shape:

```
   Config
   ───────────────────────────────────────────────────────────────
   run_dir       : path                ← where sockets live
   data_dir      : path                ← where sema.redb lives
   arca_root     : path                ← ~/.arca
   daemons       : list of DaemonSpec  ← order = launch order
   frontend      : optional DaemonSpec ← mentci-egui — launched
                                          last, torn down first

   DaemonSpec
   ───────────────────────────────────────────────────────────────
   name          : DaemonName          ← typed: criome | nexus | …
   binary        : path                ← nix-store path
   args          : list of string
   env           : list of EnvAssignment  (typed; ${var} expansion)
   socket        : optional path       ← listener socket if any
   restart_policy: RestartPolicy       ← Always | OnFailure | Never
   depends_on    : list of DaemonName  ← spawn ordering
   readiness     : Readiness

   Readiness — closed enum, three shapes:
     SocketBound { path, timeout }     wait for socket file to exist
     Handshake   { socket, timeout }   wait for signal handshake round-trip
     Immediate                         mark ready straight away
```

The supervisor is a noun owning the verbs; methods only,
no free functions:

```
   Supervisor
   ───────────────────────────────────────────────────────────────
   fields:
     config   : Config
     children : map[DaemonName → Child]
     control  : ControlSocket

   methods:
     new(config)                  →  Supervisor
     launch_all()                 →  Result<()>
     shutdown_all(grace)          →  Result<()>
     swap(name, new_binary_path)  →  Result<()>
     run()                        →  Result<()>          ← the only
                                                          function that
                                                          owns the loop
```

Concrete shapes land as skeleton code in process-manager's repo before the
first commit (per [criome ARCH §13.6 "Skeleton-as-design over
prose-as-design"](../repos/criome/ARCHITECTURE.md#13--update-policy)) —
the structure here is the *outline*, not a substitute for the
typed code.

---

## 3 · Configuration in nota

The configuration is itself a typed record — call it `Config` — encoded
in nota text and parsed via nota-codec into the Rust struct above. This
eats our own dog food: nota is the typed-text language; using it for
configuration validates that the language reaches even the lowest-stakes
surface.

### 3.1 Example shape — ${XDG_CONFIG_HOME}/mentci/process-manager.nota

```nota
(Config
  run_dir   "${XDG_RUNTIME_DIR}/mentci"
  data_dir  "${XDG_DATA_HOME}/mentci"
  arca_root "${HOME}/.arca"

  daemons [
    (Daemon
      name           "criome"
      binary         "@criome-daemon@"          ; nix-substituted at build
      socket         "${run_dir}/criome.sock"
      env            [
        (Env "CRIOME_SOCKET" "${run_dir}/criome.sock")
        (Env "SEMA_PATH"     "${data_dir}/sema.redb")
      ]
      restart_policy Always
      depends_on     []
      readiness      (Handshake
                       socket  "${run_dir}/criome.sock"
                       timeout 5s))

    (Daemon
      name           "nexus"
      binary         "@nexus-daemon@"
      socket         "${run_dir}/nexus.sock"
      env            [
        (Env "NEXUS_SOCKET"  "${run_dir}/nexus.sock")
        (Env "CRIOME_SOCKET" "${run_dir}/criome.sock")
      ]
      restart_policy Always
      depends_on     [criome]
      readiness      (Handshake
                       socket  "${run_dir}/nexus.sock"
                       timeout 5s))
  ]

  frontend
    (Daemon
      name           "mentci-egui"
      binary         "@mentci-egui@"
      env            [
        (Env "CRIOME_SOCKET" "${run_dir}/criome.sock")
        (Env "NEXUS_SOCKET"  "${run_dir}/nexus.sock")
      ]
      restart_policy OnFailure
      depends_on     [criome nexus]
      readiness      Immediate))
```

The nota syntax above is a placeholder shape — actual delimiters /
sigils follow nota's grammar (the delimiter-family matrix in
[criome ARCHITECTURE §9](../repos/criome/ARCHITECTURE.md#9--grammar-shape)).
The point is: typed records, parser already exists, validation is just
the type checker.

### 3.2 Why nota for config (and not toml/yaml)

- **Workspace-wide contract uniformity.** Every typed text in the
  ecosystem is nota; every typed binary is rkyv. Config is typed text
  → nota.
- **Validation is free.** nota-codec produces a `Config` value or a
  typed error. No second validation layer.
- **Self-hosting feeds the loop.** Once schema-in-sema lands, this
  config-record's *type definition* could itself be a sema record —
  the supervisor reads the type from the engine it supervises. Not
  needed for the first version, but the path stays open.

### 3.3 ${var} expansion

A small substitution pass before nota-decoding: `${XDG_RUNTIME_DIR}`,
`${HOME}`, plus the config-internal references (`${run_dir}`). This
keeps the config readable and lets the same file work across users.

The `@criome-daemon@` placeholder is filled in by nix at build time
(per [tools-documentation/rust/nix-packaging.md](../repos/tools-documentation/rust/nix-packaging.md)
substitution patterns), so the generated config carries fixed-store
paths.

---

## 4 · Lifecycle visuals

### 4.1 Spawn

```
   process-manager starts
       │
       ▼
   read nota config
       │
       ▼
   ensure run_dir + data_dir + arca_root exist (mkdir -p; perms 0700)
       │
       ▼
   topological-sort daemons by depends_on
       │
       ▼
   for each daemon in order:
       │
       ├── std::process::Command::new(binary)
       │      .args(args)
       │      .envs(resolved env)
       │      .stdout/stderr(piped → log-prefixer task)
       │      .spawn() → Child
       │
       ├── readiness probe:
       │      SocketBound  → poll for socket file existence
       │      Handshake    → connect + signal::Handshake exchange
       │      Immediate    → return immediately
       │
       └── on success: store Child in self.children
           on timeout: SIGTERM child, error out of launch_all
       │
       ▼
   seed criome from genesis.nexus if sema is empty (§4.2)
       │
       ▼
   spawn frontend (mentci-egui) the same way
       │
       ▼
   enter main run loop (§4.5)
```

### 4.2 Seeding sema from `genesis.nexus`

This is already canon — [criome ARCHITECTURE §10 "Bootstrap rung by rung"](../repos/criome/ARCHITECTURE.md#10--rules):

> No "before the engine runs" mode; criome runs from the first instant,
> sema starts empty, nexus messages populate it (including seed
> records via `genesis.nexus`, fed through nexus by the launcher).

process-manager is "the launcher" in that sentence. Without seeded data,
mentci-egui paints an empty GraphsNav and an empty canvas — technically
correct but pedagogically useless. Seeding gives the user something to
click on the first time the workbench opens.

#### 4.2.1 Where the seed file lives

```
   nix-store path:
       ${self.packages.${system}.process-managerConfig}/genesis.nexus

   user override (read first if present):
       ${XDG_CONFIG_HOME}/mentci/genesis.nexus
```

Two reasons for the override path: a user can edit it without
rebuilding the flake; and project-specific seeds (a particular Graph
to demo) can sit alongside the project's `process-manager.nota`.

#### 4.2.2 The seed flow

```
   process-manager has just confirmed criome readiness (Handshake round-trip OK)
       │
       ▼
   issue Query(Graph wildcard) over criome.sock
       │
       ▼  reply Records::Graph
       │
       ├── empty? ──► proceed to seed
       │
       └── non-empty? ──► skip seeding (sema already populated;
                          do not double-assert and risk duplicates
                          until idempotent Assert lands)
       │
       ▼
   read genesis.nexus from the resolved path
       │
       ▼
   pipe its contents through nexus-cli with NEXUS_SOCKET pointed
   at ${run_dir}/nexus.sock:
       cat genesis.nexus | nexus-cli
       │
       ▼  for each top-level (Assert ...) form:
       │     nexus-cli reads, nexus-daemon parses + forwards as
       │     signal::Request::Assert, criome writes, returns
       │     Outcome(Ok) per the [editor flow][edit-flow]
       │
       ▼
   on any reply that's not Outcome(Ok):
       log the diagnostic
       (do NOT abort startup — partial seed is better than no
        workbench; user can fix and re-run)
       │
       ▼
   continue to spawning mentci-egui

[edit-flow]: ../repos/criome/ARCHITECTURE.md#71-edit-m0
```

#### 4.2.3 Why `nexus-cli`, not direct rkyv asserts

process-manager could in principle build `signal::Frame` values itself and write
them to `criome.sock`, skipping nexus entirely. Reasons to go through
nexus instead:

- **Invariant B as a discipline**, not just a runtime fact: text only
  crosses at the nexus boundary. Even seed data written by an
  internal tool respects the boundary. process-manager stays free of any signal
  schema knowledge.
- **Editability.** `genesis.nexus` is human-readable and
  human-editable; the same file lives in the user's CONFIG dir as a
  starting point for their own seeds.
- **One bootstrap path, exercised every session.** The seed flow is
  the same flow nexus-cli already does for any text input — no
  separate "internal seed" path with its own bugs.

#### 4.2.4 Sketch of `genesis.nexus`

(Provisional shape — actual nexus syntax follows the
[delimiter-family matrix](../repos/criome/ARCHITECTURE.md#9--grammar-shape).)

```nexus
;; A Graph the workbench paints on first run.
(Assert (Graph "First Graph"
                nodes:    [@ticks @double @stdout]
                edges:    [@e1 @e2]
                subgraphs: []))

;; Three Nodes that live inside it.
(Assert (Node "ticks"))
(Assert (Node "double"))
(Assert (Node "stdout"))

;; Two Edges of kind Flow connecting them in a chain.
(Assert (Edge from:@ticks  to:@double  kind:Flow))
(Assert (Edge from:@double to:@stdout  kind:Flow))

;; A Principal so the user has a default identity.
(Assert (Principal display_name: "operator" note: ""))
```

The `@name` form is the bind-back convention — the parser captures
each Assert's resulting slot under the name so subsequent records can
reference it. (Whether nexus already supports this in
assertion-positions is §10 Q11 below — the alternative is two passes,
or letting criome resolve names via the `name` field after the fact.)

#### 4.2.5 Seed in upgrade / reload paths

The "is sema empty?" check protects against re-seeding:

- Fresh install → seed runs → records appear in sema.
- Subsequent `mentci up` → query finds existing Graphs → seed skipped.
- `mentci down && rm sema.redb && mentci up` → seed runs again.
- `mentci reload criome` → no seed touched (criome restarts against
  the same sema; sema is non-empty; skip).

A `mentci up --reseed` flag could force a re-run for cases where the
user *wants* to re-seed (after editing `genesis.nexus`). Worth having;
small to add.

### 4.3 Respawn on crash

```
   tokio::select! observes child.wait() resolved
       │
       ▼
   match restart_policy:
       │
       ├── Never:     mark dead; if any dependent is alive, log warning;
       │              continue
       │
       ├── OnFailure: if exit code = 0, mark stopped and continue;
       │              else fall through to Always logic
       │
       └── Always:    schedule respawn after backoff:
                        backoff[0] = 250ms
                        backoff[N] = min(backoff[N-1] * 2, 60s)
                        reset backoff to 250ms after 60s of stable run
                      then re-launch (with same readiness probe)
                      and cascade restarts to dependents whose
                        connections need re-handshake
                      (criome restart → mentci-egui driver sees
                        *Disconnected → user clicks reconnect or
                        we auto-reconnect — see §10 Q5)
```

### 4.4 Reload / swap (upgrade flow)

```
   user types `nix run .#reload criome` in another shell
       │
       ▼
   the .#reload app:
       1. nix build .#criome             (produces /nix/store/<new-hash>/...)
       2. connect to ${run_dir}/supervisor.sock
       3. send (ControlMessage Swap name:"criome"
                                    binary:"/nix/store/<new-hash>/bin/criome-daemon")
       4. await reply
       │
       ▼
   running process-manager receives Swap on its control socket:
       │
       ├── stop dependents in reverse order
       │     (mentci-egui kept alive; its driver re-handshakes
       │      after step 4)
       │
       ├── SIGTERM old criome
       │     wait for exit (or SIGKILL after grace period)
       │
       ├── update DaemonSpec.binary to new path
       │
       ├── re-launch criome from new binary, run readiness probe
       │
       └── re-launch dependents that were stopped
       │
       ▼
   reply: Ok
       │
       ▼
   user's shell prints "criome upgraded; running."
```

The control protocol itself is a tiny rkyv-framed enum on a UDS — same
length-prefixed shape as signal but a different schema. Symmetry:
everything we run speaks length-prefixed rkyv.

### 4.5 Run loop

```
   loop:
     tokio::select! {
       signal::ctrl_c()      → break with reason "ctrl-c";
       signal::sigterm()     → break with reason "sigterm";
       child_exit(name, code)→ apply restart_policy(name, code);
       control_msg(msg)      → dispatch (Reload|Swap|Stop|Status);
       fs_event(path)        → if path under watched_src:
                                 emit log "rebuild needed for X";
                                 (rebuild itself is run by user, not process-manager)
     }
   end:
   shutdown_all(grace = 5s)
```

### 4.6 Tear-down

```
   shutdown signal received (Ctrl-C, SIGTERM, or `mentci down`)
       │
       ▼
   reverse depends_on order:
       mentci-egui first   (it can re-render its goodbye-state without
                            criome alive)
       nexus next
       criome last         (longest-lived because writes happen here)
       │
       ▼
   for each child in reverse order:
       send SIGTERM
       wait up to grace_period (default 5s)
       on timeout: SIGKILL
       │
       ▼
   remove sockets and the supervisor pid file
       │
       ▼
   process-manager exits with code 0 (clean) or 1 (any child needed SIGKILL)
```

---

## 5 · Nix wiring

### 5.1 Flake apps · `mentci/flake.nix`

```nix
apps.${system} = {
  up = {
    type = "app";
    program = "${pkgs.writeShellScript "mentci-up" ''
      set -euo pipefail
      mkdir -p "''${XDG_DATA_HOME:-$HOME/.local/share}/mentci"
      mkdir -p "''${XDG_RUNTIME_DIR:-/tmp/$UID}/mentci"
      exec ${self.packages.${system}.process-manager}/bin/process-manager \
        --config ${self.packages.${system}.process-managerConfig}/process-manager.nota
    ''}";
  };

  down.program = "${pkgs.writeShellScript "mentci-down" ''
    exec ${self.packages.${system}.process-manager}/bin/process-manager-control stop
  ''}";

  reload.program = "${pkgs.writeShellScript "mentci-reload" ''
    daemon="''${1:?usage: reload <daemon-name>}"
    new_path=$(nix build --no-link --print-out-paths .#"''${daemon}")
    exec ${self.packages.${system}.process-manager}/bin/process-manager-control \
      swap "$daemon" "$new_path/bin/$daemon"
  ''}";
};
```

The `process-manager` binary supervises; the `process-manager-control` binary (same crate,
`[[bin]]` entry) is the one-shot CLI for the control socket — a thin
sender of `ControlMessage` rkyv frames.

### 5.2 The generated config derivation

```nix
packages.${system}.process-managerConfig = pkgs.runCommand "process-manager-config" { } ''
  mkdir -p $out
  substitute ${./process-manager.nota.in} $out/process-manager.nota \
    --replace '@criome-daemon@'  "${self.packages.${system}.criome}/bin/criome-daemon" \
    --replace '@nexus-daemon@'   "${self.packages.${system}.nexus}/bin/nexus-daemon" \
    --replace '@mentci-egui@'    "${self.packages.${system}.mentci-egui}/bin/mentci-egui"
'';
```

All store paths land in the resolved config. process-manager itself never invokes
nix; nix produces the binaries and the config; process-manager runs them.

### 5.3 NixOS module (later, when CriomOS deploys this)

The same `Config` shape (typed in `process-manager`) becomes the input to a NixOS
module that emits one systemd unit per daemon plus one for `process-manager`
itself acting as a plain `Type=notify` service. Until then, process-manager
foreground-runs in the user's shell.

---

## 6 · How this composes with the dev shell

The `nix develop` shell (already exists in
[mentci/devshell.nix](../devshell.nix)) gains three convenience aliases:

```bash
alias up='nix run .#up'
alias down='nix run .#down'
alias reload='nix run .#reload'
```

When iterating on a daemon's Rust source, the loop is:

```
   edit code
     │
     ▼
   reload <daemon-name>
     │
     ▼  (under the hood)
   nix build .#<daemon>           ← cached; only changed crate rebuilds
   process-manager-control swap <name>       ← in-place hot-swap
     │
     ▼
   workbench shows the new behaviour without losing the running session
```

This is why process-manager is a separate process from nix, and why the swap path
goes through a control socket rather than process-manager watching the filesystem
itself: the **user controls when a swap happens**, nix is the build
tool, process-manager is the runtime.

(File-system watching process-manager could grow later — `process-manager watch on` to enable
auto-rebuild — but not in the first version. Per the
[push-not-pull discipline](../repos/tools-documentation/programming/push-not-pull.md),
the rebuild trigger is a *user action* arriving on the control socket,
not a poll of the source tree.)

---

## 7 · What I think of the ideas in your prompt

In order of how strongly I agree:

**`mentci up` / `mentci down` as the surface — yes, strongly.** This is
the right shape. A single command bringing the whole stack up keeps
the friction at zero, and it scales from "one user on a dev machine"
to "the same machinery wrapped in a NixOS module" without redesign. The
flake-app form (`nix run .#up`) is the right unwrapping until the
ergonomic shim (`mentci` CLI) shows it pulls its weight.

**Nota for configuration — yes, slightly with a caveat.** Eating our
own dog food validates the language at a low-stakes surface, and the
type-checker-as-validator wins are real. The caveat is bootstrapping:
nota-codec is a sibling crate that process-manager has to depend on, and changes
to the codec break process-manager's parser. Today the codec is small enough that
this risk is negligible; if it grows complex, process-manager could vendor a
minimal subset. I'd commit to nota for config and revisit if the
coupling ever bites.

**Coordinator as a small tool — yes, but it has to be its own crate.**
This is the [micro-components rule](../repos/tools-documentation/programming/micro-components.md):
new capability = new crate. Putting supervision into mentci or criome
is the failure mode the rule closes. Naming it `process-manager` is provisional —
see §10 Q1.

**Respawn on error — yes, with bounded backoff.** The bounded part is
load-bearing; without an exponential cap the supervisor turns crash
loops into fork bombs. The cap shape (250ms → 60s, reset after stable
run) is well-trodden territory.

**Rebuild / reconfigure on upgrade — yes, but the right shape is
"swap a binary path on the running supervisor"**, not "supervisor
rebuilds itself." nix builds; process-manager runs. Keeping that line clean is
the same discipline as criome-runs-nothing applied one level out.

**Replace a component with an upgraded version — yes, this is the
swap operation in §4.3.** The `Reload` is "re-read config from
disk and apply diffs." The `Swap` is "force this daemon to a new
binary path even if config didn't change." Both useful; both small.

**The bit I'd push back on:** nothing fundamental. The only place I'd
sharpen the framing is "a simple tool that will coordinate this." It
*should* be simple, but supervision touches process lifetimes,
filesystem state, signals, and IPC — there's load-bearing complexity
the simplicity has to *contain*, not deny. Beauty test: when the code
reads as obviously right, the simplicity is real; when it reads as a
loose collection of edge cases, process-manager is missing structure.

---

## 8 · What this design does *not* try to be

- **Not a separate-from-the-OS thing.** process-manager's
  terminal home is *inside* CriomOS as a system-level service
  module. The dev-shell foreground run via `nix run .#up` is
  the same binary, the same `Config`, the same control protocol —
  just invoked in user-session context until CriomOS deploys it.
  There is no second supervisor "for production."
- **Not a user-facing CLI surface.** No `mentci up` / `mentci
  down` / `mentci reload` shim on `$PATH`. Once process-manager
  is in CriomOS, "running mentci" means logging in and using the
  GUI shell — the daemons are already running as system
  services. The dev-shell `nix run .#up` is plumbing for the
  current era only; it does not graduate to a user-facing CLI.
- **Not a container orchestrator.** No images, no networks, no
  multi-host. One user, one host, one process-manager.
- **Not a log aggregator.** Pass-through to stdout/stderr today;
  structured logging is a separate concern in a separate crate
  if and when it lands.
- **Not a build tool.** nix is the build tool; process-manager
  runs binaries nix produced.
- **Not a replacement for systemd.** When process-manager is
  CriomOS-deployed, it runs *under* systemd as a `Type=notify`
  service supervising its children. systemd supervises
  process-manager; process-manager supervises the daemon
  cluster.

---

## 9 · A small invariant table

| concern | process-manager's answer |
|---|---|
| every reusable verb belongs to a noun | `Supervisor::launch_all`, `::shutdown_all`, `::swap` — never free functions |
| push, not pull | child-exit and control-socket events drive the loop; no polling of process state or filesystem (file-watching is opt-in and on a real `inotify`/`kqueue` notifier, not a poll) |
| typed-protocol IPC | control socket carries an rkyv-framed `ControlMessage` enum, length-prefixed exactly like signal |
| typed config | nota record → typed Rust struct via nota-codec |
| skeleton-as-design | the new `process-manager` repo opens with the shapes in §2.3 + a `flake.nix`, before the run loop has a body |
| one capability, one crate, one repo | process-manager is a new repo; the in-mentci-flake `apps.up/down/reload` is a thin shim |

---

## 10 · Status of the open questions

Most of the questions below resolve cleanly against principles
already in the workspace's design canon (per
[`tools-documentation/programming/`](../repos/tools-documentation/programming/)
+ [criome/ARCHITECTURE.md](../repos/criome/ARCHITECTURE.md)). The
genuinely open ones — the calls that need a personal preference,
not a principle — are a much shorter list.

### 10.1 Resolved by the principles

```
   Q   │ resolution                              │ grounded on
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q2  │ start every CANON daemon (option a):    │ beauty: uniform >
       │ criome + nexus + mentci-egui today;     │   conditional;
       │ adds forge + arca slots when those      │ perfect-specificity
       │ land                                    │   (criome ARCH §2D)
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q3  │ separate OS processes per daemon         │ micro-components.md:
       │                                          │   one capability,
       │                                          │   one crate, one
       │                                          │   binary
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q4  │ per-user persistent under XDG paths:    │ XDG conventions; per-
       │   sema.redb at ${XDG_DATA_HOME}/mentci/ │   project couples
       │   sockets at ${XDG_RUNTIME_DIR}/mentci/ │   state to working
       │   arca at ${HOME}/.arca/                │   directory; ephemeral
       │ --data-dir as escape hatch              │   loses sema across
       │                                          │   reboots
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q7  │ process-manager's parser knows about forge + arca  │ skeleton-as-design
       │ slots from day one; rejects unknown     │   (criome ARCH §13.6)
       │ daemons                                  │
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q8  │ TWO keys, two owners. process-manager   │ criome ARCH §10.2
       │ creates the directories and ensures      │   (criome owns
       │ permissions; neither key is its to       │   the capability-
       │ mint or hold:                            │   signing key);
       │  • criome's capability-signing key —    │ signal/src/auth.rs
       │    signs tokens forge/arca verify.      │   §AuthProof::
       │    Lives at ${XDG_DATA_HOME}/mentci/    │   BlsSignature
       │    criome/signing.bls (chmod 0600);     │   {signer:Slot}
       │    criome mints on first run.           │   names the per-
       │  • the user's per-Principal BLS key —   │   Principal signing
       │    each user signs every Frame's        │   path
       │    auth_proof with this. Lives at       │
       │    ${XDG_DATA_HOME}/mentci/principal.   │
       │    bls (chmod 0600); mentci-egui's      │
       │    bootstrap (or a small                │
       │    mentci-keygen one-shot) mints.       │
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q9  │ ship `process-manager watch on` from day one as    │ push-not-pull.md:
       │ a small inotify/kqueue-driven actor;    │   watcher must be
       │ off by default; emits "rebuild needed   │   event-driven, never
       │ for X" log lines                         │   a poll loop
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q10 │ rkyv-on-UDS with ControlMessage enum,   │ all-rkyv-except-
   Q12 │ length-prefixed exactly like signal     │   nexus-text rule
       │                                          │   (criome ARCH §10)
   ────┼──────────────────────────────────────────┼─────────────────────────
   Q11 │ first genesis.nexus uses fixed slot     │ criome ARCH §10
   (in │ values from the genesis-seed reserved   │   "Bootstrap rung by
   part│ range [0, 1024); bind-on-Assert in       │   rung" reserves the
   ─)  │ nexus is the long-term shape and lands  │   range; nexus-grammar
       │ when the parser supports it             │   evolution is the
       │                                          │   path forward
```

### 10.2 Genuinely open — the calls that need Li

**Q5 — Reconnect after intentional swap.** Provisional answer:
auto-reconnect on a process-manager-initiated swap (the swap is itself a
user-initiated state replacement, so auto-reconnect is *aligned*
with intent), keep the chip-click discipline for crashes (where
the user did not initiate the disconnect). The seam: process-manager's swap
path emits a control message to mentci-egui's driver flagging
the disconnect as intentional; the driver flips a one-shot
"auto-reconnect-on-next-disconnect" hint. Confirm this UX shape?

**Q11 (the rest) — What to seed.** The chain shape (a small
hand-crafted Graph + 3 Nodes + 2 Edges + a Principal) is in
§4.2.4. The actual *names* — whether to reuse the existing
handshake-test seed ("Echo Pipeline" / "ticks" / "double" /
"stdout") or pick fresh ones that frame what mentci is — are
open. This is the user's first impression of mentci on a fresh
install; what shows up matters more than the implementation
cost.

---

*End report 114.*
