use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A bridge protocol version. See docs/protocol.md for when each part changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProtocolVersion {
    /// Incompatible changes.
    pub major: u16,
    /// Backward-compatible additions.
    pub minor: u16,
}

/// The protocol this build speaks.
pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 0, minor: 2 };

impl ProtocolVersion {
    /// Whether a core speaking `self` can serve a UI speaking `ui`.
    ///
    /// While the major version is 0 any change may be incompatible, so the
    /// versions must match exactly. From 1.0, majors must match and the core
    /// must be at least as new as the UI.
    pub fn serves(self, ui: ProtocolVersion) -> bool {
        if self.major == 0 {
            self == ui
        } else {
            self.major == ui.major && self.minor >= ui.minor
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn v(major: u16, minor: u16) -> ProtocolVersion {
        ProtocolVersion { major, minor }
    }

    #[test]
    fn pre_1_0_requires_exact_match() {
        assert!(v(0, 1).serves(v(0, 1)));
        assert!(!v(0, 2).serves(v(0, 1)));
        assert!(!v(0, 1).serves(v(0, 2)));
        assert!(!v(0, 1).serves(v(1, 1)));
    }

    #[test]
    fn post_1_0_core_may_be_newer_minor() {
        assert!(v(1, 3).serves(v(1, 2)));
        assert!(v(1, 2).serves(v(1, 2)));
        assert!(!v(1, 2).serves(v(1, 3)));
        assert!(!v(2, 0).serves(v(1, 0)));
    }
}
