use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_glide_on_a_trigger_glides_a_row_pushed_down_by_what_grows_above_it() {
    let _root = RootScope::new();
    let above = create_signal(0.0_f32);
    let grown = create_trigger();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height={above} />
                <styled::Glide @test_id="slot" on={grown}>
                    <Frame @test_id="row" height=GLIDE_ROW />
                </styled::Glide>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    assert_eq!(top_of(&harness, "row"), 0.0);

    above.set(40.0);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row"),
        40.0,
        "a move the trigger did not announce lands at once"
    );

    above.set(80.0);
    grown.notify();
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "slot"),
        80.0,
        "the glide rests in its new place"
    );
    assert_eq!(
        top_of(&harness, "row"),
        40.0,
        "but its row, moved by no key, is still painted where it was"
    );
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let moving = top_of(&harness, "row");
    assert!(
        moving > 40.0 && moving < 80.0,
        "it is on its way, at {moving}"
    );
    harness.settle();
    assert_eq!(top_of(&harness, "row"), 80.0);
}
