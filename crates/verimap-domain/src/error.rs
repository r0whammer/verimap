use core::fmt;

use verimap_kernel::KernelError;

/// Why the domain rejected a value. Every variant names the offending field
/// so the boundary can report it; none is produced after a state change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// A confidence above 10 000 basis points.
    ConfidenceOutOfRange {
        /// Offending field.
        field: &'static str,
    },
    /// A scene coordinate outside ±100 km of the anchor.
    CoordinateOutOfRange {
        /// Offending field.
        field: &'static str,
    },
    /// An interval whose minimum is greater than its maximum.
    BoundsInverted {
        /// Offending field.
        field: &'static str,
    },
    /// An interval with no extent where the domain requires one.
    EmptyExtent {
        /// Offending field.
        field: &'static str,
    },
    /// NaN or an infinity.
    NonFinite {
        /// Offending field.
        field: &'static str,
    },
    /// A finite number outside its documented range.
    OutOfRange {
        /// Offending field.
        field: &'static str,
    },
    /// A value outside the site's scene bounds.
    OutsideSite {
        /// Offending field.
        field: &'static str,
    },
    /// A string that is empty, too long, or uses disallowed characters.
    InvalidText {
        /// Offending field.
        field: &'static str,
        /// What rule it broke.
        reason: &'static str,
    },
}

impl DomainError {
    /// Attaches the field name to a kernel guard's rejection.
    pub fn from_kernel(field: &'static str, error: KernelError) -> Self {
        match error {
            KernelError::ConfidenceOutOfRange => Self::ConfidenceOutOfRange { field },
            KernelError::CoordinateOutOfRange => Self::CoordinateOutOfRange { field },
            KernelError::BoundsInverted => Self::BoundsInverted { field },
            // Only `next_sequence` reports this, and it never validates a field.
            KernelError::SequenceExhausted => Self::OutOfRange { field },
        }
    }

    /// The offending field.
    pub fn field(&self) -> &'static str {
        match self {
            Self::ConfidenceOutOfRange { field }
            | Self::CoordinateOutOfRange { field }
            | Self::BoundsInverted { field }
            | Self::EmptyExtent { field }
            | Self::NonFinite { field }
            | Self::OutOfRange { field }
            | Self::OutsideSite { field }
            | Self::InvalidText { field, .. } => field,
        }
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfidenceOutOfRange { field } => {
                write!(f, "{field}: confidence above 10000 basis points")
            }
            Self::CoordinateOutOfRange { field } => {
                write!(f, "{field}: coordinate outside ±100 km of the anchor")
            }
            Self::BoundsInverted { field } => write!(f, "{field}: minimum exceeds maximum"),
            Self::EmptyExtent { field } => write!(f, "{field}: interval has no extent"),
            Self::NonFinite { field } => write!(f, "{field}: not a finite number"),
            Self::OutOfRange { field } => write!(f, "{field}: outside the documented range"),
            Self::OutsideSite { field } => write!(f, "{field}: outside the site bounds"),
            Self::InvalidText { field, reason } => write!(f, "{field}: {reason}"),
        }
    }
}

impl std::error::Error for DomainError {}
