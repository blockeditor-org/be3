use super::*;

fn line_edge(direction: LRDirection) -> EditorCommand<'static> {
    EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction,
        stop: CursorLeftRightStop::Line,
    }
}

#[test]
fn home_and_end_at_a_line_edge_stay_on_the_line() {
    let mut tester = EditorTester::new(b"one\ntwo\nthree");
    tester.set_cursor(tester.pos(5));
    tester.execute(line_edge(LRDirection::Left));
    tester.expect_content(b"one\n|two\nthree");
    tester.execute(line_edge(LRDirection::Left));
    tester.expect_content(b"one\n|two\nthree");
    tester.execute(line_edge(LRDirection::Right));
    tester.expect_content(b"one\ntwo|\nthree");
    tester.execute(line_edge(LRDirection::Right));
    tester.expect_content(b"one\ntwo|\nthree");
}
