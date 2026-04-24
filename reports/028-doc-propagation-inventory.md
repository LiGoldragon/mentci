# 028 · Documentation propagation inventory — sema-ecosystem repos

*Claude Opus 4.7 / 2026-04-24 · Audit of all user-visible docs across sema-ecosystem repos against canonical architecture (docs/architecture.md + reports/026)*

---

## Summary

Inventory of 39 documentation files across 13 core sema-ecosystem repos. Most files are clean and aligned. Found:

- **3 high-priority issues**: nexusd README mislabels its role; lojix/README and CLAUDE mismatch (repo repurposed 2026-04-24, archive not yet documented); old lojix-archive orphaned without context.
- **1 medium-priority issue**: criome/CLAUDE still refers to "criome-store" in the stack diagram (should clarify post-020 fold of forged+stored into lojixd).
- **Neutral observations**: no repos missing critical docs; nota/nexus layer clearly documented; no contamination from "text-layer" errors (SourceRecord/TokenStream/Ast) found in user-visible docs.

---

## Detailed findings by repo

### `nota/` — ALIGNED

**Files**:
- `README.md` (spec-only, 298 lines)

**Status**: Clean. Spec is canonical and accurate. No architecture-adjacent claims. No changes needed.

---

### `nota-serde/`, `nota-serde-core/`  — ALIGNED

**Files**:
- `nota-serde-core/README.md` (30 lines)
- `nota-serde/README.md` (20 lines)

**Status**: Both clean. Correctly positioned as serde implementations. No architecture references. No changes needed.

---

### `nexus/` — ALIGNED

**Files**:
- `README.md` (spec-only, 258 lines)

**Status**: Spec is canonical. Correctly frames nexus as messaging protocol superset of nota. No architecture-level claims. No changes needed.

---

### `nexus-serde/` — ALIGNED

**Files**:
- `README.md` (47 lines)

**Status**: Correctly documents serde layer. No architecture-level claims. No changes needed.

---

### `nexus-schema/` — ALIGNED

**Files**:
- `README.md` (25 lines)

**Status**: Clean. Correctly identifies repo purpose: "Rust types that shape sema database records." Mentions rkyv and serde derives. No stale claims. No changes needed.

---

### `nexus-cli/` — ALIGNED

**Files**:
- `README.md` (8 lines)

**Status**: Minimal, correct. Identifies as "Thin CLI client for nexusd daemon." No changes needed.

---

### `nexusd/` — HIGH PRIORITY (misnamed role)

**Files**:
- `README.md` (14 lines)

**Current text**:
```
The sema database daemon. Receives nexus messages, applies edits to
the database, serves queries.
```

**Issue**: Contradicts canonical architecture. Per docs/architecture.md § 2:
- nexusd is **the translator**: "text ↔ rkyv only; validates syntax + protocol version; forwards to criomed; serialises replies."
- criomed is **sema's engine**: "maintains the single truth... receives every message, applies mutations..."

**What it should say**: 
```
The messenger daemon — nexus text ↔ rkyv translator at the human boundary. 
Receives nexus syntax from clients (nexus-cli, LLMs, etc.), parses to 
rkyv-encoded CriomeRequest, forwards to criomed, serializes CriomeReply 
back to nexus text. Stateless modulo in-flight correlations. 
(Cf. docs/architecture.md § 2.)
```

**Priority**: HIGH — directly contradicts canonical architecture.

---

### `criome/` — MEDIUM PRIORITY (architectural terminology)

**Files**:
- `CLAUDE.md` (40 lines)
- `readme.md` (6 lines, stub only)

**Current text (CLAUDE.md)**:
```
The Stack

criome          runtime — hosts sema worlds, provides identity
criome-store    persistence — content-addressed bytes (blake3 → bytes)
arbor           versioning — prolly trees over the store
nexus           protocol — how agents talk to sema worlds
aski            language — how sema types are specified
sema            the format — the universal typed binary that everything is
```

**Issue**: References "criome-store" in the architecture stack, but per architecture.md § 3 and report 020, the actual store is:
- **sema** (redb, owner: criomed) — structural records
- **lojix-store** (append-only blobs, owner: lojixd) — opaque bytes

The term "criome-store" is architecturally orphaned. The repo exists (criome-store/) and implements a generic typed store (pre-MVP), but it's not in the canonical daemon-layer stack anymore. The confusion arises from report 020's folding of "forged" and "stored" daemons into "lojixd" — criome-store was conceptually the "stored" layer, now subsumed into lojixd's internal blob management.

**What it should say** (clearer framing):
```
The Stack (runtime daemons + persistence)

nexusd          messenger — text ↔ rkyv translator
criomed         engine — sema world maintainer; applies mutations, cascades
lojixd          executor — performs effects; stores blobs in lojix-store
arbor           (library, not daemon) — prolly trees for versioning
nexus           (protocol) — human-facing syntax; criome internal wire is rkyv
sema            (store, not daemon) — criomed's record database (redb-backed)
lojix-store     (store, not daemon) — lojixd's blob storage (append-only)
```

**Alternative**: Strike the diagram and reframe CLAUDE.md as a conceptual vision doc (which it already is) rather than pretending to name the daemon stack. The daemon stack lives in docs/architecture.md § 2 and should not be duplicated in repo CLAUDEs.

**Priority**: MEDIUM — doesn't directly contradict (no false claims about what daemons do), but uses orphaned terminology and conflates libraries, protocols, and stores with daemons in the stack diagram.

---

### `criome-store/` — MEDIUM PRIORITY (orphaned repo without context)

**Files**:
- `CLAUDE.md` (51 lines)

**Current text**:
```
The universal content-addressed store for the sema ecosystem. Every
object — strings, sema objects, arbor tree nodes, manifests, commits —
lives here, sorted by kind, addressed by blake3 hash.
```

**Issue**: The CLAUDE.md describes a generic typed store (`Store` trait with `kind` bytes, `ChunkStore` trait). This was pre-MVP design meant to unify sema, arbor, and artifacts under one typed store. However:

1. Per architecture.md § 3 and report 020, the actual MVP has **two separate stores**:
   - **sema** (redb, records only, owned by criomed)
   - **lojix-store** (append-only blobs, owned by lojixd)

2. criome-store as described here (universal, kind-tagged, both sema and artifacts) is not in the MVP architecture.

3. The repo exists and is referenced by arbor's `ChunkStore` trait, but its role is currently **a library used by arbor for testing**, not the universal store.

**What the CLAUDE.md should clarify**:
- Current status: pre-MVP design artifact; arbor uses its `ChunkStore` trait for testing.
- MVP reality: sema (redb) and lojix-store (append-only blobs) are separate; no unified kind-tagged store yet.
- Future: criome-store *may* evolve into a unified layer post-MVP, but that's not in scope now.

**Suggested addition to CLAUDE.md**:
```
## MVP Status

This crate implements a generic typed content-addressed store (pre-MVP design).
The actual MVP uses two separate stores:
- sema (redb-backed, owned by criomed) — structured records
- lojix-store (append-only blobs, owned by lojixd) — opaque bytes

criome-store is currently used as the ChunkStore backend for arbor testing
but is not the universal store in the MVP architecture. Post-MVP unification 
is an open question.
```

**Priority**: MEDIUM — misleading about MVP architecture, but not aggressively wrong (the repo does what it says for the scope it currently serves).

---

### `lojix/` — HIGH PRIORITY (repo repurposed, old docs not cleaned)

**Files**:
- `README.md` (25 lines)
- `CLAUDE.md` (70 lines template)
- `AGENTS.md` (105 lines)

**Current state**: The repo was repurposed on 2026-04-24. Old `lojix-archive/` holds the prior "build dialect" concept; current `lojix/` is a CriomOS deploy orchestrator.

**Issue 1 — README claims are correct for NEW purpose but CLAUDE.md is a stale template**:

README.md correctly describes the new lojix:
```
CriomOS deploy orchestrator. Projects a cluster proposal nota
through `horizon-rs` in-process, writes a content-addressed horizon
flake, and invokes `nixos-rebuild` against CriomOS...
```

CLAUDE.md is **a generic template** with boilerplate:
```
## Architecture Overview

_Add a brief overview of your project architecture_

## Conventions & Patterns

_Add your project-specific conventions here_
```

This is a **critical gap**: someone reading CLAUDE.md for lojix's actual design gets nothing. The real design is documented at `/home/li/git/CriomOS/reports/2026-04-24-ractor-tool-design.md` (external ref), but lojix/CLAUDE.md doesn't say this.

**What CLAUDE.md should say**:
```
# lojix — CriomOS deploy orchestrator

## Architecture

Design document: `/home/li/git/CriomOS/reports/2026-04-24-ractor-tool-design.md`
(external to this repo; tracks implementation via beads: `bd list --status open`).

The pipeline:
```
DeployCoordinator (supervisor; OneForOne)
  ├── ProposalReader     reads + caches the source nota
  ├── HorizonProjector   horizon-lib in-process; NOT subprocess
  ├── HorizonArtifact    writes flake.nix + horizon.json; computes narHash; tars; optionally uploads
  └── NixBuilder         spawns nix; streams stdout/stderr
```

## Repo context

On 2026-04-24, this slot was repurposed from "lojix: the build dialect" 
(held in lojix-archive/) to "lojix: deploy orchestrator for CriomOS."
The prior design was an aski language family member (like nexus/synth);
the current direction is a ractor-based orchestrator tool.
```

**Issue 2 — AGENTS.md correctly describes the new purpose**, but mentions beads workflow and hard process rules (jj, mentci three-tuple, etc.) that are CriomOS-specific, not necessarily lojix-specific. Minor but could confuse someone.

**Priority**: HIGH — CLAUDE.md is unusable as-is; README is correct but CLAUDE.md template is stale and unhelpful.

---

### `lojix-archive/` — MEDIUM PRIORITY (orphaned without clear status note)

**Files**:
- `CLAUDE.md` (44 lines)

**Current state**: Holds the prior lojix design (aski dialect for builds). Correctly documented but lacks a clear "THIS IS ARCHIVED" banner at the top.

**Current text** (CLAUDE.md):
```
# lojix — The Build Dialect

Lojix is an aski dialect purpose-built for building things.
It replaces what Cargo and Nix do today — dependency
management, build orchestration, artifact production.
...
## Status

Not yet implemented. This repo holds the future design.
```

**Issue**: "future design" is stale (it's now past). No clear note that this was superseded on 2026-04-24 by the deploy orchestrator in `/home/li/git/lojix/`. Someone cloning this might not realize the slot was repurposed.

**What should be added (top of CLAUDE.md)**:
```
> **ARCHIVED 2026-04-24**: This design (lojix as aski build dialect) was shelved.
> The `lojix/` repo slot was repurposed for a CriomOS deploy orchestrator.
> This directory preserves the prior design for historical reference.
```

**Priority**: MEDIUM — won't cause architectural confusion, but metadata is stale.

---

### `sema/` — ALIGNED

**Files**:
- `README.md` (24 lines)

**Current text**:
```
The sema database — content-addressed record storage for typed
program structure. Pseudo-sema while the system bootstraps: records
are rkyv-archived Rust values from nexus-schema, stored in redb,
addressed by their blake3 hash.
```

**Status**: Correct and well-scoped. Doesn't claim to be a daemon; correctly identifies it as a store owned by criomed. Includes forward reference to Vision.md. No changes needed.

---

### `arbor/` — ALIGNED

**Files**:
- `CLAUDE.md` (55 lines)

**Current text** (relevant excerpt):
```
## Dependency Direction

arbor depends on nothing. criome-store depends on arbor (implements
ChunkStore, uses Tree for per-kind sorted storage).
```

**Status**: Correctly describes arbor as the versioning layer. The mention of criome-store is factually accurate (criome-store does implement ChunkStore). No changes needed given the understanding that criome-store is a library, not the MVP's primary store.

---

### `mentci-next/` — ALIGNED

**Files**:
- `CLAUDE.md` (1 line: redirects to AGENTS.md)
- `AGENTS.md` (24 lines)

**Status**: Correctly frames mentci-next as the project root. AGENTS.md correctly points to tools-documentation for cross-project rules and references reports in reading order. No changes needed.

---

## Aligned, no changes needed

The following 24+ files are clean, accurate, and require no updates:

**Specs (correct by definition)**:
- nota/README.md
- nexus/README.md

**Serde implementations (correctly scoped)**:
- nota-serde-core/README.md
- nota-serde/README.md
- nexus-serde/README.md

**Schema and data layer**:
- nexus-schema/README.md
- sema/README.md

**Thin client**:
- nexus-cli/README.md

**Versioning and conceptual layers**:
- arbor/CLAUDE.md
- mentci-next/CLAUDE.md
- mentci-next/AGENTS.md

---

## Summary of changes needed

| Repo | File | Priority | Action |
|---|---|---|---|
| `nexusd` | README.md | HIGH | Retitle nexusd's role from "sema database daemon" to "translator daemon"; clarify that criomed is the engine |
| `lojix` | CLAUDE.md | HIGH | Replace template boilerplate with actual design summary; reference CriomOS design doc and beads tracking |
| `criome` | CLAUDE.md | MEDIUM | Either strike the "Stack" diagram (it conflates daemons, stores, protocols) or reframe to clarify post-020 architecture (sema + lojix-store, not criome-store) |
| `criome-store` | CLAUDE.md | MEDIUM | Add "MVP Status" section clarifying that this is a pre-MVP library, not the universal store; sema + lojix-store are separate in MVP |
| `lojix-archive` | CLAUDE.md | MEDIUM | Add archival banner at top noting 2026-04-24 supersession by lojix/ deploy orchestrator |

---

*Report 028 complete.*

*Canonical references: docs/architecture.md (updated 2026-04-24), reports/026 (2026-04-24), reports/020 (lojix single daemon), reports/019 (lojix as pillar).*

