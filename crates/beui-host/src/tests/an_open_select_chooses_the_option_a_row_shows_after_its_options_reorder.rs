use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn an_open_select_chooses_the_option_a_row_shows_after_its_options_reorder() {
    let _root = crate::reactive::RootScope::new();
    let fruits = create_signal(vec!["Apple", "Banana", "Cherry"]);
    let chosen = create_signal(None::<&str>);
    let (document, [select]) = toolbar_of(move || {
        [view! {
            <Select
                options={view! {
                    <ForEach value={fruits}>|fruit: &'static str|
                        <unstyled::ChoiceOption value=fruit label=fruit />
                    </ForEach>
                }}
                selected={chosen}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let trigger = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .trigger();
    harness.click(harness.center(trigger));
    harness.settle();
    let row = |harness: &Harness, label: &str| {
        let document = harness.document();
        document
            .layers()
            .iter()
            .filter_map(|layer| layer.overlay)
            .find_map(|overlay| text_within(document, overlay.id(), label))
            .unwrap_or_else(|| panic!("the popup shows {label}"))
    };
    row(&harness, "Apple");

    fruits.set(vec!["Cherry", "Banana", "Apple"]);
    harness.settle();
    harness.click(harness.center(row(&harness, "Apple")));
    harness.settle();
    assert_eq!(
        chosen.get_untracked(),
        Some("Apple"),
        "the row that shows Apple chooses Apple, not whatever moved into the place it was built at"
    );
}
