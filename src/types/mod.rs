//! Per-type diagram specs and layout engines.
//!
//! Each submodule defines:
//! * A `serde::Deserialize` spec struct (the JSON body schema)
//! * A `validate` helper that applies limits and builds the warning list
//! * A `layout_*` function that positions the primitives
//! * Unit tests covering the scenarios named in `specs/m-diagram-v1.spec.md`
//!
//! The top-level [`Diagram`] enum discriminates on the JSON `"type"` field.

pub mod flowchart;
pub mod layers;
pub mod pyramid;
pub mod quadrant;
pub mod tree;

pub use flowchart::{FlowchartSpec, layout_flowchart};
pub use layers::{LayersSpec, layout_layers};
pub use pyramid::{PyramidSpec, layout_pyramid};
pub use quadrant::{QuadrantSpec, layout_quadrant};
pub use tree::{TreeSpec, layout_tree};

use crate::errors::Warning;
use crate::layout::{DiagramLayout, LayoutContext};
use serde::Deserialize;

/// Top-level diagram. Discriminated on the `"type"` field in the JSON body.
///
/// ```
/// # use makepad_diagram_kit::{parse, Diagram};
/// let json = r#"{"type":"pyramid","levels":[{"label":"A"},{"label":"B"}]}"#;
/// let (diagram, _warnings) = parse(json).unwrap();
/// assert!(matches!(diagram, Diagram::Pyramid(_)));
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Diagram {
    Pyramid(PyramidSpec),
    Quadrant(QuadrantSpec),
    Tree(TreeSpec),
    Layers(LayersSpec),
    Flowchart(FlowchartSpec),
}

impl Diagram {
    /// Stable string tag used in warnings and diagnostic output.
    #[must_use]
    pub fn type_tag(&self) -> &'static str {
        match self {
            Diagram::Pyramid(_) => "pyramid",
            Diagram::Quadrant(_) => "quadrant",
            Diagram::Tree(_) => "tree",
            Diagram::Layers(_) => "layers",
            Diagram::Flowchart(_) => "flowchart",
        }
    }

    /// Total element count for density / size gating. "Element" is
    /// type-specific: pyramid levels, tree nodes, etc.
    #[must_use]
    pub fn element_count(&self) -> usize {
        match self {
            Diagram::Pyramid(s) => s.levels.len(),
            Diagram::Quadrant(s) => s.points.len(),
            Diagram::Tree(s) => tree::count_nodes(&s.root),
            Diagram::Layers(s) => s.layers.len(),
            Diagram::Flowchart(s) => s.nodes.len(),
        }
    }

    /// Collect density warnings that apply post-parse. No [`crate::ParseError`]
    /// shapes here — errors are caught earlier at validation.
    #[must_use]
    pub fn warnings(&self) -> Vec<Warning> {
        match self {
            Diagram::Pyramid(s) => pyramid::warnings(s),
            Diagram::Quadrant(s) => quadrant::warnings(s),
            Diagram::Tree(s) => tree::warnings(s),
            Diagram::Layers(s) => layers::warnings(s),
            Diagram::Flowchart(s) => flowchart::warnings(s),
        }
    }

    /// Dispatch to the per-type layout function.
    #[must_use]
    pub fn layout(&self, ctx: &LayoutContext) -> DiagramLayout {
        match self {
            Diagram::Pyramid(s) => layout_pyramid(s, ctx),
            Diagram::Quadrant(s) => layout_quadrant(s, ctx),
            Diagram::Tree(s) => layout_tree(s, ctx),
            Diagram::Layers(s) => layout_layers(s, ctx),
            Diagram::Flowchart(s) => layout_flowchart(s, ctx),
        }
    }
}
