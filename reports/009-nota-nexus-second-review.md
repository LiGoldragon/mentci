# Report 009 — nota/nexus second-round review

Follow-up to [report 008](008-nota-nexus-review.md) after the
positional-records rewrite, bind-aliasing spec, edge-case test
battery, and u128 fix. Written after a thorough agent-assisted
review of the current state.

---

## 1. Verdict

Healthy. No blockers, no crash bugs, specs coherent with code.
Five small things worth doing; one agent finding was wrong and
one is a spec ambiguity I think we should resolve soon.

Current state: 165+ tests across both serde crates, `nix flake
check` green on both, 0 ignored tests, clippy clean.

---

## 2. Agent findings — my read

### Valid

- **Bind validation is permissive.** nexus-serde accepts
  `@_h`, `@-h`, `@123` (digits first, underscore first, hyphen
  first). The spec says binds follow identifier classes —
  camelCase or kebab-case for values. Low-risk gap; tightening
  is ~15 min of work plus three tests.
- **`#[serde(rename)]` on structs is not documented as
  unsupported.** A user with `#[serde(rename = "X")]` on a
  struct `Foo` will have the derive emit `serialize_struct("X",
  ...)`, which nota-serde's de will expect as `(X …)`. That
  actually *works* — the rename passes through. But
  `#[serde(flatten)]` genuinely doesn't work with positional
  records, and nothing documents that. One test + one paragraph
  in README.
- **No real `.nota` file exists yet.** All tests are synthetic.
  Dogfooding is overdue; report 008 §4 flagged this.
- **~90% code duplication** between the two serde crates —
  already bd-tracked (`nexus-serde-bd` issue); not worth acting
  on yet.

### Wrong

- **"All four crates missing `rust-toolchain.toml`"** — false.
  nota-serde and nexus-serde both have it (verified via `ls`).
  nota and nexus are spec-only repos and don't need it per
  style.md. No action.

### Nuanced

- **`visit_seq` vs `visit_map` for structs.** Agent flagged
  this as a potential issue with `#[serde(rename)]`. Not quite
  — serde-derived Visitors implement *both*, so either works.
  The real incompatibility is with `#[serde(flatten)]`, which
  requires map semantics. Positional records can't flatten
  coherently anyway (you'd be silently interleaving field
  orders); rejecting is correct, but a clearer error would
  help.

---

## 3. Things I'd flag

### 3.1 Identifier-class ambiguity with leading `_`

Spec says `_` is "continue position; not a valid leading
character on its own for kebab." But the lexer
(nota-serde/src/lexer.rs `is_ident_start`) accepts `_` as
a start character. That means `_foo` lexes as an ident — but
which class? PascalCase requires uppercase first, camelCase
says "first char lowercase" (is `_` lowercase?), kebab-case is
"lowercase with `-`." None cleanly covers `_foo`.

Three resolutions:
- **(a)** Tighten the lexer: reject bare `_`-leading idents.
  Forces Rust code with `_private` fields to rename or use
  `#[serde(rename)]`.
- **(b)** Declare `_`-leading idents are camelCase-kindred —
  update the spec.
- **(c)** Let the parser class-dispatch on the first *non-`_`*
  character. Handles `_MyType` (Pascal), `_my_field` (camel),
  `_my-tag` (kebab).

My vote: **(b)** — simplest spec amendment, matches Rust
practice, no code change. Leading `_` always sorts as the
lowercase class it resembles.

### 3.2 `#[serde(flatten)]` not documented as forbidden

If a user puts `#[serde(flatten)]` on a field, the derive
generates map-based serialization. Our `visit_seq` path would
either silently produce wrong output or fail with a confusing
error. Add a sentence in nota/README.md's "Forbidden
constructs" and a regression test.

### 3.3 Strings containing exactly `]`

Test `inline_string_with_bracket_forces_multiline` covers
embedded `]` in multi-character strings. Untested: what about
the degenerate case `[[]]`? (Does that parse as "empty string
followed by extra `]`" or "string containing `]`"?) The
serializer's path is fine — it switches to `[| |]`. The
deserializer's behaviour is worth an explicit test.

### 3.4 Map key serialisation collisions

If a pathological serializer emits the same bytes for two
different values (e.g. a lossy custom `Serialize`), canonical
sort collides. Real code is unlikely to hit this, but it's
worth a one-line doctrine note in nota/README.md §Canonical
form: "assumes `Serialize` is injective on distinct values."

### 3.5 `rust-toolchain.toml` is out of sync with flake fenix pin

nota-serde/rust-toolchain.toml says `channel = "1.85"`. The
flake uses `fenix.packages.${system}.stable` which currently
resolves to something newer. Inconsistency; users following
rust-toolchain get 1.85, users running via `nix develop` get
the fenix-stable. Not breaking but worth aligning — either
pin fenix explicitly or loosen rust-toolchain.

---

## 4. Test coverage gaps

Agent's top-5 list; I concur with four of five:

1. **Bind name boundary cases** — `@_h`, `@-h`, `@123` should
   all reject. Three tests, ~5 min.
2. **`#[serde(flatten)]` rejection** — one failing test that
   documents the limitation.
3. **Empty/single-`]` string corners** — `[]`, `[]]`, `[[]]`
   parsing behaviour.
4. **f32 subnormals** — edge_cases covers f64 subnormals; add
   the f32 counterpart for parity.
5. *(skip)* **Map key-collision test** — too pathological to
   test reasonably; document the assumption instead.

Adding these would bring the battery to ~85-90 tests.

---

## 5. Workflow rule review

The "Nix-based tests" section in
[tools-documentation/rust/style.md](../repos/tools-documentation/rust/style.md)
is good. Two follow-ups from practice:

- The rule doesn't cover workspace flakes. Not a concern for
  sema (rule 1 forbids workspaces), but worth a note.
- The "bug-found-by-test must be fixed or bd-tracked, not
  silently `#[ignore]`'d" rule was exercised this session and
  worked well — the u128::MAX test went from `#[ignore]` →
  bd-filed → fixed → test live, which is the right flow.

---

## 6. Hygiene

All four repos clean. Remotes correct. Working tree empty on
all. bd databases healthy. No drift.

---

## 7. Suggested next steps

Priority-ordered. Each is small enough to land in one focused
pass.

1. **Resolve §3.1 (leading `_` ambiguity).** Spec-only change;
   my recommendation is (b) — update nota spec that `_`-leading
   idents sort as camelCase-kindred. 5 min.
2. **Tighten bind validation (§3.2-adjacent) + add the 3
   boundary tests.** 15 min. Clean out a real spec-code drift.
3. **Document forbidden serde attrs (§3.2) — README + one
   regression test.** Anticipates a user's first footgun. 10 min.
4. **Dogfood: first real `.nota` file.** Smallest target:
   `devshell.nix`'s `linkedRepos` as an external `repos.nota`
   read by a Nix helper. 30-60 min. Will surface
   error-message quality issues that unit tests won't.
5. **Pin fenix / align rust-toolchain.** 10 min. Removes a
   silent drift.

Steps 1-3 + 5 are pure cleanup; #4 is the first validation
against real use. I'd do them in that order.

Deferred (bd-tracked): core extraction between the two serde
crates, Pattern/Constrain/Shape wrapper types, file-inclusion
notation research (nota-n3a).

---

## 8. What's *not* a concern

Enumerating for clarity:

- Integer handling (u128 fix works, boundary covered).
- Dedent (fix works, all-blank case covered).
- Canonical form (deterministic, tested).
- `nix flake check` (passes on both crates).
- Spec-code coherence for the core grammar (positional
  records, sentinels, sigils).
- The one-artifact-per-repo rule (holding).
