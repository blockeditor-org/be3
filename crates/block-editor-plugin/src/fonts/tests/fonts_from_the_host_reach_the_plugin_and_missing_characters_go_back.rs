use super::*;
use block_plugin_api::FontFace;
use std::cell::Cell;

#[test]
fn fonts_from_the_host_reach_the_plugin_and_missing_characters_go_back() {
    forget();
    let mut plugin = HeadlessPlugin::new("fonts", "Fonts", "1");
    accepted(&mut plugin);
    let heard = Rc::new(Cell::new(0));
    let listener = heard.clone();
    watch_fonts(move |added, _| listener.set(listener.get() + added.len()));

    plugin
        .set_fonts(Fonts {
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
        })
        .unwrap();
    assert_eq!(
        heard.get(),
        2,
        "whoever watches the fonts hears of both faces"
    );
    let held = host_fonts();
    assert_eq!(held.len(), 2);
    assert_eq!(&*held[0].data, &[1, 2, 3]);

    report_missing(['\u{4e2d}', '\u{4e2d}', '\u{0928}']);
    assert_eq!(
        asked(&mut plugin),
        vec![vec!['\u{4e2d}', '\u{0928}']],
        "each missing character is asked for once"
    );
    assert!(asked(&mut plugin).is_empty(), "and only once");
}
