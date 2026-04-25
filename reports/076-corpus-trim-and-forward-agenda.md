---
title: 076 — corpus trim and forward agenda (post-trim consolidation)
date: 2026-04-25
status: living-document; supersedes 067 / 072 / 073 / 075 (deleted in this pass)
anchor: reports/070 §1 (Li 2026-04-25 first-functionality framing)
feeds: reports/070 §1, §2, §3, §6, §8; architecture.md §2 + §5 + §10
---

# 076 — corpus trim and forward agenda

This report is the **living entry-point to reports/** after a heavy
trim. New agents should read this first, then drill into the kept
reports below.

## 1. Anchor — what the engine is trying to be

The first functionality of nexus-criome is the **query+edit
language for the database**. Per [reports/070 §1](070-nexus-language-and-contract.md#L7),
quoting Li 2026-04-25:

> first we need the logic that will make nexus the greatest
> database edit-and-query language ever made … and we need the
> contract that will create this logic in rkyv messages
> into/from nexus (nexusd role), to criomed.

Everything that follows — the signal contract, the validator
pipeline, the schema-of-schema, the slot/index machinery — exists
to **carry the language**. Schema falls out of language
requirements, not the other way around. Reframings that ask "what
records do we need?" before "what must the language express?"
miss the beginning.

## 2. Live foundations (canonical homes)

These are stable. Cite from the canonical home, not from the
research reports that produced them.

| Concept | Canonical home |
|---|---|
| The 10-repo architecture, invariants A-D, project-wide rules | [architecture.md](../docs/architecture.md) |
| Workspace inventory, last-reviewed date | [docs/workspace-manifest.md](../docs/workspace-manifest.md) |
| Nexus language semantics — edit verbs, query operators, signal contract | [reports/070](070-nexus-language-and-contract.md) |
| Rkyv discipline — pinned 0.8 portable feature set, derive pattern, encode/decode API | [reports/074](074-portable-rkyv-discipline.md) |
| Lojix transition phases A-G, day-one skeleton | [reports/030](030-lojix-transition-plan.md) |
| Restate-to-refute corpus rule | [reports/069](069-restate-to-refute-rule-and-cleanup.md) |
| Edit-UX shape, request-composing shell, four write verbs | [reports/057](057-edit-ux-freshly-reconsidered.md) |
| Rung-by-rung bootstrap stages, Stage A and B near-term plan | [reports/064](064-bootstrap-as-iterative-competence.md) |
| Schema-of-schema framing, genesis-via-nexus, validator pipeline | [reports/065](065-criome-schema-design.md) |
| Pattern-matching theory, datalog-style semi-naive eval | [reports/009](009-binds-and-patterns.md) |
| Delimiter-family matrix, position-defines-meaning | [reports/013](013-nexus-syntax-proposal.md) |
| Tree-sitter grammar recommendation for editor integration | [reports/068](068-tree-sitter-grammars-for-nota-and-nexus.md) |
| Post-MVP directions (machina-chk, BLS-quorum, world-model, etc.) | [reports/060](060-post-mvp-directions.md) |
| Lojix as the third pillar, two-axis layering, nix as build backend | [reports/019](019-lojix-as-pillar.md) |

## 3. Live open questions

These are organised by the layer they belong to. None of them is a
question about a pre-specified kind taxonomy.

### 3.1 Language design (highest priority — 070 §8)

- **L1 — Subscription delivery semantics.** At-least-once
  vs at-most-once vs exactly-once. Default candidate: ALO with
  explicit ack + dedup by `(subscription_id, event_rev)`.
- **L2 — Intra-transaction forward-refs.** Within a TxnBatch,
  may an Assert reference a slot bound by a later Assert in the
  same batch? Two-pass resolution vs strict order.
- **L3 — Cascade non-termination.** When a cascading rule fires
  beyond a cycle bound, do we emit `E9999` and keep the
  originating mutation, or reject the whole batch?
- **L4 — Cross-instance scope.** Does signal cover only
  local nexusd↔criomed, with a separate `criome-net` for
  peer-to-peer criomed↔criomed? Or one envelope for both?

### 3.2 Genesis and bootstrap (criomed, blocking Stage A — 067, 064)

- **G1 — Genesis principal mechanism.** Hardcoded
  `bootstrap_principal_id` baked into the criomed binary, OR
  first-message-bypasses-permission-check. Reports/067 leans
  hardcoded.

### 3.3 Hacky-stack absorption ordering (post-MVP — 061 §3)

- **A1 — Absorption sequence.** lojix-msg crate first, or
  cluster-config-records first, or parallel?
- **A2 — CriomOS shape.** Absorbed into NixOS or replaced as
  records-native?
- **A3 — Cross-criomed primitives.** Which interaction
  primitives are first-class from day one?
- **A4 — Machina-check priority.** Value order or difficulty
  order across the seven phases?
- **A5 — World-fact category timing.** Phase 2 or defer? Needs
  stratification in MVP or retrofittable?
- **A6 — BLS quorum.** Genesis-first or deferred?

### 3.4 Schema details (resolved by language design, not in advance)

The minimum kind set emerges from what messages need to express.
Schema-of-schema (`KindDecl`, `FieldSpec`, `TypeRef`,
`VariantDecl`, `CategoryDecl`) plus slot/index machinery (`Slot`,
`Revision`, `SlotBinding`, per-kind `ChangeLogEntry`) plus the
literal/value layer are the floor — see [065 §3](065-criome-schema-design.md). Beyond that,
record families are added as the language reveals it needs them.
**No pre-listed taxonomy is the answer to "what kinds at v0.0.1?"
The answer is "whatever the v0.0.1 language demands, and no more."**

## 4. The next step

Two paths, both supported by the corpus, and they are *parallel*
not sequential:

### 4.1 Tier 1 — autonomous (~80 LoC, ~30 min)

1. Fix stale dead-report citations in
   [057](057-edit-ux-freshly-reconsidered.md#L6),
   [064](064-bootstrap-as-iterative-competence.md#L87),
   [065](065-criome-schema-design.md#L59) — rewrite to the live
   home of the lesson or drop.
2. Add canonical rkyv dep to
   [lojix-store/Cargo.toml](../../lojix-store/Cargo.toml) and
   rkyv derives to lojix-store index types named in [074 §1](074-portable-rkyv-discipline.md). Bodies stay `todo!()`.

### 4.2 Tier 2 — language-first scaffolding (signal)

The contract spec is in [070 §6](070-nexus-language-and-contract.md). Scaffolding signal
*following 070's design* is buildable now: types, rkyv derives,
encode/decode bodies, round-trip tests. The kind set referenced
by signal (envelope and verb-payload types) is small and
already named in 070 §6.6 (RawRecord, RawValue, Op, …). The
broader schema (KindDecl, FieldSpec, etc.) does not need to be
specified before signal can land.

### 4.3 What does *not* belong before this

- A pre-listed v0.0.1 kind taxonomy. Schema follows language.
- Speculative repo creation for criome-types ahead of signal.
  The newtype layer's contents are revealed by what signal
  imports.
- Further multi-angle synthesis reports. Two were written in two
  days; both are deleted in this pass.

## 5. Trim ledger (this pass)

### 5.1 Deleted (8 reports)

| Report | Reason |
|---|---|
| **017-architecture-refinements.md** | Fully subsumed by [architecture.md](../docs/architecture.md). |
| **059-nix-as-build-backend-and-macro-philosophy.md** | Canonised in architecture.md §1 + §10. |
| **066-architecture-md-audit.md** | Operational audit; work landed. |
| **067-what-to-implement-next.md** | Q-α (15-kind set) supplanted by the 070 query+edit-language framing. §1 framing carried into this report; §3 design carried into [070](070-nexus-language-and-contract.md). G1 (genesis principal) carried into §3.2 above. |
| **071-cli-protocol-and-implementation-order.md** | Client-msg policies live in [nexusd code](../../nexusd/src/client_msg/) now. The implementation-order sketch was a one-shot plan for that work, which has shipped. |
| **072-multi-angle-audit-and-path-forward.md** | Decision-point report; decisions landed. Carried the now-supplanted Q-α framing. |
| **073-rkyv-derives-criome-types-and-tests.md** | Bridging audit between Track A landing and Track B/C; both shipped (B in 074, C deferred). |
| **075-next-step-multi-angle-with-skeptical-view.md** | Carried supplanted Q-α framing. The Tier 1 / Tier 2 / Tier 3 distillation is reproduced in §4 above without the dismissed framing. |

### 5.2 Flagged for light trim (next pass)

| Report | Trim action |
|---|---|
| 004-sema-types-for-rust.md | Keep coverage statement (§3) + shape/identity pattern (§2). Drop M2 task detail. |
| 019-lojix-as-pillar.md | Keep §2 thesis + §8 guardrails. Drop daemon table (superseded). |
| 033-record-catalogue-and-cascade-consolidated.md | Keep Part 2 catalogue. Drop Part 1 (duplicates architecture.md). |
| 048-change-log-design-research.md | Keep Part 3 ChangeLogEntry shape + redb layout. Drop Parts 1-2 (philosophy). |

### 5.3 Kept as-is (12 reports)

009, 013, 030, 057, 060, 061, 064, 065, 068, 069, 070, 074.

Each is the canonical home for an insight not duplicated elsewhere.

## 6. Conventions for future agents

- Check this report (076) before reading older reports. The trim
  ledger §5 names where to find what.
- When opening a new report, ask: does the insight have a
  canonical home in architecture.md or a kept reports/ entry?
  If yes, edit the canonical home; do not write a new report.
- The "restate to refute" rule from [069](069-restate-to-refute-rule-and-cleanup.md) applies. State
  positively; silently omit excluded options; lead with the
  thing being decided.
- The query+edit-language framing (§1 above) is the anchor.
  Recommendations that frame the project as validator-infra-first
  miss the beginning.
