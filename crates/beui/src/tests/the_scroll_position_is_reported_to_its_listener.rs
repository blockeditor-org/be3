use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn the_scroll_position_is_reported_to_its_listener() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(None);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset
                    @sizing=ItemSize::Percent(100.0)
                    on_change={move |position| reported.set(Some(position))}
                >
                    <List spacing=0.0>
                        <ForEach keys={indices(100)}>|index: usize|
                            <Text color=Color32::WHITE>{format!("Row {index}")}</Text>
                        </ForEach>
                    </List>
                </Offset>
            </List>
        }
    });
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());

    let position = reported
        .get()
        .expect("the scroll never reported a position");
    assert_eq!(position.offset, 0.0);
    assert_eq!(position.viewport, VIEWPORT.y);
    assert!(position.content > position.viewport);
}
