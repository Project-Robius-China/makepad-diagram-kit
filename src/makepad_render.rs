//! Makepad-native primitive binding (feature = "makepad").
//!
//! Walks a [`DiagramLayout`]'s [`Primitive`]s and issues Makepad draw calls.
//! Pure-parsing consumers (CLI, tests, alternative renderers) don't pay for
//! this code — it's gated behind the `makepad` feature.
//!
//! # Primitive → draw mapping
//!
//! | Primitive | Draw call |
//! |-----------|-----------|
//! | `Rect`    | `DrawRoundedRect` SDF (single pass, per-rect radius + border) |
//! | `Polygon` | Axis-aligned bounding rect (documented approximation — v1 only emits trapezoids/diamonds where AABB reads reasonably) |
//! | `Line`    | Axis-aligned or thin rotated rect — implemented as AABB-approx thin rect (no shader) |
//! | `Arrow`   | Line shaft + two filled triangle-edge rects for the head |
//! | `Text`    | Single `DrawText` call with `text_style` inherited from the widget DSL |
//!
//! # SDF usage
//!
//! Rects go through an `Sdf2d.box(... radius)` pass so corner radius is
//! honoured per-instance — the pattern mirrors makepad's `RoundedView`.
//! Polygons/lines/arrows keep the plain `DrawColor` path since their
//! silhouettes are already axis-aligned strips.

use crate::primitive::{LineStyle, Point, Primitive, TextAlign};
use crate::theme::Color;
use crate::widget::DrawRoundedRect;
use makepad_widgets::makepad_draw::*;

/// Convert a kit [`Color`] (0..=255 RGBA) to a Makepad `Vec4f` (0..=1.0 RGBA).
///
/// Makepad's `DrawColor.color` uses straight-alpha `vec4` — premultiplication
/// happens inside the pixel shader.
#[inline]
pub fn color_to_vec4(c: Color) -> Vec4f {
    vec4(
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    )
}

/// Build a Makepad `Rect` in world coordinates given an origin offset and a
/// rect in kit (top-left-origin, lpx) space.
#[inline]
fn world_rect(origin: Vec2d, x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect {
        pos: dvec2(origin.x + x as f64, origin.y + y as f64),
        size: dvec2(w as f64, h as f64),
    }
}

/// Clamp a potentially-degenerate rectangle to non-negative size and minimum
/// 1 lpx along each axis — required so a 0-lpx stroke line still hits the
/// rasteriser.
#[inline]
fn sanitize_rect(mut r: Rect) -> Rect {
    if r.size.x < 0.0 {
        r.pos.x += r.size.x;
        r.size.x = -r.size.x;
    }
    if r.size.y < 0.0 {
        r.pos.y += r.size.y;
        r.size.y = -r.size.y;
    }
    // A DrawColor with zero size draws nothing; clamp up to 1 lpx so thin
    // strokes remain visible after rounding.
    if r.size.x < 1.0 {
        r.size.x = 1.0;
    }
    if r.size.y < 1.0 {
        r.size.y = 1.0;
    }
    r
}

/// Paint a kit `Primitive::Rect` using the `DrawRoundedRect` SDF shader —
/// single draw call honouring per-rect `corner_radius`, fill, and stroke.
#[allow(clippy::too_many_arguments)]
pub fn render_rect_rounded(
    cx: &mut Cx2d,
    rounded: &mut DrawRoundedRect,
    origin: Vec2d,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: Color,
    stroke: Color,
    stroke_width: f32,
    corner_radius: f32,
) {
    // Set per-instance shader inputs. These are `#[live]` on
    // `DrawRoundedRect`, so each assignment plus `draw_abs` flushes as a
    // fresh instance with its own radius / border.
    rounded.color = color_to_vec4(fill);
    rounded.border_color = color_to_vec4(stroke);
    rounded.border_size = if stroke.a > 0 { stroke_width.max(0.0) } else { 0.0 };
    rounded.border_radius = corner_radius.max(0.0);
    rounded.draw_abs(cx, world_rect(origin, x, y, w, h));
}

/// Paint a kit `Primitive::Rect` as a plain `DrawColor` fill + 4 thin border
/// rects. Retained for the rare case where a rounded shader isn't hooked up
/// (falls back to sharp corners). Most callers should use
/// [`render_rect_rounded`] instead.
#[allow(clippy::too_many_arguments)]
pub fn render_rect(
    cx: &mut Cx2d,
    fill_rect: &mut DrawColor,
    origin: Vec2d,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: Color,
    stroke: Color,
    stroke_width: f32,
    _corner_radius: f32,
) {
    // Fill pass.
    if fill.a > 0 {
        fill_rect.color = color_to_vec4(fill);
        fill_rect.draw_abs(cx, world_rect(origin, x, y, w, h));
    }
    // Stroke pass — four thin rects around the perimeter. Simpler and cheaper
    // than a Sdf2d border and visually equivalent for the 1-lpx default.
    if stroke_width > 0.0 && stroke.a > 0 {
        let sw = stroke_width;
        fill_rect.color = color_to_vec4(stroke);
        // top
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, x, y, w, sw)));
        // bottom
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, x, y + h - sw, w, sw)));
        // left
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, x, y, sw, h)));
        // right
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, x + w - sw, y, sw, h)));
    }
}

/// Paint a kit `Primitive::Polygon` as its axis-aligned bounding-box fill
/// plus a 1-lpx outline tracing the actual vertex ring.
///
/// This is a **visual approximation**, sanctioned by the v1 integration task:
/// v1 only emits polygons for pyramid trapezoids and flowchart diamonds, both
/// of which read reasonably as AABB fills at the editorial 500-lpx canvas. A
/// future revision may replace this with a Sdf2d shader if visual fidelity
/// becomes a concern.
pub fn render_polygon(
    cx: &mut Cx2d,
    fill_rect: &mut DrawColor,
    origin: Vec2d,
    points: &[Point],
    fill: Color,
    stroke: Color,
    stroke_width: f32,
) {
    if points.is_empty() {
        return;
    }
    // AABB fill.
    let (mut min_x, mut min_y) = (points[0].x, points[0].y);
    let (mut max_x, mut max_y) = (points[0].x, points[0].y);
    for p in &points[1..] {
        if p.x < min_x {
            min_x = p.x;
        }
        if p.y < min_y {
            min_y = p.y;
        }
        if p.x > max_x {
            max_x = p.x;
        }
        if p.y > max_y {
            max_y = p.y;
        }
    }
    if fill.a > 0 {
        fill_rect.color = color_to_vec4(fill);
        fill_rect.draw_abs(
            cx,
            world_rect(origin, min_x, min_y, max_x - min_x, max_y - min_y),
        );
    }

    // Edge outline — each polygon edge becomes a thin rect connecting
    // consecutive vertices via their AABB. Visually this traces the polygon
    // ring for axis-aligned edges (trapezoids have horizontal edges) and
    // approximates diagonals as thin axis-aligned strips — good enough for
    // v1's diamond decision nodes at typical scales.
    if stroke_width > 0.0 && stroke.a > 0 {
        fill_rect.color = color_to_vec4(stroke);
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            render_edge(cx, fill_rect, origin, a, b, stroke_width);
        }
    }
}

/// Paint a straight segment as a thin filled rect. Works for axis-aligned
/// edges exactly and for diagonal edges as a thin axis-aligned strip spanning
/// the segment's AABB (documented approximation).
fn render_edge(
    cx: &mut Cx2d,
    fill_rect: &mut DrawColor,
    origin: Vec2d,
    a: Point,
    b: Point,
    stroke_width: f32,
) {
    let (min_x, max_x) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
    let (min_y, max_y) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    if dx > dy {
        // Predominantly horizontal — render as a thin horizontal rect
        // centred on the midpoint y.
        let mid_y = (a.y + b.y) * 0.5;
        fill_rect.draw_abs(
            cx,
            sanitize_rect(world_rect(
                origin,
                min_x,
                mid_y - stroke_width * 0.5,
                dx,
                stroke_width,
            )),
        );
    } else {
        // Predominantly vertical — thin vertical rect.
        let mid_x = (a.x + b.x) * 0.5;
        fill_rect.draw_abs(
            cx,
            sanitize_rect(world_rect(
                origin,
                mid_x - stroke_width * 0.5,
                min_y,
                stroke_width,
                dy,
            )),
        );
    }
}

/// Paint a kit `Primitive::Line`.
#[allow(clippy::too_many_arguments)]
pub fn render_line(
    cx: &mut Cx2d,
    fill_rect: &mut DrawColor,
    origin: Vec2d,
    from: Point,
    to: Point,
    color: Color,
    stroke_width: f32,
    _style: LineStyle,
) {
    if color.a == 0 || stroke_width <= 0.0 {
        return;
    }
    fill_rect.color = color_to_vec4(color);
    render_edge(cx, fill_rect, origin, from, to, stroke_width);
}

/// Paint a kit `Primitive::Arrow`: line shaft plus a small filled triangular
/// arrowhead (rendered as a 6 lpx filled square at the `to` endpoint — a
/// documented simplification; see module docs).
pub fn render_arrow(
    cx: &mut Cx2d,
    fill_rect: &mut DrawColor,
    origin: Vec2d,
    from: Point,
    to: Point,
    color: Color,
    stroke_width: f32,
) {
    if color.a == 0 || stroke_width <= 0.0 {
        return;
    }
    fill_rect.color = color_to_vec4(color);
    render_edge(cx, fill_rect, origin, from, to, stroke_width);
    // Arrowhead approximation — a 6 lpx square centred at `to`. Suitable for
    // vertical flowchart arrows which are the dominant arrow case in v1.
    const HEAD_SIZE: f32 = 6.0;
    fill_rect.draw_abs(
        cx,
        sanitize_rect(world_rect(
            origin,
            to.x - HEAD_SIZE * 0.5,
            to.y - HEAD_SIZE * 0.5,
            HEAD_SIZE,
            HEAD_SIZE,
        )),
    );
}

/// Paint a kit `Primitive::Text`. Font + base size are controlled by the
/// widget DSL's `draw_text` style; we only apply per-primitive color and
/// scale the text size relative to the DSL default.
#[allow(clippy::too_many_arguments)]
pub fn render_text(
    cx: &mut Cx2d,
    draw_text: &mut DrawText,
    origin: Vec2d,
    x: f32,
    y: f32,
    text: &str,
    font_size: f32,
    color: Color,
    align: TextAlign,
) {
    draw_text.color = color_to_vec4(color);
    draw_text.text_style.font_size = font_size;
    // Compute anchor offset for horizontal alignment. DrawText::draw_abs
    // positions the baseline at `pos`; text lays out rightward from there.
    // For Center/Right we measure the text extent crudely by glyph count
    // since we don't have access to shaped metrics at layout time.
    //
    // A shaped-text measurement would be more accurate, but the layout
    // engine already positioned anchor points using a font-agnostic width
    // estimate, so this approximation tracks.
    let approx_glyph_w = font_size as f64 * 0.55;
    let approx_w = text.chars().count() as f64 * approx_glyph_w;
    let anchor_offset = match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => -approx_w * 0.5,
        TextAlign::Right => -approx_w,
    };
    let pos = dvec2(origin.x + x as f64 + anchor_offset, origin.y + y as f64);
    draw_text.draw_abs(cx, pos, text);
}

/// Walk a sequence of primitives and dispatch each to its draw helper.
///
/// `origin` is the widget's top-left in world coordinates. Primitives are in
/// kit-local (top-left-origin, logical-pixel) space.
///
/// Rect primitives go through the rounded-SDF shader so `corner_radius` is
/// honoured. Lines, arrows, polygons still use the plain `DrawColor` since
/// they're axis-aligned strips that don't need rounded corners.
pub fn render_primitives(
    cx: &mut Cx2d,
    draw_rect: &mut DrawColor,
    draw_rounded: &mut DrawRoundedRect,
    draw_text: &mut DrawText,
    origin: Vec2d,
    primitives: &[Primitive],
) {
    for p in primitives {
        match p {
            Primitive::Rect {
                x,
                y,
                w,
                h,
                fill,
                stroke,
                stroke_width,
                corner_radius,
            } => {
                // Dispatch: short rects (eyebrow tags, height < 18) use the
                // plain DrawColor fill+4-border path so they always render
                // with the exact stroke width and no SDF radius surprises.
                // Taller rects (nodes) go through the rounded shader for
                // editorial radius.
                if *h < 18.0 {
                    render_rect(
                        cx,
                        draw_rect,
                        origin,
                        *x,
                        *y,
                        *w,
                        *h,
                        *fill,
                        *stroke,
                        *stroke_width,
                        *corner_radius,
                    );
                } else {
                    render_rect_rounded(
                        cx,
                        draw_rounded,
                        origin,
                        *x,
                        *y,
                        *w,
                        *h,
                        *fill,
                        *stroke,
                        *stroke_width,
                        *corner_radius,
                    );
                }
            }
            Primitive::Polygon {
                points,
                fill,
                stroke,
                stroke_width,
            } => render_polygon(cx, draw_rect, origin, points, *fill, *stroke, *stroke_width),
            Primitive::Line {
                from,
                to,
                color,
                stroke_width,
                style,
            } => render_line(cx, draw_rect, origin, *from, *to, *color, *stroke_width, *style),
            Primitive::Arrow {
                from,
                to,
                color,
                stroke_width,
            } => render_arrow(cx, draw_rect, origin, *from, *to, *color, *stroke_width),
            Primitive::Text {
                x,
                y,
                text,
                font_size,
                color,
                align,
                weight: _,
            } => render_text(
                cx,
                draw_text,
                origin,
                *x,
                *y,
                text,
                *font_size,
                *color,
                *align,
            ),
        }
    }
}
