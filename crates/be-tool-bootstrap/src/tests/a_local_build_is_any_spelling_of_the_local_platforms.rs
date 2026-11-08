use super::*;

#[test]
fn a_local_build_is_any_spelling_of_the_local_platforms() {
    assert!(server::is_local(
        "root//buck/platforms:execution_platforms_local",
        ""
    ));
    assert!(server::is_local(
        " //buck/platforms:execution_platforms_local ",
        "true"
    ));
    assert!(server::is_local(
        "@root//buck/platforms:execution_platforms_local",
        ""
    ));
    assert!(server::is_local(
        "root//buck/platforms:execution_platforms",
        "false"
    ));
    assert!(!server::is_local(
        "root//buck/platforms:execution_platforms",
        ""
    ));
    assert!(!server::is_local(
        "root//buck/platforms:execution_platforms",
        "true"
    ));
}
