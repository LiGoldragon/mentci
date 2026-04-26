# 083 — The return protocol (nexus + signal)

*2026-04-26 · How replies travel back, how the two wire layers fit together, and how each of the rough edges in the reply story gets resolved. With diagrams. Companion to [reports/082](082-delimiter-set-rewrite.md) (the delimiter set) and the spec at [`nexus/spec/grammar.md`](https://github.com/LiGoldragon/nexus/blob/main/spec/grammar.md).*

---

## The whole picture

```
text-speaking clients         signal-speaking peers
(shell, REPL, LLM agent,       (the nexus daemon talking
 nexus-cli, anything            to criome — and criome's
 typing nexus by hand)          internal subsystems)
        │                                │
        │ pure nexus text                │ length-prefixed
        │ in / out                       │ rkyv frames
        ▼                                ▼
┌──────────────────┐              ┌─────────────────┐
│ /tmp/nexus.sock  │              │ /tmp/criome.sock│
│  nexus daemon    │  ── signal ► │     criome      │
│ (text translator)│  ◄ signal ── │ (validator+sema)│
└──────────────────┘              └─────────────────┘
        ▲                                ▲
        │                                │
   nexus is the                    signal is the
   public face                     internal form
```

Two distinct wire formats, one for each audience.

---

## What is signal?

Signal is the **native binary form** of the records criome holds. Same record kinds, same verbs, same sigils — encoded in rkyv (zero-copy, deterministic, schema-driven binary) instead of text. It exists because **criome is sema, and sema is by definition directly computer-cognizable**. The bytes a record occupies at rest *are* its meaning — no parsing, no interpretation. The canonical form is binary; text is a translation layer for human convenience.

A signal frame has the same structure as the nexus expression it represents. Where nexus says:

```
(Node user User)
```

signal says (conceptually):

```
┌──────────┬────────────────────────────────┐
│ 4 bytes  │  rkyv-encoded record:          │
│  length  │   kind = Node                  │
│  prefix  │   field 0 = "user"             │
│          │   field 1 = "User"             │
└──────────┴────────────────────────────────┘
```

Same content, machine-readable. Reader and writer both know the schema (the record kinds are compiled in); nothing in the bytes describes itself.

**Who speaks signal:**
- The nexus daemon (translates nexus text ↔ signal)
- criome (the engine — only speaks signal)
- Any client that wants to skip the text layer (e.g. an internal Rust service that already has typed records in hand)

**Who speaks nexus text:**
- Humans typing into a terminal
- LLM agents authoring records
- Editor LSPs
- Shell scripts piping into `nexus-cli`
- Anything else that doesn't want to deal with rkyv

The two surfaces are kept symmetric so a request can flow through both without losing meaning.

---

## How replies travel — the positional rule

Replies come back **in the same order as requests**. The N-th reply on a connection corresponds to the N-th request. There are no correlation IDs.

```
client                              daemon
  │                                    │
  ├── request 1 ─────────────────────► │
  ├── request 2 ─────────────────────► │
  ├── request 3 ─────────────────────► │
  │                                    │
  │ ◄── reply 1 ──────────────────────│
  │ ◄── reply 2 ──────────────────────│
  │ ◄── reply 3 ──────────────────────│
  │                                    │
```

The reply *form* mirrors the request form, with the same sigil discipline:

| Request | Reply |
|---|---|
| `(R …)` | `(R …)` — the record (with assigned slot encoded) |
| `~(R …)` | `~(R …)` — the new record version |
| `!(R …)` | `!(R …)` — the retracted record (last echo) |
| `?(R …)` | `?(R …)` — the would-be record |
| `(\| pat \|)` | `[<r1> <r2> …]` — sequence of matches |
| `[\| ops \|]` | `[\| reply1 reply2 … \|]` — per-op replies in batch |
| any of the above on failure | `(Diagnostic …)` — at that reply position |

Same shape both directions. Position pairs them. No new tags, no new keywords.

---

## Problem 1 — slot dependencies inside batches

A common request shape: assert two records, then assert a third that references the first two by their assigned slots.

```
client wants:
  (Node user User)             ← gets slot ?
  (Node admin Admin)           ← gets slot ?
  (Edge ?-of-user ?-of-admin "delegates")
```

Without a solution, the client has to round-trip — wait for slot 100, wait for slot 101, *then* compose the Edge. No pipelining for any chain of dependent edits.

### Solution: bind sigils as tempids inside `[\| \|]`

The `@` sigil already names holes in patterns. In an outgoing **atomic batch**, extend its meaning: `@name` is a placeholder for the slot that will be assigned. Subsequent uses of the same `@name` *within the same batch* refer to that slot.

```
[|
  (Node @u user User)
  (Node @a admin Admin)
  (Edge @u @a "delegates")
|]
```

Criome lays out the batch, assigns slots, substitutes the names:

```
[|
  (Node 100 user User)
  (Node 101 admin Admin)
  (Edge 100 101 "delegates")
|]
```

The reply is the same shape with binds resolved.

**Scope:** within one `[\| \|]` only. No cross-request bind state. Want references across requests? Round-trip + use real slots from the first reply.

```
batch:                          reply:
                                 ┌────────────────────────┐
[|                               │ [|                     │
  (Node @u user User)            │   (Node 100 user User) │
  (Node @a admin Admin)          │   (Node 101 admin     │
  (Edge @u @a "delegates")       │         Admin)         │
|]                               │   (Edge 100 101        │
                                 │         "delegates")   │
                                 │ |]                     │
                                 └────────────────────────┘
        │                                  ▲
        │  daemon resolves @u ─► 100,      │
        │  @a ─► 101, substitutes ─────────┘
        ▼
```

Zero new syntax. The `@` sigil that already means "named hole" picks up a second context.

---

## Problem 2 — mid-sequence query errors

A query reply renders as `[<r1> <r2> …]` in nexus text. Possible failure modes:

**Validation fails during query execution.** Criome runs the query and one record can't be returned (permission denial, schema mismatch). Criome's signal Reply is a `Diagnostic`, not a partial result set.

**Daemon crashes or loses client connection mid-render.** Criome has already sent a complete signal Reply to the daemon; the daemon has the typed records in hand and is rendering them to text. If the daemon dies mid-render or the client socket drops, the client sees truncated text. The kernel notifies the client with EOF; client reconnects and re-issues the query.

(Criome itself never streams text — it speaks signal only, and signal frames are atomic at the frame level. There's no "half a sequence in flight" at the criome boundary.)

### Solution: text replies are atomic at the position

The daemon doesn't emit half-results. The reply is *atomic at the position* — either you get a complete `[<r1> <r2> <r3>]`, or you get a single `(Diagnostic …)` *instead of* the sequence. Never half-and-half. Implementation: daemon receives the complete signal Reply from criome, renders it to text, writes it as one logical message to the client socket.

```
position N:
   either                          or
   ┌──────────────────────┐       ┌────────────────────┐
   │ [ <r1> <r2> <r3> ]   │       │ (Diagnostic …)     │
   └──────────────────────┘       └────────────────────┘
   complete sequence              the entire reply
                                  is the diagnostic
```

For genuine criome crashes mid-emit: the connection drops. The kernel notifies the client with EOF. Client reconnects + retries — same machinery as a network error.

For huge result sets: pagination is a follow-up. Each page is its own complete reply.

---

## Problem 3 — FIFO ordering inside a connection

One connection processes serially. If request 1 is a 10-second query and request 2 is a 1ms assert, request 2's reply waits for request 1's. No parallelism within a connection.

### Solution: parallelism = more connections

Want concurrent work? Open more connections. Each is its own serial lane.

```
connection A:
  ├─ slow query ──┐
  │               ├─ reply ─►  (waits 10s)
  │               │

connection B:
  ├─ fast assert ─┐
  │               ├─ reply ─►  (waits 1ms)
  │               │

connection C:
  *(| Node @id |) ─► subscription stream
                     (one sub per conn)
```

UDS connections are kernel-cheap (~µs). One client process can hold many. Composes naturally with the one-subscription-per-connection rule.

---

## Problem 4 — cancellation

A slow non-subscription request blocks its connection until reply. There's no `(Cancel)` verb (rejected — would introduce a privileged kind name and a per-request correlation system we don't otherwise need).

### Solution: cancel by closing the connection

```
client                              daemon
  │                                    │
  ├── slow query ───────────────────► │
  │                                    │ (criome working...)
  │                                    │
  ├── × close socket                   │
                                       │
                                       │ kernel sends EOF;
                                       │ daemon notifies criome;
                                       │ criome aborts the query
                                       │ + drops resources
```

Combined with multi-connection parallelism (Problem 3): each cancellation costs only one connection. Other work in flight on other connections is untouched.

Brutal but minimal. No new verbs, no correlation IDs, no special state.

---

## Problem 5 — subscription snapshot vs events race

Old design: `*(| pat |)` returned `[snapshot]` then streamed events. Race window: between snapshot computation and event-subscriber registration, an edit could happen — could land in neither (lost) or both (double-counted).

### Solution: subscriptions don't carry a snapshot

A subscription is *a pure event stream*. From the moment the daemon registers it, the client sees every matching event going forward. Want current state? Issue a `Query` first.

```
client                              daemon
  │                                    │
  ├── (| Node @id |) ────────────────► │   ← Query: get current state
  │ ◄── [<n1> <n2> <n3>] ─────────────│   ← reply with snapshot
  │                                    │
  ├── *(| Node @id |) ──────────────► │   ← Subscribe: from now onward
  │                                    │
  │ ◄── (Node n4 …) ───────────────── │   ← future event
  │ ◄── ~(Node n2 …) ──────────────── │   ← future event
  │ ◄── !(Node n1 …) ──────────────── │   ← future event
  │                                    │
```

**Race window between Query and Subscribe:** acceptable for M0. If an edit happens between the query reply and the subscribe registration, the client may miss it (small window in practice).

**If that race ever matters:** add an `at-revision` parameter so Subscribe picks up exactly where the Query stopped. Deferred — simpler is better today.

---

## Problem 6 — subscription replies are uniformly single records

With Problem 5's solution applied, every reply on a subscription connection is a single record (or sigil-prefixed record). No more "first reply is a sequence, then individual records" heterogeneity.

```
subscription connection:
  ┌──────────────────────────────────┐
  │ (Node a "Apple")     ← Insert    │
  │ (Node b "Banana")    ← Insert    │
  │ ~(Node a "Apricot")  ← Mutate    │
  │ !(Node b "Banana")   ← Retract   │
  │ (Node c "Cherry")    ← Insert    │
  │ … forever, until socket closes   │
  └──────────────────────────────────┘
```

Reusing the request-side sigil discipline on events: bare `(R)` = something matching arrived; `~(R)` = something matching changed; `!(R)` = something matching is gone.

Client reads top-level expressions; the sigil tells the operation. Same parser shape as a single-shot reply.

---

## What goes wrong in criome — implementation invariants

Two invariants the criome implementer must enforce. Not protocol-level; *daemon-level discipline.*

**Invariant A — atomic snapshot+subscribe (was Problem 5).** When `*(| pat |)` arrives and is dispatched, criome must in a single transactional step (a) record the subscription's revision watermark and (b) register the event subscriber. No edit may slip into a state where it's after the watermark but before the subscriber is in place.

**Invariant B — atomic batch with tempids (Problem 1).** When `[| ops |]` arrives with `@`-binds, criome must (a) parse the batch, (b) reserve a contiguous slot range for all asserts, (c) substitute bind names with reserved slots, (d) run validation across the substituted batch, (e) commit-or-fail atomically. Bind names are local to the batch; criome doesn't carry them across.

These belong in `criome/ARCHITECTURE.md` once the engine is being written.

---

## The signal-side reflection

Everything above is described in nexus text terms because that's the user-facing surface. Inside the daemon ↔ criome leg, the same operations happen but in signal frames.

```
nexus text                       signal frame (rkyv)
─────────                        ──────────────────

(Node user User)                 ┌────────────────────┐
                                 │ Frame {            │
                                 │   correlation: 1,  │
                                 │   body: Request {  │
                                 │     Assert(Node {  │
                                 │       id: "user",  │
                                 │       label:"User" │
                                 │     })             │
                                 │   }                │
                                 │ }                  │
                                 └────────────────────┘

[|                               ┌────────────────────┐
  (Node @u user User)            │ Frame {            │
  (Edge @u @u "self")            │   body: Request {  │
|]                               │     AtomicBatch([  │
                                 │       Assert(Node {│
                                 │         tempid:@u, │
                                 │         …          │
                                 │       }),          │
                                 │       Assert(Edge {│
                                 │         from:@u,   │
                                 │         to:@u, …   │
                                 │       }),          │
                                 │     ])             │
                                 │   }                │
                                 │ }                  │
                                 └────────────────────┘
```

The bind names (`@u`, `@a`, etc.) carry through into signal as a small enum: `Slot::Resolved(u64)` vs `Slot::Tempid(SmallString)`. Criome resolves Tempid → Resolved during batch layout; the substituted form is what gets persisted and what comes back on the reply.

Same structural correspondence on the reply side: Diagnostic record in nexus is a Diagnostic record in signal. Sequence of matches in nexus is a `Vec<RawRecord>` in signal. The mechanical-translation rule holds.

---

## What's not in this protocol

Things deferred or rejected:

| Rejected / deferred | Why |
|---|---|
| Correlation IDs | Position pairs replies — no IDs needed |
| Heartbeat / keepalive | UDS is local; kernel handles liveness |
| Resume after disconnect | Durable work belongs in sema as a record (e.g. a `Job`); transient work dies with the connection |
| Cancel verb | Close the socket — same machinery, no new vocabulary |
| Goodbye record | Close the socket |
| Subscription snapshot | Issue a Query first; Subscribe is forward-only |
| Comparison operators in patterns | `< > <= >= !=` reserved; design pending |
| Privileged reply kinds (`Ack`, `EndOfReply`, `Subscription`, `Event`, …) | Replies use existing sigil discipline; no new kinds |

---

## Implementation impact (concrete)

To get from where we are now to this protocol:

| File | Change |
|---|---|
| `nexus/spec/grammar.md` | Add the *Reply semantics* and *Connection semantics* sections (already drafted in the spec); document tempid use of `@` inside `[\| \|]`; document atomic-emission of replies. |
| `signal/src/slot.rs` | Extend `Slot` to a small enum: `Resolved(u64)` \| `Tempid(SmallString)`. (Or add a new `SlotRef` type; bikeshed name later.) |
| `signal/src/edit.rs` | `AtomicBatch` already exists — verify it threads tempids through. |
| `signal/src/reply.rs` | Drop `SubReady` / `SubSnapshot` (subscriptions emit events directly). Add `SubEndReason` typed enum on `SubEnd` (was a `String`). Drop `Cancel` / `Cancelled` if present. |
| `nexus-serde` | `AtomicBatch<T>` wrapper already present — verify tempid bind-names round-trip through. |
| `criome/src/validator/...` | Implement Invariants A and B as part of the validator pipeline. Will live in `schema.rs` (kind dispatch) + `write.rs` (slot assignment + tempid resolution). |
| `criome/ARCHITECTURE.md` | Document Invariants A and B alongside the validator pipeline description. |
| `nexus-cli/ARCHITECTURE.md` | Already needs rewriting (per earlier audit) — describe it as the thin text-shuttle it is, no client_msg framing. |

---

## Cross-cutting context

- Spec: [`nexus/spec/grammar.md`](https://github.com/LiGoldragon/nexus/blob/main/spec/grammar.md)
- Delimiter set: [reports/082](082-delimiter-set-rewrite.md)
- Earlier wire-layer research: [`signal/ARCHITECTURE.md`](https://github.com/LiGoldragon/signal/blob/main/ARCHITECTURE.md)
- Project-wide architecture: [`criome/ARCHITECTURE.md`](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)
