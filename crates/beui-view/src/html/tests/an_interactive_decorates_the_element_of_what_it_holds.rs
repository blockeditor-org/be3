use super::*;

#[test]
fn an_interactive_decorates_the_element_of_what_it_holds() {
    painting([200, 120], || {
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START} padding={Sides::all(10.0)}>
                    <Interactive
                        focusable=true
                        cursor={Some(CursorIcon::PointingHand)}
                        on_click={|| {}}
                    >
                        <Frame width={80.0} height={40.0} fill={BLUE} radius={Corners::all(6.0)} />
                    </Interactive>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_interactive.browser");
}
