use super::*;

use crate::settings::Layered;

const SHARED: &str = r#"# Everywhere I go
[[group]]
name = "Everywhere"
rules = [{ on = "Pause", send = "PlayPause" }]

[[group]]
name = "Celeste"
window = { exe = "Celeste.exe" }
rules = [
    { on = "MouseLeft", send = "J" },   # jump
]
"#;

const DEVICE: &str = r#"
[[group]]
name = "Celeste"
window = { exe = "Celeste.exe" }
rules = [{ on = "MouseLeft", send = "C" }]

[[group]]
name = "Laptop"
rules = [{ on = "F20", send = "Mute" }]
"#;

#[test]
fn remaps_keep_their_text_and_a_profile_adds_to_the_shared_ones() {
    let shared = edited(
        &InputSettingsContent::default(),
        [InputSettings::set_remaps(Some(SHARED))],
    );
    let root = shared.root();
    assert_eq!(
        root.remaps.as_deref(),
        Some(SHARED),
        "the text is kept as written"
    );
    assert_eq!(root.remap_groups().map(|groups| groups.len()), Ok(2));

    let own = edited(
        &InputSettingsContent::default(),
        [InputSettings::set_remaps(Some(DEVICE))],
    );
    let effective = own.root().over(&root);
    let groups = effective.remap_groups().expect("the layers read together");
    let names: Vec<&str> = groups.iter().map(|group| group.name.as_str()).collect();
    assert_eq!(
        names,
        ["Everywhere", "Celeste", "Laptop"],
        "a profile's group replaces the shared one of its name and adds the rest"
    );
    let jump = &groups[1].rules[0].action;
    assert_eq!(
        *jump,
        be_remap::Action::Send(be_remap::Chord {
            modifiers: Vec::new(),
            key: be_remap::Source::named("C").expect("a key"),
        })
    );

    let cleared = edited(&shared, [InputSettings::set_remaps(Some("  \n"))]);
    assert_eq!(cleared.root().remaps, None);
    assert_eq!(cleared.root().remap_groups(), Ok(Vec::new()));
}
