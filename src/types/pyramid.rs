//! Pyramid / funnel diagram: trapezoid layers from narrow apex to wide base.
//!
//! See `robius/diagram-design/references/type-pyramid.md`. v1 only ships the
//! point-up (pyramid) orientation; funnel (point-down) is v1.1.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{Point, Primitive, TextAlign, TextWeight};
use crate::theme::Color;
use serde::Deserialize;

/// Soft cap above which [`Warning::DensityHigh`] is emitted.
pub const SOFT_CAP: usize = 7;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "pyramid";

/// One layer in the pyramid.
#[derive(Debug, Clone, Deserialize)]
pub struct PyramidLevel {
    /// Primary label drawn inside the layer.
    pub label: String,
    /// Optional mono-font sublabel (e.g., frequency or size hint).
    #[serde(default)]
    pub sublabel: Option<String>,
}

/// JSON schema: `{"type":"pyramid","levels":[...], "accent_idx":optional}`.
///
/// `levels[0]` is the **apex** (top, narrowest); the last is the base (widest).
#[derive(Debug, Clone, Deserialize)]
pub struct PyramidSpec {
    pub levels: Vec<PyramidLevel>,
    #[serde(default)]
    pub accent_idx: Option<usize>,
}

impl PyramidSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        if let Some(idx) = self.accent_idx
            && idx >= self.levels.len()
        {
            return Err(ParseError::AccentOutOfRange {
                element_count: self.levels.len(),
                accent_idx: idx,
            });
        }
        Ok(())
    }
}

pub(crate) fn warnings(spec: &PyramidSpec) -> Vec<Warning> {
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

/// Layout the pyramid. Top layer width is ~20% of canvas, bottom ~90%; heights
/// are equal. Each layer is a trapezoid with 4 vertices.
#[must_use]
pub fn layout_pyramid(spec: &PyramidSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    let n = spec.levels.len();
    if n == 0 {
        return out;
    }

    let theme = &ctx.theme;
    let canvas_w = ctx.canvas_width;
    let canvas_h = ctx.canvas_height;

    // Leave modest top/bottom breathing room. Layers share the remaining
    // vertical band and each is `layer_h` tall.
    let pad_top = canvas_h * 0.10;
    let pad_bot = canvas_h * 0.10;
    let stack_h = canvas_h - pad_top - pad_bot;
    let layer_h = stack_h / n as f32;

    let top_w = canvas_w * 0.20;
    let bot_w = canvas_w * 0.90;
    let cx = canvas_w / 2.0;

    // Width for the shared boundary between level `i-1` and `i` — interpolate
    // between top_w (at the apex) and bot_w (past the final bottom boundary).
    let width_at = |i: usize| -> f32 {
        if n == 1 {
            // Single-level pyramid collapses to a single trapezoid with
            // equidistant top/bottom widths — use the midpoint to match the
            // average pyramid silhouette.
            return (top_w + bot_w) / 2.0;
        }
        let t = i as f32 / n as f32;
        top_w + (bot_w - top_w) * t
    };

    for (i, level) in spec.levels.iter().enumerate() {
        let y_top = pad_top + i as f32 * layer_h;
        let y_bot = y_top + layer_h;
        let w_top = width_at(i);
        let w_bot = width_at(i + 1);

        let top_l = cx - w_top / 2.0;
        let top_r = cx + w_top / 2.0;
        let bot_l = cx - w_bot / 2.0;
        let bot_r = cx + w_bot / 2.0;

        let is_accent = spec.accent_idx == Some(i);
        let (fill, stroke): (Color, Color) = if is_accent {
            (theme.palette.accent.with_alpha(32), theme.palette.accent)
        } else {
            (theme.palette.paper, theme.palette.rule)
        };

        out.push(Primitive::Polygon {
            points: vec![
                Point::new(top_l, y_top),
                Point::new(top_r, y_top),
                Point::new(bot_r, y_bot),
                Point::new(bot_l, y_bot),
            ],
            fill,
            stroke,
            stroke_width: theme.stroke_default,
        });

        // Centered label — pyramid convention draws it slightly above the
        // vertical midline so the sublabel sits below it cleanly.
        let mid_y = (y_top + y_bot) / 2.0;
        out.push(Primitive::Text {
            x: cx,
            y: mid_y + theme.typography.label_size * 0.1,
            text: level.label.clone(),
            font_size: theme.typography.label_size,
            color: theme.palette.ink,
            align: TextAlign::Center,
            weight: TextWeight::SemiBold,
        });

        if let Some(sub) = &level.sublabel {
            out.push(Primitive::Text {
                x: cx,
                y: mid_y + theme.typography.label_size + 4.0,
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
    use pretty_assertions::assert_eq;

    fn three_level() -> PyramidSpec {
        PyramidSpec {
            levels: vec![
                PyramidLevel {
                    label: "Mission".into(),
                    sublabel: None,
                },
                PyramidLevel {
                    label: "Strategy".into(),
                    sublabel: None,
                },
                PyramidLevel {
                    label: "Tactics".into(),
                    sublabel: None,
                },
            ],
            accent_idx: None,
        }
    }

    #[test]
    fn test_pyramid_basic_parse() {
        let spec = three_level();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = layout_pyramid(&spec, &ctx);

        // 3 trapezoids + 3 labels = 6 primitives.
        let polys: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Polygon { points, .. } => Some(points.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(polys.len(), 3);

        // Top trapezoid narrower than bottom.
        let top_width = |pts: &[Point]| (pts[1].x - pts[0].x).abs();
        assert!(top_width(&polys[0]) < top_width(&polys[2]));

        // Labels centered — all Text primitives must use Center alignment.
        for p in &layout.primitives {
            if let Primitive::Text { align, x, .. } = p {
                assert!(matches!(align, TextAlign::Center));
                assert!((x - 500.0).abs() < 0.001);
            }
        }
    }

    #[test]
    fn test_accent_single_element() {
        // Scenario: pyramid with accent_idx 1 → exactly one layer accented.
        let mut spec = three_level();
        spec.accent_idx = Some(1);
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = layout_pyramid(&spec, &ctx);

        let accent = ctx.theme.palette.accent;
        let paper = ctx.theme.palette.paper;

        let accented: Vec<_> = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Polygon { stroke, .. } if *stroke == accent))
            .collect();
        assert_eq!(accented.len(), 1);

        let paper_filled: Vec<_> = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Polygon { fill, .. } if *fill == paper))
            .collect();
        assert_eq!(paper_filled.len(), 2);
    }

    #[test]
    fn test_density_warning() {
        // Scenario: 8 levels (soft cap is 7) → warning returned.
        let levels = (0..8)
            .map(|i| PyramidLevel {
                label: format!("L{i}"),
                sublabel: None,
            })
            .collect();
        let spec = PyramidSpec {
            levels,
            accent_idx: None,
        };
        let w = warnings(&spec);
        assert_eq!(w.len(), 1);
        assert!(matches!(
            w[0],
            Warning::DensityHigh {
                diagram_type: "pyramid",
                count: 8,
                soft_cap: 7
            }
        ));
    }

    #[test]
    fn accent_out_of_range_rejected() {
        let mut spec = three_level();
        spec.accent_idx = Some(5);
        assert!(matches!(
            spec.validate(),
            Err(ParseError::AccentOutOfRange {
                element_count: 3,
                accent_idx: 5
            })
        ));
    }
}
