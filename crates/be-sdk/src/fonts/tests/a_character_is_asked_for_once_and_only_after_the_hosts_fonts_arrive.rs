use super::*;
use be_sdk_api::FontFace;

#[test]
fn a_character_is_asked_for_once_and_only_after_the_hosts_fonts_arrive() {
    forget();
    let mut app = HeadlessApp::new("fonts", "Fonts", "1");
    accepted(&mut app);

    report_missing(['a', '\u{4e2d}']);
    assert!(
        asked(&mut app).is_empty(),
        "before the host's fonts arrive every character is missing, and asking for them all would only send the host looking"
    );

    app.set_fonts(Fonts {
        replace: true,
        faces: vec![FontFace {
            role: FontRole::Text,
            index: 0,
            data: vec![1],
        }],
    })
    .unwrap();
    report_missing(['\u{4e2d}']);
    assert_eq!(asked(&mut app), vec![vec!['\u{4e2d}']]);

    report_missing(['\u{4e2d}']);
    assert!(
        asked(&mut app).is_empty(),
        "a character the host could not cover is not asked for again on the next layout"
    );
}
