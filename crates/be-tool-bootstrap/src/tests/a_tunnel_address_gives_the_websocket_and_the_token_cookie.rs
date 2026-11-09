use super::*;

#[test]
fn a_tunnel_address_gives_the_websocket_and_the_token_cookie() {
    let parsed =
        nativelink::parse_tunnel_url("https://box-node-abc-8099.on.boat.dev/?_token=T0k3n\n");
    assert_eq!(
        parsed,
        Ok((
            "wss://box-node-abc-8099.on.boat.dev/".to_owned(),
            "_port_auth=T0k3n".to_owned()
        ))
    );
    // The token may come after other parameters, and the path is dropped.
    assert_eq!(
        nativelink::parse_tunnel_url("https://host.example/x/y?a=1&_token=zz&b=2"),
        Ok((
            "wss://host.example/x/y/".to_owned(),
            "_port_auth=zz".to_owned()
        ))
    );
    // Without a token there is no cookie; a plain http address stays unencrypted.
    assert_eq!(
        nativelink::parse_tunnel_url("http://127.0.0.1:8099"),
        Ok(("ws://127.0.0.1:8099/".to_owned(), String::new()))
    );
    assert!(nativelink::parse_tunnel_url("box.on.boat.dev").is_err());
    assert!(nativelink::parse_tunnel_url("ftp://box.on.boat.dev").is_err());
    assert!(nativelink::parse_tunnel_url("https://").is_err());
}
