use super::*;
use crate::primitives::*;
use crate::reactive::Draw;
use crate::reactive::*;

#[test]
fn a_drawing_repaints_on_its_deadline_without_repeating_layout() {
    let _root = crate::reactive::RootScope::new();
    let drawing = NodeRef::new();
    let asking = create_signal(None);
    let mut document = build_node(move || {
        let on = create_signal(true);
        asking.update(|asking| *asking = Some(on));
        let draw: Prop<Draw> = Prop::dynamic(move || match on.get() {
            true => blinking(),
            false => still(),
        });
        view! {
            <Frame width=40.0 height=20.0>
                <Drawing @node_ref=&drawing draw={draw} />
            </Frame>
        }
    });
    let drawing = drawing.get();
    let (layouts, paints) = counted(&mut document, drawing);
    let mut harness = Harness::new(document);
    let output = harness.frame(vec![]);
    assert!(!output.repaint);
    assert!(output.repaint_after > Duration::ZERO);
    assert!(output.repaint_after <= BLINK);
    assert!(!harness.frame(vec![]).main().changed);
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (1, 1));
    harness.advance(BLINK);
    harness.frame(vec![]);
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (1, 2));
    let on = asking.get_untracked().expect("the drawing was built");
    on.set(false);
    assert_eq!(harness.frame(vec![]).repaint_after, Duration::MAX);
}
