use super::*;

fn delete_line(direction: LRDirection) -> EditorCommand<'static> {
    EditorCommand::Delete {
        direction,
        stop: CursorLeftRightStop::Line,
    }
}

#[test]
fn a_delete_to_the_lines_edge_takes_only_the_break_when_already_there() {
    let mut tester = EditorTester::new(b"one\ntwo three\nfour");
    tester.set_cursor(tester.pos(7));
    tester.execute(delete_line(LRDirection::Left));
    tester.expect_content(b"one\n| three\nfour");
    tester.execute(delete_line(LRDirection::Left));
    tester.expect_content(b"one| three\nfour");

    tester.execute(delete_line(LRDirection::Right));
    tester.expect_content(b"one|\nfour");
    tester.execute(delete_line(LRDirection::Right));
    tester.expect_content(b"one|four");
}
