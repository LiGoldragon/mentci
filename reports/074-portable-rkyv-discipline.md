# 074 — portable rkyv discipline

*Claude Opus 4.7 · 2026-04-25 · deep-research and
implementation notes for the all-rkyv-except-nexus rule
(architecture.md §10). Establishes the canonical feature
set, derive pattern, encode/decode API surface, and the
schema-evolution boundary. Grounded in rkyv 0.8.16 source
and the existing usage in `repos/nexus-schema/`.*

---

## 1 · The rule

Nexus text is the only non-rkyv messaging surface. Everything
else — client-msg, signal, future lojix-schema, sema records,
lojix-store index entries, every internal wire / storage
format — is rkyv. Per architecture.md §10.

---

## 2 · The canonical feature set

Every rkyv-using crate pins the same Cargo dependency:

```toml
rkyv = { version = "0.8", default-features = false, features = [
    "std",
    "bytecheck",
    "little_endian",
    "pointer_width_32",
    "unaligned",
] }
```

Each feature is load-bearing for portability:

| Feature | What it does | Why required |
|---|---|---|
| `std` | Vec, String, HashMap, etc. archive impls | We're not no_std; criome runs on real machines |
| `bytecheck` | Validation on read; rejects malformed/malicious bytes | Wire is untrusted in principle; UDS in practice but capability tokens cross machines later |
| `little_endian` | Pin archived integer byte order | Without this, archived integers use host endianness; cross-machine archives break |
| `pointer_width_32` | RelPtr offsets are 4 bytes | Without this, offsets follow host pointer width (4 vs 8); cross-machine archives break. 32-bit fits any single archive < 4 GiB |
| `unaligned` | Archived types readable from arbitrary byte offsets | Receiver buffers may not align; UDS gives raw bytes; without this the receiver must align before reading |

**Mandatory feature parity.** Archived types interop only when
both ends compile against the *same* feature set. A crate that
adds or drops a feature breaks archive compatibility silently
— no compile error, just wrong bytes. The exact-string match
in Cargo.toml is the discipline.

**Pinned to rkyv 0.8.x.** Minor versions within 0.8 don't
change archive layout per rkyv's stability guarantees. A
0.9 jump would require a coordinated upgrade across every
rkyv-using crate.

---

## 3 · The canonical derive pattern

Per `repos/nexus-schema/src/primitive.rs`:

```rust
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

#[derive(
    Archive,
    RkyvSerialize,
    RkyvDeserialize,
    Serialize,        // serde, for text codecs / debug
    Deserialize,      // serde
    Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct Foo {
    pub field: u32,
}
```

Aliasing rkyv's `Serialize` / `Deserialize` to `RkyvSerialize`
/ `RkyvDeserialize` avoids the symbol clash with serde's same-
named traits. Both derive sets coexist.

---

## 4 · Encode / decode API

rkyv 0.8 high-level API (rkyv::lib.rs re-exports
`api::high::*` at the crate root):

```rust
// Encode: returns AlignedVec; Deref<Target=[u8]> works for sockets.
let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value)?;

// Decode: validates + deserialises in one call.
let value: T = rkyv::from_bytes::<T, rkyv::rancor::Error>(bytes)?;
```

For zero-copy reads (when ownership transfer isn't needed):

```rust
// Validated borrow into the buffer; returns &Archived<T>.
let archived = rkyv::access::<ArchivedT, rkyv::rancor::Error>(bytes)?;
// Field reads are direct memory access on archived bytes.
```

Error type: `rkyv::rancor::Error` is the boxed-error variant.
For protocol code we typically catch any rkyv error and map to
the protocol's own error enum (`FrameDecodeError::BadArchive`
etc.) so callers don't depend on rkyv's internal error shape.

---

## 5 · Type adaptations

Most stdlib types archive natively: `String`, `Vec<T>`,
`Option<T>`, `Box<T>`, `BTreeMap<K,V>`, primitives,
fixed-size arrays.

Some don't, and need adapters or our own newtypes:

- **`PathBuf` / `OsString`** — bytes on Unix; UTF-16-ish on
  Windows. We use [`WirePath(Vec<u8>)`](repos/nexus/src/client_msg/path.rs)
  per Li 2026-04-25 ("paths are deterministic; no string
  round-trip"). The newtype lives in `client_msg` for now;
  moves to `criome-types` when that crate lands.
- **Recursive types** — rkyv handles cycles via `Box`/`Rc`
  with shared-pointer support enabled by default in `std`.
- **Foreign types we don't own** — use `#[rkyv(with = ...)]`
  attribute per the rkyv `with` module. Avoid where possible;
  wrap in our own newtype instead so the schema is ours.

---

## 6 · Schema evolution — the limit

rkyv archives are schema-fragile. Adding/removing/reordering
fields changes the archive bytes. Two consequences:

1. **No silent backward compatibility.** Old binaries can't
   read new archives or vice versa. Schema changes are
   coordinated upgrades.
2. **The `VersionSkewGuard` record** ([reports/065 §3.4](065-criome-schema-design.md))
   sits at a known slot and stores the schema-version /
   wire-format-version. criome checks at boot; hard-fails on
   mismatch. This is the architectural answer to schema
   evolution; rkyv's own version handling is not enough.

Within a single rkyv 0.8 release, reordering struct fields
*also* changes the archive layout. Discipline: append-only
field additions as a soft convention; treat any change as
breaking for safety.

---

## 7 · Wire framing

Per Li 2026-04-25 and architecture.md §10: the frame schema
*is* the framing. Both parties know the `Frame` rkyv schema;
the wire is a stream of `Archived<Frame>` instances.

`from_bytes::<Frame, E>(bytes)` validates + deserialises a
single frame from a byte slice. Splitting a TCP/UDS stream
into per-frame slices is the transport layer's concern (e.g.,
length-prefix at the framing level inside the schema, or one
frame per UDS message). Implementation detail; not a
protocol-level commitment beyond "the schema is the framing."

---

## 8 · What this report leaves to implementation

- `Frame::encode` body: `rkyv::to_bytes::<rkyv::rancor::Error>(self)
  .expect("rkyv serialisation never fails for owned data").to_vec()`
- `Frame::decode` body: `rkyv::from_bytes::<Frame,
  rkyv::rancor::Error>(bytes).map_err(|_|
  FrameDecodeError::BadArchive)`
- Round-trip test: construct, encode, decode, assert
  structural equality.
- The `WirePath` newtype already round-trips through rkyv
  via `Vec<u8>`'s native impl.

---

*End report 074.*
