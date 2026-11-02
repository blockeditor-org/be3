use super::*;

#[test]
fn an_interrupted_host_loads_no_plugin() {
    let host = host();
    host.interrupter().interrupt();

    let loaded = host.load_bytes(guest("").as_bytes());

    assert_eq!(loaded.err(), Some(INTERRUPTED.to_owned()));
}
