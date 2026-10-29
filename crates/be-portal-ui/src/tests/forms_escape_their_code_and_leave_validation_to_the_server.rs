use super::*;

#[test]
fn forms_escape_their_code_and_leave_validation_to_the_server() {
    let sign_in = render(&Page {
        code: "\"><script>".into(),
        ..page(Screen::SignIn)
    });
    assert!(sign_in.contains("value=\"&quot;&gt;&lt;script&gt;\""));
    assert!(!sign_in.contains("<script>"));
    assert!(sign_in.contains("novalidate=\"\""));
    assert!(sign_in.contains("<form method=\"post\" action=\"/auth/login\""));
    assert!(sign_in.contains("<input type=\"hidden\" name=\"csrf\" value=\""));
    assert!(sign_in.contains("<a href=\"/auth/register?code=%22%3E%3Cscript%3E\""));
    let registration = render(&Page {
        problem: Some(Problem::from_message(
            "Password confirmation: the passwords do not match.",
        )),
        email: "valid@example.com".into(),
        display_name: "Ada".into(),
        ..page(Screen::Register)
    });
    assert!(registration.contains("The passwords do not match."));
    assert!(registration.contains("autocomplete=\"new-password\""));
    for page in [&sign_in, &registration] {
        for constraint in ["required=", "maxlength=", "minlength=", "pattern="] {
            assert!(!page.contains(constraint));
        }
    }
}
