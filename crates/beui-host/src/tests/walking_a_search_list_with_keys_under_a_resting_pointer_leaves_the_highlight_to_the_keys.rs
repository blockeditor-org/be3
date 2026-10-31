use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::SearchListApi;

#[test]
fn walking_a_search_list_with_keys_under_a_resting_pointer_leaves_the_highlight_to_the_keys() {
    let _root = crate::reactive::RootScope::new();
    let list = ComponentRef::<SearchListApi<String>>::new();
    let query = create_signal(String::new());
    let items: Vec<PaletteItem> = (0..40)
        .map(|index| PaletteItem {
            key: format!("item{index:02}"),
            title: format!("Item {index:02}"),
            ..PaletteItem::default()
        })
        .collect();
    let document = build_node(move || {
        view! {
            <SearchList api={list} items query list_height=200.0 on_choose={|_: String| {}} />
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(300.0, 300.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let row_under = |harness: &Harness, pos: Pos2| {
        list.get()
            .shown()
            .into_iter()
            .find(|key| harness.rect(list.get().row(key).unwrap()).contains(pos))
    };

    harness.click(harness.center(list.get().search()));
    let rest = harness.center(list.get().row(&"item02".to_owned()).unwrap());
    harness.frame(vec![Input::PointerMoved(rest)]);
    harness.frame(Vec::new());
    assert_eq!(
        list.get().highlighted().as_deref(),
        Some("item02"),
        "moving the pointer onto a row should highlight it"
    );

    for _ in 0..15 {
        harness.key(Key::ArrowDown, Modifiers::NONE);
        harness.frame(Vec::new());
    }
    harness.frame(Vec::new());
    let under = row_under(&harness, rest);
    assert_ne!(
        under.as_deref(),
        Some("item02"),
        "the list should have scrolled rows under the resting pointer"
    );
    assert_eq!(
        list.get().highlighted().as_deref(),
        Some("item17"),
        "a row that scrolled under the resting pointer took the highlight from the keys"
    );

    harness.frame(vec![Input::PointerMoved(rest + Vec2::new(1.0, 0.0))]);
    harness.frame(Vec::new());
    assert_eq!(
        list.get().highlighted(),
        row_under(&harness, rest + Vec2::new(1.0, 0.0)),
        "moving the pointer again should highlight the row under it"
    );
}
