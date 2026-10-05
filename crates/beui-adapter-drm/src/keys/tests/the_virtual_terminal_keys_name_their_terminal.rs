use super::*;

#[test]
fn the_virtual_terminal_keys_name_their_terminal() {
    assert_eq!(virtual_terminal(keysyms::KEY_XF86Switch_VT_1), Some(1));
    assert_eq!(virtual_terminal(keysyms::KEY_XF86Switch_VT_12), Some(12));
    assert_eq!(virtual_terminal(keysyms::KEY_F1), None);
}
