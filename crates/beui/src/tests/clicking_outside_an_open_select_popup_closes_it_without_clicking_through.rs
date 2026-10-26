use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn clicking_outside_an_open_select_popup_closes_it_without_clicking_through() {
    let _root = crate::reactive::RootScope::new();
    let other_clicks = create_signal(0);
    let (select, other) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
        };
        view! {
            <List direction=Direction::Horizontal spacing=20.0>
                <Select @node_ref=&select options selected=Some(0) />
                <unstyled::Button
                    @node_ref=&other
                    on_click={move || other_clicks.update(|other_clicks| *other_clicks += 1)}
                >
                    <ButtonFace label="Other" />
                </unstyled::Button>
            </List>
        }
    });
    let (select, other) = (select.get(), other.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .trigger();
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(harness.document().api::<styled::SelectApi>(select).open());

    harness.click(harness.center(other));
    harness.frame(Vec::new());

    assert!(!harness.document().api::<styled::SelectApi>(select).open());
    assert_eq!(
        harness
            .document()
            .api::<styled::SelectApi>(select)
            .selected(),
        Some(0)
    );
    assert_eq!(other_clicks.get_untracked(), 0);
}
