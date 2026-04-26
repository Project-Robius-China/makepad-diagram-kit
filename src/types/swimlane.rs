//! Swimlane diagram: horizontal actor/team lanes with process steps and
//! handoff arrows.
//!
//! See `robius/diagram-design/references/type-swimlane.md`.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use crate::theme::Theme;
use crate::types::flowchart::EdgeRole;
use serde::Deserialize;
use std::collections::HashMap;

pub const SOFT_CAP: usize = 14;
pub const TYPE_TAG: &str = "swimlane";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StepRole {
    #[default]
    Default,
    Focal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Lane {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    pub id: String,
    pub lane: String,
    pub label: String,
    #[serde(default)]
    pub sublabel: Option<String>,
    #[serde(default)]
    pub role: StepRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SwimlaneEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub role: EdgeRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SwimlaneSpec {
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub edges: Vec<SwimlaneEdge>,
}

impl SwimlaneSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        Ok(())
    }
}

pub(crate) fn warnings(spec: &SwimlaneSpec) -> Vec<Warning> {
    if spec.steps.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.steps.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const LANE_LABEL_W: f32 = 120.0;
const LANE_H: f32 = 88.0;
const STEP_W: f32 = 144.0;
const STEP_H: f32 = 46.0;
const STEP_GAP: f32 = 52.0;
const MARGIN: f32 = 32.0;

fn draw_step(out: &mut DiagramLayout, step: &Step, center: Point, theme: &Theme) {
    let focal = step.role == StepRole::Focal;
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
    let x = center.x - STEP_W / 2.0;
    let y = center.y - STEP_H / 2.0;
    out.push(Primitive::Rect {
        x,
        y,
        w: STEP_W,
        h: STEP_H,
        fill,
        stroke,
        stroke_width: theme.stroke_default,
        corner_radius: theme.corner_radius,
    });
    out.push(Primitive::Text {
        x: center.x,
        y: if step.sublabel.is_some() {
            center.y - 6.0
        } else {
            center.y
        },
        text: step.label.clone(),
        font_size: theme.typography.label_size,
        color: stroke,
        align: TextAlign::Center,
        weight: TextWeight::SemiBold,
    });
    if let Some(sub) = &step.sublabel {
        out.push(Primitive::Text {
            x: center.x,
            y: center.y + 12.0,
            text: sub.clone(),
            font_size: theme.typography.sublabel_size,
            color: theme.palette.soft,
            align: TextAlign::Center,
            weight: TextWeight::Regular,
        });
    }
}

#[must_use]
pub fn layout_swimlane(spec: &SwimlaneSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.lanes.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let lane_top = MARGIN;
    let lane_left = MARGIN;
    let lane_right = ctx.canvas_width.max(700.0) - MARGIN;
    let lane_count = spec.lanes.len();

    let lane_id_to_idx: HashMap<&str, usize> = spec
        .lanes
        .iter()
        .enumerate()
        .map(|(i, lane)| (lane.id.as_str(), i))
        .collect();

    for (i, lane) in spec.lanes.iter().enumerate() {
        let y = lane_top + i as f32 * LANE_H;
        out.push(Primitive::Rect {
            x: lane_left,
            y,
            w: lane_right - lane_left,
            h: LANE_H,
            fill: if i % 2 == 0 {
                crate::theme::Color::rgba(0, 0, 0, 0)
            } else {
                theme.palette.leaf_tint
            },
            stroke: theme.palette.rule,
            stroke_width: theme.stroke_default,
            corner_radius: 0.0,
        });
        out.push(Primitive::Text {
            x: lane_left + 16.0,
            y: y + 24.0,
            text: lane.label.clone(),
            font_size: theme.typography.sublabel_size,
            color: theme.palette.muted,
            align: TextAlign::Left,
            weight: TextWeight::Medium,
        });
    }
    out.push(Primitive::Line {
        from: Point::new(lane_left + LANE_LABEL_W, lane_top),
        to: Point::new(
            lane_left + LANE_LABEL_W,
            lane_top + lane_count as f32 * LANE_H,
        ),
        color: theme.palette.rule,
        stroke_width: theme.stroke_default,
        style: LineStyle::Solid,
    });

    let content_left = lane_left + LANE_LABEL_W + STEP_W / 2.0 + 24.0;
    let mut centers = vec![Point::new(0.0, 0.0); spec.steps.len()];
    for (i, step) in spec.steps.iter().enumerate() {
        let lane_idx = lane_id_to_idx.get(step.lane.as_str()).copied().unwrap_or(0);
        let y = lane_top + lane_idx as f32 * LANE_H + LANE_H / 2.0;
        let x = content_left + i as f32 * (STEP_W + STEP_GAP);
        centers[i] = Point::new(x, y);
    }

    let step_id_to_idx: HashMap<&str, usize> = spec
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| (step.id.as_str(), i))
        .collect();
    for edge in &spec.edges {
        let (Some(&a), Some(&b)) = (
            step_id_to_idx.get(edge.from.as_str()),
            step_id_to_idx.get(edge.to.as_str()),
        ) else {
            continue;
        };
        let from_c = centers[a];
        let to_c = centers[b];
        let from = if to_c.x >= from_c.x {
            Point::new(from_c.x + STEP_W / 2.0, from_c.y)
        } else {
            Point::new(from_c.x - STEP_W / 2.0, from_c.y)
        };
        let to = if to_c.x >= from_c.x {
            Point::new(to_c.x - STEP_W / 2.0, to_c.y)
        } else {
            Point::new(to_c.x + STEP_W / 2.0, to_c.y)
        };
        let color = crate::types::shared::edge_color_for_role(edge.role, theme);
        out.push(Primitive::Arrow {
            from,
            to,
            color,
            stroke_width: theme.stroke_default,
            style: LineStyle::Solid,
        });
        if let Some(label) = &edge.label {
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

    for (step, center) in spec.steps.iter().zip(centers) {
        draw_step(&mut out, step, center, theme);
    }

    out
}
