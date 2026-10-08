use super::*;
use crate::headless::HeadlessPlugin;
use block_plugin_api::{FontFace, HelloAccepted, Theme};
use std::cell::Cell;

#[test]
fn fonts_from_the_host_reach_the_plugin_and_missing_characters_go_back() {
    forget();
    let mut plugin = HeadlessPlugin::new("fonts", "Fonts", "1");
    plugin.receive(Message::HelloAccepted(HelloAccepted {
        host_name: "test host".into(),
        surface: None,
        theme: Theme { dark: true },
    }));
    let heard = Rc::new(Cell::new(0));
    let listener = heard.clone();
    watch_fonts(move |added, _| listener.set(listener.get() + added.len()));

    let replies = plugin.receive(Message::Fonts(Fonts {
        replace: true,
        faces: vec![
            FontFace {
                role: FontRole::Text,
                index: 0,
                data: vec![1, 2, 3],
            },
            FontFace {
                role: FontRole::Icons,
                index: 0,
                data: vec![4],
            },
        ],
    }));
    assert!(
        replies.is_empty(),
        "fonts are accepted without a reply: {replies:?}"
    );
    assert_eq!(
        heard.get(),
        2,
        "whoever watches the fonts hears of both faces"
    );
    let held = host_fonts();
    assert_eq!(held.len(), 2);
    assert_eq!(&*held[0].data, &[1, 2, 3]);

    report_missing(['\u{4e2d}', '\u{4e2d}', '\u{0928}']);
    let outbound = plugin.outbound();
    assert!(
        outbound.contains(&Message::MissingCharacters(vec!['\u{4e2d}', '\u{0928}'])),
        "each missing character is asked for once: {outbound:?}"
    );
    assert!(
        !plugin
            .outbound()
            .iter()
            .any(|message| matches!(message, Message::MissingCharacters(_))),
        "and only once"
    );
}
