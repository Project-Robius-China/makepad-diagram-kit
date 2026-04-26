use makepad_diagram_kit::{Diagram, parse};

#[test]
fn sequence_dispatch_from_parse() {
    let body = r#"{
      "type": "sequence",
      "actors": [{"id": "a", "label": "A"}],
      "messages": []
    }"#;
    let (diagram, warnings) = parse(body).unwrap();
    assert!(warnings.is_empty());
    assert!(matches!(diagram, Diagram::Sequence(_)));
}
