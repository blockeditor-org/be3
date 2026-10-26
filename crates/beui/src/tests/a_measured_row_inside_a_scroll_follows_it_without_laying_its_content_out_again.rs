use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_measured_row_inside_a_scroll_follows_it_without_laying_its_content_out_again() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(Rect::NOTHING);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=200.0>
                    <Scroll @test_id="scroll">
                        <ForEach keys={(0..20).collect::<Vec<usize>>()}>|row: usize|
                            <Row watched={row == 2} seen={seen} />
                        </ForEach>
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(seen.get_untracked().top(), 80.0);
    let scroll = harness.find("scroll");
    harness.frame(vec![Input::PointerMoved(harness.rect(scroll).center())]);

    harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -30.0)))]);
    assert_eq!(harness.document().scroll_offset(scroll), 30.0);
    assert_eq!(
        seen.get_untracked().top(),
        50.0,
        "a measured row hears where its scroll moved it"
    );
    let latest = &harness.document().performance().latest;
    assert_eq!(
        latest.layout_passes, 1,
        "following the scroll takes no second layout"
    );
    assert!(
        latest.work.placed < 20,
        "the measured rows are not laid out again, but {} nodes were",
        latest.work.placed
    );
}

#[component]
fn Row(watched: bool, seen: Signal<Rect>) -> NodeId {
    let row = view! {
        <Frame height=40.0 fill=Color32::WHITE radius=0.0 />
    };
    let measured = measure(row);
    create_effect(move || {
        if let Some(placed) = measured.get()
            && watched
        {
            seen.set(placed);
        }
    });
    row
}
