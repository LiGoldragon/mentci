---
title: 077 — nexus and signal — separation of language and wire-format
date: 2026-04-25
anchor: Li 2026-04-25 naming decision (this conversation)
feeds: reports/070 §1+§6+§7; reports/076 §2; architecture.md §1+§2+§10
status: living-decision; updates 070 § naming, 076 § canonical homes,
        architecture.md § three-messaging-layers + § all-rkyv rule
---

# 077 — nexus and signal

## 1. The decision

Li 2026-04-25:

> I've made a decision to differentiate between two slightly
> different aspects of one thing: the messaging layer.
>
> Nexus and Signal:
>
> nexus (text) → nexusd (translates to) → signal (rkyv format
> language to interact with criome) → criome
>
> criome (response) → signal message → nexusd (translates) →
> nexus (text)
>
> *(Quote from before the 2026-04-25 nexusd → nexus repo
> rename. The daemon is now called `nexus`; the local dir is
> still `~/git/nexusd/`. The architectural meaning is
> unchanged — text in, signal out, on the criome-facing leg.)*

The previously-planned name *criome-msg* is replaced by **signal**.
This is not a cosmetic rename. It sharpens the conceptual model
in ways that matter for the architecture and the code, and it
surfaces a small number of new open questions.

## 2. What this sharpens

### 2.1 — "criome-msg" was a category error

The old name suggested the rkyv messaging layer belongs to
criome — that messages are "what criome receives." But the
emitter of those messages is **nexus**, the translator. The
receiver doesn't own the wire format.

*signal* says it correctly: nexus emits signals. A signal is the
rkyv encoding of *what nexus parsed from nexus text*. Signal is
nexus's structured form, not criome's. criome is a consumer of
signal, not its author.

### 2.2 — Two faces of nexus

The language *nexus* now has two faces:

- **nexus text** — the surface form that humans and agents type.
  Position-defines-meaning, delimiter-family matrix, records as
  operators. Defined in [reports/070 §1-§3](070-nexus-language-and-contract.md).
- **signal** — the rkyv-archived structured form, deterministic
  and zero-copy. Defined as the type catalogue in
  [nexus-schema](https://github.com/LiGoldragon/nexus-schema)
  (`pattern.rs`, `query.rs`, `edit.rs`, `value.rs`, `slot.rs`,
  `diagnostic.rs`).

The translation between the two faces is mechanical per 070 §7:
every nexus text construct has exactly one signal form, and vice
versa. This gives nexus a clean, testable contract: parse text →
emit signal, and (on the reply path) decode signal → render text.

### 2.3 — signal is a peer to nexus (architecturally; not yet practically for LLMs)

The mechanical translation rule has a non-obvious corollary: a
client that prefers to compose signal directly — bypassing nexus
text entirely — is doing a legitimate thing. A deterministic
programmatic tool (a Rust client, a script) generating a
transactional batch from program structure may compose
`AssertOp`/`MutateOp`/`TxnBatch` records in rkyv directly, send
those to nexus, and skip the parse step.

But for **LLM agents today**: this is *not* the practical
interface. Today's LLMs are trained on text and can author nexus
text fluently; they cannot author rkyv binary structures
directly. Direct LLM signal authoring is a future capability —
it will land when LLM models are trained against binary signal
formats. Until then, the practical client-side interface for
LLMs is nexus text, parsed into signal by nexus. Per Li's
correction 2026-04-25 ("not yet, not until llm models are trained
using binary signal data").

Architecturally, the peer status of signal still holds — the
"all-rkyv except nexus text" rule already implies it, and
deterministic programmatic clients exercise it. The framing is
"signal is a peer-shaped interface; nexus text is the
peer-shaped interface that LLMs can actually use today."

This re-frames *client-msg* as well. Today client-msg's `Send`
variant carries `nexus_text: String`. A future `Send` variant
could carry a signal frame directly. See **Q-S2** in §6.

## 3. Three messaging layers, named cleanly

| Layer | From → To | Form | Crate / module |
|---|---|---|---|
| **client-msg** | client (nexus-cli, agent, editor) → nexus | rkyv envelope around nexus text + control verbs | [nexus::client_msg](../../nexus/src/client_msg/) (lib half exposed for clients) |
| **signal** | nexus → criome (and reply path) | rkyv envelope around language IR | future `signal` crate (was *criome-msg*) |
| **criome-net** *(post-MVP)* | criome ↔ criome (peer-to-peer) | rkyv envelope around shared facts + signed proposals | future, sketched in [reports/070 §2.5 + §6.1](070-nexus-language-and-contract.md) |

All three are rkyv with the canonical pinned feature set per
[architecture.md §10](../docs/architecture.md). The only non-rkyv
messaging surface in the system is **nexus text**, which lives
inside a client-msg `Send` payload.

## 4. Naming sweep

### 4.1 — In the corpus (mentci)

- [reports/070](070-nexus-language-and-contract.md) — title is
  fine; §6 *"the criome-msg contract"* should read *"the signal
  contract"*. Internal references *criome-msg* → *signal*. ~12
  occurrences. The §6 type names (Frame, Body, Request, Reply,
  etc.) are the **signal envelope**.
- [reports/076 §2](076-corpus-trim-and-forward-agenda.md) — the
  row that points 070 as *"criome-msg contract"* should read
  *"signal contract"*. The trim-ledger row for the deleted 071
  references *"client-msg policies"* — unchanged; client-msg is
  the right name there.
- [architecture.md](../docs/architecture.md) — §1 / §2 / §10 gain
  the three-messaging-layers articulation and the signal name
  (see §5 of this report for proposed text).

### 4.2 — In code

- **nexus-schema** — module docstrings refer to *"wire"* and
  *"on-wire form"*. Update to *"signal-wire"* / *"signal form"*
  for clarity. Type names retain the *Raw* prefix; that prefix
  marks "before kind-resolution by criome" and remains
  load-bearing (see Q-S3 in §6).
- **future `signal` crate** — when criome-msg was planned, this
  is the crate that holds the envelope (Frame, Body, Request,
  Reply, AuthProof, Effect, OkReply, RejectedReply,
  QueryHitReply, ValidateOp, ExecutionPlan). Skeleton-as-design
  per the project pattern. Imports payload types from
  nexus-schema.
- **nexus::client_msg** — name unchanged. Its job is the
  client↔nexus leg.
- **criome** — when scaffolded, takes signal frames over UDS
  and dispatches them through the validator pipeline.

### 4.3 — In bd memory

A bd memory was saved this session naming the
nexus/signal/client-msg three-layer model. Subsequent agents
reading memories will see the convention and apply it.

## 5. Proposed architecture.md updates

### 5.1 — §1 (the pillars / overview), additive sentence

> The messaging layer is named in two pieces: **nexus** is the
> text language for humans and agents; **signal** is the rkyv
> format that nexus emits to criome and reads on the reply
> path. nexus and signal are two faces of one language — text and
> rkyv — and the translation between them is mechanical.

### 5.2 — §2 (invariants), refinement to Invariant B

The current Invariant B says *"Nexus is a request language; sema
is rkyv."* Refine to:

> Nexus is the text request language. Signal is the rkyv form of
> nexus that travels between nexus and criome. nexus text is
> never persisted as records; signal is never rendered to text
> outside nexus. There are no "nexus records." There is sema
> (rkyv records described by KindDecl), and there are signal
> messages (rkyv envelopes carrying language IR).

### 5.3 — §10 (project-wide rules), refinement to all-rkyv

The current rule says *"all-rkyv except nexus."* Refine to:

> **All-rkyv except nexus text.** The only non-rkyv messaging
> surface is the nexus *text* payload inside a client-msg `Send`.
> Every other wire / storage format — client-msg, signal,
> future criome-net, sema records, lojix-store index entries —
> is rkyv with the canonical pinned feature set.

## 6. Open questions surfaced

### Q-S1 — Is signal a peer language to nexus?

§2.3 above frames signal as architecturally peer-shaped, but
practically not yet usable by LLMs (per Li's 2026-04-25
correction). Confirm: is this nuanced framing right? Signal is a
peer for *deterministic programmatic clients*; nexus text is the
peer-shaped interface LLMs can actually author until they are
trained on binary signal data.

Lean: **architecturally yes, practically not yet for LLMs**.
The "all-rkyv except nexus text" rule still permits any client
that *can* compose rkyv to do so; that population today is
deterministic tools, not LLMs.

### Q-S2 — Should client-msg gain a signal-payload Send variant?

Today: `Send { nexus_text: String, fallback: Option<...> }`.
Possible future: `Send { payload: SendPayload, fallback: Option<...> }` where `SendPayload { Text(String), Signal(Frame) }`.

Lean: **defer**. Adding it now is speculative; agents can do the
parsing client-side and send text. Revisit when a real agent
client surfaces and the parse-time savings matter.

### Q-S3 — Drop the *Raw* prefix on signal types?

Today: `RawRecord`, `RawValue`, `RawLiteral`, `RawSegment`,
`RawPattern`, `RawConstraint` (renamed locally to
`FieldConstraint`), `RawListPattern`, `RawOp`, `RawProjection`,
`RawProjField`. The *Raw* prefix originally marked "before
kind-resolution at criome."

Lean: **keep the prefix**. Sema records of "the same shape" exist
post-resolution (e.g., a stored `Pattern` record uses a
`KindDeclId`, not a `kind_name: String`). The prefix marks the
distinction. Dropping it would conflate signal-side and sema-side
shapes. Could rename `Raw` → `Signal` for explicitness:
`SignalRecord`, `SignalValue`, etc. That is a real option;
deferred to Q-S5.

### Q-S4 — Where does the signal envelope live?

Today: planned for a `signal` crate (was *criome-msg*).

Lean: **`signal` crate**. The envelope (Frame, Body, Request,
Reply) is small (~150 LoC), distinct in role from nexus-schema's
records, and naturally separable. nexus-schema imports
boundary-light; signal crate depends on nexus-schema for IR
payload types.

### Q-S5 — Raw prefix → Signal prefix?

A follow-on to Q-S3. If the prefix is kept *and* renamed:

| Now | After |
|---|---|
| `RawRecord` | `SignalRecord` |
| `RawValue` | `SignalValue` |
| `RawLiteral` | `SignalLiteral` |
| `RawSegment` | `SignalSegment` |
| `RawPattern` | `SignalPattern` |
| `FieldConstraint` | unchanged |
| `RawListPattern` | `SignalListPattern` |
| `RawOp` | `SignalOp` |
| `RawProjection` | `SignalProjection` |
| `RawProjField` | `SignalProjField` |

Mechanical rename, ~20 occurrences in nexus-schema across six
files. No semantic change. Lean: **defer until signal crate
lands**, then rename in one pass. Keeping `Raw` is consistent
with the wider `RawRecord`/`RawValue` convention used in 070's
draft contract.

## 7. Action plan

### 7.1 — Lands now (no Li input)

1. Save bd memory naming the three-layer model. **Done.**
2. Update [reports/076 §2](076-corpus-trim-and-forward-agenda.md)
   row about 070 to read *"signal contract"* instead of
   *"criome-msg contract"*. Update [reports/076 §3.4](076-corpus-trim-and-forward-agenda.md#L) to
   mention signal as the wire form of the language IR.
3. Update [reports/070 §6](070-nexus-language-and-contract.md#L378)
   header *"the criome-msg contract"* → *"the signal contract"*.
   Internal *criome-msg* references → *signal*. Add a tombstone
   note at the §6 head pointing to this report (077) for the
   naming history.
4. Update [docs/architecture.md](../docs/architecture.md) per §5
   above (one new sentence in §1, refined Invariant B in §2,
   refined all-rkyv rule in §10).
5. Update nexus-schema module docstrings to use *signal* /
   *signal-wire* terminology where they say *wire* / *on-wire*.
6. Update [docs/workspace-manifest.md](../docs/workspace-manifest.md)
   `criome-msg` → `signal` (CANON-MISSING list).

### 7.2 — Needs Li (when ready)

- **Q-S1** confirm signal-as-peer.
- **Q-S5** confirm `Raw` → `Signal` prefix sweep, or keep `Raw`.

### 7.3 — Unlocked when signal crate is created

Skeleton-as-design for the signal envelope:

```rust
// in repos/signal/src/frame.rs (when created)
pub struct Frame {
    pub correlation_id: u64,
    pub principal_hint: Option<Slot>,
    pub auth_proof: Option<AuthProof>,
    pub body: Body,
}
pub enum Body { Request(Request), Reply(Reply) }
// Request/Reply enums per 070 §6.2 + §6.4
```

That crate carries the envelope (~150 LoC) and re-exports the
payload IR types from nexus-schema. cargo check + round-trip
tests. No Li input required to start; just one nod that the
signal crate name and location are right.

## 8. What this does *not* decide

- The set of signal verbs at v0.0.1 (Q from 070 §8 — already
  open).
- Subscription delivery semantics (Q from 070 §8 — already
  open).
- How signal frames are framed on the UDS socket beyond rkyv
  archive bytes — already settled by 074: the frame schema is
  the framing.
- Whether nexus-cli will ever speak signal directly (Q-S2).
- The exact field-resolution rules for *kind_name → KindDeclId*
  during validation step 1 (a criome concern).

These remain in [reports/076 §3](076-corpus-trim-and-forward-agenda.md)
or in 070 §8.

## 9. Summary

The nexus/signal split is the right shape. It corrects a category
error (criome did not own the wire format), it elevates signal to
peer-of-nexus status (which the all-rkyv-except-nexus-text rule
already implied), and it gives the system a clean three-layer
naming: client-msg / signal / (future) criome-net.

Action: land §7.1's autonomous updates immediately; surface Q-S1
and Q-S5 to Li when convenient; create the signal crate when
scaffolding begins.
