//! Nested containment diagram: concentric rounded rectangles representing
//! scope levels.
//!
//! See `robius/diagram-design/references/type-nested.md`.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{Primitive, TextAlign, TextWeight};
use crate::theme::Color;
use serde::Deserialize;

pub const SOFT_CAP: usize = 5;
pub const TYPE_TAG: &str = "nested";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NestedRole {
    #[default]
    Default,
    Focal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NestedLevel {
    pub label: String,
    #[serde(default)]
    pub sublabel: Option<String>,
    #[serde(default)]
    pub role: NestedRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NestedSpec {
    pub levels: Vec<NestedLevel>,
}

impl NestedSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        Ok(())
    }
}

pub(crate) fn warnings(spec: &NestedSpec) -> Vec<Warning> {
    if spec.levels.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.levels.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const OUTER_W: f32 = 760.0;
const OUTER_H: f32 = 360.0;
const INSET_X: f32 = 52.0;
const INSET_Y: f32 = 48.0;
const LABEL_PAD_X: f32 = 10.0;
const LABEL_H: f32 = 18.0;

fn ring_stroke(idx: usize, last_idx: usize, focal: bool, theme: &crate::Theme) -> Color {
    if focal {
        return theme.palette.accent;
    }
    if idx == last_idx {
        theme.palette.ink
    } else if idx + 1 == last_idx {
        theme.palette.muted
    } else {
        theme.palette.rule
    }
}

#[must_use]
pub fn layout_nested(spec: &NestedSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.levels.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let n = spec.levels.len();
    let outer_w = OUTER_W.min(ctx.canvas_width - 64.0).max(240.0);
    let outer_h = OUTER_H.min(ctx.canvas_height - 64.0).max(180.0);
    let left = ((ctx.canvas_width - outer_w) / 2.0).max(32.0);
    let top = ((ctx.canvas_height - outer_h) / 2.0).max(32.0);
    let last_idx = n - 1;
    let has_explicit_focal = spec
        .levels
        .iter()
        .any(|level| level.role == NestedRole::Focal);

    for (i, level) in spec.levels.iter().enumerate() {
        let x = left + i as f32 * INSET_X;
        let y = top + i as f32 * INSET_Y;
        let w = (outer_w - i as f32 * INSET_X * 2.0).max(80.0);
        let h = (outer_h - i as f32 * INSET_Y * 2.0).max(56.0);
        let focal =
            level.role == NestedRole::Focal || (!has_explicit_focal && i == last_idx && n > 1);
        let stroke = ring_stroke(i, last_idx, focal, theme);
        let fill = if focal {
            theme.palette.accent_tint
        } else {
            theme
                .palette
                .leaf_tint
                .with_alpha(4 + (i as u8).saturating_mul(4))
        };
        out.push(Primitive::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
            stroke_width: theme.stroke_default,
            corner_radius: theme.corner_radius,
        });

        let upper = level.label.to_uppercase();
        let label_w = upper.chars().count() as f32 * 7.0 + LABEL_PAD_X * 2.0;
        out.push(Primitive::Rect {
            x: x + 16.0,
            y: y - LABEL_H / 2.0,
            w: label_w,
            h: LABEL_H,
            fill: theme.palette.paper,
            stroke: Color::rgba(0, 0, 0, 0),
            stroke_width: 0.0,
            corner_radius: 2.0,
        });
        out.push(Primitive::Text {
            x: x + 16.0 + label_w / 2.0,
            y,
            text: upper,
            font_size: theme.typography.sublabel_size,
            color: stroke,
            align: TextAlign::Center,
            weight: TextWeight::Medium,
        });
        if let Some(sub) = &level.sublabel {
            out.push(Primitive::Text {
                x: x + 24.0,
                y: y + 24.0,
                text: sub.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.soft,
                align: TextAlign::Left,
                weight: TextWeight::Regular,
            });
        }
    }

    out
}
