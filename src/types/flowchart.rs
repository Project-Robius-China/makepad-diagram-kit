//! Flowchart diagram. Vertical single-column layout, topologically sorted.
//!
//! See `robius/diagram-design/references/type-flowchart.md`.
//! Per task brief: single-column, Kahn's algorithm for ordering. No branch
//! layout — that's a v1.1 extension.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{Point, Primitive, TextAlign, TextWeight};
use serde::Deserialize;
use std::collections::HashMap;

/// Soft cap above which [`Warning::DensityHigh`] is emitted.
pub const SOFT_CAP: usize = 15;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "flowchart";

/// Shape hint for a flowchart node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FlowShape {
    /// Rounded rectangle — step / action (default).
    #[default]
    Rect,
    /// Diamond — decision.
    Diamond,
    /// Ellipse / pill — start & end.
    Oval,
}

/// One node in the flowchart.
#[derive(Debug, Clone, Deserialize)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub shape: FlowShape,
    /// Optional eyebrow tag rendered in the top-left corner.
    #[serde(default)]
    pub tag: Option<String>,
}

/// Semantic role for a flowchart edge. Drives stroke + label color.
///
/// * `Default` — regular flow, muted stroke (current v1 behaviour).
/// * `Primary` — load-bearing edge (e.g., "SSR", "main path"); accent hue.
/// * `External` — crosses a trust / system boundary (e.g., "HTTPS"); link hue.
///
/// Matches the editorial reference in
/// `diagram-design/assets/example-architecture.html`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EdgeRole {
    #[default]
    Default,
    Primary,
    External,
}

/// One edge between two nodes. `from` / `to` reference node `id` strings.
#[derive(Debug, Clone, Deserialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub role: EdgeRole,
}

/// JSON schema.
#[derive(Debug, Clone, Deserialize)]
pub struct FlowchartSpec {
    pub nodes: Vec<FlowNode>,
    #[serde(default)]
    pub edges: Vec<FlowEdge>,
    #[serde(default)]
    pub accent_idx: Option<usize>,
}

impl FlowchartSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        if let Some(idx) = self.accent_idx
            && idx >= self.nodes.len()
        {
            return Err(ParseError::AccentOutOfRange {
                element_count: self.nodes.len(),
                accent_idx: idx,
            });
        }
        Ok(())
    }
}

pub(crate) fn warnings(spec: &FlowchartSpec) -> Vec<Warning> {
    if spec.nodes.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.nodes.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const NODE_WIDTH: f32 = 160.0;
const NODE_HEIGHT: f32 = 48.0;
const ROW_GAP: f32 = 32.0;

/// Kahn's topological sort. On cycle, falls back to input order for the
/// remaining nodes — layout is always produced, never panics.
fn topo_order(spec: &FlowchartSpec) -> Vec<usize> {
    let id_to_idx: HashMap<&str, usize> = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let n = spec.nodes.len();
    let mut in_degree = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in &spec.edges {
        if let (Some(&a), Some(&b)) = (id_to_idx.get(e.from.as_str()), id_to_idx.get(e.to.as_str()))
        {
            adj[a].push(b);
            in_degree[b] += 1;
        }
    }

    // Seed queue with zero-indegree nodes in input order.
    let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut out = Vec::with_capacity(n);
    while let Some(i) = queue.first().copied() {
        queue.remove(0);
        out.push(i);
        for &j in &adj[i] {
            in_degree[j] -= 1;
            if in_degree[j] == 0 {
                queue.push(j);
            }
        }
    }

    // Append any leftovers (cycle or orphaned) in input order.
    let visited: std::collections::HashSet<usize> = out.iter().copied().collect();
    for i in 0..n {
        if !visited.contains(&i) {
            out.push(i);
        }
    }
    out
}

/// Draw a single node at `(cx, cy)` of `shape`, pushing primitives.
fn draw_node(
    out: &mut DiagramLayout,
    node: &FlowNode,
    cx: f32,
    cy: f32,
    accent: bool,
    theme: &crate::Theme,
) {
    let x = cx - NODE_WIDTH / 2.0;
    let y = cy - NODE_HEIGHT / 2.0;
    let fill = if accent {
        theme.palette.accent.with_alpha(32)
    } else {
        theme.palette.paper
    };
    let stroke = if accent {
        theme.palette.accent
    } else {
        theme.palette.ink
    };

    match node.shape {
        FlowShape::Diamond => {
            let hx = NODE_WIDTH / 2.0;
            let hy = NODE_HEIGHT / 2.0;
            out.push(Primitive::Polygon {
                points: vec![
                    Point::new(cx, cy - hy),
                    Point::new(cx + hx, cy),
                    Point::new(cx, cy + hy),
                    Point::new(cx - hx, cy),
                ],
                fill,
                stroke,
                stroke_width: theme.stroke_default,
            });
        }
        FlowShape::Oval => {
            out.push(Primitive::Rect {
                x,
                y,
                w: NODE_WIDTH,
                h: NODE_HEIGHT,
                fill,
                stroke,
                stroke_width: theme.stroke_default,
                // Corner radius equal to half-height → pill shape.
                corner_radius: NODE_HEIGHT / 2.0,
            });
        }
        FlowShape::Rect => {
            out.push(Primitive::Rect {
                x,
                y,
                w: NODE_WIDTH,
                h: NODE_HEIGHT,
                fill,
                stroke,
                stroke_width: theme.stroke_default,
                corner_radius: theme.corner_radius,
            });
        }
    }

    // Eyebrow tag — only makes visual sense for Rect/Oval (the diamond has
    // no well-defined top-left corner). Skipped for FlowShape::Diamond.
    if let Some(tag) = &node.tag
        && !matches!(node.shape, FlowShape::Diamond)
    {
        let tag_color = if accent {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        crate::types::eyebrow::push_eyebrow(out, x, y, tag, tag_color);
    }

    // Shift label down when an eyebrow tag is present so they don't
    // overlap horizontally at the node's midline.
    let has_tag = node.tag.is_some() && !matches!(node.shape, FlowShape::Diamond);
    let label_y = if has_tag { cy + 7.0 } else { cy };
    out.push(Primitive::Text {
        x: cx,
        y: label_y,
        text: node.label.clone(),
        font_size: theme.typography.label_size,
        color: theme.palette.ink,
        align: TextAlign::Center,
        weight: TextWeight::SemiBold,
    });
}

/// Layout the flowchart.
#[must_use]
pub fn layout_flowchart(spec: &FlowchartSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.nodes.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let cx = ctx.canvas_width / 2.0;

    // Stack height: total occupancy of all nodes + gaps.
    let n = spec.nodes.len();
    let total_h = n as f32 * NODE_HEIGHT + (n.saturating_sub(1)) as f32 * ROW_GAP;
    let top = ((ctx.canvas_height - total_h) / 2.0).max(24.0);

    let order = topo_order(spec);
    // Map original node index → y position.
    let mut y_center = vec![0.0f32; n];
    for (row, &idx) in order.iter().enumerate() {
        y_center[idx] = top + NODE_HEIGHT / 2.0 + row as f32 * (NODE_HEIGHT + ROW_GAP);
    }

    // Edges first
    let id_to_idx: HashMap<&str, usize> = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    for edge in &spec.edges {
        let (Some(&a), Some(&b)) = (
            id_to_idx.get(edge.from.as_str()),
            id_to_idx.get(edge.to.as_str()),
        ) else {
            continue;
        };
        let from = Point::new(cx, y_center[a] + NODE_HEIGHT / 2.0);
        let to = Point::new(cx, y_center[b] - NODE_HEIGHT / 2.0);
        // Role → stroke color. Primary uses accent, External uses link
        // (blue), Default uses muted. The label tracks the stroke so role
        // reads consistently.
        let edge_color = match edge.role {
            EdgeRole::Default => theme.palette.muted,
            EdgeRole::Primary => theme.palette.accent,
            EdgeRole::External => theme.palette.link,
        };
        out.push(Primitive::Arrow {
            from,
            to,
            color: edge_color,
            stroke_width: theme.stroke_default,
        });
        if let Some(lbl) = &edge.label {
            // Midpoint
            let mx = (from.x + to.x) / 2.0;
            let my = (from.y + to.y) / 2.0;
            out.push(Primitive::Text {
                x: mx + 8.0,
                y: my,
                text: lbl.clone(),
                font_size: theme.typography.annotation_size,
                color: edge_color,
                align: TextAlign::Left,
                weight: TextWeight::Regular,
            });
        }
    }

    // Nodes
    for (i, node) in spec.nodes.iter().enumerate() {
        let accent = spec.accent_idx == Some(i);
        draw_node(&mut out, node, cx, y_center[i], accent, theme);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FlowchartSpec {
        FlowchartSpec {
            nodes: vec![
                FlowNode {
                    id: "start".into(),
                    label: "Start".into(),
                    shape: FlowShape::Oval,
                    tag: None,
                },
                FlowNode {
                    id: "check".into(),
                    label: "OK?".into(),
                    shape: FlowShape::Diamond,
                    tag: None,
                },
                FlowNode {
                    id: "end".into(),
                    label: "End".into(),
                    shape: FlowShape::Oval,
                    tag: None,
                },
            ],
            edges: vec![
                FlowEdge {
                    from: "start".into(),
                    to: "check".into(),
                    label: None,
                    role: EdgeRole::Default,
                },
                FlowEdge {
                    from: "check".into(),
                    to: "end".into(),
                    label: Some("yes".into()),
                    role: EdgeRole::Default,
                },
            ],
            accent_idx: None,
        }
    }

    fn find_node_y(layout: &DiagramLayout, label: &str) -> Option<f32> {
        // Labels correlate with nodes by the text primitive. For diamonds we
        // stored the center via Text.y offset — the center y equals text.y
        // minus the label_size*0.35 offset. Equivalent enough for ordering.
        for p in &layout.primitives {
            if let Primitive::Text { y, text, .. } = p
                && text == label
            {
                return Some(*y);
            }
        }
        None
    }

    #[test]
    fn test_flowchart_vertical() {
        let spec = sample();
        let ctx = LayoutContext::new(400.0, 500.0);
        let layout = layout_flowchart(&spec, &ctx);

        let y_start = find_node_y(&layout, "Start").unwrap();
        let y_check = find_node_y(&layout, "OK?").unwrap();
        let y_end = find_node_y(&layout, "End").unwrap();
        assert!(y_start < y_check, "start above check");
        assert!(y_check < y_end, "check above end");

        // Two arrows
        let arrows = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Arrow { .. }))
            .count();
        assert_eq!(arrows, 2);

        // The "yes" edge label is present.
        assert!(layout.primitives.iter().any(|p| matches!(
            p,
            Primitive::Text { text, .. } if text == "yes"
        )));

        // Diamond for "check" → expect a Polygon.
        let polys = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Polygon { .. }))
            .count();
        assert_eq!(polys, 1);
    }

    #[test]
    fn topo_orders_linear_chain() {
        let spec = sample();
        let order = topo_order(&spec);
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn density_warning_fires_above_cap() {
        let mut spec = sample();
        for i in 0..20 {
            spec.nodes.push(FlowNode {
                id: format!("n{i}"),
                label: format!("N{i}"),
                shape: FlowShape::Rect,
                tag: None,
            });
        }
        let w = warnings(&spec);
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn accent_idx_out_of_range_rejected() {
        let mut spec = sample();
        spec.accent_idx = Some(99);
        assert!(matches!(
            spec.validate(),
            Err(ParseError::AccentOutOfRange { .. })
        ));
    }

    #[test]
    fn edge_role_colors() {
        // Scenario: three edges with Default / Primary / External roles →
        // their arrow strokes must match muted / accent / link.
        let spec = FlowchartSpec {
            nodes: vec![
                FlowNode {
                    id: "a".into(),
                    label: "A".into(),
                    shape: FlowShape::Rect,
                    tag: None,
                },
                FlowNode {
                    id: "b".into(),
                    label: "B".into(),
                    shape: FlowShape::Rect,
                    tag: None,
                },
                FlowNode {
                    id: "c".into(),
                    label: "C".into(),
                    shape: FlowShape::Rect,
                    tag: None,
                },
                FlowNode {
                    id: "d".into(),
                    label: "D".into(),
                    shape: FlowShape::Rect,
                    tag: None,
                },
            ],
            edges: vec![
                FlowEdge {
                    from: "a".into(),
                    to: "b".into(),
                    label: None,
                    role: EdgeRole::Default,
                },
                FlowEdge {
                    from: "b".into(),
                    to: "c".into(),
                    label: Some("SSR".into()),
                    role: EdgeRole::Primary,
                },
                FlowEdge {
                    from: "c".into(),
                    to: "d".into(),
                    label: Some("HTTPS".into()),
                    role: EdgeRole::External,
                },
            ],
            accent_idx: None,
        };
        let ctx = LayoutContext::new(400.0, 500.0);
        let layout = layout_flowchart(&spec, &ctx);

        let pal = ctx.theme.palette;
        let arrow_colors: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Arrow { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(arrow_colors, vec![pal.muted, pal.accent, pal.link]);

        // The label for a Primary edge uses the accent color, External uses
        // link — verified by scanning the text primitives.
        let has_primary_label = layout.primitives.iter().any(|p| {
            matches!(p, Primitive::Text { text, color, .. } if text == "SSR" && *color == pal.accent)
        });
        assert!(has_primary_label, "Primary edge label must use accent");
        let has_external_label = layout.primitives.iter().any(|p| {
            matches!(p, Primitive::Text { text, color, .. } if text == "HTTPS" && *color == pal.link)
        });
        assert!(has_external_label, "External edge label must use link");
    }
}
