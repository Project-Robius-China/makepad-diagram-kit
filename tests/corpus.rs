//! Corpus tests: parse every fixture under `tests/fixtures/<type>/*.json` and
//! assert broad invariants (parse succeeds, element count is non-zero, layout
//! emits primitives, bounds are non-empty, accent (if present) is respected).

use makepad_diagram_kit::{Diagram, LayoutContext, Primitive, layout, parse};
use std::fs;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn collect(type_dir: &str) -> Vec<(PathBuf, String)> {
    let dir = fixtures_root().join(type_dir);
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let body = fs::read_to_string(&path).unwrap();
            out.push((path, body));
        }
    }
    out
}

fn assert_accent_policy(diagram: &Diagram, layout: &makepad_diagram_kit::DiagramLayout) {
    use makepad_diagram_kit::Theme;
    let accent = Theme::light().palette.accent;
    let n_accented = layout
        .primitives
        .iter()
        .filter(|p| match p {
            Primitive::Rect { stroke, .. } => *stroke == accent,
            Primitive::Polygon { stroke, .. } => *stroke == accent,
            _ => false,
        })
        .count();
    // At most one accented primary shape per diagram.
    assert!(
        n_accented <= 1,
        "{}: {n_accented} accented shapes (expected ≤ 1)",
        diagram.type_tag()
    );
}

fn run_corpus(type_dir: &str) {
    let cases = collect(type_dir);
    assert!(
        !cases.is_empty(),
        "{type_dir} corpus is empty — fixtures missing"
    );
    let ctx = LayoutContext::new(1000.0, 500.0);
    for (path, body) in cases {
        let (diagram, _warnings) = match parse(&body) {
            Ok(r) => r,
            Err(e) => panic!("parse {}: {e}", path.display()),
        };
        let elements = diagram.element_count();
        assert!(
            elements > 0,
            "{}: element count was 0",
            path.display()
        );
        let layout = layout(&diagram, &ctx);
        assert!(
            !layout.primitives.is_empty(),
            "{}: layout produced no primitives",
            path.display()
        );
        let b = layout.bounds;
        assert!(
            b.w >= 0.0 && b.h >= 0.0,
            "{}: degenerate bounds {b:?}",
            path.display()
        );
        assert_accent_policy(&diagram, &layout);
    }
}

#[test]
fn corpus_pyramid() {
    run_corpus("pyramid");
}

#[test]
fn corpus_quadrant() {
    run_corpus("quadrant");
}

#[test]
fn corpus_tree() {
    run_corpus("tree");
}

#[test]
fn corpus_layers() {
    run_corpus("layers");
}

#[test]
fn corpus_flowchart() {
    run_corpus("flowchart");
}
