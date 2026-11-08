use super::super::links::local_links;

#[test]
fn a_link_that_prefers_this_machine_is_reported() {
    let event = |identifier: &str, prefers_local: bool| {
        format!(
            r#"{{"Event":{{"data":{{"SpanEnd":{{"data":{{"ActionExecution":{{"name":{{"category":"rustc","identifier":"{identifier}"}},"prefers_local":{prefers_local},"requires_local":false,"key":{{"owner":{{"TargetLabel":{{"label":{{"package":"root//crates/a","name":"bin"}}}}}}}}}}}}}}}}}}}}"#
        )
    };
    let events = [
        event("link [pic]", false),
        event("metadata-fast", true),
        event("link [pic]", true),
    ]
    .join("\n");
    assert_eq!(
        local_links(&events),
        ["root//crates/a:bin (link [pic])".to_owned()]
    );
}
