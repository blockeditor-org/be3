use super::*;

const SETTINGS: &str = "Dialog \"Text settings\"";

fn assert_points_at(host: &mut HostTest, tab: (f32, f32, f32, f32), from: &str) {
    let card = host.place(SETTINGS);
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
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    let tab = host.place("Tab \"Text\"");

    host.click("#dock.editors.menu");
    host.click("MenuItem \"Text settings\"");
    host.wait(SETTINGS);
    host.run(&["settle"]);
    assert_points_at(&mut host, tab, "the tab's menu");
    host.snapshot("the_text_settings_point_at_the_tab_from_its_menu");
    host.key("Escape");
    host.gone(SETTINGS);

    host.click("#text.surface");
    host.key("ctrl+shift+p");
    host.type_text("Text settings");
    host.key("Enter");
    host.wait(SETTINGS);
    host.run(&["settle"]);
    assert_points_at(&mut host, tab, "the palette");
    host.key("Escape");
    host.gone(SETTINGS);
    host.close();
}
