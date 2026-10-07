use super::*;
use crate::headless::HeadlessPlugin;
use block_plugin_api::{FontFace, HelloAccepted, PROTOCOL_VERSION, Theme};

fn asked(plugin: &mut HeadlessPlugin) -> Vec<Vec<char>> {
    plugin
        .outbound()
        .into_iter()
        .filter_map(|message| match message {
            Message::MissingCharacters(characters) => Some(characters),
            _ => None,
        })
        .collect()
}

#[test]
fn a_character_is_asked_for_once_and_only_after_the_hosts_fonts_arrive() {
    forget();
    let mut plugin = HeadlessPlugin::new("fonts", "Fonts", "1");
    plugin.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        surface: None,
        theme: Theme { dark: true },
    }));

    report_missing(['a', '\u{4e2d}']);
    assert!(
        asked(&mut plugin).is_empty(),
        "before the host's fonts arrive every character is missing, and asking for them all would only send the host looking"
    );

    plugin.receive(Message::Fonts(Fonts {
        replace: true,
        faces: vec![FontFace {
            role: FontRole::Text,
            index: 0,
            data: vec![1],
        }],
    }));
    report_missing(['\u{4e2d}']);
    assert_eq!(asked(&mut plugin), vec![vec!['\u{4e2d}']]);

    report_missing(['\u{4e2d}']);
    assert!(
        asked(&mut plugin).is_empty(),
        "a character the host could not cover is not asked for again on the next layout"
    );
}
