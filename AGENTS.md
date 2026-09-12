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

## Stack status

Every dependency is pinned by immutable git rev. Never introduce a branch pin.

On the Datom stack: the introspect wire (`signal-introspect` 2.0 rkyv `Signal`
frames, rendered as canonical datom text) and the preflight launch packet
(`mentci::preflight`, `schema/preflight-launch.datom.md`).

Still on the retired stack, and not this repository's to move: `signal-mentci`,
`meta-signal-mentci`, `meta-signal-criome`, `signal-criome` and `mentci-lib`.
They keep `signal-frame` envelopes and the DOTOS codec. When those contracts
are rewritten, update this consumer — do not add a shim, a feature alias, or a
dual decode path here.
