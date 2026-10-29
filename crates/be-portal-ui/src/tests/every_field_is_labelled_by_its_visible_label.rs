use super::*;

#[test]
fn every_field_is_labelled_by_its_visible_label() {
    let sign_in = render(&page(Screen::SignIn));
    assert!(sign_in.contains("<input type=\"email\" name=\"email\""));
    assert!(sign_in.contains("aria-label=\"Email\""));
    assert!(sign_in.contains("aria-label=\"Password\""));
    let registration = render(&page(Screen::Register));
    for label in ["Display name", "Email", "Password", "Confirm password"] {
        assert!(
            registration.contains(&format!("aria-label=\"{label}\"")),
            "{label} has no label"
        );
    }
    for screen in [Screen::SignIn, Screen::Register, Screen::Forgot, Screen::Reset] {
        let html = render(&page(screen));
        for input in html.split("<input").skip(1) {
            let tag = &input[..input.find('>').unwrap()];
            assert!(
                tag.contains("type=\"hidden\"") || tag.contains("aria-label=\""),
                "an input has no label: <input{tag}>"
            );
        }
    }
}
