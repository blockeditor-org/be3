use super::*;
use beui_macros::view;

#[test]
fn styled_text_inputs_on_the_html_core() {
    painting([320, 200], || {
        let note = create_signal("A note\nover two lines".to_owned());
        let title = create_signal("Shopping".to_owned());
        view! {
            <Root>
                <Frame padding={Padding::all(Space::Lg)}>
                    <List spacing=Space::Md>
                        <TextInput value={title} />
                        <TextInput value={note} multiline=true />
                    </List>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_styled_text_inputs.browser");
}
