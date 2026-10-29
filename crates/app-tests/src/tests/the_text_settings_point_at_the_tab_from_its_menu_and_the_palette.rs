use super::*;

const SETTINGS: &str = "Dialog \"Text settings\"";

fn assert_points_at(app: &mut AppTest, tab: (f32, f32, f32, f32), from: &str) {
    let card = app.place(SETTINGS);
    let middle = tab.0 + tab.2 / 2.0;
    assert!(
        card.1 > tab.1 + tab.3 && card.1 < tab.1 + tab.3 + 60.0,
        "opened from {from}, the settings open just under the tab: {card:?} under {tab:?}"
    );
    assert!(
        card.0 < middle && card.0 + card.2 > middle,
        "around its middle: {card:?} for {tab:?}"
    );
}

#[test]
fn the_text_settings_point_at_the_tab_from_its_menu_and_the_palette() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    let tab = app.place("Tab \"Text\"");

    app.click("#dock.editors.menu");
    app.click("MenuItem \"Text settings\"");
    app.wait(SETTINGS);
    app.run(&["settle"]);
    assert_points_at(&mut app, tab, "the tab's menu");
    app.snapshot("the_text_settings_point_at_the_tab_from_its_menu");
    app.key("Escape");
    app.gone(SETTINGS);

    app.click("#text.surface");
    app.key("ctrl+shift+p");
    app.type_text("Text settings");
    app.key("Enter");
    app.wait(SETTINGS);
    app.run(&["settle"]);
    assert_points_at(&mut app, tab, "the palette");
    app.key("Escape");
    app.gone(SETTINGS);
    app.close();
}
