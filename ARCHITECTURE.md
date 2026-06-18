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

The internal schemas are imported from the validated Mentci PoC and kept
standalone until the contract remotes exist. The daemon binary is deliberately
not bootstrapped yet: doing so now would require local path dependencies on
unpublished contract crates or duplicating the PoC transport, both of which are
the wrong production foundation.

The next production slice is to create or confirm canonical remotes for
`signal-mentci`, `meta-signal-mentci`, `signal-standard`, and `mentci`, then
wire the daemon crate against generated contract nouns and `mentci-lib`.
