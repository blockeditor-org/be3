use block_editor_beui::be_block::display_settings::LockAfter;

use super::*;

#[test]
fn the_lock_follows_the_screens_until_given_a_time_of_its_own() {
    let mut editor = editor(vec![gaming_monitor()]);
    let root = content(&editor).root();
    assert_eq!(root.lock_after(), LockAfter::WithScreens);
    assert_eq!(root.lock_time(), ScreenOff::DEFAULT.after());

    choose(&mut editor, LOCK_AFTER, 3);
    editor.snapshot("the_lock_after_five_minutes");
    let root = content(&editor).root();
    assert_eq!(root.lock_after(), LockAfter::After { minutes: 5 });
    assert_eq!(root.lock_time(), Some(std::time::Duration::from_secs(300)));

    choose(&mut editor, LOCK_AFTER, 8);
    assert_eq!(content(&editor).root().lock_time(), None, "never locks");
}
