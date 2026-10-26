//! Messages between the UI thread and the core worker.
//!
//! The UI sends JSON [`UiRequest`]s; the worker passes each to the Rust
//! core, which answers with exactly one JSON [`CoreEvent`], in order. The
//! first request on a worker is [`UiRequest::Hello`], which negotiates the
//! protocol version so a cached UI never talks to an incompatible WASM
//! build (docs/protocol.md).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProtocolVersion;
use crate::U64String;
use crate::scene::{EntityDetailDto, PlacementDto, SceneUpdateDto, SessionDto};
use crate::site::SiteDto;

/// Largest accepted request, in bytes of JSON, checked before parsing.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Longest accepted build identifier, in bytes.
pub const BUILD_ID_MAX_BYTES: usize = 64;

/// Most scene ticks one `advance` or `review_step` may cover.
pub const MAX_TICKS_PER_REQUEST: u32 = 100;

/// A position the UI picked, in scene-frame metres. The core quantizes it
/// to millimetres and validates it against the site.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointMDto {
    /// East, metres.
    pub x_m: f64,
    /// North, metres.
    pub y_m: f64,
    /// Up, metres.
    pub z_m: f64,
}

/// A picked position and where its height came from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlacedPointDto {
    /// Position, metres.
    pub position: PointMDto,
    /// Height source; a ground-plane placement is stored at z = 0.
    pub placement: PlacementDto,
}

/// A request from the UI thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiRequest {
    /// First message on a new worker: offer the UI's protocol version.
    Hello {
        /// Protocol the UI speaks.
        protocol: ProtocolVersion,
        /// UI build identifier, for diagnostics only.
        #[schemars(length(min = 1, max = 64), regex(pattern = r"^[!-~]+$"))]
        ui_build: String,
    },
    /// Run the live simulation forward. Live mode only.
    Advance {
        /// Scene ticks to simulate.
        #[schemars(range(min = 1, max = 100))]
        ticks: u32,
    },
    /// Place a marker. Live mode only.
    AddMarker {
        /// Label, 1–128 bytes of UTF-8 without control characters.
        #[schemars(length(min = 1, max = 128))]
        label: String,
        /// Where.
        at: PlacedPointDto,
    },
    /// Relabel and/or move a marker. Live mode only.
    EditMarker {
        /// Annotation ID.
        id: String,
        /// The revision the operator is editing; the edit fails if the
        /// marker changed since.
        base_revision: U64String,
        /// New label.
        #[serde(default)]
        #[schemars(length(min = 1, max = 128))]
        label: Option<String>,
        /// New position.
        #[serde(default)]
        at: Option<PlacedPointDto>,
    },
    /// Delete an annotation. Live mode only.
    DeleteAnnotation {
        /// Annotation ID.
        id: String,
        /// The revision the operator saw.
        base_revision: U64String,
    },
    /// Delete a track. Live mode only.
    RemoveEntity {
        /// Track ID.
        id: String,
        /// The revision the operator saw.
        base_revision: U64String,
    },
    /// Ask for a track's details and history in the current view.
    InspectEntity {
        /// Track ID.
        id: String,
    },
    /// Show the state at scene tick `tick`, replayed from the journal. The
    /// live session pauses and is not changed.
    Review {
        /// Scene tick to show; clamped to the live tick.
        tick: U64String,
    },
    /// Move a review forward through the journal. Review mode only.
    ReviewStep {
        /// Scene ticks to move.
        #[schemars(range(min = 1, max = 100))]
        ticks: u32,
    },
    /// Leave review and show the live session again. (An empty struct
    /// variant, not a unit variant, so unknown fields are still rejected.)
    GoLive {},
    /// Ask for the canonical state hash of the current view.
    StateHash {},
}

/// The core's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoreEvent {
    /// Handshake accepted.
    Ready {
        /// Protocol the core speaks.
        protocol: ProtocolVersion,
        /// Core (WASM) build identifier, for diagnostics only.
        core_build: String,
        /// The session's site frame.
        site: SiteDto,
        /// Session versions and parameters.
        session: SessionDto,
        /// The complete initial view.
        scene: SceneUpdateDto,
    },
    /// What the view shows changed (or, for a request that changed nothing,
    /// the unchanged timeline).
    Scene {
        /// The change.
        update: SceneUpdateDto,
    },
    /// A track's details in the current view.
    EntityDetail {
        /// The track and its history.
        detail: EntityDetailDto,
    },
    /// The canonical state hash of the current view (docs/protocol.md).
    StateHash {
        /// Scene tick of the view.
        tick: U64String,
        /// Last sequence included.
        sequence: U64String,
        /// 64-bit FNV-1a of the canonical encoding, 16 lowercase hex digits.
        #[schemars(regex(pattern = r"^[0-9a-f]{16}$"))]
        hash: String,
    },
    /// Handshake refused: this UI and core cannot work together. The UI must
    /// not send further requests; reloading fetches a matching bundle.
    Incompatible {
        /// Protocol the core speaks.
        core_protocol: ProtocolVersion,
        /// Protocol the UI offered.
        ui_protocol: ProtocolVersion,
    },
    /// The request was rejected; no state changed.
    Error {
        /// Machine-readable reason.
        code: ErrorCode,
        /// Human-readable detail. Render as text only.
        message: String,
    },
}

/// Why a request was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Larger than [`MAX_REQUEST_BYTES`].
    RequestTooLarge,
    /// Not valid JSON, or not a known request shape.
    MalformedRequest,
    /// Well-formed, but a field fails validation.
    InvalidField,
    /// A request other than `hello` before a successful handshake.
    HandshakeRequired,
    /// The named object does not exist.
    NotFound,
    /// The named object was deleted.
    Deleted,
    /// The object changed since the edit's base revision.
    PreconditionFailed,
    /// The request needs live mode, but the view is reviewing history (or
    /// the reverse).
    WrongMode,
    /// The session journal is full; start a new session.
    JournalFull,
    /// The scene rules refused the change (for example a position outside
    /// the site).
    Rejected,
    /// A failure inside the core; a bug, not bad input.
    Internal,
}

/// A rejected request: the [`CoreEvent::Error`] to send back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestError {
    /// Machine-readable reason.
    pub code: ErrorCode,
    /// Human-readable detail.
    pub message: String,
}

impl RequestError {
    /// A rejection with `code` and a human-readable `message`.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<RequestError> for CoreEvent {
    fn from(e: RequestError) -> Self {
        CoreEvent::Error {
            code: e.code,
            message: e.message,
        }
    }
}

/// Parses and validates one request. The size limit is checked before any
/// parsing or allocation for the request's contents.
pub fn parse_request(json: &str) -> Result<UiRequest, RequestError> {
    if json.len() > MAX_REQUEST_BYTES {
        return Err(RequestError::new(
            ErrorCode::RequestTooLarge,
            format!(
                "request is {} bytes; the limit is {MAX_REQUEST_BYTES}",
                json.len()
            ),
        ));
    }
    let request: UiRequest = serde_json::from_str(json)
        .map_err(|e| RequestError::new(ErrorCode::MalformedRequest, e.to_string()))?;
    match &request {
        UiRequest::Hello { ui_build, .. } => validate_build_id("ui_build", ui_build)?,
        UiRequest::Advance { ticks } | UiRequest::ReviewStep { ticks }
            if *ticks == 0 || *ticks > MAX_TICKS_PER_REQUEST =>
        {
            return Err(RequestError::new(
                ErrorCode::InvalidField,
                format!("ticks: 1 to {MAX_TICKS_PER_REQUEST}"),
            ));
        }
        // Labels, IDs, and positions are validated by the domain when the
        // request is converted to a command.
        _ => {}
    }
    Ok(request)
}

/// A build identifier is 1 to 64 bytes of printable ASCII without spaces.
fn validate_build_id(field: &str, id: &str) -> Result<(), RequestError> {
    let printable = id.bytes().all(|b| b.is_ascii_graphic());
    if id.is_empty() || id.len() > BUILD_ID_MAX_BYTES || !printable {
        return Err(RequestError::new(
            ErrorCode::InvalidField,
            format!("{field}: 1 to {BUILD_ID_MAX_BYTES} printable ASCII characters, no spaces"),
        ));
    }
    Ok(())
}

/// Schema root naming every bridge message type, so generated TypeScript
/// declares each exactly once. Never sent on the wire.
#[derive(JsonSchema)]
#[schemars(title = "WorkerBridge")]
pub struct WorkerBridgeSchema {
    /// UI to core.
    pub request: UiRequest,
    /// Core to UI.
    pub event: CoreEvent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PROTOCOL_VERSION;

    fn code(json: &str) -> Option<ErrorCode> {
        parse_request(json).err().map(|e| e.code)
    }

    #[test]
    fn parses_hello() {
        let r = parse_request(&format!(
            r#"{{"type":"hello","protocol":{{"major":{},"minor":{}}},"ui_build":"abc123"}}"#,
            PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor
        ));
        assert_eq!(
            r,
            Ok(UiRequest::Hello {
                protocol: PROTOCOL_VERSION,
                ui_build: "abc123".into()
            })
        );
    }

    #[test]
    fn rejects_malformed_and_unknown() {
        assert_eq!(code(""), Some(ErrorCode::MalformedRequest));
        assert_eq!(code("{"), Some(ErrorCode::MalformedRequest));
        assert_eq!(code("[]"), Some(ErrorCode::MalformedRequest));
        assert_eq!(
            code(r#"{"type":"launch_rockets"}"#),
            Some(ErrorCode::MalformedRequest)
        );
        // Missing and extra fields.
        assert_eq!(
            code(r#"{"type":"hello","protocol":{"major":0,"minor":1}}"#),
            Some(ErrorCode::MalformedRequest)
        );
        assert_eq!(
            code(r#"{"type":"hello","protocol":{"major":0,"minor":1},"ui_build":"a","extra":1}"#),
            Some(ErrorCode::MalformedRequest)
        );
        assert_eq!(
            code(r#"{"type":"hello","protocol":{"major":0,"minor":1,"patch":0},"ui_build":"a"}"#),
            Some(ErrorCode::MalformedRequest)
        );
        // Versions are u16: negative, fractional, and too-large values fail to parse.
        for v in ["-1", "1.5", "65536", "\"0\""] {
            let json = format!(
                r#"{{"type":"hello","protocol":{{"major":{v},"minor":1}},"ui_build":"a"}}"#
            );
            assert_eq!(code(&json), Some(ErrorCode::MalformedRequest), "major {v}");
        }
    }

    #[test]
    fn rejects_bad_build_ids() {
        for id in ["", "has space", "tab\t", "ünicode", &"x".repeat(65)] {
            let json = serde_json::json!({
                "type": "hello", "protocol": {"major": 0, "minor": 1}, "ui_build": id
            })
            .to_string();
            assert_eq!(code(&json), Some(ErrorCode::InvalidField), "{id:?}");
        }
        let json = serde_json::json!({
            "type": "hello", "protocol": {"major": 0, "minor": 1}, "ui_build": "x".repeat(64)
        })
        .to_string();
        assert!(parse_request(&json).is_ok());
    }

    #[test]
    fn parses_commands_with_u64_strings() {
        let r = parse_request(
            r#"{"type":"edit_marker","id":"op:1","base_revision":"18446744073709551615","label":"Gate"}"#,
        );
        assert_eq!(
            r,
            Ok(UiRequest::EditMarker {
                id: "op:1".into(),
                base_revision: U64String(u64::MAX),
                label: Some("Gate".into()),
                at: None,
            })
        );
        // u64 values must be strings: a JSON number is malformed.
        assert_eq!(
            code(r#"{"type":"delete_annotation","id":"op:1","base_revision":3}"#),
            Some(ErrorCode::MalformedRequest)
        );
        assert_eq!(
            parse_request(r#"{"type":"go_live"}"#),
            Ok(UiRequest::GoLive {})
        );
        assert_eq!(
            code(r#"{"type":"go_live","now":true}"#),
            Some(ErrorCode::MalformedRequest)
        );
    }

    #[test]
    fn tick_counts_are_bounded() {
        for (ticks, ok) in [(0, false), (1, true), (100, true), (101, false)] {
            let json = format!(r#"{{"type":"advance","ticks":{ticks}}}"#);
            assert_eq!(parse_request(&json).is_ok(), ok, "{ticks}");
            let json = format!(r#"{{"type":"review_step","ticks":{ticks}}}"#);
            assert_eq!(parse_request(&json).is_ok(), ok, "{ticks}");
        }
        assert_eq!(
            code(r#"{"type":"advance","ticks":-1}"#),
            Some(ErrorCode::MalformedRequest)
        );
    }

    #[test]
    fn picked_positions_must_be_numbers() {
        let json = r#"{"type":"add_marker","label":"A","at":{"position":{"x_m":1.5,"y_m":-2,"z_m":0},"placement":"ground_plane"}}"#;
        assert!(parse_request(json).is_ok());
        // JSON has no NaN or infinity; these are malformed, not values.
        for bad in ["NaN", "Infinity", "\"1\"", "null"] {
            let json = format!(
                r#"{{"type":"add_marker","label":"A","at":{{"position":{{"x_m":{bad},"y_m":0,"z_m":0}},"placement":"ground_plane"}}}}"#
            );
            assert_eq!(code(&json), Some(ErrorCode::MalformedRequest), "{bad}");
        }
    }

    #[test]
    fn size_limit_is_checked_first() {
        let at_limit = " ".repeat(MAX_REQUEST_BYTES);
        assert_eq!(code(&at_limit), Some(ErrorCode::MalformedRequest));
        let over = " ".repeat(MAX_REQUEST_BYTES + 1);
        assert_eq!(code(&over), Some(ErrorCode::RequestTooLarge));
    }
}
