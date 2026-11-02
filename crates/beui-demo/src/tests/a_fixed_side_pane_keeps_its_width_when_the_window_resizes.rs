use super::*;

#[test]
fn a_fixed_side_pane_keeps_its_width_when_the_window_resizes() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let note = test.rect_of("bars.note");
    let stage = test.rect_of("bars.stage");

    test.resize(Vec2::new(WIDE.x + 300.0, WIDE.y));
    test.frame(Vec::new());
    test.settle();
    let wider = test.rect_of("bars.note");
    assert_eq!(
        wider.width(),
        note.width(),
        "the side pane keeps its width in points: {wider:?} from {note:?}"
    );
    assert_eq!(
        test.rect_of("bars.stage").width(),
        stage.width() + 300.0,
        "the stage takes all the room the window gained"
    );
    test.snapshot("a_fixed_side_pane_keeps_its_width_when_the_window_resizes");
}
