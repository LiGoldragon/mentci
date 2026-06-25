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
that names the scaffold identity, skills to load, minimal files/context to
mount, session identity, persistent harness session request, model knobs,
sandbox/privacy flags, and typed stop conditions. The preflight is the routing
and prompt-building engine; it is not a deterministic rule router. Thinness is
intentional: the scaffold includes only the minimal support plus
`skills/skills.nota`, and the session agent expands its own context from there.

```nota
;; Target preflight output. Pseudo-NOTA for documentation, not the wire schema.
(MentciPreflight <scaffold> <skills> <session-identity> <persistent-session> <model-selection> <sandbox-privacy> <stop-conditions>)
;;   scaffold          : (Scaffold <identity> <version> <minimal-files>)
;;   skills            : [SkillName]
;;   session-identity  : (SessionIdentity <lane-name> <lane-metadata> <addressable-handle> <lookup-path>)
;;   persistent-session: (PersistentSession <requested> <harness-kind> <adapter> <driver>)
;;   model-selection   : (ModelSelection <preflight-model> <harness-session-model>)
;;   sandbox-privacy   : (SandboxPrivacy <jj-sandboxed> <primary-forbidden> <private-scope-closed>)
;;   stop-conditions   : [(IdleTimeout <duration>) | (TurnCap <turns>) | CompletionSignal]
```

The session is persistent, named, and addressable. `orchestrate` lanes own the
lane name, lane metadata, addressing, and session lookup. The terminal-cell
driver owns process liveness: process handle, send/read loop, idle timeout,
close signal, and stalled-output detection. Harness adapters plug into that one
driver for Claude Code, Codex, pi, and open-ended harnesses.

The first proof domain is a sandboxed jj task. It must not run against primary.
The proof value is the working slice and the failure modes it exposes; no
rigorous savings metric is required for the first pass. Scaffold identities are
versioned in the first schema, while reuse and caching mechanics stay deferred
until the thin slice exists.

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
  a versioned scaffold identity, the skill names to load, minimal source
  locators or files to mount, a session identity, a separate persistent-session
  request, two model knobs, dedicated sandbox/privacy flags, and typed stop
  conditions. The session identity is distinct from the persistent-session
  request/boolean: lane naming, metadata, handle, and lookup are address fields,
  not generic constraints. The stop conditions include idle timeout, turn cap,
  and completion signal variants. The model knobs are semantic slots only: one
  cheap/contained preflight model and one separate cheap harness-session model.
  Concrete provider model identifiers are outside this contract.
- The scaffold is minimal. It includes `skills/skills.nota` as the expansion
  index and enough local context for the harness agent to start; the agent is
  responsible for loading further skills and repo context from the index rather
  than receiving a broad pre-read bundle.
- Session creation is persistent, named, and addressable. A successful launch
  registers a lane name, lane metadata, an addressable session handle, and a
  lookup path owned by `orchestrate` lanes.
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
