use super::*;

use crate::settings::Layered;

const SHARED: &str = r#"
[[group]]
name = "Everywhere"
rules = [{ on = "Pause", send = "PlayPause" }]

[[group]]
name = "Celeste"
window = { exe = "Celeste.exe" }
rules = [{ on = "MouseLeft", send = "J" }]
"#;

#[test]
fn remaps_written_as_text_keep_their_groups_and_layer_by_group() {
    let groups = be_remap::text::parse(SHARED).expect("the remaps parse");
    let shared = edited(
        &InputSettingsContent::default(),
        [InputSettings::default().set_remaps(groups.clone())],
    );
    let root = shared.root();
    assert_eq!(root.remap_groups(), groups);
    let ids: Vec<Uuid> = root.remaps.keys().copied().collect();

    let mut changed = groups.clone();
    changed[1].rules.clear();
    let shared = edited(&shared, [root.set_remaps(changed.clone())]);
    let root = shared.root();
    assert_eq!(root.remap_groups(), changed);
    assert_eq!(
        root.remaps.keys().copied().collect::<Vec<_>>(),
        ids,
        "a group keeps its id while its name stays"
    );

    let device = be_remap::text::parse(
        "[[group]]\nname = \"Laptop\"\nrules = [{ on = \"F20\", send = \"Mute\" }]",
    )
    .expect("the remaps parse");
    let own = edited(
        &InputSettingsContent::default(),
        [InputSettings::default().set_remaps(device.clone())],
    );
    let names: Vec<String> = own
        .root()
        .over(&root)
        .remap_groups()
        .into_iter()
        .map(|group| group.name)
        .collect();
    assert_eq!(
        names,
        ["Everywhere", "Laptop", "Celeste"],
        "a device's groups add to the shared ones, everywhere first"
    );

    let removed = edited(&shared, [root.set_remaps(Vec::new())]);
    assert!(removed.root().remaps.is_empty());
}
