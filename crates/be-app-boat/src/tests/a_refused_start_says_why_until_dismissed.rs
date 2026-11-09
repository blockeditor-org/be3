use super::*;

#[test]
fn a_refused_start_says_why_until_dismissed() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.test.click("boat.sandbox.bx_r4t8w3ne.start");
    boat.test.run();
    boat.test.click("boat.sandbox.bx_r4t8w3ne.confirm");
    boat.test.run();
    let resume = boat.fetch();
    boat.answer(&resume, 402, PAYMENT_REQUIRED);

    assert_eq!(
        boat.label("boat.notice"),
        "Your plan's compute hours are used up for this month."
    );
    assert_eq!(
        boat.label("boat.sandbox.bx_r4t8w3ne.detail"),
        "Stopped · small, 2 vCPU, 4 GB"
    );

    boat.test.click("boat.notice.dismiss");
    boat.test.run();
    assert!(!boat.test.shown("boat.notice"));
}
