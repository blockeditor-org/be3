use super::*;

#[test]
fn insets_give_the_content_area_of_a_region_resized_since_they_were_reported() {
    let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let content = Rect::from_min_max(Pos2::new(220.0, 40.0), Pos2::new(800.0, 600.0));
    let insets = Insets::between(region, content);
    assert_eq!(insets.inside(region), content);

    let resized = Rect::from_min_size(Pos2::new(30.0, 10.0), Vec2::new(500.0, 400.0));
    assert_eq!(
        insets.inside(resized),
        Rect::from_min_max(Pos2::new(250.0, 50.0), Pos2::new(530.0, 410.0)),
        "the sidebar and the top bar keep their sizes and the content takes the rest"
    );
}
