use super::*;

#[test]
fn a_deployed_build_names_its_be_per_arch() {
    let json = r#"{"id":"abc","program_id":"be-host","terminal_hosts":[{"arch":"arm64","url":"https://be3.example/data/abc/be-aarch64-unknown-linux-gnu/be","sha256":"00ff"}]}"#;
    let metadata = Metadata::parse(json).unwrap();
    let arm = metadata.terminal_host("arm64").expect("arm64 is listed");
    assert_eq!(arm.sha256, "00ff");
    assert!(metadata.terminal_host("x86_64").is_none());

    let for_be = Metadata::parse(&metadata.for_be()).unwrap();
    assert_eq!(for_be.id, "abc");
    assert!(
        for_be.terminal_hosts.is_empty(),
        "be's own build-info names no be"
    );
    assert!(!metadata.for_be().contains("default_server"));
}
