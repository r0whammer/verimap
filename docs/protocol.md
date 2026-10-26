# Worker bridge protocol

The messages between the UI thread and the core worker. Version **0.1**.
The source of truth is `crates/verimap-protocol`. Never edit the generated
files `schemas/worker-bridge.schema.json` (JSON Schema 2020-12) and
`web/src/generated/protocol.ts` by hand. Regenerate them with `just gen`;
`just check-generated` fails if they are stale.

## Transport

The UI starts a dedicated module worker (`web/src/worker/core.worker.ts`).
The worker loads the WASM core and passes each message to `Core.handle` as a
JSON string. Each request produces one event. The worker posts the parsed
event back to the UI. Messages are JSON objects tagged by `type`.

TypeScript types describe the messages but validate nothing at runtime. The
Rust side parses and validates every request. The UI checks that an event
has a known `type` and fails the session if it does not.

## Messages (0.1)

| Direction | `type` | Fields | Meaning |
| --- | --- | --- | --- |
| UI → core | `hello` | `protocol`, `ui_build` | Opens the session. Must be first. |
| core → UI | `ready` | `protocol`, `core_build`, `site` | Handshake accepted; `site` is the bundled site's frame and bounds. |
| core → UI | `incompatible` | `core_protocol`, `ui_protocol` | The core cannot serve this UI; the UI terminates the worker. |
| core → UI | `error` | `code`, `message` | The request was rejected; session state is unchanged. |

Error codes: `request_too_large`, `malformed_request`, `invalid_field`,
`internal`. `message` is diagnostic text for people. The UI renders it as
text and never interprets it.

## Validation before use

`parse_request` checks, in this order:

1. size: at most 64 KiB of UTF-8 (`MAX_REQUEST_BYTES`), checked before
   parsing;
2. shape: valid JSON matching a known message. Unknown fields and unknown
   `type` values are rejected (`deny_unknown_fields`);
3. fields: `ui_build` must be 1–64 printable ASCII characters.

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
- 64-bit values (sequences, timestamps) must be decimal strings, never JSON
  numbers. None exist in 0.1.
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
