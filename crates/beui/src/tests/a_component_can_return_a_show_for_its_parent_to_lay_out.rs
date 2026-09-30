use super::*;
use crate::reactive::{
    DynamicSegment, List, ListChild, Prop, Show, build, component, create_signal, view,
};

#[test]
fn a_component_can_return_a_show_for_its_parent_to_lay_out() {
    let (shown, set_shown) = create_signal(false);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame height=40.0 color=Color32::WHITE />
                <Extra shown />
                <Frame @test_id="after" height=40.0 color=Color32::WHITE />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let after = harness.find("after");
    assert_eq!(harness.rect(after).top(), 40.0);

    with_installed(harness.document_mut(), |_| set_shown.set(true));
    harness.frame(Vec::new());

    assert_eq!(harness.rect(after).top(), 60.0);
}

#[component]
fn Extra(shown: Prop<bool>) -> DynamicSegment<ListChild> {
    view! {
        <Show condition={shown}>
            <Frame height=20.0 color=Color32::BLACK />
        </Show>
    }
}
