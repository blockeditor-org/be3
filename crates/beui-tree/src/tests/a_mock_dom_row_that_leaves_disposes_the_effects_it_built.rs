use super::*;

#[test]
fn a_mock_dom_row_that_leaves_disposes_the_effects_it_built() {
    let scope = Scope::new();
    let texts = Rc::new(Cell::new(0));
    let cleanups = Rc::new(Cell::new(0));
    let (suffix, set_suffix) = create_signal("!".to_string());
    let (items, set_items) = create_signal(vec![1, 2]);
    let list = scope.run(|| {
        let texts = texts.clone();
        let cleanups = cleanups.clone();
        view! {
            <Element tag="ul">
                <ForEach keys={items}>
                    {move |item: u32| {
                        let cleanups = cleanups.clone();
                        let suffix = suffix.clone();
                        on_cleanup(move || cleanups.set(cleanups.get() + 1));
                        let text = create_memo(move || format!("{item}{}", suffix.get()));
                        view! {
                            <Element tag="li" text={text} texts={texts.clone()} />
                        }
                    }}
                </ForEach>
            </Element>
        }
    });
    assert_eq!(list.html(), "<ul><li>1!</li><li>2!</li></ul>");
    assert_eq!(texts.get(), 2);

    set_items.set(vec![2]);
    assert_eq!(list.html(), "<ul><li>2!</li></ul>");
    assert_eq!(
        cleanups.get(),
        1,
        "the row that left must dispose the scope it was built in"
    );

    set_suffix.set("?".to_string());
    assert_eq!(list.html(), "<ul><li>2?</li></ul>");
    assert_eq!(
        texts.get(),
        3,
        "only the row that stayed may run its effects again"
    );

    drop(scope);
    list.release();
    assert_eq!(
        cleanups.get(),
        2,
        "releasing the tree disposes the rows it still holds"
    );
}
