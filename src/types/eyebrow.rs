//! Eyebrow tag — small outlined pill in the top-left of a node box.
//!
//! Usage pattern from `diagram-design/assets/example-tree.html` lines 96-97
//! (ROOT) and 103-104 (CAT):
//!
//! ```svg
//! <rect x="428" y="88" width="32" height="12" rx="2"
//!       fill="transparent" stroke="rgba(11,13,11,0.40)" stroke-width="0.8"/>
//! <text x="444" y="97" fill="#0b0d0b" font-size="7"
//!       font-family="'Geist Mono', monospace" text-anchor="middle"
//!       letter-spacing="0.08em">ROOT</text>
//! ```
//!
//! Visual rule: 8 px inset from the node's top-left corner, 12 px tall,
//! width scales with tag length. Uppercase, mono, ~7 px.

use crate::primitive::{Primitive, TextAlign, TextWeight};
use crate::theme::Color;

/// Vertical inset of the tag box from the node rect's top edge.
pub(crate) const INSET_Y: f32 = 6.0;
/// Horizontal inset of the tag box from the node rect's left edge.
pub(crate) const INSET_X: f32 = 8.0;
/// Tag box height. Bumped 12 → 16 so the 9-lpx mono text has breathing
/// room inside the outline (Makepad's DrawText at font_size 7 visually
/// overflowed the previous 12-lpx box).
pub(crate) const TAG_HEIGHT: f32 = 16.0;
/// Tag font size (lpx).
pub(crate) const TAG_FONT_SIZE: f32 = 9.0;
/// Stroke thickness for the tag outline (`stroke-thin` per style guide).
pub(crate) const TAG_STROKE: f32 = 0.8;
/// Corner radius for the tag outline (`radius-sm` per style guide).
pub(crate) const TAG_RADIUS: f32 = 2.0;

/// Tag width given the uppercase-tag length, empirically tuned against the
/// reference HTML (`ROOT` → 36 lpx, `CAT` → 30 lpx, `EXT` → 30 lpx).
#[inline]
pub(crate) fn tag_width(len: usize) -> f32 {
    // ~7 lpx per uppercase mono glyph at 9-lpx size + 10 lpx combined padding.
    (len as f32) * 7.0 + 10.0
}

/// Push the two primitives that make up an eyebrow tag: the outline rect
/// and the uppercase text.
///
/// `node_x`, `node_y` refer to the node rect's top-left corner.
/// `stroke` is the tag outline color; the text uses the same color so tag
/// hue tracks node role (accent nodes get an accent-hued tag).
pub(crate) fn push_eyebrow(
    out: &mut crate::layout::DiagramLayout,
    node_x: f32,
    node_y: f32,
    tag: &str,
    stroke: Color,
) {
    let upper: String = tag.to_uppercase();
    let w = tag_width(upper.chars().count());
    let x = node_x + INSET_X;
    let y = node_y + INSET_Y;

    out.push(Primitive::Rect {
        x,
        y,
        w,
        h: TAG_HEIGHT,
        // Transparent fill — the node rect shows through.
        fill: Color::rgba(0, 0, 0, 0),
        stroke,
        stroke_width: TAG_STROKE,
        corner_radius: TAG_RADIUS,
    });

    // Centred text inside the tag box. Empirically Makepad renders the
    // glyph top ~2 lpx above `pos.y` at small sizes (internal ascent +
    // leading), so we shift further down by ~0.5 of the font size from
    // the top to land the visual mid-line on the box center.
    out.push(Primitive::Text {
        x: x + w / 2.0,
        y: y + TAG_HEIGHT * 0.5,
        text: upper,
        font_size: TAG_FONT_SIZE,
        color: stroke,
        align: TextAlign::Center,
        weight: TextWeight::Medium,
    });
}
