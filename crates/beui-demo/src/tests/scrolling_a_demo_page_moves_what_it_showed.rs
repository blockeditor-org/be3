use super::*;

const STEP: f32 = 40.0;

#[test]
fn scrolling_a_demo_page_moves_what_it_showed() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Menus);
    test.frame(vec![beui::Event::PointerMoved(pos2(
        WIDE.x * 0.75,
        WIDE.y / 2.0,
    ))]);
    for _ in 0..2 {
        test.frame(vec![beui::Event::Scroll(Vec2::new(0.0, -STEP))]);
        let output = test.output().expect("the demo painted a frame");
        let moved = output
            .moved()
            .expect("the page's rounded pane does not stop the scroll copying what it showed");
        assert_eq!(moved.by, Vec2::new(0.0, -STEP));
        let landed = moved.to();
        let damaged: f32 = output
            .damage
            .rects()
            .iter()
            .map(|rect| rect.intersect(landed))
            .filter(|rect| rect.is_positive())
            .map(|rect| rect.width() * rect.height())
            .sum();
        let whole = landed.width() * landed.height();
        assert!(
            damaged < whole / 4.0,
            "the copied rows keep their pixels: {damaged} of {whole} were damaged"
        );
    }
}
