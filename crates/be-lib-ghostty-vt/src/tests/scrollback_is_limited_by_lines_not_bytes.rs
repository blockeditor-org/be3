use crate::Terminal;

#[test]
fn scrollback_is_limited_by_lines_not_bytes() {
    let mut terminal = Terminal::new(80, 24, 5_000).unwrap();
    for line in 0..20_000 {
        terminal.write(format!("line {line}\r\n").as_bytes());
    }
    // The library's default 10,000-byte cap would keep about a thousand of
    // these rows. The line limit prunes whole pages of history, so what is
    // kept lands within a page of it rather than on it.
    let kept = terminal.scrollback_rows();
    assert!(
        (4_000..=6_000).contains(&kept),
        "kept {kept} rows of scrollback"
    );
}
