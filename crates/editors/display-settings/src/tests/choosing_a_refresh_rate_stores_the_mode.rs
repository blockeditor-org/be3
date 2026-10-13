use super::*;

#[test]
fn choosing_a_refresh_rate_stores_the_mode() {
    let mut editor = editor(vec![gaming_monitor()]);
    editor.snapshot("choosing_a_refresh_rate_stores_the_mode");
    assert_eq!(content(&editor).root().mode(MONITOR), None);

    choose(&mut editor, REFRESH, 1);

    assert_eq!(
        content(&editor).root().mode(MONITOR),
        Some(saved(2560, 1440, 143_998))
    );
}
