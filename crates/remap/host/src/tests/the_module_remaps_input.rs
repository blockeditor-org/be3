use super::*;

#[test]
fn the_module_remaps_input() {
    let mut remapper = remapper(
        r#"
        [[group]]
        window = { exe = "Cairn.exe" }
        rules = [{ on = "Q", steps = [{ tap = "Space", hold_ms = 40 }, { tap = "Q", hold_ms = 40 }] }]

        [[group]]
        rules = [{ on = "MouseLeft", send = "X" }]
        "#,
    );

    assert_eq!(remapper.input(press("MouseLeft"), NOW), vec![press("X")]);
    assert_eq!(
        remapper.input(release("MouseLeft"), NOW),
        vec![release("X")]
    );
    assert_eq!(remapper.input(press("Q"), NOW), vec![press("Q")]);
    assert_eq!(remapper.input(release("Q"), NOW), vec![release("Q")]);

    let cairn = Window {
        exe: "Cairn.exe".to_owned(),
        ..Window::default()
    };
    assert_eq!(remapper.focus(Some(cairn), NOW), vec![]);
    assert_eq!(remapper.input(press("Q"), NOW), vec![press("Space")]);
    assert_eq!(remapper.deadline(), Some(NOW + 40_000));
    assert_eq!(
        remapper.timer(NOW + 40_000),
        vec![release("Space"), press("Q")]
    );
    assert_eq!(remapper.timer(NOW + 80_000), vec![release("Q")]);
    assert_eq!(remapper.deadline(), None);
    assert!(remapper.take_problems().is_empty());
}
