use super::*;
use crate::drag::Carry;
use crate::primitives::{Frame, Interactive, List};
use crate::reactive::*;
use crate::unstyled::Scroll;

const RED: Color32 = Color32::from_rgb(200, 30, 30);

/// The red rects drawn, each with the clip it is drawn in.
fn drawn(harness: &mut Harness) -> Vec<(Rect, Rect)> {
    harness
        .frame(Vec::new())
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Rect {
                rect, color, clip, ..
            } if *color == RED => Some((*rect, *clip)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_carried_item_its_scroll_cuts_is_drawn_whole_and_put_down_only_at_home() {
    let _root = RootScope::new();
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <List spacing=0.0>
                <Frame width=100.0 height=60.0>
                    <Scroll style={plain_scroll()}>
                        <List spacing=0.0>
                            <Frame width=100.0 height=40.0 />
                            <Interactive
                                drag={Func::new(|_: PointerPress| Some(Carry::new(1_u32)))}
                            >
                                <Frame @test_id="item" width=100.0 height=40.0 fill=RED />
                            </Interactive>
                        </List>
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let home = harness.rect(harness.find("item"));
    let [(at_home, cut)] = drawn(&mut harness)[..] else {
        panic!("the item is drawn once at home");
    };
    assert_eq!(at_home, home);
    assert!(
        cut.max.y < home.max.y,
        "and its scroll cuts its lower half there"
    );

    let grab = home.min + vec2(50.0, 10.0);
    let by = vec2(200.0, 40.0);
    harness.down(grab, PointerButton::Primary);
    for step in 1..=4 {
        harness.frame(vec![Input::PointerMoved(grab + by * (step as f32 / 4.0))]);
    }
    harness.settle();
    assert_eq!(
        harness.rect(harness.find("item")),
        home,
        "carried, it keeps its place"
    );
    let [(carried, clip)] = drawn(&mut harness)[..] else {
        panic!("carried, the item is drawn once, by its replica, and not at home");
    };
    assert_eq!(carried, home.translate(by), "where it floats");
    assert_eq!(
        carried.intersect(clip),
        carried,
        "whole, its lower half too"
    );

    harness.up(grab + by, PointerButton::Primary);
    harness.settle();
    let [(put_down, _)] = drawn(&mut harness)[..] else {
        panic!("put down, the item is drawn once");
    };
    assert_eq!(put_down, home, "at home only");
}
