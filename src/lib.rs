//! # makepad-diagram-kit
//!
//! Streaming-renderable editorial diagrams for Makepad 2.0 apps.
//!
//! This v1 library is **renderer-agnostic**. It consumes a compact JSON
//! "diagram spec" and produces positioned [`Primitive`]s (rects, polygons,
//! lines, arrows, text) that any backend — Makepad, HTML Canvas, an SVG
//! printer — can walk and paint. A Makepad-native binding is scaffolded under
//! the `makepad` feature and will land in a later task.
//!
//! Five diagram types ship in v1:
//! - `pyramid` — ranked trapezoid layers
//! - `quadrant` — 2-axis scatter
//! - `tree` — parent → children hierarchy
//! - `layers` — stacked horizontal bands
//! - `flowchart` — vertical decision flow
//!
//! ## Quickstart
//!
//! ```
//! use makepad_diagram_kit::{parse, layout, LayoutContext};
//!
//! let json = r#"{"type":"pyramid","levels":[
//!     {"label":"Mission"},
//!     {"label":"Strategy"},
//!     {"label":"Tactics"}
//! ]}"#;
//!
//! let (diagram, warnings) = parse(json).unwrap();
//! assert!(warnings.is_empty());
//!
//! let ctx = LayoutContext::new(1000.0, 500.0);
//! let layout = layout(&diagram, &ctx);
//! assert!(layout.primitive_count() > 0);
//! ```
//!
//! ## Streaming
//!
//! LLMs emit JSON progressively. [`parse_lossy`] accepts a prefix and tries to
//! produce a best-effort [`Diagram`] by closing open brackets:
//!
//! ```
//! # use makepad_diagram_kit::parse_lossy;
//! let partial = r#"{"type":"layers","layers":[{"label":"A"},{"label":"B""#;
//! let diagram = parse_lossy(partial).unwrap();
//! # let _ = diagram;
//! ```
//!
//! See `specs/m-diagram-v1.spec.md` for the full contract.

#![warn(clippy::all)]

pub mod errors;
pub mod layout;
pub mod primitive;
pub mod streaming;
pub mod theme;
pub mod types;

// Makepad-native bindings — feature-gated so pure-parsing consumers stay
// lightweight. `widget` exports `DiagramView`; `makepad_render` exports the
// primitive → draw-call helpers in case embedders want to paint into their
// own widgets instead of hosting the whole `DiagramView`.
#[cfg(feature = "makepad")]
pub mod makepad_render;
#[cfg(feature = "makepad")]
pub mod widget;

#[cfg(feature = "makepad")]
pub use widget::DiagramView;

pub use errors::{ParseError, Warning};
pub use layout::{DiagramLayout, LayoutContext};
pub use primitive::{LineStyle, Point, Primitive, Rect, TextAlign, TextWeight};
pub use streaming::parse_lossy;
pub use theme::{Color, Palette, Theme, Typography};
pub use types::{
    Diagram, FlowchartSpec, LayersSpec, PyramidSpec, QuadrantSpec, TreeSpec, layout_flowchart,
    layout_layers, layout_pyramid, layout_quadrant, layout_tree,
};

/// Hard and soft limits enforced during parsing.
///
/// Hard limits fail with a [`ParseError`]; soft caps surface a
/// [`Warning::DensityHigh`] on successful parse.
pub struct DiagramLimits;

impl DiagramLimits {
    /// Hard ceiling on the raw JSON body. Gated before `serde_json::from_str`.
    pub const MAX_BODY_BYTES: usize = 200 * 1024;

    /// Hard ceiling on primary-axis element count per diagram.
    pub const MAX_NODES: usize = 30;

    /// Soft cap for pyramid levels.
    pub const SOFT_CAP_PYRAMID: usize = types::pyramid::SOFT_CAP;
    /// Soft cap for tree total nodes.
    pub const SOFT_CAP_TREE: usize = types::tree::SOFT_CAP;
    /// Soft cap for flowchart nodes.
    pub const SOFT_CAP_FLOWCHART: usize = types::flowchart::SOFT_CAP;
    /// Soft cap for quadrant points.
    pub const SOFT_CAP_QUADRANT: usize = types::quadrant::SOFT_CAP;
    /// Soft cap for layer rows.
    pub const SOFT_CAP_LAYERS: usize = types::layers::SOFT_CAP;
}

/// Parse a full JSON diagram body.
///
/// Returns `(diagram, warnings)` on success. Warnings are advisory — the
/// diagram renders correctly regardless. On failure, returns a [`ParseError`]
/// with the specific reason.
///
/// # Errors
///
/// See [`ParseError`] variants:
/// * `Malformed` for JSON syntax errors.
/// * `UnknownType` for an unsupported `"type"` discriminator.
/// * `AccentOutOfRange` when `accent_idx` points past the element count.
/// * `BodyTooLarge` when the input exceeds [`DiagramLimits::MAX_BODY_BYTES`]
///   — gated **before** `serde_json::from_str`.
/// * `TooManyNodes` when the element count exceeds [`DiagramLimits::MAX_NODES`].
pub fn parse(body: &str) -> Result<(Diagram, Vec<Warning>), ParseError> {
    // Hard gate on body size — must not touch serde_json.
    if body.len() > DiagramLimits::MAX_BODY_BYTES {
        return Err(ParseError::BodyTooLarge(body.len()));
    }

    // Pre-scan the `"type"` field so we can surface UnknownType with a clean
    // message. serde_json's tagged enum failure message is noisy; this keeps
    // the error path ergonomic.
    if let Some(unknown) = pre_scan_unknown_type(body) {
        return Err(ParseError::UnknownType(unknown));
    }

    let diagram: Diagram = serde_json::from_str(body)?;

    // Hard node cap — post-parse but before returning.
    let count = diagram.element_count();
    if count > DiagramLimits::MAX_NODES {
        return Err(ParseError::TooManyNodes {
            actual: count,
            limit: DiagramLimits::MAX_NODES,
        });
    }

    // Per-type invariants (including accent range).
    match &diagram {
        Diagram::Pyramid(s) => s.validate()?,
        Diagram::Quadrant(s) => s.validate()?,
        Diagram::Tree(s) => s.validate()?,
        Diagram::Layers(s) => s.validate()?,
        Diagram::Flowchart(s) => s.validate()?,
    }

    Ok((diagram.warnings(), diagram).rev_pair())
}

// Tiny helper so `parse` reads naturally — (warnings, diagram) but we want
// the tuple ordered `(diagram, warnings)` in the public signature.
trait RevPair<A, B> {
    fn rev_pair(self) -> (B, A);
}
impl<A, B> RevPair<A, B> for (A, B) {
    fn rev_pair(self) -> (B, A) {
        (self.1, self.0)
    }
}

/// Cheap pre-scan: find `"type"` and check it against the 5 known tags.
/// Returns `Some(tag)` if the tag is present and unknown, `None` otherwise
/// (including when `"type"` is missing — that case is deferred to
/// `serde_json` for a standard Malformed error).
fn pre_scan_unknown_type(body: &str) -> Option<String> {
    const KNOWN: &[&str] = &["pyramid", "quadrant", "tree", "layers", "flowchart"];
    let marker = "\"type\"";
    let idx = body.find(marker)?;
    let rest = &body[idx + marker.len()..];
    // Expect: optional whitespace, `:`, optional whitespace, `"tag"`.
    let after_colon = rest.find(':')?;
    let tail = &rest[after_colon + 1..].trim_start();
    let tail_bytes = tail.as_bytes();
    if tail_bytes.first() != Some(&b'"') {
        return None;
    }
    // Read to the closing quote.
    let mut end = 1;
    while end < tail_bytes.len() && tail_bytes[end] != b'"' {
        end += 1;
    }
    if end >= tail_bytes.len() {
        return None;
    }
    let tag = &tail[1..end];
    if KNOWN.contains(&tag) {
        None
    } else {
        Some(tag.to_string())
    }
}

/// Dispatch to the appropriate per-type layout engine.
#[must_use]
pub fn layout(diagram: &Diagram, ctx: &LayoutContext) -> DiagramLayout {
    diagram.layout(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_contains_position() {
        let bad = r#"{"type":"pyramid","levels":[{"label":"A"},]}"#;
        let err = parse(bad).unwrap_err();
        match err {
            ParseError::Malformed {
                line,
                column: _,
                message: _,
            } => assert_eq!(line, 1),
            _ => panic!("expected Malformed, got {err:?}"),
        }
    }

    #[test]
    fn test_unknown_type_rejected() {
        let body = r#"{"type":"sunburst","data":[]}"#;
        let err = parse(body).unwrap_err();
        assert!(matches!(err, ParseError::UnknownType(ref t) if t == "sunburst"));
    }

    #[test]
    fn test_oversize_body_rejected() {
        // Build a JSON-looking string that exceeds MAX_BODY_BYTES. The gate
        // must fire BEFORE serde_json runs.
        let body = "x".repeat(DiagramLimits::MAX_BODY_BYTES + 1);
        let err = parse(&body).unwrap_err();
        match err {
            ParseError::BodyTooLarge(size) => {
                assert!(size > DiagramLimits::MAX_BODY_BYTES);
            }
            _ => panic!("expected BodyTooLarge, got {err:?}"),
        }
    }

    #[test]
    fn too_many_nodes_rejected() {
        // Build a pyramid with MAX_NODES + 1 levels.
        let mut levels = String::new();
        for i in 0..=DiagramLimits::MAX_NODES {
            if i > 0 {
                levels.push(',');
            }
            levels.push_str(&format!(r#"{{"label":"L{i}"}}"#));
        }
        let body = format!(r#"{{"type":"pyramid","levels":[{levels}]}}"#);
        let err = parse(&body).unwrap_err();
        assert!(matches!(err, ParseError::TooManyNodes { .. }));
    }

    #[test]
    fn density_warning_pyramid_8_levels() {
        let mut levels = String::new();
        for i in 0..8 {
            if i > 0 {
                levels.push(',');
            }
            levels.push_str(&format!(r#"{{"label":"L{i}"}}"#));
        }
        let body = format!(r#"{{"type":"pyramid","levels":[{levels}]}}"#);
        let (_d, warnings) = parse(&body).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::DensityHigh { .. }));
    }

    #[test]
    fn accent_idx_out_of_range_rejected_at_parse() {
        let body = r#"{"type":"pyramid","levels":[{"label":"A"}],"accent_idx":5}"#;
        let err = parse(body).unwrap_err();
        assert!(matches!(
            err,
            ParseError::AccentOutOfRange {
                element_count: 1,
                accent_idx: 5
            }
        ));
    }
}
