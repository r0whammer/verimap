use verimap_protocol::bridge::{CoreEvent, ErrorCode, UiRequest, parse_request};
use verimap_protocol::site::SiteDto;
use verimap_protocol::{PROTOCOL_VERSION, ProtocolVersion};

/// Build identifier baked in at compile time from `VERIMAP_BUILD_ID`
/// (set by the justfile from git), or `dev`.
pub const CORE_BUILD: &str = match option_env!("VERIMAP_BUILD_ID") {
    Some(id) => id,
    None => "dev",
};

/// Sent if an event cannot be serialized, which would be a core bug.
const SERIALIZE_FAILED: &str =
    r#"{"type":"error","code":"internal","message":"failed to serialize core event"}"#;

/// Core state for one worker. Platform independent so native tests cover
/// exactly what the WASM build runs.
#[derive(Debug, Default)]
pub struct CoreSession {
    negotiated: Option<ProtocolVersion>,
}

impl CoreSession {
    /// A session awaiting `hello`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The protocol agreed with the UI, if the handshake succeeded.
    pub fn negotiated(&self) -> Option<ProtocolVersion> {
        self.negotiated
    }

    /// Handles one JSON request and returns one JSON event. Never panics on
    /// malformed input; rejected requests leave the session unchanged.
    pub fn handle_json(&mut self, request_json: &str) -> String {
        let event = match parse_request(request_json) {
            Ok(request) => self.handle(request),
            Err(e) => e.into(),
        };
        serde_json::to_string(&event).unwrap_or_else(|_| SERIALIZE_FAILED.to_owned())
    }

    /// Handles one validated request.
    pub fn handle(&mut self, request: UiRequest) -> CoreEvent {
        match request {
            UiRequest::Hello { protocol, .. } => self.hello(protocol),
        }
    }

    fn hello(&mut self, ui_protocol: ProtocolVersion) -> CoreEvent {
        if !PROTOCOL_VERSION.serves(ui_protocol) {
            self.negotiated = None;
            return CoreEvent::Incompatible {
                core_protocol: PROTOCOL_VERSION,
                ui_protocol,
            };
        }
        match verimap_sim::demo_site() {
            Ok(site) => {
                self.negotiated = Some(ui_protocol);
                CoreEvent::Ready {
                    protocol: PROTOCOL_VERSION,
                    core_build: CORE_BUILD.to_owned(),
                    site: SiteDto::from(&site),
                }
            }
            Err(e) => CoreEvent::Error {
                code: ErrorCode::Internal,
                message: format!("bundled demo site is invalid: {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verimap_domain::SiteFrame;

    fn hello(major: u16, minor: u16) -> String {
        format!(
            r#"{{"type":"hello","protocol":{{"major":{major},"minor":{minor}}},"ui_build":"test"}}"#
        )
    }

    fn event(json: &str) -> CoreEvent {
        serde_json::from_str(json).expect("core always emits a valid CoreEvent")
    }

    #[test]
    fn matching_hello_is_ready_with_valid_site() {
        let mut s = CoreSession::new();
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor));
        let CoreEvent::Ready { protocol, site, .. } = event(&out) else {
            panic!("expected ready, got {out}");
        };
        assert_eq!(protocol, PROTOCOL_VERSION);
        assert_eq!(s.negotiated(), Some(PROTOCOL_VERSION));
        // The site survives the wire and revalidates in the domain.
        let back = SiteFrame::try_from(&site).expect("site DTO revalidates");
        assert_eq!(Some(back), verimap_sim::demo_site().ok());
    }

    #[test]
    fn mismatched_hello_is_incompatible_and_clears_negotiation() {
        let mut s = CoreSession::new();
        s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor));
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor + 1));
        assert!(
            matches!(event(&out), CoreEvent::Incompatible { .. }),
            "{out}"
        );
        assert_eq!(s.negotiated(), None);
    }

    #[test]
    fn malformed_request_is_an_error_event_and_changes_nothing() {
        let mut s = CoreSession::new();
        for bad in ["", "null", "{\"type\":\"hello\"}", "\u{0}"] {
            let out = s.handle_json(bad);
            assert!(
                matches!(
                    event(&out),
                    CoreEvent::Error {
                        code: ErrorCode::MalformedRequest,
                        ..
                    }
                ),
                "{bad:?} -> {out}"
            );
            assert_eq!(s.negotiated(), None);
        }
    }

    #[test]
    fn site_wire_format_is_stable() {
        let mut s = CoreSession::new();
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["type"], "ready");
        assert_eq!(v["site"]["site_id"], "demo-site-alpha");
        assert_eq!(v["site"]["anchor"]["longitude_deg"], -98.35);
        assert_eq!(v["site"]["bounds"]["x"]["min_mm"], -100_000);
        assert_eq!(v["site"]["bounds"]["z"]["max_mm"], 80_000);
    }
}
