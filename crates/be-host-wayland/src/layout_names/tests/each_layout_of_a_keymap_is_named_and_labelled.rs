use super::*;

fn keyboard(layout: &str, variant: &str) -> KeyboardConfig {
    KeyboardConfig {
        layout: layout.to_owned(),
        variant: variant.to_owned(),
        options: String::new(),
        repeat_delay: 600,
        repeat_rate: 25,
    }
}

#[test]
fn each_layout_of_a_keymap_is_named_and_labelled() {
    let named = named_layouts(&keyboard("us,de", "")).expect("the keymap compiles");
    assert_eq!(
        named,
        [
            NamedLayout {
                name: "English (US)".to_owned(),
                label: "EN".to_owned(),
            },
            NamedLayout {
                name: "German".to_owned(),
                label: "DE".to_owned(),
            },
        ]
    );

    let labels: Vec<String> = named_layouts(&keyboard("us,us", ",intl"))
        .expect("the keymap compiles")
        .into_iter()
        .map(|layout| layout.label)
        .collect();
    assert_eq!(
        labels,
        ["EN1", "EN2"],
        "layouts that share a label are told apart by their place"
    );

    assert_eq!(
        named_layouts(&keyboard("nowhere", "")),
        None,
        "a keymap that does not compile names nothing"
    );
}
