use super::*;
use crate::reactive::{Direction, List, NodeRef, build, view};
use crate::styled::Select;

#[test]
fn clicking_outside_an_open_select_popup_closes_it_without_clicking_through() {
    let _root = crate::reactive::RootScope::new();
    let (other_clicks, set_other_clicks) = create_signal(0);
    let (select, other) = (NodeRef::new(), NodeRef::new());
    let document = build({
        move || {
            let options = view! {
                <unstyled::ChoiceOption label="Apple" />
                <unstyled::ChoiceOption label="Banana" />
            };
            view! {
                <List direction=Direction::Horizontal spacing=20.0>
                    <Select @node_ref=&select options selected=Some(0) />
                    <unstyled::Button
                        @node_ref=&other
                        on_click={move || set_other_clicks.update(|other_clicks| *other_clicks += 1)}
                    >
                        <ButtonFace label="Other" />
                    </unstyled::Button>
                </List>
            }
        }
    });
    let (select, other) = (select.get(), other.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = unstyled::select_trigger(harness.document(), select);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(styled::select_open(harness.document(), select));

    harness.click(harness.center(other));
    harness.frame(Vec::new());

    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert_eq!(other_clicks.get_untracked(), 0);
}
