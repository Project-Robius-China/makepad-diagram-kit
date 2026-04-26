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

    // Rounded-rect shader for node boxes. Per-rect `border_radius`,
    // `border_color`, `border_size` are `#[live]` instance fields on the
    // `DrawRoundedRect` struct below — set them from Rust before each
    // `draw_abs` and the SDF honours them per instance.
    set_type_default() do #(DrawRoundedRect::script_shader(vm)){
        ..mod.draw.DrawQuad

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let inset = self.border_size
            let bw = max(1.0 self.rect_size.x - inset * 2.0)
            let bh = max(1.0 self.rect_size.y - inset * 2.0)
            let requested_r = max(0.0 self.border_radius)
            // `sdf.box()` breaks when `r` approaches half the size, so
            // pill-like nodes use a direct capsule distance field rather than
            // relying on boolean ops between multiple SDF primitives.
            let max_r = max(0.0 min(bw bh) * 0.5 - 0.5)
            if bw >= bh && requested_r >= max_r {
                let px = self.pos.x * self.rect_size.x - inset
                let py = self.pos.y * self.rect_size.y - inset
                let pill_r = bh * 0.5
                let cap_x = clamp(px pill_r max(pill_r bw - pill_r))
                let cap_y = pill_r
                let d = length(vec2(px - cap_x py - cap_y)) - pill_r
                let aa = sdf.aa
                let fill_a = clamp(0.5 - d / aa 0.0 1.0)
                let stroke_a = if self.border_size > 0.0 {
                    clamp(0.5 - (abs(d) - self.border_size * 0.5) / aa 0.0 1.0)
                } else {
                    0.0
                }
                let fill = vec4(
                    self.color.rgb * self.color.a * fill_a
                    self.color.a * fill_a
                )
                let stroke = vec4(
                    self.border_color.rgb * self.border_color.a * stroke_a
                    self.border_color.a * stroke_a
                )
                return stroke + fill * (1.0 - stroke.w)
            } else {
                sdf.box(
                    inset
                    inset
                    bw
                    bh
                    min(requested_r max_r)
                )
            }
            sdf.fill_keep(self.color)
            if self.border_size > 0.0 {
                sdf.stroke(self.border_color self.border_size)
            }
            return sdf.result
        }
    }

    // Dot-pattern background shader. A full-canvas quad that steps a grid
    // and draws a small dot at each cell corner. `tile`, `radius`, and the
    // dot `color` are uniforms — the pattern is identical across the
    // widget so per-instance variation isn't required.
    set_type_default() do #(DrawDotPattern::script_shader(vm)){
        ..mod.draw.DrawQuad

        pixel: fn() {
            let p = self.pos * self.rect_size
            let tile = max(2.0 self.tile)
            // Compute per-cell coordinates via fract — avoids any
            // collision with the `mod` DSL keyword used for imports.
            let cell = vec2(fract(p.x / tile) fract(p.y / tile)) * tile
            let d = length(cell - vec2(self.radius self.radius))
            let a = smoothstep(self.radius self.radius - 1.0 d)
            return vec4(self.color.rgb * self.color.a * a self.color.a * a)
        }
    }

    // Directional arrowhead shader. `direction` is encoded as:
    // 0 = right, 1 = left, 2 = down, 3 = up. The quad itself is an
    // axis-aligned 9x9-ish box; the shader cuts a true triangle inside it.
    set_type_default() do #(DrawArrowHead::script_shader(vm)){
        ..mod.draw.DrawQuad

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let w = self.rect_size.x
            let h = self.rect_size.y
            let d = self.direction
            if d < 0.5 {
                sdf.move_to(w, h * 0.5)
                sdf.line_to(0.0, h)
                sdf.line_to(0.0, 0.0)
            } else if d < 1.5 {
                sdf.move_to(0.0, h * 0.5)
                sdf.line_to(w, 0.0)
                sdf.line_to(w, h)
            } else if d < 2.5 {
                sdf.move_to(w * 0.5, h)
                sdf.line_to(0.0, 0.0)
                sdf.line_to(w, 0.0)
            } else {
                sdf.move_to(w * 0.5, 0.0)
                sdf.line_to(w, h)
                sdf.line_to(0.0, h)
            }
            sdf.close_path()
            sdf.fill(self.color)
            return sdf.result
        }
    }

    // Circle shader for true dots and Venn sets. Like DrawRoundedRect,
    // all fields are per-instance so each circle can carry its own radius,
    // fill, stroke, and alpha.
    set_type_default() do #(DrawCircle::script_shader(vm)){
        ..mod.draw.DrawQuad

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let max_r = max(0.0 min(self.rect_size.x self.rect_size.y) * 0.5 - self.border_size * 0.5 - 0.5)
            let radius = min(max(0.0 self.radius) max_r)
            sdf.circle(self.rect_size.x * 0.5 self.rect_size.y * 0.5 radius)
            sdf.fill_keep(self.color)
            if self.border_size > 0.0 {
                sdf.stroke(self.border_color self.border_size)
            }
            return sdf.result
        }
    }

    mod.widgets.DiagramViewBase = #(DiagramView::register_widget(vm))

    mod.widgets.DiagramView = set_type_default() do mod.widgets.DiagramViewBase{
        width: Fill
        height: Fit
        min_height: 220.0
        draw_rect +: {
            color: #faf7f2
        }
        draw_rounded +: {
            color: #faf7f2
            border_color: #1c1917
            border_size: 1.0
            border_radius: 12.0
        }
        draw_dot_pattern +: {
            color: #1c1917
            tile: 22.0
            radius: 1.0
        }
        draw_arrow_head +: {
            color: #1c1917
            direction: 0.0
        }
        draw_circle +: {
            color: #0000
            border_color: #1c1917
            border_size: 1.0
            radius: 4.0
        }
        draw_text +: {
            color: #1c1917
            text_style: theme.font_regular{
                font_size: 11.0
                line_spacing: 1.2
            }
        }
    }
}

/// Custom DrawQuad subclass that renders a filled rounded rectangle with an
/// optional border. All four fields are `#[live]` instance attributes so the
/// caller can vary radius/border per-rect without starting a new draw call.
#[derive(Script, ScriptHook, Debug)]
#[repr(C)]
pub struct DrawRoundedRect {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4f,
    #[live]
    pub border_color: Vec4f,
    #[live]
    pub border_size: f32,
    #[live]
    pub border_radius: f32,
}

/// Custom DrawQuad subclass that paints a dot-grid pattern across its
/// rectangle. Used once per DiagramView to give the canvas the editorial
/// "graph paper" feel. All three fields are `#[live]` so the app DSL can
/// tweak the density / dot color without recompiling.
#[derive(Script, ScriptHook, Debug)]
#[repr(C)]
pub struct DrawDotPattern {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4f,
    #[live]
    pub tile: f32,
    #[live]
    pub radius: f32,
}

/// Custom DrawQuad subclass that paints a single filled triangular arrowhead.
/// The renderer supplies a tight axis-aligned rect plus a cardinal direction.
#[derive(Script, ScriptHook, Debug)]
#[repr(C)]
pub struct DrawArrowHead {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4f,
    #[live]
    pub direction: f32,
}

/// Custom DrawQuad subclass that paints a filled/stroked circle.
#[derive(Script, ScriptHook, Debug)]
#[repr(C)]
pub struct DrawCircle {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4f,
    #[live]
    pub border_color: Vec4f,
    #[live]
    pub border_size: f32,
    #[live]
    pub radius: f32,
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
    /// Rounded-rect pass — uses per-instance `color` / `border_color` /
    /// `border_size` / `border_radius` thanks to the `instance` keyword
    /// in the DrawRoundedRect shader DSL, so node (radius 6) and tag
    /// (radius 2) rects can share a single draw batch.
    #[live]
    pub draw_rounded: DrawRoundedRect,
    /// Full-canvas dot pattern painted once before the primitives.
    #[live]
    pub draw_dot_pattern: DrawDotPattern,
    /// True triangular arrowhead pass. Shafts are still drawn as line rects.
    #[live]
    pub draw_arrow_head: DrawArrowHead,
    /// True circle pass for dots, timeline markers, and Venn sets.
    #[live]
    pub draw_circle: DrawCircle,
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
    // body or available width changes.
    #[rust]
    current_layout: Option<DiagramLayout>,
    #[rust]
    error_message: Option<String>,
    #[rust]
    layout_input_width: Option<f32>,
    #[rust]
    layout_draw_width: f32,
    #[rust]
    theme: Theme,
}

impl DiagramView {
    const DEFAULT_CANVAS_W: f32 = 500.0;

    fn resolve_canvas_width(parent_width: f64) -> f32 {
        if parent_width.is_finite() && parent_width > 0.0 {
            (parent_width as f32).max(Self::DEFAULT_CANVAS_W)
        } else {
            Self::DEFAULT_CANVAS_W
        }
    }

    /// Parse + lay out the current body. Populates either `current_layout`
    /// or `error_message`. Called when the body or available width changes.
    fn recompute(&mut self, canvas_width: f32) {
        self.current_layout = None;
        self.error_message = None;
        self.layout_input_width = Some(canvas_width);
        self.layout_draw_width = canvas_width;

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
                self.error_message = Some("could not parse diagram (unclosed JSON?)".to_string());
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
        self.layout_draw_width = (out.bounds.x + out.bounds.w + 16.0).ceil().max(canvas_w);
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
        let parent_rect = cx.peek_walk_turtle(walk);
        let canvas_w = Self::resolve_canvas_width(parent_rect.size.x);
        let width_changed = self
            .layout_input_width
            .is_none_or(|w| (w - canvas_w).abs() > 0.5);

        if (self.current_layout.is_none() && self.error_message.is_none()) || width_changed {
            self.recompute(canvas_w);
        }

        // Decide the final walk height based on the layout's bounds.
        let content_h = if let Some(l) = &self.current_layout {
            (l.bounds.y + l.bounds.h).max(self.min_height as f32) as f64
        } else {
            self.min_height
        };
        walk.width = Size::Fixed(self.layout_draw_width as f64);
        walk.height = Size::Fixed(content_h.max(self.min_height));

        // Error path: draw an error banner and return.
        if let Some(msg) = self.error_message.clone() {
            walk.height = Size::Fixed(32.0);
            self.draw_error(cx, walk, &msg);
            return DrawStep::done();
        }

        // Happy path: walk the turtle, paint the paper + dot pattern, then
        // draw every primitive at the resolved origin.
        let bounds_rect = cx.walk_turtle(walk);

        // Paper fill covering the widget's walk area.
        self.draw_rect.color = color_to_vec4(self.theme.palette.paper);
        self.draw_rect.draw_abs(cx, bounds_rect);

        // Dot-pattern overlay at ~10% opacity of `ink` on `paper`. The
        // shader reads `self.color` as straight RGBA so we inject the
        // ink colour and let its alpha set the pattern opacity.
        let ink = self.theme.palette.ink;
        let dot_color = vec4(
            ink.r as f32 / 255.0,
            ink.g as f32 / 255.0,
            ink.b as f32 / 255.0,
            0.10,
        );
        self.draw_dot_pattern.color = dot_color;
        self.draw_dot_pattern.draw_abs(cx, bounds_rect);

        if let Some(l) = self.current_layout.take() {
            render_primitives(
                cx,
                &mut self.draw_rect,
                &mut self.draw_rounded,
                &mut self.draw_arrow_head,
                &mut self.draw_circle,
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
            self.layout_input_width = None;
            self.layout_draw_width = Self::DEFAULT_CANVAS_W;
            self.redraw(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiagramView;

    #[test]
    fn diagram_view_uses_parent_width_when_available() {
        assert_eq!(DiagramView::resolve_canvas_width(920.0), 920.0);
    }
}
