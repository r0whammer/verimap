//! The VeriMap core as the browser's dedicated worker sees it.
//!
//! These bindings only move JSON across the WASM boundary; all behavior is
//! in [`CoreSession`] and the crates it calls, which also run natively. The
//! worker (`web/src/worker/core.worker.ts`) passes each request string to
//! [`Core::handle`] and posts the returned event string back to the UI.

#![forbid(unsafe_code)]

mod session;

pub use session::{CORE_BUILD, CoreSession};
use wasm_bindgen::prelude::wasm_bindgen;

/// One core session, owned by one worker.
#[wasm_bindgen]
pub struct Core {
    session: CoreSession,
}

#[wasm_bindgen]
impl Core {
    /// Starts a session that has not yet negotiated a protocol.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Core {
        Core {
            session: CoreSession::new(),
        }
    }

    /// Handles one JSON `UiRequest`; always returns one JSON `CoreEvent`.
    pub fn handle(&mut self, request_json: &str) -> String {
        self.session.handle_json(request_json)
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

/// This core's build identifier, for diagnostics before the handshake.
#[wasm_bindgen(js_name = coreBuild)]
pub fn core_build() -> String {
    CORE_BUILD.to_owned()
}
