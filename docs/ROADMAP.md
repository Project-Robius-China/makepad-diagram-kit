# makepad-diagram-kit Roadmap

Reference source: `../robius/diagram-design`

## Current Status

- Core parser + renderer-agnostic layout are in place.
- Publicly shipped diagram types: `pyramid`, `quadrant`, `tree`, `layers`, `flowchart`, `architecture`, `sequence`, `state`, `er`, `timeline`, `swimlane`, `nested`, `venn`.
- Feature-gated Makepad binding exists via `DiagramView` and `makepad_render`.

## Recently Completed

- `sequence` is now wired through the public `Diagram` surface.
- Sequence fixtures, corpus coverage, and gallery coverage are in place.
- Sequence lifelines and return messages now preserve dashed stroke semantics in the Makepad renderer.
- Makepad renderer contract tests now guard capsule radii, dashed returns, and the public Makepad surface.
- `state`, `er`, `timeline`, `swimlane`, `nested`, and `venn` are wired through parse/layout/corpus/gallery.
- `Circle` is now a first-class primitive for state dots, timeline markers, and Venn sets, with a Makepad SDF shader.
- The public docs and task contract are aligned with the shipped type set.

## Verification Snapshot

Last verified: 2026-04-24.

- `cargo test` passes for default features, including 13 corpus fixture groups.
- `cargo test --features makepad` passes, including Makepad renderer contract coverage.
- `cargo run --example gallery` renders all 13 shipped diagram types with zero warnings.

## Near Term

### 1. Visual Regression Coverage

- Extend renderer contract tests into screenshot-like coverage only if geometry contracts stop catching regressions.
- Prioritize theme parity and any future shader-specific regressions.

## Mid Term

### 2. Downstream Streaming Integration

- Hook this kit into `streaming-markdown-kit`'s diagram code fence flow.
- Land a `diagram_block`-style path parallel to the existing mermaid block pipeline.
- Verify partial JSON streaming keeps layout growth monotonic and visually stable.

### 3. Editorial Primitives

- Annotation callouts from `primitive-annotation.md`
- Sketchy / hand-drawn variant from `primitive-sketchy.md`

These should land only after the main technical diagram grammar is stable.

## Later

### 4. Advanced Layout

- Re-evaluate Dagre or similar only if architecture / state / swimlane diagrams outgrow hand-written layouts.
- Do not add graph-layout dependencies just for parity theater.

## Non-Goals for Now

- Interactive editing
- PNG / SVG export pipeline
- Runtime theme switching
- Rich animation system
- Full WYSIWYG parity with the original HTML assets
