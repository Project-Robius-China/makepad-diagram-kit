//! Renderer-agnostic drawing primitives.
//!
//! A [`Primitive`] describes *what* to draw, not *how*. Coordinates are logical
//! pixels (lpx) with origin at the top-left. A downstream renderer (e.g., a
//! Makepad widget gated under the `makepad` feature in a later task) walks
//! these primitives and issues its native draw calls.

use crate::theme::Color;

/// 2D point in logical pixels. Deliberately not named `Vec2` — we don't want
/// consumers to conflate this with a SIMD math type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Construct a point from its components.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle described by its top-left corner and size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Construct a rect from its components.
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Empty rect at the origin. Useful as a default for an empty layout.
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Extend this rect to contain `p`.
    pub fn expand_to(&mut self, p: Point) {
        if self.w == 0.0 && self.h == 0.0 && self.x == 0.0 && self.y == 0.0 {
            self.x = p.x;
            self.y = p.y;
            return;
        }
        let right = (self.x + self.w).max(p.x);
        let bottom = (self.y + self.h).max(p.y);
        self.x = self.x.min(p.x);
        self.y = self.y.min(p.y);
        self.w = right - self.x;
        self.h = bottom - self.y;
    }
}

/// Horizontal text alignment anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Typographic weight hint for the renderer. Renderers free to approximate
/// with the nearest available weight in their font stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWeight {
    Regular,
    Medium,
    SemiBold,
}

/// Line dashing style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
}

/// A single drawing primitive emitted by a layout engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    /// Axis-aligned rectangle. `stroke_width == 0.0` means fill-only.
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Color,
        stroke: Color,
        stroke_width: f32,
        corner_radius: f32,
    },
    /// Circle with optional fill and stroke. `(cx, cy)` is the center.
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: Color,
        stroke: Color,
        stroke_width: f32,
    },
    /// Arbitrary polygon — used for trapezoids (pyramid layers) and diamonds
    /// (flowchart decisions). Points must be in draw order and form a simple
    /// closed shape.
    Polygon {
        points: Vec<Point>,
        fill: Color,
        stroke: Color,
        stroke_width: f32,
    },
    /// Straight line segment.
    Line {
        from: Point,
        to: Point,
        color: Color,
        stroke_width: f32,
        style: LineStyle,
    },
    /// Arrow — a line segment with a head at `to`. Renderer decides arrowhead
    /// geometry (typically small triangle, ~6 lpx).
    Arrow {
        from: Point,
        to: Point,
        color: Color,
        stroke_width: f32,
        style: LineStyle,
    },
    /// Single line of text. `(x, y)` is the baseline anchor per the `align`
    /// mode.
    Text {
        x: f32,
        y: f32,
        text: String,
        font_size: f32,
        color: Color,
        align: TextAlign,
        weight: TextWeight,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_expand_tracks_extents() {
        let mut r = Rect::zero();
        r.expand_to(Point::new(10.0, 10.0));
        r.expand_to(Point::new(20.0, 30.0));
        assert_eq!(r, Rect::new(10.0, 10.0, 10.0, 20.0));
    }
}
