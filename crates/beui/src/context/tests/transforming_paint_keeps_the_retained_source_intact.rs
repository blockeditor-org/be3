use super::*;

#[test]
fn transforming_paint_keeps_the_retained_source_intact() {
    for prefix in [false, true] {
        let source = retained();
        let original = source.as_ref().clone();
        let clip = Rect::from_min_max(pos2(15.0, 15.0), pos2(25.0, 25.0));
        let context = Context::new();
        let output = context.run(RawInput::default(), |ctx| {
            if prefix {
                ctx.extend(&source);
            }
            ctx.scaled(2.0, || ctx.clipped(clip, || ctx.extend(&source)));
        });
        assert!(*source == original);
        assert_eq!(output.shapes().len(), 1 + usize::from(prefix));
        if prefix {
            assert!(output.shapes()[0] == source[0]);
        }
        let Shape::Rect {
            rect: painted,
            clip: painted_clip,
            ..
        } = &output.shapes()[usize::from(prefix)]
        else {
            panic!("expected a rectangle");
        };
        assert_eq!(*painted, rect().scaled(2.0));
        assert_eq!(*painted_clip, clip.scaled(2.0));
    }
}
