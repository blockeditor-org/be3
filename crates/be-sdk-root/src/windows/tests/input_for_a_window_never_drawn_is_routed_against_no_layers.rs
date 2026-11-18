use super::*;

#[test]
fn input_for_a_window_never_drawn_is_routed_against_no_layers() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    let elsewhere = Some(7);
    assert!(
        windows
            .route(elsewhere, vec![button(150.0, true)])
            .is_empty(),
        "nothing is drawn in that window, so the press lands on nothing"
    );
    assert!(
        windows.routes(elsewhere),
        "its router holds the press until it lifts"
    );
    assert!(
        windows
            .route(elsewhere, vec![button(150.0, false)])
            .is_empty()
    );
    assert!(!windows.routes(elsewhere), "and then goes");
    assert!(
        windows.routes(None),
        "the window drawn keeps its router whatever another's input does"
    );
}
