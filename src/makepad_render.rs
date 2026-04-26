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
//! | `Arrow`   | Line shaft + SDF triangular arrowhead |
//! | `Text`    | Single `DrawText` call with `text_style` inherited from the widget DSL |
//!
//! # SDF usage
//!
//! Rects go through an `Sdf2d.box(... radius)` pass so corner radius is
//! honoured per-instance — the pattern mirrors makepad's `RoundedView`.
//! Polygons/lines/arrows keep the plain `DrawColor` path since their
//! silhouettes are already axis-aligned strips.

use crate::primitive::{LineStyle, Point, Primitive, Rect as PrimitiveRect, TextAlign};
use crate::theme::Color;
use crate::widget::{DrawArrowHead, DrawCircle, DrawRoundedRect};
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
    rounded.border_size = if stroke.a > 0 {
        stroke_width.max(0.0)
    } else {
        0.0
    };
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

/// Paint a kit `Primitive::Circle` using the `DrawCircle` SDF shader.
#[allow(clippy::too_many_arguments)]
pub fn render_circle(
    cx: &mut Cx2d,
    circle: &mut DrawCircle,
    origin: Vec2d,
    center_x: f32,
    center_y: f32,
    radius: f32,
    fill: Color,
    stroke: Color,
    stroke_width: f32,
) {
    if radius <= 0.0 {
        return;
    }
    circle.color = color_to_vec4(fill);
    circle.border_color = color_to_vec4(stroke);
    circle.border_size = if stroke.a > 0 {
        stroke_width.max(0.0)
    } else {
        0.0
    };
    circle.radius = radius;
    circle.draw_abs(
        cx,
        world_rect(
            origin,
            center_x - radius,
            center_y - radius,
            radius * 2.0,
            radius * 2.0,
        ),
    );
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
    for r in line_segment_rects(a, b, stroke_width, LineStyle::Solid) {
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, r.x, r.y, r.w, r.h)));
    }
}

fn segment_rect(a: Point, b: Point, stroke_width: f32) -> PrimitiveRect {
    let (min_x, max_x) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
    let (min_y, max_y) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    if dx > dy {
        // Predominantly horizontal — render as a thin horizontal rect
        // centred on the midpoint y.
        let mid_y = (a.y + b.y) * 0.5;
        PrimitiveRect::new(min_x, mid_y - stroke_width * 0.5, dx, stroke_width)
    } else {
        // Predominantly vertical — thin vertical rect.
        let mid_x = (a.x + b.x) * 0.5;
        PrimitiveRect::new(mid_x - stroke_width * 0.5, min_y, stroke_width, dy)
    }
}

/// Split a line/arrow shaft into draw rects. Dashed style is implemented here
/// rather than in the layout layer so all diagram types get identical stroke
/// semantics.
fn line_segment_rects(
    from: Point,
    to: Point,
    stroke_width: f32,
    style: LineStyle,
) -> Vec<PrimitiveRect> {
    match style {
        LineStyle::Solid => vec![segment_rect(from, to, stroke_width)],
        LineStyle::Dashed => {
            const DASH: f32 = 8.0;
            const GAP: f32 = 6.0;
            let dx = to.x - from.x;
            let dy = to.y - from.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= f32::EPSILON {
                return vec![segment_rect(from, to, stroke_width)];
            }
            let ux = dx / len;
            let uy = dy / len;
            let mut rects = Vec::new();
            let mut pos = 0.0;
            while pos < len {
                let end = (pos + DASH).min(len);
                if end > pos {
                    let a = Point::new(from.x + ux * pos, from.y + uy * pos);
                    let b = Point::new(from.x + ux * end, from.y + uy * end);
                    rects.push(segment_rect(a, b, stroke_width));
                }
                pos += DASH + GAP;
            }
            rects
        }
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
    style: LineStyle,
) {
    if color.a == 0 || stroke_width <= 0.0 {
        return;
    }
    fill_rect.color = color_to_vec4(color);
    for r in line_segment_rects(from, to, stroke_width, style) {
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, r.x, r.y, r.w, r.h)));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowHeadDirection {
    Right,
    Left,
    Down,
    Up,
}

impl ArrowHeadDirection {
    fn as_shader_value(self) -> f32 {
        match self {
            ArrowHeadDirection::Right => 0.0,
            ArrowHeadDirection::Left => 1.0,
            ArrowHeadDirection::Down => 2.0,
            ArrowHeadDirection::Up => 3.0,
        }
    }

    #[cfg(test)]
    fn shader_points(self, w: f32, h: f32) -> [Point; 3] {
        match self {
            ArrowHeadDirection::Right => [
                Point::new(w, h * 0.5),
                Point::new(0.0, h),
                Point::new(0.0, 0.0),
            ],
            ArrowHeadDirection::Left => [
                Point::new(0.0, h * 0.5),
                Point::new(w, 0.0),
                Point::new(w, h),
            ],
            ArrowHeadDirection::Down => [
                Point::new(w * 0.5, h),
                Point::new(0.0, 0.0),
                Point::new(w, 0.0),
            ],
            ArrowHeadDirection::Up => [
                Point::new(w * 0.5, 0.0),
                Point::new(w, h),
                Point::new(0.0, h),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ArrowHeadGeometry {
    rect: PrimitiveRect,
    direction: ArrowHeadDirection,
}

/// Compute the tight draw quad for a cardinal SDF triangle arrowhead.
fn arrow_head_geometry(from: Point, to: Point) -> ArrowHeadGeometry {
    let dy = to.y - from.y;
    let dx = to.x - from.x;
    const HEAD: f32 = 9.0;

    if dy.abs() > dx.abs() {
        if dy > 0.0 {
            ArrowHeadGeometry {
                rect: PrimitiveRect::new(to.x - HEAD * 0.5, to.y - HEAD, HEAD, HEAD),
                direction: ArrowHeadDirection::Down,
            }
        } else {
            ArrowHeadGeometry {
                rect: PrimitiveRect::new(to.x - HEAD * 0.5, to.y, HEAD, HEAD),
                direction: ArrowHeadDirection::Up,
            }
        }
    } else if dx >= 0.0 {
        ArrowHeadGeometry {
            rect: PrimitiveRect::new(to.x - HEAD, to.y - HEAD * 0.5, HEAD, HEAD),
            direction: ArrowHeadDirection::Right,
        }
    } else {
        ArrowHeadGeometry {
            rect: PrimitiveRect::new(to.x, to.y - HEAD * 0.5, HEAD, HEAD),
            direction: ArrowHeadDirection::Left,
        }
    }
}

fn render_arrow_head(
    cx: &mut Cx2d,
    draw_arrow_head: &mut DrawArrowHead,
    origin: Vec2d,
    from: Point,
    to: Point,
    color: Color,
) {
    let head = arrow_head_geometry(from, to);
    draw_arrow_head.color = color_to_vec4(color);
    draw_arrow_head.direction = head.direction.as_shader_value();
    draw_arrow_head.draw_abs(
        cx,
        sanitize_rect(world_rect(
            origin,
            head.rect.x,
            head.rect.y,
            head.rect.w,
            head.rect.h,
        )),
    );
}

/// Paint a kit `Primitive::Arrow`: line shaft plus a small directional
/// arrowhead.
#[allow(clippy::too_many_arguments)]
pub fn render_arrow(
    cx: &mut Cx2d,
    fill_rect: &mut DrawColor,
    draw_arrow_head: &mut DrawArrowHead,
    origin: Vec2d,
    from: Point,
    to: Point,
    color: Color,
    stroke_width: f32,
    style: LineStyle,
) {
    if color.a == 0 || stroke_width <= 0.0 {
        return;
    }
    fill_rect.color = color_to_vec4(color);
    for r in line_segment_rects(from, to, stroke_width, style) {
        fill_rect.draw_abs(cx, sanitize_rect(world_rect(origin, r.x, r.y, r.w, r.h)));
    }
    render_arrow_head(cx, draw_arrow_head, origin, from, to, color);
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    fn winding(points: [Point; 3]) -> f32 {
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .map(|(a, b)| a.x * b.y - a.y * b.x)
            .sum()
    }

    #[test]
    fn horizontal_arrow_head_is_one_directional_triangle_geometry() {
        let head = arrow_head_geometry(Point::new(10.0, 20.0), Point::new(110.0, 20.0));

        assert_eq!(head.direction, ArrowHeadDirection::Right);
        assert_eq!(head.rect, PrimitiveRect::new(101.0, 15.5, 9.0, 9.0));
    }

    #[test]
    fn arrow_head_shader_points_share_visible_winding() {
        for direction in [
            ArrowHeadDirection::Right,
            ArrowHeadDirection::Left,
            ArrowHeadDirection::Down,
            ArrowHeadDirection::Up,
        ] {
            assert!(
                winding(direction.shader_points(9.0, 9.0)) > 0.0,
                "{direction:?} should use the same winding as the visible left arrowhead"
            );
        }
    }

    #[test]
    fn dashed_horizontal_line_breaks_into_short_segments() {
        let solid = line_segment_rects(
            Point::new(10.0, 20.0),
            Point::new(110.0, 20.0),
            2.0,
            LineStyle::Solid,
        );
        let dashed = line_segment_rects(
            Point::new(10.0, 20.0),
            Point::new(110.0, 20.0),
            2.0,
            LineStyle::Dashed,
        );

        assert_eq!(solid.len(), 1);
        assert!(dashed.len() > 3, "expected visible dash segments");
        assert!(dashed.iter().all(|r| r.w <= 8.1));
        assert!(dashed.iter().map(|r| r.w).sum::<f32>() < 100.0);
    }
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
    // Use DrawText's own layout engine to measure the text's shaped size,
    // then compute a truly centered position instead of guessing at
    // glyph metrics. `(x, y)` is interpreted as the desired VISUAL CENTER
    // of the text — horizontal center per `align`, vertical center always.
    //
    // This is the Makepad-idiomatic path (DrawText::layout returns a
    // LaidoutText with `size_in_lpxs` — see draw/src/shader/draw_text.rs
    // line 2449). DrawText::draw_abs then places the text's top-left at
    // pos, so we subtract half-size to center.
    let laid = draw_text.layout(
        cx,
        0.0,
        0.0,
        None,
        false,
        makepad_widgets::makepad_draw::Align::default(),
        text,
    );
    let text_w = laid.size_in_lpxs.width as f64;
    let text_h = laid.size_in_lpxs.height as f64;
    let anchor_x_offset = match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => -text_w * 0.5,
        TextAlign::Right => -text_w,
    };
    let pos = dvec2(
        origin.x + x as f64 + anchor_x_offset,
        origin.y + y as f64 - text_h * 0.5,
    );
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
#[allow(clippy::too_many_arguments)]
pub fn render_primitives(
    cx: &mut Cx2d,
    draw_rect: &mut DrawColor,
    draw_rounded: &mut DrawRoundedRect,
    draw_arrow_head: &mut DrawArrowHead,
    draw_circle: &mut DrawCircle,
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
            Primitive::Circle {
                cx: circle_x,
                cy: circle_y,
                r,
                fill,
                stroke,
                stroke_width,
            } => render_circle(
                cx,
                draw_circle,
                origin,
                *circle_x,
                *circle_y,
                *r,
                *fill,
                *stroke,
                *stroke_width,
            ),
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
            } => render_line(
                cx,
                draw_rect,
                origin,
                *from,
                *to,
                *color,
                *stroke_width,
                *style,
            ),
            Primitive::Arrow {
                from,
                to,
                color,
                stroke_width,
                style,
            } => render_arrow(
                cx,
                draw_rect,
                draw_arrow_head,
                origin,
                *from,
                *to,
                *color,
                *stroke_width,
                *style,
            ),
            Primitive::Text {
                x,
                y,
                text,
                font_size,
                color,
                align,
                weight: _,
            } => render_text(
                cx, draw_text, origin, *x, *y, text, *font_size, *color, *align,
            ),
        }
    }
}
