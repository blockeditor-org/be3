use super::*;

#[test]
fn a_mock_dom_row_that_leaves_disposes_the_effects_it_built() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let texts = create_signal(0);
    let cleanups = create_signal(0);
    let suffix = create_signal("!".to_string());
    let items = create_signal(vec![1, 2]);
    let list = scope.run(|| {
        view! {
            <Element tag="ul">
                <ForEach keys={items}>|item: u32| {
                    on_cleanup(move || cleanups.update(|cleanups| *cleanups += 1));
                    let text = create_memo(move || format!("{item}{}", suffix.get()));
                    view! {
                        <Element tag="li" text={text} texts={texts} />
                    }
                }</ForEach>
            </Element>
        }
    });
    assert_eq!(
        list.html(),
        "<ul id=0><li id=1>1!</li><li id=2>2!</li></ul>"
    );
    assert_eq!(texts.get_untracked(), 2);

    items.set(vec![2]);
    assert_eq!(list.html(), "<ul id=0><li id=2>2!</li></ul>");
    assert_eq!(
        cleanups.get_untracked(),
        1,
        "the row that left must dispose the scope it was built in"
    );

    suffix.set("?".to_string());
    assert_eq!(list.html(), "<ul id=0><li id=2>2?</li></ul>");
    assert_eq!(
        texts.get_untracked(),
        3,
        "only the row that stayed may run its effects again"
    );

    drop(scope);
    assert_eq!(
        cleanups.get_untracked(),
        2,
        "disposing the scope that built the tree disposes the rows it still holds"
    );
}
