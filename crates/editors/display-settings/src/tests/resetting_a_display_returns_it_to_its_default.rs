use super::*;

#[test]
fn resetting_a_display_returns_it_to_its_default() {
    let mut editor = editor(vec![gaming_monitor()]);
    choose(&mut editor, REFRESH, 2);
    assert_eq!(
        content(&editor).root().mode(MONITOR),
        Some(saved(2560, 1440, 59_951))
    );

    editor.click(RESET);
    editor.run();

    assert_eq!(content(&editor).root().mode(MONITOR), None);
    assert!(content(&editor).root().monitors.is_empty());
}
