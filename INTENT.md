# INTENT — mentci

Mentci is the human approval organ for the local per-Unix-user criome. It is
a daemon, because the programmable UI state is daemon-owned state: every TUI,
CLI, editor integration, status bar, popup, and agentic client renders the
same canonical state and submits events back to the daemon.

## Purpose

Mentci presents questions that require the psyche, especially criome
escalations. It keeps pending questions, decisions, subscriptions, and the UI
revision as durable daemon state. Clients do not own approval logic; they
subscribe to projected state and send typed responses.

## Component triad

- `mentci` — this daemon/runtime repository. It owns the daemon, thin CLI, and
  daemon-local Nexus and SEMA schemas.
- `signal-mentci` — the working programmable-UI contract:
  `PresentQuestion`, `PushUpdate`, `ObserveInterfaceState`, `AnswerQuestion`,
  edited-answer proposal admission, and observation retraction.
- `meta-signal-mentci` — the meta policy contract for startup configuration
  and reconfiguration: typed component socket endpoints, persona identity, and
  enabled notification clients.

## Constraints

- Criome is per-Unix-user. Mentci talks to the user's local criome; it is not a
  multi-user shared approval daemon. Socket paths are typed by component and
  authority lane, such as `Mentci` and `MetaCriome`, because Mentci will connect
  to more components than criome alone.
- Criome owns key custody. Mentci presents key-unlock and approval surfaces;
  the real cryptographic signing path waits on the criome key-custody work.
- Verdicts are closed: approve suggested answer, reject, or defer. Editing a
  suggestion creates a new typed proposal object that goes through normal
  criome authorization; the edited answer is not carried inside a verdict.
- criome owns the pending-approval queue. When the local criome runs in
  client approval mode it parks every submission; mentci lists and observes
  criome's parked submissions over the meta socket and approves each by its
  `AuthorizationRequestSlot`, rather than re-supplying the full evaluation
  by value. Per Spirit t00s.
- Thin clients do not talk to criome directly. They submit ordinary
  `signal-mentci` answers to the mentci daemon; the daemon is the sole
  criome-facing approver when configured with a `MetaCriome` socket. A daemon
  without `MetaCriome` still serves ordinary/read-only mentci observations
  and does not submit criome approval verdicts.
- Full interface projections carry the daemon's criome access mode as
  `CriomeAccess`. `ReadWrite` means the daemon has a criome write bridge and
  can route answers to criome; `ReadOnly` means clients observe only.
- The SEMA state is the canonical UI state. A client render exists only after
  a SEMA revision changed and the daemon published a projected state delivery.
- Local UI revision is a plain monotonic counter for the single-machine
  daemon. Attested moments are reserved for a future cross-machine subscriber
  scope.
