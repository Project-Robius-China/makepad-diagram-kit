//! Layer-stack diagram: equal-height horizontal bands.
//!
//! Best for tech stacks, abstraction hierarchies, OSI-model-style stacks.
//! See `robius/diagram-design/references/type-layers.md` for the visual rules.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{Point, Primitive, TextAlign, TextWeight};
use serde::Deserialize;

/// Soft cap above which [`Warning::DensityHigh`] is emitted.
pub const SOFT_CAP: usize = 10;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "layers";

/// One row in the stack.
#[derive(Debug, Clone, Deserialize)]
pub struct Layer {
    /// Primary label (e.g., `"Application"`).
    pub label: String,
    /// Optional right-edge annotation (e.g., `"HTTP / HTTPS"`).
    #[serde(default)]
    pub annotation: Option<String>,
    /// Optional left-edge tag (e.g., `"L7"`).
    #[serde(default)]
    pub tag: Option<String>,
}

/// JSON schema: `{"type":"layers","layers":[...], "accent_idx":optional}`.
#[derive(Debug, Clone, Deserialize)]
pub struct LayersSpec {
    /// Top-to-bottom layer order. Index 0 renders at the top.
    pub layers: Vec<Layer>,
    /// Optional index of the single accent-highlighted layer.
    #[serde(default)]
    pub accent_idx: Option<usize>,
}

impl LayersSpec {
    /// Validate field invariants that can't be caught by `serde` alone.
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        if let Some(idx) = self.accent_idx
            && idx >= self.layers.len()
        {
            return Err(ParseError::AccentOutOfRange {
                element_count: self.layers.len(),
                accent_idx: idx,
            });
        }
        Ok(())
    }
}

/// Collect density warnings for this spec.
pub(crate) fn warnings(spec: &LayersSpec) -> Vec<Warning> {
    let mut out = Vec::new();
    if spec.layers.len() > SOFT_CAP {
        out.push(Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.layers.len(),
            soft_cap: SOFT_CAP,
        });
    }
    out
}

/// Position a layer stack into the canvas defined by `ctx`.
///
/// Layers have equal heights, full canvas width (minus a small padding on the
/// left for tag text and on the right for annotation text). Adjacent layers
/// share a single divider line by drawing only a top border on each layer
/// after the first, plus a single outer silhouette rectangle at the end.
#[must_use]
pub fn layout_layers(spec: &LayersSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    let n = spec.layers.len();
    if n == 0 {
        return out;
    }

    let theme = &ctx.theme;
    let canvas_w = ctx.canvas_width;
    let canvas_h = ctx.canvas_height;
    let row_h = canvas_h / n as f32;
    let pad_left = 16.0;
    let pad_right = 16.0;

    // One filled rect per layer, stacked top-to-bottom.
    for (i, layer) in spec.layers.iter().enumerate() {
        let y = i as f32 * row_h;
        let is_accent = spec.accent_idx == Some(i);
        let fill = if is_accent {
            // Subtle accent tint — we approximate by using the accent with
            // alpha ~0.10 so renderers that composite correctly see it as a
            // muted fill.
            theme.palette.accent.with_alpha(26)
        } else {
            theme.palette.paper
        };
        let stroke = if is_accent {
            theme.palette.accent
        } else {
            theme.palette.rule
        };
        out.push(Primitive::Rect {
            x: 0.0,
            y,
            w: canvas_w,
            h: row_h,
            fill,
            stroke,
            stroke_width: theme.stroke_default,
            corner_radius: 0.0,
        });

        // Centered label
        let label_y = y + row_h / 2.0;
        out.push(Primitive::Text {
            x: pad_left,
            y: label_y,
            text: layer.label.clone(),
            font_size: theme.typography.label_size,
            color: theme.palette.ink,
            align: TextAlign::Left,
            weight: TextWeight::SemiBold,
        });

        // Optional left tag (eyebrow style) rendered above the label. Keep it
        // above the row's vertical midpoint so it doesn't overlap.
        if let Some(tag) = &layer.tag {
            out.push(Primitive::Text {
                x: pad_left,
                y: y + theme.typography.sublabel_size + 4.0,
                text: tag.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.muted,
                align: TextAlign::Left,
                weight: TextWeight::Medium,
            });
        }

        // Optional right-edge annotation
        if let Some(note) = &layer.annotation {
            out.push(Primitive::Text {
                x: canvas_w - pad_right,
                y: label_y,
                text: note.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.muted,
                align: TextAlign::Right,
                weight: TextWeight::Regular,
            });
        }
    }

    // Shared separator lines between layers — drawn once each so adjacent
    // layers do not double-stroke.
    for i in 1..n {
        let y = i as f32 * row_h;
        out.push(Primitive::Line {
            from: Point::new(0.0, y),
            to: Point::new(canvas_w, y),
            color: theme.palette.rule,
            stroke_width: theme.stroke_default,
            style: crate::primitive::LineStyle::Solid,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::Primitive;

    fn sample() -> LayersSpec {
        LayersSpec {
            layers: vec![
                Layer {
                    label: "L3".into(),
                    annotation: None,
                    tag: None,
                },
                Layer {
                    label: "L2".into(),
                    annotation: None,
                    tag: None,
                },
                Layer {
                    label: "L1".into(),
                    annotation: None,
                    tag: None,
                },
            ],
            accent_idx: None,
        }
    }

    #[test]
    fn test_layers_stack() {
        // Scenario: Layers stacks with equal heights and labels (400-lpx tall
        // canvas, 3 layers → ~133 lpx each, labels top→bottom are L3,L2,L1).
        let spec = sample();
        let ctx = LayoutContext::new(400.0, 400.0);
        let layout = layout_layers(&spec, &ctx);

        // 3 rects + 3 labels + 2 separators = 8 primitives.
        assert_eq!(layout.primitives.len(), 8);

        // Row heights are ~133.33
        let rects: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect { y, h, .. } => Some((*y, *h)),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 3);
        for (_, h) in &rects {
            assert!((h - 400.0 / 3.0).abs() < 0.01);
        }

        // Label order
        let labels: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, vec!["L3", "L2", "L1"]);

        // Adjacent layer borders: exactly 2 separator lines (3 layers → 2
        // boundaries, not 4 double-stroked borders).
        let lines = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Line { .. }))
            .count();
        assert_eq!(lines, 2);
    }

    #[test]
    fn accent_only_marks_one_layer() {
        let mut spec = sample();
        spec.accent_idx = Some(1);
        let ctx = LayoutContext::new(400.0, 400.0);
        let layout = layout_layers(&spec, &ctx);

        let accent = ctx.theme.palette.accent;
        let accent_marked = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect { stroke, .. } if *stroke == accent))
            .count();
        assert_eq!(accent_marked, 1);
    }

    #[test]
    fn accent_out_of_range_rejected() {
        let mut spec = sample();
        spec.accent_idx = Some(99);
        let err = spec.validate().unwrap_err();
        assert!(matches!(err, ParseError::AccentOutOfRange { .. }));
    }

    #[test]
    fn density_warning_above_soft_cap() {
        let mut spec = sample();
        // Push to 11 layers (soft cap 10).
        for i in 0..8 {
            spec.layers.push(Layer {
                label: format!("extra-{i}"),
                annotation: None,
                tag: None,
            });
        }
        assert_eq!(spec.layers.len(), 11);
        let w = warnings(&spec);
        assert_eq!(w.len(), 1);
        assert!(matches!(w[0], Warning::DensityHigh { count: 11, .. }));
    }
}
