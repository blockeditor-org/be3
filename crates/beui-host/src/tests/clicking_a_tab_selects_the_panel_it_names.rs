use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn clicking_a_tab_selects_the_panel_it_names() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(0);
    let (document, [tabs]) = toolbar_of(|| {
        [view! {
            <Tabs
                options={view! {
                    <unstyled::ChoiceOption label="List" />
                    <unstyled::ChoiceOption label="Load" />
                }}
                selected={reported}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(tabs)
            .selected()
            .unwrap_or(0),
        0
    );

    let list = shown_fit(harness.document(), tabs);
    let second = harness.document().children(list)[1];
    harness.click(harness.center(second));
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(tabs)
            .selected()
            .unwrap_or(0),
        1
    );
    assert_eq!(reported.get_untracked(), 1);
}
