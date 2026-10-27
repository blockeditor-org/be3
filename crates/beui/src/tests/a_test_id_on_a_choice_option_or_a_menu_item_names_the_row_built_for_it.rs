use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{ChoiceOption, MenuItem};

#[test]
fn a_test_id_on_a_choice_option_or_a_menu_item_names_the_row_built_for_it() {
    let _root = crate::reactive::RootScope::new();
    let chosen = create_signal(None::<&str>);
    let sorted = create_signal(Vec::new());
    let (document, [_group, menu]) = toolbar_of(move || {
        [
            view! {
                <RadioGroup
                    options={view! {
                        <ChoiceOption value="day" label="Day" @test_id="period.day" />
                        <ChoiceOption value="week" label="Week" @test_id="period.week" />
                    }}
                    selected={chosen}
                />
            },
            view! {
                <MenuButton
                    label="Sort"
                    items={view! {
                        <MenuItem label="By name" @test_id="sort.name" />
                        <MenuItem label="By date" @test_id="sort.date" />
                    }}
                    on_select={move |path: Vec<usize>| sorted.update(|sorted| sorted.push(path))}
                />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.settle();

    harness.click(harness.center(harness.find("period.week")));
    harness.settle();
    assert_eq!(chosen.get_untracked(), Some("week"));

    harness.click(harness.center(menu));
    harness.settle();
    harness.click(harness.center(harness.find("sort.date")));
    harness.settle();
    assert_eq!(sorted.get_untracked(), [vec![1]]);
}
