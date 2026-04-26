# 082 — Delimiter set rewrite (nota / nexus / serde stack)

*2026-04-26 · The delimiter design we landed on in the design conversation, implemented end-to-end across the spec docs, the lexer, the ser/de kernel, and the two wrapper crates. Nix-checked everywhere.*

---

## What changed

The locked grammar moved from a 7-delimiter flat set to a clean three-family design where each family has a plain and a piped form, plus quote-bounded strings:

| Family | Plain | Piped |
|---|---|---|
| round `( )` | record | `(\| \|)` pattern |
| square `[ ]` | sequence | `[\| \|]` atomic batch |
| curly `{ }` | shape | `{\| \|}` constrain |

Strings: `" "` inline + `""" """` multiline (was `[ ]` + `[\| \|]`).

`< >` removed entirely (was sequence delimiter); `<` `>` `<=` `>=` `!=` reserved for future comparison operators.

Sigils: added `?` (validate, dry-run any verb) and `*` (subscribe to a pattern); existing `;;` `#` `~` `!` `@` `=` unchanged.

Verbs (all expressed by sigil × delimiter composition, no privileged kind names): Assert, Mutate, Retract, Validate, Query, Subscribe, Constrain, Atomic-batch. Patch is Mutate-with-pattern.

Reply semantics: positional pairing of replies to requests (no correlation IDs); reply forms reuse the request-side sigil discipline; one subscription per connection; FIFO ordering; close-the-socket to end.

---

## Per-repo summary

### nota (spec only)

| File | Change |
|---|---|
| `README.md` | Full rewrite. 2 delimiter pairs + 2 string forms + 2 sigils. Reserved-tokens section added. |
| `example.nota` | Rewritten: `[hello]` → bare or `"hello"`, `<1 2 3>` → `[1 2 3]`, multiline content indented past the `"""`. |

Pushed: nota main → `8e8feb3`.

### nexus (spec only)

| File | Change |
|---|---|
| `spec/grammar.md` | Full rewrite. Three-family design documented. Verbs table. Reply semantics section new. |
| `spec/examples/flow-graph.nexus` | Rewritten in the new grammar. Bare-ident strings used where eligible (e.g. `(Edge 102 104 writes)` not `"writes"`). |
| `spec/examples/patterns-and-edits.nexus` | Rewritten; demonstrates `~`, `!`, `?`, `*`, `(\| \|)`, `{\| \|}`, `[\| \|]`. |

Pushed: nexus main → `2b0b92a`.

### nota-serde-core (lexer + ser/de kernel)

| File | Change |
|---|---|
| `src/lexer.rs` | Token enum: removed `LAngle`/`RAngle`; added `LBracket`/`RBracket`/`LBracketPipe`/`RBracketPipe`/`Question`/`Star`. String reading switched to `" "` (with `\\` `\"` `\n` `\t` `\r` escapes) + `""" """` (verbatim + auto-dedent). `<` and `>` produce reserved-token errors in both dialects. `?` and `*` are nexus-only. |
| `src/ser.rs` | Sequences/tuples emit `[ ]`. Strings emit bare-when-eligible OR `" "` OR `""" """`. New sentinels `VALIDATE_SENTINEL`/`SUBSCRIBE_SENTINEL`/`ATOMIC_BATCH_SENTINEL` emit `?value` / `*value` / `[\| items… \|]`. Bare-ident-string path preserved (PascalCase / camelCase / kebab-case → bare; only `true` / `false` / `None` excluded). |
| `src/de.rs` | Read paths corresponding to ser changes. `deserialize_str` accepts both `Token::Str` and `Token::Ident` (the bare-ident-string path is preserved). |
| `tests/edge_cases.rs` | ~1000 lines. 100 tests: bare-string round-trips for all three ident classes, fallback to `" "` for spaces/quotes/newlines/leading-digit, fallback to `""" """` for multiline content, reserved-token errors for `<` and `>`, dedent semantics, escape semantics, sequence shape with `[ ]`, edge cases. |
| `tests/nexus_mode.rs` | 54 tests: `?` and `*` lex in nexus mode, error in nota; `[\| \|]` lexes as one atomic-batch open/close pair; sentinel dispatch for Validate/Subscribe/AtomicBatch; nested wrappers. |
| `tests/example_config.rs` | 3 tests: hand-written sample with bare-ident, `" "`, `""" """`, `[ ]`, byte literal, nested record. Round-trip-equivalence check. |
| `lib.rs` | Single unit test. |

**158 tests pass under `nix flake check`.** Pushed: nota-serde-core main → `0a7a047`.

### nota-serde (façade)

| File | Change |
|---|---|
| `Cargo.toml` | Bumped nota-serde-core git rev pin to `0a7a047b51`. |
| `Cargo.lock` | Lock refresh. |
| `flake.nix` | `outputHashes` bumped to `sha256-3Z3Ly1yBuQmEJi1lS1b36hgqM4mpbBg8Fh2rQ4qXUuI=`. |
| `src/lib.rs` | Docstring updated to the 2 delimiters / 2 string forms / 2 sigils framing. |
| `tests/smoke.rs` | 7 tests: bare-ident, `" "`, `""" """`, `[ ]` sequences, `< >` rejection, nested records. |

**7 tests + 1 doctest pass under `nix flake check`.** Pushed: nota-serde main → `7e37581`.

### nexus-serde (façade + wrappers)

| File | Change |
|---|---|
| `Cargo.toml` | Bumped nota-serde-core git rev pin to `0a7a047b51`. Removed the local `[patch]` entry now that core is published. |
| `Cargo.lock` | Lock refresh. |
| `flake.nix` | `outputHashes` bumped to `sha256-3Z3Ly1yBuQmEJi1lS1b36hgqM4mpbBg8Fh2rQ4qXUuI=`. |
| `src/lib.rs` | Three new wrapper types: `Validate<T>` (sentinel `@NexusValidate`, emits `?value`), `Subscribe<T>` (sentinel `@NexusSubscribe`, emits `*value`), `AtomicBatch<T>` (sentinel `@NexusAtomicBatch`, emits `[\| items… \|]`). Existing `Bind` / `Mutate` / `Negate` unchanged. Six wrappers total. |
| `tests/nexus_wrappers.rs` | 18 tests: existing Bind/Mutate/Negate round-trips kept; new tests cover Validate/Subscribe/AtomicBatch round-trips with primitives, structs, and nested wrappers (`Validate(Mutate(...))`, `Subscribe(Negate(...))`, `Validate(AtomicBatch(...))`); `nota_subset_roundtrip` updated to expect `[ ]` sequences. |

**18 tests + 1 doctest pass under `nix flake check`.** Pushed: nexus-serde main → `0afb303`.

---

## Bare-ident-string handling — verified

Per Li's mid-task flag, the bare-ident-string emission path was preserved across the rewrite:

- `(Node user User)` ↔ `Node { id: "user".into(), label: "User".into() }` — both fields round-trip bare.
- `(Edge 102 104 writes)` ↔ `Edge { ..., label: Some("writes".into()) }` — `Option<String>::Some(s)` with bare-eligible `s` emits bare.
- `(Package "true")` ↔ `Package { name: "true".into() }` — reserved keyword content forces the `" "` form.
- `(Person "hello world")` ↔ `Person { ..., greeting: "hello world".into() }` — content with a space forces `" "`.
- `(Story """multiline\ncontent""")` ↔ multi-line content forces `""" """`.

Tests in `nota-serde-core/tests/edge_cases.rs` (the `bare_strings` module) cover all six cases.

---

## Cross-cutting points

- **Process near-miss.** When the implementation agent was spawned, two of my parallel `Write` calls had silently failed — meaning the agent saw the *old* example files (over-quoted, using `[ ]` for strings, `< >` for sequences). The agent worked from the spec docs (correct) rather than the examples, so no wrong code landed; the verification pass confirmed bare-ident-string emission is canonical. Lesson saved as a memory: **scan each result in a parallel-tool batch before treating the bundle as successful.**
- **`[patch]` discipline.** While core was unpublished, `nexus-serde/Cargo.toml` carried a local `[patch]` redirecting the git URL to `../nota-serde-core`. After core was committed and pushed, both downstream crates pin a specific git rev (`0a7a047b51`) and have no `[patch]` entries. The `outputHashes` in their `flake.nix` files were bumped accordingly.
- **No comparison operators.** `<` `>` `<=` `>=` `!=` are *reserved* — the lexer errors on encounter. Their grammar design is deferred per the design conversation; for M0, equality matching uses positional placement in patterns.
- **No new privileged kinds at the validator.** Every verb is a sigil + delimiter composition. `Together` / `Ack` / `EndOfReply` etc. were rejected during design and never made it to implementation.

---

## Cross-cutting context

- Spec: [`nota/README.md`](https://github.com/LiGoldragon/nota/blob/main/README.md), [`nexus/spec/grammar.md`](https://github.com/LiGoldragon/nexus/blob/main/spec/grammar.md)
- Examples: [`nexus/spec/examples/`](https://github.com/LiGoldragon/nexus/tree/main/spec/examples/)
- Implementation: [`nota-serde-core`](https://github.com/LiGoldragon/nota-serde-core), [`nota-serde`](https://github.com/LiGoldragon/nota-serde), [`nexus-serde`](https://github.com/LiGoldragon/nexus-serde)
