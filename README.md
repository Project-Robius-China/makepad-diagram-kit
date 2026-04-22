# makepad-diagram-kit

Streaming-renderable editorial diagrams for Makepad 2.0 apps. JSON-spec input,
renderer-agnostic vector output. Six diagram types inspired by the editorial
aesthetic of
[cathrynlavery/diagram-design](https://github.com/cathrynlavery/diagram-design):

| Type           | Use case                                         |
| -------------- | ------------------------------------------------ |
| `pyramid`      | Ranked hierarchies · `orientation:"down"` funnel |
| `quadrant`     | 2-axis positioning scatter (Impact×Effort)       |
| `tree`         | Parent → children hierarchies · elbow edges      |
| `layers`       | Stacked abstractions (OSI, tech stacks)          |
| `flowchart`    | Vertical decision flow · edge roles              |
| `architecture` | 2D layered system diagram · role-tagged nodes    |

The library is **renderer-agnostic**: `layout()` produces positioned
`Primitive`s (rects, polygons, lines, arrows, text); any backend can paint
them. Enable the `makepad` feature for the bundled `DiagramView` widget with
Sdf-shaded rounded nodes, dot-pattern background, and auto-centered text.

## Quickstart

```rust
use makepad_diagram_kit::{parse, layout, LayoutContext};

let json = r#"{"type":"pyramid","levels":[
    {"label":"Mission"},
    {"label":"Strategy"},
    {"label":"Tactics"}
]}"#;

let (diagram, _warnings) = parse(json).unwrap();
let ctx = LayoutContext::new(1000.0, 500.0);
let out = layout(&diagram, &ctx);
println!("{} primitives", out.primitive_count());
```

Run the full gallery:

```
cargo run --example gallery
```

## Streaming support

LLMs emit JSON progressively. `parse_lossy` accepts a prefix and closes open
brackets to recover a best-effort `Diagram`:

```rust
use makepad_diagram_kit::parse_lossy;
let partial = r#"{"type":"layers","layers":[{"label":"A"}"#;
let diagram = parse_lossy(partial).unwrap();
```

Already-rendered primitives stay stable across re-layouts — re-centering is
permitted (pyramid / tree) but never drift of earlier elements beyond
proportional reflow.

## Theming

`Theme::light()` (default) ships the editorial skin: paper `#faf7f2`,
ink `#1c1917`, accent `#b5523a`. `Theme::dark()` ships paper `#1c1917`,
ink `#faf7f2`, accent `#d97757`. Override via `LayoutContext::with_theme`.

## Limits

- Max body: 200 KB (gated before `serde_json::from_str`)
- Max nodes: 30 per diagram
- Soft caps (emit `Warning::DensityHigh`): pyramid 7, layers 10, flowchart 15,
  tree 20, quadrant 20

## Spec

Full contract lives in
[`specs/m-diagram-v1.spec.md`](specs/m-diagram-v1.spec.md).

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT) at your option.
