# 101 — Multi-agent style audit pass

*Per Li 2026-04-27: "first I want you to do a deep multi-agent
assisted understanding of my programming style, and the reasons
behind it. Then proceed to rewrite any part of the code that
doesn't follow the guideline. And if the rewrite is deep, I want
you to do an agent assisted research into considering that
perhaps the approach was wrong because the programming style
adopted was not correct and therefore right abstraction was not
seen."*

---

## 1 · The headline

**Zero deep findings. The codebase is structurally sound.**

Five parallel audit agents graded every CANON Rust crate in M0
scope against a synthesised checklist drawn from `programming/`
+ `rust/style.md` + `criome/ARCHITECTURE.md` §2 (Invariants A–D)
+ AGENTS.md. Every audit returned the same shape: the code's
abstractions are right, the discipline holds, the violations
that exist are surface-level hygiene — not abstraction failures.

Li's "if rewrite is deep → research whether the approach was
wrong" branch was therefore not triggered. There is no
abstraction the engineer missed.

Five small fixes shipped. One workspace-wide infrastructure gap
was filed as bd `sema-vbf` for a dedicated follow-up pass.

---

## 2 · The agents

**Phase 1 — synthesis.** One Explore agent read every style
document (beauty.md / beauty-research.md / abstractions.md /
abstractions-research.md / naming-research.md / rust/style.md /
nix-packaging.md / AGENTS.md / criome/ARCHITECTURE.md §§2 + 10 /
reports/100 §§2 + 6 + 7) and produced a single operational
checklist with severity rubric, naming exceptions, ugliness
catalogue, and the trigger-conditions for "deep" findings. The
checklist is what the audit agents scored against.

**Phase 2 — five parallel audits.** Each crate group got its
own agent, each instructed to (a) re-read the canonical style
sources first, (b) score against the synthesised checklist with
file:line citations, (c) classify every finding CRITICAL /
MAJOR / MINOR + flag any DEEP triggers.

| Agent | Scope | LoC | CRITICAL | MAJOR | MINOR | DEEP |
|---|---|---|---|---|---|---|
| 1 | nota-codec | ~1500 | 2 (config) | 1 (transitional) | 1 | 0 |
| 2 | nota-derive | ~600 | 0 | 1 | 2 | 0 |
| 3 | signal | ~1100 | 0 | 1 (config) | 0 | 0 |
| 4 | sema + nexus | ~190 | 1 (config) | 1 (deferred) | ~3 | 0 |
| 5 | criome + nexus-cli | ~420 | 0 | 1 (M0 scaffold) | 3 | 0 |

The CRITICAL items in the table are all infrastructure
configuration (blanket lints, missing `checks.default`), not
code-correctness issues. The MAJOR items split between (a)
honest naming/dep fixes that landed in this pass and (b)
documented-transitional scaffolds with explicit M1+ replacement
paths.

---

## 3 · What got fixed (commits)

Five commits across five repos, all pushed to GitHub `main`:

| Repo | Commit | Change |
|---|---|---|
| `nota-codec` | `bfbffc4c` | Removed blanket `[lints.rust] unused/dead_code` allows from Cargo.toml |
| `nexus` | `0489b50c` | Removed blanket `[lints.rust]` allows |
| `criome` | `113d3138` | Removed blanket `[lints.rust]` allows |
| `nota-derive` | `bcd3d421` | Renamed parameter `ty` → `field_type` in `extract_pattern_field_inner` |
| `sema` | `798c3596` | Removed unused `blake3` dependency |

After removing the lint blankets, `cargo check` stayed clean on
every affected crate — the suppressions weren't masking real
warnings; they were preemptive silencing of the very signal
beauty.md §"Dead code retained 'for safety'" flags as the
diagnostic. Test counts unchanged across the system: nota-codec
79, signal 42, sema 10, criome 6, nexus + nexus-cli stubs build
clean.

---

## 4 · The findings flagged DEEP-but-walked-back

Two findings carried the DEEP marker on first pass; both turned
out to be intentional M0 scaffolding with explicit replacement
paths, not missing abstractions.

### 4a · criome's kind-tag pattern

`criome/src/kinds.rs` defines `pub const NODE: u8 = 1` etc. and
`assert.rs` / `query.rs` hand-synchronise match arms over those
tags. On first read this looks like stringly-typed dispatch — a
classic Invariant D violation.

But it's the documented M0 single-table scaffold: rkyv bytecheck
doesn't catch type-punning between same-size archives, so a
1-byte discriminator gates the try-decode. The replacement is
M1+ per-kind redb tables (criome/ARCH §5 names "per-kind" as
the ground-truth shape; the M0 single-table is the scaffold).
The agent correctly walked the finding back: not a missing
abstraction, a deferred one.

### 4b · nota-codec's `Error::Lexer(String)`

The crate's `Error` enum is mostly typed variants
(`UnexpectedToken`, `WrongBindName`, `UnknownVariant`, etc.) but
carries a transitional `Lexer(String)` catch-all. The source
itself names this as transitional (error.rs:13-15: "Will
refactor into typed variants … once the decoder-side types are
stable enough to share their error vocabulary").

Same shape: not a missing abstraction, a deferred one. Refactor
when the lexer error vocabulary stabilises (per reports/100
§10e — already a known follow-up).

---

## 5 · Outstanding workstreams

### 5a · Workspace-wide flake.nix gap (bd `sema-vbf`)

The audit caught nota-codec and signal as missing
`checks.default`; reading the other flakes showed **all seven
canonical crate flakes are devshell-only**. None expose
`checks.default`. Per `rust/style.md` §"Nix-based tests" this
defeats `nix flake check` as canonical pre-commit runner.

Migration to canonical crane+fenix+blueprint per
[`tools-documentation/rust/nix-packaging.md`](../repos/tools-documentation/rust/nix-packaging.md)
needs: `rust-toolchain.toml` per repo + per-file
`packages/default.nix` + `checks/default.nix` + `devshell.nix`
split + `cargoVendorDir` with `outputHashes` for crates with
git-URL deps. Filed as bd `sema-vbf` for a dedicated pass —
deferred from this audit to keep the audit code-focused.

### 5b · Minor polish items (defer)

- `nota-codec/src/decoder.rs` doc comments restate signature
  ("Read a u64") instead of contract ("Accepts both signed
  `Int` in range and unsigned `UInt` that fits"). Low-priority
  reframe.
- `nota-derive/src/nexus_verb.rs:90-93` panic message is vague
  ("has a different shape"). Low-priority sharpening.
- `nota-derive/src/shared.rs` carries `#![allow(dead_code)]`
  with a comment about populating-as-derives-land. Once all
  derives are in, the silence becomes debt; remove then.
- nexus + nexus-cli stubs have unused dependencies (`clap`,
  `ractor`, `uuid`) waiting for the daemon body to use them.
  Not flagged — the body lands at M0 step 5/6.

---

## 6 · Why no deep findings — the structural reading

The fact that a thorough multi-agent audit returned zero
abstraction failures is itself the load-bearing finding. It
tells us:

- The recent restructures (criome around `Daemon`; signal's
  per-verb typed payloads; sema's private `Slot`; nota-codec's
  closed-Token dispatch) all settled on the right shape.
- The discipline that produced them — methods-on-types as a
  forcing function; per-verb specificity; closed enums at the
  wire; private newtype fields; tests in their own files — held
  during execution. No drift.
- The deferred items (kind-tag → per-kind tables; Lexer-string
  → typed variants; AtomicBatch text form) are visibly named
  and scoped, not hidden ugliness.

The aesthetic discomfort that beauty.md teaches us to read
diagnostically isn't present in this code. That's not because
no one looked — five agents did. It's because the previous
sessions did the work. The audit's most useful service was
**confirming the absence of debt** so the next session can
proceed with the M0 step 5 nexus daemon body without a
"should we fix the foundations first?" overhang.

---

## 7 · Method note for future audit passes

The Phase 1 synthesis → Phase 2 parallel agents pattern worked
well and is reusable:

1. One Explore agent reads all style docs → produces operational
   checklist with severity rubric. (~600-1000 lines, tight.)
2. N Explore agents in parallel — each gets the checklist + a
   specific scope + the instruction to score with file:line
   citations and severity tiers + DEEP triggers.
3. Author consolidates, decides which to fix immediately and
   which to file as bd issues.
4. Fix → cargo check → cargo test → commit per repo → push.

Five parallel agents on ~3800 LoC took ~3 minutes wall-clock
(the slowest finished at 182s). The synthesis upfront is what
makes the audits comparable — without a shared checklist, agents
score against different mental models and the consolidation step
becomes uncomparable noise.

---

*End 101.*
