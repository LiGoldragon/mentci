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
mount, model knobs, and the persistent harness session request. The preflight
is the routing and prompt-building engine; it is not a deterministic rule
router. Thinness is intentional: the scaffold includes only the minimal support
plus `skills/skills.nota`, and the session agent expands its own context from
there.

```nota
;; Target preflight output. Pseudo-NOTA for documentation, not the wire schema.
(MentciPreflight <scaffold> <skills> <session> <model-selection> <constraints>)
;;   scaffold        : (Scaffold <identity> <version> <minimal-files>)
;;   skills          : [SkillName]
;;   session         : (HarnessSession <lane-name> <harness-kind> <adapter> <driver>)
;;   model-selection : (ModelSelection <preflight-model> <harness-session-model>)
;;   constraints     : [ConstraintText]
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
