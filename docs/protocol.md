# Worker bridge protocol

The messages between the UI thread and the core worker. Version **0.2**.
The source of truth is `crates/verimap-protocol`. Never edit the generated
files `schemas/worker-bridge.schema.json` (JSON Schema 2020-12) and
`web/src/generated/protocol.ts` by hand. Regenerate them with `just gen`;
`just check-generated` fails if they are stale.

## Transport

The UI starts a dedicated module worker (`web/src/worker/core.worker.ts`).
The worker loads the WASM core and passes each message to `Core.handle` as a
JSON string. Each request produces exactly one event, and events come back
in request order. The worker posts the parsed event back to the UI.
Messages are JSON objects tagged by `type`.

The UI (`web/src/worker/bridge.ts`) matches answers to requests by that
order: it keeps a FIFO of pending requests, and an event with no pending
request fails the session. Requests sent before the handshake completes, or
pending when the worker fails, resolve with an `internal` error instead of
hanging.

TypeScript types describe the messages but validate nothing at runtime. The
Rust side parses and validates every request. The UI checks that an event
has a known `type` and fails the session if it does not.

## Messages (0.2)

Requests (UI → core):

| `type` | Fields | Mode | Effect |
| --- | --- | --- | --- |
| `hello` | `protocol`, `ui_build` | before handshake | Opens the session. Must be first. |
| `advance` | `ticks` (1–100) | live | Runs the simulation and pipeline one tick at a time. |
| `add_marker` | `label`, `at` | live | Commits a new operator marker. |
| `edit_marker` | `id`, `base_revision`, `label?`, `at?` | live | Relabels and/or moves a marker if it is still at `base_revision`. |
| `delete_annotation` | `id`, `base_revision` | live | Deletes (tombstones) a marker. |
| `remove_entity` | `id`, `base_revision` | live | Deletes (tombstones) a track. |
| `inspect_entity` | `id` | either | A track's details and history in the current view. |
| `review` | `tick` | either | Shows the state at `tick` (clamped to the live tick), replayed from the journal. |
| `review_step` | `ticks` (1–100) | review | Moves the review forward through the journal. |
| `go_live` | — | either | Leaves review. |
| `state_hash` | — | either | Canonical state hash of the current view. |

Events (core → UI):

| `type` | Fields | Answers |
| --- | --- | --- |
| `ready` | `protocol`, `core_build`, `site`, `session`, `scene` | `hello` accepted. `scene` is a complete (replacing) view. |
| `incompatible` | `core_protocol`, `ui_protocol` | `hello` refused. The UI terminates the worker. |
| `scene` | `update` | Every request that changes or switches the view. |
| `entity_detail` | `detail` | `inspect_entity`. |
| `state_hash` | `tick`, `sequence`, `hash` | `state_hash`. |
| `error` | `code`, `message` | Any request that was refused. No state changed. |

Error codes: `request_too_large`, `malformed_request`, `invalid_field`,
`handshake_required`, `not_found`, `deleted`, `precondition_failed`,
`wrong_mode`, `journal_full`, `rejected`, `internal`. `message` is
diagnostic text for people. The UI renders it as text and never interprets
it.

### Scene updates

`SceneUpdateDto` carries what the view shows, not raw observations:

- `replace: true`: the lists are the complete view, and the UI discards
  everything else (`ready`, `review`, `go_live`).
- `replace: false`: upserts (`entities`, `annotations`) and removals
  (`removed_entities`, `removed_annotations`) by ID. They cover every object
  the request created, changed, or deleted, and every track whose freshness
  changed because the clock moved.
- `timeline`: the view's mode, tick, and last sequence, the live tick and
  sequence, and the journal's size and bound.
- `stats`: the live session's observation counts.
- `problem`: set when a request stopped part way and the update holds only
  what was done. For example, a full journal stops `advance`.

The UI applies updates in arrival order (`web/src/scene/model.ts`). It makes
no domain decisions.

### Live and review

The local core runs a single session. `review` replays the committed journal
from the start into a separate read-only store, up to the last event at or
before `tick`. While reviewing:

- requests that would change the session (`advance`, edits) fail with
  `wrong_mode`;
- the live session does not advance, so scene time is paused;
- `go_live` discards the review store and shows the unchanged live state.

Reviewing therefore never mutates the live session. The e2e suite checks
this by comparing the live state hash before and after a review.

### Edits and revisions

Every committed event has the next authority sequence, and an object's
revision is the sequence of the event that last changed it. Edits and
deletions name the `base_revision` the operator saw. If the object has
changed since then, the edit fails with `precondition_failed`, and a
deleted object reports `deleted`. Deletions leave tombstones, so a deleted
ID is never reused or resurrected in the session.

Marker positions arrive as `PlacedPointDto`: metres plus a `placement`
saying where the height came from. In 0.2 the only placement is
`ground_plane`, which stores the marker at z = 0 m. The UI states that this
is an assumed height, not a measured surface.

### State hash

`state_hash` is the 64-bit FNV-1a of the view's canonical encoding
(`verimap_domain::canonical`). The encoding writes fields in a fixed order
and maps in ID order, so native and WASM builds produce the same bytes for
the same events. It is a checksum for comparing replays, not a
cryptographic digest. `fixtures/replay/demo-seed-1.json` records native
hashes of the seeded demo session at several ticks. The Rust tests and the
browser e2e suite (WASM) both compare against it.

## Validation before use

`parse_request` checks, in this order:

1. size: at most 64 KiB of UTF-8 (`MAX_REQUEST_BYTES`), checked before
   parsing;
2. shape: valid JSON matching a known message. Unknown fields and unknown
   `type` values are rejected (`deny_unknown_fields`);
3. fields: `ui_build` must be 1–64 printable ASCII characters; `ticks`
   must be 1–100 (`MAX_TICKS_PER_REQUEST`); `u64` strings must be
   canonical (below).

Domain values are validated when the request is converted to a command:
IDs, labels (1–128 bytes of UTF-8 without control characters), finite
coordinates that round to millimetres within range, and positions inside
the site. The command is checked against the current state before anything
is committed.

A rejected request produces an `error` event and does not change the
session.

## Versions and compatibility

`ProtocolVersion { major, minor }` is defined in `verimap-protocol`
(`PROTOCOL_VERSION`) and is generated into TypeScript, so the UI and the
core are built from the same constant.

- **While `major` is 0**, any change may be incompatible. The core serves
  only a UI with the identical version.
- **From 1.0**, a core serves a UI with the same major version and an equal
  or lower minor version. Additions that an older UI can ignore bump the
  minor version. Removing or renaming a message or field, changing a field's
  meaning or units, or tightening a bound bumps the major version.
- Requests reject unknown fields. A UI therefore must not send a field its
  negotiated core version does not define.

The build IDs (`ui_build`, `core_build`) come from
`git describe --always --dirty` at build time, or `dev`. They are exchanged
and shown in the details panel for diagnosis. **They are not compared**:
the protocol version alone decides compatibility. The app has no offline
cache yet, so the UI and core always come from the same server build. When
the service worker arrives (M3), the cache will be versioned as one complete
UI/worker/WASM bundle, so a mixed-version bundle is never activated.

## Encoding rules

- Coordinates are integer millimetres within ±10⁸ (see
  [coordinates.md](coordinates.md)). These are exact as JSON numbers in
  JavaScript, because 10⁸ is far below 2⁵³.
- 64-bit values (sequences, revisions, ticks, counts, the seed) are
  `U64String`: a canonical decimal string with no sign, no leading zeros
  (except `"0"`), and at most 20 digits, within `u64`. They are never JSON
  numbers, so JavaScript cannot round them. The UI converts one to a
  `Number` only for a slider, and only when it is a safe integer; scene time
  is formatted with `BigInt`.
- Positions in requests are metres (`PointMDto`, finite `f64`). The core
  rounds them to millimetres, half away from zero, and rejects
  out-of-range values.
- Floating-point values (anchor degrees and metres) are JSON numbers and
  must be finite. Rust rejects non-finite values in domain conversion.
- **String lengths:** the schema's `maxLength` counts Unicode code points,
  but Rust enforces the limit in UTF-8 **bytes**. A string that fits the
  schema can still be rejected. Rust is authoritative.

## Schema generation

`cargo run -p verimap-protocol --bin verimap-schemas -- schemas` writes the
schema from `WorkerBridgeSchema`. Its `x-verimap-protocol-version` field
records the protocol version. `web/scripts/gen-types.mjs` converts it with
`json-schema-to-typescript`. Before conversion, the script removes keywords
that sit beside a `$ref`, so each shared definition becomes one TypeScript
type rather than numbered copies. It also appends the `PROTOCOL_VERSION`
constant.
