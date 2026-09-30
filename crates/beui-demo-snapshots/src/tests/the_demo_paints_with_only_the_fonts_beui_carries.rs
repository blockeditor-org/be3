use super::*;
use beui::FontSources;

const WINDOW: Vec2 = Vec2::new(1100.0, 800.0);

#[test]
fn the_demo_paints_with_only_the_fonts_beui_carries() {
    let mut test = demo(WINDOW);
    for page in PAGES {
        test.click(&format!("demo.catalog.{}", page.title()));
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
