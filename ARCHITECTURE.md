# ARCHITECTURE — mentci

Mentci is a first-class component daemon that hosts the programmable approval
surface for the local criome.

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

## Possible Future Design — Prompt-To-Bead-Weave Harness Sessions

This section is target architecture for the next thin slice, not current daemon
behavior. Mentci becomes the entry surface for aligned prompts that should turn
into a weave of BEADS jobs and a running agent harness session.

The slice stays harness-agnostic. A prompt enters Mentci, a cheap contained API
preflight model analyzes the prompt, and the preflight emits fixed-schema NOTA
that names the scaffold identity, minimal scaffold/context pointers to
mount, session identity, persistent harness session request,
sandbox/privacy flags, and typed stop conditions. The preflight is the routing
and prompt-building engine; it is not a deterministic rule router. Thinness is
intentional: the scaffold includes only the minimal support plus
`skills/skills.nota`, and the session agent expands its own context from there.

The fixed preflight launch schema is the NOTA contract artifact at
`schema/preflight-launch.nota.md`. It is the canonical schema surface for
scaffold identity and version, scaffold/context pointers, session identity,
persistent-session request, sandbox/privacy posture, typed stop conditions, and
residual launch constraints. Adapter identity, terminal-cell driver identity,
provider model identifiers, and terminal launch policy are downstream
adapter/session launch-plan details, not fields in this front-door packet.

The session is persistent, named, and addressable. `orchestrate` lanes own the
lane name, lane metadata, addressing, and session lookup. The terminal-cell
driver owns process liveness: process handle, send/read loop, idle timeout,
close signal, and stalled-output detection. Harness adapters plug into that one
driver. Claude Code, Codex, pi, and open-ended shells are current adapter
identity examples, not generic contract semantics.

The first proof domain is a sandboxed jj task. It must not run against primary.
The proof value is the working slice and the failure modes it exposes; no
rigorous savings metric is required for the first pass. Scaffold identities are
versioned in the first schema, while reuse and caching mechanics stay deferred
until the thin slice exists.

### Persistent Harness Session Addressing Contract

This is the contract between Mentci's prompt-to-work surface, `orchestrate`
lanes, and the terminal-cell driver. It defines identity and lookup only; it is
not a backend integration plan and it does not make `orchestrate` the owner of
process liveness.

Mentci requests a named harness session by sending `orchestrate` a
session-address request derived from the preflight output:

```nota
;; Pseudo-NOTA for documentation, not the wire schema.
(HarnessSessionAddressRequest <session-identity> <persistent-session> <launch-metadata> <sandbox-privacy>)
;;   session-identity  : (SessionIdentity <lane-name> <lane-metadata> <addressable-handle> <lookup-path>)
;;   lane-metadata     : (LaneMetadata <discipline> <session-intent> <harness-kind> <adapter-kind> <scaffold-identity> <scaffold-version>)
;;   persistent-session: (PersistentSession <requested> <harness-kind> <adapter-kind> <driver-kind>)
;;   launch-metadata   : adapter/session-owned metadata outside MentciPreflightLaunch
```

`session-identity` is the durable address. `persistent-session` is the launch
request that says Mentci wants a long-lived harness session. They stay separate:
an address can be stored, matched, diagnosed, or retired without interpreting
the persistence boolean as identity.

`orchestrate` owns the lane/session address record:

- The `lane-name` is the stable lookup key and must be derived from the session
  intent, not from the harness provider.
- `lane-metadata` records the discipline, session intent, harness kind, adapter
  kind, scaffold identity, and scaffold version.
  It records no process handle, no read/write loop state, no idle timer, and no
  stalled-output detector.
- The `addressable-handle` is the token Mentci returns to later callers. It is
  an address for routing a future feed/read/close request, not proof that a
  process is alive.
- The `lookup-path` names the `orchestrate` lane lookup surface that can resolve
  the handle back to the lane/session address record.

On first launch, `orchestrate` registers the lane/session address if no record
exists. If a record already exists for the requested name with matching identity
metadata, it returns the existing address instead of minting a duplicate. If the
name exists with different identity metadata, the request is a typed address
conflict and no terminal-cell process is started.

Later Mentci operations address the session in two steps:

1. Resolve the handle or lane name through `orchestrate` lane lookup.
2. Pass the resolved terminal-cell addressing data to the terminal-cell driver
   for live feed/read/close work.

The terminal-cell driver owns liveness for the resolved session: process handle,
send/read loop, idle timeout, close signal, stalled-output detection, and the
adapter-level launch/read/write errors. `orchestrate` may diagnose "unknown
address", "address conflict", or "known address is closed/retired"; it must not
diagnose "the process is healthy" from lane metadata.

Review witnesses for this contract:

- Existing-session lookup: a second request for the same lane name and matching
  metadata returns the original addressable handle and does not start a second
  terminal-cell process.
- Unknown-session diagnosis: a feed/read/close request whose handle has no
  `orchestrate` lane address record fails as an unknown address before reaching
  the terminal-cell driver.
- Closed-session diagnosis: a request whose address record is closed or retired
  fails as closed at the address layer; a process that stalls, exits, or misses
  idle timing is diagnosed by terminal-cell, not by `orchestrate`.
- Privacy/sandbox guard: every first-proof address carries sandbox/privacy
  metadata requiring a sandboxed jj task, forbidding `/home/li/primary` as the
  jj working copy, and keeping private scope closed by default.

### Harness Adapter Contract

Harness adapters are thin translation objects over one terminal-cell-backed
driver. The adapter knows how to turn Mentci's typed launch request into the
argv, environment, initial terminal input, later terminal input, output-event
classification, and metadata for one harness family. The terminal-cell driver
owns the running process and PTY lifecycle for every adapter.

The generic adapter surface is:

- **Identity and capability metadata.** The adapter reports an adapter identity,
  a harness-kind identity, a contract version, supported launch knobs, supported
  input modes, output event classes it can report, close modes it can request,
  and whether the implementation is verified for the current proof slice. Known
  registry examples include Claude Code, Codex, pi, and an open-ended shell
  harness, but those names are examples of adapter identities only. No behavior
  is inferred from a provider name by the generic contract.
- **Launch command construction.** The adapter receives the scaffold path,
  requested working directory, sandbox/privacy flags, semantic harness-session
  model knob, environment overlay, and initial prompt object. It returns a
  terminal launch plan: executable, argv, environment, working directory,
  terminal size preference, and optional initial input bytes. The adapter does
  not spawn the process; the terminal-cell driver does.
- **Initial prompt and scaffold handoff.** The adapter receives a typed object
  containing the user prompt, the minimal scaffold identity, mounted source
  locators, selected skills, stop conditions, and proof constraints. It may
  render that object into the child process' initial terminal text, a file inside
  the scaffold, or both, but it must report which handoff path it used. The
  rendered prompt is adapter-owned text; the typed object remains Mentci's
  contract surface.
- **Model knob mapping.** The adapter maps the semantic harness-session model
  knob into whatever command-line option, environment value, prompt text, or
  no-op its harness supports. Concrete provider model identifiers are not part
  of this contract. Unsupported or unverified model mapping returns a typed
  adapter error instead of guessing.
- **Send framing.** Later Mentci input reaches the adapter as typed feed
  objects. The adapter renders each feed to terminal bytes and names whether it
  expects line-oriented input, raw bytes, a file handoff plus trigger text, or
  no interactive feed support. The terminal-cell driver writes the bytes through
  its single PTY input path.
- **Read/event framing.** The terminal-cell driver supplies transcript deltas,
  worker lifecycle events, terminal exit, idle timeout, stalled-output detection,
  and close results. The adapter may classify transcript deltas into generic
  events such as output observed, prompt requested, completion signaled, or
  adapter diagnostic. Generic Mentci code must not depend on provider-specific
  transcript wording; an unclassified transcript delta is still valid output.
- **Close behavior.** The adapter declares the close request it supports:
  graceful terminal input, interrupt, terminate, kill, or driver-default close.
  The driver performs the close and reports terminal outcome. Adapter close
  logic may request a rendered pre-close input sequence, but it cannot own the
  process handle or decide liveness.
- **Error reporting.** Adapter errors are typed by phase: unsupported
  capability, invalid launch request, launch-plan construction failure, prompt
  rendering failure, model mapping failure, feed rendering failure, event
  classification failure, close request failure, and unverified adapter detail.
  Driver errors stay driver errors: process start failure, PTY/control-socket
  failure, write failure, read failure, idle timeout, stalled output, terminal
  exit, and close failure.

The terminal-cell driver responsibilities are centralized and adapter-neutral:
process handle, child PTY, send/read loop, transcript capture, terminal worker
lifecycle, idle timeout, stalled-output detection, close signal, terminal exit,
and the final terminal outcome. The driver may expose transcript and worker
events to an adapter for classification, but it never calls adapter-specific
code to decide whether the process is alive.

The first proof adapter for a sandboxed jj task only needs enough capability to
construct a launch plan, hand off the typed scaffold/prompt, render one feed
input, surface raw output plus any generic completion signal it can verify,
request a close, and return typed adapter errors. It does not need full parity
with every registered adapter identity, verified quota/usage parsing, concrete
model identifier selection, or provider-specific transcript semantics.

### First-Slice Acceptance Contract

This contract is the acceptance gate for the thin routing slice. It names the
observable behavior that must hold before implementation beads may treat the
prompt-to-harness path as proven.

- A prompt enters Mentci through a single explicit request path. The request
  preserves the prompt text, the requested work surface, and any hard
  constraints, including the requirement that the first jj proof is sandboxed
  and never runs against primary.
- The first pass is an API preflight. It analyzes the prompt and builds the
  harness launch prompt and scaffold; it is not a deterministic rule router.
- The preflight output is valid NOTA against a fixed schema. The schema carries
  a versioned scaffold identity, minimal source locators or files to mount, a
  session identity, a separate persistent-session request, dedicated
  sandbox/privacy flags, and typed stop conditions. The session identity is
  distinct from the persistent-session
  request/boolean: lane naming, metadata, handle, and lookup are address fields,
  not generic constraints. The stop conditions include idle timeout, turn cap,
  and completion signal variants. The launch packet carries no provider,
  adapter, terminal-driver, concrete model, readiness, or permission-policy
  fields; those belong to adapter/session launch plans below Mentci.
- The scaffold is minimal. It includes `skills/skills.nota` as the expansion
  index and enough local context for the harness agent to start; the agent is
  responsible for loading further skills and repo context from the index rather
  than receiving a broad pre-read bundle.
- Session creation is persistent, named, and addressable. A successful launch
  registers a lane name, lane metadata, an addressable session handle, and a
  lookup path owned by `orchestrate` lanes, following the persistent harness
  session addressing contract above.
- The terminal-cell driver owns liveness for every harness session: process
  handle, send/read loop, idle timeout, close signal, and stalled-output
  detection. Mentci can feed additional input to the named session and read
  later output after the launch request returns.
- Harnesses are pluggable adapters over the same terminal-cell driver. The
  generic contract names harness kind and adapter identity, but acceptance of
  the routing slice cannot depend on provider-specific transcript wording or
  Claude-, Codex-, pi-, or open-ended-harness behavior outside the adapter.
- The first proof runs against a sandboxed jj task. Acceptance requires an
  end-to-end witness that routes prompt input through preflight output, minimal
  scaffold creation, persistent named session launch, at least one feed/read
  exchange, and close or idle handling without touching `/home/li/primary` as a
  jj working copy.
- Failure-mode capture is part of the slice. The witness records, at minimum,
  failures for invalid preflight NOTA, missing required skills, sandbox
  violation, harness process start failure, idle timeout, stalled output, close
  failure, and adapter-level launch/read/write errors.

Deferred from this first slice: rigorous savings metrics, scaffold
reuse/caching mechanics, full adapter parity, concrete model identifier
selection, and the downstream implementation of the preflight engine,
terminal-cell driver, adapters, or proof run.
