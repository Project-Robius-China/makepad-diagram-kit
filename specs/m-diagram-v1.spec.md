spec: task
name: "M-diagram-v1 — editorial-diagram library for Makepad streaming UIs"
tags: [diagram, makepad, streaming, editorial, json-spec]
estimate: 4d
---

## Intent

Build `makepad-diagram-kit`, a Rust library that consumes a compact JSON
"diagram spec" and produces pixel-perfect vector drawing commands through
Makepad 2.0's native `DrawColor` / `DrawText` / `DrawLine` / `DrawSvg`
primitives. The v1 covers 5 diagram types inspired by the editorial
aesthetic of cathrynlavery/diagram-design:

1. **Pyramid / funnel** — ranked layers
2. **Quadrant** — 2-axis positioning scatter
3. **Tree** — parent → children hierarchy
4. **Layers** — stacked abstractions (technology stack, architecture layers)
5. **Flowchart** — decision logic with vertical flow

The design rules echo the upstream skill: 4/10 density target, one
accent color, confident restraint, removal-driven composition.

Downstream consumer target: the existing streaming-markdown-kit /
Makepad Markdown widget — diagrams land via a ```` ```diagram ```` code
fence whose JSON payload is streamed from an LLM. The widget dispatches
through a new `diagram_block` template, mirroring the existing
`mermaid_block` hook pattern.

## Decisions

- **Input format**: JSON. Validates via `serde::Deserialize`; reject unknown top-level `type` values with `ParseError::UnknownType`. Each diagram type has its own body schema (e.g., `{"type": "pyramid", "levels": [...]}`, `{"type": "quadrant", "axes": {...}, "points": [...]}`).
- **Output**: a renderer-agnostic `DiagramLayout` struct containing positioned primitives (boxes, text, lines, arcs). The `makepad` feature gate exposes a `render_to_cx(cx, &layout, walk)` helper that walks the layout and issues Makepad draw calls. Consumers targeting other backends can walk the same struct.
- **Rendering path**: Makepad-native `DrawColor` (rounded boxes, fills), `DrawText` (labels), `DrawLine` (connectors, axes). NO intermediate SVG. NO usvg / resvg. Same zero-new-deps discipline as streaming-markdown-kit's M-img-3.
- **Layout engines**: hand-written per type for v1. Later phases may adopt Dagre (via rusty-mermaid) for complex flow/architecture graphs.
- **Streaming**: each diagram type's layout engine MUST be idempotent under prefix input. Given a partial JSON body that is a prefix of some valid body, the engine returns a partial-but-renderable `DiagramLayout` (e.g., pyramid with the first 2 of 5 levels rendered). On every re-render with a longer prefix, output grows monotonically — no repositioning of already-rendered primitives unless the diagram type requires it (e.g., pyramid re-centers when a new bottom layer arrives; this is OK).
- **Style tokens**: default to the upstream SKILL.md palette — paper `#faf7f2`, ink `#1c1917`, accent `#b5523a`, muted `#78716c`, rule `#e7e5e4`. Tokens are a `Theme` struct; consumers can swap. Dark variant ships at parity.
- **Typography**: two font families, same as streaming-markdown-kit (LXGWWenKai for CJK, Liberation / Noto for Latin). No new font resources; share.
- **Accent policy**: each spec MAY set `accent_idx: Option<usize>` to highlight exactly 1 element per diagram. Enforced at parse: if `accent_idx` points past the diagram's element count, reject with `ParseError::AccentOutOfRange`.
- **Error path**: malformed JSON → `ParseError`; consumer renders an inline placeholder "⚠ diagram error" with the first-line summary, rather than panicking. Parity with M-img-* error semantics.
- **Size caps**: max 200 KB JSON body (gate before parse). Max 30 nodes per diagram (gate after parse). Both enforced via `DiagramLimits` const.
- **Density lint**: parsing emits a `Warning::DensityHigh` when a diagram type's main element count exceeds its type-specific soft cap (pyramid > 7 levels, tree > 20 nodes, flowchart > 15 nodes, quadrant > 20 points, layers > 10 layers). Warnings do NOT block render — they surface up the call stack for telemetry / UI hints.
- **No animation in v1**. `DrawSvg` not needed. Diagrams are static at current state.
- **Streaming buffer parse**: piggyback on streaming-markdown-kit's `sanitizer` — if consumers are running that pipeline, an unclosed JSON body at end-of-stream is trimmed to the last well-formed prefix. Kit consumers get this for free; direct callers can use our `parse_lossy` for the same behavior.

## Boundaries

### Allowed Changes

- `makepad-diagram-kit/**` — all files in this new repo
- `streaming-markdown-kit/specs/markdown-image-rendering.spec.md` — only to add a pointer line referencing this new kit as a sibling

### Forbidden

- Do NOT modify `robius/makepad/` in this task — aichat integration and the `diagram_block` widget template hook are a separate follow-up spec
- Do NOT add any dependency on `usvg`, `resvg`, `image`, or `rusty-mermaid` in v1 — all rendering must go through Makepad-native primitives
- Do NOT ship a DSL parser. JSON only. Mermaid-slop aesthetics are explicitly rejected by the source skill
- Do NOT add animation, tooltip, or click-interaction features in v1

## Out of Scope

- Dark theme switching at runtime (ship dark variant as a separate `Theme::dark()` constant, not a runtime toggle)
- Sequence / State / ER / Architecture / Timeline / Swimlane / Nested / Tree-with-collapsible / Venn — v2 or later
- Export to PNG / SVG file — consumers can screenshot
- Interactive editing
- RTL text layout
- Hand-drawn "sketchy" variant from the source skill (primitive-sketchy.md) — v2
- Annotation / callout primitive from the source skill (primitive-annotation.md) — v2

## Completion Criteria

Scenario: Pyramid parses and produces positioned layers
  Test: test_pyramid_basic_parse
  Level: unit
  Targets: PyramidSpec deserialize + PyramidLayout computation
  Given a JSON body `{"type":"pyramid","levels":[{"label":"Mission"},{"label":"Strategy"},{"label":"Tactics"}]}`
  When parsed
  Then 3 positioned trapezoids are returned
  And the first (top) trapezoid's width is narrower than the bottom
  And each trapezoid's label is centered within it

Scenario: Quadrant maps axis-labeled points to 2D coordinates
  Test: test_quadrant_axis_mapping
  Level: unit
  Targets: QuadrantSpec axis range + point projection
  Given a JSON body with axes `{"x":{"min":0,"max":100},"y":{"min":0,"max":10}}` and points `[{"x":50,"y":5,"label":"mid"},{"x":100,"y":10,"label":"tr"},{"x":0,"y":0,"label":"bl"}]`
  When the quadrant is laid out to a 400x400 canvas
  Then "mid" lands at canvas center (200, 200)
  And "tr" lands at top-right (400, 0) within 1 pixel
  And "bl" lands at bottom-left (0, 400) within 1 pixel

Scenario: Tree lays out parent → children in stratified rows
  Test: test_tree_stratified_layout
  Level: unit
  Targets: TreeSpec hierarchical layout (row-by-depth)
  Given a JSON body `{"type":"tree","root":{"label":"A","children":[{"label":"B"},{"label":"C","children":[{"label":"D"}]}]}}`
  When laid out
  Then nodes A, (B, C), D occupy 3 distinct y-rows in order
  And edges connect A→B, A→C, C→D
  And no two siblings overlap horizontally

Scenario: Layers stacks with equal heights and labels
  Test: test_layers_stack
  Level: unit
  Given a JSON body `{"type":"layers","layers":[{"label":"L3"},{"label":"L2"},{"label":"L1"}]}`
  When laid out to a 400-lpx-tall canvas
  Then each of 3 layers has a height of ~133 lpx
  And the labels appear in order L3 (top), L2, L1 (bottom)
  And adjacent layer borders share a single separator line (not two)

Scenario: Flowchart parses nodes + edges and positions them vertically
  Test: test_flowchart_vertical
  Level: unit
  Given a JSON body `{"type":"flowchart","nodes":[{"id":"start","label":"Start"},{"id":"check","label":"OK?","shape":"diamond"},{"id":"end","label":"End"}],"edges":[{"from":"start","to":"check"},{"from":"check","to":"end","label":"yes"}]}`
  When laid out
  Then node "start" is above node "check" which is above node "end"
  And 2 edges are present with edge "check→end" labeled "yes"
  And diamond shape is rendered for "check"

Scenario: Accent index highlights exactly one element
  Test: test_accent_single_element
  Level: unit
  Given a pyramid with `accent_idx: 1`
  When laid out
  Then exactly one level has `fill: accent`
  And all other levels have `fill: paper`

Scenario: Malformed JSON produces a parse error with line info
  Test: test_parse_error_contains_position
  Level: unit
  Given a JSON body with a trailing comma `{"type":"pyramid","levels":[{"label":"A"},]}`
  When parsed
  Then result is `Err(ParseError::Malformed { line: 1, column: _, message: _ })`

Scenario: Unknown diagram type is rejected
  Test: test_unknown_type_rejected
  Level: unit
  Given a JSON body `{"type":"sunburst","data":[]}`
  When parsed
  Then result is `Err(ParseError::UnknownType("sunburst"))`

Scenario: Oversize JSON body rejected before parse
  Test: test_oversize_body_rejected
  Level: unit
  Given a JSON body larger than 200 KB
  When parsed
  Then result is `Err(ParseError::BodyTooLarge(_))`
  And serde_json::from_str is NOT invoked

Scenario: Prefix input produces partial layout with monotonic growth
  Test: test_pyramid_prefix_layout
  Level: unit
  Targets: streaming idempotency — critical for flicker-free LLM token feed
  Given a valid pyramid spec with 5 levels
  When the JSON body is truncated to after level 2's closing brace (via parse_lossy)
  Then the partial layout contains exactly 2 trapezoids
  And their positions match the first 2 trapezoids of the full 5-level layout, up to proportional re-centering

Scenario: Density warning fires above soft cap
  Test: test_density_warning
  Level: unit
  Given a pyramid with 8 levels (soft cap is 7)
  When parsed
  Then result is `Ok((layout, warnings))` where warnings contains `Warning::DensityHigh`
