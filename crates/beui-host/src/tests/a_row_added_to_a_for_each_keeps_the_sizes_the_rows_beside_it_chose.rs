use super::*;
use crate::reactive::*;

#[test]
fn a_row_added_to_a_for_each_keeps_the_sizes_the_rows_beside_it_chose() {
    let _root = crate::reactive::RootScope::new();
    let keys = create_signal(vec![0usize]);
    let shown = create_signal(0usize);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ForEach keys={keys}>|key: usize| {
                    let sizing = create_memo(move || match shown.get() == key {
                        true => ItemSize::Percent(100.0),
                        false => ItemSize::Fixed(0.0),
                    });
                    view! {
                        <Frame @crate::reactive::sizing={sizing} @test_id={format!("row.{key}")} />
                    }
                }</ForEach>
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

    keys.set(vec![0, 1]);
    shown.set(1);
    harness.frame(Vec::new());
    keys.set(vec![0, 1, 2]);
    shown.set(2);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(harness.find("row.0")).height(),
        0.0,
        "the row that gave up its share keeps the size it chose when another row is set_keys"
    );
    assert_eq!(
        harness.rect(harness.find("row.1")).height(),
        0.0,
        "the row that gave up its share last keeps the size it chose as well"
    );
    assert_eq!(
        harness.rect(harness.find("row.2")).height(),
        VIEWPORT.y,
        "the row that was set_keys takes the share the others gave up"
    );
}
