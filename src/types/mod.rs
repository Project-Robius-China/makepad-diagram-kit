//! Per-type diagram specs and layout engines.
//!
//! Each submodule defines:
//! * A `serde::Deserialize` spec struct (the JSON body schema)
//! * A `validate` helper that applies limits and builds the warning list
//! * A `layout_*` function that positions the primitives
//! * Unit tests covering the scenarios named in `specs/m-diagram-v1.spec.md`
//!
//! The top-level [`Diagram`] enum discriminates on the JSON `"type"` field.

pub mod architecture;
pub mod er;
pub mod flowchart;
pub mod layers;
pub mod nested;
pub mod pyramid;
pub mod quadrant;
pub mod sequence;
pub mod state;
pub mod swimlane;
pub mod timeline;
pub mod tree;
pub mod venn;

// Shared eyebrow-tag helper used by node-based diagram types (tree,
// flowchart, pyramid). Kept package-private: consumers should not render
// tags directly — they set `tag: Option<String>` on nodes and the layout
// engines emit the primitives.
pub(crate) mod eyebrow;

// Cross-type helpers (edge role → color, etc.) — kept crate-private, only
// the layout engines consume them. Exposed via the shared module so a
// single rule governs both `flowchart` and `architecture` strokes.
pub(crate) mod shared;

pub use architecture::{ArchitectureSpec, layout_architecture};
pub use er::{ErSpec, layout_er};
pub use flowchart::{FlowchartSpec, layout_flowchart};
pub use layers::{LayersSpec, layout_layers};
pub use nested::{NestedSpec, layout_nested};
pub use pyramid::{PyramidSpec, layout_pyramid};
pub use quadrant::{QuadrantSpec, layout_quadrant};
pub use sequence::{SequenceSpec, layout_sequence};
pub use state::{StateSpec, layout_state};
pub use swimlane::{SwimlaneSpec, layout_swimlane};
pub use timeline::{TimelineSpec, layout_timeline};
pub use tree::{TreeSpec, layout_tree};
pub use venn::{VennSpec, layout_venn};

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
    Architecture(ArchitectureSpec),
    Sequence(SequenceSpec),
    State(StateSpec),
    Er(ErSpec),
    Timeline(TimelineSpec),
    Swimlane(SwimlaneSpec),
    Nested(NestedSpec),
    Venn(VennSpec),
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
            Diagram::Architecture(_) => "architecture",
            Diagram::Sequence(_) => "sequence",
            Diagram::State(_) => "state",
            Diagram::Er(_) => "er",
            Diagram::Timeline(_) => "timeline",
            Diagram::Swimlane(_) => "swimlane",
            Diagram::Nested(_) => "nested",
            Diagram::Venn(_) => "venn",
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
            Diagram::Architecture(s) => s.nodes.len(),
            Diagram::Sequence(s) => s.actors.len() + s.messages.len(),
            Diagram::State(s) => s.states.len() + s.transitions.len(),
            Diagram::Er(s) => s.entities.len() + s.relationships.len(),
            Diagram::Timeline(s) => s.events.len(),
            Diagram::Swimlane(s) => s.lanes.len() + s.steps.len(),
            Diagram::Nested(s) => s.levels.len(),
            Diagram::Venn(s) => s.sets.len() + s.intersections.len(),
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
            Diagram::Architecture(s) => architecture::warnings(s),
            Diagram::Sequence(s) => sequence::warnings(s),
            Diagram::State(s) => state::warnings(s),
            Diagram::Er(s) => er::warnings(s),
            Diagram::Timeline(s) => timeline::warnings(s),
            Diagram::Swimlane(s) => swimlane::warnings(s),
            Diagram::Nested(s) => nested::warnings(s),
            Diagram::Venn(s) => venn::warnings(s),
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
            Diagram::Architecture(s) => layout_architecture(s, ctx),
            Diagram::Sequence(s) => layout_sequence(s, ctx),
            Diagram::State(s) => layout_state(s, ctx),
            Diagram::Er(s) => layout_er(s, ctx),
            Diagram::Timeline(s) => layout_timeline(s, ctx),
            Diagram::Swimlane(s) => layout_swimlane(s, ctx),
            Diagram::Nested(s) => layout_nested(s, ctx),
            Diagram::Venn(s) => layout_venn(s, ctx),
        }
    }
}
