use super::*;

#[test]
fn a_child_that_opens_fails_or_is_refused_takes_no_frame() {
    // A live child would take its placer's frame, but one whose editor is still opening, whose
    // app failed or that is refused (open above it) draws no screen: its placer keeps its
    // chrome, as be-host draws a child's frame while it loads.
    let opening = up(1, ChildMode::Live);
    let failed = up(2, ChildMode::Live);
    *failed.shared.showing.borrow_mut() = Showing::Failed("the app stopped".to_owned());
    let refused = up(3, ChildMode::Live);
    *refused.shared.showing.borrow_mut() = Showing::Failed(ALREADY_OPEN_ABOVE.to_owned());
    let children = placing(vec![opening, failed, refused]);
    for id in [1, 2, 3] {
        assert!(!children.shows_frame(EditorInstanceId(1), ViewId::MAIN, ChildId(id)));
    }
}
