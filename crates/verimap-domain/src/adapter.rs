//! The sensor-adapter contract shared by simulated and future physical
//! sources. Transport parsing stays outside the domain. This prototype
//! assumes a known platform pose: nothing here estimates motion.

/// Version of this contract. A source built against another version is
/// rejected by the session that hosts it.
pub const ADAPTER_INTERFACE_VERSION: u32 = 1;

/// Whether the adapter is producing observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterPhase {
    /// `start` has been called and acquisitions are attempted.
    Running,
    /// `stop` has been called, or the adapter has not been started.
    Stopped,
}

/// What the operator should believe about a source right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceHealth {
    /// Recent scheduled acquisitions were delivered.
    Healthy,
    /// Running, but the latest scheduled acquisition was missed.
    Degraded,
    /// Not running.
    Unavailable,
}

/// Which products a source can emit. A false flag means the product is
/// absent, not that it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCapabilities {
    /// Platform pose in the scene frame.
    pub reports_pose: bool,
    /// Object detections.
    pub reports_detections: bool,
    /// Range returns (LiDAR-like). Not produced yet.
    pub reports_ranges: bool,
}

impl core::fmt::Display for AdapterPhase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
        })
    }
}

impl core::fmt::Display for SourceHealth {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_and_health_name_themselves() {
        assert_eq!(AdapterPhase::Running.to_string(), "running");
        assert_eq!(AdapterPhase::Stopped.to_string(), "stopped");
        assert_eq!(SourceHealth::Healthy.to_string(), "healthy");
        assert_eq!(SourceHealth::Degraded.to_string(), "degraded");
        assert_eq!(SourceHealth::Unavailable.to_string(), "unavailable");
        assert_eq!(ADAPTER_INTERFACE_VERSION, 1);
        let caps = SourceCapabilities {
            reports_pose: true,
            reports_detections: false,
            reports_ranges: false,
        };
        assert!(caps.reports_pose);
        assert!(!caps.reports_ranges);
    }
}
