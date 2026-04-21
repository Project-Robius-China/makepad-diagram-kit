//! v1.2 editorial polish demo — prints JSON snippets users can paste into
//! aichat (via a ```diagram fenced code block) to see the new features:
//!
//! - eyebrow tags inside nodes (`tag: "ROOT"`, etc.)
//! - rounded corners (automatic via the new `DrawRoundedRect` shader)
//! - dot-pattern canvas background (painted automatically by `DiagramView`)
//! - flowchart edge roles: `Default` / `Primary` (accent) / `External` (link)
//!
//! Run with:
//! ```text
//! cargo run --example polish_demo
//! ```
//!
//! Each block is a standalone diagram body — copy/paste the contents between
//! the `=== BEGIN ===` and `=== END ===` markers into a chat message wrapped
//! in a ```diagram fence.

use makepad_diagram_kit::{LayoutContext, layout, parse};

/// Tree with eyebrow tags — showcases Task 1 (eyebrow) + Task 2 (rounded).
const TREE_TAGS: &str = r#"{
  "type": "tree",
  "accent_path": [],
  "root": {
    "label": "Skills",
    "tag": "root",
    "children": [
      {
        "label": "Design",
        "tag": "cat",
        "sublabel": "ui · visual · ux",
        "children": [
          {"label": "polish", "tag": "leaf"},
          {"label": "critique", "tag": "leaf"}
        ]
      },
      {
        "label": "Engineering",
        "tag": "cat",
        "sublabel": "ship · review · test",
        "children": [
          {"label": "review", "tag": "leaf"},
          {"label": "test", "tag": "leaf"}
        ]
      },
      {
        "label": "Research",
        "tag": "cat",
        "sublabel": "investigate · analyze",
        "children": [
          {"label": "explore", "tag": "leaf"}
        ]
      }
    ]
  }
}"#;

/// Pyramid with eyebrow tags on each level.
const PYRAMID_TAGS: &str = r#"{
  "type": "pyramid",
  "levels": [
    {"label": "Mission", "tag": "L0", "sublabel": "why"},
    {"label": "Strategy", "tag": "L1", "sublabel": "how"},
    {"label": "Tactics", "tag": "L2", "sublabel": "what"}
  ],
  "accent_idx": 0
}"#;

/// Flowchart with mixed edge roles — Task 4 in action.
const FLOWCHART_EDGES: &str = r#"{
  "type": "flowchart",
  "nodes": [
    {"id": "client", "label": "Client", "tag": "ext", "shape": "oval"},
    {"id": "edge",   "label": "Edge",   "tag": "svc", "shape": "rect"},
    {"id": "ssr",    "label": "SSR",    "tag": "svc", "shape": "rect"},
    {"id": "api",    "label": "API",    "tag": "svc", "shape": "rect"},
    {"id": "db",     "label": "DB",     "tag": "data","shape": "rect"}
  ],
  "edges": [
    {"from": "client", "to": "edge",  "label": "HTTPS", "role": "external"},
    {"from": "edge",   "to": "ssr",   "label": "SSR",   "role": "primary"},
    {"from": "ssr",    "to": "api",   "label": "RPC",   "role": "default"},
    {"from": "api",    "to": "db",    "label": "SQL",   "role": "default"}
  ],
  "accent_idx": 2
}"#;

/// Flowchart with an eyebrow tag + accent — the simplest minimal demo.
const FLOWCHART_MINIMAL: &str = r#"{
  "type": "flowchart",
  "nodes": [
    {"id": "a", "label": "Input",   "tag": "start"},
    {"id": "b", "label": "Process", "tag": "step"},
    {"id": "c", "label": "Output",  "tag": "end"}
  ],
  "edges": [
    {"from": "a", "to": "b"},
    {"from": "b", "to": "c", "label": "result", "role": "primary"}
  ],
  "accent_idx": 1
}"#;

fn show(label: &str, body: &str) {
    println!("=== BEGIN {label} ===");
    println!("{body}");
    println!("=== END {label} ===\n");

    // Cheap round-trip sanity check so the demo file also doubles as an
    // integration test: every snippet must parse + lay out without error.
    let (d, warnings) = parse(body).unwrap_or_else(|e| panic!("{label} failed to parse: {e}"));
    let ctx = LayoutContext::new(1000.0, 500.0);
    let l = layout(&d, &ctx);
    println!(
        "(parsed OK: {} primitives, {} warning(s))\n",
        l.primitive_count(),
        warnings.len()
    );
}

fn main() {
    show("TREE_TAGS", TREE_TAGS);
    show("PYRAMID_TAGS", PYRAMID_TAGS);
    show("FLOWCHART_EDGES", FLOWCHART_EDGES);
    show("FLOWCHART_MINIMAL", FLOWCHART_MINIMAL);
}
