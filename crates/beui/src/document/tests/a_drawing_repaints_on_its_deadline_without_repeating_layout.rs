use super::*;
use crate::reactive::{Draw, Drawing, NodeRef, Prop, build, create_signal, view};

#[test]
fn a_drawing_repaints_on_its_deadline_without_repeating_layout() {
    let drawing = NodeRef::new();
    let asking = Rc::new(RefCell::new(None));
    let mut document = build({
        let (drawing, asking) = (drawing.clone(), asking.clone());
        move || {
            let (on, set_on) = create_signal(true);
            asking.replace(Some(set_on));
            let draw: Prop<Draw> = Prop::Dynamic(Rc::new(move || match on.get() {
                true => blinking(),
                false => still(),
            }));
            view! {
                <Frame width=40.0 height=20.0>
                    <Drawing @node_ref=&drawing draw={draw} />
                </Frame>
            }
        }
    });
    let drawing = drawing.get();
    let (layouts, paints) = counted(&mut document, drawing);
    let mut harness = Harness::new(document);
    let output = harness.frame(vec![]);
    assert!(!output.repaint);
    assert!(output.repaint_after > Duration::ZERO);
    assert!(output.repaint_after <= BLINK);
    assert!(!harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    harness.document.next_paint = Some(Instant::now());
    harness.frame(vec![]);
    assert_eq!((layouts.get(), paints.get()), (1, 2));
    let set_on = asking.borrow().clone().expect("the drawing was built");
    crate::reactive::with_reactive_scope(harness.document_mut(), move || set_on.set(false));
    assert_eq!(harness.frame(vec![]).repaint_after, Duration::MAX);
}
