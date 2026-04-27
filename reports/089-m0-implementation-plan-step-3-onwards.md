# 089 — M0 implementation plan, steps 3 → 7

*Detailed plan for the remaining M0 work after step 1 (signal
rewrite — done, 18 tests) and step 2 (sema body — done, 7 tests).
Companion to 088. Five steps left, with shape, code sketches,
tests, decisions surfaced during planning.*

---

## 1 · Recap of what's done

- [signal rewrite](../repos/signal/src/) per Invariant D: per-verb
  typed payloads (`AssertOp`, `MutateOp`, `QueryOp`, `Records`),
  paired Query kinds (`NodeQuery`, `EdgeQuery`, `GraphQuery`,
  `KindDeclQuery`), `KindDecl` schema-as-data type.
  18 round-trip tests cover every verb shape end-to-end.
- [sema body](../repos/sema/src/lib.rs): `Sema::open / store /
  get` over a redb file, monotone slot counter starting at
  `SEED_RANGE_END = 1024`, persistent across reopens. 7 tests.

Both pushed to main on their respective repos.

---

## 2 · Step 3 — criome body (~150 LoC)

The daemon: UDS accept loop, length-prefixed Frame I/O, dispatch
per Request variant, sema integration.

### 2.1 Sema kind-tag revision — RETIRED

Earlier draft proposed prepending a u8 kind-tag to sema payload
bytes so criome could enumerate records by kind. **Retired** —
the per-verb typed-payload design (per [088](088-closed-vs-open-schema-research.md))
makes this unnecessary: criome dispatches each `AssertOp` variant
to its kind-specific store path (per-kind tables, when they
arrive in M1+). Sema's current `store(&[u8]) → Slot` and
`get(Slot) → Option<Vec<u8>>` API stays as-is for M0; per-kind
storage discipline lands when more than 4 kinds exist.

### 2.2 Criome layout

```
criome/src/
├── main.rs       — entry: open sema, bind UDS, accept loop
├── lib.rs        — re-exports + Result type
├── error.rs      — existing; add new variants
├── uds.rs        — Listener wrapper around tokio UnixListener
├── dispatch.rs   — NEW: Request → Reply
├── kinds.rs      — NEW: KIND_NODE/EDGE/GRAPH/KINDDECL u8 consts
├── handshake.rs  — NEW: handshake handler (small)
├── assert.rs     — NEW: AssertOp dispatch + sema.store
├── query.rs      — NEW: QueryOp dispatch + matcher
└── validator/    — existing stubs (unchanged for M0)
```

### 2.3 Code shape

```rust
// criome/src/main.rs
use std::path::PathBuf;
use std::sync::Arc;
use criome::{uds::Listener, Result};
use sema::Sema;

#[tokio::main]
async fn main() -> Result<()> {
    let socket_path = "/tmp/criome.sock";
    let sema_path: PathBuf = std::env::var("SEMA_PATH")
        .unwrap_or_else(|_| "/tmp/sema.redb".into())
        .into();
    let sema = Arc::new(Sema::open(&sema_path)?);
    Listener::bind(socket_path).await?.run(sema).await
}

// criome/src/uds.rs
pub struct Listener { listener: tokio::net::UnixListener }

impl Listener {
    pub async fn bind(path: &str) -> Result<Self> {
        let _ = std::fs::remove_file(path);  // clear stale socket
        Ok(Listener { listener: tokio::net::UnixListener::bind(path)? })
    }

    pub async fn run(self, sema: Arc<Sema>) -> Result<()> {
        loop {
            let (socket, _) = self.listener.accept().await?;
            let sema = sema.clone();
            tokio::spawn(async move {
                let _ = handle_connection(socket, sema).await;
            });
        }
    }
}

async fn handle_connection(mut socket: UnixStream, sema: Arc<Sema>) -> Result<()> {
    loop {
        let frame = read_frame(&mut socket).await?;
        let reply = dispatch::handle(frame, &sema);
        write_frame(&mut socket, reply).await?;
    }
}

// length-prefixed Frame I/O — 4-byte BE u32 + N rkyv bytes
async fn read_frame(socket: &mut UnixStream) -> Result<Frame> { ... }
async fn write_frame(socket: &mut UnixStream, frame: Frame) -> Result<()> { ... }
```

```rust
// criome/src/dispatch.rs
pub fn handle(frame: Frame, sema: &Sema) -> Frame {
    let reply = match frame.body {
        Body::Request(req) => process(req, sema),
        Body::Reply(_) => return reject(),  // criome doesn't process replies
    };
    Frame { principal_hint: None, auth_proof: None, body: Body::Reply(reply) }
}

fn process(req: Request, sema: &Sema) -> Reply {
    match req {
        Request::Handshake(h)    => handshake::handle(h),
        Request::Assert(op)      => assert::handle(op, sema),
        Request::Query(op)       => query::handle(op, sema),
        Request::Mutate(_)       => deferred("Mutate", "M1"),
        Request::Retract(_)      => deferred("Retract", "M1"),
        Request::AtomicBatch(_)  => deferred("AtomicBatch", "M1"),
        Request::Subscribe(_)    => deferred("Subscribe", "M2"),
        Request::Validate(_)     => deferred("Validate", "M1"),
    }
}

fn deferred(verb: &str, milestone: &str) -> Reply {
    Reply::Outcome(OutcomeMessage::Diagnostic(Diagnostic {
        level: DiagnosticLevel::Error,
        code: "E0099".into(),
        message: format!("{verb} verb not implemented in M0; planned for {milestone}"),
        primary_site: None,
        context: vec![],
        suggestions: vec![],
        durable_record: None,
    }))
}
```

```rust
// criome/src/assert.rs
pub fn handle(op: AssertOp, sema: &Sema) -> Reply {
    let (kind_tag, bytes_result) = match op {
        AssertOp::Node(n)     => (kinds::NODE,      encode(&n)),
        AssertOp::Edge(e)     => (kinds::EDGE,      encode(&e)),
        AssertOp::Graph(g)    => (kinds::GRAPH,     encode(&g)),
        AssertOp::KindDecl(k) => (kinds::KIND_DECL, encode(&k)),
    };
    match bytes_result.and_then(|b| sema.store(kind_tag, &b).map_err(...)) {
        Ok(_slot) => Reply::Outcome(OutcomeMessage::Ok(Ok {})),
        Err(e)    => Reply::Outcome(OutcomeMessage::Diagnostic(...)),
    }
}

fn encode<T>(value: &T) -> Result<Vec<u8>>
where T: rkyv::Serialize<...>,
{
    rkyv::to_bytes::<rkyv::rancor::Error>(value)
        .map(|b| b.to_vec())
        .map_err(|e| Error::Encode(e.to_string()))
}
```

```rust
// criome/src/query.rs
pub fn handle(op: QueryOp, sema: &Sema) -> Reply {
    let result = match op {
        QueryOp::Node(q)     => find_nodes(sema, q).map(Records::Node),
        QueryOp::Edge(q)     => find_edges(sema, q).map(Records::Edge),
        QueryOp::Graph(q)    => find_graphs(sema, q).map(Records::Graph),
        QueryOp::KindDecl(q) => find_kind_decls(sema, q).map(Records::KindDecl),
    };
    match result {
        Ok(records) => Reply::Records(records),
        Err(e)      => Reply::Outcome(OutcomeMessage::Diagnostic(...)),
    }
}

fn find_nodes(sema: &Sema, q: NodeQuery) -> Result<Vec<Node>> {
    let mut out = Vec::new();
    for entry in sema.iter_kind(kinds::NODE)? {
        let (_slot, bytes) = entry?;
        let node: Node = decode(&bytes)?;
        if matches_node(&node, &q) {
            out.push(node);
        }
    }
    Ok(out)
}

fn matches_node(node: &Node, query: &NodeQuery) -> bool {
    matches_pattern_field(&node.name, &query.name)
}

fn matches_pattern_field<T: PartialEq>(value: &T, pattern_field: &PatternField<T>) -> bool {
    match pattern_field {
        PatternField::Wildcard | PatternField::Bind(_) => true,
        PatternField::Match(literal) => value == literal,
    }
}
```

### 2.4 Tests for criome

Three integration tests with a temp sema file and direct
`dispatch::handle` calls (no UDS round-trip needed for unit
correctness — the UDS path is exercised by the end-to-end
test in step 5):

1. `assert_node_then_query_finds_it` — Assert + Query Wildcard
   returns the node.
2. `assert_three_kinds_query_filters_correctly` — Assert
   Node + Edge + Graph; QueryOp::Node returns only the node.
3. `query_with_match_filters_by_value` — Assert two Nodes
   with different names; Query Match returns only the
   matching one.
4. `unimplemented_verb_returns_e0099` — Mutate returns
   Diagnostic E0099.

Plus the existing 18 signal tests still cover the Frame
round-trip path.

---

## 3 · Step 4 — RETIRED (parser landed)

The original §3 / §3.1 dramatised PatternField dispatch as a
"hard part." It wasn't. See
[091](091-pattern-rethink.md) for the corrected design and
[`nexus/src/parse.rs`](../repos/nexus/src/parse.rs) for the
actual implementation (`QueryParser` type, ~240 LoC including
helpers, 24 tests). PatternField::Bind carries no payload (the
bind name is the schema field name at that position); the
parser validates `@<name>` against the expected schema field
name and rejects mismatches.

Step 4 is folded into step 5 (the daemon body) — there is no
separate parser-kernel work needed.

---

## 4 · Step 5 — nexus daemon body (~200 LoC)

### 4.1 Files

```
nexus/src/
├── main.rs    — entry: bind /tmp/nexus.sock, accept, spawn handler
├── lib.rs     — existing: error + module re-exports
├── error.rs   — existing
├── handler.rs — NEW: per-connection handler (text in/out)
├── parse.rs   — NEW: text → typed Request (uses nota-serde-core for asserts; hand-written for queries per §3)
└── render.rs  — NEW: typed Reply → text (uses nota-serde-core)
```

### 4.2 Per-connection flow

```rust
async fn handle_conn(mut client_sock: UnixStream) -> Result<()> {
    // Open a paired criome connection for this client session
    let mut criome = UnixStream::connect("/tmp/criome.sock").await?;
    do_handshake(&mut criome).await?;

    let mut text_buffer = String::new();
    let mut read_buf = [0u8; 4096];

    loop {
        let n = client_sock.read(&mut read_buf).await?;
        if n == 0 { break; }
        text_buffer.push_str(std::str::from_utf8(&read_buf[..n])?);

        // Drain complete top-level expressions
        loop {
            match parse::next_top_level(&text_buffer)? {
                None => break,  // need more bytes
                Some((request, consumed)) => {
                    let reply = exchange(&mut criome, request).await?;
                    let response = render::reply(reply)?;
                    client_sock.write_all(response.as_bytes()).await?;
                    client_sock.write_all(b"\n").await?;
                    text_buffer.drain(..consumed);
                }
            }
        }
    }
    Ok(())
}
```

### 4.3 Parser dispatch

`parse::next_top_level` recognizes the verb from the leading
sigil/delimiter:

```
(Foo …)        → Request::Assert(AssertOp::Foo(...))     via nota-serde::from_str_nexus
(| Foo …|)     → Request::Query(QueryOp::Foo(...))       via hand-written §3 parser
~(Foo …)       → Request::Mutate(MutateOp::Foo{...})     via nota-serde-nexus (sentinel-based existing)
!slot          → Request::Retract(RetractOp{slot, ...})  hand-written
[| op1 op2 |]  → Request::AtomicBatch(...)               via nota-serde existing
?(...)         → Request::Validate(...)                  via nota-serde existing
*(| ... |)     → Request::Subscribe(...)                 hand-written wrapper around §3
```

For M0 only `(...)` and `(| ...|)` need to actually work; the
others can return `Diagnostic E0099` from the daemon side
(mirroring criome's deferred verbs from §2.3).

### 4.4 Reply rendering

```rust
pub fn reply(reply: Reply) -> Result<String> {
    match reply {
        Reply::HandshakeAccepted(_) => Ok("(Ok)".to_string()),  // collapsed for client
        Reply::HandshakeRejected(r) => Ok(render_handshake_reject(r)),

        Reply::Outcome(OutcomeMessage::Ok(_))            => Ok("(Ok)".into()),
        Reply::Outcome(OutcomeMessage::Diagnostic(d))    => Ok(render_diagnostic(&d)?),

        Reply::Outcomes(items) => {
            // sequence of (Ok) / (Diagnostic …) — emit as [(Ok) (Diag) …]
            let mut s = String::from("[");
            for (i, item) in items.iter().enumerate() {
                if i > 0 { s.push(' '); }
                s.push_str(&render_outcome_message(item)?);
            }
            s.push(']');
            Ok(s)
        }

        Reply::Records(Records::Node(ns))    => render_typed_seq(&ns),
        Reply::Records(Records::Edge(es))    => render_typed_seq(&es),
        Reply::Records(Records::Graph(gs))   => render_typed_seq(&gs),
        Reply::Records(Records::KindDecl(k)) => render_typed_seq(&k),
    }
}

fn render_typed_seq<T: serde::Serialize>(items: &[T]) -> Result<String> {
    // NO HARDCODING (per criome ARCH Invariant D + AGENTS.md):
    // all rendering of typed Rust values goes through
    // nota-serde-core, never hardcoded text strings.
    nota_serde_core::to_string_nexus(items).map_err(|e| ...)
}
```

This honors the [criome arch Invariant D + Q6 decision](../repos/criome/ARCHITECTURE.md#invariant-d):
all rendering goes through nota-serde-core, never hardcoded.

### 4.5 Tests

- Unit tests on `parse::next_top_level` covering each verb
  shape's text → Request roundtrip (against the example
  flow-graph.nexus content).
- Unit tests on `render::reply` covering Ok / Diagnostic /
  typed Records.
- Integration test (in step 5 closing): full daemon spin-up,
  connect, send `(Node "User")`, expect `(Ok)` response.

---

## 5 · Step 6 — nexus-cli (~30 LoC)

```rust
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn main() -> Result<()> {
    let arguments: Vec<String> = std::env::args().collect();
    let input = match arguments.get(1) {
        Some(file) if file != "-" => std::fs::read_to_string(file)?,
        _ => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
    };

    let mut socket = UnixStream::connect("/tmp/nexus.sock")?;
    socket.write_all(input.as_bytes())?;
    socket.shutdown(std::net::Shutdown::Write)?;

    let mut response = String::new();
    socket.read_to_string(&mut response)?;
    print!("{response}");
    Ok(())
}
```

No tokio, no signal, no parser deps. Pure shuttle. The
[nexus-cli ARCHITECTURE.md](../repos/nexus-cli/ARCHITECTURE.md)
calls this out: *"Text is text. nexus-cli does not parse nexus;
it just shuttles bytes."*

Also note: that arch doc still has the stale `client_msg`
stale references — now fixed in the doc-cleanup pass;
fixing those concurrently with this step would be opportunistic.

---

## 6 · Step 7 — `genesis.nexus` (~30 LoC text)

A text file shipped with the criome binary (in
`criome/genesis.nexus`). At first boot, criome's main.rs
checks an empty sema, dispatches genesis through the same
Assert path that user data uses, and KindDecl records land in
sema.

```nexus
;; genesis.nexus — bootstrap KindDecls for the v0.0.1 schema.
;; Asserted by criome at first boot via the same Assert path
;; user data takes. Self-describing: KindDecl is itself the
;; first kind declared.

(KindDecl "KindDecl"
  [(FieldDecl "name"   "String"    One)
   (FieldDecl "fields" "FieldDecl" Many)])

(KindDecl "FieldDecl"
  [(FieldDecl "name"        "String"      One)
   (FieldDecl "type-name"   "String"      One)
   (FieldDecl "cardinality" "Cardinality" One)])

(KindDecl "Node"
  [(FieldDecl "name" "String" One)])

(KindDecl "Edge"
  [(FieldDecl "from" "Slot"         One)
   (FieldDecl "to"   "Slot"         One)
   (FieldDecl "kind" "RelationKind" One)])

(KindDecl "Graph"
  [(FieldDecl "title"     "String" One)
   (FieldDecl "nodes"     "Slot"   Many)
   (FieldDecl "edges"     "Slot"   Many)
   (FieldDecl "subgraphs" "Slot"   Many)])
```

**Bootstrap mechanic** (in criome main.rs):

```rust
async fn maybe_run_genesis(sema: &Sema) -> Result<()> {
    if sema.iter_kind(kinds::KIND_DECL)?.next().is_some() {
        return Ok(());  // already initialized
    }
    let genesis_text = include_str!("../genesis.nexus");
    // Parse + dispatch each KindDecl through normal Assert path
    for kind_decl in parse_all_asserts(genesis_text)? {
        let _ = assert::handle(kind_decl, sema);
    }
    Ok(())
}
```

This means M0 has TWO parse paths in criome:
- The wire path (signal Frame from network)
- The genesis-text path at boot (one-shot from embedded text)

For M0 simplest, the genesis path can use nota-serde-core's
`from_str_nexus` directly to deserialize each KindDecl, then
hand it to `assert::handle`. No frame envelope, just typed
values.

---

## 7 · Open decisions — answered or RETIRED

The decisions surveyed here have all been settled or rendered
obsolete:

- **§7.1 Sema kind-tag storage form** — RETIRED. The per-verb
  typed-payload design (per [088](088-closed-vs-open-schema-research.md))
  doesn't need sema-side kind tagging; criome dispatches each
  `AssertOp` variant to its kind-specific store path. Sema's
  `store(&[u8]) → Slot` API stays as-is for M0.
- **§7.2 Verb scope for M0** — settled: M0 implements
  Handshake + Assert + Query in criome; Mutate / Retract /
  AtomicBatch / Subscribe / Validate return `Diagnostic
  E0099` until M1+.
- **§7.3 Parser approach** — settled by [091](091-pattern-rethink.md):
  hand-written `QueryParser` in the nexus daemon. Already
  implemented in [`nexus/src/parse.rs`](../repos/nexus/src/parse.rs).
- **§7.4 Handshake at CLI ↔ daemon leg** — settled: no
  handshake on that leg; the daemon handles the signal
  handshake on its criome leg.
- **§7.5 nexus-cli stale arch doc** — done in the doc-cleanup
  pass.

---

## 8 · Order, dependencies, totals

```
   ┌─ step 3 (criome body) ─────────────────────┐
   │  3.1  uds.rs accept loop (~40 LoC)         │   sema (step 2) already
   │  3.2  dispatch + assert + query (~80 LoC)  │   landed; kind-tag
   │  3.3  4 integration tests                  │   revision RETIRED
   └────────────────────────────────────────────┘
              │
              ▼
   ┌─ step 7 (genesis.nexus + bootstrap) ───────┐
   │  ~30 LoC text + ~20 LoC bootstrap glue     │   needs §3 (criome
   │                                             │   processes Assert)
   └────────────────────────────────────────────┘
              │
              ▼
   ┌─ step 5 (nexus daemon body) ───────────────┐
   │  5.1  bind + accept (~30 LoC)              │   needs §3 (criome up);
   │  5.2  text parsing dispatch (~80 LoC)      │   QueryParser already
   │       — uses nota-serde-core for asserts    │   landed in nexus/src/
   │       — uses QueryParser for queries       │   parse.rs (24 tests)
   │  5.3  reply rendering (~50 LoC)            │
   │  5.4  unit tests (~6) + 1 integration      │
   └────────────────────────────────────────────┘
              │
              ▼
   ┌─ step 6 (nexus-cli) ───────────────────────┐
   │  6.1  text shuttle (~30 LoC)               │   needs §5 (daemon up);
   │                                             │   nexus-cli/ARCH.md
   │                                             │   stale-fix DONE
   └────────────────────────────────────────────┘

Step 4 (parser-kernel extension) FOLDED into step 5; QueryParser
already landed at [`nexus/src/parse.rs`](../repos/nexus/src/parse.rs).

Total LoC estimate remaining: ~310 (criome 150 + nexus daemon
130 + nexus-cli 30, give or take). The parser/sema parts of M0
are done.
```

End-to-end demo on completion: `nexus-cli example.nexus` where
example.nexus contains `(Node "User")` and `(| Node @name |)`,
with daemon + criome running, returns:
```
(Ok)
[(Node User)]
```

---

## 9 · What I'll do next

If decisions §7.1 prepend / §7.2 Assert+Query-only / §7.3
hand-written / §7.4 no-handshake / §7.5 concurrent are all
yes, I'll proceed in order: 3.0 → 3.1 → 3.2 → 3.3 → 7 → 5 →
6, committing per logical chunk. Each commit follows the
S-expression style; tests pass before each push.

Estimated 5-7 commits across 4 repos (sema, criome, nexus,
nexus-cli) plus one for the genesis text in criome.

If you want any of the §7 decisions different, say so and
I'll adjust before starting.

---

*End 089.*
