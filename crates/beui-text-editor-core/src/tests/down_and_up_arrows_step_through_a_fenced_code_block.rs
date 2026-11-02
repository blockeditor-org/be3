use super::*;

#[test]
fn down_and_up_arrows_step_through_a_fenced_code_block() {
    let content = "```ts\nHello\n```\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let hello_end = content.find("Hello").unwrap() + "Hello".len();
    let fence_end = content.rfind("```").unwrap() + 3;

    tester.set_cursor(tester.pos(hello_end));
    let step = |tester: &mut EditorTester, direction| {
        tester.execute(EditorCommand::MoveCursorUpDown {
            direction,
            mode: VerticalMoveMode::Move,
            metric: CursorHorizontalPositionMetric::Byte,
            stop: CursorLeftRightStop::UnicodeGraphemeCluster,
        });
        let cursor = tester.editor.cursor_positions()[0];
        tester.editor.position_index(cursor.pos.focus).unwrap()
    };

    assert_eq!(step(&mut tester, UDDirection::Down), fence_end);
    assert_eq!(step(&mut tester, UDDirection::Down), content.len());
    assert_eq!(step(&mut tester, UDDirection::Up), fence_end);
    assert_eq!(step(&mut tester, UDDirection::Up), hello_end);
}
