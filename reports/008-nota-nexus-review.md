# Report 008 — nota / nota-serde / nexus / nexus-serde review

Post-implementation review of the work shipped in the Phase 2-7
sequence. Asks what could have been done better and what needs your
call next.

---

## 1. Verdict

Solid foundation. No crash bugs, no serde-compliance gaps, clean
superset relation (nota ⊂ nexus). ~116 tests + doctests passing,
clippy clean. Five real issues surfaced worth addressing; two
involve spec changes, three are implementation-only.

---

## 2. Issues that need fixing

### 2.1 Identifier character set drift (spec + code)

**What:** The nota spec ([nota/README.md §Identifiers](../repos/nota/README.md))
defines three identifier classes but says nothing about `_`. The
lexer I shipped
([nota-serde/src/lexer.rs](../repos/nota-serde/src/lexer.rs))
accepts `_` in both start and continue positions. So `my_field`
lexes but isn't spec-defined.

**Why it matters:** Rust structs routinely have fields like
`my_field` with underscores. If the lexer matched the spec
literally (no `_`), you couldn't deserialize most Rust structs
without renaming. The current lexer is right to accept `_`; the
spec is wrong to omit it.

**Recommended fix:** Update the nota spec — add a clarifying line
that `_` is permitted in all three identifier classes (as an
ordinary letter-equivalent). Keep the case rules (first-char
uppercase for Pascal, first-char lowercase for camel/kebab,
`-` for kebab). One-line spec addition, no code change.

### 2.2 Strings containing `|]` are unrepresentable

**What:** [nota-serde/src/ser.rs](../repos/nota-serde/src/ser.rs)
returns `Error::StringContainsMultilineCloser` if a string contains
the substring `|]`. That's because multiline strings are delimited
by `[| ... |]` with no escape syntax, so `|]` inside content would
prematurely terminate the string.

**Why it matters:** Arbitrary user text — code comments, error
messages, chat history — eventually contains `|]`. Today those
values can't be serialized. This is a real, if rare, usability
gap.

**Options:**
- **(a) Add escape syntax.** E.g. `\|]` → literal `|]`. Minimal
  change but introduces the one thing Li avoided: escapes inside
  string delimiters. Also changes canonical form subtly.
- **(b) Change the closer.** Use something less likely to collide,
  e.g. `[|` / `|/]` or `[#|` / `|#]`. Defeats the "matching pair"
  aesthetic.
- **(c) Document as limitation.** Users needing this store the
  bytes in a `Vec<u8>` and serialize with `#<hex>`. Ugly but
  explicit.
- **(d) Switch to length-prefixed strings.** Breaks the
  human-writability invariant. Reject.

**My vote:** **(c) document** for MVP, **(a) add escape** later if
pressure materializes. Real configs are unlikely to hit this;
agent-message traffic might.

### 2.3 `Bind` identifier validation is broader than the spec

**What:** [nexus-serde/src/ser.rs](../repos/nexus-serde/src/ser.rs)
validates that a `Bind` name matches `[a-zA-Z0-9_-]+`. That's more
permissive than the three identifier classes (e.g. it accepts a
bind like `@123foo` which starts with a digit, or `@-foo` starting
with `-`).

**Why it matters:** Consistency. Downstream parsers may assume
bind names follow the general identifier rules; my validation
doesn't enforce them.

**Recommended fix:** Tighten to match one of the three identifier
classes. Cleanest choice: require the bind name to be camelCase
or kebab-case (no PascalCase, since bind holes are runtime values
not types). This is an implementation-only fix; no spec change.

### 2.4 Sentinel-name dispatch is name-collision-risky

**What:** [nexus-serde/src/lib.rs](../repos/nexus-serde/src/lib.rs)
uses `#[serde(rename = "@NexusBind")]` etc. on the three wrapper
types. The serializer and deserializer dispatch by matching this
string. If an unrelated user struct uses
`#[serde(rename = "@NexusBind")]`, it silently becomes a Bind
wrapper.

**Why it matters:** Coupling-prone. The sentinel names are
effectively a protected namespace that isn't enforced. If nexus
ever adds a 4th wrapper, old serialized text containing the new
name breaks.

**Possible alternatives** (none free):
- **Marker trait** — introduce `trait NexusWrapper: Sealed {}` and
  require wrappers implement it. Problem: serde's derive doesn't
  know about custom traits, so the dispatch still ends up
  stringly-typed somewhere.
- **Magic string prefix** — keep sentinel names but document the
  `@Nexus…` prefix as reserved. Low-effort, low-reward.
- **Custom derive** — ship a `nexus_serde_derive` proc-macro crate
  that marks types precisely. Real solution; significant effort.

**My vote:** Defer. Document the `@Nexus…` prefix as reserved in
nexus-serde's README. Revisit when it bites, not before.

### 2.5 Duplication between nota-serde and nexus-serde

**What:** nexus-serde is a fork of nota-serde's code — roughly
~90% overlap across lexer, ser, de. If a bug is fixed in one
crate, it won't automatically propagate.

**Why it matters:** Maintenance burden. Divergent bug-fix state
is a real risk.

**Options:**
- **Keep independent** (current). Accept the duplication; sync
  manually when bugs are found.
- **Extract `nota-serde-core`** — a third crate housing shared
  types (Token, Lexer internals, common ser/de structure).
  nota-serde and nexus-serde become thin wrappers.
- **nexus-serde depends on nota-serde** — re-export its types and
  extend. Requires nota-serde to expose internals (`pub` on
  Lexer and Token), which widens its API.

**My vote:** **Keep independent for now**, revisit after the
first real bug lands in one and needs porting to the other. Two
crates at ~1300 LoC each is small enough to compare manually.

---

## 3. Things I noted but don't think are worth fixing now

- Multiline dedent treats tabs and spaces as 1 byte each. Mixed
  indentation produces surprising output. Real but obscure.
- No test for float edge cases (`-0.0`, subnormals). Default
  Rust formatting handles them; round-trip tests would catch
  drift but haven't.
- Map canonical-sort is by serialized key bytes. For complex
  keys (struct, enum) the byte comparison is well-defined but
  not obviously meaningful. String and integer keys are the
  normal case.
- The review agent flagged "integer overflow panic" — false
  alarm; I verified all int conversion paths use `.map_err(...)?`.

---

## 4. What's next — prioritised

**Clear (no decision needed):**

1. **§2.1 + §2.3 fixes.** Update nota README (one line about `_`),
   tighten Bind validation. 10 minutes of work.
2. **Dogfood.** Pick one real config and port it to `.nota`.
   Smallest candidate: a new nota file that replaces
   [mentci-next/devshell.nix's linkedRepos array](../devshell.nix)
   with an external `repos.nota` file read by a Nix helper.
   Bigger-win candidate: rewrite bd config in nota (but bd
   doesn't read nota — would need a tool to convert on load).

**Needs your call:**

3. **§2.2 string `|]` handling** — document-as-limitation now,
   or design an escape syntax now? My vote: document.
4. **§2.4 sentinel-name dispatch** — keep / document / proc-macro?
   My vote: keep + document.
5. **§2.5 duplication** — keep / extract core / depend directly?
   My vote: keep.

**Blocked on existing bd decisions (unchanged):**

6. nexus-schema M2 method-body layer — still blocked on
   `nexus-schema-5rw` (cross-ref ID vs Name) and `nexus-schema-wq3`
   (Callable split).
7. Pattern / Constrain / Shape wrapper types in nexus-serde —
   need your design call on how nexusd / nexus-cli model messages
   against the grammar. Three shapes to consider (from the review
   agent): (a) typed wrappers like `Pattern<T>`, (b) AST nodes
   like `PatternNode` in nexus-serde itself, (c) opaque byte
   regions parsed by consumers. My gut: **(a)** once the
   consumer-side data types emerge.

---

## 5. Questions for you

Numbered for easy reply.

1. **Identifier underscores** — accept the spec update described
   in §2.1? (I'd edit [nota/README.md](../repos/nota/README.md) to
   add a one-line note that `_` is permitted in all three
   identifier classes.)

2. **String `|]` policy** — document-as-limitation (§2.2 option c),
   or spec an escape (option a)?

3. **Bind name class** — restrict to camelCase + kebab-case? (My
   vote.) Or keep the permissive validator?

4. **Sentinel-name dispatch** — leave as-is with a "reserved"
   note in the README, or reconsider? (My vote: leave.)

5. **Duplication between the two serde crates** — park, or
   extract a shared core? (My vote: park until it bites.)

6. **Pattern / Constrain / Shape** — wrap them as typed wrappers
   (`Pattern<T>` etc.) once the messaging types exist, raw-AST,
   or opaque bytes? (My vote: typed wrappers, deferred until
   nexusd / nexus-cli pull.)

7. **Dogfood target** — prefer the flake-inputs manifest
   (`repos.nota`), the bd config, or a new workspace-level file?
   Or skip dogfood and move straight to unblocking M2?

---

## 6. What I'd do without further input

If you say "decide and proceed," my plan:

1. Apply §2.1 spec tweak and §2.3 validator tightening now.
2. Document §2.2 and §2.4 as limitations in the respective
   READMEs.
3. Write a small `.nota` file that the nota-serde integration
   test parses (the smallest possible dogfood — no tool
   rewiring).
4. Stop. Wait for your calls on the three nexus-schema /
   Pattern-wrapper design questions before touching M2.

Steps 1-3 take ~30 minutes and don't touch any decision
you've reserved.
