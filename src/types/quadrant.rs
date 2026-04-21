//! Quadrant / 2×2 scatter diagram.
//!
//! See `robius/diagram-design/references/type-quadrant.md`.
//!
//! Mapping rule: the full canvas is the data space; axis labels ride the outer
//! edges of the canvas without shrinking the plot area. Y axis is inverted
//! (data-space max → screen-space 0).

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use serde::Deserialize;

/// Soft cap above which [`Warning::DensityHigh`] is emitted.
pub const SOFT_CAP: usize = 20;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "quadrant";

/// One axis range + display labels.
#[derive(Debug, Clone, Deserialize)]
pub struct Axis {
    pub min: f32,
    pub max: f32,
    /// Label for the LOW end (drawn near origin for x, near bottom for y).
    #[serde(default)]
    pub low_label: Option<String>,
    /// Label for the HIGH end.
    #[serde(default)]
    pub high_label: Option<String>,
}

/// Both axes.
#[derive(Debug, Clone, Deserialize)]
pub struct Axes {
    pub x: Axis,
    pub y: Axis,
}

/// One plotted point.
#[derive(Debug, Clone, Deserialize)]
pub struct QuadrantPoint {
    pub x: f32,
    pub y: f32,
    pub label: String,
}

/// JSON schema: `{"type":"quadrant","axes":{...},"points":[...], "accent_idx":optional}`.
#[derive(Debug, Clone, Deserialize)]
pub struct QuadrantSpec {
    pub axes: Axes,
    pub points: Vec<QuadrantPoint>,
    #[serde(default)]
    pub accent_idx: Option<usize>,
}

impl QuadrantSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        if let Some(idx) = self.accent_idx
            && idx >= self.points.len()
        {
            return Err(ParseError::AccentOutOfRange {
                element_count: self.points.len(),
                accent_idx: idx,
            });
        }
        Ok(())
    }
}

pub(crate) fn warnings(spec: &QuadrantSpec) -> Vec<Warning> {
    if spec.points.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.points.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

/// Map data-space `(x, y)` to screen coordinates. Y is inverted.
fn project(p: &QuadrantPoint, axes: &Axes, w: f32, h: f32) -> Point {
    let x_span = (axes.x.max - axes.x.min).max(f32::EPSILON);
    let y_span = (axes.y.max - axes.y.min).max(f32::EPSILON);
    let nx = (p.x - axes.x.min) / x_span;
    let ny = (p.y - axes.y.min) / y_span;
    Point::new(nx * w, (1.0 - ny) * h)
}

/// Lay out the quadrant. Canvas fully used as plot area; axis labels are placed
/// on the outer edges without inset.
#[must_use]
pub fn layout_quadrant(spec: &QuadrantSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    let theme = &ctx.theme;
    let w = ctx.canvas_width;
    let h = ctx.canvas_height;
    let cx = w / 2.0;
    let cy = h / 2.0;

    // Axis crosshair
    out.push(Primitive::Line {
        from: Point::new(0.0, cy),
        to: Point::new(w, cy),
        color: theme.palette.ink,
        stroke_width: theme.stroke_default,
        style: LineStyle::Solid,
    });
    out.push(Primitive::Line {
        from: Point::new(cx, 0.0),
        to: Point::new(cx, h),
        color: theme.palette.ink,
        stroke_width: theme.stroke_default,
        style: LineStyle::Solid,
    });

    // Axis labels — outer edges. These sit OUTSIDE the plot area so they don't
    // shift the data projection. Renderer decides clipping.
    let annotation = theme.typography.annotation_size;
    if let Some(lo) = &spec.axes.x.low_label {
        out.push(Primitive::Text {
            x: 8.0,
            y: cy - 8.0,
            text: lo.clone(),
            font_size: annotation,
            color: theme.palette.muted,
            align: TextAlign::Left,
            weight: TextWeight::Medium,
        });
    }
    if let Some(hi) = &spec.axes.x.high_label {
        out.push(Primitive::Text {
            x: w - 8.0,
            y: cy - 8.0,
            text: hi.clone(),
            font_size: annotation,
            color: theme.palette.muted,
            align: TextAlign::Right,
            weight: TextWeight::Medium,
        });
    }
    if let Some(lo) = &spec.axes.y.low_label {
        out.push(Primitive::Text {
            x: cx + 8.0,
            y: h - 8.0,
            text: lo.clone(),
            font_size: annotation,
            color: theme.palette.muted,
            align: TextAlign::Left,
            weight: TextWeight::Medium,
        });
    }
    if let Some(hi) = &spec.axes.y.high_label {
        out.push(Primitive::Text {
            x: cx + 8.0,
            y: 8.0 + annotation,
            text: hi.clone(),
            font_size: annotation,
            color: theme.palette.muted,
            align: TextAlign::Left,
            weight: TextWeight::Medium,
        });
    }

    // Plotted points
    let dot_radius = 4.0;
    for (i, p) in spec.points.iter().enumerate() {
        let pos = project(p, &spec.axes, w, h);
        let is_accent = spec.accent_idx == Some(i);
        let fill = if is_accent {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        // Represent the dot as a small filled square (Rect w/ corner_radius
        // equal to half its side — renderer treats large radii as circular).
        out.push(Primitive::Rect {
            x: pos.x - dot_radius,
            y: pos.y - dot_radius,
            w: dot_radius * 2.0,
            h: dot_radius * 2.0,
            fill,
            stroke: fill,
            stroke_width: 0.0,
            corner_radius: dot_radius,
        });

        // Label — offset 8 px from the dot so it doesn't overlap.
        out.push(Primitive::Text {
            x: pos.x + 8.0,
            y: pos.y - 2.0,
            text: p.label.clone(),
            font_size: theme.typography.sublabel_size,
            color: theme.palette.ink,
            align: TextAlign::Left,
            weight: TextWeight::Medium,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> QuadrantSpec {
        QuadrantSpec {
            axes: Axes {
                x: Axis {
                    min: 0.0,
                    max: 100.0,
                    low_label: None,
                    high_label: None,
                },
                y: Axis {
                    min: 0.0,
                    max: 10.0,
                    low_label: None,
                    high_label: None,
                },
            },
            points: vec![
                QuadrantPoint {
                    x: 50.0,
                    y: 5.0,
                    label: "mid".into(),
                },
                QuadrantPoint {
                    x: 100.0,
                    y: 10.0,
                    label: "tr".into(),
                },
                QuadrantPoint {
                    x: 0.0,
                    y: 0.0,
                    label: "bl".into(),
                },
            ],
            accent_idx: None,
        }
    }

    fn dot_center(r: &Primitive) -> Option<Point> {
        match r {
            Primitive::Rect { x, y, w, h, .. } => Some(Point::new(x + w / 2.0, y + h / 2.0)),
            _ => None,
        }
    }

    #[test]
    fn test_quadrant_axis_mapping() {
        let spec = sample_spec();
        let ctx = LayoutContext::new(400.0, 400.0);
        let layout = layout_quadrant(&spec, &ctx);

        // Gather dot centers in insertion order.
        let dots: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(dot_center)
            .collect();
        assert_eq!(dots.len(), 3);

        // "mid" at (200, 200) exactly.
        assert!((dots[0].x - 200.0).abs() < 1.0);
        assert!((dots[0].y - 200.0).abs() < 1.0);

        // "tr" — data (100, 10) → canvas (400, 0).
        assert!((dots[1].x - 400.0).abs() < 1.0);
        assert!((dots[1].y - 0.0).abs() < 1.0);

        // "bl" — data (0, 0) → canvas (0, 400).
        assert!((dots[2].x - 0.0).abs() < 1.0);
        assert!((dots[2].y - 400.0).abs() < 1.0);
    }

    #[test]
    fn accent_point_rendered_in_accent_color() {
        let mut spec = sample_spec();
        spec.accent_idx = Some(1);
        let ctx = LayoutContext::new(400.0, 400.0);
        let layout = layout_quadrant(&spec, &ctx);
        let accent = ctx.theme.palette.accent;

        let accented = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect { fill, .. } if *fill == accent))
            .count();
        assert_eq!(accented, 1);
    }

    #[test]
    fn density_warning_fires_above_cap() {
        let mut spec = sample_spec();
        while spec.points.len() <= SOFT_CAP {
            spec.points.push(QuadrantPoint {
                x: 50.0,
                y: 5.0,
                label: "x".into(),
            });
        }
        let w = warnings(&spec);
        assert_eq!(w.len(), 1);
    }
}
