# 128 — Session handoff: end-of-day 2026-05-02

*Compact handoff for the next session. Captures the gas-city
adoption arc — packaging, deployment, philosophy-city build-out,
operational lessons. Lifetime: until the next session reads it
and supersedes or deletes.*

---

## 0 · Read first (in order)

1. `lore/INTENTION.md` — what the project is for.
2. `lore/AGENTS.md` — workspace contract.
3. `workspace/AGENTS.md` — workspace-meta-repo carve-outs.
4. `workspace/reports/126-gas-city-pivot-2026-05-01.md` — the
   architecture / fit decision; still load-bearing.
5. `workspace/reports/127-gas-city-hardcore-usage-2026-05-01.md` —
   the operator's manual; reference for vocabulary / patterns.
6. `lore/gas-city/basic-usage.md` and `vocabulary.md` — daily-use
   ops reference.
7. This report.

---

## 1 · What landed this session

### Packaging
- **`gascity-nix`** repo created at `~/git/gascity-nix/` and
  `github:LiGoldragon/gascity-nix`. `buildGo125Module` over
  upstream `gastownhall/gascity`, currently tracking
  `origin/main` past v1.0.0 — v1.0.0 shipped without the bd-init
  timeout fix (#1264) and without the mksh-shebang fix we needed
  for CriomOS. `postPatch` rewrites every `#!/bin/sh` shebang in
  embedded `examples/` scripts to bash before the Go embed step
  bakes them in — required because `environment.binsh = mksh` on
  CriomOS strips bash idioms from gas-city's bd-start script.

### Deployment chain
- **CriomOS-home** — `gascity` and `annas-mcp` flake inputs added,
  `cli-tools.nix` packages: `gc`, `bd`, `dolt`, `tmux`, `lsof`,
  `procps`, `util-linux`, plus `annas` (Anna's Archive CLI built
  inline + wrapped with gopass-driven env injection). Dropped the
  bespoke `buildGoModule` for beads — using `pkgs.beads` from
  nixpkgs everywhere now.
- **`programs.git.settings.beads.role = "maintainer"`** added
  declaratively so gas-city's bd-init script's `git config
  --global beads.role` check passes against the read-only
  home-manager-managed `~/.config/git/config`.
- **CriomOS** — `criomos-home` lock bumped, deployed via
  `lojix-cli switch` (current generation: 83+). Mksh-as-/bin/sh
  preserved (intentional CriomOS choice in
  `modules/nixos/normalize.nix:74`); the gascity-nix flake's
  shebang patch handles it at the package level.

### Philosophy-city
- New repo at `~/philosophy-city/`,
  `github:LiGoldragon/philosophy-city`. Five-agent ensemble
  driving conversation about engineering aesthetics:
  - **mayor** (claude, max effort, `mode = "always"`) —
    orchestrator
  - **aesthete** (claude, max) — beauty
  - **theorist** (claude, max) — correctness
  - **pragmatist** (codex, xhigh) — cost / shipping
  - **devil** (codex, xhigh) — opposition
  - Mayor later added **librarian** (codex) and **researcher**
    (claude) as city-scoped agents; matching directories
    `library/` and `research/` registered as rigs.
- Prompt design landed on the minimal-personality-stanza shape
  (per Li's Bitter-Lesson framing) on top of gas-city's default
  agent operational scaffolding (`bd ready` → read → reply in
  notes → close → loop). The personality stanza is one to three
  lines — name the concern, don't script its content. The model
  brings the substance per topic.

### Bug fixes shipped
- gas-city v1.0.0 → main HEAD bump caught the bd-init 30s
  timeout fix; `gc start` now completes in ~3s instead of timing
  out at 48s.
- Shebang patch (described above) for mksh-as-/bin/sh.
- `programs.git.settings.beads.role` for the read-only gitconfig
  problem.
- Researcher / librarian had `bd ready --rig <name>` in their
  prompts but were declared city-scoped; slings missed them
  silently. Dropped the `--rig` flag from both prompts.
- Convoy validation bug on rig stores
  (`invalid issue type: convoy`): bd v1.0.0's runtime config
  table didn't honor the YAML `types.custom` fallback. Fixed by
  explicit `bd config set types.custom "..."` against each rig
  store. Auto-convoys for rig-scoped slings work now.
- `[[named_session]]` schema for rig-scoped agents documented:
  use `scope = "rig"` (NOT slash-prefix in template, NOT a
  `binding` sub-table). Confirmed via gas-city source
  `internal/config/config.go:303`.

### Mayor's autonomous-shutdown habit
- Mayor at one point ran `gc stop` on its own (probably during a
  perceived "wind-down"). Later it tried `gc restart` after the
  first prohibition was too narrow.
- Resolved: mayor's prompt now forbids the full lifecycle command
  set: `gc stop / start / restart / unregister / init`,
  `gc supervisor *`, and supervisor-API register/unregister.
  Allowed: everything else (sling, mail, bd, session, rig, agent,
  formula, order, status, reload, prime, handoff, service, skill).
- See `philosophy-city/agents/mayor/prompt.template.md` —
  "City lifecycle is Li's, not yours" section.

### tmux Shift+Enter inside city sessions
- Runtime fix applied: `tmux -L philosophy-city set-option -g
  extended-keys on`. Not yet persistent across city restarts.
  Durable fix is either upstream-patching gas-city's
  `internal/runtime/tmux/tmux.go` (where it already sets
  `mouse off`) or per-agent `session_setup` on mayor.

---

## 2 · Current state

### Repos
| Repo | Branch | Latest | Notes |
|---|---|---|---|
| `gascity-nix` | main | `ca22370` | tracks gascity origin/main + shebang patch |
| `CriomOS-home` | main | `3489cca` | annas + gascity wired |
| `CriomOS` | main | `2edfd2f` | criomos-home bumped, deployed |
| `philosophy-city` | main | `778b425` | mayor full-lifecycle prohibition |
| `lore` | main | `2024c8f` | gas-city/basic-usage.md + vocabulary.md |

### Running services
- `gascity-supervisor.service` (systemd --user) — alive.
  `Linger=no`, so it dies on logout. Re-enable trivially with
  `sudo loginctl enable-linger li` if persistence wanted.
- `philosophy-city` registered with the supervisor.
- Mayor session always-on; on-demand workers spawn when slung.

### Open threads waiting on mayor (in mayor's mail)
| Mail bead | Subject | Status |
|---|---|---|
| `pc-wisp-d9k` | Fixed: rig-scoped named_session syntax + convoy type bug | unread |
| `pc-wisp-o19` | Rig audit: dolt at 38% CPU from 3 dispatchers | unread |
| `pc-wisp-2jr` | Slow dolt polling | unread (superseded by 5ci) |
| `pc-wisp-5ci` | Suspend the rigs — they're the dolt CPU culprit | unread |

Mayor will surface all four on its next turn. The instruction is
to suspend `library` and `research` rigs (they're vestigial — no
agents pull work from their stores) and to write a justification
audit for keeping the rigs at all.

### Active work beads in philosophy-city
- `pc-sn48` (Librarian: add Blake, Bastiat, Montesquieu — natural-law tier) — slung, unclaimed
- `pc-jswt` (Researcher: greatest Vedic works on balanced thinking/acting/resting) — slung, unclaimed
- Plus 17 order-spawned dog-patrol beads that auto-respawn on cooldown (visual noise; ignore them in normal use)

### Known irritations
- **Dolt CPU ~28-38%** under load. Diagnosed: hardcoded
  `workflowServeWakeSweepInterval = 1s` × 3 dispatchers (city +
  library + research) × 4-6 bd queries per tick. Mayor instructed
  to suspend the two rig dispatchers (`gc rig suspend library`,
  `gc rig suspend research`). The city + 4 conversational seats
  are plenty; rigs were premature.
- `gc reload` reports "No config changes detected" on
  prompt-only edits. Annoying but not broken — the prompt is
  delivered to the *next* spawn, not pushed into running
  sessions. To force, `gc session kill <name>`.
- Mayor doesn't auto-check mail when idle. Mail accumulates;
  delivered on the next turn. Operator pattern: send mail, then
  `gc session nudge mayor "..."` if you want immediate action.

---

## 3 · Where to pick up

Pick one based on energy:

1. **Get the rigs suspended.** Mayor has the audit + suspend
   instructions in mail. After it processes them, dolt CPU
   should drop to ~10-15%. If mayor stalls, run the suspends
   yourself: `gc rig suspend library` and `gc rig suspend
   research` from the city root.
2. **Make the tmux Shift+Enter fix durable.** Either patch
   gas-city upstream (`internal/runtime/tmux/tmux.go` — add
   `set-option -g extended-keys on` next to where it does
   `set-option -t name mouse off`) and ship via the gascity-nix
   flake, or add `session_setup` on mayor.
3. **Have a real conversation with the city.** That's the whole
   point — sling a topic to the four-seat ensemble and read
   their replies + mayor's synthesis. The aesthete/theorist
   already produced solid work on Rust `Option` patterns
   yesterday; the codex side (pragmatist/devil) still needs
   substantive testing.
4. **Decide the librarian / researcher long-term shape.** They
   exist as city-scoped agents using rig directories as
   workspaces. If you want them strictly rig-scoped (each pulls
   only from its own rig's bead store, addressed as
   `library/librarian` etc.), that's a pack.toml refactor
   (`dir = "library"` field + change all sling paths). Mayor
   can do it, but only if we conclude the strict isolation is
   worth the addressing overhead. Default recommendation: stay
   city-scoped, the rig directories are just folders.

---

## 4 · Conventions worth carrying forward

Discovered or confirmed this session:

- **The "minimum personality stanza" prompt shape.** One concern,
  one line. Operational scaffolding from gas-city's defaults.
  Don't smuggle taste-priors into agent prompts; let positions
  emerge in dialogue. ([Bitter Lesson](http://www.incompleteideas.net/IncIdeas/BitterLesson.html)
  applied to prompt engineering.)
- **Mayor's lifecycle blocklist.** Any city you author for an
  autonomous mayor needs the same prohibition. Otherwise mayor
  *will* "tidy up" by stopping the city. The `core.gc-city` skill
  in the auto-injected prompt appendix advertises the stop
  command — mayor knows how, finds reasons to use it.
- **Rig dispatchers are the dolt CPU bottleneck**, not the
  controller's `patrol_interval`. Each rig you add costs ~12-15%
  baseline dolt load. Add a rig only when an agent actually pulls
  work from its store.
- **`/bin/sh = mksh` on CriomOS** is intentional and wired in
  `CriomOS/modules/nixos/normalize.nix`. Anything that ships
  bash-flavored `#!/bin/sh` scripts needs a shebang patch in our
  packaging layer.
- **Per-Li**: "create a repo" always means public on GitHub.
  Saved in `bd remember`.

---

## 5 · Files / docs inventory

```
~/git/gascity-nix/                     # public, our flake
~/git/CriomOS/                         # ours, deployed
~/git/CriomOS-home/                    # ours, deployed
~/git/philosophy-city/                 # ours, public
~/git/lore/gas-city/                   # docs (basic-usage + vocabulary)
~/philosophy-city/                     # the running city directory
  agents/{mayor,aesthete,pragmatist,theorist,devil,librarian,researcher}/prompt.template.md
  pack.toml
  city.toml
  library/                             # rig dir (suspension pending)
  research/                            # rig dir (suspension pending)
  .gc/                                 # runtime, gitignored
~/git/workspace/reports/
  122 — schema bootstrap arc (active, sema/schema)
  124 — schema brainstorm raw (active, awaiting fold)
  126 — gas-city pivot
  127 — gas-city hardcore usage
  128 — this handoff
```

---

## 6 · Open questions / decisions waiting

- **Bwrap cage** for philosophy-city agents. Designed but not
  built. Currently the workspace-boundary rule is prompt-level
  only; agents could write outside `~/philosophy-city/` if they
  decided to. The cage seam is the `exec` provider script
  wrapping `bwrap` (per report 126 §5). Defer until you see an
  agent actually misbehave; the prompt-level guard has been
  enough so far.
- **stylix → claude-code theme drift.** Claude's theme is in
  `~/.claude/settings.json` as a user-managed file; stylix
  doesn't write there. Right durable answer is `inputs.hexis.lib.mkManagedConfig`
  (already a flake input) writing only the `theme` key based on
  `config.stylix.polarity`. Module not yet authored.
- **gas-city upstream PR** for the mksh shebang issue. Worth
  filing once we've used the patched build for a few days. Same
  for an `extended-keys = on` PR for the tmux setup.
- **Persistent linger** (`loginctl enable-linger li`). Not
  enabled. Cities die on logout. Acceptable on a workstation;
  worth flipping if you start treating philosophy-city as a
  daemon you talk to throughout the day.
