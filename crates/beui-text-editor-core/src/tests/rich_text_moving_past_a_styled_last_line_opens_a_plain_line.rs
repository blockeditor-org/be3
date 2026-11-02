use super::*;

fn down() -> EditorCommand<'static> {
    EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        mode: VerticalMoveMode::Move,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    }
}

fn up() -> EditorCommand<'static> {
    EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        mode: VerticalMoveMode::Move,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    }
}

#[test]
fn rich_text_moving_past_a_styled_last_line_opens_a_plain_line() {
    let mut tester = EditorTester::rich(b"intro\nfn main() {}");
    tester.set_cursor(tester.pos(8));
    tool(&mut tester, "code_block");
    tester.execute(down());
    tester.expect_content(b"intro\nfn main() {}|");
    assert_eq!(
        formatted(&tester),
        "intro\n[code_block=rust]fn main() {}",
        "the first Down only reaches the end of the last line"
    );

    tester.execute(down());
    tester.expect_content(b"intro\nfn main() {}\n|");
    assert_eq!(formatted(&tester), "intro\n[code_block=rust]fn main() {}\n");
    tester.execute(EditorCommand::InsertText(b"after"));
    assert_eq!(
        formatted(&tester),
        "intro\n[code_block=rust]fn main() {}\nafter"
    );
    tester.execute(down());
    tester.expect_content(b"intro\nfn main() {}\nafter|");

    tester.execute(EditorCommand::Undo);
    tester.execute(EditorCommand::Undo);
    assert_eq!(formatted(&tester), "intro\n[code_block=rust]fn main() {}");

    tester.set_cursor(tester.pos(0));
    tester.execute(up());
    tester.expect_content(b"|intro\nfn main() {}");
    tool(&mut tester, "heading1");
    tester.execute(up());
    tester.expect_content(b"|\nintro\nfn main() {}");
    assert_eq!(
        formatted(&tester),
        "\n[heading=1]intro\n[code_block=rust]fn main() {}",
        "Up from the start of a styled first line opens a plain line above it"
    );
}
