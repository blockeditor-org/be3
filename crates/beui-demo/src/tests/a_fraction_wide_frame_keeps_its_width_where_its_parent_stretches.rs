use super::*;
use beui_macros::view;

#[test]
fn a_fraction_wide_frame_keeps_its_width_where_its_parent_stretches() {
    let document = build(|| {
        view! {
            <Root>
                <Frame @test_id="page" padding=Padding::all(Space::Md)>
                    <List @test_id="column" spacing=Space::Md>
                        <Frame
                            @test_id="bar"
                            width={Length::Extent(Extent::fraction(0.25))}
                            height=Length::Size(Size::Xs)
                            fill=ColorRole::Accent
                        />
                        <Frame
                            @test_id="stretched"
                            height=Length::Size(Size::Xs)
                            fill=ColorRole::Border
                        />
                        <Frame
                            @test_id="box"
                            height=Length::Size(Size::Xl3)
                            fill=ColorRole::Surface
                        >
                            <Frame
                                @test_id="inner"
                                width={Length::Extent(Extent::fraction(0.5))}
                                fill=ColorRole::AccentSoft
                            />
                        </Frame>
                    </List>
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(240.0, 200.0));

    let column = test.rect_of("column");
    let bar = test.rect_of("bar");
    assert_eq!(
        bar.left(),
        column.left(),
        "the bar starts where the column does"
    );
    assert_eq!(
        bar.width(),
        column.width() * 0.25,
        "a fraction wide bar in a stretching column keeps its fraction of the width"
    );
    assert_eq!(
        test.rect_of("stretched").width(),
        column.width(),
        "a frame with no width of its own still stretches"
    );
    let outer = test.rect_of("box");
    let inner = test.rect_of("inner");
    assert_eq!(inner.min, outer.min, "the inner frame starts at the corner");
    assert_eq!(
        inner.size(),
        Vec2::new(outer.width() * 0.5, outer.height()),
        "a frame stretched by its frame keeps its own width and stretches only its height"
    );
    test.snapshot("a_fraction_wide_frame_keeps_its_width_where_its_parent_stretches");
}
