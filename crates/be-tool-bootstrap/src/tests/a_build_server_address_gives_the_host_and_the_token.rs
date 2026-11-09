use super::*;

#[test]
fn a_build_server_address_gives_the_host_and_the_token() {
    assert_eq!(
        nativelink::parse_address("https://box-node-abc-8099.on.boat.dev/?_token=T0k3n\n"),
        Ok((
            "box-node-abc-8099.on.boat.dev".to_owned(),
            "T0k3n".to_owned()
        ))
    );
    // The token may come after other parameters.
    assert_eq!(
        nativelink::parse_address("https://host.example?a=1&_token=zz&b=2"),
        Ok(("host.example".to_owned(), "zz".to_owned()))
    );
    // Without a token there is none (a proxy adds the header).
    assert_eq!(
        nativelink::parse_address("https://host.example/"),
        Ok(("host.example".to_owned(), String::new()))
    );
    assert!(nativelink::parse_address("box.on.boat.dev").is_err());
    assert!(nativelink::parse_address("http://box.on.boat.dev").is_err());
    assert!(nativelink::parse_address("https://").is_err());
    assert!(nativelink::parse_address("https://host.example/x/y").is_err());
    assert!(nativelink::parse_address("https://host.example:8443/").is_err());
}
