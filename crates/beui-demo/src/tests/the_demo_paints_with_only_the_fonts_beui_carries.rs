use super::*;
use beui::FontSources;

#[test]
fn the_demo_paints_with_only_the_fonts_beui_carries() {
    let mut test = demo(WIDE);
    for page in PAGES {
        open(&mut test, page);
    }
    test.frame(Vec::new());

    let bundled = FontSources::bundled();
    let sources = test.fonts().sources();
    assert_eq!(
        test.fonts().generation(),
        0,
        "no font was added while painting"
    );
    assert!(
        [
            (&sources.proportional, &bundled.proportional),
            (&sources.monospace, &bundled.monospace),
            (&sources.fallback, &bundled.fallback),
            (&sources.icons, &bundled.icons),
        ]
        .into_iter()
        .all(|(held, carried)| held.len() == carried.len()
            && held
                .iter()
                .zip(carried)
                .all(|(held, carried)| held.same(carried))),
        "the fonts are the ones beui carries, not the system's"
    );
}
