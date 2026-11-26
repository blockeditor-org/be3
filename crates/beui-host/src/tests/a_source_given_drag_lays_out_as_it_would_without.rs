use super::*;
use crate::drag::Carry;
use crate::primitives::*;
use crate::reactive::*;

/// What an `Interactive` given `drag` wraps its children in takes no room of its own: a
/// child it stretches, grows or fills is as long as without it, in either direction.
#[test]
fn a_source_given_drag_lays_out_as_it_would_without() {
    let _root = RootScope::new();
    let document = build_node(move || {
        crate::unstyled::drag_board();
        let lift = || Func::new(|_: PointerPress| Some(Carry::new(1_u32)));
        view! {
            <List spacing=10.0>
                <Interactive>
                    <Frame @test_id="plain.stretched" height=30.0 />
                </Interactive>
                <Interactive drag={lift()}>
                    <Frame @test_id="drag.stretched" height=30.0 />
                </Interactive>
                <List direction=Direction::Horizontal spacing=10.0>
                    <Frame width=50.0 height=30.0 />
                    <ListItem grow=1.0>
                        <Interactive>
                            <Frame @test_id="plain.grown" height=30.0 />
                        </Interactive>
                    </ListItem>
                </List>
                <List direction=Direction::Horizontal spacing=10.0>
                    <Frame width=50.0 height=30.0 />
                    <ListItem grow=1.0>
                        <Interactive drag={lift()}>
                            <Frame @test_id="drag.grown" height=30.0 />
                        </Interactive>
                    </ListItem>
                </List>
                <Frame height=60.0>
                    <Interactive>
                        <Frame @test_id="plain.filled" height={Extent::fraction(1.0)} />
                    </Interactive>
                </Frame>
                <Frame height=60.0>
                    <Interactive drag={lift()}>
                        <Frame @test_id="drag.filled" height={Extent::fraction(1.0)} />
                    </Interactive>
                </Frame>
                <Frame @test_id="after" height=10.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let size = |id: &str| harness.rect(harness.find(id)).size();
    assert_eq!(
        size("drag.stretched"),
        size("plain.stretched"),
        "a stretched child"
    );
    assert_eq!(size("drag.grown"), size("plain.grown"), "a grown child");
    assert_eq!(
        size("drag.filled"),
        size("plain.filled"),
        "a child that fills its height, as a canvas does"
    );
    assert_eq!(size("drag.filled").y, 60.0);
    assert_eq!(
        top_of(&harness, "drag.stretched") - top_of(&harness, "plain.stretched"),
        40.0,
        "and the wrapper adds no height and no gap"
    );
    assert_eq!(top_of(&harness, "after"), 4.0 * 40.0 + 2.0 * 70.0);
}
