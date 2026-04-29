# 110 — System architecture at the end-to-end build milestone

*Forward-looking snapshot. Captures the architectural picture at
the milestone when **a user can author a flow graph as records
in sema, issue a `BuildRequest`, and receive a working compiled
binary** referenced from sema by hash. Lifetime: lives until the
described shape is encoded in the canonical docs + skeleton-as-
design code, then deleted. Created 2026-04-29 to verify the
agent's mental model.*

## 0 · TL;DR

The milestone described here is **M5 / end-to-end build** — the
first time the project's central thesis is demonstrated end to
end: records → working actor runtime. Concretely:

- `signal::BuildRequest` verb shipped (the new request criome
  accepts/denies and forwards).
- `criome` validates + forwards records to `forge` as a signal
  verb. **criome itself runs nothing.**
- `forge` links `prism` and runs the full pipeline internally:
  prism emits `.rs` → workdir assembly → `nix build` → bundle
  into arca.
- `CompiledBinary` record asserted to sema; reply chain to the
  client.

mentci's GUI editor (M3-M4 / parallel track) may or may not be
present at this milestone — this report covers the back-end
through-line independently.

---

## 1 · Component map — the three clusters

```
                       ╔═══════════════════════════╗
                       ║      STATE CLUSTER        ║
                       ║                           ║
                       ║   ┌───────────────────┐   ║
                       ║   │      criome       │   ║
                       ║   │  (state-engine)   │   ║
                       ║   │                   │   ║
                       ║   │  validates ·      │   ║
                       ║   │  forwards ·       │   ║
                       ║   │  persists         │   ║
                       ║   │                   │   ║
                       ║   │  runs nothing     │   ║
                       ║   └────────┬──────────┘   ║
                       ║            │ writes/reads ║
                       ║            ▼              ║
                       ║   ┌───────────────────┐   ║
                       ║   │       sema        │   ║
                       ║   │    (database)     │   ║
                       ║   │       redb        │   ║
                       ║   └───────────────────┘   ║
                       ╚═══════════╤═══════════════╝
                                   │
                                   │ signal (rkyv)
                                   │ over UDS
                                   │
                ┌──────────────────┼─────────────────┐
                │                  │                 │
        ╔═══════▼════════╗   ╔═════▼═════════╗   ┌───▼─────────┐
        ║   FRONT-ENDS   ║   ║  EXECUTOR     ║   │ direct      │
        ║                ║   ║   CLUSTER     ║   │ signal      │
        ║                ║   ║               ║   │ speakers    │
        ║  nexus daemon  ║   ║  ┌─────────┐  ║   │             │
        ║   (text↔sig)   ║   ║  │ forge   │  ║   │ agents,     │
        ║       ▲        ║   ║  │ daemon  │  ║   │ scripts,    │
        ║       │ text   ║   ║  │         │  ║   │ workspace   │
        ║       ▼        ║   ║  │ links   │  ║   │ tools       │
        ║  nexus-cli     ║   ║  │ prism   │  ║   └─────────────┘
        ║                ║   ║  │ runs nix│  ║
        ║                ║   ║  │ bundles │  ║
        ║  GUI repo      ║   ║  │         │  ║
        ║   (egui)       ║   ║  └────┬────┘  ║
        ║       ▲        ║   ║       │       ║
        ║       │ uses   ║   ║       │       ║
        ║       ▼        ║   ║       │ writes║
        ║  mentci-lib    ║   ║       ▼       ║
        ║  (gesture→sig) ║   ║  ┌─────────┐  ║
        ║                ║   ║  │ forge-  │  ║
        ║                ║   ║  │  store  │  ║
        ║  + future      ║   ║  │  (FS,   │  ║
        ║    mobile/alt  ║   ║  │   redb  │  ║
        ║    UIs         ║   ║  │   index)│  ║
        ╚════════════════╝   ║  └─────────┘  ║
                             ╚═══════════════╝

      ┌── wire-type crate ───┐    ┌── library crates ──┐
      │      signal          │    │       prism        │
      │   (every wire —      │    │  (records → Rust)  │
      │    front-ends ↔      │    │  linked by forge   │
      │    criome and        │    │                    │
      │    criome ↔ forge)   │    │     mentci-lib     │
      │                      │    │  (gesture→signal)  │
      │   nota / nota-codec  │    │  linked by GUI     │
      │   nota-derive        │    │  + alt UIs         │
      │   (text codec for    │    └────────────────────┘
      │    nexus dialect)    │
      └──────────────────────┘   ┌── workspace ────────┐
                                 │       mentci        │
                                 │     (umbrella)      │
                                 │                     │
                                 │  tools-documenta-   │
                                 │  tion (cross-       │
                                 │  project rules)     │
                                 └─────────────────────┘
```

Three clusters communicate via typed protocols. The crates that
are *only types* (no runtime) sit underneath, consumed by
multiple participants.

---

## 2 · Component roles (terse)

| Component | Role | What it depends on |
|---|---|---|
| **sema** | the database — records' home (redb) | nothing |
| **criome** | the state-engine — validates, persists, forwards. Runs nothing. | sema, signal |
| **signal** | the workspace's typed wire protocol — Frame envelope + handshake + auth + records + front-end verbs (rkyv types only) | nota-codec, rkyv |
| **signal-forge** | layered atop signal — carries the criome ↔ forge wire (Build, Deploy, store-entry operations); compile-time isolation from front-ends | signal |
| **nexus daemon** | text ↔ signal gateway | signal, nota-codec |
| **nexus-cli** | thin text client | (UDS to nexus daemon) |
| **forge daemon** | executor — links prism, runs nix, bundles | signal, prism, arca |
| **arca** | content-addressed artifact filesystem + redb index | redb |
| **prism** | library: records → Rust source | signal (record types) |
| **mentci-lib** | library: gesture → signal envelope, criome connection | signal |
| **GUI repo** | egui flow-graph editor | mentci-lib, egui |
| **nota / nota-codec / nota-derive** | text codec stack for nexus dialect | rkyv |
| **lojix-cli** | deploy CLI (transitioning to thin signal-speaking client of forge) | signal |
| **mentci** | workspace umbrella — design corpus, agent rules, dev shell | (workspace-only) |
| **tools-documentation** | cross-project rules + tool docs | (no runtime) |

---

## 3 · Wire protocol — `signal` (criome's protocol)

Every signal-speaking client (nexus daemon, mentci-lib through
GUI, agents, scripts) sends `signal::Request` over UDS to criome
and receives `signal::Reply`.

```
signal::Request
│
├─ Handshake(HandshakeRequest)        ── must be first on the connection
│
├── EDIT (mutating) ──
├─ Assert(AssertOperation)            ── introduce a new record
├─ Mutate(MutateOperation)            ── replace existing record at slot
├─ Retract(RetractOperation)          ── remove record at slot
├─ AtomicBatch(AtomicBatch)           ── all-or-nothing sequence
│
├── READ ──
├─ Query(QueryOperation)              ── one-shot read
├─ Subscribe(QueryOperation)          ── push-subscription [M2+]
│
├── DRY-RUN ──
├─ Validate(ValidateOperation)        ── would-be outcome without commit
│
└── DISPATCH ──
   └─ BuildRequest(BuildRequestOp)    ── compile a graph [NEW @ M5]


signal::Reply
│
├─ HandshakeAccepted(HandshakeReply)
├─ HandshakeRejected(HandshakeRejectionReason)
│
├── EDIT/DISPATCH replies ──
├─ Outcome(OutcomeMessage)            ── one OutcomeMessage per edit
├─ Outcomes(Vec<OutcomeMessage>)      ── per-position for batches
│
└── QUERY reply ──
   └─ Records(Records)                ── typed per-kind result
                                          (Records::Node(Vec<Node>) etc.)


OutcomeMessage
├─ Ok(Ok)                             ── success acknowledgement
└─ Diagnostic(Diagnostic)             ── code, message, site, suggestions


BuildRequestOp     [NEW]
└─ target: Slot                       ── refers to a Graph record in sema
   (further fields TBD: nix-attr override, target-platform, etc.)
```

**Perfect specificity.** Each verb's payload is its own typed
enum naming the kinds it operates on (`AssertOperation::Node` /
`...::Edge` / `...::Graph`). No generic record wrapper.

---

## 4 · The criome → forge leg (also signal)

**signal is the workspace's only wire protocol.** The
criome→forge leg uses signal too — same envelope, same
handshake, same rkyv framing as front-end → criome. forge
accepts the effect-bearing subset of signal verbs; rejects
the others.

```
effect-bearing signal verbs (criome → forge)
│
├─ Build { graph, nodes, edges, ... }   ── records → CompiledBinary
│                                          (forge runs prism +
│                                           workdir + nix + bundle
│                                           internally)
│
├─ Deploy { host, mode, ... }           ── nixos-rebuild
│
└─ store-entry operations               ── get / put / materialize
                                           / delete (some shipped at
                                           build time; full set lands
                                           with arca reader/
                                           writer bodies)


reply payloads (forge → criome)
│
├─ BuildOk { store_entry_hash, narhash, wall_ms }
├─ DeployOk { generation, wall_ms }
└─ Failed { code, message }
```

**criome does NOT run prism, NOT write files, NOT spawn nix.**
forge owns all of that. criome's role on this leg: forward the
records-bundled signal verb, await the typed reply, assert a
`CompiledBinary` (or `Deployed`, etc.) record back to sema.

The exact field shapes for `Build`'s payload settle when
forge-daemon is wired; what's locked is **the protocol is
signal**.

---

## 5 · Library API surfaces

```
prism (linked by forge daemon)
─────────────────────────────────────────────────────
INPUT:   FlowGraphSnapshot {
           graph: &Graph,
           nodes: &[Node],
           edges: &[Edge],
         }                                — signal types

OUTPUT:  Emission {
           files: Vec<EmittedFile>,       — full set of .rs source
         }

         EmittedFile {
           path: PathBuf,                 — relative to workdir root
           contents: String,
         }

TEMPLATES (one per node-kind, hand-coded in prism):
  Source       ─→ ractor Actor with external-boundary State
  Transformer  ─→ ractor Actor with 1→1 message handler
  Sink         ─→ ractor Actor with consumer State
  Junction     ─→ ractor Actor with multi-port (Merge/Broadcast/Balance/Zip)
  Supervisor   ─→ ractor Actor whose handle_supervisor_evt does the work

TEMPLATES (one per edge RelationKind):
  Flow / DependsOn / Calls / Implements / Contains / References / ...
  ─→ each determines the wire shape between the two actors


mentci-lib (linked by GUI repo + future alt UIs)
─────────────────────────────────────────────────────
INPUT:   user gestures (typed events)
           ── click / drag / keyboard / commit-on-enter

OUTPUT:  signal::Request envelopes, sent over UDS to criome
         + criome connection management (UDS, handshake, framing)
         + reply demux: per-gesture diagnostic surface

         CriomeLink {
           connect(socket_path) -> Self,
           send(Request) -> Future<Reply>,
           subscribe(QueryOperation) -> Stream<Records>,
         }

GESTURE → SIGNAL MAPPING:
  drag-new-box  ─→ Assert(Node)
  drag-wire     ─→ Assert(Edge)
  delete-box    ─→ Retract(...)
  rename-box    ─→ Mutate(Node { slot, new, expected_rev })
  bulk-edit     ─→ AtomicBatch([...])      ── per Q11 RESOLVED


signal (no runtime — types only)
─────────────────────────────────────────────────────
Re-exported by: nexus daemon, criome, mentci-lib, agents,
                forge (decodes records and effect-bearing
                verbs), lojix-cli

Carries: Frame envelope + Request/Reply types + record kinds
         (Node, Edge, Graph, RelationKind, Slot, ...) +
         Diagnostic + handshake + auth (BLS G1 capability
         tokens) + effect-bearing verbs (Build, Deploy, store
         operations)

Wire: rkyv 0.8 portable feature set
      (std + bytecheck + little_endian + pointer_width_32 + unaligned)
```

---

## 6 · Flow — Edit (existing M0)

```
USER          NEXUS-CLI       NEXUS DAEMON       CRIOME            SEMA
 │               │                 │                │                │
 │ (Assert       │                 │                │                │
 │   (Node "X")) │                 │                │                │
 │ ── text ─────▶│                 │                │                │
 │               │ ── UDS text ───▶│                │                │
 │               │                 │ parse text  →  │                │
 │               │                 │ signal::Request│                │
 │               │                 │   ::Assert(Node│                │
 │               │                 │     { name: X})│                │
 │               │                 │ ── UDS rkyv ──▶│                │
 │               │                 │                │ validate:      │
 │               │                 │                │  schema/refs/  │
 │               │                 │                │  perms/inv.    │
 │               │                 │                │ ── write ─────▶│
 │               │                 │                │ ◀── ack ───────│
 │               │                 │ ◀── Reply ─────│                │
 │               │                 │   Outcome(Ok)  │                │
 │               │                 │ render text  → │                │
 │               │ ◀── UDS text ───│                │                │
 │ ◀── text ─────│                 │                │                │
```

`mentci-lib` clients skip the nexus daemon — they speak signal
directly to criome (per Q5 RESOLVED).

---

## 7 · Flow — Query (existing M0)

```
CLIENT          CRIOME             SEMA
 │                │                 │
 │ Query(NodeQuery│                 │
 │   { name: ?* })│                 │
 │ ── UDS rkyv ──▶│                 │
 │                │ scan Node table │
 │                │ filter by name  │
 │                │ ── read ───────▶│
 │                │ ◀── Vec<Node> ──│
 │                │                 │
 │ ◀── Reply ─────│                 │
 │  Records::Node │                 │
 │   (Vec<Node>)  │                 │
```

---

## 8 · Flow — Build (NEW @ M5 — the milestone flow)

```
USER     NEXUS DAEMON    CRIOME              LOJIX (links prism)              SEMA
 │            │             │                   │                               │
 │BuildRequest│             │                   │                               │
 │ @target    │             │                   │                               │
 │── text ───▶│             │                   │                               │
 │            │parse →      │                   │                               │
 │            │signal::Build│                   │                               │
 │            │Request{Slot}│                   │                               │
 │            │── UDS rkyv ▶│                   │                               │
 │            │             │ validate target:  │                               │
 │            │             │  Slot resolves to │                               │
 │            │             │  a Graph?         │                               │
 │            │             │ refs ok?          │                               │
 │            │             │ perms ok?         │                               │
 │            │             │ ◀── read records ─────────────────────────────────│
 │            │             │   Graph + Nodes   │                               │
 │            │             │   + Edges         │                               │
 │            │             │                   │                               │
 │            │             │ forward via       │                               │
 │            │             │ signal::          │                               │
 │            │             │   Build(records)  │                               │
 │            │             │ ── UDS rkyv ─────▶│                               │
 │            │             │                   │ ┌─ inside forge ─────────────┐│
 │            │             │                   │ │ call prism (lib):          ││
 │            │             │                   │ │  emit .rs from records     ││
 │            │             │                   │ │ FileMaterialiser:          ││
 │            │             │                   │ │  write workdir to disk     ││
 │            │             │                   │ │ NixRunner:                 ││
 │            │             │                   │ │  spawn nix build           ││
 │            │             │                   │ │  ↓ result: /nix/store/...  ││
 │            │             │                   │ │ StoreWriter:               ││
 │            │             │                   │ │  copy + RPATH-rewrite      ││
 │            │             │                   │ │  + blake3 + redb-index     ││
 │            │             │                   │ │  ↓ store_entry_hash        ││
 │            │             │                   │ └────────────────────────────┘│
 │            │             │ ◀── BuildOk ──────│                               │
 │            │             │  { store_entry_   │                               │
 │            │             │     hash, ... }   │                               │
 │            │             │                   │                               │
 │            │             │ assert            │                               │
 │            │             │ CompiledBinary{   │                               │
 │            │             │  graph: target,   │                               │
 │            │             │  store_entry_hash,│                               │
 │            │             │  narhash, ...}    │                               │
 │            │             │ ─── write ────────────────────────────────────────▶
 │            │             │ ◀── ack ───────────────────────────────────────────│
 │            │             │                   │                               │
 │            │ ◀── Reply ──│                   │                               │
 │            │  Outcome(Ok)│                   │                               │
 │ ◀── text ──│             │                   │                               │
```

**criome's role end-to-end: validate, read, forward, await,
assert, reply.** No subprocess. No file write. No external
tool. No prism link.

**forge's role: receive records, emit, materialize, build,
bundle, reply.** Everything that's "doing" lives here.

---

## 9 · Flow — Subscribe (M2+ — push, never pull)

```
CLIENT                CRIOME                                   SEMA
 │                       │                                       │
 │ Subscribe(NodeQuery   │                                       │
 │   { ... })            │                                       │
 │ ── UDS rkyv ─────────▶│                                       │
 │                       │ register subscription                 │
 │                       │ ◀── any matching write ───────────────│
 │                       │ ◀──    "                ──────────────│
 │ ◀── push: Records ────│                                       │
 │ ◀── push: Records ────│ ◀── any matching write ───────────────│
 │ ◀── push: Records ────│                                       │
 │     ...               │                                       │
 │                       │                                       │
 │ (close socket)        │                                       │
 │ ─── EOF ─────────────▶│ subscription dies with the connection │
```

No initial snapshot — issue a `Query` first if you want current
state. Per `tools-documentation/programming/push-not-pull.md`,
clients **defer** their real-time feature until Subscribe ships
rather than poll while waiting.

---

## 10 · mentci UI — parallel track (M3-M4, independent of M5)

```
USER       GUI REPO           MENTCI-LIB              CRIOME
gesture       │                    │                     │
 │            │                    │                     │
 │ click /    │                    │                     │
 │ drag /     │                    │                     │
 │ keyboard   │                    │                     │
 │──gesture──▶│                    │                     │
 │            │ buffered locally   │                     │
 │            │ until commit       │                     │
 │            │ (Enter, mouse-up,  │                     │
 │            │  explicit submit)  │                     │
 │            │                    │                     │
 │            │ ── commit ────────▶│                     │
 │            │                    │ translate to        │
 │            │                    │ signal::Request     │
 │            │                    │ ── UDS rkyv ───────▶│
 │            │                    │                     │ validate
 │            │                    │                     │ persist or
 │            │                    │                     │ reject
 │            │                    │ ◀── Reply ──────────│
 │            │ ◀── outcome ───────│                     │
 │            │                    │                     │
 │            │ on Outcome(Ok):    │                     │
 │            │   re-render        │                     │
 │            │   (Subscribe push  │                     │
 │            │    delivered the   │                     │
 │            │    new state)      │                     │
 │            │                    │                     │
 │            │ on Diagnostic:     │                     │
 │            │   surface in UI    │                     │
 │            │   next to the      │                     │
 │            │   failed gesture   │                     │
 │            │                    │                     │
```

**Load-bearing property: the UI never holds state that
contradicts criome.** Local in-flight buffer (typing in
progress, wire mid-drag) is *pending input*, not a contradicting
projection. On commit, the gesture becomes one signal envelope
(or one `AtomicBatch` for composite gestures per Q11 RESOLVED).

---

## 11 · Open shapes (the agent's known unknowns)

| Item | Open question |
|---|---|
| `signal::Build` payload fields | precise field set for the records-carrying verb criome forwards to forge |
| `BuildRequestOp` payload fields | beyond `target: Slot` — nix-attr override, target-platform, env knobs? |
| Capability tokens | criome-signed BLS G1 tokens shape; verification path in forge daemon |
| `mentci-lib`'s exact API | precise type names + connection lifecycle (auto-reconnect? handshake retry?) |
| GUI repo name | "mentci" remains the working name in design docs until that repo is created |
| Subscribe payload format | what arrives on the stream — a snapshot delta? a full record? |
| per-kind sema tables | physical layout in redb (replaces the M0 1-byte discriminator) |
| `RelationKind` control-plane variants | `Supervises`, `EscalatesTo` — exact set when the Supervisor kind lands |

These are not blockers — each can be settled when the relevant
component is wired.

---

## 12 · What's NOT here (intentionally)

- **No deployment topology.** Whether components compile into
  one binary, many binaries, or talk over a network is left
  open. The architecture is *source-organization*, not
  deployment (per
  [`tools-documentation/programming/micro-components.md`](../repos/tools-documentation/programming/micro-components.md)).
- **No `nexus`-text grammar additions.** The sigil for
  `BuildRequest` is TBD; nexus parser+renderer wire-in is a
  thin layer.
- **No M6 self-host close.** That's the next layer — criome's
  own request flow expressed as records, prism emits criome
  from them, recompile, loop closes (`bd mentci-next-zv3`,
  `bd mentci-next-ef3`). Mechanism shown here is the
  prerequisite.
- **No mentci UI screens.** The UI's visual design (egui
  widget choices, theming, astrological chart layouts) is
  out of scope here — this report is about the wire and
  components, not the pixels.
- **No CriomOS / horizon-rs / lojix-cli deploy flows.** Those
  are an existing parallel track that retains its current
  shape; they migrate to thin signal-speaking clients of
  forge when
  forge-daemon is wired.

---

## 13 · The criome-runs-nothing rule, illustrated

For verification — the rule as it appears across this picture:

| Concern | criome | forge |
|---|---|---|
| Validates request | ✓ | — |
| Reads from sema | ✓ | — |
| Writes to sema | ✓ | — |
| Forwards typed verbs | ✓ | — |
| Awaits replies | ✓ | — |
| Persists outcome records | ✓ | — |
| Spawns subprocesses | — | ✓ (nix) |
| Writes files outside sema | — | ✓ (workdir + arca) |
| Links prism (library call) | — | ✓ |
| Runs nix-via-crane-and-fenix | — | ✓ |
| Bundles + RPATH-rewrite | — | ✓ |
| Updates redb index in arca | — | ✓ |

If a future agent finds itself adding a "spawn", "write file",
"link prism", "run X" capability to criome, that's the failure
mode the doctrine closes. Add to forge instead — or, if it's a
new capability with its own bounded context, a new component.

---

## 14 · Lifetime

This report is forward-looking — it captures the shape *we
expect to converge on*. It lives in `reports/` until:

- `criome/ARCHITECTURE.md` carries the BuildRequest flow at
  full fidelity (currently has the corrected §7 Compile flow
  but `BuildRequest` itself is unsignalled there).
- `signal/` carries the `BuildRequest` verb as a typed struct
  + matching `BuildRequestOp`.
- `signal/` carries the records-carrying `Build` verb that
  criome forwards to forge (alongside the existing front-end
  verbs).
- `prism/` and `forge/` carry the skeleton-as-design code
  matching this picture.
- `mentci-lib/` and the GUI repo exist (or are explicitly
  scoped to a later milestone).

When all of those exist, this report is deleted. Until then it
is a verification artifact: if the picture above is wrong, this
is the place to correct it before code starts.
