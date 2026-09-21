use super::*;
use crate::reactive::{ItemSize, create_memo, create_signal, with_reactive_scope};

#[test]
fn a_row_added_to_a_for_each_keeps_the_sizes_the_rows_beside_it_chose() {
    let (keys, set_keys) = create_signal(vec![0usize]);
    let (shown, set_shown) = create_signal(0usize);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <ForEach keys={keys}>
                    {move |key: usize| {
                        let shown = shown.clone();
                        let sizing = create_memo(move || match shown.get() == key {
                            true => ItemSize::Percent(100.0),
                            false => ItemSize::Fixed(0.0),
                        });
                        view! {
                            <Frame @sizing={sizing} @test_id={format!("row.{key}")} />
                        }
                    }}
                </ForEach>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(harness.find("row.0")).height(),
        VIEWPORT.y,
        "the only row takes the whole list"
    );

    let added = set_keys.clone();
    let showing = set_shown.clone();
    with_reactive_scope(harness.document_mut(), move || {
        added.set(vec![0, 1]);
        showing.set(1);
    });
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), move || {
        set_keys.set(vec![0, 1, 2]);
        set_shown.set(2);
    });
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(harness.find("row.0")).height(),
        0.0,
        "the row that gave up its share keeps the size it chose when another row is added"
    );
    assert_eq!(
        harness.rect(harness.find("row.1")).height(),
        0.0,
        "the row that gave up its share last keeps the size it chose as well"
    );
    assert_eq!(
        harness.rect(harness.find("row.2")).height(),
        VIEWPORT.y,
        "the row that was added takes the share the others gave up"
    );
}
