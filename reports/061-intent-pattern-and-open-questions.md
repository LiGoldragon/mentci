# 061 — intent-pattern & open questions

*Claude Opus 4.7 · multi-agent deep-research pass on the
nexus/criome/sema engine. Reads the canonical corpus
(architecture.md + 12 surviving reports + MEMORY.md + auto-
memory), sibling-repo skeletons, git history, bd memories,
the CriomOS cluster, and horizon-rs internals. Synthesises
what is certain, flags what is genuinely open, and ends in a
prioritised question bank.*

*Seven framings were stripped from earlier drafts after Li
corrected them: (1) importing aski design vocabulary — aski
is retired, not a design input; (2) "personal-scale" posture
— CriomOS aims to supersede proprietary OSs globally; (3)
"federated global sema" — all sema are local; (4) code-vs-
world-facts as resolvable boundary — it is intrinsic
category; (5) "self-hosting close needs bit-for-bit
identity" — obviously not, any more than new rustc versions
must be byte-identical to their predecessors; (6) multi-
criome as closed-vs-federated — each sema holds a subjective
view; they communicate, agree on most things, disagree on
some, and negotiate; (7) "legibility-to-agents as stated
axis" — nexus is the agent interface; the question was
confused. §5 records each contamination and the guardrails
added to stop recurrence.*

---

## 1 · The certain pattern

### 1.1 · Sema is gravitational, not compositional

Sema is not a component among peers; it is the centre that
everything else orbits. signal, nexus grammar, rsc
emission, lojix-store handles, capability tokens, rules,
cascades — each exists because sema exists. The move from
"sema is a data store" to "sema is the evaluation itself"
(commit `e9be6c2`) is the clearest signal: sema is not
storage, it is the state of the world at any instant, and
criomed is its engine.

### 1.2 · Content-addressing is the spine

blake3 over canonical rkyv is the identity of everything
that matters. Slot-refs add indirection for renames; content-
hash is never malleable. Reads are mmap-friendly zero-copy;
writes go through one gate. rkyv, redb, blake3 all converge
on one bet: canonical binary with identity-as-hash makes
caching, reproducibility, GC, and cross-instance record-
sharing by hash fall out for free.

### 1.3 · Rust is only an output

Sema holds records. rsc projects them to `.rs`. No reverse
path, ever. No ingester. Every line of Rust this project
canonically depends on must be authored as records via nexus
requests. Hand-written `.rs` during bootstrap is scaffolding.

### 1.4 · Single-writer criomed is the invariant gate

Validation happens at one place. Everything crossing into
sema is content-hashed, schema-checked, reference-resolved,
permission-stamped. No eventually-consistent path, no
validate-later workflow. The hallucination wall lives at
criomed's write path.

### 1.5 · Two stores, three daemons, one language

sema (redb-backed records; criomed owns writes) and lojix-
store (content-addressed filesystem; lojixd owns writes) are
the canonical stores per instance. nexusd, criomed, lojixd
are the three daemons. nexus — over its nota base syntax —
is the one language.

### 1.6 · Nix is the bootstrap runtime; lojix is the destination

crane + fenix is the build backend today. lojix-store is
canonical from day one of the MVP; real implementation lands
after the skeleton. Eventually lojix absorbs what nix does
with the 11 concrete nix problems fixed. "An expanded and
more correct nix."

### 1.7 · We author no macros; we freely call third-party ones

Code-gen patterns live as sema rules, produce impl records,
rsc emits them. No `macro_rules!` or proc-macro crates
authored by us. But `#[derive(...)]`, `#[tokio::main]`, etc.
remain ecosystem calls — rsc emits them verbatim. *Author
no macros; call any macro.*

### 1.8 · Skeleton-as-design over prose-as-design

Types and traits carry the design; `cargo check` enforces
consistency. Reports say *why*; skeleton code says *what*.
lojix-store/src/ is the reference exemplar.

### 1.9 · CriomOS aims to supersede proprietary OSs globally

World-scale ambition pursued at personal cadence. The
architecture backs this: NixOS substrate (reproducible,
cluster-capable), content-addressing as primary identity
(industrial primitive, not laptop convenience), cluster-
first network-neutral topology, lojix replacing
infrastructure. Framings like "personal-scale," "craftsperson
workshop," "self-hosted-self" underestimate the scope.

### 1.10 · Sema is local; reality is subjective

Every sema instance is one machine's view of reality. There
is no global sema, no federated global database, no single
logical truth. Per Li 2026-04-25: reality is intrinsically
subjective — every system ever designed holds a view, and
every system that will ever be designed will hold a view.
Different criomed instances communicate richly: agree on
most things, disagree on some, communicate more to
negotiate agreement. The interaction degree will exceed
current-web scale by a wide margin. Content-addressing
makes this tractable without requiring a single logical
store.

### 1.11 · Category structure is intrinsic; machina is the code category

Records split into categories (code records, world-fact
records, operational records, authz records) and those
splits reflect ontological reality, not schema convenience.
Per Li 2026-04-25: code that runs the engine can never be
in the same category as "how many eggs are in Li's fridge"
— the separation is an intrinsic fact of reality. Sema's
`CategoryDecl` + `KindDecl` machinery encodes that reality.

Names for the categories matter because operations are
category-scoped. Li 2026-04-25 named the code category
**machina** — the subset of sema that compiles to Rust in
v1. "Machina" is what the Rust type-checker, the native
checker (§3.5), and rsc all operate over. World-fact
records (eggs, observations, arbitrary knowledge) are
outside machina and cannot affect runtime type-checking. A
name like "semachk" is wrong because the check is not over
all of sema; it is over machina. Names for the other
categories (world-facts, operational, authz) are open.

### 1.12 · Self-hosting close is normal software engineering

Self-hosting close means the engine works correctly and its
canonical crates are authored as records. Bit-for-bit
identity with the bootstrap version is not a requirement —
new rustc versions are not byte-identical to their
predecessors, and nexus-criome-lojix self-hosting is
analogous. Per Li 2026-04-25: of course a working result
without bit-for-bit identity is acceptable.

### 1.13 · Nexus is the interface for agents to interact with criome

Agents (LLMs, humans at a shell, scripts) interact with
criome through nexus; text in, criomed-validated records
out. "Legibility to agents" is not a separate design axis
alongside nexus — nexus *is* the agent-facing interface.

---

## 2 · Philosophical moves (readings, not invariants)

### 2.1 · "Records edit records"

The centre of gravity is *queryable daemon*, not *compile
pipeline*: nexus request → nexusd → criomed → sema. The
target is a record that says this-should-exist; rsc is
downstream. The daemon is the program; the records are its
state; nexus is how you edit them. This is a database
paradigm applied to a programming environment.

### 2.2 · A constructed lexicon

`sema`, `nota`, `nexus`, `criome`, `lojix`, `horizon`,
`mentci` — constructed, philosophical, mostly Lojban-
influenced names. "sema" is *meaning*; "nota" is *mark*;
"nexus" is *binding*; "criome" gestures at *paradigm-of-
computation*; "lojix" is a play on *nix*. The lexicon
signals that the project is a theory to internalise, not a
product to adopt.

---

## 3 · Tensions still live

### 3.1 · The bootstrap paradox

Hand-written Rust during bootstrap is scaffolding;
canonical code is nexus-authored records. But how much
hand-written Rust is acceptable along the way? What is the
path from today's ~10 KLOC of scaffolding to ~0? When does
the first canonical record get authored in nexus — before
or after nexusd exists? Named, not scheduled.

### 3.2 · Hacky-stack → proper-stack absorption ordering

Li clarified 2026-04-25: the current stack that keeps the
OS running daily — cluster repos + lojix monolith +
horizon-rs + CriomOS-as-configured — is transitional
scaffolding, not architecture. Each piece eventually
absorbs into sema + proper lojix stack: horizon-rs's
projection logic becomes rules in sema; `ClusterProposal`
becomes a sema record criomed validates; NixOS module
configuration becomes records lojixd authors; the lojix
monolith becomes the thin CLI of report/030 Phase E.
Ordering is open — lojix-msg first (additive), or cluster-
config-records first (proves the non-code pattern), or
parallel?

### 3.3 · Lojix transition path (resolved — see report/030)

Current `lojix/` is a monolith in production (daily
deploys). Do not rebuild; *adapt*. A thin CLI binary
constructs `lojix-msg` envelopes for `lojixd` during
transition. Li keeps using the same `lojix` command-line
until the full engine is operational. Report/030 Phases
B–E, verbatim. Guardrail: never break `lojix deploy …` from
Li's shell workflow.

### 3.4 · Cross-criomed interaction primitives

Given §1.10 (each sema is a subjective local view;
instances negotiate agreement), what are the first-class
primitives? Candidates: content-addressed record-sharing by
hash; cross-instance subscriptions; capability tokens
across machine boundaries; signed proposals between
quorums; negotiation protocols for reconciling
disagreements; bulk-closure transfer for lojix-store.
Which are architectural from day one, which retrofittable?
The scale of interaction Li indicated (beyond current web)
forces early decisions.

### 3.5 · Machina-check prioritisation

Report/060 §5 names 7 native-checker phases (schema →
module-graph → visibility → orphan → unused → trait-solve
→ body-typeck) — these are checks over **machina**
records, not all of sema. Report/060 used the name
"semachk," which is wrong per §1.11; the checker is a
machina-check (machina-chk). Real priority order — what
buys most value first — may not match implementation-
difficulty order. Report/060 needs a rename pass alongside
the priority decision.

### 3.6 · CriomOS absorption shape

CriomOS (as-currently-configured) is part of the hacky
stack, superseded by sema + proper lojix stack. Two
flavours: (a) *absorbed* — NixOS modules become sema
records lojixd authors against a NixOS underneath that
stays; (b) *replaced* — a records-native OS that does not
need NixOS at all. Each reshapes the lojix roadmap.

---

## 4 · Prioritised question bank

### Q1 · Hacky-stack absorption ordering

From §3.2: what ordering of absorption? (a) lojix-msg first
(report/030 Phase B, additive); (b) cluster-config records
first (proves the non-code-records pattern on a real use-
case); (c) parallel. Knock-on: when is "ClusterProposal as
sema record" scheduled — Phase 1 of MVP, Phase 2, or after?

### Q2 · CriomOS supersession: absorbed or replaced?

From §3.6: is CriomOS (a) absorbed — NixOS modules become
sema records lojixd authors against a NixOS underneath that
stays, or (b) replaced — a records-native OS that does not
need NixOS at all? Determines whether lojix ultimately
subsumes nix + NixOS together or just nix-the-builder.

### Q3 · Cross-criomed interaction primitives

From §3.4: which primitives are first-class from day one?
Content-addressed record-sharing, cross-instance
subscriptions, cross-machine capability tokens, signed
proposals, negotiation protocols, bulk-closure transfer?

### Q4 · Machina-check priority

Of the 7 machina-chk phases (schema, module-graph,
visibility, orphan, unused, trait-solve, body-typeck),
which lands first in value order, independent of
difficulty? (Also: confirm "machina-chk" or propose a
better name.)

### Q5 · Lojix Phase B trigger

Report/030 Phase B (author `lojix-msg` crate) is additive
and breaks nothing. Start it now as an empty-commitment
crate, or defer until lojixd-scaffolding is closer so the
verb shapes are informed by imminent consumption?

### Q6 · Rsc projection target: minimalism vs ecosystem-compat

rsc emits `.rs` + `Cargo.toml` + `flake.nix`. Does it also
emit `#[doc(...)]`, `#[serde(rename = "…")]`, `#[repr(C)]`,
`#[inline]`, `#[cfg(...)]`? Or is the projection minimalist
and ecosystem-facing annotations are a post-MVP concern?

### Q7 · World-fact category timing

World-fact records (Entity, Relation, Observation per
report/060 §3) are a distinct category from code records
(§1.11). If they land in Phase 2, sema substrate needs
stratification support from day one (per report/060 §1).
Is stratification schema-level support in the MVP, or
retrofittable later?

### Q8 · BLS quorum: genesis-first or deferred

Report/060 §2 says MVP runs single-operator with capability
policy and no BLS; Phase-1 introduces a hardcoded genesis
quorum at first boot. Is the genesis-quorum-on-first-boot
pattern committed (needs launch config today), or still
placeholder prose?

---

## 5 · Why framings crept in and how to stop them

Seven contaminations surfaced in earlier drafts. Each
follows the same pattern: *agents read surface signals,
build plausible framings, and treat them as true in the
absence of a stated contradiction.* Fix: **name the
rejected frames explicitly**, not just the accepted ones.
Guardrails landed in architecture.md §10, bd memories
(prime-loaded at session start), and repo-level banners.

### 5.1 · aski as a live design input

I mined aski's retirement for architectural insights, then
synthesised its principles (Identity-is-Location, compile-
pipeline framing, aski-type-system) as currently load-
bearing. It is not. aski's CLAUDE.md still reads
authoritative; surface overlap (delimiter families, case
rules) invites agents to assume shared lineage. Fix: §10
rule + bd memory + banner on aski's CLAUDE.md.

### 5.2 · Personal-scale posture

Read single-writer criomed + Li's shell workflow + AGENTS.md
everywhere and inferred "craftsperson workshop." Wrong —
CriomOS supersedes proprietary OSs globally (now §1.9). Fix:
§10 rule + bd memory.

### 5.3 · Global / federated sema

Framed multi-criome as "closed worlds vs. federated global
sema." Wrong — no global database exists. Fix: §10 rule +
bd memory; §1.10 and §3.4 reframed.

### 5.4 · Category boundary as tension

Framed code-vs-world-fact as "permanent or bootstrap-
simplification?" Wrong — categories are intrinsic (now
§1.11). Fix: §10 rule + bd memory.

### 5.5 · Self-hosting close as open definition

Asked "what does 'done' mean" as if bit-for-bit identity
were a candidate bar. Stupid question — analogous to asking
whether new rustc versions must be byte-identical to
predecessors. Fix: recorded as §1.12 + bd memory.

### 5.6 · Cross-sema as federation problem

Kept swinging between "closed" and "federated" when Li's
framing is *subjective views negotiating agreement.* That
is a philosophical stance about reality, not a distributed-
systems category. Fix: §1.10 rewritten + bd memory.

### 5.7 · Legibility-to-agents as stated axis

Conflated "project is built to be tractable for LLM agents
working on it" (good hygiene, no declaration needed) with
"what is the agent-facing interface" (already decided:
nexus). Per Li, the question was confused. Fix: §1.13
names nexus as the agent interface; no separate axis.

### 5.8 · Systemic rule

Candidate addition to architecture.md §12 (update policy):
when a framing is considered and rejected, state the
rejection in §10, not just the acceptance in the body. Past
recurring rejected frames — aski-as-input, personal-scale,
global-database, boundary-as-tension, bit-for-bit-identity,
federation, legibility-axis, sema-as-data-store, four-
daemon topology, ingester-for-Rust, lojix-store-as-blob-DB,
banner-wrong-reports — all reappeared in agent output until
stated as rejections. The rule is **reject-loud**: make the
rejection visible to the next agent.

---

*End report 061.*
