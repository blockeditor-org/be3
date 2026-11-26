use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_glide_on_a_signal_glides_when_the_signal_changes_with_the_move() {
    let _root = RootScope::new();
    let above = create_signal(0.0_f32);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height={above} />
                <styled::Glide @test_id="slot" on={above}>
                    <Frame @test_id="row" height=GLIDE_ROW />
                </styled::Glide>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    assert_eq!(top_of(&harness, "row"), 0.0);

    above.set(40.0);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "slot"),
        40.0,
        "the glide rests in its new place"
    );
    assert_eq!(
        top_of(&harness, "row"),
        0.0,
        "but the signal it follows changed with the move, so its row is still where it was"
    );
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let moving = top_of(&harness, "row");
    assert!(
        moving > 0.0 && moving < 40.0,
        "it is on its way, at {moving}"
    );
    harness.settle();
    assert_eq!(top_of(&harness, "row"), 40.0);
}
