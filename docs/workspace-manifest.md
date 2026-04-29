# Workspace manifest

The sema-ecosystem lives as sibling git repos under `~/git/`.
Devshell entry creates symlinks in `repos/`, and the multi-root
`mentci.code-workspace` file exposes the same set to VSCode /
Codium.

For implementation detail, read each repo's `ARCHITECTURE.md`.
For project-wide architecture, read [`criome/ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md).

---

## Status vocabulary

| Status | Meaning |
|---|---|
| **CANON** | Currently canonical; agents freely operate. |
| **TRANSITIONAL** | Canonical today, in a migration; read the linked report. |
| **SHELVED** | Design-valid but post-MVP; not active. |

---

## CANON

| Repo | Role |
|---|---|
| `tools-documentation` | Cross-project rules and tool docs. |
| `criome` | The engine — validator pipeline + sema host. Project-wide architecture lives here. |
| `nota` | Spec — data grammar (nota ⊂ nexus). |
| `nota-codec` | Typed Decoder + Encoder for nota and nexus dialects. Runtime half of the codec stack. |
| `nota-derive` | Proc-macro derives for nota-codec — NotaRecord, NotaEnum, NotaTransparent, NotaTryTransparent, NexusPattern, NexusVerb. |
| `nexus` | The nexus language — grammar spec under `spec/` + translator daemon (text ↔ signal). |
| `signal` | Binary language — wire envelope + IR + record kinds. The workspace's typed wire protocol; spoken on every leg. |
| `signal-forge` | Layered protocol crate atop signal. Carries the criome ↔ forge wire (Build, Deploy, store-entry operations). Skeleton-as-design. |
| `sema` | The records DB (redb-backed). |
| `nexus-cli` | Text client. |
| `forge` | The forge daemon — executor (build, store-write, deploy actors). The bare name doubles as the family namespace. |
| `arca` | Content-addressed filesystem + index DB. **One library + one daemon.** arca-daemon is the privileged writer (write-only staging, multi-store, capability-token-gated). General-purpose; forge is the most active writer of many. |
| `mentci-lib` | Heavy application logic for the mentci interaction surface. Holds workbench state, view snapshots, schema-aware constructor flows, dual-daemon connection management, per-kind canvas renderers, theme/layout interpretation. Consumed by every mentci-* GUI shell. Skeleton-as-design. |
| `mentci-egui` | First incarnation of the mentci interaction surface — thin egui shell atop mentci-lib. Linux + Mac first-class. Skeleton-as-design. |

### CriomOS cluster

| Repo | Role |
|---|---|
| `CriomOS` | NixOS-based host OS for the sema ecosystem. |
| `horizon-rs` | Horizon projection library. |
| `CriomOS-emacs` | Emacs configuration as a CriomOS module. |
| `CriomOS-home` | Home-manager configuration as a CriomOS module. |

## TRANSITIONAL

| Repo | Note |
|---|---|
| `lojix-cli` | Working deploy CLI for CriomOS. Migrates to a thin signal-speaking client of the `forge` daemon when that lands. Don't rewrite. |
| `prism` | Stub today. Records-to-Rust source projector — code-emission subcomponent of `forge-daemon`'s runtime-creation pipeline. Renamed from `rsc` 2026-04-28. |

## SHELVED

| Repo | Reason |
|---|---|
| `arbor` | Prolly-tree versioning over records; post-MVP. |

---

## Update protocol

1. Update this manifest when a repo's status changes.
2. Add or remove the repo's entry in `devshell.nix` `linkedRepos` (drives the symlinks) and `mentci.code-workspace` (drives the editor multi-root view).
3. Write a `reports/NNN-*.md` describing the change.
4. Commit + push.
