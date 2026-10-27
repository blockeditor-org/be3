use super::*;

#[test]
fn a_frame_sizes_pads_fills_rounds_and_aligns_what_it_holds() {
    painting([320, 240], || {
        view! {
            <Root clear_color={PAPER}>
                <List direction={Direction::Horizontal} align={Align::Start} spacing={12.0}>
                    <Frame
                        width={120.0}
                        height={120.0}
                        padding={Sides::all(12.0)}
                        fill={BLUE}
                        radius={Corners::all(16.0)}
                        align={Align2::CENTER}
                    >
                        {swatch(AMBER, 40.0, 40.0)}
                    </Frame>
                    <Frame
                        width={Extent::px(120.0)}
                        aspect_ratio={Some(2.0)}
                        fill={GREEN}
                        outline={Some(Outline::new(INK, 3.0).offset(2.0))}
                        align={Align2::new(Align::End, Align::Start)}
                    >
                        {swatch(INK, 20.0, 20.0)}
                    </Frame>
                </List>
            </Root>
        }
    })
    .snapshot("html_frame.browser");
}
