use super::*;
use crate::tw;

#[test]
fn typed_styles_collect_only_mounted_rules_and_preserve_order() {
    let html = render_fragment(|| {
        let _unused = view! {
            <elements::Div
                css=[
                    <tw::Grid />
                ]
            />
        };
        view! {
            <tw::Styles />
            <elements::Div
                class="existing"
                css=[
                    <tw::P4 />
                    <tw::Px4 />
                    <tw::Md>
                        <tw::Hover>
                            <tw::Underline />
                        </tw::Hover>
                    </tw::Md>
                ]
            />
            <elements::Div
                css=[
                    <tw::P4 />
                    <tw::Px4 />
                    <tw::Md>
                        <tw::Hover>
                            <tw::Underline />
                        </tw::Hover>
                    </tw::Md>
                ]
            />
        }
    });
    assert!(!html.contains("display:grid"));
    assert_eq!(html.matches("padding:1rem;").count(), 1);
    assert!(html.contains("@media (min-width:48rem){.bht-"));
    assert!(html.contains(":hover{text-decoration:underline;}"));
    assert!(html.find("padding:1rem;").unwrap() < html.find("padding-left:1rem;").unwrap());
    let classes: Vec<_> = html
        .split("class=\"")
        .skip(1)
        .map(|part| part.split('"').next().unwrap())
        .collect();
    assert_eq!(classes.len(), 2);
    assert_eq!(classes[0], format!("existing {}", classes[1]));
}
