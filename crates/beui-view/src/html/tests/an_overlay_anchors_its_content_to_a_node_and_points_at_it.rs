use super::*;

#[test]
fn an_overlay_anchors_its_content_to_a_node_and_points_at_it() {
    painting([320, 220], || {
        let menu = NodeRef::new();
        let tip = NodeRef::new();
        let bubble = NodeRef::new();
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START} padding={Sides::all(20.0)}>
                    <List spacing={80.0} align={Align::Start}>
                        <Frame @node_ref=&menu width={80.0} height={30.0} fill={BLUE} />
                        <Frame @node_ref=&tip width={60.0} height={30.0} fill={GREEN} />
                        <Overlay anchor={&menu} placement={Placement::BelowStart} policy={LayerPolicy::Pass} open={true}>
                            <Frame width={100.0} height={60.0} fill={AMBER} />
                        </Overlay>
                        <Overlay
                            anchor={&tip}
                            placement={Placement::Popover { gap: 12, margin: 8 }}
                            policy={LayerPolicy::Pass}
                            pointer={bubble}
                            beak={8.0}
                            open={true}
                        >
                            <Frame @node_ref=&bubble padding={Sides::all(8.0)} radius={Corners::all(6.0)} fill={INK}>
                                <Text color={PAPER}>"Pointing up"</Text>
                            </Frame>
                        </Overlay>
                    </List>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_overlay.browser");
}
