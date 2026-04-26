---
title: 084 — text wire for the nexus daemon's client-facing socket
date: 2026-04-26
anchor: Li 2026-04-26 "the client-facing socket carries nexus text and only nexus text — no rkyv, no signal frames, no binary handshake, no smart-client tier"
feeds: criome/ARCHITECTURE.md §3+§4 ; nexus/spec/grammar.md ; reports/082+083 (binary protocol on the internal leg)
status: research synthesis with concrete proposal; awaits Li's calls on the open decisions in §13
---

# 084 — text wire for the nexus daemon's client-facing socket

The nexus daemon owns two distinct sockets. The internal one
(`/tmp/criome.sock`) carries typed binary frames between the
nexus daemon and criome — that wire is signal, designed in
reports 082+083. The client-facing one (`/tmp/nexus.sock`) is a
different beast: it carries **nexus text and only nexus text**.
LLM agents, scripts, REPLs, LSPs, the CLI — every client speaks
the same plain-text protocol.

This report studies how text-stream protocols on local sockets
handle framing, correlation, streaming, cancellation, and
errors; then proposes the minimum text-wire design that uses
only existing nexus syntax for everything user-visible.

The aesthetic the design holds itself to:

- **Schema-as-framing.** Reader and writer share the nexus
  grammar; nothing in the bytes describes itself.
- **The protocol is nexus, not a layer on top of it.** Every
  request, reply, subscription event, error, and cancellation
  is written as a nexus expression in the existing grammar.
- **Pascal-named records, never new sigils.** Per the closed
  sigil budget in `criome/ARCHITECTURE.md` §10.
- **Friction-free for LLMs writing by hand.** No protocol
  envelope to remember; no correlation IDs to invent; no
  handshake to perform.

---

## 1 · Survey of comparable text-stream protocols

Each subsection summarises one protocol's framing, correlation,
streaming, and lifecycle in one paragraph. Citations point at
primary specifications.

### 1.1 — nREPL (Clojure)

nREPL frames messages as **bencode** dictionaries on a long-
lived TCP socket. Each message is a self-delimited bencoded
map; the receiver reads bytes until the bencode parser closes
its top-level map. Correlation is by `:id` field — every
request carries one, the reply echoes it. A single request can
produce many replies (an `:eval` may stream `:value`, `:out`,
`:err` deliveries before a final `{:status ["done"]}`); the
`:status` set names the lifecycle state. Cancellation is the
`interrupt` op (a separate message naming the in-flight `:id`).
Errors come back as a reply with `:status ["error"]` and a
human-readable `:err` field. The session is implicit — there's
no "open" message; the first request creates the session.
Sources: `nrepl.org/nrepl/1.1/design/transports.html` ;
`github.com/nrepl/nrepl/blob/master/src/clojure/nrepl/transport.clj`.

The take-away: **streamed multi-reply per request** is the
universal LISP-REPL pattern; the `:status` field that closes
the stream is the load-bearing convention. The `:id` for
correlation is what lets a session multiplex.

### 1.2 — SLIME / SWANK (Common Lisp)

SWANK frames messages as `(length: 6 hex digits)(s-expr)` —
six ASCII hex characters giving the byte length, immediately
followed by an s-expression. Inside the s-expr, the first
position is a tag like `:emacs-rex` (request from Emacs),
`:return` (server reply), `:write-string` (output),
`:debug` (entered debugger). Requests carry an integer id at
a fixed position; the matching `:return` echoes it. Streaming
is via separate top-level forms (`:write-string` deliveries
arrive between request and final reply). Errors that drop into
the debugger send `:debug` and pause until the user picks a
restart. Cancellation: an `:emacs-interrupt` tag. Sources:
`common-lisp.net/project/slime/doc/html/Communication-with-Emacs.html` ;
the SLIME source `swank/swank.lisp` and `slime.el`.

The take-away: SWANK proves that **wrapping every transport
event in an s-expression with a leading tag** — request, reply,
out-of-band output, debugger entry — is a usable design. The
length prefix is a kludge for parser efficiency; with a
streaming s-expression parser, it's not necessary. The
**explicit per-event tag** pattern is what SLIME demonstrates
works at scale.

### 1.3 — Geiser (Scheme)

Geiser is simpler than SLIME: a Scheme REPL exposed over a
TCP socket. The framing is **read until balanced parens** plus
a sentinel string (`<gz>` and `</gz>`) wrapping each reply
batch. Requests are bare s-expressions; replies are
s-expression lists in a known shape. Correlation is implicit
— request/reply alternation, one in flight at a time per
session. No subscriptions, no async. Sources:
`geiser.nongnu.org/manual/geiser.html` ; the Geiser source
`elisp/geiser-connection.el`.

The take-away: **read-until-balanced** is sufficient for
self-delimiting s-expressions. The sentinel wrapping is for
robustness against partial reads, not parsing — modern stream
parsers don't need it. **Request/reply alternation** with no
correlation works fine for an interactive REPL; it breaks
down for subscriptions.

### 1.4 — Redis RESP

RESP is a typed text protocol: each value has a type prefix
(`+` simple string, `-` error, `:` integer, `$` bulk string
with length, `*` array with count). Framing is recursive: an
array contains N values, each parsed by the same rules. CRLF
terminates each scalar; bulk strings carry an explicit length.
Correlation is **strict pipeline ordering**: reply N matches
request N on the same connection. Pub/Sub is a mode switch
— after `SUBSCRIBE`, the connection cannot run normal
commands; published messages arrive as 3-element arrays
(`["message", channel, payload]`) intermixed with no
correlation. Errors are inline (`-ERR message\r\n`).
Cancellation isn't a wire concept; client closes the socket.
Sources: `redis.io/topics/protocol` ; the Redis source
`src/networking.c`.

The take-away: **strict pipeline ordering** removes the need
for explicit IDs entirely; it's the simplest possible
correlation scheme. The subscribe-mode-lock is brutal but
honest — it acknowledges that streaming and request/reply
have fundamentally different semantics.

### 1.5 — HTTP/1.x

HTTP/1.x is line-oriented for the request line and headers,
length-or-chunked for the body. Framing: `Content-Length` or
`Transfer-Encoding: chunked` tells the receiver where the body
ends. Correlation is **strict pipeline ordering** in the
specification; in practice browsers don't pipeline because of
head-of-line blocking. Streaming server-pushes use chunked
encoding plus an open connection (Server-Sent Events extends
this with `event:`/`data:` lines and a documented format).
Cancellation: client closes the socket. Errors are status
codes in the response line. Sources:
`datatracker.ietf.org/doc/html/rfc9112` ;
`html.spec.whatwg.org/multipage/server-sent-events.html`.

The take-away: HTTP/1.x is the granddaddy of text protocols;
its lessons are durability of "framing tells you where the
body ends" and the Server-Sent Events pattern of "named
event types in a long-lived response stream."

### 1.6 — JSON-RPC over stdio (LSP)

The Language Server Protocol uses JSON-RPC 2.0 over stdio with
HTTP-style framing: `Content-Length: N\r\n\r\n{json}`. Each
message is a JSON object with `jsonrpc: "2.0"`, an optional
`id` (number or string), and either `method` + `params`
(request) or `result` / `error` (reply). **No `id` means a
notification** (one-way; no reply). Correlation is by `id`;
servers MAY process out of order. Cancellation is the
`$/cancelRequest` notification with the target id; servers
SHOULD respond with an error for the cancelled request.
Streaming is via *progress* notifications — the request carries
a `workDoneToken`, the server emits `$/progress` notifications
tagged with the same token. Sources:
`microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/` ;
`jsonrpc.org/specification`.

The take-away: LSP demonstrates the **request/notification
distinction** done well — the absence of an `id` field signals
"don't reply." Progress-via-token is the LSP equivalent of
named subscription IDs. The cancel-as-notification pattern
proves cancellation can be a normal message rather than a
special wire verb.

### 1.7 — Plan 9 con / styx

Plan 9's user-facing serial-line interface (`con(1)`) is line-
oriented text — the user types commands; the kernel echoes
output line by line. There's no framing beyond newlines and
no correlation: the terminal is a half-duplex conversation,
the user types one command and waits for output to settle
before typing the next. Inside the kernel, `styx` (the wire
form of 9P, see report 082 §1.2) handles structured framing.
Sources: `9p.io/sys/man/8/con.html` ; `9p.io/sys/doc/`.

The take-away: terminal-style text protocols work for human
interaction precisely because the human imposes the timing.
Once a non-human caller is in the loop — and especially once
streaming is needed — line-oriented protocols force you to
invent framing (sentinels, prompts, length prefixes).

### 1.8 — IRC / SMTP / NNTP

The line-oriented family. Each command is one line ending
`\r\n`. Multi-line replies are bracketed by a continuation
convention: SMTP uses `250-First line\r\n250 Last line\r\n`
(dash for continuation, space for last); NNTP and SMTP use
**dot-stuffing** for arbitrary-length payloads — a single `.`
on a line ends the body, lines starting with `.` get an extra
`.` prepended. Correlation is **strict pipeline ordering** at
the line level; a multi-line reply fully completes before the
next request. Numeric reply codes (`250`, `550`) are a
parseable status; trailing text is human-readable. Pub/Sub
isn't in the original protocols; IRC adds it as a mode
(`JOIN #channel` then channel messages arrive interleaved
with replies). Sources: `datatracker.ietf.org/doc/html/rfc5321`
(SMTP) ; `datatracker.ietf.org/doc/html/rfc1459` (IRC) ;
`datatracker.ietf.org/doc/html/rfc3977` (NNTP).

The take-away: **dot-stuffing** is the historical answer to
"how do you embed arbitrary text in a line-oriented protocol";
the modern answer is "use a self-delimiting parser format."
S-expressions self-delimit via balanced parens, removing the
need for either dot-stuffing or length prefixes.

### 1.9 — MPD (Music Player Daemon)

MPD's protocol is line-oriented over TCP/UDS. Each command is
one line; the server replies with zero or more `key: value`
lines and terminates with `OK\r\n` or `ACK [code@line] {cmd}
message\r\n`. Correlation is **strict pipeline ordering**;
clients can batch with `command_list_begin` / `command_list_end`.
The `idle` command opens a wait-for-state-change subscription:
the server holds the response until something changes, then
sends a `changed: subsystem` line followed by `OK`. The client
reissues `idle` to resume. There's no concurrent subscriptions
on one connection — `idle` blocks the connection. Cancellation
of `idle` is the `noidle` command (only valid mid-idle).
Sources: `mpd.readthedocs.io/en/latest/protocol.html`.

The take-away: **`OK` / `ACK` as terminators** are the simplest
possible reply-end markers; the explicit `noidle` cancel
pattern is what 9P's `Tflush` and gRPC's `RST_STREAM` look
like at the text level.

### 1.10 — Erlang ports (text-term mode)

When an Erlang port runs in `{packet, line}` or text-term mode,
external programs send Erlang term literals — bare s-expression-
flavoured text — separated by newlines. The Erlang side parses
each line via `erl_scan` + `erl_parse`. Correlation is by
embedding a tag inside the term: `{ref(), Reply}` where `ref()`
is a unique reference the requester remembers. Streaming uses
the same channel — the receiver reads all incoming terms;
ordering is FIFO. Sources:
`erlang.org/doc/man/erl_interface.html` ;
`erlang.org/doc/apps/erts/erl_ext_dist.html`.

The take-away: **embedding the correlation in the term itself**
(rather than a transport-layer header) is the term-language
approach. The same idea works for nexus: if correlation is
needed, it lives inside a record, not as a wire-layer field.

---

## 2 · The aesthetic, restated as constraints

These follow from the established project rules
(`criome/ARCHITECTURE.md` §10, `nexus/spec/grammar.md`, and
Li's 2026-04-26 framing).

1. **Plain nexus text on the wire.** No length prefix, no
   envelope, no out-of-band framing. The bytes on the socket
   are valid `.nexus` source.

2. **Schema-as-framing.** Reader and writer share the nexus
   grammar. Self-delimitation comes from balanced delimiters;
   nothing in the bytes describes its own shape.

3. **No new keywords; no new sigils.** Per the closed sigil
   budget. New mechanisms land as PascalCase records, not as
   new punctuation.

4. **No protocol verbs that aren't records.** Cancel,
   subscribe, unsubscribe, errors — all are nexus expressions
   in the existing grammar.

5. **No privileged kinds at the validator.** The daemon
   recognises certain Pascal-named records (`Subscribe`,
   `Unsubscribe`, `Goodbye`, `Diagnostic`, `Subscription`,
   `Event`) as protocol-level records it acts on; criome's
   validator knows nothing of them. (Consistent with the
   reports/082 §2 distinction: "this name maps to this Rust
   type" is fine; "this name triggers special validator
   logic" is not.)

6. **LLM-first.** The primary client is an LLM agent typing
   nexus by hand. The protocol must be writable in one pass
   without consulting documentation.

7. **One protocol for everyone.** No tier distinction. The
   CLI, an LSP, a curl-equivalent shell pipeline, an LLM —
   all speak the same wire.

8. **Defer.** Anything not strictly needed for M0 (single-
   operator, single-connection, flow-graph kinds) ships as
   `todo!()` server-side or doesn't ship at all.

These constraints rule out: explicit length prefixes, JSON-RPC
style envelopes, correlation IDs as wire-layer fields,
keyword-tagged framing (`+OK`, `-ERR`, `:status`), separate
control-plane channels, smart/dumb client tiers, version
negotiation handshakes.

---

## 3 · Question 1 — Message boundary

**The question.** S-expressions self-delimit (matched parens).
Is "read until balanced" sufficient? Or do we need an explicit
terminator? What about whitespace between expressions?

**The answer.** **Read until the top-level parser completes
one expression.** Whitespace between top-level expressions is
ignored. No terminator is required; none is forbidden.

**Rationale.**

nexus is a stream of top-level expressions. Each top-level
expression is either:

- A record `(Kind …)` — bare, prefixed with `~`, prefixed
  with `!`, or wrapped in `~(\| pattern \|) (replacement)`
- A pattern `(\| Kind …\|)` — bare or in a constrain
  block `{\| … \|}`
- A pattern with shape projection `(\| … \|) { fields }`

Every top-level form has matched outer delimiters. The
top-level parser reads characters into a streaming nota
tokeniser; once the outer delimiter pair closes, the
expression is complete and the parser hands it to the
dispatcher. Then it resumes consuming whitespace and
comments until the next non-trivia byte signals a new
expression.

**Comments and whitespace.** `;;` comments and ASCII
whitespace between top-level forms are ignored — the daemon
discards them on the way through. Inside a form, the existing
nexus rules apply; comments may not appear inside a single
record but may appear between top-level forms.

**EOF semantics.** When the socket reaches EOF mid-expression,
the daemon emits a `(Diagnostic …)` describing the unclosed
form (a `Parse` diagnostic, code E0001) and closes the
connection. EOF between top-level expressions is the clean-
disconnect case; the daemon closes its half too.

**Why no terminator.** Adding a terminator (newline, NUL,
empty line) would add a second framing mechanism on top of
balanced delimiters — duplicate truth. The grammar already
self-delimits; relying on it is the schema-as-framing
principle.

**Buffering for the writer.** The daemon writes one complete
top-level expression at a time; partial writes are a kernel
concern. Clients that read one expression at a time use the
same logic in reverse.

**Concrete example.** A client sends three asserts back to
back without any separator:

```
(Node user [User])(Node nexus [nexus daemon])(Node criome [criome daemon])
```

The daemon parses each `(Node …)` form, dispatches it,
returns each reply in order. Whitespace makes the wire
prettier:

```
(Node user [User])
(Node nexus [nexus daemon])
(Node criome [criome daemon])
```

Both are equivalent; both arrive parsed identically.

---

## 4 · Question 2 — Correlation

**The question.** When multiple requests are in flight on a
multiplexed connection, how do replies match requests?

**The answer.** **Strict FIFO request/reply ordering on a
single connection.** No correlation IDs in the wire. Replies
arrive in the order the requests were sent.

**Rationale.**

For an LLM writing nexus by hand, **no correlation is the
right answer**. Inventing a correlation ID per request is
exactly the kind of protocol magic that LLMs get wrong. The
LLM types a request, reads the reply, types the next.

For pipelined scripts, **strict ordering is sufficient**:
issuing five asserts in sequence and reading five replies in
order works without any per-request bookkeeping. This is the
RESP, MPD, IRC, SMTP pattern (§§1.4, 1.8, 1.9). HTTP/1.x
specified pipelining the same way.

For long-running operations, **strict ordering would block**
— but in the nexus model, "long-running" maps to subscriptions
(see §6) and to async-job-as-record patterns (see §10). Both
sidestep the need for out-of-order replies on a single
connection.

**The subscription wrinkle.** A subscription opens a stream
of events that arrive interleaved with later requests' replies.
The subscription events carry a slot reference (the
`Subscription` record's slot) so the client can route them.
Subscription events are not replies in the FIFO sense — they
are unsolicited records the daemon emits whenever the criome
backend reports a match. They never delay the FIFO reply
queue: requests sent after the subscription opens get their
replies in order; subscription events are inserted between
those replies as they fire.

This matches Redis's pub/sub model (§1.4) — subscription
events are tagged at the record level, not the protocol
level. The difference: Redis switches the connection to
"subscribe mode" and disallows normal commands; nexus does
not, because subscription events are distinguishable by
their record kind (`Event …`) from request replies (which
are bare records of the same kinds the client asserted).

**Compared to nREPL / SLIME / LSP.** Those protocols use
explicit IDs because they multiplex many concurrent slow
requests over one connection (Emacs has many evaluation
panes; LSP has many concurrent code actions). nexus's M0
workload — flow-graph asserts and queries — is sub-millisecond
per request; FIFO ordering on a connection is fine. If a
specific use case ever demands true async multiplexing, the
client can open a second UDS connection — which is free on
a local socket.

**One reply per request.** Most requests produce exactly one
reply expression. A query that returns N records produces N
reply expressions (one per match) followed by a closing
`(EndOfReply)` marker. Subscriptions produce a `Subscription`
acknowledgement followed by `Event` records over time.
Specifically:

| Request shape | Reply shape | Count |
|---|---|---|
| `(Node id [label])` (assert) | `(Ack <slot>)` | 1 |
| `~(Node …)` (mutate) | `(Ack <slot>)` | 1 |
| `!(Node …)` (retract) | `(Ack)` | 1 |
| `(\| Node @id @label \|)` (query) | N matching records, then `(EndOfReply)` | N+1 |
| `(Subscribe (\| Node @id @label \|))` | `(Subscription <slot>)` then `Event` records over time | 1 + ∞ |
| `(Unsubscribe <slot>)` | `(Ack)` | 1 |
| `(Goodbye)` | `(Goodbye)` then close | 1 then EOF |
| (parse failure on any of the above) | `(Diagnostic …)` | 1 |

**`EndOfReply` is the close-marker for queries.** A query
that returned three matches arrives as four expressions:
`(Node a [Alice])(Node b [Bob])(Node c [Charlie])(EndOfReply)`.
The marker lets the client know the query is complete without
needing to count expected matches. (It's the equivalent of
SMTP's dot-on-a-line and MPD's `OK`.)

---

## 5 · Question 3 — Reply form

**The question.** What does a reply look like? Same record-
shape as requests? Some `(Ok …)` `(Err …)` family? How do we
keep this from becoming privileged kind names?

**The answer.** **Replies are records in the same grammar as
requests. Successful queries return matching records as bare
asserts; edits return `(Ack [optional slot])`; failures
return `(Diagnostic …)`; subscription events return
`(Event <subscriptionSlot> <change>)`; query streams close
with `(EndOfReply)`.** All of these are PascalCase records,
identical in syntactic shape to user-asserted records.

**Rationale.**

The principle: a reply is a record the daemon happens to send
to the client. Nothing about its syntax distinguishes it from
a record the client could have sent; the *direction of flow*
is the only protocol-level distinction.

**The reply record kinds:**

| Kind | When emitted | Carries |
|---|---|---|
| `Ack` | After a successful edit (assert / mutate / retract / unsubscribe) | Optional `<slot>` for asserts that minted one |
| `(matching record)` | One per query match | The matched record verbatim |
| `EndOfReply` | After the last match of a query | nothing |
| `Diagnostic` | After any failure (parse, schema, ref, validation, permission) | level + code + message + site + suggestions |
| `Subscription` | After a successful `(Subscribe …)` | The subscription's slot |
| `Event` | When a subscribed pattern matches a new commit | subscription-slot + change-kind + record |
| `Goodbye` | After `(Goodbye)`, before close | nothing |

**Why these specific names:**

- `Ack` — short, present in MPD and many other protocols;
  unambiguous "your edit succeeded."
- `Diagnostic` — already exists in signal as a typed record
  (`signal/src/diagnostic.rs`); the text reply form is the
  textual rendering of the same struct.
- `Subscription` — what got opened.
- `Event` — what happens on a subscription.
- `EndOfReply` — explicit close marker for queries that
  return N items.
- `Goodbye` — clean disconnect ack.

**Why not `(Ok …)` / `(Err …)` style:**

`Ok`/`Err` are Rust idioms; in a record-oriented language they
read as low-information wrappers. `Ack` says specifically "I
applied your edit"; `Diagnostic` says specifically "here is
structured failure information." Each carries its own meaning.

**Are these "keywords"?**

By the prompt's definition — "no privileged kinds at the
validator" — these are not keywords. The validator never sees
them: they live entirely in the daemon's text-rendering layer.
Internally the daemon receives a typed `signal::Reply` from
criome and renders it to one of these record kinds for the
client. They are no more privileged than `Node`/`Edge`/`Graph`
are — they're just record kinds the client and daemon agree
on.

The criome validator's universe is the M0 set
(`Node`/`Edge`/`Graph`); it has no reason to ever encounter
an `Ack` or `Diagnostic` as input data. The daemon's universe
is the protocol set (`Ack`, `Diagnostic`, `Subscription`,
`Event`, `EndOfReply`, `Subscribe`, `Unsubscribe`, `Goodbye`)
plus the M0 set.

**A query example end to end:**

Client writes:

```
(| Node @id @label |)
```

Daemon parses; forwards to criome as a signal `Query`; criome
returns three matches; daemon renders:

```
(Node user [User])
(Node nexus [nexus daemon])
(Node criome [criome daemon])
(EndOfReply)
```

Client parses each top-level expression; sees `(EndOfReply)`;
knows the query is complete; sends the next request.

**An edit example:**

Client writes:

```
(Node lojix [lojix daemon])
```

Daemon parses; forwards to criome; criome accepts and assigns
slot 105; daemon renders:

```
(Ack 105)
```

Client knows the assert succeeded and learned the slot.

**A failure example:**

Client writes a malformed expression (unclosed paren):

```
(Node lojix [lojix daemon
```

The daemon's nexus parser fails. The daemon emits:

```
(Diagnostic Error E0001 [unclosed delimiter ( at offset 17] _ <> <>)
```

(Field positions: `level`, `code`, `message`, `primary_site`,
`context`, `suggestions`. `_` is the wildcard for "no
primary_site"; empty `<>` for empty `context` and
`suggestions`. The exact text rendering follows the canonical
nexus form for a `Diagnostic` record.)

The connection then closes — a parse failure mid-form means
the daemon cannot find the next form-boundary to recover.
(See §11 for the rationale.)

---

## 6 · Question 4 — Subscriptions / streaming

**The question.** A subscription is one request → many replies
over time. How do we mark "this reply belongs to subscription
X"? How do we signal end-of-stream / errors on a subscription?

**The answer.** **A subscription is opened by `(Subscribe
<pattern>)`. The daemon replies with `(Subscription <slot>)`.
Subsequent matching changes arrive as `(Event <slot> <change>)`
records. End-of-stream is `(EndOfSubscription <slot> <reason>)`.
Errors mid-stream are `(Diagnostic …)` records carrying the
subscription's slot in their `primary_site`.**

**The records:**

```
;; opening
(Subscribe (| Node @id @label |))

;; daemon ack
(Subscription 7)

;; events as they fire (Insert / Update / Retract are
;; PascalCase change kinds)
(Event 7 (Insert (Node user [User])))
(Event 7 (Update (Node nexus [nexus daemon — refactored])))
(Event 7 (Retract (Node user [User])))

;; daemon-initiated close
(EndOfSubscription 7 ConnectionClosing)

;; client cancellation
(Unsubscribe 7)

;; daemon ack of cancellation
(Ack)
```

**Rationale.**

The subscription's identity is its slot. The slot is minted
by criome and rendered as a bare integer (per the
`flow-graph.nexus` example's slot rendering). The client uses
the slot in `(Unsubscribe <slot>)` to cancel.

**Why a slot, not a fresh per-connection counter:** slots are
the universal identity in sema. A `Subscription` is a record
type criome can mint slots for; the slot is meaningful in
the same coordinate system as everything else. If a future
feature wants to query "what subscriptions are open?" the
answer is "query the `Subscription` records." (Whether the
subscription record is *durable* in sema or transient in
the daemon's memory is the call in D-4 below; the slot
identity works either way.)

**`Event` carries a change-kind:**

```
Event = (Event <subscriptionSlot> <Change>)
Change = (Insert <record>) | (Update <record>) | (Retract <record>)
```

`Insert` for a fresh assert that newly matches; `Update` for
a mutate of an already-matching record; `Retract` for a record
that previously matched but has been removed. This matches
the `signal::Reply` variants `SubAssert` / `SubMutate` /
`SubRetract` (per reports/082 §8) — translated to text-form
nested records.

**Initial snapshot:** a `(Subscribe …)` may carry an option
to receive the current matches before live diffs begin. The
shape:

```
(Subscribe (| Node @id @label |) WithSnapshot)
```

The daemon replies:

```
(Subscription 7)
(Event 7 (Insert (Node user [User])))
(Event 7 (Insert (Node nexus [nexus daemon])))
(SnapshotComplete 7)
;; ... live diffs from here on as Insert/Update/Retract
```

The `WithSnapshot` option is a bare ident-class token in
position 2 of the `Subscribe` record; if absent, no snapshot
is sent.

**`EndOfSubscription` reason:**

A typed enum-style identifier in position 2:

```
EndOfSubscription <slot> <Reason>
Reason = ClientUnsubscribed | ConnectionClosing
       | SubscriptionInvalidated
       | (LimitReached <kind> <limit>)
       | (Other <Diagnostic>)
```

Bare PascalCase tokens for the unit variants (`ClientUnsubscribed`,
`ConnectionClosing`, `SubscriptionInvalidated`); record form
for variants that carry data (`LimitReached`, `Other`).

(This is the textual face of the `SubEndReason` enum from
reports/082 §14.3 — no semantics added; only the rendering
changes.)

**Errors mid-subscription:** `(Diagnostic Warning E#### message
(Slot 7) <> <>)` — the `primary_site` field carries the
subscription's slot. The client matches on the diagnostic's
site to know which subscription is affected. The subscription
remains open after a warning-level diagnostic; an error-level
one closes the subscription with `(EndOfSubscription 7
(Other <Diagnostic>))`.

---

## 7 · Question 5 — Cancellation

**The question.** Can a client cancel an in-flight request?
What's the syntactic form?

**The answer.** **Subscriptions are cancelled by
`(Unsubscribe <slot>)`. Non-subscription requests are not
cancellable in M0** — they complete in sub-millisecond and
the server replies before any cancel could arrive. Long-
running operations are modelled as records-in-sema (per the
async-job pattern, §10), not as cancellable requests.

**Rationale.**

For M0:

- Asserts / mutates / retracts complete in sub-millisecond
  (flow-graph kinds, redb single-writer). Cancel is unobservable.
- Queries on M0 datasets (handfuls of records) complete
  similarly fast.
- Subscriptions are the only legitimately long-running
  operation; `(Unsubscribe <slot>)` handles them.

For post-M0:

- A `(Compile <opus>)` request takes seconds to minutes. The
  right shape (per reports/082 §10) is "Compile asserts a
  `BuildJob` record with status `Pending`; the client
  subscribes to the BuildJob's status; lojix updates the
  status to `Done` or `Failed`; the subscription fires."
  The original Compile request returned in milliseconds; the
  long-running work is observable through sema.

This shifts cancellation into the sema domain — the client
mutates the BuildJob's status to `Cancelled`, criome
notifies lojix, lojix aborts. No new wire concept.

**`Unsubscribe` syntax:**

```
(Unsubscribe 7)
```

A bare positional record with one field (the subscription
slot). Daemon replies `(Ack)` once the subscription is closed
on the criome side.

**Why not a `Cancel` record kind for general requests?**

Adding `(Cancel <correlationId>)` would require correlation
IDs on every request — exactly the wire-layer field §4 ruled
out. The right answer for non-subscription cancellation is
"the request will complete before you can cancel it"; for
genuinely long-running work, "the work is a record; the cancel
is a mutate."

---

## 8 · Question 6 — Errors

**The question.** When the daemon's parser rejects malformed
text, when criome rejects an edit, when a query returns no
matches — how does that surface? Plain text? A record? A
`(Diagnostic …)` form? Should the syntax be the same regardless
of failure stage?

**The answer.** **All failures surface as `(Diagnostic …)`
records.** The Diagnostic struct carries level + code + message
+ site + context + suggestions, identical at the text and
binary layers. A query with zero matches is not a failure —
it returns `(EndOfReply)` with no matches preceding it.

**Diagnostic codes (already specified in `signal/src/diagnostic.rs`):**

- `E0xxx` — parse failures (E0001 = unclosed delimiter,
  E0002 = unknown sigil, E0003 = trailing tokens)
- `E1xxx` — schema failures (unknown record kind, wrong
  field count, type mismatch in a position)
- `E2xxx` — reference failures (slot doesn't exist, slot
  refers to wrong kind)
- `E3xxx` — invariant failures (rule violation, integrity
  check)
- `E4xxx` — permission failures (capability missing,
  operator-uid mismatch)
- `E5xxx` — quorum / signature failures (post-MVP)
- `E6xxx` — cascade failures (downstream rule rejection)
- `E9xxx` — implementation gaps (`todo!()` paths)

**Same syntax at every stage:**

A parse failure looks the same as a validator failure looks
the same as a permission failure — they differ only in the
`code` field and the `primary_site` (which may be a source
span for parse errors and a slot for validator errors).
Clients learn one record shape and handle every error class
through it.

**Empty queries are not errors:**

```
(| Node nonexistent @label |)
;; reply:
(EndOfReply)
```

Zero matches followed by the close marker. No diagnostic.

**Multi-error replies:**

A `TxnBatch` (post-M0) may produce N diagnostics from one
failed transaction. Each renders as a `(Diagnostic …)`
record; `(EndOfReply)` closes the batch:

```
(Diagnostic Error E1001 [unknown kind Foo] (SourceSpan 0 13 [<txn>]) <> <>)
(Diagnostic Error E2001 [slot 99 does not exist] (Slot 99) <> <>)
(EndOfReply)
```

The single-failure case still emits `(EndOfReply)` for
shape consistency.

**Diagnostic record canonical text form:**

```
(Diagnostic <level> <code> <message> <primary_site> <context> <suggestions>)
```

Six positional fields. `<level>` is one of `Error`, `Warning`,
`Info`. `<code>` is the bracketed code string. `<message>` is
a bracketed human-readable string. `<primary_site>` is a
record (or `_` wildcard if none): `(Slot 42)` or `(SourceSpan
17 5 [<source>])` or `(OpInBatch 3)`. `<context>` is a
sequence `<("key" "value") ("key" "value")>`. `<suggestions>`
is a sequence of `(Suggestion …)` records.

(Bracketed strings use the existing nexus `[…]` form for
strings that contain whitespace.)

---

## 9 · Question 7 — Handshake / hello

**The question.** Does the protocol need an opening exchange?
How does the daemon communicate its protocol version (if at
all)?

**The answer.** **No handshake. The client opens the socket
and immediately starts sending requests.** No version
negotiation. No greeting line.

**Rationale.**

For an LLM agent, every byte of protocol ceremony is friction.
A handshake forces the LLM to remember to send it before any
real work; an LLM forgets, the daemon rejects, the LLM gets
confused about why a perfectly-fine request didn't work.
Handshakes are a category mistake when LLMs are the primary
client.

For version evolution, two mechanisms cover the long term:

1. **Backward-compatible additions.** New record kinds, new
   variants in enum-shaped record positions — both can land
   without breaking existing clients. The closed sigil budget
   means no syntactic-level change can break existing parsers.

2. **Path-based versioning if ever needed.** If a breaking
   change becomes truly necessary, the daemon binds a second
   socket at a versioned path (`/tmp/nexus-v2.sock`) alongside
   the original. Clients targeting v2 connect there. No
   handshake needed; the *socket path* is the version
   selector.

For M0, neither matters — there's only one version. The wire
just is what it is.

**What the daemon does on accept:**

1. Accept the UDS connection.
2. SO_PEERCRED check — verify the connecting uid is the
   operator's uid; close the socket if not.
3. Spawn a per-connection reader task.
4. Read top-level expressions; dispatch each.

No "Hello" frame; no version exchange; no greeting; no `+OK
nexus 1.0 ready`. A client that wants to verify the daemon
is alive sends `(\| Node _ _ \|)` (a no-cost query); a non-
empty reply or `(EndOfReply)` confirms.

---

## 10 · Question 8 — Pipelining / in-flight

**The question.** Can multiple requests be sent before any
reply arrives? Does the protocol require strict request/reply
alternation, or full async multiplexing?

**The answer.** **Pipelining is allowed; the daemon processes
requests serially per connection and replies in FIFO order.**
Multiple in-flight requests are fine on the wire; the daemon
queues them.

**Rationale.**

A client may send any number of requests back-to-back without
waiting for replies:

```
(Node a [Alice])(Node b [Bob])(Node c [Charlie])(| Node @id @label |)
```

The daemon reads them sequentially, processes them in order,
and replies in order:

```
(Ack 100)
(Ack 101)
(Ack 102)
(Node a [Alice])
(Node b [Bob])
(Node c [Charlie])
(EndOfReply)
```

This is the HTTP/1.x and Redis pipelining pattern. It removes
round-trip latency for batches of cheap requests.

**Concurrency on the daemon side:**

Per-connection: requests run serially. The reader task feeds
a per-connection async work queue; the worker processes one
request at a time, awaits criome's reply, writes the rendered
reply to the socket, then handles the next.

Across connections: each connection is its own tokio task;
the daemon serves N connections concurrently. criome itself
is a single-writer over redb — concurrent edits across
connections serialise at the criome level.

**Subscriptions don't block the queue:**

After `(Subscribe …)`, the daemon's writer task may emit
`(Event …)` records at any moment between regular replies.
The reader task is unaffected; new requests still queue
behind any in-flight one. Race-free because all writes to
a connection's socket go through one tokio task (the
per-connection writer); subscription events and request
replies serialise there.

---

## 11 · Question 9 — Connection lifecycle

**The question.** When does the connection close? On client
EOF? On a `(Goodbye)` record? On idle timeout? What happens
to in-flight subscriptions when a connection closes?

**The answer.** **Three close conditions:**

1. **Client EOF** — the client's end-of-file on read; the
   daemon reaps subscriptions and closes its end.

2. **Clean `(Goodbye)`** — client sends `(Goodbye)`; daemon
   replies `(Goodbye)`; both close. Symmetric clean shutdown.

3. **Daemon-initiated close** — on parse failure mid-form
   (E0001 unclosed delimiter), on SO_PEERCRED rejection at
   accept, on daemon shutdown.

**No idle timeout in M0.** Connections may stay open
indefinitely.

**Rationale.**

The kernel is the liveness arbiter (per reports/082 §11).
On UDS, EOF arrives immediately when the peer dies; no
timeout is needed.

**Subscription cleanup on close:**

When the connection closes (any cause), the daemon iterates
its per-connection subscription set and sends signal
`Unsubscribe { subscription_id }` requests to criome for
each. The subscriptions on the criome side are dropped; no
further events flow.

If the daemon initiated the close cleanly, it sends
`(EndOfSubscription <slot> ConnectionClosing)` for each
subscription before the `(Goodbye)` reply, so the client
sees a tidy ending. On dirty close (kernel EOF), the
events stop arriving and the client knows from the EOF
that subscriptions are gone.

**Why no idle timeout:**

Idle timeouts are for protecting servers from leaking
sockets when clients vanish without closing. UDS doesn't
have this problem — clients on the same machine that
crash are detected by the kernel within milliseconds. If
this becomes a concern post-M0 (say, because of a buggy
client process that holds sockets open), the right
solution is a per-process socket cap, not a per-connection
timeout.

**Why `(Goodbye)` exists:**

Letting the client signal "I'm done; please drain
subscriptions tidily" is friendlier than relying on
abrupt-close cleanup. The client gets a confirmation
that drain completed (via the daemon's `(Goodbye)` reply)
before the socket closes.

---

## 12 · Question 10 — Whitespace, comments, formatting

**The question.** nexus has `;;` line comments and free
whitespace. Does that pass through the protocol unchanged?
Does the daemon care about reply formatting?

**The answer.** **Whitespace and comments inside requests
are accepted and ignored. The daemon emits replies in
canonical nexus form (per `nexus/spec/grammar.md` Canonical
Form section): single-space separators, no comments, sorted
where the schema dictates.**

**Rationale.**

Inbound: clients are humans, LLMs, scripts — all of which
may format requests however they like. The daemon's parser
is the same nexus-serde parser used everywhere else; it
accepts the full grammar including comments.

Outbound: the daemon emits canonical form. Reasons:

- Predictability for clients (they know what to expect).
- Trivial to write tests that compare reply text.
- Round-tripping replies through nexus-serde is lossless.

**Specific canonical-form rules for replies:**

- Single ASCII space between adjacent tokens within a record.
- One newline between top-level reply expressions.
- No comments in replies.
- Strings: bare ident-class token if the value matches
  ident grammar; otherwise `[…]` bracketed form.
- Floats render shortest-roundtrip with mandatory `.`.
- Slots render as bare integers.
- `Option` fields: `_` wildcard for `None`; transparent for
  `Some(x)`.

**Inbound parse tolerance:**

The daemon accepts:

- Any whitespace (spaces, tabs, CRs, LFs) between top-level
  forms.
- `;;` comments anywhere whitespace is allowed.
- Mixed canonical/non-canonical formatting within records.

It does NOT accept:

- Anything outside the nexus grammar.
- Top-level expressions that aren't valid nexus forms.

A canonical-form-only mode is not specified for M0; if
performance matters later (skipping the comment-stripper
fast path), a flag could be added — but inbound is rare
enough that the tolerance has no measurable cost.

**An LLM-friendly aside:**

Because the daemon accepts comments, an LLM agent can write
self-annotating requests:

```
;; Asserting the criome flow-graph nodes in display order
(Node user   [User])
(Node nexus  [nexus daemon])
(Node criome [criome daemon])
```

The comments help the LLM keep track of what it's doing; the
daemon discards them on the way through. This matches how
nexus source files are written and read.

---

## 13 · Recommended minimal text-wire design

This section pulls the answers together. Names use PascalCase
per `Node`/`Edge`/`Graph` convention.

### 13.1 — The wire

A long-lived UDS connection between client and the nexus
daemon. The bytes on the wire are valid nexus text. No
length prefix, no envelope, no header.

Socket path: `/tmp/nexus.sock` (file mode `0600` owned by
the operator's uid; SO_PEERCRED check at accept time).

### 13.2 — Request grammar (subset of nexus grammar)

Anything the existing nexus grammar accepts as a top-level
form is a valid request. The daemon's interpretation:

| Top-level form | Interpretation |
|---|---|
| `(Kind …)` | Assert this record |
| `~(Kind …)` | Mutate to this record |
| `~(\| pattern \|) (replacement)` | Mutate-with-pattern |
| `!(Kind …)` | Retract this record |
| `(\| pattern \|)` | Query |
| `(\| pattern \|) { fields }` | Query with shape projection |
| `{\| (…) (…) \|}` | Query with constraint conjunction |
| `(Subscribe <pattern>)` | Open a subscription |
| `(Subscribe <pattern> WithSnapshot)` | Open with initial snapshot |
| `(Unsubscribe <slot>)` | Close a subscription |
| `(Goodbye)` | Clean disconnect |

`<pattern>` here is one of the pattern forms — `(\| … \|)`,
`{\| … \|}`, or a pattern with shape projection.

The daemon recognises `Subscribe`, `Unsubscribe`, and
`Goodbye` as protocol-level record kinds it dispatches on.
All other Pascal-named records are forwarded to criome as
edits or queries (depending on whether the form is a record
or a pattern).

### 13.3 — Reply grammar

Replies are top-level nexus expressions. The daemon emits
exactly the kinds in this table, in canonical form, one per
top-level expression.

| Reply kind | Shape | When |
|---|---|---|
| `Ack` | `(Ack)` or `(Ack <slot>)` | Successful edit / unsubscribe / goodbye-internal |
| (matching record) | `(Kind …)` | One per query match |
| `EndOfReply` | `(EndOfReply)` | After last query match (also after zero matches) |
| `Diagnostic` | `(Diagnostic Error <code> <msg> <site> <ctx> <sugg>)` | Any failure |
| `Subscription` | `(Subscription <slot>)` | After `(Subscribe …)` |
| `Event` | `(Event <slot> (Insert <record>))` etc. | Subscription event |
| `SnapshotComplete` | `(SnapshotComplete <slot>)` | End of initial snapshot in `WithSnapshot` mode |
| `EndOfSubscription` | `(EndOfSubscription <slot> <reason>)` | Subscription closed |
| `Goodbye` | `(Goodbye)` | Reply to `(Goodbye)` request |

Total: 9 reply-record kinds. Plus the matching records
themselves (which can be of any kind the schema knows).

### 13.4 — Connection lifecycle

```
client                                  daemon
  │                                       │
  ├─ open UDS /tmp/nexus.sock ──────────► │
  │                                       │ (accept; SO_PEERCRED check;
  │                                       │  spawn per-conn task)
  │                                       │
  ├─ (Node user [User]) ────────────────► │
  │                                       │
  │ ◄────────────────── (Ack 100) ─────── │
  │                                       │
  ├─ (Subscribe (| Node @id @lbl |)) ───► │
  │                                       │
  │ ◄────────────── (Subscription 5) ──── │
  │                                       │
  ├─ (Node nexus [nexus daemon]) ───────► │
  │                                       │
  │ ◄──────────────── (Ack 101) ───────── │
  │ ◄── (Event 5 (Insert (Node nexus [nexus daemon]))) ─── (subscription fires)
  │                                       │
  ├─ (| Node @id @lbl |) ───────────────► │
  │                                       │
  │ ◄────────── (Node user [User]) ────── │
  │ ◄────── (Node nexus [nexus daemon]) ─ │
  │ ◄─────────── (EndOfReply) ─────────── │
  │                                       │
  ├─ (Unsubscribe 5) ───────────────────► │
  │                                       │
  │ ◄────────────── (Ack) ─────────────── │
  │                                       │
  ├─ (Goodbye) ─────────────────────────► │
  │                                       │
  │ ◄────────── (Goodbye) ─────────────── │
  │                                       │
  ─── socket closes ───
```

### 13.5 — The daemon's main loop

```
nexus daemon main loop (per connection):

  reader task:
    loop {
      expr = nexus_serde::read_one_expression(socket)
      if expr is parse failure:
        write (Diagnostic Error E0001 …) to socket
        close socket; break
      enqueue expr in work_queue
    }

  worker task:
    loop {
      expr = work_queue.recv()
      match expr:
        (Subscribe pattern [WithSnapshot]):
          send signal::Subscribe to criome over /tmp/criome.sock
          await SubReady; mint Subscription slot
          spawn subscription_relay task for that slot
          write (Subscription <slot>) to writer
          if WithSnapshot:
            await SubSnapshot; emit (Event <slot> (Insert <record>)) for each
            emit (SnapshotComplete <slot>)

        (Unsubscribe slot):
          send signal::Unsubscribe to criome
          await Ok
          write (Ack) to writer

        (Goodbye):
          drain pending; emit (EndOfSubscription …) for each open sub
          send signal::Goodbye to criome (drops criome side state)
          write (Goodbye) to writer; close

        any other top-level expr:
          parse as signal request via nexus-serde + classification:
            bare record → signal::Assert
            ~record → signal::Mutate
            !record → signal::Retract
            pattern → signal::Query
          send signal request to criome
          await reply
          render reply to nexus text:
            Ok(slot)              → (Ack <slot>)
            Ok(no_slot)           → (Ack)
            Rejected(diagnostic)  → (Diagnostic …)
            QueryHit(records)     → one (Kind …) per record + (EndOfReply)
            ValidateResult(…)     → either (Ack) or (Diagnostic …)
          write rendered reply to writer
    }

  subscription_relay task (one per active subscription):
    loop {
      sub_event = await criome over signal channel
      match sub_event:
        SubAssert(record)  → (Event <slot> (Insert <record>))
        SubMutate(rec_new) → (Event <slot> (Update <rec_new>))
        SubRetract(record) → (Event <slot> (Retract <record>))
        SubError(diag)     → (Diagnostic Warning … (Slot <slot>) …)
        SubEnd(reason)     → (EndOfSubscription <slot> <reason>)
                              and exit loop
      write rendered to writer (serialised through writer task)
    }

  writer task:
    loop {
      msg = writer_channel.recv()
      socket.write_all(msg).await
    }
```

The writer task serialises all writes (request replies +
subscription events + diagnostics) through one channel so
they don't interleave at the byte level. The reader task
parses the inbound stream and never writes; the worker task
processes one request at a time per connection.

### 13.6 — What signal needs (very little)

Signal is unchanged for the client-facing leg because it's
not on it. The internal nexus daemon ↔ criome leg keeps
exactly the design from reports/082.

The only signal change suggested by this report is dropping
the `client_msg` module entirely from the `nexus` crate (per
reports/082 §15) — that module was the prior "client-facing
binary protocol" attempt; the text wire replaces it.

Specifically:

| Module | Status |
|---|---|
| `nexus/src/client_msg/` | Delete entirely |
| `signal/src/{frame,request,reply,...}` | Unchanged |
| `nexus-cli/src/main.rs` | Rewrite as a thin shell: read argv-as-text or stdin-as-text, open UDS, write nexus text, read until `(EndOfReply)` or `(Ack …)` or `(Diagnostic …)`, print to stdout, exit |

### 13.7 — A self-contained example

A complete LLM-agent session asserting nodes, querying them,
opening a subscription, and disconnecting:

```nexus
;; Open: client connects to /tmp/nexus.sock
;; (no handshake)

;; First, assert three nodes
(Node user   [User])
(Node nexus  [nexus daemon])
(Node criome [criome daemon])
;; Daemon emits, in order:
;;   (Ack 100)
;;   (Ack 101)
;;   (Ack 102)

;; Now query everything
(| Node @id @label |)
;; Daemon emits:
;;   (Node user [User])
;;   (Node nexus [nexus daemon])
;;   (Node criome [criome daemon])
;;   (EndOfReply)

;; Open a live subscription with initial snapshot
(Subscribe (| Node @id @label |) WithSnapshot)
;; Daemon emits:
;;   (Subscription 50)
;;   (Event 50 (Insert (Node user [User])))
;;   (Event 50 (Insert (Node nexus [nexus daemon])))
;;   (Event 50 (Insert (Node criome [criome daemon])))
;;   (SnapshotComplete 50)

;; Insert a fourth node — the subscription fires
(Node lojix [lojix daemon])
;; Daemon emits:
;;   (Ack 103)
;;   (Event 50 (Insert (Node lojix [lojix daemon])))

;; Close the subscription
(Unsubscribe 50)
;; Daemon emits:
;;   (Ack)

;; Disconnect cleanly
(Goodbye)
;; Daemon emits:
;;   (Goodbye)
;; Then closes the socket.
```

---

## 14 · Comparison table — what each protocol gives, what we keep

| Concern | nREPL | SLIME | Geiser | RESP | HTTP/1.x | LSP | MPD | nexus text wire |
|---|---|---|---|---|---|---|---|---|
| Framing | bencode self-delim | length-prefix + s-expr | balanced parens + sentinel | type-prefix + CRLF | header + body | length-prefix + JSON | newline | balanced parens (schema-as-framing) |
| Correlation | `:id` field | integer in s-expr | none (alternation) | pipeline order | pipeline order | `id` field | pipeline order | pipeline order |
| Streaming | `:status` flow | `:write-string` interleaved | n/a | pub/sub mode | chunked + SSE | `$/progress` token | `idle` blocks | `Event` records carrying subscription slot |
| Cancellation | `interrupt` op | `:emacs-interrupt` | n/a | close socket | close socket | `$/cancelRequest` notification | `noidle` | `Unsubscribe` for subs; long-running work via record-in-sema |
| Errors | `:status ["error"]` | `:debug` form | error in result | `-ERR …` | status code | `error` field | `ACK` line | `(Diagnostic …)` record |
| Handshake | none | none | none | none | request line | initialize req | none | none |
| Comments | n/a | n/a | scheme `;` | n/a | n/a | n/a | n/a | nexus `;;` (parsed, ignored) |

The nexus-text-wire row is the minimum that uses nothing
outside the existing nexus grammar.

---

## 15 · What this report does *not* propose

- **A parallel binary client interface.** Signal stays
  internal-only. Clients that want binary speed should
  speak signal directly (which is already an option for
  Rust callers), not via a separate "nexus binary" wire.

- **TLS or any encryption.** Local UDS only. The OS is the
  security boundary.

- **An authentication record.** SO_PEERCRED is sufficient
  for M0; multi-party identity is a post-MVP concern with
  its own design.

- **Compression.** Text is plenty fast over a local socket
  for M0 scale. Adding gzip would create a "did the client
  send compressed?" handshake we're avoiding.

- **A `Help` or `ListKinds` introspection request.** Clients
  should ship the schema (or read `nexus/spec/`). The
  daemon doesn't expose self-description verbs.

- **Per-request quotas / deadlines / budgets.** No `Timeout`
  field, no `MaxRows` clause. Client-side concerns; if a
  query is too big, the client closes the socket.

- **Output format negotiation.** Replies are always
  canonical nexus form. No JSON mode, no machine-vs-human
  toggle, no "pretty-print" option.

---

## 16 · Risks

| Risk | Mitigation |
|---|---|
| Pipelining without correlation breaks if a single request takes very long | M0 requests are sub-millisecond; long-running work is record-in-sema (Compile + BuildJob pattern). Connections are cheap; clients open a second one if they need to overlap a slow query with edits. |
| Subscription events interleaved with replies can confuse a client expecting strict alternation | `Event` records are distinguishable by their kind name; clients dispatch by `(Event …)` vs everything else. The slot in the Event is the subscription marker. |
| Parse failure mid-form forces a connection close | Yes, by design. The daemon cannot recover the next form-boundary from invalid syntax. Clients reconnect; idempotent operations are safe to retry. |
| Subscription event ordering vs reply ordering not strictly defined | The writer task serialises both through one channel; "first written wins" is the only guarantee. Clients should not assume an event-vs-reply ordering. |
| Diagnostic codes evolve | The code is a `String` field per `signal/src/diagnostic.rs`; new codes land additively. Clients matching on code prefix (`E1xxx` for schema) are stable across new specific codes. |
| LLM agent gets confused about which open subscriptions it has | The LLM can query them by sending `(\| Subscription @slot \|)` if subscriptions are records-in-sema (D-4 below); otherwise, the LLM tracks them locally. |
| Two clients writing the same canonical-form output but with different bracket choices for strings | The canonical-form rules in `nexus/spec/grammar.md` are deterministic: bare ident-class → bare; otherwise `[…]`. nexus-serde implements the rule once; both reader and writer use it. |
| Clients accidentally embed `(Goodbye)` in user-supplied text and trigger a disconnect | Strings inside `[…]` are not parsed as records — the bracket form is opaque to the dispatch layer. Only top-level `(Goodbye)` triggers the close. |

---

## 17 · Source acknowledgements

Specifications and source consulted:

- nREPL: `nrepl.org/nrepl/1.1/design/transports.html`;
  `github.com/nrepl/nrepl/blob/master/src/clojure/nrepl/transport.clj`.
- SLIME / SWANK: `common-lisp.net/project/slime/doc/html/Communication-with-Emacs.html`;
  the SLIME source `swank/swank.lisp`.
- Geiser: `geiser.nongnu.org/manual/geiser.html`.
- Redis RESP: `redis.io/topics/protocol`; the Redis source
  `src/networking.c`.
- HTTP/1.1: `datatracker.ietf.org/doc/html/rfc9112`;
  Server-Sent Events: `html.spec.whatwg.org/multipage/server-sent-events.html`.
- LSP: `microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/`;
  JSON-RPC 2.0: `jsonrpc.org/specification`.
- Plan 9 con: `9p.io/sys/man/8/con.html`.
- SMTP: `datatracker.ietf.org/doc/html/rfc5321`;
  IRC: `datatracker.ietf.org/doc/html/rfc1459`;
  NNTP: `datatracker.ietf.org/doc/html/rfc3977`.
- MPD: `mpd.readthedocs.io/en/latest/protocol.html`.
- Erlang ports: `erlang.org/doc/man/erl_interface.html`;
  `erlang.org/doc/apps/erts/erl_ext_dist.html`.
- SO_PEERCRED behaviour: `man 7 unix` (Linux).

Internal references:

- `criome/ARCHITECTURE.md` §3 (request flow), §10 (project
  rules: closed sigil budget; PascalCase records; sigils as
  last resort).
- `nexus/spec/grammar.md` (canonical grammar).
- `nexus/spec/examples/flow-graph.nexus`,
  `patterns-and-edits.nexus` (worked examples).
- `signal/src/diagnostic.rs` (Diagnostic struct + codes).
- `mentci/reports/082` (binary protocol on the internal leg —
  applies unchanged here).
- `mentci/reports/078` §1.3 (UDS framing research),
  M0 / M5 milestones.

---

## 18 · Decision points for Li

Five choices Li resolves before the wire ships. Each has 2-3
options, pros and cons, and a lean.

### D-1 — `(EndOfReply)` for single-record replies?

**Background.** Edits return `(Ack [optional slot])`; queries
return N records plus `(EndOfReply)`. Should single-record
edits also emit `(EndOfReply)`? It would make every request
have a uniform "exactly one closing marker" convention.

**Options.**

- (A) **`(EndOfReply)` only after multi-record replies (queries).**
  The current §5/§13 proposal. Edits stand alone as `(Ack …)`.
- (B) **`(EndOfReply)` after every reply, always.** Uniform
  shape; clients always read until `(EndOfReply)` regardless
  of request kind. Costs one extra expression per edit.
- (C) **`(EndOfReply)` only when zero records returned by a
  query; multi-record queries terminate naturally as the
  client knows to expect them.** Brittle — clients have to
  know the query's result count in advance.

**Lean: (A).** Edits have a natural single-reply shape; `Ack`
already conveys "this is the response, and there is one of
them." Queries genuinely need a terminator because their
count is open. Differentiating is honest.

### D-2 — Subscription identity: criome-minted slot vs daemon-local counter?

**Background.** §6 proposes the `Subscription`'s identity is
its slot (criome-minted, sema-coordinated). Alternative: the
daemon mints a per-connection counter (1, 2, 3, …) and the
slot is hidden behind it.

**Options.**

- (A) **Slot from criome (records-in-sema for subscriptions).**
  Subscription is a real record. Queryable; introspectable;
  durable across daemon restart (post-MVP). One extra sema
  write per Subscribe.
- (B) **Daemon-local `u64` counter.** Transient; per-
  connection; gone on disconnect. No sema involvement. Same
  pattern as reports/082 §8.
- (C) **Hybrid: daemon mints; on first event, record is
  promoted to sema if requested.** Optional persistence.

**Lean: (B) for M0.** Promoting subscriptions to sema records
is a real design choice with implications for the schema; the
transient form is sufficient for the immediate need. (Same
lean as reports/082 D-4. The slot rendering in text doesn't
change between A and B — both render as bare integers.)

### D-3 — `(Subscribe (\| pattern \|) WithSnapshot)` form: ident-token option vs explicit record?

**Background.** §6 proposes `(Subscribe <pattern> WithSnapshot)`
where `WithSnapshot` is a bare ident token in position 2.
Alternative: encode the option as a nested record.

**Options.**

- (A) **`WithSnapshot` as a bare ident.** Concise. Reads as
  natural English. Schema: `Subscribe { pattern: Pattern,
  options: SubscribeOptions }` where options is a unit
  variant enum (`Default | WithSnapshot`).
- (B) **`(Subscribe <pattern> (Options Snapshot))` nested
  record.** More verbose; clearer that the second position
  carries a structured option. Easier to extend with new
  options (fan-out, max-events, etc.).
- (C) **Two distinct records: `(Subscribe …)` and
  `(SubscribeWithSnapshot …)`.** No options at all; explicit
  per-feature. Most explicit; doesn't scale to combinations
  of options.

**Lean: (A) for one option; reconsider when a second option
appears.** Bare-ident enum variants in nexus follow the
existing grammar's pattern (per `flow-graph.nexus`'s
`Edge.label = None`). Single-option case is clean; if more
options arrive, switch to (B) without breaking compatibility
(the parser can accept either an ident or a record in
position 2).

### D-4 — The `Subscription` reply: bare slot or full record?

**Background.** §6 says `(Subscription <slot>)` — a record
carrying just the slot. Alternative: `(Subscription <slot>
<patternEcho>)` — also echoes the pattern that was opened.

**Options.**

- (A) **`(Subscription <slot>)`.** Minimal; client already
  knows the pattern (it sent it). One field.
- (B) **`(Subscription <slot> <pattern>)`.** Echoes pattern;
  clients with multiple opens-in-flight don't have to track
  request order to learn which slot belongs to which
  pattern. Adds rendering cost.
- (C) **`(Subscription <slot>)` + a `(\| Subscription @slot
  @pattern \|)` queryable view in sema.** Lookup on demand.

**Lean: (A).** The client just sent the pattern; echoing
is redundant. With FIFO ordering on the connection, the
matching is unambiguous. (B) becomes attractive only if
post-M0 we add async multiplexing of subscribe requests.

### D-5 — Daemon's parse-failure recovery: close vs skip-to-next-form?

**Background.** §11 proposes that a parse failure mid-form
closes the connection. Alternative: skip ahead to the next
plausible form-boundary and continue, letting the client
recover without reconnecting.

**Options.**

- (A) **Close on parse failure.** Simple; honest; client
  reconnects. Loses zero-cost recovery for typos.
- (B) **Skip to next top-level form.** Daemon scans for a
  balanced top-level open delimiter (`(`, `~`, `!`, `{`,
  `*` if added, etc.) and resumes parsing. Requires
  heuristics; risk of misalignment cascading into more
  failures.
- (C) **Emit diagnostic + send a `(SyncMarker)` request from
  the client to resume.** Explicit reset signal; client
  controls when recovery happens.

**Lean: (A) for M0.** The simplest behaviour; LLMs producing
malformed nexus is rare-enough-to-ignore (they have schema
context). Reconnect cost on UDS is microseconds. Revisit if
a use case for recovery surfaces.

---

*End report 084.*
