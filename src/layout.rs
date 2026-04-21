//! Layout output container and input context shared across types.

use crate::primitive::{Point, Primitive, Rect};
use crate::theme::Theme;

/// Input to a per-type layout function.
///
/// `canvas_width` / `canvas_height` define the bounding box the layout engine
/// should fit into. Both are in logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct LayoutContext {
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub theme: Theme,
}

impl LayoutContext {
    /// Construct a layout context with the default [`Theme::light`].
    #[must_use]
    pub fn new(canvas_width: f32, canvas_height: f32) -> Self {
        Self {
            canvas_width,
            canvas_height,
            theme: Theme::light(),
        }
    }

    /// Override the theme.
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Default for LayoutContext {
    fn default() -> Self {
        // Mirrors the editorial SVG viewBox used by reference assets: 1000 × 500
        // is a reasonable default for previews and `gallery` example output.
        Self::new(1000.0, 500.0)
    }
}

/// Positioned output of a diagram layout pass.
#[derive(Debug, Clone, Default)]
pub struct DiagramLayout {
    /// Draw order — earlier primitives render under later ones.
    pub primitives: Vec<Primitive>,
    /// Tight bounding box around all emitted primitives.
    pub bounds: Rect,
}

impl DiagramLayout {
    /// Empty layout. Used when input has zero parseable elements.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Push a primitive and update the running bounds.
    pub fn push(&mut self, p: Primitive) {
        update_bounds(&mut self.bounds, &p);
        self.primitives.push(p);
    }

    /// Number of primitives currently in the layout.
    #[must_use]
    pub fn primitive_count(&self) -> usize {
        self.primitives.len()
    }
}

fn update_bounds(b: &mut Rect, p: &Primitive) {
    match p {
        Primitive::Rect { x, y, w, h, .. } => {
            b.expand_to(Point::new(*x, *y));
            b.expand_to(Point::new(*x + *w, *y + *h));
        }
        Primitive::Polygon { points, .. } => {
            for pt in points {
                b.expand_to(*pt);
            }
        }
        Primitive::Line { from, to, .. } | Primitive::Arrow { from, to, .. } => {
            b.expand_to(*from);
            b.expand_to(*to);
        }
        Primitive::Text { x, y, .. } => {
            // Text extent is font-dependent; bounds track the anchor.
            b.expand_to(Point::new(*x, *y));
        }
    }
}
