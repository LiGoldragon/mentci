# mentci

Mentci is the daemon for the programmable human approval surface. It keeps the
canonical UI state, lets clients subscribe to projected views, and routes
closed approval verdicts back toward the user's local criome.

This checkout contains the daemon-local Nexus and SEMA schemas plus the first
runtime slice:

- `mentci-daemon` starts from one binary `meta-signal-mentci` `Configure`
  signal frame.
- `mentci` is a thin client that sends one `signal-mentci` request, either as a
  length-prefixed binary frame file or as `signal-mentci`'s own text form, and
  writes the binary reply frame to stdout.
- `mentci` also has one-argument readable atoms over the daemon socket:
  `observe`, `observe:full`, `observe:pending`, `observe:status`,
  `observe:notifications`, `answer:approve:<question>`,
  `answer:reject:<question>`, and `answer:defer:<question>`. Answer atoms send
  `AnswerQuestion` to the mentci daemon; the daemon routes criome-backed
  approvals to criome when configured with `MetaCriome`.

Every dependency is pinned by immutable git rev. No branch pins: a branch pin
resolves to whatever that branch happens to be, which is how this repository's
contracts drifted out from under it.

The daemon's introspect wire runs on the Datom stack. `signal-introspect` 2.0
speaks portable rkyv `Signal` frames of `Query` and `Response`, and what mentci
shows of a reply is that contract's own canonical datom text. Preflight launch
packets are datom values too; `schema/preflight-launch.datom.md` describes the
shape and `mentci::preflight` defines it.

The Mentci wire itself still runs on `signal-frame` envelopes, because
`signal-mentci`, `meta-signal-mentci` and `meta-signal-criome` have not been
rewritten onto the Datom stack. `dotos` is no longer a dependency of this
crate, and this crate requests no `dotos-text` feature; the DOTOS codec still
reaches the build transitively, through `signal-mentci`'s own default feature.

The runtime depends only on canonical remote contract crates. It does not use
local path dependencies for the signal contracts.

Prompt-to-work routing and the generic harness adapter contract are documented
in `ARCHITECTURE.md` under "Possible Future Design - Prompt-To-Bead-Weave
Harness Sessions".
