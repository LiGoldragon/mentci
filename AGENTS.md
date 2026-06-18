# Agent instructions — mentci

Read `INTENT.md` first, then this file, then `ARCHITECTURE.md`.

## Repo role

`mentci` is the daemon repository for the Mentci component triad. It hosts
the runtime daemon, its thin CLI client, and the daemon-local Nexus and SEMA
schemas. The external contracts live in sibling repositories:
`signal-mentci` for the programmable UI working signal and
`meta-signal-mentci` for daemon configuration.

## Current implementation boundary

This repository currently carries the daemon-local schemas only. Do not add
local path dependencies on `signal-mentci`, `meta-signal-mentci`, or
`signal-standard`; wait for canonical remotes and use normal git
dependencies. The temporary PoC transport in `/tmp/mentci-poc` proved the
shape, but production code should use the shared `mentci-lib` model and the
generated contract nouns.
