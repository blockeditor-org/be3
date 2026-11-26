use super::*;
use be_sdk_beui::beui::styled::Theme;
use be_sdk_beui::beui::{Key, Modifiers};
use be_sdk_beui::{Preferences, ThemeKind};

fn fill(theme: ThemeKind) -> String {
    let [red, green, blue, _] = Theme::of(theme).background.to_array();
    format!("fill=\"#{red:02x}{green:02x}{blue:02x}\"")
}

#[test]
fn an_export_draws_in_the_theme_the_host_shows() {
    // The theme the host shows when the dialog opens is stored in the export's settings, so the
    // dialog's own app, whatever theme its host shows, makes the same bytes.
    let mut editor = editor(&[entity(Uuid::from_u128(1))]);
    editor.key(
        Key::A,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    editor.set_preferences(Preferences {
        theme: ThemeKind::Light,
        ..Preferences::default()
    });
    let mut dialog = export_dialog(&mut editor, "svg");
    dialog.set_preferences(Preferences {
        theme: ThemeKind::Dark,
        ..Preferences::default()
    });
    dialog.run();
    dialog.click("export.copy");
    dialog.run();
    let copied = dialog.take_copied("image/svg+xml");
    let [item] = copied.as_slice() else {
        panic!("Copy puts one SVG on the clipboard, put {}", copied.len());
    };
    let svg = String::from_utf8(item.data.clone()).expect("the SVG is text");
    assert!(
        svg.contains(&fill(ThemeKind::Light)) && !svg.contains(&fill(ThemeKind::Dark)),
        "the background is the light theme's, as the host showed it when the dialog opened:\n{svg}"
    );
}
