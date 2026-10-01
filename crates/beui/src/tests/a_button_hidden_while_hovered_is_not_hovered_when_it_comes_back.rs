use super::*;
use crate::reactive::{
    List, ReadSignal, ShowKeepAlive, Text, WriteSignal, build, create_signal, view,
    with_reactive_scope,
};
use crate::unstyled::ButtonHandle;

struct Held {
    shown: WriteSignal<bool>,
    hovered: Option<ReadSignal<bool>>,
}

#[test]
fn a_button_hidden_while_hovered_is_not_hovered_when_it_comes_back() {
    let held: Rc<RefCell<Option<Held>>> = Rc::default();
    let sink = held.clone();
    let document = build(move || {
        let (shown, set_shown) = create_signal(true);
        sink.replace(Some(Held {
            shown: set_shown,
            hovered: None,
        }));
        let hovering = sink.clone();
        view! {
            <List spacing=0.0>
                <ShowKeepAlive condition={shown}>
                    <unstyled::Button
                        @test_id={"hidden.button"}
                        on_click={|| {}}
                        content={move |handle: ButtonHandle| {
                            if let Some(held) = hovering.borrow_mut().as_mut() {
                                held.hovered = Some(handle.hovered.clone());
                            }
                            view! {
                                <Text string="Themes" />
                            }
                        }}
                    />
                </ShowKeepAlive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let button = harness
        .document()
        .find_test_id("hidden.button")
        .expect("the button");
    let at = harness.center(button);
    let hovered = held
        .borrow()
        .as_ref()
        .and_then(|held| held.hovered.clone())
        .expect("the button reported its hover");

    harness.frame(vec![Event::PointerMoved(at)]);
    assert!(hovered.get(), "the pointer hovers the button");

    let hide = |harness: &mut Harness, shown: bool| {
        let held = held.borrow();
        let set = &held.as_ref().expect("the view published its setter").shown;
        with_reactive_scope(harness.document_mut(), || set.set(shown));
        harness.frame(Vec::new());
    };
    hide(&mut harness, false);
    harness.frame(vec![Event::PointerGone]);
    hide(&mut harness, true);

    assert!(
        !hovered.get(),
        "the button left the screen under the pointer, so it comes back unhovered"
    );
}
