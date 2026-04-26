# M-Diagram Next Phases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the next practical phase of `makepad-diagram-kit` after v1 contract alignment: ship the expanded diagram set end-to-end, strengthen regression coverage, and prepare downstream integration.

**Architecture:** Keep the crate renderer-agnostic at the core and continue treating the Makepad binding as a feature-gated consumer of `DiagramLayout`. Avoid broad refactors; wire `sequence` through the existing `Diagram` dispatch path and add coverage around the specific renderer behaviors that already proved fragile.

**Tech Stack:** Rust, serde, Makepad 2.0 (`makepad-widgets` feature), existing unit tests and corpus fixtures.

---

### Task 1: Wire `sequence` Into the Public Diagram Surface

**Files:**
- Modify: `src/types/mod.rs`
- Modify: `src/lib.rs`
- Modify: `README.md`
- Modify: `examples/gallery.rs`
- Test: `src/types/sequence.rs`

- [x] **Step 1: Write the failing integration test**

Add a parse-level assertion near the existing top-level dispatch coverage that expects:

```rust
let body = r#"{"type":"sequence","actors":[{"id":"a","label":"A"}],"messages":[]}"#;
let (diagram, _) = parse(body).unwrap();
assert!(matches!(diagram, Diagram::Sequence(_)));
```

- [x] **Step 2: Run the targeted test to verify it fails**

Run: `cargo test sequence_dispatch_from_parse -- --nocapture`
Expected: compile fails because `Diagram::Sequence` is not wired yet

- [x] **Step 3: Add the minimal public wiring**

Wire `sequence` through:

- `src/types/mod.rs` module declaration + `pub use`
- `Diagram` enum
- `Diagram::type_tag`
- `Diagram::element_count`
- `Diagram::warnings`
- `Diagram::layout`
- `src/lib.rs` re-exports

- [x] **Step 4: Update user-facing discovery**

Add `sequence` to:

- README supported types table
- `examples/gallery.rs` demo list

- [x] **Step 5: Run focused verification**

Run:

Run: `cargo test sequence -- --nocapture`

Expected: PASS

### Task 2: Add Sequence Fixtures and Corpus Coverage

**Files:**
- Create: `tests/fixtures/sequence/basic.json`
- Modify: `tests/corpus.rs`
- Modify: `README.md`

- [x] **Step 1: Write the failing corpus expectation**

Add a `corpus_sequence` test entry in `tests/corpus.rs` that loads `tests/fixtures/sequence/`.

- [x] **Step 2: Run the corpus selector and verify it fails**

Run: `cargo test corpus_sequence -- --nocapture`
Expected: FAIL because the fixture directory or handler is missing

- [x] **Step 3: Add the minimal fixture set**

Start with one compact fixture:

```json
{
  "type": "sequence",
  "actors": [
    {"id": "user", "label": "User"},
    {"id": "api", "label": "API", "role": "focal"}
  ],
  "messages": [
    {"from": "user", "to": "api", "label": "POST /login", "role": "primary"}
  ]
}
```

- [x] **Step 4: Wire corpus dispatch**

Make `tests/corpus.rs` enumerate and validate `sequence` the same way it already does for `flowchart` and `architecture`.

- [x] **Step 5: Re-run corpus verification**

Run: `cargo test corpus_sequence -- --nocapture`
Expected: PASS

### Task 3: Add Renderer Regression Coverage for Makepad-Specific Shapes

**Files:**
- Create: `tests/makepad_render_contract.rs`
- Modify: `src/widget.rs`
- Modify: `Cargo.toml` only if a test target needs `required-features = ["makepad"]`

- [x] **Step 1: Write the failing regression harness**

Add a Makepad-feature-gated contract test that at minimum compiles and exercises the rounded-rect shader path through a tiny fixture or constructor path.

- [x] **Step 2: Run the renderer contract test to verify the initial gap**

Run: `cargo test --features makepad makepad_render_contract -- --nocapture`
Actual: PASS after adding `tests/makepad_render_contract.rs` with capsule, dashed-return, and public Makepad-surface guards.

- [x] **Step 3: Implement the smallest useful guard**

Guard at least these behaviors:

- capsule path for `oval`
- standard rounded-rect path for `rect`
- `cargo check --features makepad` stays green

- [x] **Step 4: Re-run focused renderer verification**

Run:

```bash
cargo check --features makepad
cargo test --features makepad makepad_render_contract -- --nocapture
```

Expected: PASS

### Task 4: Prepare Downstream Integration Contract

**Files:**
- Modify: `specs/m-diagram-v1.spec.md` only if clarifying references is needed
- Create: `docs/superpowers/plans/2026-04-23-streaming-markdown-diagram-block.md`
- Reference: sibling repo `../streaming-markdown-kit`

- [x] **Step 1: Capture the boundary before coding**

Write a dedicated follow-up plan for `streaming-markdown-kit` integration rather than blending cross-repo work into the current crate.

Actual: created `docs/superpowers/plans/2026-04-23-streaming-markdown-diagram-block.md`.

- [x] **Step 2: List the exact downstream touch points**

Identify:

- code fence detection
- `diagram_block` template path
- partial JSON streaming behavior
- error placeholder rendering

Actual touch points are captured in `docs/superpowers/plans/2026-04-23-streaming-markdown-diagram-block.md`:

- `../streaming-markdown-kit/src/remend.rs`
- `../streaming-markdown-kit/tests/remend.rs`
- `../streaming-markdown-kit/src/lib.rs`
- `../robius/makepad/widgets/src/markdown.rs`
- `../robius/makepad/examples/aichat/src/main.rs`

- [x] **Step 3: Define verification commands**

Include exact selectors for both repos before any implementation starts.

Actual verification commands are captured in the downstream plan:

```bash
cd ../streaming-markdown-kit
cargo test test_remend_protects_diagram_block_content -- --nocapture
cargo test remend_streaming_display_keeps_cursor_after_synthesized_fence_closer -- --nocapture
cargo test streaming_display_appends_cursor -- --nocapture

cd ../robius/makepad
cargo check -p makepad-example-aichat
```

- [x] **Step 4: Stop**

Do not implement cross-repo integration in the same change set as `sequence` wiring.
