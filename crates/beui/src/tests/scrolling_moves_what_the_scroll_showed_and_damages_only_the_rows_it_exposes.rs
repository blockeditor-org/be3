use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn scrolling_moves_what_the_scroll_showed_and_damages_only_the_rows_it_exposes() {
    let document = build_node(|| {
        view! {
            <Frame fill=Color32::BLACK radius=0.0>
                <List spacing=0.0>
                    <Frame height=200.0>
                        <Scroll @test_id="scroll">
                            <ForEach keys={(0..30).collect::<Vec<usize>>()}>|row: usize|
                                <Frame height=40.0>
                                    <Text>{format!("row {row}")}</Text>
                                </Frame>
                            </ForEach>
                        </Scroll>
                    </Frame>
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let scroll = harness.find("scroll");
    let viewport = harness.rect(scroll);
    harness.frame(vec![Input::PointerMoved(viewport.center())]);

    let output = harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -40.0)))]);
    assert_eq!(harness.document().scroll_offset(scroll), 40.0);
    let moved = output
        .main()
        .moved()
        .expect("scrolling moves what the scroll already showed");
    assert_eq!(moved.by, vec2(0.0, -40.0));
    assert_eq!(moved.to().height(), viewport.height() - 40.0);
    let damaged: f32 = output
        .main()
        .damage
        .rects()
        .iter()
        .map(|rect| area(rect.intersect(moved.to())))
        .sum();
    assert!(
        damaged < area(moved.to()) / 4.0,
        "the rows the scroll moved keep their pixels: {damaged} of {} were damaged",
        area(moved.to())
    );
}

fn area(rect: Rect) -> f32 {
    match rect.is_positive() {
        true => rect.width() * rect.height(),
        false => 0.0,
    }
}
