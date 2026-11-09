use super::*;

#[test]
fn starting_a_stopped_sandbox_asks_first() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.test.click("boat.sandbox.bx_r4t8w3ne.start");
    boat.test.run();
    assert!(!boat.test.has_fetches(), "starting bills, so it asks first");
    boat.test.snapshot("starting_a_stopped_sandbox_asks_first");

    boat.test.click("boat.sandbox.bx_r4t8w3ne.confirm");
    boat.test.run();
    let resume = boat.fetch();
    assert_eq!(resume.request.method, "POST");
    assert_eq!(
        resume.request.url,
        "https://boat.dev/api/v1/sandboxes/bx_r4t8w3ne/resume"
    );
    boat.answer(&resume, 202, RESUMING);

    assert_eq!(
        boat.label("boat.sandbox.bx_r4t8w3ne.detail"),
        "Starting · small, 2 vCPU, 4 GB"
    );
    boat.test.snapshot("a_started_sandbox_shows_it_is_starting");

    boat.wait(Duration::from_secs(3));
    let sooner = boat.fetch();
    assert_eq!(sooner.request.url, LIST, "a start is followed more often");
}
