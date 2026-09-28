use super::*;
use crate::reactive::{List, Show, build, create_signal, view};

#[test]
fn showing_a_child_at_the_end_of_a_list_damages_only_that_child() {
    let (shown, set_shown) = create_signal(false);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame height=40.0 color=Color32::WHITE />
                <Frame height=40.0 color=Color32::from_gray(128) />
                <Show condition={shown}>
                    <Frame height=20.0 color=Color32::BLACK />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_installed(harness.document_mut(), |_| set_shown.set(true));
    let damage = harness.frame(Vec::new()).damage.bounds();

    assert!(damage.is_positive());
    assert!(
        damage.top() >= 80.0 && damage.bottom() <= 100.0,
        "only the row that appeared is repainted, not {damage:?}"
    );
}
