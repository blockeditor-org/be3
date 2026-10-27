use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::ChoiceOption;

#[test]
fn a_value_keyed_choice_keeps_its_value_when_options_reorder_and_shows_none_once_it_is_removed() {
    let _root = crate::reactive::RootScope::new();
    let fruits = create_signal(vec!["Apple", "Banana", "Cherry"]);
    let chosen = create_signal(Some("Banana"));
    let (document, [select, group]) = toolbar_of(move || {
        [
            view! {
                <Select
                    options={view! {
                        <ForEach keys={fruits}>|fruit: &'static str|
                            <ChoiceOption value=fruit label=fruit />
                        </ForEach>
                    }}
                    selected={chosen}
                />
            },
            view! {
                <RadioGroup
                    options={view! {
                        <ForEach keys={fruits}>|fruit: &'static str|
                            <ChoiceOption value=fruit label=fruit />
                        </ForEach>
                    }}
                    selected={chosen}
                />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let shown = |harness: &Harness| {
        (
            harness
                .document()
                .api::<unstyled::SelectApi<&str>>(select)
                .selected(),
            harness
                .document()
                .api::<styled::ChoiceApi<&str>>(group)
                .selected(),
        )
    };
    assert_eq!(shown(&harness), (Some("Banana"), Some("Banana")));

    with_installed(harness.document_mut(), |_| {
        fruits.set(vec!["Cherry", "Apple", "Banana"]);
    });
    harness.frame(Vec::new());
    assert_eq!(
        shown(&harness),
        (Some("Banana"), Some("Banana")),
        "a choice follows its value to wherever it moved, not the place it sat at"
    );

    with_installed(harness.document_mut(), |_| {
        fruits.set(vec!["Cherry", "Apple"]);
    });
    harness.frame(Vec::new());
    assert_eq!(
        shown(&harness),
        (None, None),
        "a chosen value that left the options shows nothing chosen, never a neighbour"
    );
    assert_eq!(
        chosen.get_untracked(),
        Some("Banana"),
        "showing nothing chosen does not write over the bound value"
    );

    with_installed(harness.document_mut(), |_| {
        fruits.set(vec!["Banana", "Cherry", "Apple"]);
    });
    harness.frame(Vec::new());
    assert_eq!(shown(&harness), (Some("Banana"), Some("Banana")));
}
