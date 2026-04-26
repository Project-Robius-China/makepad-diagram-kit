//! ER / data-model diagram: entity boxes with field rows and relationship
//! cardinalities.
//!
//! See `robius/diagram-design/references/type-er.md`.

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use crate::theme::Theme;
use crate::types::flowchart::EdgeRole;
use serde::Deserialize;
use std::collections::HashMap;

pub const SOFT_CAP: usize = 10;
pub const TYPE_TAG: &str = "er";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EntityRole {
    #[default]
    Default,
    Focal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FieldKey {
    #[default]
    None,
    #[serde(alias = "primary")]
    Pk,
    #[serde(alias = "foreign")]
    Fk,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityField {
    pub name: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub key: FieldKey,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<EntityField>,
    #[serde(default)]
    pub role: EntityRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub from_cardinality: Option<String>,
    #[serde(default)]
    pub to_cardinality: Option<String>,
    #[serde(default)]
    pub role: EdgeRole,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErSpec {
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
}

impl ErSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        Ok(())
    }
}

pub(crate) fn warnings(spec: &ErSpec) -> Vec<Warning> {
    if spec.entities.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.entities.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

const ENTITY_W: f32 = 208.0;
const HEADER_H: f32 = 36.0;
const FIELD_H: f32 = 18.0;
const MIN_ENTITY_H: f32 = 86.0;
const COL_GAP: f32 = 72.0;
const ROW_GAP: f32 = 48.0;
const MARGIN: f32 = 40.0;

fn entity_h(entity: &Entity) -> f32 {
    (HEADER_H + entity.fields.len().max(1) as f32 * FIELD_H + 16.0).max(MIN_ENTITY_H)
}

fn field_label(field: &EntityField) -> String {
    let prefix = match field.key {
        FieldKey::None => "",
        FieldKey::Pk => "# ",
        FieldKey::Fk => "-> ",
    };
    match &field.r#type {
        Some(t) if !t.is_empty() => format!("{prefix}{}: {t}", field.name),
        _ => format!("{prefix}{}", field.name),
    }
}

fn draw_entity(out: &mut DiagramLayout, entity: &Entity, x: f32, y: f32, h: f32, theme: &Theme) {
    let focal = entity.role == EntityRole::Focal;
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
    out.push(Primitive::Rect {
        x,
        y,
        w: ENTITY_W,
        h,
        fill,
        stroke,
        stroke_width: theme.stroke_default,
        corner_radius: theme.corner_radius,
    });
    out.push(Primitive::Line {
        from: Point::new(x, y + HEADER_H),
        to: Point::new(x + ENTITY_W, y + HEADER_H),
        color: theme.palette.rule,
        stroke_width: theme.stroke_default,
        style: LineStyle::Solid,
    });
    crate::types::eyebrow::push_eyebrow(out, x, y, "ENTITY", stroke);
    out.push(Primitive::Text {
        x: x + ENTITY_W / 2.0,
        y: y + 24.0,
        text: entity.name.clone(),
        font_size: theme.typography.label_size,
        color: stroke,
        align: TextAlign::Center,
        weight: TextWeight::SemiBold,
    });

    if entity.fields.is_empty() {
        out.push(Primitive::Text {
            x: x + 16.0,
            y: y + HEADER_H + 20.0,
            text: "(fields omitted)".to_string(),
            font_size: theme.typography.sublabel_size,
            color: theme.palette.soft,
            align: TextAlign::Left,
            weight: TextWeight::Regular,
        });
    } else {
        for (i, field) in entity.fields.iter().enumerate() {
            out.push(Primitive::Text {
                x: x + 16.0,
                y: y + HEADER_H + 16.0 + i as f32 * FIELD_H,
                text: field_label(field),
                font_size: theme.typography.sublabel_size,
                color: match field.key {
                    FieldKey::Pk => stroke,
                    FieldKey::Fk => theme.palette.muted,
                    FieldKey::None => theme.palette.soft,
                },
                align: TextAlign::Left,
                weight: if field.key == FieldKey::Pk {
                    TextWeight::Medium
                } else {
                    TextWeight::Regular
                },
            });
        }
    }
}

fn edge_points(from: crate::primitive::Rect, to: crate::primitive::Rect) -> (Point, Point) {
    let from_c = Point::new(from.x + from.w / 2.0, from.y + from.h / 2.0);
    let to_c = Point::new(to.x + to.w / 2.0, to.y + to.h / 2.0);
    if (to_c.x - from_c.x).abs() >= (to_c.y - from_c.y).abs() {
        if to_c.x >= from_c.x {
            (
                Point::new(from.x + from.w, from_c.y),
                Point::new(to.x, to_c.y),
            )
        } else {
            (
                Point::new(from.x, from_c.y),
                Point::new(to.x + to.w, to_c.y),
            )
        }
    } else if to_c.y >= from_c.y {
        (
            Point::new(from_c.x, from.y + from.h),
            Point::new(to_c.x, to.y),
        )
    } else {
        (
            Point::new(from_c.x, from.y),
            Point::new(to_c.x, to.y + to.h),
        )
    }
}

#[must_use]
pub fn layout_er(spec: &ErSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.entities.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let n = spec.entities.len();
    let cols = n.min(3);
    let rows = n.div_ceil(cols);
    let total_w = cols as f32 * ENTITY_W + cols.saturating_sub(1) as f32 * COL_GAP;
    let left = ((ctx.canvas_width - total_w) / 2.0).max(MARGIN);
    let mut row_heights = vec![MIN_ENTITY_H; rows];
    for (i, entity) in spec.entities.iter().enumerate() {
        row_heights[i / cols] = row_heights[i / cols].max(entity_h(entity));
    }
    let total_h: f32 = row_heights.iter().sum::<f32>() + rows.saturating_sub(1) as f32 * ROW_GAP;
    let top = ((ctx.canvas_height - total_h) / 2.0).max(MARGIN);

    let mut rects = vec![crate::primitive::Rect::zero(); n];
    let mut y = top;
    for (row, row_h) in row_heights.iter().copied().enumerate() {
        for col in 0..cols {
            let idx = row * cols + col;
            if idx >= n {
                break;
            }
            let h = entity_h(&spec.entities[idx]);
            let x = left + col as f32 * (ENTITY_W + COL_GAP);
            rects[idx] = crate::primitive::Rect::new(x, y + (row_h - h) / 2.0, ENTITY_W, h);
        }
        y += row_h + ROW_GAP;
    }

    let id_to_idx: HashMap<&str, usize> = spec
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();

    for rel in &spec.relationships {
        let (Some(&a), Some(&b)) = (
            id_to_idx.get(rel.from.as_str()),
            id_to_idx.get(rel.to.as_str()),
        ) else {
            continue;
        };
        let (from, to) = edge_points(rects[a], rects[b]);
        let color = crate::types::shared::edge_color_for_role(rel.role, theme);
        out.push(Primitive::Line {
            from,
            to,
            color,
            stroke_width: theme.stroke_default,
            style: LineStyle::Solid,
        });
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let ux = dx / len;
        let uy = dy / len;
        if let Some(card) = &rel.from_cardinality {
            out.push(Primitive::Text {
                x: from.x + ux * 14.0,
                y: from.y + uy * 14.0 - 8.0,
                text: card.clone(),
                font_size: theme.typography.annotation_size,
                color,
                align: TextAlign::Center,
                weight: TextWeight::Medium,
            });
        }
        if let Some(card) = &rel.to_cardinality {
            out.push(Primitive::Text {
                x: to.x - ux * 14.0,
                y: to.y - uy * 14.0 - 8.0,
                text: card.clone(),
                font_size: theme.typography.annotation_size,
                color,
                align: TextAlign::Center,
                weight: TextWeight::Medium,
            });
        }
        if let Some(label) = &rel.label {
            out.push(Primitive::Text {
                x: (from.x + to.x) * 0.5,
                y: (from.y + to.y) * 0.5 - 10.0,
                text: label.clone(),
                font_size: theme.typography.annotation_size,
                color,
                align: TextAlign::Center,
                weight: TextWeight::Regular,
            });
        }
    }

    for (entity, rect) in spec.entities.iter().zip(rects) {
        draw_entity(&mut out, entity, rect.x, rect.y, rect.h, theme);
    }

    out
}
