use super::*;

#[test]
fn a_portal_shows_a_node_built_elsewhere() {
    painting([200, 120], || {
        let lent = view! {
            <Frame width={80.0} height={40.0} fill={GREEN} />
        };
        view! {
            <Root clear_color={PAPER}>
                <List spacing={8.0} align={Align::Start}>
                    {swatch(BLUE, 80.0, 30.0)}
                    <Frame padding={Sides::all(6.0)} outline={Some(Outline::new(INK, 1.0))}>
                        <Portal node={Some(lent)} />
                    </Frame>
                    <Portal />
                </List>
            </Root>
        }
    })
    .snapshot("html_portal.browser");
}
