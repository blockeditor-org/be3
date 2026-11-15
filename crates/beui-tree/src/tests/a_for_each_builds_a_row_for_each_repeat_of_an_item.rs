use super::*;
use crate::reactive::{ForEach, ReadSignal};

#[test]
fn a_for_each_builds_a_row_for_each_repeat_of_an_item() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let items = create_signal(vec!["a", "b", "a"]);
    let page = scope.run(|| {
        view! {
            <Element tag="ol">
                <ForEach value={items}>|item: &'static str, index: ReadSignal<usize>|
                    <Element
                        tag="li"
                        text={create_memo(move || format!("{}{item}", index.get()))}
                    />
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
    assert_eq!(
        text(&page),
        "0a 1b 2a",
        "each repeat is a row of its own, built at its own place"
    );
    items.set(vec!["a", "a", "b", "a"]);
    assert_eq!(text(&page), "0a 1a 2b 3a");
    items.set(vec!["b", "a"]);
    assert_eq!(text(&page), "0b 1a");
}
