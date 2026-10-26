use super::*;

#[test]
fn an_offset_clips_its_child_to_its_box() {
    painting([200, 160], || {
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START}>
                    <Frame height={100.0} width={120.0} outline={Some(Outline::new(INK, 1.0))}>
                        <Offset offset={0.0}>
                            <List spacing={4.0}>
                                {swatch(BLUE, 120.0, 40.0)}
                                {swatch(GREEN, 120.0, 40.0)}
                                {swatch(AMBER, 120.0, 40.0)}
                            </List>
                        </Offset>
                    </Frame>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_offset.browser");
}
