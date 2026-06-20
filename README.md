# mentci

Mentci is the daemon for the programmable human approval surface. It keeps the
canonical UI state, lets clients subscribe to projected views, and routes
closed approval verdicts back toward the user's local criome.

This checkout contains the daemon-local Nexus and SEMA schemas plus the first
runtime slice:

- `mentci-daemon` starts from one binary `meta-signal-mentci` `Configure`
  signal frame.
- `mentci` is a thin client that sends one `signal-mentci` request, either as a
  length-prefixed binary frame file or as NOTA text, and writes the binary reply
  frame to stdout.
- `mentci` also has one-argument criome approval atoms:
  `criome:parked`, `criome:approve:<slot>`, `criome:reject:<slot>`, and
  `criome:defer:<slot>`. They use `MENTCI_CRIOME_META_SOCKET` or
  `/tmp/criome-meta.socket`.

The runtime depends only on canonical remote contract crates. It does not use
local path dependencies for the signal contracts.
