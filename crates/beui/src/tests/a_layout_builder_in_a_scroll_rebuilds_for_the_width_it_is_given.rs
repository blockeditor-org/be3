use super::*;
use crate::reactive::{LayoutBuilder, ReadSignal, Text, build, clone, create_memo, view};
use crate::styled::Scroll;

#[test]
fn a_layout_builder_in_a_scroll_rebuilds_for_the_width_it_is_given() {
    let document = build(|| {
        view! {
            <Scroll>
                <Frame height=900.0>
                    <LayoutBuilder>
                        {move |size: ReadSignal<Vec2>| {
                            let wide = create_memo(clone!(size -> move || size.get().x >= 480.0));
                            let direction = create_memo(clone!(wide -> move || match wide.get() {
                                true => Direction::Horizontal,
                                false => Direction::Vertical,
                            }));
                            let label = create_memo(move || format!("{:.0} wide", size.get().x));
                            view! {
                                <List direction spacing=8.0>
                                    <Text string={label} @test_id={"width"} />
                                    <Text string="beside" />
                                </List>
                            }
                        }}
                    </LayoutBuilder>
                </Frame>
            </Scroll>
        }
    });
    let mut harness = Harness::sized(document, vec2(320.0, 240.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    for _ in 0..4 {
        let center = pos2(100.0, 100.0);
        harness.frame(vec![Event::PointerMoved(center)]);
    }
    *harness.viewport_mut() = vec2(640.0, 240.0);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
}
