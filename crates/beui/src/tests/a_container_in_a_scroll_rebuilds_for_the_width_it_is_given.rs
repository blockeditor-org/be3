use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::*;

#[test]
fn a_container_in_a_scroll_rebuilds_for_the_width_it_is_given() {
    let document = build_node(|| {
        view! {
            <Scroll>
                <Frame height=900.0>
                    <Container>|size: ReadSignal<Vec2>| {
                        let wide = create_memo(move || size.get().x >= 480.0);
                        let direction = create_memo(move || match wide.get() {
                            true => Direction::Horizontal,
                            false => Direction::Vertical,
                        });
                        let label = create_memo(move || format!("{:.0} wide", size.get().x));
                        view! {
                            <List direction spacing=8.0>
                                <Text @test_id={"width"}>{label}</Text>
                                <Text>"beside"</Text>
                            </List>
                        }
                    }</Container>
                </Frame>
            </Scroll>
        }
    });
    let mut harness = Harness::sized(document, vec2(320.0, 240.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    for _ in 0..4 {
        let center = pos2(100.0, 100.0);
        harness.frame(vec![Input::PointerMoved(center)]);
    }
    harness.resize(vec2(640.0, 240.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
}
