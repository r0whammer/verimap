//! Messages between the UI thread and the core worker.
//!
//! The UI sends JSON [`UiRequest`]s; the worker passes each to the Rust
//! core, which answers with exactly one JSON [`CoreEvent`]. The first request
//! on a worker is [`UiRequest::Hello`], which negotiates the protocol version
//! so a cached UI never talks to an incompatible WASM build (docs/protocol.md).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProtocolVersion;
use crate::site::SiteDto;

/// Largest accepted request, in bytes of JSON, checked before parsing.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Longest accepted build identifier, in bytes.
pub const BUILD_ID_MAX_BYTES: usize = 64;

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
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
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
        let r = parse_request(
            r#"{"type":"hello","protocol":{"major":0,"minor":1},"ui_build":"abc123"}"#,
        );
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
    fn size_limit_is_checked_first() {
        let at_limit = " ".repeat(MAX_REQUEST_BYTES);
        assert_eq!(code(&at_limit), Some(ErrorCode::MalformedRequest));
        let over = " ".repeat(MAX_REQUEST_BYTES + 1);
        assert_eq!(code(&over), Some(ErrorCode::RequestTooLarge));
    }
}
