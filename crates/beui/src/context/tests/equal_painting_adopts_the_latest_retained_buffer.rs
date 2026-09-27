use super::*;

#[test]
fn equal_painting_adopts_the_latest_retained_buffer() {
    let source = retained();
    let replacement = Rc::new(source.as_ref().clone());
    let context = Context::new();
    let first = context.run(RawInput::default(), |ctx| ctx.extend(&source));
    let equal = context.run(RawInput::default(), |ctx| {
        ctx.extend(&replacement);
        ctx.report_damage(rect());
    });
    assert!(!equal.changed);
    assert!(first.shapes() == equal.shapes());
    let previous = context.inner.previous.borrow();
    assert!(Rc::ptr_eq(&previous.as_ref().unwrap().shapes, &replacement));
}
