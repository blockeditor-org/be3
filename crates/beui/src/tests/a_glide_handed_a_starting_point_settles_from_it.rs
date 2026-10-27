use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_glide_handed_a_starting_point_settles_from_it() {
    let _root = RootScope::new();
    let shown = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=100.0 />
                <Show condition={shown}>
                    <Glide @test_id="slot" from={Some(pos2(200.0, 300.0))}>
                        <Frame @test_id="row" width=50.0 height=GLIDE_ROW fill=Color32::WHITE />
                    </Glide>
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();

    shown.set(true);
    harness.frame(Vec::new());
    assert_eq!(top_of(&harness, "slot"), 100.0, "the glide rests below the spacer");
    assert_eq!(
        harness.rect(harness.find("row")).min,
        pos2(200.0, 300.0),
        "but its row first paints at the point it was handed, in window coordinates"
    );
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let on_its_way = harness.rect(harness.find("row")).min;
    assert!(
        on_its_way.x < 200.0 && on_its_way.y < 300.0 && on_its_way.y > 100.0,
        "then glides towards where it rests, and is at {on_its_way:?}"
    );
    harness.frames_to_settle();
    assert_eq!(harness.rect(harness.find("row")).min, pos2(0.0, 100.0));
}
