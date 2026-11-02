use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::SearchListApi;

#[test]
fn a_screen_reader_follows_the_highlight_through_a_search_list() {
    let _root = crate::reactive::RootScope::new();
    let list = ComponentRef::<SearchListApi<String>>::new();
    let items: Vec<PaletteItem> = (0..6)
        .map(|index| PaletteItem {
            key: format!("item{index}"),
            title: format!("Item {index}"),
            ..PaletteItem::default()
        })
        .collect();
    let document = build_node(move || {
        view! {
            <SearchList api={list} items columns=3 on_choose={|_: String| {}} />
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 400.0));
    harness.frame(Vec::new());
    harness.click(harness.center(list.get().search()));
    harness.frame(Vec::new());

    let read = |harness: &Harness| {
        ::beui_reactive::flush();
        let document = harness.document();
        let view = document
            .accessibility_view()
            .expect("the document has an accessibility tree");
        let search = view.id_of(list.get().search());
        let field = view.get(&search).expect("the search field is described");
        let selected: Vec<String> = list
            .get()
            .shown()
            .into_iter()
            .filter(|key| {
                let row = view.id_of(list.get().row(key).unwrap());
                view.get(&row)
                    .is_some_and(|row| row.is_selected() == Some(true))
            })
            .collect();
        let active = list.get().shown().into_iter().find(|key| {
            field.active_descendant() == Some(view.id_of(list.get().row(key).unwrap()))
        });
        (view.focus() == Some(search), selected, active)
    };

    assert_eq!(
        read(&harness),
        (true, vec!["item0".to_owned()], Some("item0".to_owned())),
        "the search field holds the focus, and names the highlighted option, which reads as selected"
    );

    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(list.get().highlighted().as_deref(), Some("item4"));
    assert_eq!(
        read(&harness),
        (true, vec!["item4".to_owned()], Some("item4".to_owned())),
        "the arrow keys move the selected option and the field's active descendant with the highlight"
    );
}
