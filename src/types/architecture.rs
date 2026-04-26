//! Architecture diagram. 2D layered layout for system / data-flow diagrams.
//!
//! See `robius/diagram-design/references/type-architecture.md` and
//! `assets/example-architecture.html`. The target aesthetic is the
//! Cloudflare → Astro Origin → MDX Bundle flow — role-tagged boxes, edges
//! with semantic colors, 2D horizontal (or vertical) layout.
//!
//! # Layout
//!
//! This v1 ships a hand-written BFS layered layout rather than pulling in
//! the `rusty-mermaid-dagre` crate (which would require two additional
//! direct deps — `rusty-mermaid-graph` and `rusty-mermaid-core` — since
//! neither `Graph` nor `Direction` are re-exported). The BFS variant is
//! deterministic, stable under re-orderings of the JSON, and adequate for
//! the 3-8 node complexity typical of architecture sketches. Reach for
//! Dagre only if/when ports, clusters, or orthogonal edge routing become
//! required.
//!
//! # Role treatments
//!
//! Node roles map to fill/stroke per the diagram-design style-guide
//! "Node type → treatment" table. Since the v1 Palette does not expose
//! separate `ink @ 0.05` / `ink @ 0.03` / `ink @ 0.02` variants, the
//! `external` / `input` / `optional` roles collapse onto existing tokens
//! (`leaf_tint`, `muted`/`soft`) — see [`role_colors`]. Dashed borders for
//! `optional` / `security` are deferred to v1.1 along with the
//! `LineStyle::Dashed` primitive wiring.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use crate::theme::{Color, Theme};
use crate::types::flowchart::EdgeRole;
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};

/// Soft cap above which [`Warning::DensityHigh`] is emitted. Architecture
/// diagrams stay readable up to ~12 nodes; beyond that the 2D layout
/// starts to overflow the canvas.
pub const SOFT_CAP: usize = 12;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "architecture";

/// Semantic role for an architecture node. Drives fill + stroke color per
/// the diagram-design style-guide "Node type → treatment" table.
///
/// Only `Focal` promotes to accent; the other roles use existing muted /
/// rule / leaf_tint tokens so the palette stays intentional (one accent
/// per diagram).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    /// Backend service, container, or compute surface. Paper fill, ink
    /// stroke — the default "box" treatment. Default if role is omitted.
    #[default]
    Backend,
    /// Focal node — the load-bearing component (e.g. the origin SSR
    /// renderer in the reference HTML). Accent_tint fill + accent stroke.
    /// Convention: at most 1-2 per diagram.
    Focal,
    /// Data store / bundle / artifact. Leaf_tint fill + muted stroke.
    Store,
    /// External system (browser, third-party API). Leaf_tint fill + rule
    /// stroke — quiet so the interior flow stays the hero.
    External,
    /// User / batch input. Leaf_tint fill + soft stroke.
    Input,
    /// Optional surface. V1 renders solid; v1.1 will emit
    /// [`crate::primitive::LineStyle::Dashed`] once that round-trips to
    /// the Makepad renderer.
    Optional,
    /// Security / trust-boundary node. V1 renders solid accent; v1.1
    /// adds the dashed border pending renderer support.
    Security,
}

/// One node in the architecture diagram.
#[derive(Debug, Clone, Deserialize)]
pub struct ArchNode {
    /// Unique ID referenced by edges. Not rendered.
    pub id: String,
    /// Primary label (human-readable name, Geist sans).
    pub label: String,
    /// Eyebrow tag rendered top-left — typically a 3-4 letter code
    /// ("EDGE", "ORIG", "BUN") that hints at the node's type.
    #[serde(default)]
    pub tag: Option<String>,
    /// Second line under the label (mono, smaller) — e.g. "Pages · cache",
    /// "SSR + MDX", "assets · og images".
    #[serde(default)]
    pub sublabel: Option<String>,
    /// Role → fill/stroke treatment. Defaults to `Backend` when omitted.
    #[serde(default)]
    pub role: NodeRole,
}

/// One directed edge between two nodes. `from` / `to` reference node
/// `id` strings. Shares [`EdgeRole`] with `flowchart` so the dispatch
/// helper stays in one place.
#[derive(Debug, Clone, Deserialize)]
pub struct ArchEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub role: EdgeRole,
}

/// Layout orientation — which axis the primary flow runs along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    /// Left → right (the reference HTML's orientation). Default.
    #[default]
    Lr,
    /// Top → bottom.
    Tb,
}

/// Architecture JSON spec.
///
/// ```json
/// {
///   "type": "architecture",
///   "orientation": "lr",
///   "nodes": [
///     {"id":"reader","label":"Reader","tag":"EXT","role":"external"},
///     {"id":"edge","label":"Cloudflare","tag":"EDGE","role":"backend",
///      "sublabel":"Pages · cache"},
///     {"id":"orig","label":"Astro Origin","tag":"ORIG","role":"focal",
///      "sublabel":"SSR + MDX"}
///   ],
///   "edges": [
///     {"from":"reader","to":"edge","label":"HTTPS","role":"external"},
///     {"from":"edge","to":"orig","label":"SSR","role":"primary"}
///   ]
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ArchitectureSpec {
    pub nodes: Vec<ArchNode>,
    #[serde(default)]
    pub edges: Vec<ArchEdge>,
    #[serde(default)]
    pub orientation: Orientation,
}

impl ArchitectureSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        // No accent_idx on architecture — role drives accent. Nothing to
        // cross-check beyond id uniqueness and edge endpoints, which the
        // layout tolerates (unknown ids are skipped).
        Ok(())
    }
}

pub(crate) fn warnings(spec: &ArchitectureSpec) -> Vec<Warning> {
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

// --- Visual constants ---------------------------------------------------

/// Node rect width. Matches tree.rs so corpus + gallery render consistent.
pub(crate) const NODE_WIDTH: f32 = 160.0;
/// Node rect height. Matches tree.rs.
pub(crate) const NODE_HEIGHT: f32 = 48.0;
/// Horizontal gap between adjacent rank columns (LR) / rows (TB).
const RANK_GAP: f32 = 72.0;
/// Gap between siblings within the same rank.
const SIBLING_GAP: f32 = 32.0;
/// Canvas inset from the top/left edge.
const MARGIN: f32 = 24.0;

// --- Role → palette -----------------------------------------------------

/// Map a [`NodeRole`] to its (fill, stroke, stroke_width) triple under the
/// given theme.
///
/// Public at the crate level so the unit test in this module and any
/// future gallery tooling can verify the mapping without re-deriving it.
#[must_use]
pub(crate) fn role_colors(role: NodeRole, theme: &Theme) -> (Color, Color, f32) {
    let pal = theme.palette;
    match role {
        NodeRole::Focal => (pal.accent_tint, pal.accent, theme.stroke_default),
        NodeRole::Backend => (pal.paper, pal.ink, theme.stroke_default),
        NodeRole::Store => (pal.leaf_tint, pal.muted, theme.stroke_default),
        // external is meant to be "lighter than store" — we approximate by
        // reusing leaf_tint but dropping to the rule color for the stroke
        // so the box reads as quieter than a store.
        NodeRole::External => (pal.leaf_tint, pal.rule, theme.stroke_default),
        NodeRole::Input => (pal.leaf_tint, pal.soft, theme.stroke_default),
        // v1.1: dashed border pending LineStyle wiring through the
        // Makepad renderer. Same palette, solid for now.
        NodeRole::Optional => (pal.leaf_tint, pal.rule, theme.stroke_default),
        NodeRole::Security => (pal.accent_tint, pal.accent, theme.stroke_default),
    }
}

/// Does this role promote its eyebrow tag + label to the accent hue? Only
/// `Focal` does — matches tree.rs / flowchart.rs accent rule.
#[inline]
fn role_is_accent(role: NodeRole) -> bool {
    matches!(role, NodeRole::Focal | NodeRole::Security)
}

// --- Rank assignment (BFS / longest-path) -------------------------------

/// Assign each node a non-negative rank such that rank(to) > rank(from) for
/// every edge. Uses Kahn's algorithm on the DAG (cycles are broken by
/// keeping the earliest-seen rank); this gives the "longest path from a
/// source" layering that Dagre produces by default.
///
/// Returns one vector with `ranks[i] = rank(nodes[i])`.
fn assign_ranks(spec: &ArchitectureSpec) -> Vec<usize> {
    let n = spec.nodes.len();
    let id_to_idx: HashMap<&str, usize> = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_deg = vec![0usize; n];
    for e in &spec.edges {
        if let (Some(&a), Some(&b)) = (id_to_idx.get(e.from.as_str()), id_to_idx.get(e.to.as_str()))
            && a != b
        {
            adj[a].push(b);
            in_deg[b] += 1;
        }
    }

    // Sources (in-degree 0) start at rank 0. BFS assigns max(rank(pred)+1)
    // to each node — the longest-path rule.
    let mut rank = vec![0usize; n];
    let mut q: VecDeque<usize> = VecDeque::new();
    for (i, degree) in in_deg.iter().enumerate() {
        if *degree == 0 {
            q.push_back(i);
        }
    }
    let mut remaining = in_deg.clone();
    while let Some(i) = q.pop_front() {
        for &j in &adj[i] {
            rank[j] = rank[j].max(rank[i] + 1);
            remaining[j] -= 1;
            if remaining[j] == 0 {
                q.push_back(j);
            }
        }
    }

    // Any nodes still with non-zero remaining participated in a cycle —
    // leave them at whatever rank Kahn managed to assign (0 if they never
    // got touched). This is defensive; `validate` passes cycles through.
    rank
}

/// Group node indices by rank, preserving input order within each rank so
/// JSON ordering controls sibling placement.
fn group_by_rank(ranks: &[usize]) -> Vec<Vec<usize>> {
    let max_rank = ranks.iter().copied().max().unwrap_or(0);
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
    for (i, &r) in ranks.iter().enumerate() {
        groups[r].push(i);
    }
    groups
}

// --- Rendering ----------------------------------------------------------

/// Push primitives for one node at top-left `(x, y)`.
fn draw_node(out: &mut DiagramLayout, node: &ArchNode, x: f32, y: f32, theme: &Theme) {
    let (fill, stroke, stroke_w) = role_colors(node.role, theme);
    out.push(Primitive::Rect {
        x,
        y,
        w: NODE_WIDTH,
        h: NODE_HEIGHT,
        fill,
        stroke,
        stroke_width: stroke_w,
        corner_radius: theme.corner_radius,
    });

    let accent = role_is_accent(node.role);
    if let Some(tag) = &node.tag {
        let tag_color = if accent {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        crate::types::eyebrow::push_eyebrow(out, x, y, tag, tag_color);
    }

    // Label position follows tree.rs convention: shift down when a tag is
    // present (tag sits top-left), shift up when a sublabel is present
    // (sublabel sits underneath).
    let has_tag = node.tag.is_some();
    let has_sub = node.sublabel.is_some();
    let mut label_y = y + NODE_HEIGHT / 2.0;
    if has_tag {
        label_y += 7.0;
    }
    if has_sub {
        label_y -= 5.0;
    }
    let label_color = if accent {
        theme.palette.accent
    } else {
        theme.palette.ink
    };
    out.push(Primitive::Text {
        x: x + NODE_WIDTH / 2.0,
        y: label_y,
        text: node.label.clone(),
        font_size: theme.typography.label_size,
        color: label_color,
        align: TextAlign::Center,
        weight: TextWeight::SemiBold,
    });
    if let Some(sub) = &node.sublabel {
        out.push(Primitive::Text {
            x: x + NODE_WIDTH / 2.0,
            y: y + NODE_HEIGHT * 0.72,
            text: sub.clone(),
            font_size: theme.typography.sublabel_size,
            color: theme.palette.soft,
            align: TextAlign::Center,
            weight: TextWeight::Regular,
        });
    }
}

/// Layout the architecture diagram.
#[must_use]
pub fn layout_architecture(spec: &ArchitectureSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.nodes.is_empty() {
        return out;
    }
    let theme = &ctx.theme;

    // 1. Rank assignment: longest-path BFS.
    let ranks = assign_ranks(spec);
    let groups = group_by_rank(&ranks);

    // 2. Per-node centers keyed by node index. We compute them by walking
    //    ranks left-to-right (LR) or top-to-bottom (TB) and placing each
    //    rank's members evenly along the perpendicular axis.
    let n = spec.nodes.len();
    let mut centers = vec![Point::new(0.0, 0.0); n];

    match spec.orientation {
        Orientation::Lr => {
            // x ← rank, y ← order within rank.
            // Perpendicular extent (height) determined by the widest rank.
            let max_rank_size = groups.iter().map(Vec::len).max().unwrap_or(1);
            let rank_total_h = max_rank_size as f32 * NODE_HEIGHT
                + (max_rank_size.saturating_sub(1)) as f32 * SIBLING_GAP;
            let canvas_mid_y = (ctx.canvas_height.max(rank_total_h + 2.0 * MARGIN)) / 2.0;

            for (r, group) in groups.iter().enumerate() {
                let k = group.len();
                let band_h = k as f32 * NODE_HEIGHT + (k.saturating_sub(1)) as f32 * SIBLING_GAP;
                let top = canvas_mid_y - band_h / 2.0;
                let x_center = MARGIN + NODE_WIDTH / 2.0 + r as f32 * (NODE_WIDTH + RANK_GAP);
                for (i, &idx) in group.iter().enumerate() {
                    let y_center = top + NODE_HEIGHT / 2.0 + i as f32 * (NODE_HEIGHT + SIBLING_GAP);
                    centers[idx] = Point::new(x_center, y_center);
                }
            }
        }
        Orientation::Tb => {
            let max_rank_size = groups.iter().map(Vec::len).max().unwrap_or(1);
            let rank_total_w = max_rank_size as f32 * NODE_WIDTH
                + (max_rank_size.saturating_sub(1)) as f32 * SIBLING_GAP;
            let canvas_mid_x = (ctx.canvas_width.max(rank_total_w + 2.0 * MARGIN)) / 2.0;

            for (r, group) in groups.iter().enumerate() {
                let k = group.len();
                let band_w = k as f32 * NODE_WIDTH + (k.saturating_sub(1)) as f32 * SIBLING_GAP;
                let left = canvas_mid_x - band_w / 2.0;
                let y_center = MARGIN + NODE_HEIGHT / 2.0 + r as f32 * (NODE_HEIGHT + RANK_GAP);
                for (i, &idx) in group.iter().enumerate() {
                    let x_center = left + NODE_WIDTH / 2.0 + i as f32 * (NODE_WIDTH + SIBLING_GAP);
                    centers[idx] = Point::new(x_center, y_center);
                }
            }
        }
    }

    // 3. Edges first (so boxes paint on top). Role → stroke color via the
    //    shared helper — same dispatch flowchart uses. Routing is a single
    //    straight Arrow from the source rect's midpoint (out-side) to the
    //    target rect's midpoint (in-side) — adequate for the typical 3-8
    //    node layouts. Orthogonal elbow routing is a v1.1 extension once
    //    rank-spanning edges need dodging.
    let id_to_idx: HashMap<&str, usize> = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    for edge in &spec.edges {
        let (Some(&a), Some(&b)) = (
            id_to_idx.get(edge.from.as_str()),
            id_to_idx.get(edge.to.as_str()),
        ) else {
            continue;
        };
        if a == b {
            continue;
        }
        let from_c = centers[a];
        let to_c = centers[b];
        let (from, to) = match spec.orientation {
            Orientation::Lr => (
                Point::new(from_c.x + NODE_WIDTH / 2.0, from_c.y),
                Point::new(to_c.x - NODE_WIDTH / 2.0, to_c.y),
            ),
            Orientation::Tb => (
                Point::new(from_c.x, from_c.y + NODE_HEIGHT / 2.0),
                Point::new(to_c.x, to_c.y - NODE_HEIGHT / 2.0),
            ),
        };
        let color = crate::types::shared::edge_color_for_role(edge.role, theme);
        out.push(Primitive::Arrow {
            from,
            to,
            color,
            stroke_width: theme.stroke_default,
            style: LineStyle::Solid,
        });
        if let Some(lbl) = &edge.label {
            let mx = (from.x + to.x) / 2.0;
            let my = (from.y + to.y) / 2.0;
            // Small vertical lift for LR so the label sits above the
            // arrow shaft rather than on top of it. TB lifts horizontally.
            let (tx, ty, align) = match spec.orientation {
                Orientation::Lr => (mx, my - 6.0, TextAlign::Center),
                Orientation::Tb => (mx + 8.0, my, TextAlign::Left),
            };
            out.push(Primitive::Text {
                x: tx,
                y: ty,
                text: lbl.clone(),
                font_size: theme.typography.annotation_size,
                color,
                align,
                weight: TextWeight::Regular,
            });
        }
    }

    // 4. Nodes.
    for (i, node) in spec.nodes.iter().enumerate() {
        let c = centers[i];
        draw_node(
            &mut out,
            node,
            c.x - NODE_WIDTH / 2.0,
            c.y - NODE_HEIGHT / 2.0,
            theme,
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse};

    const README_JSON: &str = r#"{
      "type": "architecture",
      "orientation": "lr",
      "nodes": [
        {"id":"reader","label":"Reader","tag":"EXT","role":"external"},
        {"id":"edge","label":"Cloudflare","tag":"EDGE","role":"backend","sublabel":"Pages · cache"},
        {"id":"orig","label":"Astro Origin","tag":"ORIG","role":"focal","sublabel":"SSR + MDX"},
        {"id":"bun","label":"MDX Bundle","tag":"BUN","role":"backend"},
        {"id":"cms","label":"Content CMS","tag":"CMS","role":"store"}
      ],
      "edges": [
        {"from":"reader","to":"edge","label":"HTTPS","role":"external"},
        {"from":"edge","to":"orig","label":"SSR","role":"primary"},
        {"from":"orig","to":"bun","label":"READ MDX"},
        {"from":"orig","to":"cms","label":"QUERY"}
      ]
    }"#;

    /// Scenario 1 in the task brief: the README architecture JSON (5 nodes,
    /// 4 edges) parses end-to-end and the layout positions all 5 nodes.
    #[test]
    fn architecture_basic_parse() {
        let (diagram, warnings) = parse(README_JSON).unwrap_or_else(|e| panic!("parse: {e}"));
        assert!(matches!(diagram, Diagram::Architecture(_)));
        assert!(warnings.is_empty(), "no density warning at 5 nodes");

        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = diagram.layout(&ctx);

        // Count node rects by width/height — eyebrow tags also emit Rects
        // but at width ~31 lpx (tag_width(3) = 31) / 16 lpx tall, so a
        // dimension filter cleanly separates them.
        let node_rects = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Rect { w, h, .. } if (*w - NODE_WIDTH).abs() < 0.01
                        && (*h - NODE_HEIGHT).abs() < 0.01
                )
            })
            .count();
        assert_eq!(node_rects, 5, "expected 5 node rects, got {node_rects}");

        // 4 arrows, one per edge.
        let arrows = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Arrow { .. }))
            .count();
        assert_eq!(arrows, 4);

        // Layout is 2D: distinct y values across nodes (MDX Bundle and
        // Content CMS share a rank but split perpendicular to the flow).
        let ys: std::collections::BTreeSet<i32> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect { y, w, h, .. }
                    if (*w - NODE_WIDTH).abs() < 0.01 && (*h - NODE_HEIGHT).abs() < 0.01 =>
                {
                    Some(y.round() as i32)
                }
                _ => None,
            })
            .collect();
        assert!(
            ys.len() >= 2,
            "expected ≥ 2 distinct y-rows (2D layout), got {ys:?}"
        );
    }

    /// Scenario 2 in the task brief: node role → fill color matches the
    /// palette rules in style-guide.md's "Node type → treatment" table.
    #[test]
    fn architecture_role_fill_mapping() {
        let theme = Theme::light();

        // Pairwise: each role's (fill, stroke) must match the table.
        let cases = [
            (
                NodeRole::Focal,
                theme.palette.accent_tint,
                theme.palette.accent,
            ),
            (NodeRole::Backend, theme.palette.paper, theme.palette.ink),
            (
                NodeRole::Store,
                theme.palette.leaf_tint,
                theme.palette.muted,
            ),
            (
                NodeRole::External,
                theme.palette.leaf_tint,
                theme.palette.rule,
            ),
            (NodeRole::Input, theme.palette.leaf_tint, theme.palette.soft),
            (
                NodeRole::Optional,
                theme.palette.leaf_tint,
                theme.palette.rule,
            ),
            (
                NodeRole::Security,
                theme.palette.accent_tint,
                theme.palette.accent,
            ),
        ];
        for (role, want_fill, want_stroke) in cases {
            let (fill, stroke, _) = role_colors(role, &theme);
            assert_eq!(fill, want_fill, "role {role:?} fill");
            assert_eq!(stroke, want_stroke, "role {role:?} stroke");
        }

        // End-to-end: the README JSON must produce exactly one accent-stroke
        // rect (the focal "Astro Origin" node) — accent policy is one per
        // diagram (matching tree / flowchart behaviour).
        let (d, _) = parse(README_JSON).unwrap();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = d.layout(&ctx);
        let accent_rects = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Rect { stroke, w, h, .. }
                        if *stroke == theme.palette.accent
                            && (*w - NODE_WIDTH).abs() < 0.01
                            && (*h - NODE_HEIGHT).abs() < 0.01
                )
            })
            .count();
        assert_eq!(
            accent_rects, 1,
            "expected exactly one focal rect with accent stroke, got {accent_rects}"
        );
    }

    /// Scenario 3 in the task brief: primary edge uses accent, external
    /// uses link, default uses muted. Verified via the shared helper.
    #[test]
    fn architecture_edge_role_color() {
        let (d, _) = parse(README_JSON).unwrap();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = d.layout(&ctx);
        let pal = ctx.theme.palette;

        // Collect arrow colors in edge declaration order. The README
        // sequence is: external, primary, default, default.
        let arrow_colors: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Arrow { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(
            arrow_colors,
            vec![pal.link, pal.accent, pal.muted, pal.muted],
            "edge colors must follow role → palette mapping"
        );

        // Matching edge labels use the same color so the role reads
        // consistently — verify the "SSR" (primary) label is accent and
        // the "HTTPS" (external) label is link.
        let primary_label_ok = layout.primitives.iter().any(|p| {
            matches!(
                p,
                Primitive::Text { text, color, .. } if text == "SSR" && *color == pal.accent
            )
        });
        assert!(primary_label_ok, "primary label must use accent");
        let external_label_ok = layout.primitives.iter().any(|p| {
            matches!(
                p,
                Primitive::Text { text, color, .. } if text == "HTTPS" && *color == pal.link
            )
        });
        assert!(external_label_ok, "external label must use link");
    }

    #[test]
    fn rank_assignment_places_longest_path() {
        // reader → edge → orig → {bun, cms}
        // Expected ranks: reader=0, edge=1, orig=2, bun=3, cms=3
        let (d, _) = parse(README_JSON).unwrap();
        let spec = match d {
            Diagram::Architecture(s) => s,
            _ => unreachable!(),
        };
        let ranks = assign_ranks(&spec);
        let by_id: std::collections::HashMap<_, _> = spec
            .nodes
            .iter()
            .zip(ranks.iter())
            .map(|(n, r)| (n.id.clone(), *r))
            .collect();
        assert_eq!(by_id["reader"], 0);
        assert_eq!(by_id["edge"], 1);
        assert_eq!(by_id["orig"], 2);
        assert_eq!(by_id["bun"], 3);
        assert_eq!(by_id["cms"], 3);
    }

    #[test]
    fn orientation_tb_swaps_axes() {
        // Same topology, TB orientation — the arrow endpoints should run
        // along the y-axis rather than the x-axis, i.e. from.y < to.y for
        // the first edge.
        let body = r#"{"type":"architecture","orientation":"tb",
            "nodes":[
              {"id":"a","label":"A"},
              {"id":"b","label":"B"}
            ],
            "edges":[{"from":"a","to":"b"}]
        }"#;
        let (d, _) = parse(body).unwrap();
        let ctx = LayoutContext::new(600.0, 600.0);
        let layout = d.layout(&ctx);
        let arrow = layout
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Arrow { from, to, .. } => Some((*from, *to)),
                _ => None,
            })
            .expect("arrow present");
        assert!(
            arrow.0.y < arrow.1.y,
            "TB: expected from.y < to.y, got from={:?} to={:?}",
            arrow.0,
            arrow.1
        );
        // And x should be identical (same rank column).
        assert!(
            (arrow.0.x - arrow.1.x).abs() < 0.5,
            "TB: expected shared x, got from.x={} to.x={}",
            arrow.0.x,
            arrow.1.x
        );
    }

    #[test]
    fn density_warning_above_cap() {
        let mut nodes = String::new();
        for i in 0..15 {
            if i > 0 {
                nodes.push(',');
            }
            nodes.push_str(&format!(r#"{{"id":"n{i}","label":"N{i}"}}"#));
        }
        let body = format!(r#"{{"type":"architecture","nodes":[{nodes}]}}"#);
        let (_, warnings) = parse(&body).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::DensityHigh { .. }));
    }

    #[test]
    fn empty_nodes_produce_empty_layout() {
        let body = r#"{"type":"architecture","nodes":[]}"#;
        let (d, _) = parse(body).unwrap();
        let ctx = LayoutContext::new(400.0, 400.0);
        let layout = d.layout(&ctx);
        assert!(layout.primitives.is_empty());
    }
}
