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
                    <ForEach keys={fruits}>|fruit: &'static str|
                        <unstyled::ChoiceOption
                            value=fruit
                            label=fruit
                            @test_id={format!("fruit.{fruit}")}
                        />
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
    assert!(harness.shown("fruit.Apple"), "the popup lists the options");

    fruits.set(vec!["Cherry", "Banana", "Apple"]);
    harness.settle();
    let apple = harness.find("fruit.Apple");
    assert!(
        text_within(harness.document(), apple, "Apple").is_some(),
        "the row keeps showing the option it was built for"
    );
    harness.click(harness.center(apple));
    harness.settle();
    assert_eq!(
        chosen.get_untracked(),
        Some("Apple"),
        "the row named Apple chooses Apple, not whatever moved into the place it was built at"
    );
}
