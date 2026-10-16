use super::*;

const SCRIPT: &str = r#"
[[group]]
name = "Everywhere"
rules = [
  { on = "F20+WheelDown", send = "VolumeDown" },
  { on = "F20+WheelUp", send = "VolumeUp" },
  { on = "Pause", send = "PlayPause" },
]

[[group]]
name = "Animal Well"
window = { exe = "Animal Well.exe" }
rules = [
  { on = "MouseLeft", send = "X" },
  { on = "Tab", send = "C" },
  { on = "WheelUp", tap = "1", hold_ms = 20 },
]

[[group]]
name = "Cave Story"
window = { exe = "Doukutsu.exe", title = "Cave \"Story\"" }
rules = [
  { on = "MouseLeft", send = "F1", keep = true },
]

[[group]]
name = "Cairn"
window = { exe = "Cairn.exe" }
rules = [
  { on = "Q", steps = [{ tap = "Space", hold_ms = 40 }, { wait_ms = 5 }, { tap = "Q", hold_ms = 40 }] },
]

[[group]]
name = "Order of the Sinking Star"
window = { exe = "sinking_star.exe" }
rules = [
  { on = "Alt+W", not_held = ["Shift"], tap = "LeftShift+W" },
  { on = "Alt+D", tap = "D", blind = true },
]
"#;

#[test]
fn the_text_form_reads_back_what_it_prints() {
    let groups = text::parse(SCRIPT).expect("the script parses");
    assert_eq!(groups.len(), 5);
    let printed = text::print(&groups);
    assert_eq!(text::parse(&printed), Ok(groups));
    assert_eq!(printed.trim(), SCRIPT.trim());
}
