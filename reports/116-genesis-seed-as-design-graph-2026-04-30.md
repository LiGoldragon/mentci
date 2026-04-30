# 116 — Genesis seed: the project's own design, as a flow-graph

*Answer to Q11-the-rest from [reports/114](114-mentci-stack-supervisor-draft-2026-04-30.md):
the seed shouldn't be toy data — it's the architectural map of the
project itself, so mentci-ui's first frame on a fresh install paints
the design and the user shapes the engine from inside the engine.
Lifetime: until the seed lands as a real `genesis.nexus` file in the
mentci repo and this report folds into mentci's `ARCHITECTURE.md` or
gets deleted.*

---

## 0 · The shape in one picture

A single Graph titled **Criome** holds one Node per canonical
component and one Edge per architectural relationship between them.
process-manager pipes this through nexus-cli on first run; criome
asserts every record; mentci-egui paints the result.

```
                          ┌─── Graph: "Criome" ───┐
                          │   nodes: [...]        │
                          │   edges: [...]        │
                          │   subgraphs: []       │
                          └───────────┬───────────┘
                                      │ Contains (Graph→Node membership)
              ┌────────────┬──────────┼──────────┬─────────────┐
              ▼            ▼          ▼          ▼             ▼
          ┌──────┐    ┌──────┐   ┌──────┐   ┌────────┐   ┌──────────┐
          │ sema │    │criome│   │signal│   │ nexus  │   │mentci-egui│   …
          └──────┘    └──────┘   └──────┘   └────────┘   └──────────┘
                       (etc — every CANON component is a Node;
                        every architectural relation is an Edge
                        between two Nodes)
```

Adding to the design from inside mentci-ui then becomes the natural
loop: drag a new Node onto the canvas → it asserts as a `Node`
record → drag a wire to it → asserts an `Edge` of the chosen
RelationKind → the design grows.

---

## 1 · Why this is the right seed

| placeholder seed (what 114 §4.2.4 sketched) | design seed (this report) |
|---|---|
| "Echo Pipeline" / "ticks" / "double" / "stdout" | "Criome" / "sema" / "criome" / "nexus" / … |
| toy data, doesn't connect to the real work | the actual architectural map; what the user came here to think about |
| user's first impression: a demo | user's first impression: the project itself |
| no path from "look at it" to "edit it" | the design graph is *editable* in mentci-ui; new canonical components land as new Nodes, new dependencies as new Edges |
| seeds the cache; that's it | seeds the cache *and* dogfoods the design loop |

The placeholder was a way to put records into sema so the canvas
wasn't blank. The design seed makes the canvas *useful* on first
view — it shows the thing the user is here to build, in the form
the user will be editing it.

---

## 2 · The component Nodes

Every CANON repo from [`docs/workspace-manifest.md`](../docs/workspace-manifest.md)
becomes a Node. Provisional Node-name table:

```
   Name              Role
   ─────────────────────────────────────────────────────────────────────
   sema              records database (redb)
   criome            state-engine around sema
   signal            wire protocol (every leg)
   signal-forge      criome ↔ forge wire (effect-bearing verbs)
   signal-arca       writers ↔ arca-daemon wire (deposit verbs)
   nota              text-grammar specification
   nota-codec        codec runtime (encode/decode)
   nota-derive       codec proc-macros
   nexus             nexus-grammar daemon (text ↔ signal)
   nexus-cli         text-mode client
   forge             build/deploy executor (links prism, runs nix)
   arca              content-addressed filesystem
   arca-daemon       privileged store writer
   prism             records → .rs projector
   mentci-lib        heavy MVU library (any GUI shell consumes)
   mentci-egui       first GUI shell (egui)
   mentci            workspace umbrella + meta-deploy aggregator
   process-manager   the supervisor (per reports/114)
   signal-derive     schema descriptors (per reports/115)
```

19 Nodes. Each is one `(Assert (Node "<name>"))` in `genesis.nexus`.

Future kinds (Source/Transformer/Sink/Junction/Supervisor —
see [criome ARCH §11 "Open shapes"](../repos/criome/ARCHITECTURE.md#11--open-shapes))
add a `kind` field per Node; until that lands, every Node here is
"a thing the project contains," and visual differentiation comes
from the RelationKind of the Edges around it.

---

## 3 · The architectural Edges

The relationships, grouped by RelationKind. Every row is one Edge
record asserted in `genesis.nexus`.

```
   Contains  (composition; from contains to)
   ─────────────────────────────────────────────────────
   criome           contains  sema
   arca-daemon      contains  arca
   mentci           contains  every other component  (workspace umbrella)

   DependsOn  (compile / runtime dependency)
   ─────────────────────────────────────────────────────
   criome           depends-on  signal              (speaks signal)
   nexus            depends-on  signal
   nexus            depends-on  criome              (forwards requests)
   nexus-cli        depends-on  nexus               (uses the grammar)
   forge            depends-on  signal-forge
   forge            depends-on  signal-arca         (deposits to arca-daemon)
   arca-daemon      depends-on  signal-arca
   mentci-lib       depends-on  signal
   mentci-egui      depends-on  mentci-lib
   process-manager  depends-on  signal              (control-message protocol)
   nota-codec       depends-on  nota                (implements the grammar)
   nota-derive      depends-on  nota-codec          (emits codec impls)
   signal           depends-on  nota-codec          (records derive NotaRecord)
   signal           depends-on  nota-derive
   signal-forge     depends-on  signal              (extends the envelope)
   signal-arca      depends-on  signal
   signal-derive    depends-on  signal              (emits schema over types)

   Calls  (forge links prism; analogous to "invokes")
   ─────────────────────────────────────────────────────
   forge            calls  prism                    (links the lib)

   References  (knows about, supervises — placeholder until
                Supervises lands per criome ARCH §11)
   ─────────────────────────────────────────────────────
   process-manager  references  criome
   process-manager  references  nexus
   process-manager  references  forge
   process-manager  references  arca-daemon
   process-manager  references  mentci-egui

   Produces  (X produces Y as an artefact)
   ─────────────────────────────────────────────────────
   prism            produces  signal                (records → .rs;
                                                     .rs files compile
                                                     to signal-derive's
                                                     emission target)
                                                     (this edge is
                                                     marginal — see §4)
```

~28 Edges total in the first cut. The shape is honestly recursive:
`signal-derive` is itself a component this design graph is about,
and the design graph itself is a `Graph` record stored in sema —
which is the whole point of the dogfooding.

---

## 4 · Open shapes

A short list of edges where the right RelationKind isn't obviously
in the current closed enum, plus naming questions.

| open shape | the question |
|---|---|
| `process-manager → criome` (and the other supervised daemons) | `Supervises` is named in [criome ARCH §11](../repos/criome/ARCHITECTURE.md#11--open-shapes) as a future RelationKind variant. Use `References` until it lands; flip to `Supervises` when added. |
| `prism → signal` ("produces .rs from signal records") | `Produces` is a stretch — what prism actually produces is *Rust source text*, which then compiles to artefacts that are *consumers* of signal, not signal itself. Drop this edge from the first seed and revisit. |
| `mentci → every component` (Contains as workspace umbrella) | This explodes the edge count. Alternative: omit; the *fact* that mentci is the workspace is implicit in the Graph being titled "Criome" and living in mentci's repo. Or: include as documentation. |
| Graph title "Criome" vs "Sema Ecosystem" vs "Mentci Workspace" | Three reasonable names. "Criome" matches [criome ARCH §0](../repos/criome/ARCHITECTURE.md#0--tldr) — the project being built is criome. "Mentci Workspace" matches the assemblage framing. Pick one. |
| Whether to include shelved/transitional repos (lojix-cli, arbor, signal in transitional flux) | Default: include only CANON. Transitional/shelved earn their place as records when their status changes. |

---

## 5 · The slot reservation

Per [criome ARCH §10 "Bootstrap rung by rung"](../repos/criome/ARCHITECTURE.md#10--rules)
the genesis seed reserves slots `[0, 1024)`. A first-pass
allocation:

```
   Slot range     Purpose
   ─────────────────────────────────────────────────────
   0              the default Principal ("operator")
   1              the design Graph ("Criome")
   16 .. 63       component Nodes (≤ 48; current count: 19)
   64 .. 255      architectural Edges (≤ 192; current count: ~28)
   256 .. 1023    headroom for the design's growth before
                  the reserved range is exhausted; subsequent
                  user-asserted records start at 1024
```

Lots of headroom. The design can roughly quintuple in size before
it bumps the range.

Hard-coded slot values in `genesis.nexus` let Edges reference Nodes
by slot without needing bind-on-Assert in the nexus parser. (See
the bind-on-Assert thread in [reports/114 §10.1](114-mentci-stack-supervisor-draft-2026-04-30.md#101--resolved-by-the-principles)
Q11 row — option (b) — which this seed exercises.)

---

## 6 · The recursion

The design graph is *itself* a record in sema. The first time the
user adds a new canonical component to the workspace (via the
constructor flow in mentci-ui), the design graph grows by one
Node. The first time a new architectural dependency lands (via the
drag-wire flow), the design graph grows by one Edge.

The project's design becomes the artefact the project edits, in
the engine the project is. That is the whole shape — and the
genesis seed is what bootstraps it.

---

*End report 116.*
