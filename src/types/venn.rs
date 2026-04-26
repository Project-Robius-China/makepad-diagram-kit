//! Venn / set-overlap diagram.
//!
//! See `robius/diagram-design/references/type-venn.md`.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use crate::theme::{Color, Theme};
use serde::Deserialize;
use std::collections::HashMap;

pub const SOFT_CAP: usize = 3;
pub const TYPE_TAG: &str = "venn";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IntersectionRole {
    #[default]
    Default,
    Focal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VennSet {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub sublabel: Option<String>,
    #[serde(default)]
    pub radius: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VennIntersection {
    pub sets: Vec<String>,
    pub label: String,
    #[serde(default)]
    pub role: IntersectionRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VennSpec {
    pub sets: Vec<VennSet>,
    #[serde(default)]
    pub intersections: Vec<VennIntersection>,
}

impl VennSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        Ok(())
    }
}

pub(crate) fn warnings(spec: &VennSpec) -> Vec<Warning> {
    if spec.sets.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.sets.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const DEFAULT_R: f32 = 136.0;

fn set_colors(idx: usize, theme: &Theme) -> (Color, Color) {
    match idx % 3 {
        0 => (
            theme.palette.ink.with_alpha(10),
            theme.palette.ink.with_alpha(150),
        ),
        1 => (theme.palette.muted.with_alpha(12), theme.palette.muted),
        _ => (theme.palette.soft.with_alpha(12), theme.palette.soft),
    }
}

fn centers(n: usize, ctx: &LayoutContext) -> Vec<Point> {
    let cx = ctx.canvas_width * 0.5;
    let cy = ctx.canvas_height * 0.5;
    match n {
        0 => Vec::new(),
        1 => vec![Point::new(cx, cy)],
        2 => vec![Point::new(cx - 84.0, cy), Point::new(cx + 84.0, cy)],
        3 => vec![
            Point::new(cx - 84.0, cy - 36.0),
            Point::new(cx + 84.0, cy - 36.0),
            Point::new(cx, cy + 88.0),
        ],
        _ => {
            let radius = 96.0;
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    Point::new(cx + a.cos() * radius, cy + a.sin() * radius)
                })
                .collect()
        }
    }
}

fn set_label_pos(idx: usize, center: Point, r: f32, n: usize) -> (Point, TextAlign) {
    match (n, idx) {
        (1, _) => (Point::new(center.x, center.y - r - 22.0), TextAlign::Center),
        (2, 0) | (3, 0) => (
            Point::new(center.x - r * 0.58, center.y - r - 18.0),
            TextAlign::Center,
        ),
        (2, 1) | (3, 1) => (
            Point::new(center.x + r * 0.58, center.y - r - 18.0),
            TextAlign::Center,
        ),
        (3, 2) => (Point::new(center.x, center.y + r + 26.0), TextAlign::Center),
        _ => (Point::new(center.x, center.y - r - 18.0), TextAlign::Center),
    }
}

#[must_use]
pub fn layout_venn(spec: &VennSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.sets.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let centers = centers(spec.sets.len(), ctx);
    let id_to_idx: HashMap<&str, usize> = spec
        .sets
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    for (i, (set, center)) in spec.sets.iter().zip(centers.iter().copied()).enumerate() {
        let r = set.radius.unwrap_or(DEFAULT_R).clamp(48.0, 180.0);
        let (fill, stroke) = set_colors(i, theme);
        out.push(Primitive::Circle {
            cx: center.x,
            cy: center.y,
            r,
            fill,
            stroke,
            stroke_width: theme.stroke_default,
        });
    }

    for (i, (set, center)) in spec.sets.iter().zip(centers.iter().copied()).enumerate() {
        let r = set.radius.unwrap_or(DEFAULT_R).clamp(48.0, 180.0);
        let (_, stroke) = set_colors(i, theme);
        let (pos, align) = set_label_pos(i, center, r, spec.sets.len());
        out.push(Primitive::Text {
            x: pos.x,
            y: pos.y,
            text: set.label.clone(),
            font_size: theme.typography.label_size,
            color: stroke,
            align,
            weight: TextWeight::SemiBold,
        });
        if let Some(sub) = &set.sublabel {
            out.push(Primitive::Text {
                x: pos.x,
                y: pos.y + 14.0,
                text: sub.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.soft,
                align,
                weight: TextWeight::Regular,
            });
        }
    }

    for intersection in &spec.intersections {
        let pts: Vec<Point> = intersection
            .sets
            .iter()
            .filter_map(|id| {
                id_to_idx
                    .get(id.as_str())
                    .and_then(|&idx| centers.get(idx).copied())
            })
            .collect();
        if pts.is_empty() {
            continue;
        }
        let pos = Point::new(
            pts.iter().map(|p| p.x).sum::<f32>() / pts.len() as f32,
            pts.iter().map(|p| p.y).sum::<f32>() / pts.len() as f32,
        );
        let focal = intersection.role == IntersectionRole::Focal;
        out.push(Primitive::Text {
            x: pos.x,
            y: pos.y,
            text: intersection.label.clone(),
            font_size: theme.typography.label_size,
            color: if focal {
                theme.palette.accent
            } else {
                theme.palette.ink
            },
            align: TextAlign::Center,
            weight: TextWeight::SemiBold,
        });
        if focal {
            out.push(Primitive::Line {
                from: Point::new(pos.x - 30.0, pos.y + 14.0),
                to: Point::new(pos.x + 30.0, pos.y + 14.0),
                color: theme.palette.accent,
                stroke_width: theme.stroke_default,
                style: LineStyle::Solid,
            });
        }
    }

    out
}
