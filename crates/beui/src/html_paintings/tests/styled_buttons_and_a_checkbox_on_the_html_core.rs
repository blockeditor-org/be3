use super::*;
use beui_macros::view;

#[test]
fn styled_buttons_and_a_checkbox_on_the_html_core() {
    painting([320, 160], || {
        let checked = create_signal(true);
        view! {
            <Root>
                <Frame padding={Padding::all(Space::Lg)}>
                    <List spacing=Space::Md>
                        <List direction={crate::Direction::Horizontal} spacing=Space::Sm>
                            <Button label="Save" variant=ButtonVariant::Primary />
                            <Button label="Cancel" />
                            <Button label="Off" disabled=true />
                        </List>
                        <Checkbox label="Remember me" checked={checked} />
                    </List>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_styled_buttons_and_checkbox.browser");
}
