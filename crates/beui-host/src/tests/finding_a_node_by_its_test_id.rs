use super::*;
use crate::reactive::*;

#[test]
fn finding_a_node_by_its_test_id() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                @test_id={"toolbar.button"}
                on_click={move || clicks.update(|clicks| *clicks += 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(harness.find("toolbar.button")));

    assert_eq!(clicks.get_untracked(), 1);
    assert_eq!(harness.document().find_test_id("toolbar.missing"), None);
}
