use super::*;

#[test]
fn a_block_another_build_saved_offers_reset_delete_and_try_anyway() {
    let pressed = Pressed::default();
    let (reset, delete, try_anyway) = (
        pressing(&pressed, "reset"),
        pressing(&pressed, "delete"),
        pressing(&pressed, "try"),
    );
    let mut stepped = stepped(move || {
        view! {
            <StaleNotice
                title="Saved by another build"
                detail="Another build saved this block in a shape this build may read wrongly."
                on_reset={reset}
                on_delete={delete}
                on_try={try_anyway}
            />
        }
    });
    snapshot(&mut stepped, "a_block_another_build_saved");
    for button in [
        "stale-notice.reset",
        "stale-notice.delete",
        "stale-notice.try",
    ] {
        click(&mut stepped, button);
    }
    assert_eq!(*pressed.borrow(), ["reset", "delete", "try"]);
}
