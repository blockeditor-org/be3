use super::*;

#[test]
fn a_refused_start_has_the_host_say_why() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.test.click("boat.sandbox.bx_r4t8w3ne.start");
    boat.test.click("boat.sandbox.bx_r4t8w3ne.confirm");
    let resume = boat.fetch();
    boat.answer(&resume, 402, PAYMENT_REQUIRED);

    assert_eq!(
        boat.test.take_reported_failures(),
        ["Your plan's compute hours are used up for this month."]
    );
    assert_eq!(
        boat.label("boat.sandbox.bx_r4t8w3ne.detail"),
        "Stopped · small, 2 vCPU, 4 GB"
    );
}
