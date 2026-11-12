use super::*;
use server::Chosen;

#[test]
fn a_build_server_named_by_hand_is_left_alone() {
    let generated = "[buck2_re_client]\nengine_address = 127.0.0.1:20728\ntls = false\n";
    let remote = "root//buck/platforms:execution_platforms";
    assert_eq!(server::chosen(remote, "", "", None), Chosen::Generated);
    assert_eq!(
        server::chosen(remote, "", "127.0.0.1:20728", Some(generated)),
        Chosen::Generated
    );
    assert_eq!(
        server::chosen(remote, "", "grpc://example.com:443", Some(generated)),
        Chosen::ServerByHand
    );
    assert_eq!(
        server::chosen(remote, "", "grpc://example.com:443", None),
        Chosen::ServerByHand
    );
    assert_eq!(
        server::chosen(
            "root//buck/platforms:execution_platforms_local",
            "",
            "grpc://example.com:443",
            Some(generated)
        ),
        Chosen::Here
    );
}
