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
                selected=0
                on_change={move |selected| {
                    reported.set(selected);
                }}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);

    let second = harness.document().children(tabs)[1];
    harness.click(harness.center(second));
    harness.frame(Vec::new());

    assert_eq!(styled::tabs_selected(harness.document(), tabs), 1);
    assert_eq!(reported.get_untracked(), 1);
}
