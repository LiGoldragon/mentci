# 066 — architecture.md audit: flagged items for review

*Claude Opus 4.7 · 2026-04-25 · forensic audit per Li
directive: "It should be the golden document, and edited
with extreme care." This report flags items in
architecture.md that are not extremely-solid-certain
representations of intent, plus the two specific issues Li
named directly (reports reading-list inside architecture;
external relative links that point outside the meta-repo).
For review; no unilateral edits to architecture.md beyond
this report's filing.*

*No agents used. The architecture doc is in-context for me;
delegating an audit risks importing fresh hallucinations
exactly when precision is needed.*

---

## 1 · The two Li-named issues

### 1.1 · No report links in architecture, period

> *"why are we linking all those reports in architecture?"*
>
> *"I dont think we should link the reports in architecture.
> thats like liberally writing in the architecture document,
> which I just said should be edited with extreme care"*

The architecture doc is canonical reference for the engine's
shape. Linking reports inside it makes architecture.md
*derivative of the report tree*: every report change risks
needing an architecture.md edit, exactly when architecture.md
should be edited only with extreme care.

The principle: cross-references go *into* architecture.md
from reports, not *out of* architecture.md to reports.
Architecture.md states what is. Reports cite architecture.md
when a decision lands; architecture.md does not cite reports.

**All 17 report links in architecture.md to remove:**

Inline (4):

| Line | Section | Current | Issue |
|---|---|---|---|
| 230 | §5 sema | `Concrete schema in [reports/048]…` | Reading-pointer; pure derivative |
| 421 | §8 transitional-state note | `…the migration in [reports/030]` | The warning *itself* is load-bearing; the link is the problem. See §1.1.1 below |
| 468 | §9 grammar | `See [reports/013] for the matrix derivation…` | Reading-pointer |
| 469 | §9 grammar | `…and [reports/056] for the request-only lens refinements` | Reading-pointer **and** broken link (reports/056 was deleted) |

Reading list (13): all of §11 lines 597-626.

**Direction (for Li to confirm):** remove all 17. §11
deleted entirely. The four inline links removed; surrounding
prose either keeps its assertion without the report link or
the assertion goes too if it isn't load-bearing for
architecture.

#### 1.1.1 · The §8 transitional-state warning needs a new home

The warning at lines 417-423 ("the `lojix/` repo is currently
Li's working CriomOS deploy orchestrator … Agents must not
treat the layout above as an instruction to delete the
existing crate") is genuinely load-bearing for any agent
touching lojix. Removing the reports/030 link is fine; the
warning itself should stay or relocate.

Candidate homes for the warning:
- `repos/lojix/CLAUDE.md` (the lojix repo's own onboarding,
  where any agent reading the lojix repo will see it before
  editing).
- `docs/workspace-manifest.md` (which is allowed to discuss
  per-repo status; reports may be cited there since
  workspace-manifest is operational, not architectural).
- Both.

The architecture-side warning could become a single
sentence with no report link: "lojix is in a transitional
state; see workspace-manifest." — which itself is a doc
cross-reference but stays inside `docs/`.

### 1.2 · External relative links that escape the meta-repo

> *"we should link to the repos using the repos/ symlink, not
> a 'it works on my machine' relative links that point
> outside the meta-repo (like ../../lojix-store)"*

The `repos/` symlink directory at the mentci-next root
contains symlinks to every active sibling repo (criome,
CriomOS, CriomOS-emacs, CriomOS-home, horizon-rs, lojix,
lojix-store, nexus, nexus-cli, nexusd, nexus-schema,
nexus-serde, nota, nota-serde, nota-serde-core, rsc, sema,
tools-documentation). Anything inside mentci-next can link
to a sibling repo via `repos/<repo-name>/...` regardless of
where the repo is checked out on disk.

**Found instances of `../../...` in architecture.md:**

| Line | Current text | Should be |
|---|---|---|
| 629 | `[\`lojix-store/src/\`](../../lojix-store/src/)` | `[\`lojix-store/src/\`](../repos/lojix-store/src/)` |

Only one external relative link found. (Reports inside
`reports/` use `../reports/NNN-...md` which is internal-
relative — fine.)

**Direction:** the one link on line 629 fixed to use
`repos/`. Sweep the rest of `mentci-next` for the same
pattern in a separate pass.

---

## 2 · Internal contradictions

### 2.1 · §1 vs §5 disagree on lojix-store timing

**§1 (lines 58-62):**

> "lojix-store is a content-addressed filesystem ...
> referenced from sema by hash. **During the bootstrap era,
> `/nix/store` is the de-facto store**; lojix-store's real
> implementation is **deferred** until we're actively
> replacing nix."

**§5 (lines 234-253):**

> "lojix-store is the **canonical artifact store from day
> one**. ... `/nix/store` is a transient build-intermediate,
> not a destination. ... Why not defer lojix-store: ...
> deferred implementations rot."

These say opposite things — §1 says deferred, §5 says day-
one. Both can't be true.

The earlier rounds in this session ratified §5's framing
(lojix-store skeleton-as-design landed; bd memory
`lojix-store-is-a-content-addressed-filesystem-nix`). §1's
language is stale.

**Direction:** §1 to be rewritten to align with §5 — lojix-
store skeleton is the design from day one; real
implementation lands when its preconditions are met
(reports/030 phasing).

### 2.2 · §4 lojixd-actor list includes `CargoRunner`

**§4 (line 182):**

> "internal actors: CargoRunner (spawns cargo per RunCargo
> plan), NixRunner (spawns nix/nixos-rebuild), ..."

But §1 (line 64), §10 ("Nix is the build backend"), and
reports/059 commit to nix as the build backend. Cargo runs
*inside* nix builds (crane wraps cargo); lojixd doesn't
spawn cargo directly. There's no `RunCargo` plan in the
canonical verb list (§6 lists `RunNix`, `BundleIntoLojixStore`,
etc., not `RunCargo`).

**Direction:** drop `CargoRunner` from the lojixd actor
list; it's a remnant of an earlier framing where lojixd
invoked cargo directly.

---

## 3 · Stale references

### 3.1 · reports/056 link is broken (line 469)

§9 footer (line 469-470):

> "See [reports/013] for the matrix derivation and
> [reports/056] for the request-only lens refinements."

reports/056 was deleted in the consolidation sweep
(2026-04-25 commit `e9bac5b1`). The link resolves to a
404. Its content was absorbed into architecture.md §9
itself.

This is now subsumed by §1.1 above (no report links in
architecture, period); both reports/013 and reports/056
links go.

### 3.2 · Header timestamp is outdated (line 3)

> "*Living document · last revision 2026-04-24 · canonical
> reference for the engine's shape*"

The doc has been edited multiple times on 2026-04-25.

**Direction:** update timestamp, or drop it (timestamps
encourage timestamp-edit drift; a "last meaningful change"
field would mean more, but jj log already serves that).

---

## 4 · Type-shape specifics that may be over-specified for §

Per the scope rule (lines 9-21): "*This file is high-level
concepts only. … type sketches go in reports.*" Some
sections include specific field lists that arguably violate
this:

### 4.1 · §5 SlotBinding field list (lines 219-225)

> "Sema's index maps `slot → { current_content_hash,
> display_name, valid_from, valid_to }` as `SlotBinding`
> records."

Specific field names are type-spec territory. This sits at
the boundary — it's prose-described shape, but the field
names are committed.

### 4.2 · §5 ChangeLogEntry shape (line 227)

> "`ChangeLogEntry { rev, op, new/old hash, principal,
> sig_proof }`"

Same boundary issue. Could be redirected to reports/048
instead of inlined.

### 4.3 · §6 names that include shape hints

> "**SlotBinding** — `{ slot, content_hash, display_name,
> valid_from, valid_to }`. Bitemporal."

§6 is "Key type families (named, not specified)" — yet the
SlotBinding entry specifies fields. Same for `MemberEntry —
{ slot, visibility, kind }`.

**Direction:** decide whether §5/§6 inline these shapes
(slight rule violation but useful for top-level reference)
or redirect to report/048 / a future criome-schema report.
This is a judgment call; flagging for awareness.

---

## 5 · Naming inconsistencies

### 5.1 · `machina` named only in rejected-framings; not in positive prose

Li named the code category `machina` (bd memory
`category-structure-is-intrinsic-code-category-is-named`).
Architecture.md mentions machina only in §10 "Rejected
framings" — as part of explaining why "semachk" is wrong.
The positive content of the doc (§1, §6, §7) uses "code
records" without naming the category.

This means a reader of architecture.md won't learn that the
code category has a name unless they read the rejected-
framings list.

**Direction:** introduce the name `machina` in §1 or §6
when first naming the code category.

### 5.2 · `criome-schema` not yet referenced

reports/065 names `criome-schema` as the operational-state
schema crate. The doc's §1 / §8 doesn't reflect this — §1
mentions "schema, rules, plans, authz, history" without
distinguishing the schema crate that holds them.

**Direction:** wait for Li's decisions on report/065 §6 (Q1
crate-name boundary) before adding to architecture.md.

---

## 6 · Hand-wave / under-specified language

### 6.1 · §3 "rule-engine feasibility"

Line 125 lists "rule-engine feasibility" as a check criomed
runs on every request. This is vague:

- At Stage A-C (per reports/064), no Rule records exist; the
  check is trivially passing.
- At Stage D+, Rule records may reject mutations that
  violate invariants encoded as `is_must_hold` rules.

The phrase "feasibility" suggests a rule-engine that decides
whether a request is implementable, which isn't the design.

**Direction:** rename to "invariant preservation (Rule
records with `is_must_hold = true`)" or similar; or simply
"rule check (presently trivial; activates when Rule records
load)." Avoid "feasibility."

### 6.2 · §7 Compile+self-host loop is detailed enough to date

Lines 358-386 spell out the compile loop in eight steps with
specific verb names and field names. Per the scope rule
this is borderline — it's prose-described data flow, but
many of the names (`narhash`, `wall_ms`, `toolchain_pin`)
are field-level commitments.

**Direction:** judgment call. Could either thin to one-
sentence-per-step or move to a report and link from §7. As-
is, the section is one of the most-likely-to-go-stale
parts of the doc.

---

## 7 · Possible omissions

### 7.1 · No mention of the "rung by rung" principle outside §10

The bootstrap-rung-by-rung rule landed in §10 today. But §1,
§3, §4, §7 describe steady-state architecture without
acknowledging that the system goes through stages of
increasing competence. A reader of §7 who has not yet read
§10 might assume the full compile-loop runs from day one.

**Direction:** consider a brief paragraph in §1 or a new
sub-section pointing to the §10 rung-by-rung rule and
naming the iterative-bootstrap framing — once the framing
is settled.

### 7.2 · `genesis.nexus` not mentioned anywhere except §10

The genesis-via-nexus mechanism (per reports/064 §2.1 and
report/065 §2) is the *only* path for seed records to enter
sema. Architecture.md mentions it only in §10 as the
"bootstrap rung by rung" rule. §1's daemon descriptions and
§4's "the three daemons" do not mention how criomed boots,
how sema gets its first records, or what `genesis.nexus`
is.

**Direction:** add a short paragraph under §3 or §4 once
the genesis mechanism is fully ratified — but do not edit
without Li's confirmation.

### 7.3 · `reject-loud` rule not in §12 update policy

§10 has the reject-loud rule (state rejected framings
explicitly). §12 update policy does not mention it. So a
future agent reading §12 alone might not know to add new
rejections.

**Direction:** add reject-loud to §12 step list.

---

## 8 · Things that look correct (not flagged)

Recording these so the audit shows what passed:

- §2 invariants A/B/C — load-bearing, consistent with all
  Li corrections.
- §10 rules list — all rules consistent with bd memories;
  reject-loud subsection complete with the six rejected
  framings.
- §6 Slot / Opus / Derivation / OpusDep names — all match
  ratified terminology.
- §8 three-pillar framing (criome ⊇ {sema, lojix}) — Li-
  ratified.
- §9 sigil budget (six total) and delimiter-family matrix —
  consistent with reports/013 and Li's framing.
- §1 macro philosophy paragraph — matches bd memory.
- §4 daemon invariants ("Text crosses only at nexusd's
  boundary" etc.) — all consistent.

---

## 9 · Summary table

| # | Severity | Section | Issue | Direction |
|---|---|---|---|---|
| 1.1 | Li-named | §5/§8/§9/§11 | 17 report links inside architecture.md (4 inline + 13 reading-list) | Remove all 17; §11 deleted; §8 transitional warning relocated to lojix/CLAUDE.md or workspace-manifest |
| 1.2 | Li-named | §11 line 629 | `../../lojix-store/src/` external link | Replace with `../repos/lojix-store/src/` (and also goes when §11 is deleted) |
| 2.1 | High | §1 vs §5 | Lojix-store deferred vs day-one contradiction | Align §1 to §5's day-one framing |
| 2.2 | High | §4 line 182 | `CargoRunner` lojixd actor stale | Drop it |
| 3.1 | Med | §9 line 469 | Broken link to deleted reports/056 | Drop the reference |
| 3.2 | Low | header line 3 | Stale "last revision" date | Update or drop |
| 4.1-4.3 | Judgment | §5, §6 | Field-shape specifics violate "high-level only" rule | Either redirect to reports or accept the boundary |
| 5.1 | Med | §1, §6 | `machina` not used in positive prose | Introduce the name |
| 5.2 | Wait | various | `criome-schema` not yet referenced | Pending Q1 in report/065 |
| 6.1 | Med | §3 line 125 | "Rule-engine feasibility" is vague | Rename to "invariant preservation" |
| 6.2 | Judgment | §7 | Compile-loop detail likely to go stale | Thin or move to report |
| 7.1-7.3 | Med | §1/§3/§12 | Bootstrap-rung-by-rung / genesis.nexus / reject-loud not propagated | Add references after Li ratifies |

---

## 10 · For Li

Awaiting direction on:

(a) Whether to apply Li-named #1.1 and #1.2 immediately, or
hold for fuller review along with the rest.

(b) Which §1-vs-§5 lojix-store framing is canonical (§5
appears correct based on session history; confirming).

(c) Whether the §4-§7 specifics that violate the "prose +
diagrams only" rule should be redirected to reports or
treated as exceptions.

(d) Whether to introduce `machina` in positive prose now,
or defer until naming for other categories settles.

(e) Whether Items 7.1-7.3 (rung-by-rung, genesis.nexus,
reject-loud propagation) should be added now or later.

This report does not edit architecture.md.

---

*End report 066.*
