//! Timeline diagram: honest horizontal axis with event dots and alternating
//! labels.
//!
//! See `robius/diagram-design/references/type-timeline.md`.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use serde::Deserialize;

pub const SOFT_CAP: usize = 14;
pub const TYPE_TAG: &str = "timeline";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EventRole {
    #[default]
    Default,
    #[serde(alias = "focal")]
    Major,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineEvent {
    pub time: String,
    pub label: String,
    #[serde(default)]
    pub sublabel: Option<String>,
    #[serde(default)]
    pub role: EventRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineSpec {
    pub events: Vec<TimelineEvent>,
    #[serde(default)]
    pub axis_label: Option<String>,
}

impl TimelineSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        Ok(())
    }
}

pub(crate) fn warnings(spec: &TimelineSpec) -> Vec<Warning> {
    if spec.events.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.events.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const MARGIN_X: f32 = 72.0;
const LABEL_DROP: f32 = 76.0;
const AXIS_LABEL_DROP: f32 = 40.0;

fn parse_iso_day(s: &str) -> Option<i32> {
    let mut parts = s.split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: i32 = parts.next()?.parse().ok()?;
    let d: i32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Civil-from-days inverse is overkill here; for layout spacing a stable
    // Gregorian ordinal is enough.
    let leap = |year: i32| -> bool { (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 };
    let month_days = [
        31,
        28 + i32::from(leap(y)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if d > month_days[(m - 1) as usize] {
        return None;
    }
    let years = y - 1970;
    let leap_count = |year: i32| -> i32 { year / 4 - year / 100 + year / 400 };
    let days_before_year = years * 365 + (leap_count(y - 1) - leap_count(1969));
    let days_before_month: i32 = month_days[..(m - 1) as usize].iter().sum();
    Some(days_before_year + days_before_month + d - 1)
}

fn positions(spec: &TimelineSpec, axis_left: f32, axis_right: f32) -> Vec<f32> {
    let n = spec.events.len();
    if n == 1 {
        return vec![(axis_left + axis_right) * 0.5];
    }
    let parsed: Option<Vec<i32>> = spec.events.iter().map(|e| parse_iso_day(&e.time)).collect();
    if let Some(days) = parsed {
        let min = *days.iter().min().unwrap();
        let max = *days.iter().max().unwrap();
        if max > min {
            let span = (max - min) as f32;
            return days
                .iter()
                .map(|d| axis_left + (*d - min) as f32 / span * (axis_right - axis_left))
                .collect();
        }
    }
    (0..n)
        .map(|i| axis_left + i as f32 / (n - 1) as f32 * (axis_right - axis_left))
        .collect()
}

#[must_use]
pub fn layout_timeline(spec: &TimelineSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.events.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let axis_left = MARGIN_X;
    let axis_right = (ctx.canvas_width - MARGIN_X).max(axis_left + 120.0);
    let axis_y = ctx.canvas_height * 0.5;
    out.push(Primitive::Line {
        from: Point::new(axis_left, axis_y),
        to: Point::new(axis_right, axis_y),
        color: theme.palette.rule,
        stroke_width: theme.stroke_default,
        style: LineStyle::Solid,
    });

    let xs = positions(spec, axis_left, axis_right);
    for (i, (event, x)) in spec.events.iter().zip(xs).enumerate() {
        let major = event.role == EventRole::Major;
        let color = if major {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        let r = if major { 6.0 } else { 4.0 };
        let label_above = i % 2 == 0;
        let label_y = if label_above {
            axis_y - LABEL_DROP
        } else {
            axis_y + LABEL_DROP
        };
        out.push(Primitive::Line {
            from: Point::new(x, axis_y),
            to: Point::new(x, label_y + if label_above { 14.0 } else { -14.0 }),
            color: theme.palette.rule,
            stroke_width: theme.stroke_default,
            style: LineStyle::Solid,
        });
        out.push(Primitive::Circle {
            cx: x,
            cy: axis_y,
            r,
            fill: color,
            stroke: color,
            stroke_width: 0.0,
        });
        out.push(Primitive::Text {
            x,
            y: label_y,
            text: event.label.clone(),
            font_size: theme.typography.label_size,
            color,
            align: TextAlign::Center,
            weight: if major {
                TextWeight::SemiBold
            } else {
                TextWeight::Medium
            },
        });
        if let Some(sub) = &event.sublabel {
            out.push(Primitive::Text {
                x,
                y: label_y + 14.0,
                text: sub.clone(),
                font_size: theme.typography.sublabel_size,
                color: theme.palette.soft,
                align: TextAlign::Center,
                weight: TextWeight::Regular,
            });
        }
        out.push(Primitive::Text {
            x,
            y: axis_y + AXIS_LABEL_DROP,
            text: event.time.clone(),
            font_size: theme.typography.annotation_size,
            color: theme.palette.soft,
            align: TextAlign::Center,
            weight: TextWeight::Regular,
        });
    }

    if let Some(axis_label) = &spec.axis_label {
        out.push(Primitive::Text {
            x: axis_left,
            y: axis_y + 24.0,
            text: axis_label.clone(),
            font_size: theme.typography.annotation_size,
            color: theme.palette.muted,
            align: TextAlign::Left,
            weight: TextWeight::Regular,
        });
    }

    out
}
