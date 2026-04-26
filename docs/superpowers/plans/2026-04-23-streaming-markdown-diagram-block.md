# Streaming Markdown Diagram Block Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make streamed ```` ```diagram ```` fences render through `makepad-diagram-kit::DiagramView` as reliably as existing Mermaid/Splash blocks, including mid-stream partial JSON.

**Architecture:** Keep `streaming-markdown-kit` responsible only for safe text shaping (`sanitize` + `remend` + cursor placement). Keep Makepad widget routing in `../robius/makepad/widgets/src/markdown.rs`, where `diagram_block` already mirrors `splash_block`. Do not move diagram parsing into `streaming-markdown-kit`; the fence body remains opaque markdown payload until the Makepad widget forwards it to `DiagramView`.

**Tech Stack:** Rust 2024, `streaming-markdown-kit`, Makepad 2.0 `Markdown` widget, `makepad-diagram-kit`, `pulldown-cmark`, `cargo test`, `cargo check`.

---

## File Map

- `../streaming-markdown-kit/src/remend.rs`: add `diagram` to the bypass-language list so JSON payloads are opaque to inline markdown repair rules.
- `../streaming-markdown-kit/tests/remend.rs`: add regression coverage that diagram fence content is not modified and only the fence closer is synthesized.
- `../streaming-markdown-kit/src/lib.rs`: fix cursor placement when `remend` synthesizes a trailing fence closer; the cursor must not turn the closer into ```` ```▋ ````.
- `../robius/makepad/widgets/src/markdown.rs`: verify existing `diagram_block` forwarding behavior; only modify if tests reveal a gap.
- `../robius/makepad/examples/aichat/src/main.rs`: verify the DSL keeps `diagram_block` with inner id `diagram_view`; only modify if the template drifts.

## Task 1: Treat `diagram` Fences As Opaque In `remend`

**Files:**
- Modify: `../streaming-markdown-kit/src/remend.rs`
- Modify: `../streaming-markdown-kit/tests/remend.rs`

- [ ] **Step 1: Write the failing remend regression**

Add this test next to `test_remend_protects_mermaid_block_content`:

```rust
#[test]
fn test_remend_protects_diagram_block_content() {
    let src = "```diagram\n{\"type\":\"flowchart\",\"nodes\":[{\"id\":\"a\",\"label\":\"**literal**\"}]}";
    let out = remend(src);

    assert!(
        out.ends_with("\n```"),
        "expected fenced-code closer at end, got {:?}",
        out.as_ref()
    );
    assert!(
        out.contains("\"label\":\"**literal**\""),
        "diagram JSON payload should be passed through unchanged"
    );
    assert_eq!(out, format!("{src}\n```"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd ../streaming-markdown-kit
cargo test test_remend_protects_diagram_block_content -- --nocapture
```

Expected: FAIL until `diagram` is in `BYPASS_LANGUAGES`.

- [ ] **Step 3: Add `diagram` to bypass languages**

Change:

```rust
const BYPASS_LANGUAGES: &[&str] = &["mermaid", "math", "tex", "latex", "typst", "asciidoc"];
```

to:

```rust
const BYPASS_LANGUAGES: &[&str] = &[
    "mermaid", "diagram", "math", "tex", "latex", "typst", "asciidoc",
];
```

- [ ] **Step 4: Re-run remend coverage**

Run:

```bash
cargo test test_remend_protects_diagram_block_content -- --nocapture
cargo test test_remend_protects_mermaid_block_content -- --nocapture
```

Expected: PASS.

## Task 2: Keep Streaming Cursor Outside Synthesized Fence Closers

**Files:**
- Modify: `../streaming-markdown-kit/src/lib.rs`

- [ ] **Step 1: Write the failing cursor-placement test**

Add this test in `src/lib.rs`'s existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn remend_streaming_display_keeps_cursor_after_synthesized_fence_closer() {
    let raw = "Before\n```diagram\n{\"type\":\"flowchart\",\"nodes\":[{\"id\":\"a\",\"label\":\"A\"}]}";
    let out = streaming_display_with_latex_autowrap_remend(raw, inline_code_options());

    assert!(
        out.ends_with("\n```\n▋"),
        "cursor must be outside the synthesized fence closer, got {out:?}"
    );
    assert!(
        !out.contains("```▋"),
        "cursor on the closer line prevents CommonMark from closing the fence"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd ../streaming-markdown-kit
cargo test remend_streaming_display_keeps_cursor_after_synthesized_fence_closer -- --nocapture
```

Expected: FAIL if output ends with ```` ```▋ ````.

- [ ] **Step 3: Insert a newline before the cursor when remend synthesized a fence closer**

Implement the smallest helper in `src/lib.rs`:

```rust
fn push_streaming_cursor(out: &mut String) {
    if out.ends_with("\n```") || out.ends_with("\n~~~") {
        out.push('\n');
    }
    out.push(DEFAULT_TAIL);
}
```

Then replace direct cursor appends in `streaming_display_with_latex_autowrap_remend`:

```rust
let mut out = String::with_capacity(closed.len() + DEFAULT_TAIL.len_utf8() + 1);
out.push_str(&closed);
push_streaming_cursor(&mut out);
out
```

Do not change final non-streaming render paths; final text should still be raw text without cursor.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test remend_streaming_display_keeps_cursor_after_synthesized_fence_closer -- --nocapture
cargo test streaming_display_appends_cursor -- --nocapture
cargo test test_remend_protects_diagram_block_content -- --nocapture
```

Expected: PASS.

## Task 3: Verify Makepad Markdown `diagram_block` Wiring

**Files:**
- Inspect: `../robius/makepad/widgets/src/markdown.rs`
- Inspect: `../robius/makepad/examples/aichat/src/main.rs`
- Modify only if the assertions below are false.

- [ ] **Step 1: Verify widget routing source**

Confirm `widgets/src/markdown.rs` has:

```rust
let is_diagram =
    matches!(&kind, CodeBlockKind::Fenced(lang) if lang.as_ref() == "diagram");
```

and that `End(TagEnd::CodeBlock)` forwards `diagram_block_string` to:

```rust
item.widget(cx, ids!(diagram_view)).set_text(cx, dbs);
```

- [ ] **Step 2: Verify app DSL template source**

Confirm `examples/aichat/src/main.rs` defines:

```rust
diagram_block := View {
    width: Fill
    height: Fit
    diagram_view := DiagramView {
        width: Fit
        height: Fit
    }
}
```

The inner id must stay `diagram_view`; changing it breaks the markdown widget dispatch.

- [ ] **Step 3: Run downstream compile check**

Run:

```bash
cd ../robius/makepad
cargo check -p makepad-example-aichat
```

Expected: PASS.

## Task 4: End-To-End Manual Verification Case

**Files:**
- No code changes unless a prior task fails.

- [ ] **Step 1: Run aichat with the proxy environment fixed**

Run:

```bash
cd ../robius/makepad
HTTPS_PROXY= HTTP_PROXY= ALL_PROXY= cargo run -p makepad-example-aichat
```

- [ ] **Step 2: Send the natural-language prompt**

Use:

```text
画一个 sequence diagram：参与者有 User、API Gateway、Database。User 先调用 API Gateway，消息是 POST /login，标成 primary。然后 API Gateway 调用 Database，消息是 SELECT user。Database 返回给 API Gateway，消息是 row。最后 API Gateway 返回给 User，消息是 200 OK，标成 primary。把 API Gateway 设为 focal，User 的 tag 是 CLIENT，API Gateway 的 tag 是 MW，Database 的 tag 是 STORE。
```

Expected:

- The response contains a ```` ```diagram ```` fenced JSON block, not Mermaid.
- The diagram renders while/after streaming as `DiagramView`.
- Left-to-right calls have solid shafts and visible right arrowheads.
- Return messages have dashed shafts and visible left arrowheads.
- No raw JSON block is shown to the user unless parse fails.

## Task 5: Full Regression Pass

**Files:**
- No new files.

- [ ] **Step 1: Run streaming-markdown-kit tests**

Run:

```bash
cd ../streaming-markdown-kit
cargo test
```

Expected: PASS.

- [ ] **Step 2: Run diagram kit tests**

Run:

```bash
cd ../makepad-diagram-kit
cargo test
cargo test --features makepad
```

Expected: PASS.

- [ ] **Step 3: Run downstream compile checks**

Run:

```bash
cd ../robius/makepad
cargo check -p makepad-example-aichat
```

Expected: PASS.

## Notes

- `streaming-markdown-kit` should not depend on `makepad-diagram-kit`; it only needs to preserve and close fenced text correctly.
- `makepad-diagram-kit` should not depend on `streaming-markdown-kit`; it only consumes JSON once `DiagramView::set_text` receives a body.
- Do not add a second diagram parser path in the markdown widget. The single dispatch point is `diagram_block -> diagram_view.set_text`.
