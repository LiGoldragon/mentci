# 111 — First mentci-ui: introspection workbench + basic interaction

*Research report. The first incarnation of mentci is an
introspection workbench that lets a human see into criome's
running state and edit the database directly through gestures.
This revision (2026-04-29) absorbs Li's answers to the prior
draft's first seven open questions and proposes deeper ones.
Concentrates on **visuals**; code shapes belong in skeleton-as-
design once the visual answers settle.*

---

## 0 · Aim

The first mentci-ui exists for one reason: **let the human shaping
the engine see it clearly enough to participate in its design**.
Per [INTENTION](../INTENTION.md), introspection is first-class —
the surface is a peer of the engine, not a downstream consumer.

Two intertwined goals:

1. **Introspection** — every record, every change, every
   subscription, every wire frame, every diagnostic visible at the
   surface.
2. **Direct interaction** — gestures that produce signal verbs,
   validated by criome, reflected on accept.

These are inseparable: a viewer without interaction is read-only;
interaction without introspection is an admin tool. The first
mentci-ui must do both at once.

---

## 1 · Direction of intent

The shape below is constrained by seven decisions Li has made
about the surface. Each shapes the rest:

- **Subscribe is foundational, not deferred.** The first mentci-ui
  assumes Subscribe ships in the engine first; the canvas is live
  to pushed updates from the start. There is no poll-on-refresh
  fallback; the engine work for Subscribe is part of this
  milestone, not after it.
- **mentci is a family, not a thing.** The repo split is one
  mentci-lib (gesture→signal core; schema-aware; manages
  connections) plus one repo per GUI library — `mentci-egui`,
  `mentci-iced`, `mentci-flutter`, etc. Each consumes mentci-lib;
  Flutter (and other non-Rust libs) additionally consumes a
  foreign-interface bridge. The first mentci-ui picks one library;
  others follow.
- **Agents and humans use different surfaces.** Agents (LLMs,
  scripts, automations) use nexus text against criome via the
  nexus-daemon. Humans use gestures via mentci. Both reach the
  same engine; the surface is tuned to the audience.
- **Daemons compose.** mentci connects to *two* daemons: criome
  (editing, queries, subscriptions; over signal) and nexus-daemon
  (signal↔nexus rendering, used for display only). Nexus is not
  embedded; it is consulted as a rendering service.
- **Visual configuration is sema state.** Themes and layouts are
  candidate record kinds. Editing a theme is the same Assert/
  Mutate path as editing any other record; the change is itself
  visible in the wire pane.
- **Surfaces are dynamic.** Panes appear when there's something to
  show and disappear when not (diagnostics pane only when ≥1
  diagnostic exists). Some panes (wire) are user-toggled even
  when content exists.
- **No raw nexus typing.** Humans never type wire payloads by
  hand; nexus is hard for humans (struct-size and field-order
  memorisation). Editing happens through schema-aware constructor
  flows — drag-wire opens an edge-editing flow that surfaces
  RelationKind options, description fields, target picker.

---

## 2 · What must be visible

Introspection is design pressure. Categories of engine state the
surface must reveal:

- **Records.** Every record, by kind, with slot, current hash,
  display name, current revision.
- **The graph.** Flow-graph rendering when the selection is a
  Graph — Graph node, member Nodes, Edges with `RelationKind`
  visually encoded.
- **History.** The change log per slot — Assert / Mutate /
  Retract, content before/after, principal, hash transition,
  timestamp.
- **Diagnostics.** Validation rejections shown as first-class
  events.
- **The wire.** Every signal frame, both directions, at typed-
  variant level.
- **Subscriptions.** What's subscribed, what's pushing, when.
- **Connection state.** For *both* daemon connections (criome and
  nexus-daemon).
- **Cascades.** When a write triggers further changes, the cascade
  is visible — not collapsed.
- **The surface itself.** Theme, layout, pane visibility — also
  sema records, also visible, also editable through the same
  surface.

---

## 3 · The workbench

A multi-pane shell where panes appear or hide based on relevance.
Always-visible panes: Graphs nav, Canvas, Inspector. State-driven
panes: Diagnostics (when ≥1 unread diagnostic exists), Wire (user
toggle). No nexus REPL pane — humans don't author at the wire
level; constructor flows replace text input.

```
┌────────────────────────────────────────────────────────────────┐
│ [● criome  v0.1.0]  [● nexus  v0.1.0]    [⊞ wire] [⌗ themes]   │
├──────────┬─────────────────────────────────────┬───────────────┤
│          │                                     │               │
│  GRAPHS  │             CANVAS                  │  INSPECTOR    │
│          │                                     │               │
│ ▸ Echo   │      ⊙ Source ──Flow──▶ ⊡ Transf    │   slot 1042   │
│   Pipe   │            ╲                │       │   ───────     │
│ ▸ Build  │             ╲              Flow     │   kind: Node  │
│   Defs   │              Flow            │      │   name: Echo  │
│ ▸ Authz  │                ╲             ▼      │   rev:  7     │
│ ▸ Theme  │                 ▼          ⊠ Sink   │   hash: 8a3f… │
│   Layout │              ⊠ Sink                 │               │
│   ...    │                                     │  HISTORY      │
│          │                                     │  ▼ rev 7  now │
│          │                                     │  ▼ rev 6  -3m │
│ + new G  │                                     │  ▼ rev 5  -8m │
│          │                                     │               │
│          │     [pane appears below only when needed]           │
├──────────┴─────────────────────────────────────┴───────────────┤
│ ⚠ DIAGNOSTICS (2)                                       [clear]│
│ ✗ rev8 STALE_REV slot 1042 · ↳ jump                            │
│ ⚠ rev7 batch partial 2/3 · ↳ inspect                           │
│                                                                │
│ (this strip not shown when diagnostics list is empty)          │
└────────────────────────────────────────────────────────────────┘
```

When the user toggles `[⊞ wire]`, a wire strip slides up between
canvas and the diagnostics strip — same dynamics as diagnostics.
The header shows both daemon connections explicitly: criome and
nexus.

The point of the dynamic shape is that the surface adapts to
*what's true right now*. Empty panes do not waste screen real
estate; relevant panes appear when relevant.

---

## 4 · The flow-graph canvas

The centrepiece. When the selection is a Graph, the canvas
renders it with kinds, edges, and state visually encoded.

```
                    ╭───────────────╮
                    │               │
                    │   ⊙  Source   │
                    │    "ticks"    │
                    │               │
                    ╰───────┬───────╯
                            │
                          Flow
                            │
                            ▼
                    ╭───────────────╮
                    │               │
                    │   ⊡ Transf.   │
                    │    "double"   │
                    │               │
                    ╰─┬───────┬─────╯
                      │       │
                    Flow    Flow
                      │       │
                      ▼       ▼
              ╭──────────╮ ╭──────────╮
              │          │ │          │
              │ ⊠ Sink   │ │ ⊠ Sink   │
              │ "stdout" │ │ "log"    │
              │          │ │          │
              ╰──────────╯ ╰──────────╯
```

Visual encoding (interim — final palette deferred per Li Q5):

- **Glyph encodes kind.** ⊙ Source · ⊡ Transformer · ⊠ Sink ·
  ⊕ Junction · ▶ Supervisor.
- **Stroke style encodes RelationKind.** Closed-enum variants get
  consistent stroke styles; the encoding lives in mentci-lib so
  every mentci-* GUI can reproduce it.
- **Colour reserved for state.** Pending optimistic edit, stale
  (subscription push pending), rejected (failed write). Kind is
  glyph; state is colour.
- **Labels.** Display name first; slot id second on hover; hash
  never on canvas (lives in inspector).

The canvas is **always live**. Subscription pushes from criome
update the canvas in place. When a record arrives, its node
visibly transitions through "stale" to current.

---

## 5 · The inspector

Selected slot's complete state. Two stacked sections:

```
SLOT 1042                                              [▢ pin]
═══════════════════════════════════════════════════════════════
 kind:        Node
 name:        Echo
 rev:         7
 hash:        8a3f7c…
 last write:  17:23:14   (Mutate, by Li)
 referenced:  3 edges in · 1 edge out
───────────────────────────────────────────────────────────────
 [as nexus]   (Node "Echo")            ← rendered via nexus-daemon
───────────────────────────────────────────────────────────────

HISTORY (full log; scroll for older)
═══════════════════════════════════════════════════════════════

▼ rev 7  ·  17:23:14   ·  Mutate     ·  by Li
│   slot   1042
│   before Node { name: "Doubler", kind: Transformer }
│   after  Node { name: "Echo",    kind: Transformer }
│   hash   cd9e…  →  8a3f…

▼ rev 6  ·  17:20:02   ·  Assert     ·  by Li
│   created  Node { name: "Doubler", kind: Transformer }
│   hash    cd9e…
│
═══════════════════════════════════════════════════════════════
```

The "as nexus" line is rendered by querying the nexus-daemon for
the canonical text form of the current record. The same payload
is also available as a typed view; the nexus form is the agent-
oriented representation re-used for human reading.

Every history entry's arrow scrubs the canvas backward to that
point in time. History is first-class; it scrolls fully (Q8).

---

## 6 · Diagnostics surface

Validation outcomes are first-class. Every Outcome that's not Ok,
every Reply carrying a Diagnostic, lands here in chronological
order, with permanent jump-link to the slot or batch concerned.

```
DIAGNOSTICS                                          [clear all]
════════════════════════════════════════════════════════════════

✗  17:23:14   STALE_REVISION
   Mutate { slot: 1042, expected_rev: 5, actual_rev: 7 }
   suggestion: refresh slot 1042 and retry
   ↳ jump to slot 1042

⚠  17:23:08   PARTIAL_BATCH
   AtomicBatch [3 ops]   2 ok, 1 SCHEMA_FAIL on op#2
     op#2:  Assert(Edge { from: 9999, to: 1042, kind: Flow })
            slot 9999 does not exist
   ↳ inspect batch · ↳ retry without op#2

✓  17:20:02   ok
   Assert(Node { name: "Doubler", kind: Transformer })
   ↳ slot 1042
```

The pane appears only when the diagnostic list is non-empty.
When the user clears it (or all entries scroll off the retention
window), the pane vanishes and the canvas takes back the screen
real estate.

Diagnostics also overlay on the canvas at the affected node — a
red border, cleared on next successful write to the slot.
Surfacing in two places (pane + at-site) is intentional: at-site
shows *where*; pane shows *when* and *what suggestion*.

---

## 7 · Wire inspector

Every signal frame in either direction, at typed-variant level.
Toggled on/off (Li Q4). When on, frames render via nexus-daemon
for the human-readable nexus form.

```
WIRE                                          [pause] [filter…]
═══════════════════════════════════════════════════════════════

→ 17:23:08.412  req#41  to criome
                Mutate(MutOp::Node { slot:1042, expected_rev:6,
                                     new:Node{…} })
                [as nexus]   ~(Node 1042 (Node "Echo" …))

← 17:23:08.418  req#41  from criome
                Outcome(Ok)
                [as nexus]   (Ok)

→ 17:23:09.001  req#42  to criome
                Query(QueryOp::Node(NodeQuery{…}))
                [as nexus]   (| Node @name |)

← 17:23:09.004  req#42  from criome
                Records(Records::Node([…]))     [3 items]

⇣ 17:23:20.012  sub#3   from criome (push)
                Records(Records::Node([…]))     [1 item]

═══════════════════════════════════════════════════════════════
   →  request out      ←  reply in      ⇣  subscription push
```

The "as nexus" line on each frame is rendered by sending the
typed payload to nexus-daemon and showing what comes back. This
is the same path Li's eventual agents use to read the wire from
the terminal — the introspection surface re-uses the agent
surface's text rendering.

The wire pane is the strongest expression of introspection-first
in the surface. Opt-in (Q4) but always available.

---

## 8 · Schema-aware constructor flows

Direct manipulation maps to signal verbs. Every gesture opens a
*constructor flow* — a context-specific surface that knows the
schema for the verb being constructed and surfaces the right
fields with the right typed options.

The map gesture → verb (high level):

```
USER GESTURE                              SIGNAL VERB

drag-new-box (kind palette)         →     Assert(Node)
drag-wire from box A to box B       →     Assert(Edge)
select box, edit field in inspector →     Mutate(Node)
select box / wire, Backspace        →     Retract
multi-select + bulk edit            →     AtomicBatch
```

But each gesture opens a *flow* before the verb is sent:

```
DRAG-WIRE FLOW (drag from box A to box B)
═══════════════════════════════════════════════════════════════

  ⊙ Source ╌╌╌╌╌╌╌╌╌╌╌▶ ⊡ Transf.            (pending preview)
                                              wire shown dashed

  ┌──────────────────────────────────────────┐
  │  NEW EDGE                                │
  │                                          │
  │  from:        slot 1043 ("ticks")        │
  │  to:          slot 1042 ("double")       │
  │  kind:        ┌─ select ────────────────┐│
  │               │ ▸ Flow                  ││
  │               │   DependsOn             ││
  │               │   Contains              ││
  │               │   References            ││
  │               │   Produces              ││
  │               │   Consumes              ││
  │               │   Calls                 ││
  │               │   Implements            ││
  │               │   IsA                   ││
  │               └─────────────────────────┘│
  │  description: [_________________________]│
  │                                          │
  │              [cancel]  [commit]          │
  └──────────────────────────────────────────┘

   ↑                                       ↑
   schema knowledge from mentci-lib        commit sends
                                           Assert(Edge {…})
```

Constructor-flow principles:

- **Pre-show but uncommitted.** The wire appears visually as soon
  as the drag completes (dashed, pending colour). Nothing leaves
  the wire until the user clicks commit. The pending preview is
  *intent*, not state.
- **Schema knowledge lives in mentci-lib.** When mentci-lib knows
  Edge has a `kind: RelationKind` field, the constructor surfaces
  RelationKind variants. Adding a new variant in `signal/flow.rs`
  reaches the GUI through mentci-lib, not by hand-editing the GUI.
- **Commit at gesture-end.** No optimistic UI. The canvas reflects
  criome's accept; if criome rejects, the pending wire vanishes
  and a diagnostic appears.
- **Equivalence with the agent path.** Whatever an agent could
  send via nexus, a human can build via gestures. The two paths
  converge at the same signal verb.

Other flows (drag-new-box, rename, mutate-field, batch-edit)
follow the same constructor-flow pattern with verb-appropriate
fields.

---

## 9 · Connection topology — two daemons

mentci connects to two daemons. The header shows both
explicitly.

```
                           ┌──────────────┐
                           │   mentci-*   │
                           │     GUI      │
                           └──────┬───────┘
                                  │ uses
                                  ▼
                           ┌──────────────┐
                           │  mentci-lib  │
                           │  (manages    │
                           │   both       │
                           │   connections)│
                           └──┬────────┬──┘
                              │        │
                signal        │        │  signal
              (editing,       │        │  (signal↔nexus
               queries,       │        │   rendering;
               subscribe)     │        │   parsing if
                              │        │   ever needed)
                              ▼        ▼
                       ┌──────────┐ ┌──────────────┐
                       │  criome  │ │ nexus-daemon │
                       └──────────┘ └──────────────┘

  Two independent connections. Each has its own handshake,
  protocol version, lifecycle state. The header status bar
  shows both. Either can be down independently:
   • nexus down, criome up  → labels render as raw typed
                              payloads instead of nexus text;
                              rest of surface works.
   • criome down, nexus up  → no editing, no canvas updates;
                              wire pane retention persists;
                              user reconnects deliberately.
```

mentci-lib owns both connections. The GUI sees a unified
"engine" surface; the dual-daemon split is hidden from the
widget code (and revealed in the header for the introspecting
human).

This composition replaces "embed nexus in mentci." nexus-daemon
is consulted as a rendering service. The same service the
agent's nexus-cli talks to.

---

## 10 · GUI library — the first mentci-ui

Per Q2, the first mentci-ui is one repo named after its GUI
library. Linux + Mac are first-class; both are developer OSes.
Survey of candidates by what the introspection workbench needs:

| Library | Lang | Linux | Mac | Custom canvas | Live updates fit | Maturity for graph editors |
|---|---|---|---|---|---|---|
| **egui** | Rust | ● | ● | ● strong | ● immediate-mode = natural fit | ● strong (`egui_node_graph`, `rerun`) |
| iced | Rust | ● | ● | ◐ ok | ◐ Elm-architecture; structured | ◐ moderate |
| gpui | Rust | ◐ | ● | ● strong | ● strong | ◐ small ecosystem |
| slint | Rust | ● | ● | ◐ declarative-leaning | ● | ✗ not its niche |
| dioxus desktop | Rust | ● | ● | ◐ via `rsx!` | ● | ✗ not its niche |
| Flutter | Dart | ● | ● | ● strong | ● | ◐ moderate (foreign-interface tax) |
| Qt + cxx-qt | C++/Rust | ● | ● | ● strong | ● | ● strong but heavy |
| Tauri | Web/Rust | ● | ● | ● via web | ● | ◐ JS-side complexity |

What the workbench specifically needs:

- Custom rendering for the canvas (graph drawing, dynamic edges,
  state-coloured nodes).
- Many dynamically appearing/disappearing panes.
- Live-update friendliness — when a subscription push arrives,
  the affected views must refresh without ceremony.
- Detail-heavy text views (inspector, wire pane).
- Linux + Mac equally first-class; native feel desirable but not
  the highest priority on either.

**Recommendation: egui.** Cited principles:

- **Clarity (INTENTION priority 1).** egui's immediate-mode model
  reads cleanly in code that has to redraw on every subscription
  push — render-from-current-state matches the surface's logical
  shape exactly.
- **Introspection (INTENTION priority 3).** egui's debug overlay,
  inspection mode, and built-in performance instrumentation are
  themselves introspectable from the running app — the surface is
  inspectable at the GUI-toolkit level too.
- **Strong fit for the canvas's specific shape.** Custom node-graph
  editors are an established egui pattern (`egui_node_graph`);
  Rerun's visualization workbench is the closest existing system
  to the introspection workbench above and is built on egui.
- **No foreign-interface bridge needed.** Rust-native; mentci-lib
  consumed directly.
- **Both platforms first-class** without extra ceremony.

What egui gives up: native widget feel, declarative model. Both
are recoverable in later mentci-* family members (mentci-iced for
Elm-arch, mentci-flutter for native polish).

The first mentci-ui is therefore **`mentci-egui`**. Other family
members follow as the family is exercised; the architecture
treats all of them as peers atop mentci-lib.

---

## 11 · What this asks of the engine

Introspection and the dual-daemon composition shape the engine.

- **Subscribe must ship as part of this milestone.** The canvas's
  always-live property assumes Subscribe; without it, the surface
  cannot uphold push-not-pull. The engine work for Subscribe is
  scope here, not deferred.
- **State must be representable, not just queryable.** Every
  record kind needs a canonical visual rendering.
- **Diagnostics carry structured suggestions.** The diagnostics
  pane displays `suggestion` directly; criome must populate it
  with actionable structured data, not strings.
- **Wire frames are inspectable typed payloads end-to-end.** No
  string-tagged variants, no opaque blobs.
- **Subscriptions push whole records.** Diff reconstruction is
  fragile; full records on push.
- **Cascades are observable.** When write A triggers derivation
  B, both are visible; not a single collapsed event.
- **Time is queryable.** History scrubber implies point-in-time
  reads against sema's bitemporal index.
- **Nexus rendering is a service, not a library.** The
  nexus-daemon must accept "render this signal payload as nexus
  text" and "parse this nexus text to a signal verb" — the second
  for completeness even though humans don't drive that path.
- **Visual configuration as records is welcome.** Theme and
  layout record kinds land in signal alongside Graph/Node/Edge.
  The engine introspects its own surface configuration.
- **Schema-as-data feeds constructor flows.** mentci-lib's
  schema-awareness comes from somewhere — either compile-time
  codegen from signal types, or runtime-readable schema records.
  The choice is a deeper question (§12 Q3).

The deepest ask: **nothing the engine does is hidden from the
human shaping it**, and **the surface is itself part of the
engine's introspectable state**.

---

## 12 · Open questions, deeper

The questions Li answered have settled the shape; these are the
deeper ones the new shape opens. Each cites the principle that
frames it.

### On the family

| # | Question | Principle |
|---|---|---|
| Q1 | mentci-lib's API contract: what surface does it expose to mentci-egui, mentci-iced, mentci-flutter such that all three render the *same* introspection workbench? Data + commands + subscription stream? Trait-based widget contract? Side-effect channel? | Clarity. The contract must be small enough that a Flutter-side foreign-interface bridge is straightforward. |
| Q2 | Do all mentci-* family members render the same workbench layout, or does each use its native idiom? | Introspection. If layout is records, every family member reads the same layout records and produces the same shape. If layout is per-implementation, family members diverge. The recursive answer (layout-as-records) ties Q2 to Q12. |
| Q3 | Schema knowledge for constructor flows: compile-time codegen from signal Rust types, or runtime-readable schema records sema can produce? The latter makes the schema *itself* introspectable; the former is simpler. | Introspection. Sema-readable schema records mean the engine reveals not just its data but its data shapes — agents can see what kinds exist by querying. Compile-time codegen hides the schema in the binaries. |
| Q4 | Cross-implementation testing: how do we verify mentci-egui and mentci-flutter produce the same outcomes for the same gestures? | Correctness. The contract's correctness is the family's correctness; if one implementation drifts, the family fragments. |

### On the dual-daemon composition

| # | Question | Principle |
|---|---|---|
| Q5 | Is the nexus-daemon connection persistent (one connection per mentci session) or ephemeral (per-render request)? Per-render is simpler; persistent enables Subscribe-shaped pushes for nexus rendering changes (e.g., grammar updates). | Push, never pull. Persistent fits the discipline; ephemeral creates a request-response pattern that's slightly off-shape for a workbench. |
| Q6 | When nexus-daemon is down but criome is up, the wire pane and inspector cannot render to nexus text. Do they fall back to typed-payload rendering (a different visual form), or hide the nexus-form line until nexus is back? | Introspection. Hiding is the wrong move (it hides that nexus is down). Falling back makes both states visible. |
| Q7 | When criome is down but nexus is up, what does the surface do? Editing is impossible; introspection of cached state is partial. Does the surface freeze, or enter a degraded read-only mode showing last-known state with explicit staleness? | Introspection. Stale-with-staleness-named is honest; freeze hides what we know. |
| Q8 | Does the agent surface (nexus-cli + LLM tools) also use nexus-daemon for *its* rendering, or does the agent surface render text directly? | Components per function. One rendering service serving all consumers is cleaner than per-consumer rendering. Implies nexus-daemon is the canonical signal↔text codec for everyone. |

### On schema-aware constructors

| # | Question | Principle |
|---|---|---|
| Q9 | When the user drags a wire and the kind selector appears, does it list all `RelationKind` variants always, or only those *valid* for the source/target node-kinds (some relations make sense only for certain pairs)? Validity = compile-time-typed at the wire? Runtime-validated by criome? | Perfect specificity. If only some pairs are valid, the schema should say so; the constructor flow then surfaces only valid options. The engine must have the validity rules as records or in code. |
| Q10 | When schema evolves (a new node-kind ships in signal), does mentci-egui auto-discover and surface it on the kind palette, or does mentci-egui require an update? Implies Q3's answer determines this. | Introspection. Auto-discovery means schema-as-records (Q3); explicit update means compile-time. |
| Q11 | Constructor flows for verbs other than Assert/Edge — Mutate's expected_rev field, Retract's confirmation, AtomicBatch's composition. Each verb has its own flow shape. Do all flows live in mentci-lib (single source of UI behaviour) or are they reimplemented per GUI library? | Components per function. Centralising in mentci-lib means the family converges; per-GUI flows lets each library use idiomatic interactions but risks divergence. |

### On themes-and-layouts-as-records

| # | Question | Principle |
|---|---|---|
| Q12 | What record kinds capture UI configuration? Candidates: `Theme`, `Layout`, `PaneVisibility`, `KeybindMap`. What's the granularity — one big `WorkbenchConfig` or many small kinds? | Perfect specificity. Many small kinds match the engine's existing pattern (one kind per concept). |
| Q13 | When the user opens mentci-egui for the first time (no theme record yet), what shows? A built-in default that runs without sema state? A genesis-style theme record that ships with the binary and gets asserted on first connect? | Bootstrap discipline. Self-bootstrapping (no sema state needed to render anything) is simpler; genesis-asserts-on-first-connect is recursively introspectable from the start. |
| Q14 | Are themes per-user or global to a sema instance? If per-user, identity records become load-bearing — and identity is currently underspecified (capability tokens reference principal, but no Principal record kind exists). | Introspection. Per-user means the agent shaping the engine sees their own surface configured; global means everyone shares. The right answer depends on whether sema has a notion of "user" yet. |
| Q15 | Do themes describe *intent* (semantic colours: "selected", "stale", "rejected") that each GUI library renders in its native palette, or *appearance* (RGB values) that every GUI follows literally? | Clarity. Intent is portable across mentci-egui / mentci-flutter; appearance is more direct but couples the theme to a particular library's rendering. |

### On concurrent agents

| # | Question | Principle |
|---|---|---|
| Q16 | Two agents (one human in mentci-egui, one LLM via nexus-cli) connected to the same criome at the same time. Do they see each other's edits live (via subscriptions)? Yes, by Subscribe semantics — but does mentci-egui surface "another connection just made an edit"? | Introspection. The fact that another agent is acting *is* engine state; surfacing it makes coordination possible. Hiding it makes concurrent activity surprising. |
| Q17 | Wire pane: does it show frames from *only this connection*, or *all connections* the daemon is serving? "Only this" is the obvious default; "all" treats the wire pane as a true engine-wide observability surface. | Introspection. The deeper answer is "all" — the human shaping the engine wants to see what every agent is doing. But "all" implies criome exposes a wire-tap subscription, which it doesn't today. |
| Q18 | When a concurrent agent edits a node the human is in the middle of editing, what happens to the human's pending constructor flow? Forced cancel? Inline diff with prompt? Optimistic continuation? | Accept-and-reflect. The engine's truth wins; the pending flow must surface the conflict and let the user re-confirm. |

### On the canvas

| # | Question | Principle |
|---|---|---|
| Q19 | Node positions on the canvas: stored as records (so layout is sema state, shared across mentci-* family members) or stored locally (per-client, possibly per-user)? Layout-as-records ties to Q12. | Same as Q12. Records are introspectable and shared; local is faster to set up and per-agent. |
| Q20 | Large graphs (hundreds, thousands of nodes): how does the canvas degrade gracefully? Auto-layout? Mini-map? Level-of-detail collapse? Filtered subgraphs? | Clarity. The canvas must remain readable; an unreadable canvas is hidden state. |
| Q21 | When a Graph contains another Graph (Contains edge), does the canvas drill in (replacing the view), expand inline (nested boxes), or both (toggle)? | Introspection. Both is most flexible; pick a default. |

### Recursive

| # | Question | Principle |
|---|---|---|
| Q22 | If every UI surface element (theme, layout, kind palette) is a record, is the *agent* themselves a record? Identity, session state, capabilities. This connects to the authz model but raises the question now: what does the surface show about *who is acting*? | Introspection. The deepest reveal: the engine knows who is shaping it. The current architecture has capability tokens reference a principal but no Principal record kind. |
| Q23 | Is mentci-egui itself eventually a Graph (records → Rust source via prism, building this very binary)? If so, the first mentci-ui's running form *is* a sema record, asserting itself on first run. | Self-host. The endgame answer is yes; for the first mentci-ui, almost certainly no — but the surface should not preclude it. |

---

## 13 · What is not in this report

- **Visual aesthetics.** Per Li Q5: obvious choices for now;
  rich palette comes later, possibly via theme-as-records (Q12).
- **The mentci-lib API surface in code.** Skeleton-as-design
  work belongs in mentci-lib's own ARCHITECTURE.md once Q1 settles.
- **Mobile / alternative form-factors.** Desktop workbench first;
  alternatives follow once the first surface is right.
- **The eventual universal-UI scope.** This is the introspection
  workbench, the surface that begins earning the wider scope.

---

## 14 · Lifetime

This report lives until:

- The §12 questions are answered (or explicitly deferred with a
  stated reason).
- The shape is encoded in `mentci-lib`'s skeleton-as-design and
  `mentci-egui`'s first scaffolding.
- The first running mentci-egui shows records on the canvas,
  accepts a constructor-flow gesture, and shows a diagnostic
  served from criome.

When those exist, this report is deleted; its content has moved
into the implementation it described.

---

*End report 111.*
