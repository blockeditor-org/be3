use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::ButtonHandle;

#[derive(Clone, Copy)]
struct Held {
    shown: Signal<bool>,
    hovered: Option<ReadSignal<bool>>,
}

#[test]
fn a_button_hidden_while_hovered_is_not_hovered_when_it_comes_back() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<Held>>(None);
    let document = build_node(move || {
        let shown = create_signal(true);
        held.update(|held| {
            *held = Some(Held {
                shown,
                hovered: None,
            })
        });
        view! {
            <List spacing=0.0>
                <ShowKeepAlive condition={shown}>
                    <unstyled::Button
                        @test_id={"hidden.button"}
                        on_click={|| {}}
                    >|handle: ButtonHandle| {
                        held.update(|held| {
                            if let Some(held) = held {
                                held.hovered = Some(handle.hovered);
                            }
                        });
                        view! {
                            <Text>"Themes"</Text>
                        }
                    }</unstyled::Button>
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
        .get_untracked()
        .and_then(|held| held.hovered)
        .expect("the button reported its hover");

    harness.frame(vec![Input::PointerMoved(at)]);
    assert!(hovered.get(), "the pointer hovers the button");

    let hide = |harness: &mut Harness, shown: bool| {
        let set = held
            .get_untracked()
            .expect("the view published its setter")
            .shown;
        set.set(shown);
        harness.frame(Vec::new());
    };
    hide(&mut harness, false);
    harness.frame(vec![Input::PointerLeft]);
    hide(&mut harness, true);

    assert!(
        !hovered.get(),
        "the button left the screen under the pointer, so it comes back unhovered"
    );
}
