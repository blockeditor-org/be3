use super::*;

#[test]
fn composing_retained_documents_preserves_paint_order() {
    let source = retained();
    let context = Context::new();
    let output = context.run(RawInput::default(), |ctx| {
        ctx.extend(&source);
        ctx.painter().rect_filled(rect(), 0.0, Color32::BLACK);
        ctx.extend(&source);
    });
    assert_eq!(source.len(), 1);
    assert_eq!(output.shapes().len(), 3);
    assert!(output.shapes()[0] == source[0]);
    assert!(matches!(&output.shapes()[1], Shape::Rect { color, .. } if *color == Color32::BLACK));
    assert!(output.shapes()[2] == source[0]);
}
