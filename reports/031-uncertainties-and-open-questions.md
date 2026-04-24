# 031 — uncertainties and open questions after the deep review

*Claude Opus 4.7 / 2026-04-24 · session-close synthesis of what
we still don't know, after three rounds of research + adversarial
critique (reports 022–029) + the lojix transition plan (030).
Organised by load-bearingness, not by topic. The goal: hand Li
a prioritised list of decisions that unblock the most work.*

---

## Priority 0 — Load-bearing for the "sema holds code as logic" thesis

These are the concerns that, if wrong, invalidate the current
framing. They need answers or working hypotheses before much
implementation.

### 0.1 · Hash-refs vs name-refs (the biggest hole)

Report 027 §1 surfaced a direct contradiction: 026 claims
"references are content-hash IDs validated at mutation time",
but the live `nexus-schema` crate stores `Type::Named(TypeName)`
and other string-ref variants. Under the current code, a
reference is a *name* that resolves against the enclosing
module scope — same as Rust text. The content-hash invariant
doesn't hold.

This is not a small thing. The whole "rustc 'unresolved import'
errors can't exist in sema" claim rides on hash-refs. If we
keep string-refs, the ingester and criomed need a name-
resolution pass, which is a substantial subsystem (and opens
the door to exactly the class of errors we claimed to banish).

**Decision needed**: hash-ref only, name-ref only, or dual-mode?

- Hash-ref only means rewriting `nexus-schema` references to
  carry `StructId(Hash)` etc. Every assertion requires the
  referent to exist first — ordering constraint. Upside:
  validation is structural and trivial.
- Name-ref keeps the code as-is but requires a name-resolver
  inside criomed (simpler ingester; heavier criomed).
- Dual-mode: user-authored records carry names; criomed resolves
  them at commit time and stores both. Storage-expensive;
  behaviourally closest to what users expect.

**Lean**: dual-mode is probably what this really is. Be honest
in docs — "references are content-hash IDs *after* criomed
resolves names at commit time." Updating 026's language would
resolve the contradiction without breaking existing code.

### 0.2 · Mutually recursive functions

Two functions `f` and `g` reference each other. Content-hashing
requires no cycles. Unison's answer (cited in 022) is strongly-
connected-component typing — the SCC hashes as a unit; within
the SCC, references are by position. 026 never engages this.

**Decision needed**: do we adopt Unison-style SCC hashing? If
not, how do we hash `f` when `f.body` references `g.hash` and
`g.body` references `f.hash`?

**Lean**: SCC hashing, with the SCC treated as a single "Module
fragment" record. Matches Unison; requires an SCC pass at
mutation time in criomed. Implementation deferral acceptable
for single-function MVP; must be decided before multi-function
workspaces.

### 0.3 · The ingester is not weekend-sized

027 §2 and 029 §1 agree: going from `.rs` text to fully-
resolved semantic records is half of rustc's frontend. Name
resolution alone is non-trivial; macros require a
`macro_rules!` engine; proc-macros require subprocess execution;
external crates (`std`, `serde`) require linking metadata from
somewhere.

Report 026 describes the ingester as a "one-shot bootstrap
tool". 027's reality-check says it's neither one-shot (LLMs
emit text constantly) nor small.

**Decision needed**: What is the ingester's actual scope, and
where does it live?

- MVP-constrained ingester: just parses our *own* workspace,
  skips macros (require `derive(…)` only; error on others),
  treats external crates as opaque `ExternCrate(hash_of_lockfile_entry)`
  references.
- Full ingester: basically rust-analyzer-without-the-IDE.
  Years.
- Use rust-analyzer as the ingester: link `ra_ap_*` crates;
  let r-a do the lowering; translate r-a's HIR to nexus-schema
  records. Possibly the right MVP answer per 029 §7.

**Lean**: link r-a's crates for the ingest path. Accept the
dependency; translate HIR→nexus-schema records at the
boundary. Keep lojixd's rustc-as-derivation separate; the
ingester is an auxiliary tool, not runtime-critical once the
engine self-hosts record edits.

---

## Priority 1 — Load-bearing for MVP self-hosting

These need answers for the self-hosting loop to actually close,
but they don't threaten the whole framing.

### 1.1 · Edit UX: how do humans and LLMs mutate records?

027 §3 presses: a function body is hundreds of records; users
don't type those. What's the actual surface?

- Write nexus syntax that parses 1:1 to record trees — but
  nexus syntax for a non-trivial function body is a wall of
  parens.
- Patch verbs (`(Patch (Fn resolve_pattern) (body :) (Block
  …))`) — needs a path language in nexus grammar; not
  specified in reports/013.
- Edit text via rsc, then re-ingest — round-trips lose comments
  and formatting, degrades the codebase.
- Both: have a `nexus-edit <opus>` mode that rsc-projects to
  text, drops user in `$EDITOR`, re-ingests on save. Comments
  and formatting are lost but the user workflow is familiar.

**Lean**: MVP accepts text-edit-via-rsc-roundtrip as the edit
UX, explicitly lossy (no comments preserved; rsc's formatting
is authoritative). Ship nexus-patch verbs in reports/013's
grammar Phase 2. Document the loss.

### 1.2 · Comments and doc-comments

026 says nothing about them. rsc projecting records to `.rs`
would emit un-commented code — which breaks every Rust
codebase's documentation contract. Possibilities:

- A `Doc { target: RecordId, text: String }` sidecar record.
- A `Doc` field inline on documentable records (`Fn`, `Struct`,
  …).
- Lost; accept that sema's code has no docs and we document in
  an external sema report.

**Lean**: `Doc` as an inline `Option<DocStrId>` field on
documentable records. DocStr is its own record kind holding
markdown text as a blob. Comments are not first-class (the
sema model says comments are lossy); doc-comments are
first-class.

### 1.3 · Cargo.toml, flake.nix, and friends

Report 027 §5 notes these are not obviously records. 017 has
Opus for the Rust-artifact slice of Cargo.toml. Flake.nix is
unaddressed.

**Decision needed**: for each of Cargo.toml (workspace-table,
patch-table, features-table), flake.nix, flake.lock,
rust-toolchain.toml, .gitignore, README, tests, integration
tests, proc-macro crates, build.rs — is it a record kind, an
opaque attachment (blob), or out-of-scope?

**Lean**: MVP treats whole-file fallbacks as `FileAttachment`
(blob) with type-specific parsers added opportunistically.
Opus absorbs the minimal Cargo.toml surface needed for `cargo
check` to succeed. Flake.nix is a `FlakeRef` field on Derivation
(already specced).

### 1.4 · Cascade cost and firewalls

027 §6 asks: if a user renames a field `foo.bar` → `foo.baz`,
every `FieldAccess` record in the workspace updates. Is that
O(workspace)? How do we bound the worst case?

029 §4 confirms the r-a firewall pattern is applicable:
invalidate at the granularity of containing `Fn`, not every
expression. But the hash-cascade is still real — if `Fn.body`
changes hash, every record embedding `FnId(H_old)` is stale.

**Decision needed**: is the cascade "swing the current-state
pointer; leave old records addressable" or "rewrite every
referring record"? First is cheap but asks "current" and
"historical" to be separate concepts; second is clean but
O(workspace) per edit.

**Lean**: swing the pointer. Historical records keep pointing
at the old `Fn`; new reads follow the current-state pointer.
This is how git refs work; we already have `OpusRoot` +
history table per architecture.md §3. Formalise the pattern
for all named-ref tables.

### 1.5 · Rules as records — bootstrap and edit safety

027 §7 asks: if a user retracts the `Module → Fn resolution`
rule, does criomed brick? What prevents this?

024 says genesis rules are hardcoded in criomed's seed. But
after seed, they're ordinary records. The engine is one
`(Retract rule://module-to-fn-resolution)` from losing core
behaviour.

**Decision needed**: are engine-critical rules protected?

- Soft: convention — don't retract rules whose name starts with
  `criomed://`.
- Hard: criomed compiled-in rule-set is the floor; user-added
  rules can layer but not override or retract the floor.
- Verification: criomed on startup checks that the seed rules
  exist in sema; re-asserts them if missing. This is the
  Unison "decompile and verify" pattern.

**Lean**: hard protection. Seed rules are immutable from the
criome-msg surface. Adding user rules is fine; modifying seed
rules requires a criomed recompile (which itself flows through
the self-host loop).

---

## Priority 2 — Load-bearing for ergonomics and post-MVP

These can slip past self-hosting but matter for usability and
the post-MVP semachk subsystem.

### 2.1 · Diagnostic spans — translating rustc errors to records

027 §4 surfaces complexity: rustc diagnostics have primary +
secondary + suggestion + backtrace spans. 026's "rsc emits a
span table" is right in principle but doesn't cover compound
spans.

**Decision needed**: what's the rustc-diagnostic → sema-record
translation strategy?

**Lean**: MVP stores `Diagnostic { rustc_json_payload: Blob }`
as raw JSON. Give the clients tools to parse it. Iterate the
structured translation post-MVP when we see which fields we
actually need to query against.

### 2.2 · semachk's real feasibility

027 §10: semachk (native type/trait/borrow checker in criomed)
is a multi-team-year project. 029 §7 narrows it: semachk is
basically "r-a's back half with content-hash IDs replacing
salsa-interned IDs". Not trivial, but not from scratch.

**Decision needed**: does semachk ever happen? Or is rustc-as-
derivation permanent?

- Option A: rustc-as-derivation forever. Accept the latency
  and parity-trail of rustc nightlies.
- Option B: semachk happens, aimed at the cheap phases first
  (parse + name-res + module graph), rustc keeps body typeck
  and borrow check forever.
- Option C: semachk targets full parity, a long-term project.

**Lean**: B. The cheap phases give real ergonomic wins
(instant "find references", instant "did I break this public
API"). The expensive phases stay with rustc because parity is
impossibly hard and rustc's value is its oracle-status.

### 2.3 · Versioning skew between criomed and sema

027 §9: old sema + new criomed. MVP answer: hard error.
Is that fine, or do we need migrations?

**Lean**: hard error is fine until someone actually hits it.
Document the constraint; fix via manual sema-rebuild in the
rare case.

---

## Priority 3 — lojix transition uncertainties

From report 030 §9, restated for priority.

### 3.1 · Thin CLI's home

Where does the Phase B `lojix-msg`-constructing binary live?
Separate `lojix-cli/` repo, or a binary slot inside `lojix/`
until Phase F, or fold into `nexus-cli` once criomed brokers
deploys?

**Lean**: start as a second binary target inside `lojix/`
(`src/bin/lojix-cli.rs`) during Phase B–C; extract to its own
repo only if the crate gets heavy. Avoids a premature repo
proliferation.

### 3.2 · lojixd transport details

UDS + length-prefixed rkyv is the clear default; no real
contenders. Non-blocking; can be decided at implementation
time.

### 3.3 · Generalising deploy verbs vs keeping them CriomOS-specific

Current lojix is CriomOS-specific. Terminal-state lojixd should
be lojix-generic.

**Lean**: Phase B's `lojix-msg` verbs are generic
(`DeployRun { target_host, flake_ref, action, overrides }`);
the thin CLI supplies CriomOS defaults as a convenience layer.
This matches how nix itself separates flake-agnostic daemons
from convenience wrappers.

### 3.4 · Phase ordering relative to criomed

If criomed is months away, Phase B–C can proceed independently.
If criomed scaffolding starts soon, lojix-msg should know
criomed will be a client.

**Lean**: Phase B (write lojix-msg) anytime; Phase C (scaffold
lojixd listening) waits until Li wants a second daemon process
to manage.

---

## Priority 4 — Doc hygiene and future agents

### 4.1 · Obsolete terminology still in some repos

Report 028 identified issues in nexusd/README (patched),
criome/CLAUDE.md (patched), criome-store/CLAUDE.md (patched),
lojix-archive/CLAUDE.md (patched), lojix/CLAUDE.md (patched
with BEWARE banner). No text-layer contamination in repo docs
themselves — contamination was confined to reports 023/024/025,
now banner-fixed.

### 4.2 · mentci-next reports are getting dense

Reports 001–031 are ~31 documents. Reading order in
`architecture.md §9` is getting long. Future-session new agents
read the top-of-reading-order + canonical and ideally skip the
rest.

**Lean**: keep pruning. Delete reports that are fully
superseded rather than leaving banners; banners are only for
reports with useful decision-journey content. 023/024/025 may
merit deletion once their non-contaminated content is
absorbed into successor reports.

### 4.3 · PRIME.md (bd session-close protocol)

Updated this session to use `jj commit` + `jj bookmark set
main -r @-`. Check that this flows cleanly across multiple
commits in a session. If `jj commit` + bookmark move leaves the
working copy in a state that's annoying for rapid iteration,
revisit.

---

## How these questions interact

```
              0.1 hash vs name refs ─────┐
                    │                    │
                    ▼                    ▼
              0.2 SCC hashing       0.3 ingester scope
                    │                    │
                    └────────┬───────────┘
                             │
                             ▼
                       1.1 edit UX
                       1.4 cascade cost
                       1.5 rule safety
                             │
                             ▼
                       2.1 diagnostic translation
                       2.2 semachk scope
                             │
                             ▼
                       long-term shape
```

Priority-0 decisions cascade. In particular, the hash-vs-name
refs question (0.1) feeds everything downstream — if it's
dual-mode, every other subsystem needs to know.

Lojix-transition questions (3.x) are orthogonal; they block
lojix work but not sema work.

---

## Short list Li might act on first

If Li wants to unblock the most work per decision:

1. **Decide 0.1** — hash vs name vs dual-mode references.
   Updates `nexus-schema`, report 026, architecture.md §1.
2. **Accept or reject 0.3's lean** — use rust-analyzer crates
   for ingest. Opens or forecloses a whole implementation
   strategy.
3. **Decide 1.1** — edit UX model. Affects nexus grammar
   Phase 2 plans and how users/LLMs interact with sema day-to-
   day.
4. **Kick off lojix Phase B** — write `lojix-msg` crate from
   existing lojix in-process types. Minimal risk, unblocks
   Phase C whenever it's needed.

Everything else can wait on these.

---

*End report 031.*
