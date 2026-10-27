use super::*;
use beui_macros::view;

fn sign_in() -> crate::primitives::Root {
    let email = create_signal("ada@example.com".to_owned());
    let password = create_signal("\">secret".to_owned());
    let code = create_signal(String::new());
    view! {
        <Root>
            <Frame padding={Padding::all(Space::Lg)}>
                <Form action="/auth/login" hidden={vec![("csrf", "token\"<".to_owned())]}>
                    <List spacing=Space::Md>
                        <TextInput
                            value={email}
                            label="Email"
                            kind=InputKind::Email
                            name="email"
                            autocomplete="username"
                        />
                        <TextInput
                            value={password}
                            label="Password"
                            kind=InputKind::Password
                            name="password"
                            autocomplete="current-password"
                        />
                        <TextInput
                            value={code}
                            placeholder="Code shown in Block"
                            name="code"
                            autocomplete="one-time-code"
                        />
                        <List direction={crate::Direction::Horizontal} spacing=Space::Md>
                            <Button label="Sign in" variant=ButtonVariant::Primary submit=true />
                            <Link
                                label="Forgot your password?"
                                href={Some("/auth/forgot".to_owned())}
                            />
                        </List>
                    </List>
                </Form>
            </Frame>
        </Root>
    }
}

#[test]
fn a_sign_in_form_on_the_html_core() {
    let markup = crate::html::render_to_string(sign_in);
    for expected in [
        "<form method=\"post\" action=\"/auth/login\" novalidate",
        "<input type=\"hidden\" name=\"csrf\" value=\"token&quot;&lt;\"",
        "<input type=\"email\" value=\"ada@example.com\" name=\"email\" autocomplete=\"username\"",
        "<input type=\"password\" value=\"&quot;&gt;secret\" name=\"password\" autocomplete=\"current-password\"",
        "placeholder=\"Code shown in Block\"",
        "<button type=\"submit\"",
        "<a href=\"/auth/forgot\"",
    ] {
        assert!(markup.contains(expected), "{expected} in {markup}");
    }
    painting([360, 260], sign_in).snapshot("html_sign_in_form.browser");
}
