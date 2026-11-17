#[test]
fn the_sandbox_finds_no_staged_app_for_an_id_no_manifest_has() {
    let found = crate::staged_app("be3.nowhere");
    assert_eq!(
        found.err(),
        Some("no staged app has the id be3.nowhere".to_owned())
    );
}
