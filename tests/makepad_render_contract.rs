#![cfg(feature = "makepad")]

use makepad_diagram_kit::{
    DiagramView, LayoutContext, LineStyle, Primitive, Theme, layout, makepad_render::color_to_vec4,
    parse,
};

fn primary_rects(primitives: &[Primitive]) -> Vec<(f32, f32, f32)> {
    primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Rect {
                w,
                h,
                corner_radius,
                ..
            } if *w >= 100.0 && *h >= 40.0 => Some((*w, *h, *corner_radius)),
            _ => None,
        })
        .collect()
}

#[test]
fn makepad_render_contract_flowchart_preserves_capsule_and_rect_radii() {
    let body = r#"{
      "type":"flowchart",
      "nodes":[
        {"id":"a","label":"Oval A","shape":"oval"},
        {"id":"b","label":"Rect B","shape":"rect"}
      ],
      "edges":[{"from":"a","to":"b"}]
    }"#;
    let (diagram, warnings) = parse(body).unwrap();
    assert!(warnings.is_empty());
    let out = layout(&diagram, &LayoutContext::new(500.0, 250.0));
    let rects = primary_rects(&out.primitives);

    assert_eq!(rects.len(), 2);
    assert!(
        rects.iter().any(|(_, h, r)| (*r - *h * 0.5).abs() < 0.01),
        "oval nodes must reach the renderer as true capsule radii: {rects:?}"
    );
    assert!(
        rects
            .iter()
            .any(|(_, _, r)| (*r - Theme::light().corner_radius).abs() < 0.01),
        "rect nodes must keep the theme's standard rounded-rect radius: {rects:?}"
    );
}

#[test]
fn makepad_render_contract_sequence_preserves_dashed_returns() {
    let body = r#"{
      "type": "sequence",
      "actors": [
        {"id":"user","label":"User","tag":"CLIENT"},
        {"id":"api","label":"API Gateway","tag":"MW","role":"focal"},
        {"id":"db","label":"Database","tag":"STORE"}
      ],
      "messages": [
        {"from":"user","to":"api","label":"POST /login","role":"primary"},
        {"from":"api","to":"db","label":"SELECT user"},
        {"from":"db","to":"api","label":"row","kind":"return"},
        {"from":"api","to":"user","label":"200 OK","kind":"return","role":"primary"}
      ]
    }"#;
    let (diagram, warnings) = parse(body).unwrap();
    assert!(warnings.is_empty());
    let out = layout(&diagram, &LayoutContext::new(1000.0, 500.0));

    let lifelines = out
        .primitives
        .iter()
        .filter(|p| {
            matches!(
                p,
                Primitive::Line {
                    style: LineStyle::Dashed,
                    ..
                }
            )
        })
        .count();
    let arrow_styles: Vec<_> = out
        .primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Arrow {
                from, to, style, ..
            } => Some((from.x < to.x, *style)),
            _ => None,
        })
        .collect();

    assert_eq!(lifelines, 3);
    assert_eq!(
        arrow_styles,
        vec![
            (true, LineStyle::Solid),
            (true, LineStyle::Solid),
            (false, LineStyle::Dashed),
            (false, LineStyle::Dashed)
        ]
    );
}

#[test]
fn makepad_render_contract_venn_reaches_renderer_as_true_circles() {
    let body = r#"{
      "type": "venn",
      "sets": [
        {"id":"a","label":"A"},
        {"id":"b","label":"B"},
        {"id":"c","label":"C"}
      ],
      "intersections": [
        {"sets":["a","b","c"],"label":"ABC","role":"focal"}
      ]
    }"#;
    let (diagram, warnings) = parse(body).unwrap();
    assert!(warnings.is_empty());
    let out = layout(&diagram, &LayoutContext::new(1000.0, 500.0));

    let circles = out
        .primitives
        .iter()
        .filter(|p| matches!(p, Primitive::Circle { .. }))
        .count();

    assert_eq!(circles, 3, "venn sets must stay true circles");
}

#[test]
fn makepad_render_contract_public_makepad_surface_compiles() {
    let _view_type: Option<DiagramView> = None;
    let v = color_to_vec4(Theme::light().palette.accent);

    assert!(v.x > 0.0);
    assert_eq!(v.w, 1.0);
}
