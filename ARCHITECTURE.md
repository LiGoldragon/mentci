# ARCHITECTURE — mentci

Mentci is a first-class component daemon that hosts the programmable approval
surface for the local criome.

## 0.5 · Direction

Mentci is the human approval organ for the local per-Unix-user criome. It is a daemon because the programmable UI state is daemon-owned state: every TUI, CLI, editor integration, status bar, popup, and agentic client renders the same canonical state and submits events back to the daemon. Clients do not own approval logic; they subscribe to projected state and send typed responses.

Mentci also grows toward prompt-to-work routing, but as one of three components and not the owner of the flow: a prompt enters Mentci, `orchestrate` decides whether to reuse or create a session and opens it, and `harness` runs and observes that session through terminal-cell. Mentci itself stays user-interface ingress and egress only — it holds no provider, session-choice, or process-liveness logic. The first proof runs on a sandboxed jj task, not on primary. See the "Possible Future Design" section for the target architecture.

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
  length-prefixed binary `signal-mentci` frame file, a `.dotos` request file, or
  inline DOTOS text. It connects to the local daemon socket and writes the binary
  reply frame to stdout. The text form is `signal-mentci`'s own, and
  `signal-mentci` is still a DOTOS contract; when that contract is rewritten
  onto the Datom stack this input becomes datom text and this crate follows it.
- The same one-argument CLI also accepts observation atoms:
  `observe`, `observe:full`, `observe:pending`, `observe:status`, and
  `observe:notifications`. These commands still talk only to the mentci daemon
  and render the reply through `mentci-lib`'s shared `ObservationModel`.
- The CLI also accepts answer atoms:
  `answer:approve:<question>`, `answer:reject:<question>`, and
  `answer:defer:<question>`. These lower to `AnswerQuestion` on the mentci
  socket and render the typed daemon reply through `mentci-lib`; they do not
  open a criome socket directly.
- The daemon speaks two wires. The Mentci wire carries `signal-mentci` over
  Unix sockets inside the shared `signal-frame` envelope, because that contract
  has not yet moved. The introspect wire carries `signal-introspect` 2.0
  portable rkyv `Signal` frames of its `Query` and `Response` types: four
  big-endian length bytes and exactly that many contract bytes, with no
  envelope, no route and no sub-reply layer.
- What the daemon shows of an introspect reply is the contract's own canonical
  datom text. `IntrospectionObservation` is the pane body as a value, so every
  pane the daemon renders can be read straight back into that type.
- Preflight launch packets are datom values, described in
  `schema/preflight-launch.datom.md` and defined by
  `mentci::preflight::MentciPreflightLaunch`. The model is asked for one
  canonical datom value; the type is the whole schema, and a packet that does
  not compose is refused by arity and variant head before any validation rule
  runs.
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
behavior. It supersedes the earlier "Prompt-To-Bead-Weave Harness Sessions"
direction, which placed the terminal-cell driver and the harness adapters inside
Mentci, made `orchestrate` an address-only lane registry, and kept no harness daemon
in the loop. The psyche accepted the opposite ownership split, so that older
direction is retired.

The detailed typed message and durable record schemas for this design are not
duplicated here. They live in the accepted design spec in the primary workspace at
`agent-outputs/MentciOrchestrateSessionFlow/Design-SessionFlowSpec.md` (with its
adversarial review alongside). This section records the durable direction and
boundaries; the spec carries the wire and store detail an implementer needs.

### Three ownership regions

A prompt becomes running agent work by crossing three components, each owning one
concern and nothing more:

- **Mentci — user interface, message ingress and egress only.** Mentci accepts a
  prompt from a client, forwards it, and renders the live session back to the
  client. It holds no provider, launch, session-choice, or liveness logic; it does
  not distinguish Claude from any other harness, and it never spawns or drives a
  process. Any work-surface or hard-constraint values that ride with a prompt are
  opaque routing hints Mentci forwards verbatim — Mentci does not compute launch or
  sandbox posture.
- **Orchestrate — session choose, create, reuse, archive, and owner of the durable
  session store.** Orchestrate decides whether a prompt continues an existing
  session or starts a new one, allocates a harness instance, opens the session on
  it, and records the session in a durable store modeled on its existing `Worktree`
  record. Session archive and garbage collection are orchestrate-owned and
  stop-driven. Orchestrate owns the session identity; it is not an address-only
  registry.
- **Harness — Claude launch, observe, and close, driven through terminal-cell.**
  Harness builds the launch command, drives the process through the external
  `terminal-cell` PTY primitive, observes the running session, and closes it.
  Liveness — the send and read loop, idle and stall detection, exit — lives in
  harness, not in Mentci. The provider-neutral adapter that turns a launch request
  into argv and classifies transcript output into neutral events lives here too.

`terminal-cell` stays a generic PTY primitive underneath harness; it is not harness-
or provider-specific.

### Session routing — the closed model call

A cheap, contained model call reads each incoming prompt and decides prompt to
existing-versus-new session, emitting a fixed-schema plan (scaffold pointers,
session identity, stop conditions, sandbox posture). This is the engine formerly
called "preflight"; it is renamed **session routing** (`SessionRouter` /
`RouteSession` / `SessionRoutingPlan`) and owned by orchestrate. The rename is
deliberate: "preflight" named *when* the call ran (at Mentci's front door); the
honest name is *what* it does — route a prompt to a session decision. It is a
closed, fixed-schema model call, distinct from the open-ended AI that runs inside a
harness. It also composes cleanly against the per-turn **message router** — the
deferred path that delivers subsequent turns into a live session: the session router
runs once per prompt-to-session, the message router once per turn.

### Typed message flow and instance addressing

The three components talk over their typed signal contracts, producers pushing and
consumers subscribing rather than polling:

1. A client submits a prompt to Mentci over `signal-mentci`.
2. Mentci forwards it to orchestrate over `signal-orchestrate` (a new
   Mentci-to-orchestrate dependency that does not exist today).
3. Orchestrate runs the session-routing model call, consults its session store,
   decides reuse-versus-create, reserves a free harness instance, and opens the
   session on it over `signal-harness`.
4. Harness drives the session through terminal-cell and pushes provider-neutral
   transcript and lifecycle events. Orchestrate subscribes to keep its store fresh;
   Mentci subscribes to render live output. Both watch the same instance stream —
   many watchers of one session, not many sessions on one stream.

**One session per harness instance.** A harness instance is fixed at daemon startup,
carries one harness kind and one terminal endpoint, and hosts exactly one live
session; its name is the whole live-session key. There is no session multiplexing
inside an instance. Many concurrent sessions are realized as many instances drawn
from the fixed configured pool — long, compaction-heavy runs coexist by occupying
distinct instances, never by contending for one slot. Concurrency is bounded by the
pool, and there is no eviction. Because distinct routes can race for the last free
instance, orchestrate reserves an instance atomically — conditional on no other live
session already holding it — before it opens, so two routes cannot double-book one
instance. The durable session identity outlives any single instance: a session goes
idle when its agent stops and later resumes on whatever instance is free.

### Session lifecycle — done when it stops, never interrupted

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
trigger eviction, archival, or a forced handover. The consequences the design must
preserve:

- A session leaves the hot set only on a harness-reported stop. There is no
  wall-clock age sweep.
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
pushes to a passive, structured statusline command on its own cadence. That
statusline figure is primary and stays primary, because it is the only source that
reports while the agent is actively working mid-turn. Harness forwards that figure
into its session observations. It never sums transcript usage tokens itself — that
transcript format is documented as internal and version-unstable. Injecting
`/context` into a session is a permitted **at-rest-only fallback**: it is a query
message, not an interrupt, but it renders a usable figure only when the session is
idle (between turns), so it is scoped to at-rest sessions — inject `/context` and
parse its rendered output when the primary statusline figure is missing for an idle
session. Where no figure has ever been observed, the session is treated as of unknown
size and fully reusable; no number is synthesized to fill the gap.

### First proof and preserved constraints

The first proof domain is a sandboxed jj task and must never run against
`/home/li/primary` as a jj working copy; private scope stays closed by default. The
scaffold stays minimal — `skills/skills.dotos` as the expansion index plus enough
local context to start — and the session agent expands its own context from there.
The proof value is the working slice and the failure modes it exposes (invalid
routing output, missing required skills, sandbox violation, process start failure,
idle timeout, stalled output, close failure, adapter-level launch/read/write errors);
no savings metric is required for the first pass.

### Open decisions (not yet settled)

Recorded so they are not mistaken for accepted architecture; the design spec carries
the detail and evidence:

- terminal-cell versus the archived terminal-daemon for the live proof: settled
  intent picks terminal-cell, but reaching a working Claude TUI through
  terminal-cell's launch surface is unproven.
- No resume-id validity probe exists anywhere; a failed `--resume` is handled by
  attempt-and-fall-through to a fresh launch, which must be shown to fail gracefully.
- Provider and model vocabulary: one owner for the session record's provider kind,
  one model type across its lifecycle (launch knob, observed, stored), and a
  provider-neutral resume locator.
- Harness peer naming: after this design harness has at least two ordinary peers —
  orchestrate (opens sessions) and the deferred message-router (delivers turns) — so
  the harness contract's "router" prose should name both explicitly.
- Exact Claude Code statusline field spellings and their stability against the
  installed Claude Code version.
