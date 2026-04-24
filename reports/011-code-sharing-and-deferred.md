# Report 011 — code sharing + deferred items

Research synthesis on four deferred items:
- Cross-crate duplication (~92%) between nota-serde and nexus-serde.
- Pattern / Constrain / Shape wrapper-type design.
- File-inclusion notation for nota (bd `nota-n3a`).
- Three minor test additions still outstanding.

Background: [report 009](009-nota-nexus-second-review.md) noted the
duplication was bd-tracked; [report 010 §9](010-nota-nexus-third-review.md)
listed the deferred items unchanged.

---

## 1. Duplication inventory

Precise numbers, both crates' `src/` measured line-by-line:

| File | nota-serde | nexus-serde | Shared | Unique to nexus |
|---|---:|---:|---:|---|
| `error.rs` | 40 | 40 | 100% | 0 — identical except one word |
| `lexer.rs` | 432 | 451 | ~90% | 8 token variants + 30 LoC dispatch |
| `ser.rs` | 621 | 628 | ~95% | 40 LoC sentinel dispatch in `serialize_newtype_struct` |
| `de.rs` | 726 | 721 | ~95% | 40-50 LoC sentinel dispatch + sub-deserializer glue |
| `lib.rs` | 30 | 78 | ~40% | 3 wrapper-type definitions |
| **Total** | **~1850** | **~1920** | **~92%** | **~140 LoC genuinely nexus-specific** |

Translation: nexus-serde is nota-serde plus ~140 lines of
sentinel-dispatch + wrapper types. Everything else is copy.

---

## 2. Three approaches evaluated

### (a) `nexus-serde` depends on `nota-serde`

Make nota-serde's `Lexer`, `Token`, core `Error` variants, and
low-level ser/de machinery `pub`. nexus-serde imports them and
adds its sentinel layer.

- **Pro:** Single source of truth. Bug-fixes propagate. Rule 1
  intact (two crates in two repos).
- **Con:** Widens nota-serde's public API. Every internal refactor
  of nota-serde becomes semver-relevant for nexus-serde.

### (b) Third crate `nota-serde-core`

Extract a kernel crate holding `Token`, `Lexer`, `Error`, shared
parser utilities. Both nota-serde and nexus-serde depend on it.

- **Pro:** Cleanest boundaries. Public API of nota-serde stays
  narrow. Scales to a hypothetical third format.
- **Con:** Three repos (tension with Rule 1's "one crate per
  repo" — though technically rule 1 is "one artifact per repo"
  which a lib crate satisfies). Release-coordination friction
  across three crates.

### (c) Keep duplication

- **Pro:** No refactor cost.
- **Con:** Ongoing drift risk. Every bug-fix is done twice. The
  cost grows with the surface area.

### Recommendation: **(a), with (b) as an upgrade path**

Reasoning:
- **(a) is the smallest step that resolves the duplication.**
  ~10 hours of work. No new repo. Matches patterns in the serde
  ecosystem (serde_json exposes its internals; ciborium has
  `ciborium-ll` low-level crate as a private dep).
- If nota-serde's public API starts churning painfully across
  nexus-serde's usage (i.e. the coordination cost surfaces),
  that's the signal to escalate to (b). Until then, overhead of
  a third repo isn't justified.
- **(c) is explicitly not acceptable.** Duplication this large
  will drift; it already started to during the bare-string
  rollout (char bug existed in both but was fixed once).

---

## 3. Extraction plan for (a)

Three phases, ~10-12 hours total.

### Phase 1 — expose nota-serde internals

In `nota-serde/src/lib.rs`, add:

```rust
pub mod lexer;   // promotes from `mod`
pub mod error;   // already pub via re-export; now direct module access
// Keep `ser` and `de` as `mod` (internal); only specific items pub
```

Document in the module headers: *these are internal-facing APIs;
minor-version changes may break downstream consumers.* Semver
discipline: breaking changes to `lexer::Token` → minor bump
pre-1.0, major bump post-1.0.

### Phase 2 — refactor nexus-serde

Delete `nexus-serde/src/{error,lexer}.rs`. Replace with `pub use`
re-exports:

```rust
pub use nota_serde::lexer::{Lexer, Token};
pub use nota_serde::error::{Error, Result};
```

Open question: does nexus-serde need to *extend* `Token` with its
own variants (`Tilde`, `At`, `Bang`, `{`, `}`, `(|`, `|)`, `{|`,
`|}`)? Two sub-options:

- **(a1)** Keep one unified `Token` enum in nota-serde that
  already has the nexus variants. nota-serde's deserializer
  errors on them with "reserved for nexus." Cost: nota-serde's
  API carries constructs it doesn't use.
- **(a2)** Add an extensible token mechanism (e.g. `Token::Custom(u8)`
  or a type parameter). Complicates the lexer significantly.
- **(a3)** nexus-serde defines its own `NexusToken` enum wrapping
  nota's `Token` + extra variants. Requires the lexer to be
  parameterisable over the token type.

**Recommend (a1).** The extra variants are a small tax on
nota-serde's API; simpler code than (a2)/(a3); keeps the lexer
single-implementation. Document those variants as "reserved for
the nexus superset" in the `Token` enum docs.

For ser/de: nexus-serde keeps its own `Serializer`/`Deserializer`
types. They delegate almost everything to nota-serde via a
trait-method-forwarding wrapper; the sentinel-dispatch for
`@NexusBind`/`@NexusMutate`/`@NexusNegate` is the only genuine
override.

### Phase 3 — test + release

- All 218 tests must still pass (they exercise the public API,
  not internals, so they should).
- Update `Cargo.toml` on nexus-serde: add `nota-serde = "0.1"`
  as dep.
- `nix flake check` green on both.
- Publish nota-serde 0.1.x (or 0.2.0 if public API shift is
  breaking-flagged), then nexus-serde 0.1.1.

---

## 4. Pattern / Constrain / Shape wrapper types — defer

The agent research evaluated four designs (typed wrappers,
generic Pattern<T>, string-based, defer). All involve tradeoffs
and none is clearly right without a real consumer (nexusd /
nexus-cli) driving the choice.

**Recommendation: defer.**

- The grammar already lexes `(| |)` / `{| |}` / `{ }`. Consumers
  can parse them via a raw-token path if they need to today.
- Speculative design without a concrete consumer risks building
  the wrong abstraction.
- When nexusd starts handling query messages, the first
  serialisation pattern it wants will drive the API
  (typed-wrapper vs. AST-node vs. string).

**Interim:** nexus-serde's README already documents these as
"deferred pending consumer design" — leave as-is.

---

## 5. File-inclusion notation (nota)

Li wants nota to support referencing a file's contents inline:
(a) a file's contents appear as a string value, (b) a subtree of
a record extracts into a separate file and references back.

### Prior art (summary)

- **YAML** `!include` — external library, relative paths, no sandbox.
- **Jsonnet** `import` / `importstr` — cycle detection built-in.
- **Dhall** `./path.dhall` — paths as expressions; imports are
  preserved in canonical form.
- **HCL** `file()` / `templatefile()` — function-call form.
- **Nix** `builtins.readFile` / `import` — code + hash-pinning
  for remote.
- **JSON Schema** `$ref` — URI-based, supports internal pointers.
- **XML** XInclude — structural, with `parse="xml"` vs `"text"`.

### Proposal: `@file(./path)` form

The agent proposed reusing `@` with a keyword-like `file`. One
concern: **collision with nexus's bind syntax `@<ident>`.** The
agent correctly notes `@file(` is lexically distinct from `@file`
(bind) because of the trailing `(`. But reading nota text,
`@file` looks like a bind named `file`. Ambiguity is mostly
syntactic-not-semantic (parser can disambiguate), but the
cognitive load is real.

**Alternative sigil: `&` prefix**, which is currently unused in
nexus. `&(./path)` or `&file(./path)` would avoid the conflict
and be uniformly recognisable.

**My recommendation:** `&(./path)` — single-sigil, single-form.
`&` followed by `(` begins an include. Inside the parens is a
path. Clean, no collision with `@`, matches the "sigil
introduces a thing" pattern already in the grammar.

Open question for you: `&` or `@file` or something else?

### Canonical form — two options

**(A) Always inline.** Canonical form reads each included file
and inserts its content. Result: a single self-contained nota
file. Simplest determinism.

**(B) Preserve references with content hashes.** Canonical form
keeps `&(./path)` but annotates each with `#<blake3>`. Readers
resolve by hash.

**My read:** For an ecosystem where content-addressing is
central (sema), **(B) is more natural** — the include becomes a
typed reference in the content-addressed web, not a build-time
pre-processor. Option (A) loses the relationship between
files; option (B) makes "this record references that subtree"
visible in the canonical form.

But (B) requires the deserialiser to resolve hashes, which adds
complexity. For MVP, (A) is pragmatic.

**Open question for you: (A) or (B)?**

### Scope — when to implement

This is **not MVP-blocking.** Horizon-rs isn't going to need
file-inclusion on day one. Earliest-reasonable implementation
window is after:
- Code-sharing extraction lands (so the feature lives in one
  place).
- nexusd's wire format stabilises (so we don't design
  file-include semantics and then retrofit them).

Rough: Q3 2026 or later. Not blocking anything today.

---

## 6. Minor test additions

Three tests deferred from report 010 §6:

1. **Non-ASCII forces bracket** — 3 lines.
2. **Colon-in-string forces bracket** — 3 lines.
3. **String value equal to a type name** (round-trip without
   collision) — 5 lines.

Total: ~10 minutes including commit. Bundle into whichever
next change touches `tests/edge_cases.rs`. No reason to do a
standalone commit.

---

## 7. Priority ordering

1. **Code sharing (Phase 1 → 2 → 3).** Unblocks everything.
   Eliminates the only maintenance hazard. ~10-12 hrs.
2. **Minor tests** — bundle into the code-sharing PR. ~10 min.
3. **File inclusion spec decision** — make the `&` vs `@file`
   and `(A)` vs `(B)` calls; implementation later.
4. **Pattern / Constrain / Shape** — defer until nexusd needs
   it.

---

## 8. Open questions for you

1. **Code-sharing sub-option:** (a1) extend nota-serde's Token
   with the nexus variants (my recommendation), (a2) parameterise
   the lexer over Token, or (a3) nexus-serde maintains its own
   extended token type?
2. **File-inclusion sigil:** `&(./path)` (my recommendation) or
   `@file(./path)` or something else?
3. **File-inclusion canonical form:** (A) always inline or (B)
   preserve with content hashes (my recommendation for sema's
   content-addressed model)?
4. **Pattern/Constrain/Shape deferral:** confirm you're OK
   leaving these unimplemented until nexusd surfaces a concrete
   need?
5. **Ordering:** OK with code-sharing first?

Answer any of these and I'll execute. Defaults in my
recommendations above if you'd rather I pick.
