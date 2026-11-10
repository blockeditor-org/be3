use super::*;

#[test]
fn a_build_server_epoch_is_read_only_from_its_own_answer() {
    assert_eq!(
        nativelink::parse_epoch(200, "63ff4bf183391d89\n"),
        "63ff4bf183391d89"
    );
    // A scheduler that serves no epoch: NativeLink answers the path itself.
    assert_eq!(nativelink::parse_epoch(404, "63ff4bf183391d89"), "");
    assert_eq!(nativelink::parse_epoch(200, ""), "");
    // Nothing that could break the line it is written into, or a relay's version.
    assert_eq!(nativelink::parse_epoch(200, "a b"), "");
    assert_eq!(nativelink::parse_epoch(200, "a\nb"), "");
    assert_eq!(nativelink::parse_epoch(200, &"a".repeat(65)), "");
}
