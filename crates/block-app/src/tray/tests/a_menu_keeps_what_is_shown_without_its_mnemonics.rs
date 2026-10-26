use super::*;

#[test]
fn a_menu_keeps_what_is_shown_without_its_mnemonics() {
    let children = [
        entry(1, vec![("label", Value::from("_Open Chat"))], vec![]),
        entry(
            2,
            vec![
                ("label", Value::from("Hidden")),
                ("visible", Value::from(false)),
            ],
            vec![],
        ),
        entry(3, vec![("type", Value::from("separator"))], vec![]),
        entry(
            4,
            vec![
                ("label", Value::from("Status")),
                ("children-display", Value::from("submenu")),
            ],
            vec![
                entry(
                    5,
                    vec![
                        ("label", Value::from("Away")),
                        ("toggle-type", Value::from("radio")),
                        ("toggle-state", Value::from(1_i32)),
                    ],
                    vec![],
                ),
                entry(
                    6,
                    vec![
                        ("label", Value::from("Do not disturb")),
                        ("toggle-type", Value::from("checkmark")),
                        ("toggle-state", Value::from(0_i32)),
                        ("enabled", Value::from(false)),
                    ],
                    vec![],
                ),
            ],
        ),
        entry(7, vec![("label", Value::from("Save__as"))], vec![]),
        Value::from("not an entry"),
    ];
    let properties: HashMap<String, OwnedValue> = HashMap::new();
    let root = raw_entry(
        0,
        properties
            .iter()
            .map(|(key, value)| (key.as_str(), &**value)),
        children.iter(),
        0,
    );
    let entry = |id: i32, label: &str| TrayMenuItem {
        id,
        label: label.to_owned(),
        enabled: true,
        ..TrayMenuItem::default()
    };
    assert_eq!(
        menu(&root),
        vec![
            entry(1, "Open Chat"),
            TrayMenuItem {
                separator: true,
                ..entry(3, "")
            },
            entry(4, "Status"),
            TrayMenuItem {
                depth: 1,
                toggle: TrayToggle::Radio(true),
                ..entry(5, "Away")
            },
            TrayMenuItem {
                depth: 1,
                toggle: TrayToggle::Check(false),
                enabled: false,
                ..entry(6, "Do not disturb")
            },
            entry(7, "Save_as"),
        ]
    );
}
