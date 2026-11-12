use super::*;

#[test]
fn the_picker_opens_from_the_top_of_a_long_page() {
    let mut rules = String::new();
    for _ in 0..30 {
        rules.push_str("  { on = \"F13\", send = \"Escape\" },\n");
    }
    let config = format!(
        "[[group]]\nname = \"Everywhere\"\nrules = [{{ on = \"CapsLock\", send = \"Escape\" }}]\n\n[[group]]\nname = \"Game\"\nwindow = {{ exe = \"game.exe\" }}\nrules = [\n{rules}]\n"
    );
    let mut editor = editor(RemapState::On, Some(&config));
    editor.resize(Vec2::new(800.0, 600.0));
    editor.run();
    editor.click("input-remaps.group.0.rule.0");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.on");
    editor.run();
    assert!(editor.shown("input-remaps.picker"), "the picker opens wherever the page is scrolled");
}
