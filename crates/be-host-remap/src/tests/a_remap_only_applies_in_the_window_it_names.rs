use super::*;

#[test]
fn a_remap_only_applies_in_the_window_it_names() {
    let mut engine = engine(
        r#"
        [[group]]
        name = "Celeste"
        window = { exe = "celeste.EXE" }
        rules = [{ on = "MouseLeft", send = "J" }]

        [[group]]
        name = "Everywhere"
        rules = [{ on = "MouseLeft", send = "K" }, { on = "Pause", send = "PlayPause" }]
        "#,
    );

    engine.focus(Some(window("Celeste.exe")));
    assert_eq!(engine.input(press("MouseLeft"), NOW), vec![press("J")]);
    engine.focus(Some(window("Cuphead.exe")));
    assert_eq!(
        engine.input(release("MouseLeft"), NOW),
        vec![release("J")],
        "a press keeps the rule it started with when the focus moves"
    );
    assert_eq!(engine.input(press("MouseLeft"), NOW), vec![press("K")]);
    assert_eq!(engine.input(release("MouseLeft"), NOW), vec![release("K")]);
    engine.focus(None);
    assert_eq!(engine.input(press("Pause"), NOW), vec![press("PlayPause")]);
}
