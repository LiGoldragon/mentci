# 108 — Flow graph as shared substrate: three projections of one data model

*Architectural design report. The first concrete use case for criome.
Captures Li's idea verbatim before any implementation work; surfaces
the decisions to make before code starts. Lifecycle: lives until the
design is encoded into criome/ARCHITECTURE.md + the relevant repos,
then deleted.*

## 0 · TL;DR

A flow graph is a set of records in criome's sema. The same records
project into **three surfaces**:

1. **nexus text** — already shipping; the text edit/inspect surface.
2. **runtime creation orchestrated by `lojix-daemon`** — `prism`
   emits `.rs` from the records (records → `.rs` source files);
   lojix-daemon orchestrates the surrounding work via existing
   `lojix-schema` verbs (`RunNix` for the nix-via-crane-and-fenix
   compile, `BundleIntoLojixStore` for the artifact landing). The
   emitted code is a working ractor-based actor runtime; what runs
   is the system the records describe. Full flow lives in
   [`criome/ARCHITECTURE.md` §7](../repos/criome/ARCHITECTURE.md).
3. **mentci UI** — renders the graph visually in real time; user
   gestures (click, drag, typing — the keyboard counts) translate into
   signal edit messages, criome validates, the UI reflects the
   accepted change.

Criome remains the single source of truth and the only validator. The
three surfaces are *projections*, never independently authoritative.

## 1 · The core proposition

The flow graph is **a specification**. Three readers consume the same
specification for different ends: humans-and-agents read it as text,
the build system reads it to emit a runtime, the UI reads it to draw
pixels and accept gestures. None of the three holds independent state;
all three round-trip through criome.

The flow graph is also **the first concrete use case for criome's
self-hosting loop**: `lojix-daemon` orchestrates the
records-to-runtime pipeline, with `prism` emitting the Rust source
(the daemon assembles the workdir, calls nix-via-crane, lands the
artifact in lojix-store). Its first customer is criome itself (per
`bd mentci-next-0tj`), but the same mechanism handles user-authored
flow graphs that compile to user-authored runtimes.

## 2 · The substrate — flow graphs as records

The record kinds already exist in [signal](https://github.com/LiGoldragon/signal):

- `Graph(title: String)` — the container.
- `Node(name: String)` — a vertex.
- `Edge(from: Slot, to: Slot, kind: RelationKind)` — labeled directed
  edge.

A *flow graph* is one `Graph` plus the closed set of `Node`s and
`Edge`s that point at it. Edges are typed by `RelationKind`; nodes
are flat for now (one `Node` shape; subkinds will arrive as new
typed structs in signal as the design demands). The graph is a
directed labeled multigraph.

The records live in sema; everything downstream reads from there.

### What's authoritative today

Schema-as-data scaffolding — `KindDecl`, `FieldDecl`, `Cardinality`,
`KindDeclQuery` — was dropped from signal in [commit 8b101c8d](https://github.com/LiGoldragon/signal/commit/8b101c8d5a3c)
under Path A of the §8 Q12 decision (the original Q12 has been
resolved). The closed Rust enum in signal is the **authoritative
type system**. New record kinds land by adding the typed struct +
the closed-enum variant + propagating through criome's hand-coded
dispatch. Schema-as-data records will be re-added when `prism` or
mentci has a real reader for them — until then, the scaffolding
would have been inert.

## 3 · Projection 1 — nexus text (already shipping)

```
(Graph "request flow")
(Node "incoming-frame")
(Node "validator")
(Edge 100 200 DependsOn)
(| Node @name |)
```

The text surface — `nexus-cli` writes text to `nexus-daemon`,
`nexus-daemon` parses to signal frames, `criome-daemon` validates +
applies, replies thread back as text. Verified end-to-end via
`mentci-integration` in `nix flake check`. **No work pending here for
this design.**

## 4 · Projection 2 — runtime creation via `lojix-daemon` (prism emits the source)

The records-to-runtime path is owned by **`lojix-daemon`**. The flow
is documented in
[`criome/ARCHITECTURE.md` §7 — Compile + self-host loop](../repos/criome/ARCHITECTURE.md):
on a `Compile` request, criome reads the Opus + transitive OpusDeps
from sema and **forwards them to lojix** as a typed `lojix-schema`
verb. **criome itself runs nothing** — per the workspace doctrine
(criome ARCH §10), criome communicates and persists; effect-bearing
work is done elsewhere. lojix-daemon links `prism` and runs the
full pipeline internally: prism emits `.rs` from the records →
lojix assembles the scratch workdir (`.rs` + `Cargo.toml` +
`flake.nix` + crane glue) → NixRunner spawns `nix build` →
StoreWriter runs `BundleIntoLojixStore` (copy-closure, RPATH
rewrite via patchelf, deterministic bundle, blake3 hash, write
tree under `~/.lojix/store/<blake3>/`). lojix replies with
`{ store_entry_hash, narhash, wall_ms }`; criome asserts a
`CompiledBinary` record back to sema.

**`prism` is the code-emission piece** of lojix's pipeline. The
rest of this section focuses on prism's piece — the code-emission
shape — since that's where the macro-programming happens. The
exact `lojix-schema` verb that carries records from criome to
lojix lands when `lojix-daemon` is wired (candidates: a new
`Build(records)` verb or a sequence of `MaterializeFiles` +
`RunNix` + `BundleIntoLojixStore`). Today both prism and
lojix-daemon are skeleton-as-design (see
[`lojix/ARCHITECTURE.md`](../repos/lojix/ARCHITECTURE.md)).

`prism` reads flow-graph records from sema and emits Rust source code.
Crucially, this is **macro programming**, not naive code generation:

- The input is **structured records**, not source-text tokens.
- The emission is **template/pattern substitution per node-kind and
  edge-kind** — the templates are hand-coded inside `prism` itself
  (one template per kind, written in Rust). When `prism` ships, adding
  a new node-kind means adding the typed struct in signal *and* the
  emission template in prism.
- The output is Rust source that **becomes a running actor system**
  when compiled.

So the analogy is to Rust's proc-macros — pattern in, source out — but
the input is records-from-sema rather than a `TokenStream`. That input
form is the load-bearing difference. Each record-kind defines its own
expansion shape; prism walks the graph and emits the union.

### What gets emitted

For a flow graph, the emitted Rust includes:

- **One ractor `Actor` per `Node`**, with the per-node-kind State /
  Arguments / Message shape determined by the node's typed kind in
  signal.
- **Typed message routes for each `Edge`**, wired between the actors
  the edge connects. `RelationKind` determines the wire protocol —
  fire-and-forget cast vs request/response call vs streaming
  subscription.
- **A root supervision tree per `Graph`**, with the Graph node serving
  as the supervision root.
- **A `main` shim that boots the supervision tree** with environment-
  driven configuration.

The emitted code follows the same patterns documented in
[`tools-documentation/rust/ractor.md`](../repos/tools-documentation/rust/ractor.md):
one actor per file, four-piece template (Actor / State / Arguments /
Message), per-verb typed `RpcReplyPort<T>` messages, supervision via
`spawn_linked`, sync façade on State where useful.

### The bootstrap loop

1. Records describing criome's own request flow live in sema.
2. lojix-daemon's pipeline runs: `prism` emits Rust from the
   records; the daemon assembles the workdir + calls nix-via-crane;
   the artifact lands in lojix-store.
3. The new criome binary reads from sema (which contains the
   records that compiled it).
4. Editing the records → re-emit → recompile → re-land → criome
   runs its new shape.

This is the self-hosting "done" moment (`bd mentci-next-ef3`).

## 5 · Projection 3 — mentci UI: live visual render + edit loop

mentci's first concrete user-facing feature is the flow-graph editor.

### Render

mentci connects to `criome-daemon` over UDS. It reads flow-graph
records and renders them as a visual graph — boxes for nodes, arrows
for edges, kind-driven styling. **Real-time**: when sema changes, the
visual updates without manual refresh.

The "real-time" requirement implies subscribe-protocol support
(M2+ on criome's roadmap). Per the workspace rule **push, not
pull** ([tools-documentation/programming/push-not-pull.md](../repos/tools-documentation/programming/push-not-pull.md)),
mentci's UI launches *after* `Subscribe` ships — there is no
poll fallback.

### Edit

User gestures translate into signal edit messages:

| Gesture | Signal verb | Argument |
|---|---|---|
| Drag a new box onto canvas | `Assert(Node)` | `Node { name: "..." }` (kind chosen by user) |
| Drag a wire between two boxes | `Assert(Edge)` | `Edge { from, to, kind }` |
| Delete a box | `Retract(Node)` | `slot` |
| Edit a box's name | `Mutate(Node)` | `{ slot, new, expected_rev }` |
| Bulk-edit (rename + retype) | `AtomicBatch([…])` | sequence of operations |

Each *committed* gesture becomes one signal message. mentci shuttles
it to `criome-daemon` directly over UDS, speaking signal.

### Local in-flight buffer — typing isn't keystroke-by-keystroke

A "gesture" in the table above is a **committed intent**, not every
mid-flight keystroke or pixel of mouse movement. Typing the name of a
new node, dragging a wire mid-air, hovering over a candidate target —
these are buffered locally in the UI and become signal messages **only
on commit** (Enter, mouse-up on a valid drop target, explicit "submit"
action). Otherwise mentci would flood criome with a request per
keystroke, and criome would validate-and-reject most of them.

This doesn't violate the "mentci never holds state that contradicts
criome" rule — in-flight buffer state isn't *contradicting* criome,
it's *pending input that hasn't been submitted yet*. The cursor is in
a text input; the wire is following the mouse; the new-node placeholder
hasn't been asserted yet. None of that exists in sema until commit.

The table's gesture rows are therefore **commit-time atoms**:

- "Edit a box's name" = the user finished typing and pressed Enter →
  one `Mutate(Node)`.
- "Drag a wire" = mouse-down on source, drag, mouse-up on target →
  one `Assert(Edge)` if the drop landed validly; nothing if the user
  dropped in dead space.
- "Drag a new box onto canvas" = the type-and-place gesture finishes
  with the placement + name commit → one `Assert(Node)`. Or
  `AtomicBatch([…])` if the user wired some initial edges in the
  same "create" gesture — composite gestures wrap atomically per
  §8 Q11 RESOLVED.

### The accept-and-reflect loop

```
1.  User gesture on canvas
       │
       ▼
2.  mentci translates → signal::Request
       │
       ▼
3.  criome-daemon validates
    (schema → refs → invariants → permissions → write → cascade)
       │
       ├─── Reject (validation failure)
       │       │
       │       ▼
       │   Reply::Outcome(Diagnostic { level, code, message, … })
       │       │
       │       ▼
       │   mentci paints rejection inline next to the failed gesture;
       │   user sees WHY and can edit-and-retry
       │
       └─── Accept
               │
               ▼
       Reply::Outcome(Ok)  +  durable record-state change
               │
               ▼
       mentci reads the new state (subscribe push or poll re-read)
               │
               ▼
       Visual updates
```

The loop's load-bearing property: **mentci never holds state that
contradicts criome**. Every accepted edit is reflected because mentci
re-reads from criome, not because mentci is its own source. This
matches the project-wide invariant — sema is the concern; everything
orbits.

## 6 · Putting it together

```
                         ┌──────────────────────────┐
                         │  flow-graph records      │
                         │  in criome's sema        │
                         └──────────────────────────┘
                            ▲          ▲          ▲
                            │          │          │
                            │ edits    │          │ edits
                            │ via      │          │ via
                            │ nexus    │          │ mentci
                            │ text     │          │ gestures
                            │          │          │
                            │          │          │
                  ┌─────────┘          │          └─────────┐
                  │                    │                    │
                  │                    │                    │
        ┌───────────────┐    ┌──────────────────┐    ┌─────────────┐
        │ nexus-daemon  │    │  lojix-daemon    │    │  mentci     │
        │ — text edit / │    │  pipeline:       │    │  — visual   │
        │   inspect     │    │  prism emits .rs │    │    render + │
        │   surface     │    │  daemon compiles │    │    edit     │
        │   (existing)  │    │  artifact lands  │    │  (M? …)     │
        └───────────────┘    │  in lojix-store  │    └─────────────┘
                             │   (M1 …)         │
                             └──────────────────┘
                                       │
                                       ▼
                              compiled actor system
                              at runtime
                              (the system the records describe)
```

## 7 · Phasing

§8 answers are in (resolved 11; deferred 1). Phases as
currently understood:

- **M0** — nexus text shuttle, criome+nexus daemons ractor-hosted, demo
  graph operations end-to-end. **Done.**
- **M1** — `prism` minimum: emits a known-shape Rust file from a
  known-shape record (start small, e.g. one `Node` record →
  one ractor `Actor` skeleton). Per-kind sema tables
  (`bd mentci-next-7tv`) land here as the storage shape `prism` reads
  from.
- **M2** — `Subscribe` request shipped on criome side; mentci can
  subscribe to record changes for live updates.
- **M3** — mentci UI v0: read-only visual render of flow graphs from
  criome.
- **M4** — mentci UI v1: gesture-driven edit; `Assert` / `Mutate` /
  `Retract` round-trip with diagnostic feedback inline.
- **M5** — `prism`'s macro projection extended to emit ractor runtime
  actor systems from flow-graph records (the "compile a graph into
  a daemon" milestone).
- **M6** — bootstrap: criome's own request-flow lives as records in
  sema; prism emits criome from them; the loop closes
  (`bd mentci-next-zv3`, `bd mentci-next-ef3`).

## 8 · Open questions

The deep dive surfaces decisions that gate concrete design work:

1. **~~Which node kinds anchor the first emission?~~** **RESOLVED** —
   Li 2026-04-28: the tentative trio is reasonable, extend it via
   research on dynamic-systems node taxonomies. Survey of Akka
   Streams, Reactor, Flink, Storm, Kafka Streams, OTP, FBP/NoFlo,
   DSP, process calculi, and Petri nets converged on a closed set
   of **5 first kinds**:
   - **Source** — zero fan-in, emits from external boundary.
   - **Transformer** — 1→1, per-message processing.
   - **Sink** — zero fan-out, consumes to external boundary.
   - **Junction** — fan-in>1 or fan-out>1, topology-only (Merge,
     Broadcast, Balance, Zip).
   - **Supervisor** — control-plane node whose explicit job is to
     host children of a subsystem. Holds child registry, restart
     strategy (one-for-one / one-for-all / rest-for-one), and
     restart history per child. Not on the data path; receives
     `SupervisionEvent`s (not user messages) via ractor's
     `handle_supervisor_evt`. Edges from a Supervisor to its
     children are **control-plane relations** — `Supervises`
     (parent→child) and `EscalatesTo` (child→parent for failure
     routing) — not data edges like `DependsOn`. Signal's
     `RelationKind` enum will grow these control-plane variants
     when the Supervisor kind lands. *Most graphs won't declare
     an explicit Supervisor*: per
     [`tools-documentation/rust/style.md` §Actors](https://github.com/LiGoldragon/tools-documentation/blob/main/rust/style.md#actors-logical-units-with-ractor),
     supervision is recursive — every parent actor supervises its
     children whether or not it has data-plane responsibilities.
     Supervisor as an explicit kind is for **fault-isolation
     boundaries** — when the design wants a node whose whole
     purpose is hosting children of a subsystem (and `style.md`'s
     "Use actors for components, not for chores" applies).

   *(Note 2026-04-29 — Li's pushback on the original "no data
   flow" framing surfaced the data-plane-vs-control-plane
   distinction. A Supervisor has data; the data is meta about
   its children, not user messages flowing through.)*

   Anti-recommendations (don't adopt): DSP `Filter` (analog
   semantics), Petri `Place` (passive token-holder), separate
   `Mixer`/`Splitter` (collapse into `Junction`),
   `gen_event`/`gen_statem` (those are edge kinds, already covered
   by `RelationKind`), Storm `Spout`/`Bolt` (coarseness), FBP
   `Process` (too generic), `Composite`/`Subnet` (handle as graph
   operation, not as a kind).

2. **~~Smallest first demo graph.~~** **RESOLVED** — Li 2026-04-29:
   the candidate is the answer. Encode criome's M0 request flow
   (`Frame → Validator → Sema → Reply`) as records, have `prism`
   emit a working daemon from them, run it, watch
   `mentci-integration` pass against the prism-emitted binary
   instead of the hand-coded one. This is the M6 bootstrap
   close (`bd mentci-next-zv3`).

3. **~~`prism`'s emission shape — proc-macro, build-script, or
   standalone binary?~~** **RESOLVED** — Li 2026-04-28: prism is a
   **library**. Not a CLI ("no reason to make it a CLI"). A
   proc-macro entry could land later as a secondary surface, but
   proc-macro alone wouldn't be enough — `lojix-daemon` (Rust) needs
   to call into prism as part of its runtime-creation orchestration,
   and that is a library call. The library reads flow-graph records
   (in-memory or via a sema reader) and emits Rust source (in-memory
   or to disk).

4. **~~mentci UI tech.~~** **RESOLVED** — Li 2026-04-28: Linux desktop
   only; pick from the top three Rust desktop frameworks for a
   real-time graph canvas with gesture-driven editing **and**
   interactive custom shapes (a wheel the user can rotate
   interactively; eventually astrological charts with rotatable
   inner/outer rings). Top 3 ranked:
   1. **egui** — immediate-mode, `egui::Painter` does arbitrary 2D
      including rotation transforms,
      [`egui-graph-edit`](https://github.com/kamirr/egui-graph-edit)
      exists as a turnkey starting point, clean nix builds via
      wgpu/glow. **Recommended.** Immediate-mode is the natural fit
      for a daemon-pushed truth-source where every frame redraws
      from current state.
   2. **iced** — Elm-architecture, retained-mode, what System76's
      cosmic desktop uses; `Canvas` widget with bezier paths +
      caches; better if the UI grows lots of form chrome around
      the canvas.
   3. **gpui** — Zed's framework on `wgpu`; highest perf ceiling
      but you'd be pinned to Zed's monorepo / vendored fork.

   Disqualified: druid (archived), slint (DSL-first, awkward custom
   canvas), dioxus-desktop (webview), makepad (wrong size+shape),
   xilem/floem/freya (still pre-1.0).

5. **~~mentci ↔ criome connection — direct UDS or via nexus-cli?~~**
   **RESOLVED** — Li 2026-04-28: **direct UDS, mentci speaks signal**.
   The architectural rule (now first-class in
   [`criome/ARCHITECTURE.md` §1](../repos/criome/ARCHITECTURE.md)):
   criome speaks **only signal**; signal is the messaging system of
   the whole sema-ecosystem. nexus is one front-end (text↔signal
   gateway, for humans/agents/scripts), mentci will be another
   (gestures↔signal). Nexus is not in mentci's path. Any future
   client (alternative editor, headless tool, etc.) connects to
   criome the same way — by speaking signal directly.

6. **~~Subscribe-first vs poll-first.~~** **RESOLVED** — Li 2026-04-29:
   **push never pull**. No polling, ever. mentci's UI launches after
   `Subscribe` ships (M2). The principle is documented as a workspace
   design rule in
   [`tools-documentation/programming/push-not-pull.md`](https://github.com/LiGoldragon/tools-documentation/blob/main/programming/push-not-pull.md)
   so future agents inherit it. Polling is wrong; producers push,
   consumers subscribe.

7. **~~Edit-to-message translation library.~~** **RESOLVED** —
   Li 2026-04-29: **`mentci-lib` as a separate crate**. Holds the
   signal-speaking logic (gesture → signal envelope translation,
   plus the criome-link + reply demux). Consumed by the future GUI
   repo and by alternative UIs (mobile, etc.) that may follow. Per
   `tools-documentation/rust/style.md` §"One Rust crate per repo",
   `mentci-lib` lives in its own dedicated repo. New bd issue
   filed.

8. **~~Diagnostic UX.~~** **RESOLVED** — Li 2026-04-29: show
   rejections **visibly** somewhere; the specific shape (inline
   overlay vs side panel vs toast) is a styling concern that can
   land later. The data model is already rich enough (code,
   message, primary_site, suggestions, severity); the UI just
   needs to surface the diagnostic non-discardably when criome
   rejects. Specific styling deferred to post-prototype.

9. **~~The "main repository" — confirm `mentci`.~~** **RESOLVED** —
   Li 2026-04-28: yes, `mentci`. Reframing: `mentci` today is two
   things at once — (a) the **workspace umbrella** (this repo: dev
   shell, design corpus, agent rules, reports), and (b) a
   **concept goalpost** (the eventual LLM-agent-assisted editor /
   universal UI). The actual GUI implementation will land in a
   **separate future repo** when work begins; "mentci" is the
   working name for it in design docs until that repo is created
   (and possibly named differently). See
   [`mentci/ARCHITECTURE.md`](../ARCHITECTURE.md) for the long-term
   framing.

10. **~~Recursive rendering — long-term.~~** **DEFERRED** —
    Li 2026-04-29: out of scope for the prototype era. "Get a
    running prototype first." Re-open when the M3+ mentci UI is
    working against real graphs.

11. **~~Composite-gesture atomicity.~~** **RESOLVED** — Li 2026-04-29:
    **atomic** — composite gestures wrap in `AtomicBatch([…])`,
    all-or-nothing. The all-or-nothing shape matches the user's
    mental model of "create *this thing*" being one step, and
    matches the natural elegance criterion (per
    [`tools-documentation/programming/beauty.md`](https://github.com/LiGoldragon/tools-documentation/blob/main/programming/beauty.md)).
    No "atomic mode" modifier — atomic is just the rule.

12. **~~`KindDecl` — naming + role in M0.~~** **RESOLVED** — Li chose
    Path A 2026-04-28. `KindDecl` + `FieldDecl` + `Cardinality` +
    `KindDeclQuery` were dropped from signal in commit 8b101c8d.
    Schema-as-data records will be re-added when `prism` or mentci has
    a real reader for them. The closed Rust enum in signal is the
    authoritative type system today; new kinds land by adding the
    typed struct and propagating through hand-coded dispatch.

## 9 · Where this report leaves to implementation

§8 questions are resolved (11) or deferred (1, Q10). Concrete
work per layer:

- **`signal`** — new `BuildRequest` verb with `target: Slot`
  payload (per Li 2026-04-29: the verb criome accepts/denies
  and forwards to lojix); 5 first node-kind structs as the
  taxonomy lands (Source / Transformer / Sink / Junction /
  Supervisor — Q1 resolved); `RelationKind` grows
  control-plane variants when Supervisor lands (`Supervises`,
  `EscalatesTo`); `Subscribe` request stays as M2 work.
- **`lojix-schema`** — new verb (or verb sequence) carrying
  records from criome to lojix; exact shape lands with
  lojix-daemon's wiring.
- **`criome`** — `BuildRequest` engine handler (validates +
  forwards to lojix); per-kind sema tables (bd
  mentci-next-7tv); `LojixLink` client module mirroring
  nexus's `CriomeLink`; diagnostic-emission richness; Subscribe
  verb at M2. **criome runs nothing** (§10) — handler validates
  and forwards; lojix executes.
- **`lojix`** — UDS listener body; receives the new build verb;
  links `prism` as a library; orchestrates prism →
  FileMaterialiser → NixRunner → StoreWriter internally.
- **`prism`** — library skeleton + first emission template
  (one node-kind → one ractor `Actor` skeleton; per Q3
  resolved: prism is a library, not a CLI). Linked by lojix.
- **`mentci-lib`** — separate crate (Q7 resolved): gesture →
  signal envelope translation + criome-link logic; consumed
  by the future GUI repo and any alternative UIs (mobile, etc).
- **GUI repo** — egui-based flow-graph editor (Q4 resolved);
  speaks signal directly to criome via mentci-lib (Q5
  resolved); landing post-prototype.

## 10 · Where this report goes when it's no longer needed

This is a design doc, not an audit. It lives in `reports/` until the
design is encoded in:

- `criome/ARCHITECTURE.md` (the project-wide architectural update —
  the "flow graphs are the substrate" framing belongs there once
  concretised).
- per-repo `ARCHITECTURE.md` updates in `prism` and `mentci` once they
  have implementation shape.
- code in `prism` + `mentci`.

When all three exist, this report is deleted. The design is in the
durable homes.
