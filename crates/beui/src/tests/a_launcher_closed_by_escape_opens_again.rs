use super::*;

#[test]
fn a_launcher_closed_by_escape_opens_again() {
    reopens_after(|harness| harness.key(Key::Escape, Modifiers::NONE));
}
