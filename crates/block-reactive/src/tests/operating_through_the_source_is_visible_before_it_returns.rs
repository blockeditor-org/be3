use super::*;

#[test]
fn operating_through_the_source_is_visible_before_it_returns() {
    let client = client();
    let source = BlockSource::new(client.create_block(Presentation::default()), || {});
    let slides = source.project(|presentation: &Presentation| presentation.slides().len());

    source.operate(add_slide(0));
    assert_eq!(slides.get_untracked(), 1);
    source.operate(add_slide(1));
    assert_eq!(slides.get_untracked(), 2);
}
