//! `DiagramView` — Makepad widget that parses a JSON diagram body, runs the
//! kit's layout engine, and paints the resulting primitives inline.
//!
//! Consumers drop it into their script DSL next to a `MermaidSvgView` /
//! `Splash` peer — the markdown widget's code-fence hook is responsible for
//! discovering the template and pushing the raw body via `set_text`.
//!
//! Feature-gated (`makepad`). Pure-parsing consumers (CLI validators,
//! alternative renderers) don't pull this in.

use crate::{
    DiagramLayout, DiagramLimits, LayoutContext, ParseError, Theme, layout, makepad_render::*,
    parse, parse_lossy, types::Diagram,
};
use makepad_widgets::makepad_derive_widget::*;
use makepad_widgets::makepad_draw::*;
use makepad_widgets::widget::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.draw
    use mod.text.*

    mod.widgets.DiagramViewBase = #(DiagramView::register_widget(vm))

    mod.widgets.DiagramView = set_type_default() do mod.widgets.DiagramViewBase{
        width: Fill
        height: Fit
        padding: Inset{left: 8 right: 8 top: 8 bottom: 8}
        min_height: 220.0
        draw_rect +: {
            color: #faf7f2
        }
        draw_text +: {
            color: #1c1917
            text_style: TextStyle{
                font_size: 11.0
                line_spacing: 1.2
            }
        }
    }
}

/// Inline diagram widget. Accepts a JSON body through `set_text`, parses it,
/// lays it out, and paints the primitives.
#[derive(Script, ScriptHook, Widget)]
pub struct DiagramView {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[redraw]
    #[live]
    pub draw_rect: DrawColor,
    #[live]
    pub draw_text: DrawText,
    #[walk]
    pub walk: Walk,

    /// Minimum drawn height so an empty layout still reserves space in the
    /// markdown flow. Override via the DSL.
    #[live(220.0)]
    pub min_height: f64,

    #[live]
    body: ArcStringMut,

    // Cached parse/layout output. Recomputed only on `set_text` when the
    // body actually changes.
    #[rust]
    current_layout: Option<DiagramLayout>,
    #[rust]
    error_message: Option<String>,
    #[rust]
    theme: Theme,
}

impl DiagramView {
    /// Parse + lay out the current body. Populates either `current_layout`
    /// or `error_message`. Called once per `set_text`.
    fn recompute(&mut self, canvas_width: f32) {
        self.current_layout = None;
        self.error_message = None;

        let body = self.body.as_ref().trim();
        if body.is_empty() {
            return;
        }

        if body.len() > DiagramLimits::MAX_BODY_BYTES {
            self.error_message = Some(format!(
                "diagram too large ({} KB cap)",
                DiagramLimits::MAX_BODY_BYTES / 1024
            ));
            return;
        }

        // Try strict parse first — preserves all validation + warnings.
        let diagram_opt: Option<Diagram> = match parse(body) {
            Ok((d, _warnings)) => Some(d),
            Err(ParseError::Malformed { .. }) => {
                // Fall back to the streaming-prefix path so mid-stream LLM
                // output still renders something rather than flashing an
                // error on every token. `parse_lossy` already returns
                // `Option<Diagram>`.
                parse_lossy(body)
            }
            Err(e) => {
                self.error_message = Some(format!("{e}"));
                None
            }
        };

        let Some(diagram) = diagram_opt else {
            if self.error_message.is_none() {
                self.error_message = Some(
                    "could not parse diagram (unclosed JSON?)".to_string(),
                );
            }
            return;
        };

        // Editorial canvases use a 2:1 aspect. Height is driven by the
        // layout engine's natural bounds; we only constrain width to the
        // widget's walk width so the diagram fits inline.
        let canvas_w = canvas_width.max(200.0);
        let canvas_h = (canvas_w * 0.5).max(self.min_height as f32);
        let ctx = LayoutContext::new(canvas_w, canvas_h).with_theme(self.theme);
        let out = layout(&diagram, &ctx);
        self.current_layout = Some(out);
    }

    /// Draw a small error banner in place of a diagram — avoids panicking on
    /// malformed input and gives the user a signal their fence was rejected.
    fn draw_error(&mut self, cx: &mut Cx2d, walk: Walk, msg: &str) {
        let turtle_rect = cx.walk_turtle(walk);
        // Light-tinted warning background.
        self.draw_rect.color = vec4(0.95, 0.88, 0.85, 1.0);
        self.draw_rect.draw_abs(cx, turtle_rect);
        // Ink color for the message text.
        self.draw_text.color = vec4(0.1, 0.07, 0.06, 1.0);
        self.draw_text.text_style.font_size = 11.0;
        let first_line = msg.lines().next().unwrap_or(msg);
        let text = format!("⚠ diagram error: {first_line}");
        self.draw_text.draw_abs(
            cx,
            dvec2(turtle_rect.pos.x + 8.0, turtle_rect.pos.y + 16.0),
            &text,
        );
    }
}

impl Widget for DiagramView {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, mut walk: Walk) -> DrawStep {
        // v1 uses a fixed editorial canvas (500 × 250 lpx) chosen to read
        // well in a chat-message flow. Parent flows wider than 500 lpx
        // leave whitespace to the right; narrower flows clip the diagram
        // — acceptable for v1 since the target aichat message widths sit
        // in the 600-800 lpx range.
        const CANVAS_W: f32 = 500.0;
        const CANVAS_H: f32 = 250.0;

        if self.current_layout.is_none() && self.error_message.is_none() {
            self.recompute(CANVAS_W);
        }

        // Decide the final walk height based on the layout's bounds.
        let content_h = if let Some(l) = &self.current_layout {
            (l.bounds.y + l.bounds.h).max(CANVAS_H) as f64
        } else {
            self.min_height
        };
        walk.width = Size::Fixed(CANVAS_W as f64);
        walk.height = Size::Fixed(content_h.max(self.min_height));

        // Error path: draw an error banner and return.
        if let Some(msg) = self.error_message.clone() {
            walk.height = Size::Fixed(32.0);
            self.draw_error(cx, walk, &msg);
            return DrawStep::done();
        }

        // Happy path: walk the turtle, paint the paper, then draw every
        // primitive at the resolved origin.
        let bounds_rect = cx.walk_turtle(walk);

        // Paper fill covering the widget's walk area.
        self.draw_rect.color = color_to_vec4(self.theme.palette.paper);
        self.draw_rect.draw_abs(cx, bounds_rect);

        if let Some(l) = self.current_layout.take() {
            render_primitives(
                cx,
                &mut self.draw_rect,
                &mut self.draw_text,
                bounds_rect.pos,
                &l.primitives,
            );
            // Restore the cached layout so subsequent redraws without a
            // body change (e.g., scroll redraw) don't re-run parse.
            self.current_layout = Some(l);
        }

        DrawStep::done()
    }

    fn text(&self) -> String {
        self.body.as_ref().to_string()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        if self.body.as_ref() != v {
            self.body.set(v);
            // Invalidate cached layout; it'll be rebuilt on next draw_walk.
            self.current_layout = None;
            self.error_message = None;
            self.redraw(cx);
        }
    }
}
