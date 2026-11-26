use super::*;

#[test]
fn a_replica_draws_a_node_its_offset_clips_whole_and_elsewhere() {
    painting([260, 160], || {
        let lent = swatch(GREEN, 100.0, 60.0);
        view! {
            <Root clear_color={PAPER}>
                <List direction={Direction::Horizontal} spacing={12.0} align={Align::Start}>
                    <Frame height={80.0} width={100.0} outline={Some(Outline::new(INK, 1.0))}>
                        <Offset offset={0.0}>
                            <List spacing={4.0}>{swatch(BLUE, 100.0, 40.0)}{lent}</List>
                        </Offset>
                    </Frame>
                    <Frame width={100.0} height={60.0}>
                        <Replica of={Some(lent)} />
                    </Frame>
                </List>
            </Root>
        }
    })
    .snapshot("html_replica.browser");
}
