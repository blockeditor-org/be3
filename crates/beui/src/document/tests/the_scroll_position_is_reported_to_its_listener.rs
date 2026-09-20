use super::*;
use crate::reactive::{ForEach, ItemSize, List, Scroll, build};

#[test]
fn the_scroll_position_is_reported_to_its_listener() {
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    on_change={move |position| sink.set(Some(position))}
                >
                    <ForEach keys={indices(100)}>
                        {|index: usize| view! {
                            <Text
                                string={format!("Row {index}")}
                                font_size=14.0
                                color=Color32::WHITE
                            />
                        }}
                    </ForEach>
                </Scroll>
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
