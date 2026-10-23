use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn showing_a_child_at_the_end_of_a_list_damages_only_that_child() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=40.0 fill=Color32::WHITE />
                <Frame height=40.0 fill={Color32::from_gray(128)} />
                <Show condition={shown}>
                    <Frame height=20.0 fill=Color32::BLACK />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_installed(harness.document_mut(), |_| shown.set(true));
    let damage = harness.frame(Vec::new()).main().damage.bounds();

    assert!(damage.is_positive());
    assert!(
        damage.top() >= 80.0 && damage.bottom() <= 100.0,
        "only the row that appeared is repainted, not {damage:?}"
    );
}
