use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use crate::styled::{Frame, Scroll};
use crate::unstyled::SearchListApi;

#[test]
fn typing_into_a_search_list_in_a_page_leaves_the_page_still_until_a_key_moves_the_highlight() {
    let _root = crate::reactive::RootScope::new();
    let page = NodeRef::new();
    let list = ComponentRef::<SearchListApi<String>>::new();
    let query = create_signal(String::new());
    let items: Vec<PaletteItem> = (0..20)
        .map(|index| PaletteItem {
            key: format!("item{index:02}"),
            title: format!("Item {index:02}"),
            ..PaletteItem::default()
        })
        .collect();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&page @sizing=ItemSize::Percent(100.0)>
                    <List spacing=0.0>
                        <Frame height=Size::Custom(150.0) />
                        <SearchList
                            api={list}
                            items
                            query
                            list_height=2000.0
                            on_choose={|_: String| {}}
                        />
                    </List>
                </Scroll>
            </List>
        }
    });
    let page = page.get();
    let mut harness = Harness::sized(document, Vec2::new(300.0, 200.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let first = list.get().row(&"item00".to_owned()).unwrap();
    assert!(
        harness.rect(first).bottom() > 200.0,
        "the first row should start below the page's view"
    );
    assert_eq!(harness.document().scroll_offset(page), 0.0);

    with_installed(harness.document_mut(), |_| query.set("1".to_owned()));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(list.get().highlighted().as_deref(), Some("item01"));
    assert_eq!(
        harness.document().scroll_offset(page),
        0.0,
        "highlighting the first match by itself scrolled the page"
    );

    let search = list.get().search();
    harness.click(harness.center(search));
    harness.frame(vec![Input::PointerLeft]);
    for _ in 0..6 {
        harness.key(Key::ArrowDown, Modifiers::NONE);
    }
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(list.get().highlighted().as_deref(), Some("item15"));
    let row = harness.rect(list.get().row(&"item15".to_owned()).unwrap());
    assert!(
        row.top() >= 0.0 && row.bottom() <= 200.0,
        "the row a key moved to sits at {row:?}, outside the page's view"
    );
}
