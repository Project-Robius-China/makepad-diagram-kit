//! Sequence diagram: actors along the top with vertical lifelines, and
//! horizontal message arrows flowing top-to-bottom between lifelines.
//!
//! See `robius/diagram-design/references/type-sequence.md` and
//! `assets/example-sequence.html`. The v1 layout targets request/response
//! flows (3-6 actors, up to ~12 messages); activation bars, return-dashed
//! messages, and numbered-prefix rendering are v1.1 extensions.
//!
//! # Layout (v1)
//!
//! * **Actors** render as boxes across the top, centered at equally spaced
//!   column x-coordinates. Column centers are laid out with a minimum
//!   [`SIBLING_GAP`] between box edges; the total row grows with the
//!   actor count rather than squeezing columns under a fixed canvas.
//! * **Lifelines** run vertically from the actor box's bottom edge down
//!   through all messages, rendered as [`Primitive::Line`] with
//!   [`LineStyle::Dashed`] and a muted stroke.
//! * **Messages** render in JSON order, each at a constant [`ROW_GAP`]
//!   y-step below the previous, as an [`Primitive::Arrow`] from the
//!   source lifeline to the target lifeline. The label sits above the
//!   shaft, centered horizontally. Role → color via the shared
//!   [`crate::types::shared::edge_color_for_role`] helper. Explicit
//!   `kind: "return"` messages and right-to-left messages render dashed,
//!   matching the common UML response convention.
//! * **Self-messages** (`from == to`) render as a right-side U-loop: a
//!   short rightward `Line` stub at `y`, a downward `Line` to `y + 32`,
//!   and an `Arrow` returning leftward onto the same lifeline. The label
//!   sits to the right of the loop.
//!
//! # What's NOT in v1
//!
//! * Activation bars (thin strips on lifelines showing control duration).
//! * Numbered-prefix rendering (the `number` field parses but doesn't
//!   render — top-level `"number": "auto"` parses too).
//! * Activation-edge routing (arrows land on lifelines, not on bar edges).

use crate::errors::{ParseError, Warning};
use crate::layout::{DiagramLayout, LayoutContext};
use crate::primitive::{LineStyle, Point, Primitive, TextAlign, TextWeight};
use crate::theme::{Color, Theme};
use crate::types::flowchart::EdgeRole;
use serde::Deserialize;
use std::collections::HashMap;

/// Soft cap above which [`Warning::DensityHigh`] is emitted. A sequence
/// diagram stays readable up to ~12 messages at a 500 lpx canvas height
/// (12 * [`ROW_GAP`] = 480 lpx) before running off the bottom.
pub const SOFT_CAP: usize = 12;
/// Stable type tag used in warnings.
pub const TYPE_TAG: &str = "sequence";

/// Semantic role for a sequence actor. Keeps the vocabulary narrow —
/// `Default` and `Focal` are the only treatments the v1 layout honours.
/// Architecture's richer role palette (`Store` / `External` / ...) isn't
/// reused because sequence actors don't carry those semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActorRole {
    /// Regular actor. Paper fill, ink stroke.
    #[default]
    Default,
    /// Focal actor (the load-bearing service). Accent_tint fill, accent
    /// stroke — the "one focal per diagram" convention applies.
    Focal,
}

/// One actor in the sequence. Renders as a top-row box; its lifeline drops
/// straight down through all the messages.
#[derive(Debug, Clone, Deserialize)]
pub struct Actor {
    /// Unique ID referenced by `from`/`to` on messages.
    pub id: String,
    /// Primary label (human name, Geist sans).
    pub label: String,
    /// Eyebrow tag rendered in the top-left of the actor box (e.g.
    /// `"CLIENT"`, `"MW"`, `"STORE"`). Rendered uppercase.
    #[serde(default)]
    pub tag: Option<String>,
    /// Second line under the label (mono, smaller) — e.g. "Browser".
    #[serde(default)]
    pub sublabel: Option<String>,
    /// Role → fill/stroke treatment. Defaults to `Default` when omitted.
    #[serde(default)]
    pub role: ActorRole,
}

/// One time-ordered message between two actors. `from`/`to` reference
/// actor `id` strings; when they match, this is a self-message (U-loop).
///
/// Shares [`EdgeRole`] with `flowchart` / `architecture` so the "role →
/// color" rule stays in one place (`shared::edge_color_for_role`).
#[derive(Debug, Clone, Deserialize)]
pub struct SequenceMessage {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub role: EdgeRole,
    /// Call vs return styling. `return` messages draw with dashed shafts.
    /// When omitted, right-to-left messages also draw dashed because most
    /// sequence diagrams use left-to-right calls and right-to-left responses.
    #[serde(default)]
    pub kind: MessageKind,
    /// Optional ordinal prefix (e.g. `"1"`, `"2a"`). Parses but doesn't
    /// render in v1 — kept so fixtures that set it don't fail to parse.
    #[serde(default)]
    pub number: Option<String>,
}

/// Message direction semantics. The v1 renderer only uses this to choose
/// solid vs dashed shafts; richer UML activation semantics remain out of
/// scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MessageKind {
    /// Regular call/request.
    #[default]
    Call,
    /// Response/return message. Aliases keep natural LLM JSON stable.
    #[serde(alias = "response", alias = "reply")]
    Return,
}

/// Sequence JSON spec.
///
/// ```json
/// {
///   "type": "sequence",
///   "actors": [
///     {"id": "user", "label": "User",        "tag": "CLIENT"},
///     {"id": "api",  "label": "API Gateway", "tag": "MW", "role": "focal"},
///     {"id": "db",   "label": "Database",    "tag": "STORE"}
///   ],
///   "messages": [
///     {"from": "user", "to": "api", "label": "POST /login", "role": "primary"},
///     {"from": "api",  "to": "db",  "label": "SELECT user"},
///     {"from": "db",   "to": "api", "label": "row"},
///     {"from": "api",  "to": "user","label": "200 OK",      "role": "primary"}
///   ]
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SequenceSpec {
    pub actors: Vec<Actor>,
    #[serde(default)]
    pub messages: Vec<SequenceMessage>,
    /// Optional top-level numbering directive. Accepts either `"auto"` or
    /// `"off"` strings; v1 parses but doesn't render. Kept so fixtures
    /// can opt in without breaking future renderers.
    #[serde(default)]
    pub number: Option<String>,
}

impl SequenceSpec {
    pub(crate) fn validate(&self) -> Result<(), ParseError> {
        // No accent_idx on sequence — role drives accent. Actor id uniqueness
        // is tolerated at layout time (duplicates shadow earlier entries);
        // message endpoints referencing unknown ids are simply skipped.
        Ok(())
    }
}

pub(crate) fn warnings(spec: &SequenceSpec) -> Vec<Warning> {
    if spec.messages.len() > SOFT_CAP {
        vec![Warning::DensityHigh {
            diagram_type: TYPE_TAG,
            count: spec.messages.len(),
            soft_cap: SOFT_CAP,
        }]
    } else {
        Vec::new()
    }
}

// --- Visual constants ---------------------------------------------------

/// Actor box width. Matches other node types (architecture / flowchart).
pub(crate) const ACTOR_WIDTH: f32 = 160.0;
/// Actor box height. Taller than a flowchart node so a tag + label +
/// sublabel all fit — same 56 lpx the reference HTML uses.
pub(crate) const ACTOR_HEIGHT: f32 = 56.0;
/// Minimum horizontal gap between adjacent actor boxes. The actual gap
/// may grow if the canvas is wider than `MARGIN*2 + n*WIDTH + (n-1)*GAP`.
pub(crate) const SIBLING_GAP: f32 = 80.0;
/// Vertical gap between successive messages (centre-to-centre).
pub(crate) const ROW_GAP: f32 = 40.0;
/// Vertical gap between the actor row's bottom edge and the first message.
const FIRST_MSG_OFFSET: f32 = 32.0;
/// Self-message loop height and stub length.
const SELF_LOOP_H: f32 = 32.0;
const SELF_LOOP_W: f32 = 36.0;
/// Canvas inset from the top/left edge.
const MARGIN: f32 = 24.0;
/// Vertical padding below the last message before the lifeline stops.
const LIFELINE_TAIL: f32 = 24.0;

// --- Role → palette -----------------------------------------------------

/// Map an [`ActorRole`] to its (fill, stroke, stroke_width) triple.
///
/// Public at the crate level so the unit tests can verify the mapping
/// without re-deriving it.
#[must_use]
pub(crate) fn actor_role_colors(role: ActorRole, theme: &Theme) -> (Color, Color, f32) {
    let pal = theme.palette;
    match role {
        ActorRole::Focal => (pal.accent_tint, pal.accent, theme.stroke_default),
        ActorRole::Default => (pal.paper, pal.ink, theme.stroke_default),
    }
}

#[inline]
fn role_is_accent(role: ActorRole) -> bool {
    matches!(role, ActorRole::Focal)
}

fn message_line_style(msg: &SequenceMessage, from_idx: usize, to_idx: usize) -> LineStyle {
    if msg.kind == MessageKind::Return || from_idx > to_idx {
        LineStyle::Dashed
    } else {
        LineStyle::Solid
    }
}

// --- Rendering helpers --------------------------------------------------

/// Draw one actor box (rect + eyebrow + label + sublabel) with its
/// top-left corner at `(x, y)`. Factored out so the main layout loop
/// reads top-down without getting lost in per-primitive details.
fn draw_actor(out: &mut DiagramLayout, actor: &Actor, x: f32, y: f32, theme: &Theme) {
    let (fill, stroke, stroke_w) = actor_role_colors(actor.role, theme);
    out.push(Primitive::Rect {
        x,
        y,
        w: ACTOR_WIDTH,
        h: ACTOR_HEIGHT,
        fill,
        stroke,
        stroke_width: stroke_w,
        corner_radius: theme.corner_radius,
    });

    let accent = role_is_accent(actor.role);
    if let Some(tag) = &actor.tag {
        let tag_color = if accent {
            theme.palette.accent
        } else {
            theme.palette.ink
        };
        crate::types::eyebrow::push_eyebrow(out, x, y, tag, tag_color);
    }

    // Label vertical position follows architecture.rs: shift down when a
    // tag sits top-left; shift up when a sublabel is below.
    let has_tag = actor.tag.is_some();
    let has_sub = actor.sublabel.is_some();
    let mut label_y = y + ACTOR_HEIGHT / 2.0;
    if has_tag {
        label_y += 7.0;
    }
    if has_sub {
        label_y -= 5.0;
    }
    let label_color = if accent {
        theme.palette.accent
    } else {
        theme.palette.ink
    };
    out.push(Primitive::Text {
        x: x + ACTOR_WIDTH / 2.0,
        y: label_y,
        text: actor.label.clone(),
        font_size: theme.typography.label_size,
        color: label_color,
        align: TextAlign::Center,
        weight: TextWeight::SemiBold,
    });
    if let Some(sub) = &actor.sublabel {
        out.push(Primitive::Text {
            x: x + ACTOR_WIDTH / 2.0,
            y: y + ACTOR_HEIGHT * 0.78,
            text: sub.clone(),
            font_size: theme.typography.sublabel_size,
            color: theme.palette.soft,
            align: TextAlign::Center,
            weight: TextWeight::Regular,
        });
    }
}

// --- Layout -------------------------------------------------------------

/// Layout the sequence diagram.
///
/// Draw order: lifelines (back), messages (arrows + labels), actor boxes
/// (front — they mask the lifeline tops). This mirrors the reference
/// example-sequence.html's SVG z-order.
#[must_use]
pub fn layout_sequence(spec: &SequenceSpec, ctx: &LayoutContext) -> DiagramLayout {
    let mut out = DiagramLayout::empty();
    if spec.actors.is_empty() {
        return out;
    }
    let theme = &ctx.theme;
    let n_actors = spec.actors.len();

    // --- Column x-coordinates ------------------------------------------
    //
    // Lay out actors with a uniform sibling gap. If the canvas is wider
    // than the minimum span, expand the gap proportionally so actors
    // occupy the available width. Floor at SIBLING_GAP so narrow canvases
    // still get a readable minimum.
    let min_span =
        n_actors as f32 * ACTOR_WIDTH + (n_actors.saturating_sub(1)) as f32 * SIBLING_GAP;
    let available = ctx.canvas_width - 2.0 * MARGIN;
    let gap = if n_actors > 1 && available > min_span {
        (available - n_actors as f32 * ACTOR_WIDTH) / (n_actors - 1) as f32
    } else {
        SIBLING_GAP
    };

    // Centre the row within the canvas so the diagram reads balanced
    // when the span is narrower than the canvas.
    let used = n_actors as f32 * ACTOR_WIDTH + (n_actors.saturating_sub(1)) as f32 * gap;
    let row_left = ((ctx.canvas_width - used) / 2.0).max(MARGIN);

    // Column center x per actor index.
    let mut col_x = vec![0f32; n_actors];
    for (i, x) in col_x.iter_mut().enumerate() {
        *x = row_left + ACTOR_WIDTH / 2.0 + i as f32 * (ACTOR_WIDTH + gap);
    }

    // --- Vertical geometry ---------------------------------------------
    let actor_top = MARGIN;
    let actor_bottom = actor_top + ACTOR_HEIGHT;
    let first_msg_y = actor_bottom + FIRST_MSG_OFFSET;
    let last_msg_y = first_msg_y + (spec.messages.len().saturating_sub(1)) as f32 * ROW_GAP;
    // Self-messages need extra room below because the loop extends downward.
    let lifeline_bottom = last_msg_y + SELF_LOOP_H + LIFELINE_TAIL;

    // --- Lifelines (drawn first so arrows / actor boxes paint on top) --
    for &cx in &col_x {
        out.push(Primitive::Line {
            from: Point::new(cx, actor_bottom),
            to: Point::new(cx, lifeline_bottom),
            color: theme.palette.rule,
            stroke_width: theme.stroke_default,
            style: LineStyle::Dashed,
        });
    }

    // --- Messages -------------------------------------------------------
    let id_to_idx: HashMap<&str, usize> = spec
        .actors
        .iter()
        .enumerate()
        .map(|(i, a)| (a.id.as_str(), i))
        .collect();

    for (mi, msg) in spec.messages.iter().enumerate() {
        let (Some(&a), Some(&b)) = (
            id_to_idx.get(msg.from.as_str()),
            id_to_idx.get(msg.to.as_str()),
        ) else {
            continue;
        };
        let y = first_msg_y + mi as f32 * ROW_GAP;
        let color = crate::types::shared::edge_color_for_role(msg.role, theme);
        // Primary role gets a slightly heavier shaft like the reference
        // HTML does for coral success lines.
        let stroke_w = match msg.role {
            EdgeRole::Primary => theme.stroke_default * 1.25,
            _ => theme.stroke_default,
        };
        let style = message_line_style(msg, a, b);

        if a == b {
            // Self-message: right-side U-loop at column a.
            let cx = col_x[a];
            // Stub right on the lifeline, down, back onto the lifeline.
            out.push(Primitive::Line {
                from: Point::new(cx, y),
                to: Point::new(cx + SELF_LOOP_W, y),
                color,
                stroke_width: stroke_w,
                style,
            });
            out.push(Primitive::Line {
                from: Point::new(cx + SELF_LOOP_W, y),
                to: Point::new(cx + SELF_LOOP_W, y + SELF_LOOP_H),
                color,
                stroke_width: stroke_w,
                style,
            });
            out.push(Primitive::Arrow {
                from: Point::new(cx + SELF_LOOP_W, y + SELF_LOOP_H),
                to: Point::new(cx, y + SELF_LOOP_H),
                color,
                stroke_width: stroke_w,
                style,
            });
            if let Some(lbl) = &msg.label {
                out.push(Primitive::Text {
                    x: cx + SELF_LOOP_W + 8.0,
                    y: y + SELF_LOOP_H / 2.0,
                    text: lbl.clone(),
                    font_size: theme.typography.annotation_size,
                    color,
                    align: TextAlign::Left,
                    weight: TextWeight::Regular,
                });
            }
        } else {
            // Regular message: horizontal arrow between lifelines.
            let src_x = col_x[a];
            let dst_x = col_x[b];
            // Tuck the endpoints in by 4 lpx so the arrow doesn't sit
            // exactly on the lifeline stroke (reads as "lands on" the
            // target, matching the reference HTML's visual).
            let lean = if dst_x > src_x { 4.0 } else { -4.0 };
            let from = Point::new(src_x + lean, y);
            let to = Point::new(dst_x - lean, y);

            out.push(Primitive::Arrow {
                from,
                to,
                color,
                stroke_width: stroke_w,
                style,
            });

            if let Some(lbl) = &msg.label {
                let mx = (from.x + to.x) / 2.0;
                out.push(Primitive::Text {
                    x: mx,
                    y: y - 6.0,
                    text: lbl.clone(),
                    font_size: theme.typography.annotation_size,
                    color,
                    align: TextAlign::Center,
                    weight: TextWeight::Regular,
                });
            }
        }
    }

    // --- Actor boxes (on top, so they mask the lifeline top edges) -----
    for (i, actor) in spec.actors.iter().enumerate() {
        let x = col_x[i] - ACTOR_WIDTH / 2.0;
        draw_actor(&mut out, actor, x, actor_top, theme);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, parse};

    const BASIC_JSON: &str = r#"{
      "type": "sequence",
      "actors": [
        {"id": "user", "label": "User",        "tag": "CLIENT"},
        {"id": "api",  "label": "API Gateway", "tag": "MW", "role": "focal"},
        {"id": "db",   "label": "Database",    "tag": "STORE"}
      ],
      "messages": [
        {"from": "user", "to": "api", "label": "POST /login", "role": "primary"},
        {"from": "api",  "to": "db",  "label": "SELECT user"},
        {"from": "db",   "to": "api", "label": "row",          "role": "default"},
        {"from": "api",  "to": "user","label": "200 OK",       "role": "primary"}
      ]
    }"#;

    /// Scenario 1: basic parse — 3 actors + 4 messages produces the
    /// expected primitive counts for each category.
    #[test]
    fn sequence_basic_parse() {
        let (diagram, warnings) = parse(BASIC_JSON).unwrap_or_else(|e| panic!("parse: {e}"));
        assert!(matches!(diagram, Diagram::Sequence(_)));
        assert!(warnings.is_empty(), "no density warning at 4 messages");

        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = diagram.layout(&ctx);

        // Actor boxes: 3 rects at ACTOR_WIDTH × ACTOR_HEIGHT.
        let actor_rects = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Rect { w, h, .. } if (*w - ACTOR_WIDTH).abs() < 0.01
                        && (*h - ACTOR_HEIGHT).abs() < 0.01
                )
            })
            .count();
        assert_eq!(actor_rects, 3, "expected 3 actor rects, got {actor_rects}");

        // Lifelines: one dashed Line per actor.
        let lifelines = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Line {
                        style: LineStyle::Dashed,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(lifelines, 3, "expected 3 dashed lifelines");

        // Messages: 4 arrows (none are self-messages in this fixture).
        let arrows = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Arrow { .. }))
            .count();
        assert_eq!(arrows, 4, "expected 4 message arrows");
    }

    /// Scenario 2: actor boxes march left-to-right at equal spacing.
    #[test]
    fn sequence_actor_positions() {
        let (d, _) = parse(BASIC_JSON).unwrap();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = d.layout(&ctx);

        let actor_xs: Vec<f32> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect { x, w, h, .. }
                    if (*w - ACTOR_WIDTH).abs() < 0.01 && (*h - ACTOR_HEIGHT).abs() < 0.01 =>
                {
                    Some(*x)
                }
                _ => None,
            })
            .collect();
        assert_eq!(actor_xs.len(), 3);
        // Strictly left-to-right.
        assert!(
            actor_xs[0] < actor_xs[1] && actor_xs[1] < actor_xs[2],
            "actors must march left-to-right: {actor_xs:?}"
        );
        // Equal spacing between adjacent column origins.
        let d1 = actor_xs[1] - actor_xs[0];
        let d2 = actor_xs[2] - actor_xs[1];
        assert!(
            (d1 - d2).abs() < 0.5,
            "adjacent gaps must match: {d1} vs {d2}"
        );
    }

    /// Scenario 3: message arrows' y-coordinates are strictly increasing
    /// in declaration order (time flows top→down).
    #[test]
    fn sequence_messages_top_to_bottom() {
        let (d, _) = parse(BASIC_JSON).unwrap();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = d.layout(&ctx);

        let arrow_ys: Vec<f32> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Arrow { from, .. } => Some(from.y),
                _ => None,
            })
            .collect();
        assert_eq!(arrow_ys.len(), 4);
        for pair in arrow_ys.windows(2) {
            assert!(
                pair[0] < pair[1],
                "message y must strictly increase: {arrow_ys:?}"
            );
        }
        // Spacing between messages matches ROW_GAP.
        let step = arrow_ys[1] - arrow_ys[0];
        assert!(
            (step - ROW_GAP).abs() < 0.5,
            "message row gap must equal ROW_GAP ({ROW_GAP}), got {step}"
        );
    }

    /// Scenario 4: edge role drives the arrow colour via the shared
    /// dispatch helper.
    #[test]
    fn sequence_role_colors() {
        let (d, _) = parse(BASIC_JSON).unwrap();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = d.layout(&ctx);
        let pal = ctx.theme.palette;

        let arrow_colors: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Arrow { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        // Declared: primary, default, default, primary.
        assert_eq!(
            arrow_colors,
            vec![pal.accent, pal.muted, pal.muted, pal.accent],
            "message colors must follow role → palette mapping"
        );

        // The focal actor box must use the accent stroke — exactly one.
        let accent_actors = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Rect { stroke, w, h, .. }
                        if *stroke == pal.accent
                            && (*w - ACTOR_WIDTH).abs() < 0.01
                            && (*h - ACTOR_HEIGHT).abs() < 0.01
                )
            })
            .count();
        assert_eq!(
            accent_actors, 1,
            "expected one focal actor with accent stroke"
        );
    }

    #[test]
    fn sequence_right_to_left_messages_render_as_dashed_returns() {
        let (d, _) = parse(BASIC_JSON).unwrap();
        let ctx = LayoutContext::new(1000.0, 500.0);
        let layout = d.layout(&ctx);

        let styles: Vec<_> = layout
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Arrow {
                    from, to, style, ..
                } => Some((from.x < to.x, *style)),
                _ => None,
            })
            .collect();

        assert_eq!(
            styles,
            vec![
                (true, LineStyle::Solid),
                (true, LineStyle::Solid),
                (false, LineStyle::Dashed),
                (false, LineStyle::Dashed),
            ]
        );
    }

    #[test]
    fn sequence_explicit_return_kind_renders_dashed_even_left_to_right() {
        let body = r#"{
          "type": "sequence",
          "actors": [
            {"id":"a","label":"A"},{"id":"b","label":"B"}
          ],
          "messages": [
            {"from":"a","to":"b","label":"ack","kind":"return"}
          ]
        }"#;
        let (d, _) = parse(body).unwrap();
        let ctx = LayoutContext::new(800.0, 400.0);
        let layout = d.layout(&ctx);

        let style = layout
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Arrow { style, .. } => Some(*style),
                _ => None,
            })
            .expect("message arrow");

        assert_eq!(style, LineStyle::Dashed);
    }

    /// Scenario 5: a self-message renders as a U-loop (2 Lines + 1 Arrow)
    /// on the sending actor's lifeline.
    #[test]
    fn sequence_self_message_renders() {
        let body = r#"{
          "type": "sequence",
          "actors": [
            {"id": "svc", "label": "Service", "tag": "SVC"}
          ],
          "messages": [
            {"from": "svc", "to": "svc", "label": "tick"}
          ]
        }"#;
        let (d, _) = parse(body).unwrap();
        let ctx = LayoutContext::new(600.0, 400.0);
        let layout = d.layout(&ctx);

        // One lifeline (dashed), two U-loop Lines (solid), one arrow.
        let dashed = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Line {
                        style: LineStyle::Dashed,
                        ..
                    }
                )
            })
            .count();
        let solid_lines = layout
            .primitives
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    Primitive::Line {
                        style: LineStyle::Solid,
                        ..
                    }
                )
            })
            .count();
        let arrows = layout
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Arrow { .. }))
            .count();
        assert_eq!(dashed, 1, "one lifeline");
        assert_eq!(solid_lines, 2, "self-loop has two solid stub segments");
        assert_eq!(arrows, 1, "self-loop terminates in one arrow");

        // The arrow must land back on the single lifeline — its `to.x`
        // equals the actor's column centre.
        let actor_x = layout
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect { x, w, h, .. }
                    if (*w - ACTOR_WIDTH).abs() < 0.01 && (*h - ACTOR_HEIGHT).abs() < 0.01 =>
                {
                    Some(*x + ACTOR_WIDTH / 2.0)
                }
                _ => None,
            })
            .expect("actor rect present");
        let arrow = layout
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Arrow { from, to, .. } => Some((*from, *to)),
                _ => None,
            })
            .unwrap();
        assert!(
            (arrow.1.x - actor_x).abs() < 0.5,
            "self-loop arrow must land on the lifeline: arrow.to.x={} lifeline.x={}",
            arrow.1.x,
            actor_x
        );
        assert!(
            arrow.0.x > arrow.1.x,
            "self-loop arrow points leftward: from={:?} to={:?}",
            arrow.0,
            arrow.1
        );
    }

    #[test]
    fn density_warning_above_cap() {
        // 13 messages trips SOFT_CAP (12).
        let mut msgs = String::new();
        for i in 0..13 {
            if i > 0 {
                msgs.push(',');
            }
            msgs.push_str(r#"{"from":"a","to":"b","label":"m"}"#);
        }
        let body = format!(
            r#"{{"type":"sequence","actors":[
                {{"id":"a","label":"A"}},{{"id":"b","label":"B"}}
            ],"messages":[{msgs}]}}"#
        );
        let (_d, warnings) = parse(&body).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], Warning::DensityHigh { .. }));
    }

    #[test]
    fn empty_actors_produce_empty_layout() {
        let body = r#"{"type":"sequence","actors":[]}"#;
        let (d, _) = parse(body).unwrap();
        let ctx = LayoutContext::new(400.0, 400.0);
        let layout = d.layout(&ctx);
        assert!(layout.primitives.is_empty());
    }

    #[test]
    fn number_field_parses_without_rendering() {
        // Both top-level `"number":"auto"` and per-message `"number":"1"`
        // round-trip through serde and don't crash the layout.
        let body = r#"{
          "type": "sequence",
          "number": "auto",
          "actors": [
            {"id":"a","label":"A"},{"id":"b","label":"B"}
          ],
          "messages": [
            {"from":"a","to":"b","label":"m","number":"1"}
          ]
        }"#;
        let (d, _) = parse(body).unwrap();
        let ctx = LayoutContext::new(800.0, 400.0);
        let layout = d.layout(&ctx);
        assert!(layout.primitive_count() > 0);
    }
}
