use super::*;

#[test]
fn a_frame_moves_turns_and_fades_what_it_holds() {
    painting([320, 160], || {
        view! {
            <Root clear_color={PAPER}>
                <List direction={Direction::Horizontal} align={Align::Center} spacing={24.0}>
                    <Frame translate={vec2(10.0, 20.0)}>{swatch(BLUE, 60.0, 60.0)}</Frame>
                    <Frame rotate={std::f32::consts::FRAC_PI_4}>{swatch(GREEN, 60.0, 60.0)}</Frame>
                    <Frame opacity={0.4}>{swatch(INK, 60.0, 60.0)}</Frame>
                    <Frame fade_edges={Sides::horizontal(20.0)}>{swatch(AMBER, 80.0, 60.0)}</Frame>
                </List>
            </Root>
        }
    })
    .snapshot("html_frame_effects.browser");
}
