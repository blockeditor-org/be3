use super::*;

#[test]
fn a_list_holds_a_shared_child_to_its_share_whatever_it_holds() {
    painting([200, 160], || {
        view! {
            <Root clear_color={PAPER}>
                <List spacing={8.0}>
                    <Offset @sizing={ItemSize::Percent(1.0)}>{swatch(BLUE, 120.0, 400.0)}</Offset>
                    {swatch(GREEN, 120.0, 30.0)}
                </List>
            </Root>
        }
    })
    .snapshot("html_list_share.browser");
}
