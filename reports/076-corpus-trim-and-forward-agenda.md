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
> into/from nexus (nexus role), to criome.

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
| The 10-repo architecture, invariants A-D, project-wide rules, rejected framings, rung-by-rung bootstrap | [architecture.md](../docs/architecture.md) |
| Workspace inventory, last-reviewed date | [docs/workspace-manifest.md](../docs/workspace-manifest.md) |
| Project-wide agent conventions (restate-to-refute rule, report-rollover, ARCHITECTURE.md convention, etc.) | [AGENTS.md](../AGENTS.md) |
| Nexus language semantics — edit verbs, query operators, signal contract | [reports/070](070-nexus-language-and-contract.md) |
| Signal naming decision and three-layer messaging story | [reports/077](077-nexus-and-signal.md) |
| Rkyv discipline — pinned 0.8 portable feature set, derive pattern, encode/decode API | [reports/074](074-portable-rkyv-discipline.md) |
| Lojix transition phases A-G | [`lojix-cli/ARCHITECTURE.md`](https://github.com/LiGoldragon/lojix-cli/blob/main/ARCHITECTURE.md) |
| Edit-UX shape, request-composing shell, four write verbs, diagnostics-as-iteration | [`nexus-cli/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus-cli/blob/main/ARCHITECTURE.md) |
| Pattern-matching theory + delimiter-family matrix + position-defines-meaning | [`nexus/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus/blob/main/ARCHITECTURE.md) |
| Schema-of-schema framing, genesis-via-nexus, validator pipeline | [reports/065](065-criome-schema-design.md) |
| Tree-sitter grammar recommendation for editor integration | [reports/068](068-tree-sitter-grammars-for-nota-and-nexus.md) |
| Per-repo internals (each canonical repo's `ARCHITECTURE.md`) | each repo at root |

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
  local nexus↔criome, with a separate `criome-net` for
  peer-to-peer criome↔criome? Or one envelope for both?

### 3.2 Genesis and bootstrap (criome, blocking Stage A — 067, 064)

- **G1 — Genesis principal mechanism.** Hardcoded
  `bootstrap_principal_id` baked into the criome binary, OR
  first-message-bypasses-permission-check. Reports/067 leans
  hardcoded.

### 3.3 Hacky-stack absorption ordering (post-MVP — 061 §3)

- **A1 — Absorption sequence.** lojix-schema crate first, or
  cluster-config-records first, or parallel?
- **A2 — CriomOS shape.** Absorbed into NixOS or replaced as
  records-native?
- **A3 — Cross-criome primitives.** Which interaction
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

## 5. Trim ledger

### 5.1 Deleted in pass 1 (2026-04-25 morning, 8 reports)

| Report | Reason |
|---|---|
| **017-architecture-refinements.md** | Fully subsumed by [architecture.md](../docs/architecture.md). |
| **059-nix-as-build-backend-and-macro-philosophy.md** | Canonised in architecture.md §1 + §10. |
| **066-architecture-md-audit.md** | Operational audit; work landed. |
| **067-what-to-implement-next.md** | Q-α (15-kind set) supplanted by the 070 query+edit-language framing. G1 (genesis principal) carried into §3.2. |
| **071-cli-protocol-and-implementation-order.md** | Client-msg policies live in [nexus code](../../nexus/src/client_msg/). |
| **072-multi-angle-audit-and-path-forward.md** | Decisions landed; carried supplanted Q-α framing. |
| **073-rkyv-derives-criome-types-and-tests.md** | Bridging audit between Track A landing and Track B/C; both shipped. |
| **075-next-step-multi-angle-with-skeptical-view.md** | Carried supplanted Q-α framing; Tier-1/2/3 reproduced in §4 above. |

### 5.2 Deleted in pass 2 (2026-04-25 afternoon, 8 reports)

| Report | Reason |
|---|---|
| **004-sema-types-for-rust.md** | DAG-via-content-hash shape implicit in [architecture.md §5](../docs/architecture.md); types live in [nexus-schema codebase](../../nexus-schema/src/). |
| **019-lojix-as-pillar.md** | Three-pillar framing absorbed by [architecture.md §1 + §4 + §8](../docs/architecture.md); daemon topology superseded by §4; lojix-family lineage implicit in §8. |
| **033-record-catalogue-and-cascade-consolidated.md** | Type catalogue lives in [nexus-schema codebase](../../nexus-schema/src/); framework duplicates architecture.md §1-§5. |
| **048-change-log-design-research.md** | ChangeLogEntry shape in [architecture.md §5](../docs/architecture.md) (per-kind tables, `(Slot, seq)` keys, rev/op/content-hashes/principal/sig-proof fields). |
| **060-post-mvp-directions.md** | Strategic directions named implicitly in [architecture.md §10](../docs/architecture.md) (machina-chk, BLS-quorum, world-model, sema-as-universal-records). Forward-looking sketches are agent-recoverable from architecture context when the time comes. |
| **061-intent-pattern-and-open-questions.md** | Canonical commitments §1.1-§1.13 are all in [architecture.md §1-§5](../docs/architecture.md); rejected framings §5 are in [§10 "Rejected framings"](../docs/architecture.md); Q-A1-A6 already in §3.3 above. |
| **064-bootstrap-as-iterative-competence.md** | Rung-by-rung philosophy is in [architecture.md §10 "Bootstrap rung by rung"](../docs/architecture.md); Stage A/B detail is implementation-roadmap territory, not architecture. |
| **069-restate-to-refute-rule-and-cleanup.md** | Rule lives in [AGENTS.md](../AGENTS.md) "Report hygiene"; cleanup work is done. |

### 5.3 Deleted in pass 3 (2026-04-25 evening, 4 reports)

Per Li 2026-04-25 *"let's extract and/or delete anything before 65 - let's keep a minimal number of reports at a time"* combined with the new per-repo ARCHITECTURE.md convention (matklad pattern; see [AGENTS.md "Per-repo `ARCHITECTURE.md` at root"](../AGENTS.md)). Each report's substance was extracted into the per-repo ARCHITECTURE.md that owns the concept, then the report was deleted.

| Report | Reason |
|---|---|
| **009-binds-and-patterns.md** | Pattern-matching theory + datalog-style semi-naive eval extracted into [`nexus/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus/blob/main/ARCHITECTURE.md) "Pattern semantics". |
| **013-nexus-syntax-proposal.md** | Delimiter-family matrix + position-defines-meaning + records-as-operators extracted into [`nexus/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus/blob/main/ARCHITECTURE.md) "Three structural choices that compound". |
| **030-lojix-transition-plan.md** | Phases A-G + add-before-subtract rule extracted into [`lojix-cli/ARCHITECTURE.md`](https://github.com/LiGoldragon/lojix-cli/blob/main/ARCHITECTURE.md) "Migration phases". |
| **057-edit-ux-freshly-reconsidered.md** | Request-composing shell + two read surfaces + diagnostics-as-iteration substrate extracted into [`nexus-cli/ARCHITECTURE.md`](https://github.com/LiGoldragon/nexus-cli/blob/main/ARCHITECTURE.md) "Edit UX" + "Diagnostics as iteration substrate". |

### 5.4 Reports that survived all three trim passes (6)

065, 068, 070, 074, 076, 077.

Comfortably under the ~12 soft cap from [AGENTS.md "Report rollover at the soft cap"](../AGENTS.md).

Each survives because:
- **065** schema-of-schema design — too detailed for `architecture.md`; lands when `criome-schema` scaffolds.
- **068** tree-sitter grammars — concrete editor-integration research; not yet acted on.
- **070** nexus language + signal contract — canonical language-design reference.
- **074** rkyv discipline — canonical wire-format reference.
- **076** this report — living entry-point + trim ledger.
- **077** nexus/signal naming — living decision record.

## 6. Conventions for future agents

- Check this report (076) before reading older reports. The trim
  ledger §5 names where to find what.
- When opening a new report, ask: does the insight have a
  canonical home in [architecture.md](../docs/architecture.md),
  a per-repo `ARCHITECTURE.md`, [AGENTS.md](../AGENTS.md), a
  kept reports/ entry, or in code? If yes, edit the canonical
  home; do not write a new report.
- The "restate-to-refute" rule and the report-rollover procedure
  live in [AGENTS.md](../AGENTS.md). State positively; silently
  omit excluded options; lead with the thing being decided. When
  the report count tips above ~12, run a rollover pass.
- The query+edit-language framing (§1 above) is the anchor.
  Recommendations that frame the project as validator-infra-first
  miss the beginning.
