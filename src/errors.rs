//! Parse-time errors and non-fatal warnings.
//!
//! Parse errors are fatal — a diagram cannot be rendered when one is returned.
//! Warnings are advisory (e.g., density lint) and always accompany a
//! successfully built [`crate::Diagram`].

use thiserror::Error;

/// Fatal error surfaced during [`crate::parse`].
///
/// Variant shapes match the scenarios in `specs/m-diagram-v1.spec.md` — do not
/// refactor the fields without also updating those assertions.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// JSON did not deserialize cleanly. `line` / `column` are 1-indexed and
    /// mirror `serde_json::Error::{line,column}`.
    #[error("malformed JSON at line {line}, column {column}: {message}")]
    Malformed {
        line: usize,
        column: usize,
        message: String,
    },

    /// The `"type"` discriminator did not name one of the 5 diagram types
    /// shipped in v1.
    #[error("unknown diagram type: {0}")]
    UnknownType(String),

    /// `accent_idx` pointed past the diagram's element count. Element count is
    /// the primary axis of the diagram (levels for pyramid, nodes for tree,
    /// etc.).
    #[error("accent index {accent_idx} out of range for {element_count} elements")]
    AccentOutOfRange {
        element_count: usize,
        accent_idx: usize,
    },

    /// JSON body exceeded [`crate::DiagramLimits::MAX_BODY_BYTES`]. Gated
    /// before `serde_json::from_str` is invoked.
    #[error("body too large: {0} bytes (max {max})", max = crate::DiagramLimits::MAX_BODY_BYTES)]
    BodyTooLarge(usize),

    /// Diagram's element count exceeded [`crate::DiagramLimits::MAX_NODES`].
    #[error("too many nodes: {actual} (max {limit})")]
    TooManyNodes { actual: usize, limit: usize },
}

/// Non-fatal advisory surfaced alongside a successful parse.
///
/// Consumers render normally but may choose to surface this as a UI hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Element count exceeds this diagram type's soft cap — readability will
    /// degrade.
    DensityHigh {
        diagram_type: &'static str,
        count: usize,
        soft_cap: usize,
    },
}

impl Warning {
    /// The diagram type tag for which this warning fired.
    #[must_use]
    pub fn diagram_type(&self) -> &'static str {
        match self {
            Warning::DensityHigh { diagram_type, .. } => diagram_type,
        }
    }
}

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        ParseError::Malformed {
            line: err.line(),
            column: err.column(),
            message: err.to_string(),
        }
    }
}
