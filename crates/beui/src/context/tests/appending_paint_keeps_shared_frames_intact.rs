use super::*;

#[test]
fn appending_paint_keeps_shared_frames_intact() {
    for top in [false, true] {
        let source = retained();
        let context = Context::new();
        let previous = context.run(RawInput::default(), |ctx| ctx.extend(&source));
        let output = context.run(RawInput::default(), |ctx| {
            ctx.extend(&source);
            let painter = ctx.painter();
            let painter = if top { painter.on_top() } else { painter };
            painter.rect_filled(rect(), 0.0, Color32::BLACK);
            ctx.report_damage(rect());
        });
        assert!(output.changed);
        assert_eq!(output.shapes().len(), 2);
        assert_eq!(source.len(), 1);
        assert!(Rc::ptr_eq(&previous.shapes, &source));
        assert!(previous.shapes() == &output.shapes()[..1]);
        let restored = context.run(RawInput::default(), |ctx| {
            ctx.extend(&source);
            ctx.report_damage(rect());
        });
        assert!(restored.changed);
        assert!(Rc::ptr_eq(&restored.shapes, &source));
    }
}
