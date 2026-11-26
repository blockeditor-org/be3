use super::*;
use crate::reactive::{ForEach, ReadSignal, Show};

#[test]
fn for_each_rows_are_runs_that_move_whole_and_keyed_rows_follow_their_item() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let keys = create_signal(vec![1_u32, 2]);
    let marked = create_signal(false);
    let page = scope.run(|| {
        view! {
            <Element tag="ul">
                <ForEach value={keys}>|key: u32|
                    <Element tag="b" text={key.to_string()} />
                    <Show value={marked}>
                        <Element tag="i" text={format!("{key}!")} />
                    </Show>
                </ForEach>
            </Element>
        }
    });
    let text = |page: &DomNode| {
        let html = page.html();
        html.split('>')
            .filter_map(|part| part.split('<').next())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(text(&page), "1 2");
    marked.set(true);
    assert_eq!(
        text(&page),
        "1 1! 2 2!",
        "a row is a run: what it adds stays with it"
    );
    keys.set(vec![2, 1]);
    assert_eq!(
        text(&page),
        "2 2! 1 1!",
        "reordering moves each row's whole run"
    );
    marked.set(false);
    assert_eq!(text(&page), "2 1");

    #[derive(Clone, PartialEq)]
    struct Item {
        id: u32,
        name: &'static str,
    }
    let items = create_signal(vec![
        Item { id: 1, name: "one" },
        Item { id: 2, name: "two" },
    ]);
    let built = std::rc::Rc::new(std::cell::Cell::new(0));
    let page = scope.run(|| {
        let built = built.clone();
        view! {
            <Element tag="ol">
                <ForEach
                    value={items}
                    key={|item: &Item| item.id}
                >|item: ReadSignal<Item>, index: ReadSignal<usize>| {
                    built.set(built.get() + 1);
                    view! {
                        <Element
                            tag="li"
                            text={create_memo(move || format!("{}:{}", index.get(), item.get().name))}
                        />
                    }
                }</ForEach>
            </Element>
        }
    });
    assert_eq!(text(&page), "0:one 1:two");
    items.set(vec![
        Item {
            id: 2,
            name: "deux",
        },
        Item { id: 1, name: "one" },
    ]);
    assert_eq!(
        text(&page),
        "0:deux 1:one",
        "a keyed row is kept, and its item and index change in place"
    );
    assert_eq!(built.get(), 2, "no row was built again");
}
