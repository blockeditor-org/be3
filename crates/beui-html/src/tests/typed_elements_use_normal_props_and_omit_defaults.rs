use super::*;

#[test]
fn typed_elements_use_normal_props_and_omit_defaults() {
    let html = render_fragment(|| {
        view! {
            <elements::Form action="/auth" method="post">
                <elements::Label r#for="code">"Code"</elements::Label>
                <elements::Input id="code" name="code" maxlength=14 required=true />
                <elements::Button name="action" value="deny" formnovalidate=true>
                    "Deny"
                </elements::Button>
            </elements::Form>
        }
    });
    assert_eq!(
        html,
        "<form action=\"/auth\" method=\"post\"><label for=\"code\">Code</label><input id=\"code\" name=\"code\" maxlength=\"14\" required=\"\"><button name=\"action\" value=\"deny\" formnovalidate=\"\">Deny</button></form>"
    );
}
