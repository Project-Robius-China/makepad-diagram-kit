//! Gallery example — builds one of each shipped diagram type from hard-coded JSON,
//! runs the layout engine, and prints a brief summary to stdout.
//!
//! Run with:
//! ```text
//! cargo run --example gallery
//! ```

use makepad_diagram_kit::{Diagram, LayoutContext, layout, parse};

const PYRAMID: &str = r#"{
  "type": "pyramid",
  "levels": [
    {"label": "Mission", "sublabel": "why we exist"},
    {"label": "Strategy", "sublabel": "how we win"},
    {"label": "Tactics", "sublabel": "what we do"}
  ],
  "accent_idx": 0
}"#;

const QUADRANT: &str = r#"{
  "type": "quadrant",
  "axes": {
    "x": {"min": 0, "max": 10, "low_label": "LOW EFFORT", "high_label": "HIGH EFFORT"},
    "y": {"min": 0, "max": 10, "low_label": "LOW IMPACT", "high_label": "HIGH IMPACT"}
  },
  "points": [
    {"x": 2, "y": 8, "label": "quick win"},
    {"x": 8, "y": 9, "label": "big bet"},
    {"x": 3, "y": 2, "label": "nice-to-have"},
    {"x": 9, "y": 1, "label": "time sink"}
  ],
  "accent_idx": 1
}"#;

const TREE: &str = r#"{
  "type": "tree",
  "root": {
    "label": "Product",
    "children": [
      {"label": "Core", "children": [
        {"label": "Parse"},
        {"label": "Layout"}
      ]},
      {"label": "Bindings"}
    ]
  },
  "accent_path": [0, 1]
}"#;

const LAYERS: &str = r#"{
  "type": "layers",
  "layers": [
    {"label": "Application", "tag": "L7", "annotation": "HTTP / gRPC"},
    {"label": "Transport", "tag": "L4", "annotation": "TCP / UDP"},
    {"label": "Internet",  "tag": "L3", "annotation": "IP"},
    {"label": "Link",      "tag": "L2", "annotation": "Ethernet"}
  ],
  "accent_idx": 0
}"#;

const FLOWCHART: &str = r#"{
  "type": "flowchart",
  "nodes": [
    {"id": "start", "label": "Receive request", "shape": "oval"},
    {"id": "auth",  "label": "Authorized?",     "shape": "diamond"},
    {"id": "serve", "label": "Serve resource",  "shape": "rect"},
    {"id": "deny",  "label": "401",             "shape": "rect"},
    {"id": "end",   "label": "Respond",         "shape": "oval"}
  ],
  "edges": [
    {"from": "start", "to": "auth"},
    {"from": "auth",  "to": "serve", "label": "yes"},
    {"from": "auth",  "to": "deny",  "label": "no"},
    {"from": "serve", "to": "end"},
    {"from": "deny",  "to": "end"}
  ],
  "accent_idx": 1
}"#;

const ARCHITECTURE: &str = r#"{
  "type": "architecture",
  "orientation": "lr",
  "nodes": [
    {"id": "user", "label": "User", "tag": "CLIENT", "role": "external"},
    {"id": "api",  "label": "API Gateway", "tag": "MW", "role": "focal"},
    {"id": "db",   "label": "Database", "tag": "STORE", "role": "store"}
  ],
  "edges": [
    {"from": "user", "to": "api", "label": "POST /login", "role": "primary"},
    {"from": "api",  "to": "db",  "label": "SELECT user"}
  ]
}"#;

const SEQUENCE: &str = r#"{
  "type": "sequence",
  "actors": [
    {"id": "user", "label": "User", "tag": "CLIENT"},
    {"id": "api",  "label": "API",  "tag": "MW", "role": "focal"},
    {"id": "db",   "label": "DB",   "tag": "STORE"}
  ],
  "messages": [
    {"from": "user", "to": "api", "label": "POST /login", "role": "primary"},
    {"from": "api",  "to": "db",  "label": "SELECT user"},
    {"from": "db",   "to": "api", "label": "row"},
    {"from": "api",  "to": "user","label": "200 OK", "role": "primary"}
  ]
}"#;

const STATE: &str = r#"{
  "type": "state",
  "orientation": "lr",
  "states": [
    {"id": "idle", "label": "Idle", "kind": "start"},
    {"id": "loading", "label": "Loading"},
    {"id": "done", "label": "Done", "kind": "end", "role": "focal"}
  ],
  "transitions": [
    {"from": "idle", "to": "loading", "label": "submit"},
    {"from": "loading", "to": "done", "label": "ok", "role": "primary"}
  ]
}"#;

const ER: &str = r#"{
  "type": "er",
  "entities": [
    {"id": "user", "name": "User", "role": "focal", "fields": [
      {"name": "id", "type": "uuid", "key": "pk"},
      {"name": "email", "type": "text"}
    ]},
    {"id": "order", "name": "Order", "fields": [
      {"name": "id", "type": "uuid", "key": "pk"},
      {"name": "user_id", "type": "uuid", "key": "fk"}
    ]}
  ],
  "relationships": [
    {"from": "user", "to": "order", "from_cardinality": "1", "to_cardinality": "N", "label": "places"}
  ]
}"#;

const TIMELINE: &str = r#"{
  "type": "timeline",
  "events": [
    {"time": "2026-01-10", "label": "Kickoff"},
    {"time": "2026-02-20", "label": "Beta", "role": "major"},
    {"time": "2026-04-01", "label": "Launch"}
  ]
}"#;

const SWIMLANE: &str = r#"{
  "type": "swimlane",
  "lanes": [
    {"id": "pm", "label": "Product"},
    {"id": "eng", "label": "Engineering"}
  ],
  "steps": [
    {"id": "brief", "lane": "pm", "label": "Write brief"},
    {"id": "build", "lane": "eng", "label": "Build", "role": "focal"},
    {"id": "ship", "lane": "eng", "label": "Ship"}
  ],
  "edges": [
    {"from": "brief", "to": "build", "label": "handoff", "role": "primary"},
    {"from": "build", "to": "ship"}
  ]
}"#;

const NESTED: &str = r#"{
  "type": "nested",
  "levels": [
    {"label": "Repo"},
    {"label": "Crate"},
    {"label": "Module", "role": "focal"}
  ]
}"#;

const VENN: &str = r#"{
  "type": "venn",
  "sets": [
    {"id": "desirable", "label": "Desirable"},
    {"id": "feasible", "label": "Feasible"},
    {"id": "viable", "label": "Viable"}
  ],
  "intersections": [
    {"sets": ["desirable", "feasible", "viable"], "label": "Product", "role": "focal"}
  ]
}"#;

fn render(name: &str, json: &str) {
    let ctx = LayoutContext::new(1000.0, 500.0);
    match parse(json) {
        Ok((diagram, warnings)) => {
            let layout = layout(&diagram, &ctx);
            println!(
                "  {name:<10} type={type:<10} elements={elements:<3} primitives={primitives:<3} \
                 bounds=({bx:6.1},{by:6.1},{bw:6.1}x{bh:6.1})  warnings={w}",
                name = name,
                type = diagram.type_tag(),
                elements = diagram.element_count(),
                primitives = layout.primitive_count(),
                bx = layout.bounds.x,
                by = layout.bounds.y,
                bw = layout.bounds.w,
                bh = layout.bounds.h,
                w = warnings.len(),
            );
        }
        Err(err) => {
            println!("  {name:<10} ERROR: {err}");
        }
    }
}

fn main() {
    println!("makepad-diagram-kit gallery");
    println!("===========================");
    println!("  canvas = 1000 x 500 lpx, theme = light");
    println!();
    render("pyramid", PYRAMID);
    render("quadrant", QUADRANT);
    render("tree", TREE);
    render("layers", LAYERS);
    render("flowchart", FLOWCHART);
    render("arch", ARCHITECTURE);
    render("sequence", SEQUENCE);
    render("state", STATE);
    render("er", ER);
    render("timeline", TIMELINE);
    render("swimlane", SWIMLANE);
    render("nested", NESTED);
    render("venn", VENN);
    println!();
    println!("  ok.");

    // Also demonstrate parse_lossy with a truncated pyramid.
    let partial = r#"{"type":"pyramid","levels":[{"label":"L1"},{"label":"L2"}"#;
    match makepad_diagram_kit::parse_lossy(partial) {
        Some(Diagram::Pyramid(p)) => {
            println!(
                "  lossy prefix: pyramid recovered {} level(s)",
                p.levels.len()
            );
        }
        Some(other) => {
            println!("  lossy prefix: recovered {}", other.type_tag());
        }
        None => println!("  lossy prefix: not recoverable"),
    }
}
