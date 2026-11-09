use super::*;

#[test]
fn stopping_a_running_sandbox_stops_it() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.test.click("boat.sandbox.bx_k7m2p9qa.stop");
    boat.test.run();
    let stop = boat.fetch();
    assert_eq!(stop.request.method, "POST");
    assert_eq!(
        stop.request.url,
        "https://boat.dev/api/v1/sandboxes/bx_k7m2p9qa/stop"
    );
    assert!(boat.label("boat.sandbox.bx_k7m2p9qa.detail").starts_with("Stopping"));
}
