use super::*;

const WRITTEN: &str = r#"[[group]]
name = "Everywhere"
rules = [
  { on = "CapsLock", send = "Escape" },
  { on = "MouseExtra", tap = "PlayPause" },
  { on = "MouseBack", send = "Ctrl+W", not_held = ["Shift"] },
]

[[group]]
name = "Celeste"
window = { exe = "Celeste.exe" }
rules = [
  { on = "F20+WheelUp", steps = [{ tap = "X", hold_ms = 30 }, { wait_ms = 80 }, { tap = "C", hold_ms = 30 }] },
  { on = "F20", tap = "C", hold_ms = 50 },
  { on = "MouseSide", send = "Z", keep = true },
]
"#;

#[test]
fn remaps_show_as_keys_in_a_list() {
    let mut editor = editor(RemapState::On, Some(WRITTEN));
    assert!(
        !editor.shown("input-remaps.group.0.rule.0.on"),
        "every remap starts closed"
    );
    editor.snapshot("remaps_show_as_keys_in_a_list");
}
