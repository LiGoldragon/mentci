# ARCHITECTURE — mentci

Mentci is a first-class component daemon that hosts the programmable approval
surface for the local criome.

## 0.5 · Direction

Mentci is the human approval organ for the local per-Unix-user criome. It is a daemon because the programmable UI state is daemon-owned state: every TUI, CLI, editor integration, status bar, popup, and agentic client renders the same canonical state and submits events back to the daemon. Clients do not own approval logic; they subscribe to projected state and send typed responses.

Mentci also grows toward prompt-to-work routing, but as one of three components and not the owner of the flow: a prompt enters Mentci, `orchestrate` decides whether to reuse or create a session and opens it, and `harness` runs Claude **headless** and observes it. Mentci is the **view** onto that headless session — user-interface ingress and egress only, holding no provider, session-choice, or process-liveness logic. The harness process and terminal-cell beneath it are dumb infrastructure; a terminal-cell attach is an optional convenience for a raw terminal, not the required host through which the session must run. The first proof runs on a sandboxed jj task, not on primary. See the "Possible Future Design" section for the target architecture.

## Engines

Signal is external and lives in the two contract repos:

- `signal-mentci` is the working signal for programmable UI requests,
  responses, and events.
- `meta-signal-mentci` is the daemon configuration and reconfiguration signal.

This repo carries the daemon-local engines:

- `schema/nexus.schema` is the internal operations vocabulary. It reacts to
  arrived signal requests, commands SEMA writes and reads, frames criome
  escalations into questions, admits edited-answer proposals, routes closed
  verdicts, and publishes projected interface state to subscribers.
- `schema/sema.schema` is the durable state vocabulary. It defines the pending
  question family, decision family, edited-answer proposal family,
  subscription family, and singleton revision family.

## State Flow

`PresentQuestion` arrives through `signal-mentci`. Nexus commands the SEMA to
admit the question, minting a daemon-local identifier and bumping the
interface revision. The daemon then publishes projected state to matching
subscribers. Answering a question records one of the closed verdicts; editing a
suggestion admits a new typed answer proposal object instead of answering the
original question directly.

## Current Status

The first runtime slice is bootstrapped against canonical remote contract
crates, not local path dependencies:

- `mentci-daemon` takes exactly one binary startup file. The file is a
  length-prefixed `meta-signal-mentci` frame whose input payload is
  `Configure(MentciDaemonConfiguration)`.
- Startup configuration carries typed component socket endpoints. The daemon
  binds the `Mentci` socket and uses the `MetaCriome` socket for criome parked
  authorization pickup and approval submission when that socket is configured.
  Without `MetaCriome`, the daemon still binds and serves ordinary/read-only
  mentci observations but has no criome write bridge. Socket paths are not
  interpreted as generic ordinary/meta positions.
- `mentci` is the thin CLI client. It takes exactly one request input: a
  length-prefixed binary `signal-mentci` frame file, a `.nota` request file, or
  inline NOTA text. It connects to the local daemon socket and writes the binary
  reply frame to stdout.
- The same one-argument CLI also accepts observation atoms:
  `observe`, `observe:full`, `observe:pending`, `observe:status`, and
  `observe:notifications`. These commands still talk only to the mentci daemon
  and render the reply through `mentci-lib`'s shared `ObservationModel` and
  `RenderNota`.
- The CLI also accepts answer atoms:
  `answer:approve:<question>`, `answer:reject:<question>`, and
  `answer:defer:<question>`. These lower to `AnswerQuestion` on the mentci
  socket and render the typed daemon reply as NOTA text; they do not open a
  criome socket directly.
- The daemon speaks `signal-mentci` over Unix sockets with the shared
  `signal-frame` envelope and generated rkyv/NOTA nouns.
- `CriomeApprovalBridge` is daemon-owned. It lists criome's parked
  authorizations and submits closed decisions by `AuthorizationRequestSlot`;
  it never resubmits an `AuthorizationEvaluation` by value.
- `InterfaceState` full projections include `CriomeAccess`: `ReadWrite` when
  `MetaCriome` is configured and the daemon has the write bridge, `ReadOnly`
  otherwise. Thin clients mirror this mode and gate answer controls from it.
- `ObserveInterfaceState` checks the configured local criome meta socket for
  parked ClientApproval authorizations before projecting interface state, so a
  newly connected client sees criome-queued requests without a separate CLI
  polling step.
- The current SEMA implementation is in-memory. It is the executable shape of
  the daemon state machine, not yet the durable persisted family.

The remaining production gaps are durable SEMA storage, notification fan-out
events beyond request/reply, and turning observe-triggered parked-authorization
pickup into a continuous subscription/push loop. Those are integration gaps
around the runtime slice, not blockers to the contract-shaped daemon boot.

## Possible Future Design — Prompt-To-Work Sessions Across Mentci, Orchestrate, and Harness

This section is accepted target architecture for the next slice, not current daemon
behavior. It supersedes two earlier framings. The first, "Prompt-To-Bead-Weave
Harness Sessions," placed the terminal-cell driver and the harness adapters inside
Mentci, made `orchestrate` an address-only lane registry, and kept no harness daemon
in the loop. The second still treated a live terminal-cell PTY as the required host
that harness drives and Mentci mirrors. The psyche accepted a headless split
instead: **Claude runs headless as the engine, Mentci is the view, and the harness
process plus terminal-cell are dumb infrastructure.** A working demo proved the
headless engine, continuity across a full harness teardown, and self-healing resume
end-to-end (mentci commit `7a0c8e44`), so both older framings are retired.

The detailed typed message and durable record schemas for this design are not
duplicated here. They live in the accepted design spec in the primary workspace at
`agent-outputs/MentciOrchestrateSessionFlow/Design-SessionFlowSpec.md` (with its
adversarial review and headless-demo evidence alongside). This section records the
durable direction and boundaries; the spec carries the wire, store, and
headless-invocation detail an implementer needs.

### Three ownership regions

A prompt becomes running agent work by crossing three components, each owning one
concern and nothing more:

- **Mentci — the view; user-interface message ingress and egress only.** Mentci
  accepts a prompt from a client, forwards it, and renders the running session back
  to the client as its view. It holds no provider, launch, session-choice, or
  liveness logic; it does not distinguish Claude from any other harness, and it
  never spawns or drives a process. Any work-surface or hard-constraint values that
  ride with a prompt are opaque routing hints Mentci forwards verbatim — Mentci does
  not compute launch or sandbox posture. The view is not tied to one window: the
  design does not foreclose several Mentci windows rendering the same canonical
  session across multiple monitors, though multi-window rendering is not built yet.
- **Orchestrate — session choose, create, reuse, archive, and owner of the durable
  session store.** Orchestrate decides whether a prompt continues an existing session
  or starts a new one, opens the session, and records it in a durable store modeled
  on its existing `Worktree` record. The store's load-bearing field is the
  **resumable session-id**: that id, together with the session's stable working
  directory, is the durable carrier of a session across turns — not a live process.
  Session archive and garbage collection are orchestrate-owned and stop-driven.
  Orchestrate owns the session identity; it is not an address-only registry.
- **Harness — runs Claude headless, observes, and closes; ephemeral.** Harness builds
  the launch, runs Claude in headless mode as the engine, observes the streamed
  events and the transcript, and closes. It need not stay running between turns: once
  a turn is done the harness is free to tear down entirely, because the session
  survives as its resumable id in orchestrate's store (see the lifecycle section).
  The provider-neutral adapter that turns a launch request into a headless invocation
  and classifies output into neutral events lives here too.

The harness process and `terminal-cell` beneath it are **dumb infrastructure**, not
the seat of the session. `terminal-cell` stays a generic PTY primitive; attaching a
live terminal-cell view to a running session is an **optional convenience** for a
human who wants a raw terminal, no longer the required host through which the session
must run. Headless is the default engine path.

### Prompt treatment — a future meta-phase

Before a prompt reaches a session it will pass through **prompt treatment**: a future
meta-stage that reads an incoming prompt and decides what to do with it. This is
direction, not built work — nothing here specs it out. Prompt treatment is expected
to hold several sub-treatments:

- **Session routing** — the closed, fixed-schema model call that decides
  prompt-to-existing-versus-new-session and emits a launch plan (scaffold pointers,
  session identity, stop conditions, sandbox posture). This is the engine formerly
  called "preflight"; the rename is deliberate — "preflight" named *when* the call
  ran (at Mentci's front door), and the honest name is *what* it does — but it is now
  understood as **one sub-treatment of prompt treatment**, not the whole front door.
  It is a closed model call, distinct from the open-ended AI that runs inside a
  harness, and it composes against a per-turn **message router** (the deferred path
  that delivers subsequent turns into a live session): the session router runs once
  per prompt-to-session, the message router once per turn.
- **Intent detection**, and a **summarized feed** of each prompt into a long-lived
  meta / dialogue session kept synced across all sessions, are further sub-treatments
  the psyche has floated.

Only session routing is carried in the design spec. The wider prompt-treatment
meta-phase and its other sub-treatments are recorded here as direction and are
explicitly not designed or implemented now.

### Typed message flow and instance allocation

The three components talk over their typed signal contracts, producers pushing and
consumers subscribing rather than polling:

1. A client submits a prompt to Mentci over `signal-mentci`.
2. Mentci forwards it to orchestrate over `signal-orchestrate` (a new
   Mentci-to-orchestrate dependency that does not exist today).
3. Orchestrate runs prompt treatment (session routing), consults its session store,
   decides reuse-versus-create, and opens the session by driving harness over
   `signal-harness` — a fresh headless run for a new session, or a resume of the
   stored session-id for a reused one.
4. Harness runs Claude headless and pushes provider-neutral transcript and lifecycle
   events. Orchestrate subscribes to keep its store fresh — above all the recovered
   session-id — while Mentci subscribes to render the session as its view. Both watch
   the same session stream: many watchers of one session, not many sessions on one
   stream.

**A live turn runs on one harness instance, but the session is not the instance.**
While a turn is running it occupies a single harness instance drawn from the fixed
configured pool, and no two turns multiplex one instance. But the durable session is
the resumable id in orchestrate's store, not the instance: between turns the harness
may tear down completely, and the next turn resumes the same session-id — from the
session's stable working directory — on whatever instance is free. Many concurrent
sessions are realized as many instances; long, compaction-heavy runs coexist by
occupying distinct instances, never by contending for one slot; concurrency is
bounded by the pool and there is no eviction. Because distinct routes can race for
the last free instance, orchestrate reserves an instance atomically — conditional on
no other live turn already holding it — before it opens.

### Session lifecycle — done when it stops, never interrupted; the session outlives the harness

The governing principle, a fixed psyche ruling, frames the whole lifecycle: **a
session is done when it stops, and nothing may interrupt a running one.**
**"Interrupt" has a precise meaning: forcibly stopping a working agent mid-work —
not any injected message.** Guidance, query, and steering messages sent into a
session are not interrupts and are permitted, including injecting `/context` to
read the accumulated size and delivering the ~200K handover nudge; neither forcibly
stops the agent, and the run stays free to keep working. What the principle forbids
is a forced stop — eviction, archival, or forced handover — of a run that has not
itself stopped. A large flow may legitimately consume a great deal of context and
pass through several compactions before it finishes; that is normal and must never
trigger eviction, archival, or a forced handover.

**The session outlives the harness.** Because Claude runs headless, the harness
process is ephemeral: it need not stay running once a turn is done. The durable
carrier of a session across turns is the **resumable session-id tracked in
orchestrate's store**, not a live process. Continuing a session is *resuming that id
with the additional prompt* — a fresh headless run against the stored id, from the
session's stable working directory. A session between turns is therefore not "hot" in
any process; it is a resumable id waiting for its next prompt. The headless demo
proved this: a follow-up prompt recalled a fact from a first turn after that first
turn's harness had fully torn down (mentci commit `7a0c8e44`).

**Self-heal on a lost session.** A guidance or steering prompt may arrive for a
session whose id the engine no longer knows — a torn-down or expired session that
reports "no such session." That is handled by **re-resuming**: attempt the resume,
and if the id is truly gone, mint a fresh session and run the prompt into it, updating
the store. The lost-session error is a typed outcome that falls through to a fresh
launch, never a failure the user sees. The demo's self-heal path proved this too.

The consequences the design must preserve:

- A session leaves the hot set only on a harness-reported stop; there is no
  wall-clock age sweep. Under the headless/ephemeral model, "leaving the hot set" is
  the ordinary end of a turn: the harness tears down and the session reverts to a
  resumable id.
- **Staleness is measured in context size, not elapsed time.** A session's "age" for
  reuse and handover is the context (token) size it has accumulated, not how long it
  has been quiet. Around 100K tokens a session is long but fully resumable; around
  200K it is old, and the next prompt for that topic is *nudged* toward the
  workspace's context-handover discipline — wrap up and spawn a fresh session rather
  than resume into an ever-growing context. This is only ever a nudge, applied to an
  already-stopped session; it never interrupts a live run, and a handover-due session
  stays fully resumable if the flow chooses to continue.

**The context figure is the harness's own number, read passively — never
self-calculated.** Its **primary** source is the Claude Code statusline JSON payload
(its `context_window` block, carrying a native past-200K flag), which Claude Code
pushes to a passive, structured statusline command on its own cadence. That statusline
figure is primary and stays primary, because it is the only source that reports while
the agent is actively working mid-turn. Harness forwards that figure into its session
observations. It never sums transcript usage tokens itself — that transcript format is
documented as internal and version-unstable. Injecting `/context` into a session is a
permitted **at-rest-only fallback**: it is a query message, not an interrupt, but it
renders a usable figure only when the session is idle (between turns), so it is scoped
to at-rest sessions — inject `/context` and parse its rendered output when the primary
statusline figure is missing for an idle session. Where no figure has ever been
observed, the session is treated as of unknown size and fully reusable; no number is
synthesized to fill the gap.

**Open item — does headless emit the statusline? (to be settled by the live-view
build).** The context-size mechanism just above sources its figure from the Claude
Code statusline `context_window` payload, which assumed a running TUI/statusline. The
proven engine path is headless `claude -p`, where the statusline may never be emitted,
while the stream-json output instead carries a per-message `usage` token breakdown. If
headless emits no statusline, the context figure would have to come from that
stream-json `usage` — which reopens the "never self-calculate; read the harness's own
number" decision above, since a per-message usage total is closer to a self-sum than
to the harness's single authoritative figure. This is left unresolved on purpose: the
live-view build settles whether headless emits a usable statusline and, if not, which
stream-json usage figure is authoritative. The staleness mechanism is not rewritten
here.

### First proof and preserved constraints

The first proof domain is a sandboxed jj task, run headless, and must never run
against `/home/li/primary` as a jj working copy; private scope stays closed by
default. The scaffold stays minimal — `skills/skills.nota` as the expansion index
plus enough local context to start — and the session agent expands its own context
from there. The proof value is the working slice and the failure modes it exposes
(invalid routing output, missing required skills, sandbox violation, process start
failure, lost/expired session, idle timeout, stalled output, close failure,
adapter-level launch/read/write errors); no savings metric is required for the first
pass.

### Open decisions (not yet settled)

Recorded so they are not mistaken for accepted architecture; the design spec carries
the detail and evidence:

- **Headless statusline versus stream-json usage for the context figure** — see the
  marked open item in the lifecycle section. Whether headless `claude -p` emits a
  usable statusline, and if not which stream-json `usage` figure is authoritative, is
  to be settled by the live-view build; the staleness mechanism is not rewritten in
  the meantime.
- The optional live terminal-cell attach view: settled intent makes terminal-cell an
  optional convenience rather than the required host, so reaching a working attached
  terminal view through terminal-cell's launch surface (versus the archived
  terminal-daemon) is unproven and only matters for that optional view, not for the
  headless engine path.
- Provider and model vocabulary: one owner for the session record's provider kind,
  one model type across its lifecycle (launch knob, observed, stored), and a
  provider-neutral resume locator.
- Harness peer naming: after this design harness has at least two ordinary peers —
  orchestrate (opens sessions) and the deferred message-router (delivers turns) — so
  the harness contract's "router" prose should name both explicitly.
- Exact Claude Code statusline field spellings and their stability against the
  installed Claude Code version (relevant only if headless is found to emit the
  statusline at all).
