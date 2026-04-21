//! Tree / hierarchy diagram. Root at top, children fan below.
//!
//! Layout: depth-wise rows. Horizontal placement uses a two-pass
//! subtree-width algorithm so siblings never overlap. Edges are drawn as
//! straight lines from parent bottom-center to child top-center (per the task
//! brief — note the visual reference uses elbow connectors; v1 punts on that).

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use serde::Deserialize;

/// Soft cap above which [`Warning::DensityHigh`] is emitted.
pub const SOFT_CAP: usize = 20;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "tree";

/// A tree node. Children are built recursively.
#[derive(Debug, Clone, Deserialize)]
pub struct TreeNode {
    pub label: String,
    #[serde(default)]
    pub sublabel: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeNode>,
}

/// JSON schema: `{"type":"tree","root":{...}, "accent_path":optional}`.
///
/// `accent_path` is a list of 0-based child indices from root to the accent
/// node. Empty vec means the root itself. `None` = no accent.
#[derive(Debug, Clone, Deserialize)]
pub struct TreeSpec {
    pub root: TreeNode,
    #[serde(default)]
    pub accent_path: Option<Vec<usize>>,
}

impl TreeSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        if let Some(path) = &self.accent_path
            && !path_exists(&self.root, path)
        {
            let total = count_nodes(&self.root);
            return Err(ParseError::AccentOutOfRange {
                element_count: total,
                // Use the last index in the path as the human-readable
                // "accent_idx" — consumers looking up the exact path can
                // consult the spec.
                accent_idx: path.last().copied().unwrap_or(0),
            });
        }
        Ok(())
    }
}

fn path_exists(node: &TreeNode, path: &[usize]) -> bool {
    let mut cur = node;
    for &i in path {
        match cur.children.get(i) {
            Some(next) => cur = next,
            None => return false,
        }
    }
    let _ = cur;
    true
}

/// Total node count in the tree (root + descendants).
pub fn count_nodes(node: &TreeNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

pub(crate) fn warnings(spec: &TreeSpec) -> Vec<Warning> {
    let total = count_nodes(&spec.root);
    if total > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: total,
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const NODE_WIDTH: f32 = 140.0;
const NODE_HEIGHT: f32 = 44.0;
const MIN_GAP: f32 = 20.0;
const ROW_GAP: f32 = 36.0;

/// Post-order pass: compute the horizontal footprint of each subtree.
fn measure(node: &TreeNode) -> f32 {
    if node.children.is_empty() {
        NODE_WIDTH
    } else {
        let children_w: f32 = node.children.iter().map(measure).sum();
        let gaps = MIN_GAP * (node.children.len().saturating_sub(1)) as f32;
        (children_w + gaps).max(NODE_WIDTH)
    }
}

#[derive(Debug)]
struct PositionedNode<'a> {
    node: &'a TreeNode,
    /// Center-x of the node rect.
    cx: f32,
    /// Top-y of the node rect.
    y: f32,
    accent: bool,
    parent: Option<usize>,
}

fn flatten<'a>(
    spec: &'a TreeSpec,
    origin_x: f32,
    origin_y: f32,
) -> Vec<PositionedNode<'a>> {
    let mut out: Vec<PositionedNode<'a>> = Vec::new();
    place(
        &spec.root,
        origin_x,
        measure(&spec.root),
        origin_y,
        None,
        &[],
        spec.accent_path.as_deref(),
        &mut out,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn place<'a>(
    node: &'a TreeNode,
    band_x: f32,
    band_w: f32,
    y: f32,
    parent: Option<usize>,
    current_path: &[usize],
    accent_path: Option<&[usize]>,
    out: &mut Vec<PositionedNode<'a>>,
) {
    let cx = band_x + band_w / 2.0;
    let my_idx = out.len();
    out.push(PositionedNode {
        node,
        cx,
        y,
        accent: accent_path == Some(current_path),
        parent,
    });

    let children = &node.children;
    if children.is_empty() {
        return;
    }

    let widths: Vec<f32> = children.iter().map(measure).collect();
    let total_children_w: f32 = widths.iter().sum();
    let gaps = MIN_GAP * (children.len().saturating_sub(1)) as f32;
    let content_w = total_children_w + gaps;
    let mut cursor = cx - content_w / 2.0;
    let child_y = y + NODE_HEIGHT + ROW_GAP;

    for (i, (child, child_w)) in children.iter().zip(widths.iter()).enumerate() {
        let mut extended = current_path.to_vec();
        extended.push(i);
        place(
            child,
            cursor,
            *child_w,
            child_y,
            Some(my_idx),
            &extended,
            accent_path,
            out,
        );
        cursor += *child_w + MIN_GAP;
    }
}

/// Lay out the tree. Edges drawn first so nodes paint on top.
#[must_use]
pub fn layout_tree(spec: &TreeSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    let theme = &ctx.theme;

    // Center the tree horizontally in the canvas.
    let total_w = measure(&spec.root);
    let origin_x = (ctx.canvas_width - total_w) / 2.0;
    let origin_y = 24.0;

    let nodes = flatten(spec, origin_x, origin_y);

    // Edges first — straight line from parent bottom-center to child
    // top-center (per task brief).
    for child in &nodes {
        if let Some(pi) = child.parent {
            let parent = &nodes[pi];
            let from = Point::new(parent.cx, parent.y + NODE_HEIGHT);
            let to = Point::new(child.cx, child.y);
            out.push(Primitive::Line {
                from,
                to,
                color: theme.palette.muted,
                stroke_width: theme.stroke_default,
                style: LineStyle::Solid,
            });
        }
    }

    // Nodes
    for n in &nodes {
        let fill = if n.accent {
            theme.palette.accent
        } else {
            theme.palette.paper
        };
        let stroke = if n.accent {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        out.push(Primitive::Rect {
            x: n.cx - NODE_WIDTH / 2.0,
            y: n.y,
            w: NODE_WIDTH,
            h: NODE_HEIGHT,
            fill,
            stroke,
            stroke_width: theme.stroke_default,
            corner_radius: theme.corner_radius,
        });
        out.push(Primitive::Text {
            x: n.cx,
            y: n.y + NODE_HEIGHT / 2.0 + theme.typography.label_size * 0.35,
            text: n.node.label.clone(),
            font_size: theme.typography.label_size,
            color: theme.palette.ink,
            align: TextAlign::Center,
            weight: TextWeight::SemiBold,
        });
        if let Some(sub) = &n.node.sublabel {
            out.push(Primitive::Text {
                x: n.cx,
                y: n.y + NODE_HEIGHT - 6.0,
                text: sub.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.muted,
                align: TextAlign::Center,
                weight: TextWeight::Regular,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> TreeSpec {
        TreeSpec {
            root: TreeNode {
                label: "A".into(),
                sublabel: None,
                children: vec![
                    TreeNode {
                        label: "B".into(),
                        sublabel: None,
                        children: vec![],
                    },
                    TreeNode {
                        label: "C".into(),
                        sublabel: None,
                        children: vec![TreeNode {
                            label: "D".into(),
                            sublabel: None,
                            children: vec![],
                        }],
                    },
                ],
            },
            accent_path: None,
        }
    }

    #[test]
    fn test_tree_stratified_layout() {
        let spec = sample_tree();
        let ctx = LayoutContext::new(800.0, 400.0);
        let layout = layout_tree(&spec, &ctx);

        // Collect node rects by label; verify stratified y-rows.
        let mut rect_for = std::collections::HashMap::new();
        let texts: Vec<&Primitive> = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Text { .. }))
            .collect();
        let rects: Vec<&Primitive> = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect { .. }))
            .collect();
        assert_eq!(rects.len(), 4);

        for r in &rects {
            if let Primitive::Rect { x, y, w, .. } = r {
                // Find the Text at the same center.
                let cx = x + w / 2.0;
                for t in &texts {
                    if let Primitive::Text {
                        x: tx, y: _ty, text, ..
                    } = t
                        && (tx - cx).abs() < 0.01
                        && !rect_for.contains_key(text)
                    {
                        rect_for.insert(text.clone(), *y);
                        break;
                    }
                }
            }
        }

        let y_a = rect_for["A"];
        let y_b = rect_for["B"];
        let y_c = rect_for["C"];
        let y_d = rect_for["D"];

        assert!(y_a < y_b, "A above B");
        assert!((y_b - y_c).abs() < 0.01, "B and C on same row");
        assert!(y_c < y_d, "C above D");

        // B and C don't overlap horizontally.
        let rect_x = |label: &str| {
            for r in &rects {
                if let Primitive::Rect { x, w, .. } = r {
                    let cx = x + w / 2.0;
                    for t in &texts {
                        if let Primitive::Text {
                            x: tx, text: tlabel, ..
                        } = t
                            && (tx - cx).abs() < 0.01
                            && tlabel == label
                        {
                            return (*x, *w);
                        }
                    }
                }
            }
            panic!("rect for {label} not found");
        };
        let (bx, bw) = rect_x("B");
        let (cx, _cw) = rect_x("C");
        assert!(bx + bw <= cx + 0.01, "B right edge must not overlap C");

        // Edges: A→B, A→C, C→D = 3 Lines
        let lines = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Line { .. }))
            .count();
        assert_eq!(lines, 3);
    }

    #[test]
    fn accent_path_marks_one_node() {
        let mut spec = sample_tree();
        spec.accent_path = Some(vec![1, 0]); // root → C → D
        let ctx = LayoutContext::new(800.0, 400.0);
        let layout = layout_tree(&spec, &ctx);
        let accent = ctx.theme.palette.accent;

        let accented = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect { stroke, .. } if *stroke == accent))
            .count();
        assert_eq!(accented, 1);
    }

    #[test]
    fn accent_path_out_of_range_rejected() {
        let mut spec = sample_tree();
        spec.accent_path = Some(vec![0, 5]); // B has no children
        assert!(matches!(
            spec.validate(),
            Err(ParseError::AccentOutOfRange { .. })
        ));
    }

    #[test]
    fn density_warning_fires_above_cap() {
        let mut root = TreeNode {
            label: "root".into(),
            sublabel: None,
            children: Vec::new(),
        };
        for i in 0..25 {
            root.children.push(TreeNode {
                label: format!("c{i}"),
                sublabel: None,
                children: vec![],
            });
        }
        let spec = TreeSpec {
            root,
            accent_path: None,
        };
        let w = warnings(&spec);
        assert_eq!(w.len(), 1);
    }
}
