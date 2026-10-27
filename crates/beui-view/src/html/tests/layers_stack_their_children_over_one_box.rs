use super::*;

#[test]
fn layers_stack_their_children_over_one_box() {
    painting([200, 160], || {
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START}>
                    <Layers>
                        {swatch(BLUE, 120.0, 100.0)}
                        <Frame align={Align2::CENTER}>{swatch(AMBER, 40.0, 40.0)}</Frame>
                    </Layers>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_layers.browser");
}
