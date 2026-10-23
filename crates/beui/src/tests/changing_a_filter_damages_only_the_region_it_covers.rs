use super::*;
use crate::filter::Filter;

#[test]
fn changing_a_filter_damages_only_the_region_it_covers() {
    let context = Context::new(crate::FreetypeFonts::default());
    let page = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
    let region = Rect::from_min_max(pos2(10.0, 10.0), pos2(60.0, 40.0));
    let frame = |blur: f32| {
        context.run(|context| {
            context.painter().rect_filled(page, 0.0, Color32::WHITE);
            context.apply_filter(Filter {
                region,
                blur,
                ..Filter::default()
            });
        })
    };
    frame(4.0);

    let still = frame(4.0);
    assert!(!still.main().changed, "an unchanged filter changes nothing");
    assert_eq!(still.main().damaged(), None);

    let wider = frame(8.0);
    assert!(
        wider.main().changed,
        "a filter that changed changes the frame"
    );
    assert_eq!(
        wider.main().damage.rects(),
        &[region],
        "a filter that changed damages only the region it covers"
    );
}
