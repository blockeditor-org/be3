use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_dialog_taller_than_the_screen_stays_on_it_and_scrolls() {
    let _root = crate::reactive::RootScope::new();
    let first = NodeRef::new();
    let last = NodeRef::new();
    let document = build_node({
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog open=true title="Tall" width={Size::Custom(300.0)} on_dismiss={|| {}}>
                        <List spacing=8.0>
                            <Frame @node_ref=&first height=80.0 />
                            <ForEach keys={(0..8).collect::<Vec<usize>>()}>|_line: usize|
                                <Frame height=80.0 />
                            </ForEach>
                            <Frame @node_ref=&last height=80.0 />
                        </List>
                    </Dialog>
                </List>
            }
        }
    });
    let (first, last) = (first.get(), last.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.rect(first).top() > 0.0,
        "the top of the dialog is on the screen"
    );
    assert!(
        harness.rect(last).top() > VIEWPORT.y,
        "what does not fit starts out below it"
    );

    harness.frame(vec![
        Input::PointerMoved(harness.center(first)),
        Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, -2000.0))),
    ]);
    harness.settle();

    assert!(
        harness.rect(last).bottom() < VIEWPORT.y,
        "the dialog scrolls the rest into view, so its bottom was on the screen too"
    );
}
