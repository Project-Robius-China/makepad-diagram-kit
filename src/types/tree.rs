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
    /// Optional eyebrow tag rendered in the top-left of the node box — e.g.
    /// "ROOT", "CAT", "EXT". Uppercase mono, small.
    #[serde(default)]
    pub tag: Option<String>,
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

const NODE_WIDTH: f32 = 160.0;
const NODE_HEIGHT: f32 = 48.0;
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

    // Edges first — orthogonal elbow routing (vertical → horizontal →
    // vertical). Each segment is axis-aligned so the renderer's
    // AABB-strip approximation of a Line primitive renders pixel-perfect.
    // Diagonal lines would degrade to visually wrong horizontal/vertical
    // strips; the elbow pattern avoids that entirely and reads as a
    // cleaner org-chart aesthetic anyway.
    for child in &nodes {
        if let Some(pi) = child.parent {
            let parent = &nodes[pi];
            let px = parent.cx;
            let py = parent.y + NODE_HEIGHT;
            let cx_pos = child.cx;
            let cy_pos = child.y;
            let mid_y = (py + cy_pos) * 0.5;
            let color = theme.palette.muted;
            let stroke_width = theme.stroke_default;
            let style = LineStyle::Solid;
            // Segment 1: parent bottom → horizontal runner
            out.push(Primitive::Line {
                from: Point::new(px, py),
                to: Point::new(px, mid_y),
                color,
                stroke_width,
                style,
            });
            // Segment 2: horizontal runner (skip if parent + child share x)
            if (px - cx_pos).abs() > 0.5 {
                out.push(Primitive::Line {
                    from: Point::new(px, mid_y),
                    to: Point::new(cx_pos, mid_y),
                    color,
                    stroke_width,
                    style,
                });
            }
            // Segment 3: horizontal runner → child top
            out.push(Primitive::Line {
                from: Point::new(cx_pos, mid_y),
                to: Point::new(cx_pos, cy_pos),
                color,
                stroke_width,
                style,
            });
        }
    }

    // Nodes
    for n in &nodes {
        // Role-based fills (mirrors diagram-design tree.html editorial skin):
        //   - accent node        → accent_tint fill + accent stroke (focal)
        //   - root (no parent)   → paper fill + ink stroke (branch head)
        //   - branch (has kids)  → paper fill + ink stroke
        //   - leaf               → leaf_tint (ink @ 5%) + soft stroke (thin 0.8)
        let is_leaf = n.node.children.is_empty();
        let is_root = n.parent.is_none();
        let (fill, stroke, stroke_w) = if n.accent {
            (
                theme.palette.accent_tint,
                theme.palette.accent,
                theme.stroke_default,
            )
        } else if is_leaf && !is_root {
            (theme.palette.leaf_tint, theme.palette.soft, 0.8)
        } else {
            (theme.palette.paper, theme.palette.ink, theme.stroke_default)
        };
        let node_x = n.cx - NODE_WIDTH / 2.0;
        out.push(Primitive::Rect {
            x: node_x,
            y: n.y,
            w: NODE_WIDTH,
            h: NODE_HEIGHT,
            fill,
            stroke,
            stroke_width: stroke_w,
            corner_radius: theme.corner_radius,
        });
        // Eyebrow tag: small outlined pill in the top-left. Tag text is the
        // user-supplied `tag` on the node; color tracks the node's role
        // (accent hue for the focal node, otherwise the same ink stroke).
        if let Some(tag) = &n.node.tag {
            let tag_color = if n.accent {
                theme.palette.accent
            } else {
                theme.palette.ink
            };
            crate::types::eyebrow::push_eyebrow(&mut out, node_x, n.y, tag, tag_color);
        }
        // Primary label. Sublabel-aware vertical offset: with a sublabel the
        // label sits above center; without, dead center.
        let has_sub = n.node.sublabel.is_some();
        let label_y = if has_sub {
            n.y + NODE_HEIGHT * 0.38 + theme.typography.label_size * 0.35
        } else {
            n.y + NODE_HEIGHT / 2.0 + theme.typography.label_size * 0.35
        };
        let label_color = if n.accent {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        out.push(Primitive::Text {
            x: n.cx,
            y: label_y,
            text: n.node.label.clone(),
            font_size: theme.typography.label_size,
            color: label_color,
            align: TextAlign::Center,
            weight: TextWeight::SemiBold,
        });
        if let Some(sub) = &n.node.sublabel {
            out.push(Primitive::Text {
                x: n.cx,
                y: n.y + NODE_HEIGHT * 0.72,
                text: sub.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.soft,
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
                tag: None,
                children: vec![
                    TreeNode {
                        label: "B".into(),
                        sublabel: None,
                        tag: None,
                        children: vec![],
                    },
                    TreeNode {
                        label: "C".into(),
                        sublabel: None,
                        tag: None,
                        children: vec![TreeNode {
                            label: "D".into(),
                            sublabel: None,
                            tag: None,
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

        // Edges: A→B, A→C, C→D = 3 parent→child relationships. With
        // orthogonal elbow routing each generates up to 3 axis-aligned
        // Line segments (vertical ↘ horizontal ↘ vertical); when parent
        // and child share x the middle segment collapses, yielding 2
        // segments for that relationship. So 3 edges ⇒ between 6 and 9
        // Line primitives.
        let lines = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Line { .. }))
            .count();
        assert!(
            (6..=9).contains(&lines),
            "expected 6-9 line segments (3 elbow edges), got {}",
            lines
        );
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
            tag: None,
            children: Vec::new(),
        };
        for i in 0..25 {
            root.children.push(TreeNode {
                label: format!("c{i}"),
                sublabel: None,
                tag: None,
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

    #[test]
    fn eyebrow_tag_rendered() {
        // Scenario: a TreeNode with `tag: Some("ROOT")` generates exactly 2
        // extra primitives — the outline rect and the uppercase text —
        // beyond the 4 nodes already in the sample tree.
        let mut spec = sample_tree();
        spec.root.tag = Some("root".into()); // lowercase input — should uppercase
        let ctx = LayoutContext::new(800.0, 400.0);
        let with_tag = layout_tree(&spec, &ctx).primitives.len();

        let mut baseline_spec = sample_tree();
        baseline_spec.root.tag = None;
        let baseline = layout_tree(&baseline_spec, &ctx).primitives.len();

        assert_eq!(
            with_tag,
            baseline + 2,
            "eyebrow tag must add exactly a rect + a text primitive"
        );

        // And the text is uppercased.
        let had_root_text = layout_tree(&spec, &ctx)
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text { text, .. } if text == "ROOT"));
        assert!(had_root_text, "eyebrow text must be uppercased");
    }
}
