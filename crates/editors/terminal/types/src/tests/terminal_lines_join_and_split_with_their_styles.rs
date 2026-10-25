use crate::terminal::{TerminalColor, TerminalLine, TerminalStyle, logical_lines};

#[test]
fn terminal_lines_join_and_split_with_their_styles() {
    let red = TerminalStyle {
        foreground: Some(TerminalColor { r: 255, g: 0, b: 0 }),
        ..TerminalStyle::default()
    };
    let mut first = TerminalLine::default();
    first.push("plain ", TerminalStyle::default());
    first.push("red", red);
    first.wrapped = true;
    let mut second = TerminalLine::default();
    second.push("more red", red);
    second.push(" tail", TerminalStyle::default());

    first.join(&second);
    assert_eq!(first.text, "plain redmore red tail");
    assert!(!first.wrapped);
    assert_eq!(
        first.spans(),
        vec![
            ("plain ", TerminalStyle::default()),
            ("redmore red", red),
            (" tail", TerminalStyle::default()),
        ]
    );

    first.truncate(10);
    assert_eq!(first.text, "plain redm");
    assert_eq!(
        first.spans(),
        vec![("plain ", TerminalStyle::default()), ("redm", red)]
    );

    assert_eq!(
        TerminalLine::styled("dim", TerminalStyle::default()).runs,
        vec![]
    );
    assert_eq!(
        logical_lines([true, false, false, true, true]),
        vec![0..2, 2..3, 3..5]
    );
    assert_eq!(logical_lines([]), vec![]);
}
