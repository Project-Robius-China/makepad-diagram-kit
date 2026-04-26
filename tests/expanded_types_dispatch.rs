use makepad_diagram_kit::{Diagram, LayoutContext, LineStyle, Primitive, parse};

#[test]
fn state_diagram_parses_and_renders_transitions() {
    let body = r#"{
      "type": "state",
      "states": [
        {"id": "idle", "label": "Idle", "kind": "start"},
        {"id": "loading", "label": "Loading"},
        {"id": "done", "label": "Done", "kind": "end", "role": "focal"}
      ],
      "transitions": [
        {"from": "idle", "to": "loading", "label": "submit"},
        {"from": "loading", "to": "done", "label": "ok", "role": "primary"}
      ],
      "orientation": "lr"
    }"#;
    let (diagram, _) = parse(body).unwrap();
    assert!(matches!(diagram, Diagram::State(_)));

    let layout = diagram.layout(&LayoutContext::new(1000.0, 500.0));
    assert!(layout.primitives.iter().any(|p| matches!(
        p,
        Primitive::Arrow {
            style: LineStyle::Solid,
            ..
        }
    )));
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "submit") })
    );
}

#[test]
fn er_diagram_parses_and_renders_entities_fields_and_cardinality() {
    let body = r##"{
      "type": "er",
      "entities": [
        {"id": "user", "name": "User", "fields": [
          {"name": "id", "type": "uuid", "key": "pk"},
          {"name": "email", "type": "text"}
        ], "role": "focal"},
        {"id": "order", "name": "Order", "fields": [
          {"name": "id", "type": "uuid", "key": "pk"},
          {"name": "user_id", "type": "uuid", "key": "fk"}
        ]}
      ],
      "relationships": [
        {"from": "user", "to": "order", "from_cardinality": "1", "to_cardinality": "N", "label": "places"}
      ]
    }"##;
    let (diagram, _) = parse(body).unwrap();
    assert!(matches!(diagram, Diagram::Er(_)));

    let layout = diagram.layout(&LayoutContext::new(1000.0, 500.0));
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "# id: uuid") })
    );
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "-> user_id: uuid") })
    );
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "N") })
    );
}

#[test]
fn timeline_diagram_parses_and_renders_axis_markers() {
    let body = r#"{
      "type": "timeline",
      "events": [
        {"time": "2026-01-01", "label": "Kickoff"},
        {"time": "2026-02-15", "label": "Beta", "role": "major"},
        {"time": "2026-04-01", "label": "Launch"}
      ]
    }"#;
    let (diagram, _) = parse(body).unwrap();
    assert!(matches!(diagram, Diagram::Timeline(_)));

    let layout = diagram.layout(&LayoutContext::new(1000.0, 500.0));
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Line { .. }))
    );
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Circle { .. }))
    );
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "Beta") })
    );
}

#[test]
fn swimlane_diagram_parses_and_renders_lane_handoffs() {
    let body = r#"{
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
    let (diagram, _) = parse(body).unwrap();
    assert!(matches!(diagram, Diagram::Swimlane(_)));

    let layout = diagram.layout(&LayoutContext::new(1000.0, 500.0));
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "Product") })
    );
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Arrow { .. }))
    );
}

#[test]
fn nested_diagram_parses_and_renders_containment_rings() {
    let body = r#"{
      "type": "nested",
      "levels": [
        {"label": "Repo"},
        {"label": "Crate"},
        {"label": "Module", "role": "focal"}
      ]
    }"#;
    let (diagram, _) = parse(body).unwrap();
    assert!(matches!(diagram, Diagram::Nested(_)));

    let layout = diagram.layout(&LayoutContext::new(1000.0, 500.0));
    let rings = layout
        .primitives
        .iter()
        .filter(|p| matches!(p, Primitive::Rect { .. }))
        .count();
    assert!(rings >= 3, "expected at least 3 containment rects");
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "MODULE") })
    );
}

#[test]
fn venn_diagram_parses_and_renders_overlapping_circles() {
    let body = r#"{
      "type": "venn",
      "sets": [
        {"id": "a", "label": "Desirable"},
        {"id": "b", "label": "Feasible"},
        {"id": "c", "label": "Viable"}
      ],
      "intersections": [
        {"sets": ["a", "b", "c"], "label": "Product", "role": "focal"}
      ]
    }"#;
    let (diagram, _) = parse(body).unwrap();
    assert!(matches!(diagram, Diagram::Venn(_)));

    let layout = diagram.layout(&LayoutContext::new(1000.0, 500.0));
    let circles = layout
        .primitives
        .iter()
        .filter(|p| matches!(p, Primitive::Circle { .. }))
        .count();
    assert_eq!(circles, 3);
    assert!(
        layout
            .primitives
            .iter()
            .any(|p| { matches!(p, Primitive::Text { text, .. } if text == "Product") })
    );
}
