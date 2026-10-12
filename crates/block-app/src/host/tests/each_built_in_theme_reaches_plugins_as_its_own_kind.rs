use super::*;

#[test]
fn each_built_in_theme_reaches_plugins_as_its_own_kind() {
    assert_eq!(
        styled::Theme::BUILT_IN.map(|(_, theme)| plugin_theme(theme)),
        [
            block_plugin_api::Theme::Dark,
            block_plugin_api::Theme::OledDark,
            block_plugin_api::Theme::Light,
            block_plugin_api::Theme::EInk,
        ]
    );
}
