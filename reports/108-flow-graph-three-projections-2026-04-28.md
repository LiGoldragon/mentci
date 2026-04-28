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
2. **`rsc` macro emission** — projects records into Rust source code that
   *is* a working ractor-based actor runtime. Not a passive data
   structure; the emitted code is the running system.
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
self-hosting loop**: `rsc` is the records-to-Rust projector that turns
the database into an executable. Its first customer is criome itself
(per `bd mentci-next-0tj`), but the same mechanism handles user-authored
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
dispatch. Schema-as-data records will be re-added when `rsc` or
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

## 4 · Projection 2 — `rsc`: records → Rust runtime (macro emission)

`rsc` reads flow-graph records from sema and emits Rust source code.
Crucially, this is **macro programming**, not naive code generation:

- The input is **structured records**, not source-text tokens.
- The emission is **template/pattern substitution per node-kind and
  edge-kind** — the templates are hand-coded inside `rsc` itself
  (one template per kind, written in Rust). When `rsc` ships, adding
  a new node-kind means adding the typed struct in signal *and* the
  emission template in rsc.
- The output is Rust source that **becomes a running actor system**
  when compiled.

So the analogy is to Rust's proc-macros — pattern in, source out — but
the input is records-from-sema rather than a `TokenStream`. That input
form is the load-bearing difference. Each record-kind defines its own
expansion shape; rsc walks the graph and emits the union.

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
2. `rsc` reads them, emits Rust, compiles via `cargo` / `crane`.
3. The new criome binary reads from sema (which contains the records
   that compiled it).
4. Editing the records → re-emit → recompile → re-deploy → criome runs
   its new shape.

This is the self-hosting "done" moment (`bd mentci-next-ef3`).

## 5 · Projection 3 — mentci UI: live visual render + edit loop

mentci's first concrete user-facing feature is the flow-graph editor.

### Render

mentci connects to `criome-daemon` over UDS. It reads flow-graph
records and renders them as a visual graph — boxes for nodes, arrows
for edges, kind-driven styling. **Real-time**: when sema changes, the
visual updates without manual refresh.

The "real-time" requirement implies subscribe-protocol support
(M2+ on criome's roadmap). Until subscribes ship, mentci can poll;
the edit loop works either way.

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
it to `criome-daemon` (path TBD: see open question 5).

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
  potentially `AtomicBatch([…])` if the user wired some initial edges
  in the same "create" gesture (see open question 11).

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
        │ nexus-daemon  │    │ rsc projects     │    │  mentci     │
        │ — text edit / │    │ records →        │    │  — visual   │
        │   inspect     │    │ Rust source via  │    │    render + │
        │   surface     │    │ macro emission   │    │    edit     │
        │   (existing)  │    │   (M1 …)         │    │  (M? …)     │
        └───────────────┘    └──────────────────┘    └─────────────┘
                                       │
                                       ▼
                              compiled actor system
                              at runtime
                              (the system the records describe)
```

## 7 · Phasing

Tentative — depends on Li's answers below.

- **M0** — nexus text shuttle, criome+nexus daemons ractor-hosted, demo
  graph operations end-to-end. **Done.**
- **M1** — `rsc` minimum: emits a known-shape Rust file from a
  known-shape record (start small, e.g. one `Node` record →
  one ractor `Actor` skeleton). Per-kind sema tables
  (`bd mentci-next-7tv`) land here as the storage shape `rsc` reads
  from.
- **M2** — `Subscribe` request shipped on criome side; mentci can
  subscribe to record changes for live updates.
- **M3** — mentci UI v0: read-only visual render of flow graphs from
  criome.
- **M4** — mentci UI v1: gesture-driven edit; `Assert` / `Mutate` /
  `Retract` round-trip with diagnostic feedback inline.
- **M5** — `rsc`'s macro projection extended to emit ractor runtime
  actor systems from flow-graph records (the "compile a graph into
  a daemon" milestone).
- **M6** — bootstrap: criome's own request-flow lives as records in
  sema; rsc emits criome from them; the loop closes
  (`bd mentci-next-zv3`, `bd mentci-next-ef3`).

## 8 · Open questions

The deep dive surfaces decisions that gate concrete design work:

1. **Which node kinds anchor the first emission?** The macro-emission
   path needs a small closed set of typed node-kind structs in signal
   for M5. `Node` / `Edge` / `Graph` carry no per-kind semantics by
   themselves. What are the first concrete kinds — Source /
   Transformer / Sink? Or domain-specific (e.g. validator-stage
   kinds for criome's own request flow)? The choice frames the
   rest of the design — both signal's new types and rsc's first
   emission templates.

2. **Smallest first demo graph.** The minimum viable end-to-end
   demonstration of the macro path. Candidate: encode criome's M0
   request flow (`Frame → Validator → Sema → Reply`) as records, have
   `rsc` emit a working daemon from them, run it, watch the integration
   test pass against the rsc-emitted binary instead of the hand-coded
   one.

3. **`rsc`'s emission shape — proc-macro, build-script, or standalone
   binary?** Three options:
   - Proc-macro reading from sema at compile time (like `sqlx::query!`
     reads schema). Tightest integration; demands proc-macro access
     to a running criome.
   - `build.rs` calling out to `rsc` to emit `.rs` files into
     `OUT_DIR` before the main compile. Decoupled from the runtime;
     the standard cargo path.
   - Standalone CLI that emits `.rs` source into a workdir; `cargo`
     builds it as a normal crate. Closest to lojix's existing shape.
   The "macro programming" framing leans toward (a); the operational
   simplicity points at (c).

4. **mentci UI tech.** Native Rust GUI (`egui`, `iced`, `gpui`)? Web
   (egui-via-wasm, htmx + SSE from a mentci-daemon, leptos)? Terminal
   (`ratatui`)? The "real-time" requirement and the gesture-driven
   editing both constrain this. Each option has very different
   implications for the project's nix-build story.

5. **mentci ↔ criome connection — direct UDS or via nexus-cli?** Two
   shapes:
   - mentci speaks signal directly over UDS, mirrors `nexus-daemon`'s
     CriomeLink. Lower latency, fewer hops, but mentci grows a signal
     speaker.
   - mentci shells out to `nexus-cli` per gesture. Reuses existing
     protocol surface, slower, requires per-gesture process spawn.
   The first shape is the natural one for real-time editing; the
   second only makes sense for one-off scripted tools.

6. **Subscribe-first vs poll-first.** mentci's UI launches before or
   after `Subscribe`? If launching pre-subscribe, mentci polls criome
   for changes (1Hz? on-demand?) and migrates to subscribes when M2
   ships.

7. **Edit-to-message translation library.** Where does the gesture
   → signal translation live? Inside mentci? In a new shared crate
   (`mentci-edit`?) consumed by both mentci and any future
   alternative UI?

8. **Diagnostic UX.** When criome rejects an edit, the
   `Diagnostic { code, message, primary_site, suggestions, … }` needs
   to land somewhere visible. Inline overlay on the rejected element?
   Side panel with history? Toast? The data model is already rich
   (machine-applicable suggestions, source spans, severity levels);
   the UX hasn't been sketched.

9. **The "main repository" — confirm `mentci`.** Speech-to-text
   couldn't transcribe the name. Working assumption: `mentci`
   (matches its long-term role per `mentci/ARCHITECTURE.md`'s "mentci
   is meant to replace the legacy software stack as the universal
   UI"). Confirm or correct.

10. **Recursive rendering — long-term.** Once `rsc` emits a runtime
    from records, can the runtime's own state be rendered as a flow
    graph in mentci? (Self-rendering of the running system.) Worth
    flagging now if it's the long-term direction — it would shape
    `Subscribe` semantics and the runtime's introspection surface.

11. **Composite-gesture atomicity.** Some user actions naturally bundle
    multiple sema mutations: "create a node and wire it to two
    existing nodes" is conceptually one intent but three signal
    messages (`Assert(Node)` + 2 × `Assert(Edge)`). Two shapes:
    - Each sub-action commits independently — the UI accumulates
      partial results; if one fails, the prior succeeded ones stay.
    - The whole gesture wraps in `AtomicBatch([…])` — all-or-nothing.
    The first shape is simpler; the second matches user mental model
    of "create *this thing*" being one step. Probably both have a
    place — the UI offers "atomic mode" via a modifier key or a
    deliberate "begin / end transaction" affordance.

12. **~~`KindDecl` — naming + role in M0.~~** **RESOLVED** — Li chose
    Path A 2026-04-28. `KindDecl` + `FieldDecl` + `Cardinality` +
    `KindDeclQuery` were dropped from signal in commit 8b101c8d.
    Schema-as-data records will be re-added when `rsc` or mentci has
    a real reader for them. The closed Rust enum in signal is the
    authoritative type system today; new kinds land by adding the
    typed struct and propagating through hand-coded dispatch.

## 9 · Where this report leaves to implementation

After Li answers the open questions, concrete work per layer:

- **`rsc`** — emission templates per node-kind / edge-kind (templates
  hand-coded in rsc); macro/templating DSL; build-system integration
  choice (per Q3).
- **`mentci`** — UI tech choice + skeleton; gesture→signal mapping;
  diagnostic surface; criome connection (per Q4–Q8).
- **`signal`** — `Subscribe` request shape (M2; design likely needs
  its own report when it lands).
- **`criome`** — per-kind sema tables (`bd mentci-next-7tv`);
  Subscribe verb; diagnostic-emission richness.

## 10 · Where this report goes when it's no longer needed

This is a design doc, not an audit. It lives in `reports/` until the
design is encoded in:

- `criome/ARCHITECTURE.md` (the project-wide architectural update —
  the "flow graphs are the substrate" framing belongs there once
  concretised).
- per-repo `ARCHITECTURE.md` updates in `rsc` and `mentci` once they
  have implementation shape.
- code in `rsc` + `mentci`.

When all three exist, this report is deleted. The design is in the
durable homes.
