use super::*;
use crate::base::Direction;
use crate::reactive::{ItemSize, List, Scroll, build, view};

#[test]
fn touch_dragging_a_horizontal_scroll_moves_it_sideways() {
    let clicks = Rc::new(Cell::new(0));
    let click_sink = clicks.clone();
    let document = build(move || {
        let items = (0..40)
            .map(|index| {
                let click_sink = click_sink.clone();
                view! {
                    <Frame width=120.0>
                        <LabelledButton
                            label={format!("Card {index}")}
                            on_click={move || click_sink.set(click_sink.get() + 1)}
                        />
                    </Frame>
                }
            })
            .collect::<Vec<_>>();
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                    children={items}
                />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let strip = harness.find("strip");

    let start = pos2(300.0, 150.0);
    let end = pos2(200.0, 150.0);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, end);
    harness.touch(TouchPhase::End, end);

    assert!((harness.document().scroll_offset(strip) - 100.0).abs() < 0.01);
    assert_eq!(clicks.get(), 0);
}
