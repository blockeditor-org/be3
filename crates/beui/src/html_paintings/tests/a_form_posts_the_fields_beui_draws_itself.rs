use super::*;
use beui_macros::view;

fn preferences() -> crate::primitives::Root {
    let remember = create_signal(true);
    let newsletter = create_signal(false);
    let region = create_signal(Some(1_usize));
    let locked = create_signal(true);
    view! {
        <Root>
            <Form action="/preferences">
                <List spacing=Space::Md>
                    <Checkbox label="Remember me" checked={remember} name="remember" />
                    <Switch label="Newsletter" on={newsletter} name="newsletter" />
                    <Checkbox label="Locked" checked={locked} name="locked" disabled=true />
                    <Select
                        label="Region"
                        name="region"
                        options={view! {
                            <ChoiceOption label="North" />
                            <ChoiceOption label="South" value="south" />
                        }}
                        selected={region}
                    />
                </List>
            </Form>
        </Root>
    }
}

#[test]
fn a_form_posts_the_fields_beui_draws_itself() {
    let markup = crate::html::render_to_string(preferences);
    for expected in [
        "<input type=\"hidden\" name=\"remember\" value=\"on\"",
        "<input type=\"hidden\" name=\"newsletter\" disabled",
        "<input type=\"hidden\" name=\"region\" value=\"south\"",
        "<input type=\"hidden\" name=\"locked\" value=\"on\" disabled",
    ] {
        assert!(markup.contains(expected), "{expected} in {markup}");
    }
}
