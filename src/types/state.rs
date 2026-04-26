//! State-machine diagram: rounded states, start/end dots, and labeled
//! transitions.
//!
//! See `robius/diagram-design/references/type-state.md`.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use crate::theme::Theme;
use crate::types::flowchart::EdgeRole;
use serde::Deserialize;
use std::collections::HashMap;

pub const SOFT_CAP: usize = 12;
pub const TYPE_TAG: &str = "state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StateKind {
    #[default]
    State,
    #[serde(alias = "initial")]
    Start,
    #[serde(alias = "final")]
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StateRole {
    #[default]
    Default,
    Focal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Lr,
    Tb,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub kind: StateKind,
    #[serde(default)]
    pub role: StateRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub role: EdgeRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateSpec {
    pub states: Vec<StateNode>,
    #[serde(default)]
    pub transitions: Vec<StateTransition>,
    #[serde(default)]
    pub orientation: Orientation,
}

impl StateSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        Ok(())
    }
}

pub(crate) fn warnings(spec: &StateSpec) -> Vec<Warning> {
    if spec.states.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.states.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const NODE_WIDTH: f32 = 144.0;
const NODE_HEIGHT: f32 = 48.0;
const DOT_R: f32 = 7.0;
const END_OUTER_R: f32 = 9.0;
const END_INNER_R: f32 = 5.0;
const GAP: f32 = 84.0;
const MARGIN: f32 = 48.0;
const SELF_LOOP: f32 = 32.0;

fn node_radius(node: &StateNode) -> f32 {
    match node.kind {
        StateKind::State => NODE_HEIGHT / 2.0,
        StateKind::Start => DOT_R,
        StateKind::End => END_OUTER_R,
    }
}

fn transition_endpoint(center: Point, node: &StateNode, dir: Point) -> Point {
    match node.kind {
        StateKind::State => {
            if dir.x.abs() >= dir.y.abs() {
                Point::new(center.x + dir.x.signum() * NODE_WIDTH / 2.0, center.y)
            } else {
                Point::new(center.x, center.y + dir.y.signum() * NODE_HEIGHT / 2.0)
            }
        }
        StateKind::Start | StateKind::End => {
            let r = node_radius(node);
            if dir.x.abs() >= dir.y.abs() {
                Point::new(center.x + dir.x.signum() * r, center.y)
            } else {
                Point::new(center.x, center.y + dir.y.signum() * r)
            }
        }
    }
}

fn draw_state(out: &mut DiagramLayout, node: &StateNode, center: Point, theme: &Theme) {
    let focal = node.role == StateRole::Focal;
    let stroke = if focal {
        theme.palette.accent
    } else {
        theme.palette.ink
    };
    let fill = if focal {
        theme.palette.accent_tint
    } else {
        theme.palette.paper
    };

    match node.kind {
        StateKind::State => {
            out.push(Primitive::Rect {
                x: center.x - NODE_WIDTH / 2.0,
                y: center.y - NODE_HEIGHT / 2.0,
                w: NODE_WIDTH,
                h: NODE_HEIGHT,
                fill,
                stroke,
                stroke_width: theme.stroke_default,
                corner_radius: theme.corner_radius,
            });
            out.push(Primitive::Text {
                x: center.x,
                y: center.y,
                text: node.label.clone(),
                font_size: theme.typography.label_size,
                color: stroke,
                align: TextAlign::Center,
                weight: TextWeight::SemiBold,
            });
        }
        StateKind::Start => {
            out.push(Primitive::Circle {
                cx: center.x,
                cy: center.y,
                r: DOT_R,
                fill: stroke,
                stroke,
                stroke_width: 0.0,
            });
            out.push(Primitive::Text {
                x: center.x,
                y: center.y + 24.0,
                text: node.label.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.soft,
                align: TextAlign::Center,
                weight: TextWeight::Regular,
            });
        }
        StateKind::End => {
            out.push(Primitive::Circle {
                cx: center.x,
                cy: center.y,
                r: END_OUTER_R,
                fill: crate::theme::Color::rgba(0, 0, 0, 0),
                stroke,
                stroke_width: theme.stroke_default,
            });
            out.push(Primitive::Circle {
                cx: center.x,
                cy: center.y,
                r: END_INNER_R,
                fill: stroke,
                stroke: crate::theme::Color::rgba(0, 0, 0, 0),
                stroke_width: 0.0,
            });
            out.push(Primitive::Text {
                x: center.x,
                y: center.y + 28.0,
                text: node.label.clone(),
                font_size: theme.typography.sublabel_size,
                color: stroke,
                align: TextAlign::Center,
                weight: TextWeight::Medium,
            });
        }
    }
}

#[must_use]
pub fn layout_state(spec: &StateSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.states.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let n = spec.states.len();
    let mut centers = vec![Point::new(0.0, 0.0); n];

    match spec.orientation {
        Orientation::Lr => {
            let total_w = n as f32 * NODE_WIDTH + n.saturating_sub(1) as f32 * GAP;
            let left = ((ctx.canvas_width - total_w) / 2.0).max(MARGIN);
            let cy = ctx.canvas_height * 0.5;
            for (i, c) in centers.iter_mut().enumerate() {
                *c = Point::new(left + NODE_WIDTH / 2.0 + i as f32 * (NODE_WIDTH + GAP), cy);
            }
        }
        Orientation::Tb => {
            let total_h = n as f32 * NODE_HEIGHT + n.saturating_sub(1) as f32 * GAP;
            let top = ((ctx.canvas_height - total_h) / 2.0).max(MARGIN);
            let cx = ctx.canvas_width * 0.5;
            for (i, c) in centers.iter_mut().enumerate() {
                *c = Point::new(cx, top + NODE_HEIGHT / 2.0 + i as f32 * (NODE_HEIGHT + GAP));
            }
        }
    }

    let id_to_idx: HashMap<&str, usize> = spec
        .states
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    for transition in &spec.transitions {
        let (Some(&a), Some(&b)) = (
            id_to_idx.get(transition.from.as_str()),
            id_to_idx.get(transition.to.as_str()),
        ) else {
            continue;
        };
        let color = crate::types::shared::edge_color_for_role(transition.role, theme);
        if a == b {
            let c = centers[a];
            let top = Point::new(c.x, c.y - NODE_HEIGHT / 2.0);
            let p1 = Point::new(top.x, top.y);
            let p2 = Point::new(top.x + SELF_LOOP, top.y - SELF_LOOP);
            let p3 = Point::new(top.x - SELF_LOOP, top.y - SELF_LOOP);
            out.push(Primitive::Line {
                from: p1,
                to: p2,
                color,
                stroke_width: theme.stroke_default,
                style: LineStyle::Solid,
            });
            out.push(Primitive::Arrow {
                from: p2,
                to: p3,
                color,
                stroke_width: theme.stroke_default,
                style: LineStyle::Solid,
            });
            if let Some(label) = &transition.label {
                out.push(Primitive::Text {
                    x: c.x,
                    y: top.y - SELF_LOOP - 8.0,
                    text: label.clone(),
                    font_size: theme.typography.annotation_size,
                    color,
                    align: TextAlign::Center,
                    weight: TextWeight::Regular,
                });
            }
            continue;
        }

        let from_c = centers[a];
        let to_c = centers[b];
        let dir = Point::new(to_c.x - from_c.x, to_c.y - from_c.y);
        let from = transition_endpoint(from_c, &spec.states[a], dir);
        let to = transition_endpoint(to_c, &spec.states[b], Point::new(-dir.x, -dir.y));
        out.push(Primitive::Arrow {
            from,
            to,
            color,
            stroke_width: theme.stroke_default,
            style: LineStyle::Solid,
        });
        if let Some(label) = &transition.label {
            out.push(Primitive::Text {
                x: (from.x + to.x) * 0.5,
                y: (from.y + to.y) * 0.5 - 8.0,
                text: label.clone(),
                font_size: theme.typography.annotation_size,
                color,
                align: TextAlign::Center,
                weight: TextWeight::Regular,
            });
        }
    }

    for (node, center) in spec.states.iter().zip(centers) {
        draw_state(&mut out, node, center, theme);
    }

    out
}
