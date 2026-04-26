spec: task
name: "M-diagram-v1 — editorial-diagram library for Makepad streaming UIs"
tags: [diagram, makepad, streaming, editorial, json-spec]
estimate: 4d
---

## Intent

Build `makepad-diagram-kit`, a Rust library that consumes a compact JSON
"diagram spec" and produces pixel-perfect vector drawing commands through
renderer-agnostic primitives plus a feature-gated Makepad 2.0 widget
binding. The v1 contract covers 13 diagram types inspired by the editorial
aesthetic of cathrynlavery/diagram-design:

1. **Pyramid / funnel** — ranked layers
2. **Quadrant** — 2-axis positioning scatter
3. **Tree** — parent → children hierarchy
4. **Layers** — stacked abstractions (technology stack, architecture layers)
5. **Flowchart** — decision logic with vertical flow
6. **Architecture** — 2D layered system diagram with role-tagged nodes
7. **Sequence** — actor lifelines with top-to-bottom messages
8. **State** — state machines with start/end dots and transitions
9. **ER** — entity relationship / data-model boxes
10. **Timeline** — milestone timelines with date-aware spacing
11. **Swimlane** — cross-functional process lanes and handoffs
12. **Nested** — containment rings for scope hierarchy
13. **Venn** — 2/3-set overlap diagrams

The design rules echo the upstream skill: 4/10 density target, one
accent color, confident restraint, removal-driven composition.

Downstream consumer target: the existing streaming-markdown-kit /
Makepad Markdown widget — diagrams land via a ```` ```diagram ```` code
fence whose JSON payload is streamed from an LLM. The widget dispatches
through a new `diagram_block` template, mirroring the existing
`mermaid_block` hook pattern.

## Decisions

- **Input format**: JSON. Validates via `serde::Deserialize`; reject unknown top-level `type` values with `ParseError::UnknownType`. Each diagram type has its own body schema (e.g., `{"type": "pyramid", "levels": [...]}`, `{"type": "quadrant", "axes": {...}, "points": [...]}`).
- **Output**: a renderer-agnostic `DiagramLayout` struct containing positioned primitives (boxes, circles, text, lines, arrows, polygons). The `makepad` feature gate exposes `DiagramView` plus `makepad_render` helpers that walk the same layout and issue Makepad draw calls. Consumers targeting other backends can walk the same struct.
- **Rendering path**: Makepad-native `DrawColor`, `DrawText`, and feature-gated custom SDF shaders for rounded node boxes / canvas background. NO intermediate SVG. NO usvg / resvg. Same zero-new-deps discipline as streaming-markdown-kit's M-img-3.
- **Layout engines**: hand-written per type for v1. `architecture` uses a deterministic layered BFS layout rather than Dagre; later phases may adopt Dagre only if orthogonal routing / ports / clusters become required.
- **Streaming**: each diagram type's layout engine MUST be idempotent under prefix input. Given a partial JSON body that is a prefix of some valid body, the engine returns a partial-but-renderable `DiagramLayout` (e.g., pyramid with the first 2 of 5 levels rendered). On every re-render with a longer prefix, output grows monotonically — no repositioning of already-rendered primitives unless the diagram type requires it (e.g., pyramid re-centers when a new bottom layer arrives; this is OK).
- **Style tokens**: default to the upstream style guide's semantic palette — paper, ink, accent, muted, rule, link, accent_tint. Tokens are a `Theme` struct; consumers can swap. Dark variant ships at parity.
- **Accent policy**: `accent_idx` remains opt-in for `pyramid`, `quadrant`, `tree`, `layers`, and `flowchart`. `architecture`, `sequence`, `state`, `er`, `swimlane`, `nested`, and `venn` are role-driven and do NOT expose `accent_idx`; focal treatment comes from `role`. `timeline` uses `role:"major"` for the single emphasized milestone.
- **Error path**: malformed JSON → `ParseError`; consumer renders an inline placeholder "⚠ diagram error" with the first-line summary, rather than panicking. Parity with M-img-* error semantics.
- **Size caps**: max 200 KB JSON body (gate before parse). Max 30 nodes per diagram (gate after parse). Both enforced via `DiagramLimits` const.
- **Density lint**: parsing emits a `Warning::DensityHigh` when a diagram type's main element count exceeds its type-specific soft cap (pyramid > 7 levels, tree > 20 nodes, flowchart > 15 nodes, quadrant > 20 points, layers > 10 layers, architecture > 12 nodes, sequence > 12 messages, state > 12 states, er > 10 entities, timeline > 14 events, swimlane > 14 steps, nested > 5 levels, venn > 3 sets). Warnings do NOT block render — they surface up the call stack for telemetry / UI hints.
- **Streaming buffer parse**: reuse streaming-markdown-kit's `sanitizer` semantics — if consumers are running that stream path, an unclosed JSON body at end-of-stream is trimmed to the last well-formed prefix. Kit consumers get this for free; direct callers can use our `parse_lossy` for the same behavior.

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
- Tree-with-collapsible — v2 or later
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

Scenario: Architecture parses role-tagged nodes into a 2D layered layout
  Test: architecture_basic_parse
  Level: unit
  Given the README architecture JSON with 5 nodes and 4 edges
  When parsed and laid out to a 1000 x 500 canvas
  Then 5 node rects are emitted
  And 4 arrows are emitted
  And node boxes occupy at least 2 distinct y-rows

Scenario: Architecture role fills and edge colors follow semantic palette rules
  Test: architecture_role_fill_mapping
  Level: unit
  Given an architecture diagram with `focal`, `backend`, `store`, and `external` node roles
  When laid out under `Theme::light()`
  Then exactly one focal node uses accent stroke plus accent_tint fill
  And backend nodes use paper fill with ink stroke
  And store nodes use leaf_tint fill with muted stroke

Scenario: Architecture edge labels mirror edge role colors
  Test: architecture_edge_role_color
  Level: unit
  Given an architecture diagram with `external`, `primary`, and `default` edges
  When laid out under `Theme::light()`
  Then external arrows and labels use `link`
  And primary arrows and labels use `accent`
  And default arrows use `muted`

Scenario: Sequence parses actor boxes lifelines and message arrows
  Test: sequence_basic_parse
  Level: unit
  Given a sequence diagram with 3 actors and 4 messages
  When parsed and laid out to a 1000 x 500 canvas
  Then 3 actor rects are emitted
  And 3 dashed lifelines are emitted
  And 4 message arrows are emitted

Scenario: Sequence return messages render with dashed shafts
  Test: sequence_right_to_left_messages_render_as_dashed_returns
  Level: unit
  Given a sequence diagram with left-to-right calls and right-to-left responses
  When laid out
  Then left-to-right calls use solid arrows
  And right-to-left responses use dashed arrows
  And an explicit `kind: "return"` message uses dashed arrows even when it points left-to-right

Scenario: Sequence focal actor and message roles follow semantic colors
  Test: sequence_role_colors
  Level: unit
  Given a sequence diagram whose focal actor is tagged `role: "focal"` and whose messages use `primary` and `default` roles
  When laid out under `Theme::light()`
  Then exactly one actor box uses accent stroke
  And primary messages use `accent`
  And default messages use `muted`

Scenario: Sequence self-message renders as a right-side U-loop
  Test: sequence_self_message_renders
  Level: unit
  Given a sequence diagram with one actor sending a message to itself
  When laid out
  Then the self-message emits 2 solid line segments and 1 arrow
  And the arrow lands back on the actor's lifeline

Scenario: State diagram parses and renders labeled transitions
  Test: state_diagram_parses_and_renders_transitions
  Level: integration
  Targets: tests/expanded_types_dispatch.rs
  Given a state diagram with `start`, normal, and `end` states
  When parsed and laid out
  Then it dispatches to `Diagram::State`
  And transition arrows are emitted
  And transition labels are rendered

Scenario: ER diagram renders fields and relationship cardinalities
  Test: er_diagram_parses_and_renders_entities_fields_and_cardinality
  Level: integration
  Targets: tests/expanded_types_dispatch.rs
  Given an ER diagram with two entities, PK fields, FK fields, and one relationship
  When parsed and laid out
  Then it dispatches to `Diagram::Er`
  And PK fields are prefixed with `#`
  And FK fields are prefixed with `->`
  And cardinality labels are emitted near relationship endpoints

Scenario: Timeline renders an axis with event markers
  Test: timeline_diagram_parses_and_renders_axis_markers
  Level: integration
  Targets: tests/expanded_types_dispatch.rs
  Given a timeline with three ISO-date events and one `major` milestone
  When parsed and laid out
  Then it dispatches to `Diagram::Timeline`
  And an axis line is emitted
  And event markers are true circle primitives
  And the major milestone label is rendered

Scenario: Swimlane renders lanes and handoff arrows
  Test: swimlane_diagram_parses_and_renders_lane_handoffs
  Level: integration
  Targets: tests/expanded_types_dispatch.rs
  Given a swimlane diagram with two lanes, three steps, and a handoff edge
  When parsed and laid out
  Then it dispatches to `Diagram::Swimlane`
  And lane labels are rendered
  And step-to-step arrows are emitted

Scenario: Nested diagram renders containment rings
  Test: nested_diagram_parses_and_renders_containment_rings
  Level: integration
  Targets: tests/expanded_types_dispatch.rs
  Given a nested containment diagram with three levels
  When parsed and laid out
  Then it dispatches to `Diagram::Nested`
  And at least three containment rects are emitted
  And level labels render as uppercase eyebrow labels

Scenario: Venn diagram renders overlapping sets as true circles
  Test: venn_diagram_parses_and_renders_overlapping_circles
  Level: integration
  Targets: tests/expanded_types_dispatch.rs
  Given a three-set Venn diagram with one focal intersection
  When parsed and laid out
  Then it dispatches to `Diagram::Venn`
  And exactly three set circles are emitted
  And the focal intersection label is rendered

Scenario: Makepad renderer preserves Venn set circles
  Test: makepad_render_contract_venn_reaches_renderer_as_true_circles
  Level: integration
  Targets: tests/makepad_render_contract.rs
  Given a Venn diagram under the `makepad` feature
  When parsed and laid out
  Then each set reaches the renderer as a `Primitive::Circle`
  And no rounded-rect approximation is used for set geometry

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
