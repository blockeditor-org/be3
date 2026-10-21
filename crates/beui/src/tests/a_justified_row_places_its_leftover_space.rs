use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_justified_row_places_its_leftover_space() {
    let _root = crate::reactive::RootScope::new();
    assert_eq!(lefts(Justify::Start), [0.0, 20.0, 40.0]);
    assert_eq!(lefts(Justify::Center), [60.0, 80.0, 100.0]);
    assert_eq!(lefts(Justify::End), [120.0, 140.0, 160.0]);
    assert_eq!(lefts(Justify::SpaceBetween), [0.0, 80.0, 160.0]);
    assert_eq!(lefts(Justify::SpaceAround), [20.0, 80.0, 140.0]);
    assert_eq!(lefts(Justify::SpaceEvenly), [30.0, 80.0, 130.0]);
}

fn lefts(justify: Justify) -> [f32; 3] {
    let tiles = [NodeRef::new(), NodeRef::new(), NodeRef::new()];
    let document = build_node({
        let [first, second, third] = tiles;
        move || {
            view! {
                <List direction=Direction::Horizontal justify spacing=0.0>
                    <Frame @node_ref=&first width=20.0 height=10.0 />
                    <Frame @node_ref=&second width=20.0 height=10.0 />
                    <Frame @node_ref=&third width=20.0 height=10.0 />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(180.0, 50.0));
    harness.frame(Vec::new());
    tiles.map(|tile| harness.rect(tile.get()).left())
}
