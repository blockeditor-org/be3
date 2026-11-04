use super::*;

const SCRIPT: &str = r#"
# f20: hold and scroll for the volume
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
fn an_autohotkey_script_reads_as_remaps() {
    let groups = text::parse(SCRIPT).expect("the script parses");
    let names: Vec<&str> = groups.iter().map(|group| group.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Everywhere",
            "Animal Well",
            "Cave Story",
            "Cairn",
            "Order of the Sinking Star"
        ]
    );
    assert_eq!(
        groups[2]
            .window
            .as_ref()
            .and_then(|window| window.title.as_deref()),
        Some("Cave \"Story\"")
    );
    assert_eq!(
        groups[3].rules[0].action,
        Action::Steps {
            steps: vec![
                Step::Tap {
                    chord: Chord {
                        modifiers: Vec::new(),
                        key: key("Space"),
                    },
                    hold_ms: 40,
                },
                Step::Wait { ms: 5 },
                Step::Tap {
                    chord: Chord {
                        modifiers: Vec::new(),
                        key: key("Q"),
                    },
                    hold_ms: 40,
                },
            ],
            blind: false,
        }
    );
    assert_eq!(
        groups[4].rules[0].not_held,
        vec![Held::Modifier(Modifier::Shift)]
    );
}
