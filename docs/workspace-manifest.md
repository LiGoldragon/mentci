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
| `signal` | Binary language — wire envelope + IR + record kinds. |
| `sema` | The records DB (redb-backed). |
| `nexus-cli` | Text client. |
| `lojix` | The lojix daemon — forge + store + deploy actors. The bare name doubles as the family namespace. |
| `lojix-schema` | criome ↔ lojix contract types. |
| `lojix-store` | Content-addressed filesystem + index DB. |

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
| `lojix-cli` | Working deploy CLI. To become a thin transport for `lojix-schema` requests once `lojix` lands. Don't rewrite. |
| `rsc` | Stub today. Will project records to Rust source when criome can supply them. |

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
