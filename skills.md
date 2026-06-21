# mentci agent guide

Read, in order, before changing this repository:

1. `INTENT.md`
2. `AGENTS.md`
3. `ARCHITECTURE.md`
4. `/home/li/primary/skills/component-triad.md`
5. `/home/li/primary/skills/rust-discipline.md`
6. `/home/li/primary/skills/kameo.md`

Mentci is the daemon/runtime repo for the human approval surface. The external
contracts live in `signal-mentci` and `meta-signal-mentci`; do not add local
path dependencies to sibling signal repos.

Criome client-approval integration uses criome's parked authorization queue.
List parked requests through criome meta, then approve, reject, or defer by
`AuthorizationRequestSlot`. Do not resubmit `AuthorizationEvaluation` by value.
Thin clients do not open criome sockets; route approvals through the mentci
daemon.
