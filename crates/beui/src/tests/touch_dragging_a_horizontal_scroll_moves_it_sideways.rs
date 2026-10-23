use super::*;
use crate::base::Direction;
use crate::input::TOUCH_DRAG_THRESHOLD;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn touch_dragging_a_horizontal_scroll_moves_it_sideways() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                >
                    <ForEach keys={indices(40)}>|index: usize|
                        <Frame width=120.0>
                            <LabelledButton
                                label={format!("Card {index}")}
                                on_click={move || clicks.update(|clicks| *clicks += 1)}
                            />
                        </Frame>
                    </ForEach>
                </Scroll>
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

    assert!(
        (harness.document().scroll_offset(strip) - (100.0 - TOUCH_DRAG_THRESHOLD)).abs() < 0.01
    );
    assert_eq!(clicks.get_untracked(), 0);
}
