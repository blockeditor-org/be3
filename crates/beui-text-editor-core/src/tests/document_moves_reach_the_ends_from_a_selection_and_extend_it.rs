use super::*;

fn document_edge(direction: LRDirection, mode: MoveMode) -> EditorCommand<'static> {
    EditorCommand::MoveCursorLeftRight {
        mode,
        direction,
        stop: CursorLeftRightStop::Document,
    }
}

#[test]
fn document_moves_reach_the_ends_from_a_selection_and_extend_it() {
    let mut tester = EditorTester::new(b"one\ntwo\nthree");
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(5),
        focus: tester.pos(6),
    });
    tester.execute(document_edge(LRDirection::Left, MoveMode::Move));
    tester.expect_content(b"|one\ntwo\nthree");
    tester.execute(document_edge(LRDirection::Right, MoveMode::Move));
    tester.expect_content(b"one\ntwo\nthree|");

    tester.set_cursor(tester.pos(5));
    tester.execute(document_edge(LRDirection::Right, MoveMode::Select));
    tester.expect_content(b"one\nt[wo\nthree|");
    tester.execute(document_edge(LRDirection::Left, MoveMode::Select));
    tester.expect_content(b"|one\nt]wo\nthree");

    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(0),
        focus: tester.pos(1),
    });
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"[o|ne\ntw[o|\nthree");
    tester.execute(document_edge(LRDirection::Right, MoveMode::Move));
    tester.expect_content(b"one\ntwo\nthree|");
}
