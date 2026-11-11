use core::fmt;

/// Why a transform or frame lookup was rejected. Nothing is applied when
/// one of these is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryError {
    /// NaN or an infinity.
    NonFinite {
        /// Offending field.
        field: &'static str,
    },
    /// A quaternion whose norm is not within [`crate::ROTATION_NORM_TOLERANCE`] of 1.
    /// The value is not renormalized.
    RotationNotUnit,
    /// A frame name that is empty, too long, or contains a control character.
    InvalidFrameName {
        /// What rule it broke.
        reason: &'static str,
    },
    /// No `T_parent_from_child` is registered for this child and version.
    UnknownFrame {
        /// Child frame name.
        frame: String,
        /// Requested transform version.
        version: u32,
    },
    /// The same child and version were registered with a different transform.
    FrameConflict {
        /// Child frame name.
        frame: String,
        /// Transform version.
        version: u32,
    },
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { field } => write!(f, "{field}: not a finite number"),
            Self::RotationNotUnit => {
                write!(
                    f,
                    "quaternion norm is not within the rotation tolerance of 1"
                )
            }
            Self::InvalidFrameName { reason } => write!(f, "frame name: {reason}"),
            Self::UnknownFrame { frame, version } => {
                write!(f, "no transform for frame {frame} version {version}")
            }
            Self::FrameConflict { frame, version } => {
                write!(f, "frame {frame} version {version} is already registered")
            }
        }
    }
}

impl std::error::Error for GeometryError {}
