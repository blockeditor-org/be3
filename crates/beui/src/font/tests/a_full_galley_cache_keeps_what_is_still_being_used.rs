use super::*;

fn fill(fonts: &mut Fonts, from: usize) {
    for index in from..from + GALLEY_CACHE_LIMIT {
        fonts.layout(
            &index.to_string(),
            FontId::proportional(14.0),
            f32::INFINITY,
            1.0,
        );
    }
}

#[test]
fn a_full_galley_cache_keeps_what_is_still_being_used() {
    let mut fonts = Fonts::new(&FontSources::default());
    let font = FontId::proportional(14.0);
    let held = fonts.layout("held", font, f32::INFINITY, 1.0);

    fill(&mut fonts, 0);
    assert!(
        fonts.layout("held", font, f32::INFINITY, 1.0) == held,
        "filling the cache once must not throw away a galley that is still in use"
    );

    fill(&mut fonts, GALLEY_CACHE_LIMIT);
    fill(&mut fonts, GALLEY_CACHE_LIMIT * 2);
    assert!(
        fonts.layout("held", font, f32::INFINITY, 1.0) != held,
        "a galley nothing has asked for is eventually evicted"
    );
}
